use std::{cell::UnsafeCell, ffi::c_void};

use wasmtime::Caller;

use crate::{
    WaliResult, WaliView,
    ctx::impls::SyscallResult,
    ctx::utils::{
        exports::get_exported_memory,
        mem::{NativePtr, WasmPtr},
    },
};

#[inline]
pub fn wali_munmap<T: WaliView>(
    mut caller: Caller<'_, T>,
    addr: WasmPtr,
    len: u32,
) -> WaliResult<SyscallResult<()>> {
    let native_ptr = NativePtr::from_wasm_ptr(&mut caller, addr);
    log::debug!(
        "SYS_munmap -- addr: {:p}, len:  {len}",
        native_ptr.raw::<UnsafeCell<u8>>()
    );

    let ctx = caller.data().ctx();
    log::debug!("locking mmap manager");
    let mut mmap_mgr = ctx
        .0
        .mmap_lock
        .lock()
        .expect("Another thread panicked while holding the mmap lock");
    log::debug!("locked mmap manager");

    let munmap_len = usize::try_from(len).expect("munmap 'len' arg needs to fit in NativePtr size");
    let retval = unsafe {
        match rustix::mm::mmap_anonymous(
            native_ptr.raw(),
            munmap_len,
            rustix::mm::ProtFlags::empty(),
            rustix::mm::MapFlags::PRIVATE | rustix::mm::MapFlags::FIXED,
        ) {
            Ok(_) => {
                mmap_mgr.track_munmap(native_ptr, munmap_len);
                Ok(())
            }
            Err(e) => match e {
                rustix::io::Errno::INVAL => Err(nc::EINVAL),
                rustix::io::Errno::NOMEM => Err(nc::ENOMEM),
                e => panic!("Could not acquire the old mapping after unmap. Error: {e}"),
            },
        }
    };

    drop(mmap_mgr); // Drop exlicitly to emphasize that we are holding the lock until OS memory is actually unmapped successfully.
    log::debug!("unlocked mmap manager");
    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_mremap<T: WaliView>(
    mut caller: Caller<'_, T>,
    old_address: WasmPtr,
    old_size: u32,
    new_size: u32,
    _flags: u32,
    _new_address: WasmPtr,
) -> WaliResult<SyscallResult<WasmPtr>> {
    // NOTE: We ignore the 'new_address' and 'flags' argument to the syscall because we need to ensure
    // that the memory is mapped into the Wasm linear memory address space
    let old_mmap_addr = NativePtr::from_wasm_ptr(&mut caller, old_address);
    log::debug!(
        "SYS_mremap -- old_addr: {:p}, old_len: {old_size}, new_len: {new_size}",
        old_mmap_addr.raw::<UnsafeCell<u8>>()
    );

    let ctx = caller.data().ctx().clone();
    log::debug!("locking mmap manager");
    let mut mmap_mgr = ctx
        .0
        .mmap_lock
        .lock()
        .expect("Another thread panicked while holding the mmap lock");
    log::debug!("locked mmap manager");

    let new_len =
        usize::try_from(new_size).expect("mremap new_size arg must fit into NativePtr size");
    let new_mmap_addr = unsafe {
        mmap_mgr.track_mmap(new_len, |requested_len: usize| {
            grow_wasm_memory(&mut caller, requested_len)
        })
    };

    // It is now safe to mmap 'requested_len' bytes at 'native_aligned_data_end'
    // because we have grown the Wasm linear memory accordingly.
    //
    // We control the LinearMemory implementation so we know
    // that the memory will not relocate and that we can safely override
    // the existing mapping at the specified address.
    let old_len =
        usize::try_from(old_size).expect("mremap old_size arg must fit into NativePtr size");
    let retval = unsafe {
        nc::mremap(
            old_mmap_addr.raw(),
            old_len,
            new_len,
            nc::MREMAP_FIXED | nc::MREMAP_MAYMOVE,
            new_mmap_addr.raw::<c_void>(),
        )
    };

    unsafe {
        match retval {
            Ok(_) => {
                // IMPORTANT:
                //
                // If the new mapping fails here this means that a slot in the Wasm linear
                // memory is now open to be occupied by other threads in the process
                // breaking sandboxing guarantees of the Wasm memory model.
                //
                // Until we have a solution for this we simply treat this as a fatal error.
                match rustix::mm::mmap_anonymous(
                    old_mmap_addr.raw(),
                    old_len,
                    rustix::mm::ProtFlags::empty(),
                    rustix::mm::MapFlags::PRIVATE | rustix::mm::MapFlags::FIXED_NOREPLACE,
                ) {
                    Ok(free_mmap_addr) => {
                        assert_eq!(
                            free_mmap_addr,
                            old_mmap_addr.raw(),
                            "Kernel did fall back to a non-fixed mapping. Could not acquire the old mapping after remap."
                        );
                        mmap_mgr.track_munmap(NativePtr::from(old_mmap_addr.raw()), old_len);
                    }
                    Err(e) => match e {
                        rustix::io::Errno::EXIST => panic!(
                            "Another thread has aquired a mapping in the Wasm linear memory!"
                        ),
                        e => panic!("Could not acquire the old mapping after remap. Error: {e}"),
                    },
                }
            }
            Err(_) => mmap_mgr.track_munmap(new_mmap_addr, new_len),
        }
    }

    drop(mmap_mgr); // We hold the lock until the syscalls are finished.
    log::debug!("unlocked mmap manager");

    let retval = SyscallResult::from(retval).map(|mmap_ptr| {
        WasmPtr::from_native_ptr(
            &mut caller,
            mmap_ptr.cast::<UnsafeCell<u8>>().cast_mut().into(),
        )
    });

    caller.data().ctx().return_or_exit(retval)
}

#[inline]
pub fn wali_mprotect<T: WaliView>(
    mut caller: Caller<'_, T>,
    addr: WasmPtr,
    size: u32,
    prot: i32,
) -> WaliResult<SyscallResult<()>> {
    let retval = unsafe {
        nc::mprotect(
            NativePtr::from_wasm_ptr(&mut caller, addr).raw(),
            usize::try_from(size).expect("mprotect 'size' arg needs to fit in NativePtr size"),
            prot,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_brk<T: WaliView>(caller: Caller<'_, T>, _addr: u32) -> WaliResult<i64> {
    log::debug!("brk syscall is a NOP in Wasm");
    caller.data().ctx().return_or_exit(0)
}

#[inline]
pub fn wali_mmap<T: WaliView>(
    mut caller: Caller<'_, T>,
    _addr: u32,
    len: u32,
    prot: i32,
    flags: i32,
    fd: i32,
    offset: i64,
) -> WaliResult<SyscallResult<WasmPtr>> {
    // NOTE: We ignore the 'addr' argument to the syscall because we need to ensure
    // that the memory is mapped into the Wasm linear memory address space
    let ctx = caller.data().ctx().clone();
    log::debug!("SYS_mmap -- len:  {len}");
    log::debug!("locking mmap manager");
    let mut mmap_mgr = ctx
        .0
        .mmap_lock
        .lock()
        .expect("Another thread panicked while holding the mmap lock");
    log::debug!("locked mmap manager");

    let requested_len = usize::try_from(len).expect("mmap length arg must fit into NativePtr size");
    let mmap_base_ptr = unsafe {
        mmap_mgr.track_mmap(requested_len, |requested_len: usize| {
            grow_wasm_memory(&mut caller, requested_len)
        })
    };

    // It is now safe to mmap 'requested_len' bytes at 'native_aligned_data_end'
    // because we have grown the Wasm linear memory accordingly.
    //
    // We control the LinearMemory implementation so we know
    // that the memory will not relocate and that we can safely override
    // the existing mapping at the specified address.

    let retval = unsafe {
        nc::mmap(
            mmap_base_ptr.raw::<c_void>(),
            requested_len,
            prot,
            flags | nc::MAP_FIXED,
            fd,
            offset as isize,
        )
    };

    if retval.is_err() {
        unsafe { mmap_mgr.track_munmap(mmap_base_ptr, requested_len) }
    }

    drop(mmap_mgr); // We hold the lock until the syscall is finished.                
    log::debug!("unlocked mmap manager");

    let retval = SyscallResult::from(retval).map(|mmap_ptr| {
        WasmPtr::from_native_ptr(
            &mut caller,
            mmap_ptr.cast::<UnsafeCell<u8>>().cast_mut().into(),
        )
    });

    caller.data().ctx().return_or_exit(retval)
}

fn grow_wasm_memory<T>(caller: &mut Caller<'_, T>, requested_len: usize) -> (NativePtr, usize) {
    let memory = get_exported_memory(caller);
    let native_page_size = nc::PAGE_SIZE;
    let wasm_page_size = usize::try_from(memory.page_size())
        .expect("Wasm memory page size must fit into NativePtr size");

    let data_end_addr = unsafe { memory.data().as_ptr().add(memory.data_size()).addr() };
    let native_aligned_data_end = data_end_addr.next_multiple_of(native_page_size);
    let wasm_aligned_data_end =
        (native_aligned_data_end + requested_len).next_multiple_of(wasm_page_size);

    let wasm_delta_in_bytes = wasm_aligned_data_end - data_end_addr;
    assert_eq!(
        wasm_delta_in_bytes % wasm_page_size,
        0,
        "Required Wasm memory space should be aliged by Wasm memory page size"
    );
    let requested_wasm_pages = wasm_delta_in_bytes / wasm_page_size;
    let delta = u64::try_from(requested_wasm_pages)
        .expect("Could not convert number of requested Wasm pages from 'usize' to 'u64'");
    memory.grow(delta).expect("Could not grow Wasm memory");

    log::debug!(
        "Growing Wasm Memory - DataEnd Addr: {:x}; Native Aligned DataEnd Addr: {:x}; New (wasm aligned) DataEnd Addr: {:x}; Grown Pages: {}; Mapped Len: {}",
        data_end_addr,
        native_aligned_data_end,
        wasm_aligned_data_end,
        requested_wasm_pages,
        requested_len
    );

    (
        NativePtr::from(data_end_addr as *mut UnsafeCell<u8>),
        wasm_delta_in_bytes,
    )
}
