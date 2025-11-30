use std::{ffi::c_void, ptr::null_mut};

use rustix::mm::{MapFlags, MprotectFlags, ProtFlags, mmap_anonymous, mprotect};
use wasmtime::{LinearMemory, MemoryCreator};

struct WaliMemory {
    mmap_ptr: usize,
    mmap_size: usize,
    guard_size: usize,
    mmap_accessible: usize,
    mem_len: usize,
    maximum: Option<usize>,
}

impl WaliMemory {
    /// Creates a new WALI Linear Memory
    ///
    /// This function expects the `maximum` and `guard_size` arguments
    /// to be aligned by the OS page size and may panic if they are not.
    fn new(minimum: usize, reserved: usize, guard_size: usize, maximum: Option<usize>) -> Self {
        let os_page_size = rustix::param::page_size();

        // Calculate size of memory the program will need at maximum
        let mmap_size = reserved + guard_size;
        assert_eq!(
            mmap_size % os_page_size,
            0,
            "Either `maximum` or `guard_size` was not aligned to OS page size!"
        );

        let mmap_ptr = unsafe {
            // The complete mmapped region needs to be zero-initialized and neither WRITE, READ nor EXECUTABLE
            // so out-of-bounds accesses to WebAssembly memory will result in an OS trap at runtime.
            mmap_anonymous(null_mut(), mmap_size, ProtFlags::empty(), MapFlags::PRIVATE)
                .expect("Could not initialize WALI LinearMemory due to failed mmap.")
        };

        // An initial chunk of memory is expected to be acessible to the Wasm program.
        // Therefore, we need to mark it as readable/writable.
        //
        // Linux potentially operates on a different page size than WebAssembly,
        // so the `length` visible to wasm programs will not necessarily match
        // the `length` made accessibly by the OS.
        //
        // This is not a problem as long as the WebAssembly memory uses page sizes
        // that are multiples of the OS page size.
        let accessible = minimum.next_multiple_of(os_page_size);
        unsafe {
            mprotect(
                mmap_ptr,
                accessible,
                MprotectFlags::READ | MprotectFlags::WRITE,
            )
            .expect("Failed to make WALI Linear Memory readable/writable using mprotect.")
        };

        Self {
            mmap_ptr: mmap_ptr as usize,
            mmap_size,
            guard_size,
            mmap_accessible: accessible,
            mem_len: minimum,
            maximum,
        }
    }
}

unsafe impl LinearMemory for WaliMemory {
    fn byte_size(&self) -> usize {
        self.mem_len
    }

    fn byte_capacity(&self) -> usize {
        self.mmap_size - self.guard_size
    }

    fn grow_to(&mut self, new_size: usize) -> wasmtime::Result<()> {
        let os_page_size = rustix::param::page_size();

        log::debug!(
            "WaliMemory: Received request to grow from size {} to size {}",
            self.mem_len,
            new_size
        );
        // We can only grow in chunks of OS pages
        let aligned_new_size = new_size.next_multiple_of(os_page_size);

        if aligned_new_size > self.byte_capacity() {
            // TODO: Investigate the consequences for user-space programs
            // ==> What do we need to assure here such that their assumptions hold true
            //     upon relocating the Linear Memory in Host Address Space
            // ==> We would need to keep track of memory-management related operations
            //     and revert them for the existing space (via unmap I guess) and
            //     reapply them to the newly allocated space.
            // ==> We probably need shared-static bookkeeping between WaliCtx functions
            //     and WaliMemory, but this needs more thinking
            // ANYWAY: WALI relies on shared memory at the moment which means the
            //         max linear memory size is always known AND for unshared memory
            //         (in case of multiple defined memories with local ones amongst them)
            //         we set the default mapping to 4GiB which is the maximum for 32-bit
            //         memory (we dont support 64-bit memory yet).
            //         Therefore, we should never run into this for now.
            unimplemented!("Relocating the WALI Linear Memory is not yet implemented.")
        } else {
            // Make sure we are safe to grow the available memory here
            assert!(new_size <= self.byte_capacity());
            // This check is necessary as the capacity may exceed the maximum size of
            // the linear memory (because we align the reserved memory region by OS page size).
            assert!(self.maximum.is_none_or(|max| new_size <= max));

            // This check is for cases where the page size of the WebAssembly memory is
            // smaller than the OS page size. In this case we might have already
            // made the necessary memory regions accessible in an earlier invocation of `grow_to`.
            if let Some(requested) = aligned_new_size.checked_sub(self.mmap_accessible)
                && requested > 0
            {
                // This invariant should always hold because `self.mmap_accessible` is aligned to the OS page size
                // and `aligned_new_size` was aligned to it above.
                //
                // z = x - y = (c1 * page_size) - (c2 * page_size) = (c1 - c2) * page_size
                //
                // `self.mmap_accessible` is aligned to the OS page size during initialization of this Linear Memory
                // and the only other place it is changed is in this method where the below check is performed to
                // guarantee that the invariant continues to hold.
                assert_eq!(
                    requested % os_page_size,
                    0,
                    "`requested` length of memory region to use in mprotect was not aligned to OS page size"
                );
                unsafe {
                    // Find the memory address where the unaccessible (but already mapped) memory region starts.
                    let start = (self.mmap_ptr as *mut u8).add(self.mmap_accessible);
                    // Memory has already been mmapped in the initialization.
                    // We only need to make the requested chunk of new memory readable/writable now,
                    // so memory accesses to this region made by Wasmtime do not result in OS traps anymore.
                    mprotect(
                        start as *mut c_void,
                        requested,
                        MprotectFlags::READ | MprotectFlags::WRITE,
                    )
                    .expect("Failed to make WALI Linear Memory readable/writable using mprotect.")
                }

                self.mmap_accessible += requested;
            }
        }

        // We can now safely set the length of the linear memory to the requested value
        self.mem_len = new_size;

        Ok(())
    }

    fn as_ptr(&self) -> *mut u8 {
        self.mmap_ptr as *mut u8
    }
}

pub struct WaliMemoryCreator;

impl WaliMemoryCreator {
    const DEFAULT_MEMORY_RESERVATION: usize = 0xFFFFFFFF; // 4 GiB: maximum 32bit memory address
}

unsafe impl MemoryCreator for WaliMemoryCreator {
    fn new_memory(
        &self,
        ty: wasmtime::MemoryType,
        minimum: usize,
        maximum: Option<usize>,
        reserved_size_in_bytes: Option<usize>,
        guard_size_in_bytes: usize,
    ) -> wasmtime::Result<Box<dyn wasmtime::LinearMemory>, String> {
        if ty.is_64() {
            return Err(
                "The WALI MemoryCreator does not support 64bit WebAssembly memories.".to_string(),
            );
        }

        let os_page_size = rustix::param::page_size();
        assert_eq!(
            guard_size_in_bytes % os_page_size,
            0,
            "`guard_size_in_bytes` was not aligned to OS page size"
        );

        let reserved = if let Some(reserved_size) = reserved_size_in_bytes {
            assert_eq!(
                reserved_size % os_page_size,
                0,
                "`reserved_size_in_bytes` was not aligned to OS page size"
            );
            reserved_size
        } else if let Some(max_size) = maximum {
            let aligned_maximum = max_size.next_multiple_of(os_page_size);
            assert_eq!(
                aligned_maximum % os_page_size,
                0,
                "`aligned_maximum` should be aligned to OS page size"
            );
            aligned_maximum
        } else {
            let aligned_default_reservation =
                Self::DEFAULT_MEMORY_RESERVATION.next_multiple_of(os_page_size);
            assert_eq!(
                aligned_default_reservation % os_page_size,
                0,
                "`aligned_default_reservation` should be aligned to OS page size"
            );
            aligned_default_reservation
        };

        Ok(Box::new(WaliMemory::new(
            minimum,
            reserved,
            guard_size_in_bytes,
            maximum,
        )))
    }
}
