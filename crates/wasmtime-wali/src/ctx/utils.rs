pub mod mem {
    use std::{
        cell::UnsafeCell,
        collections::{BTreeMap, btree_map},
        ptr,
    };

    use wasmtime::Caller;

    /// A Wasm pointer in WebAssembly address space
    ///
    /// This struct only serves as a typechecking
    /// hint to avoid accidentally treating a random
    /// u32 integer as a WebAssembly pointer.
    #[repr(transparent)]
    pub struct WasmPtr(u32);

    impl WasmPtr {
        pub fn from_native_ptr<T>(rt_ctx: &mut Caller<'_, T>, native_ptr: NativePtr) -> WasmPtr {
            if native_ptr.0.is_null() {
                return Self::null();
            }

            let memory = super::exports::get_exported_memory(rt_ctx);
            let data_ptr = memory.data().as_ptr();
            let data_offset = unsafe { native_ptr.0.sub(data_ptr.addr()) };

            Self(
                u32::try_from(data_offset.addr())
                    .expect("Offset into linear memory should fit into WasmPtr size"),
            )
        }

        #[inline]
        fn null() -> Self {
            Self(0)
        }

        #[inline]
        pub fn raw(&self) -> u32 {
            self.0
        }
    }

    impl From<u32> for WasmPtr {
        fn from(value: u32) -> Self {
            Self(value)
        }
    }

    /// A Wasm pointer in native address space
    #[repr(transparent)]
    #[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
    pub struct NativePtr(*mut UnsafeCell<u8>);

    impl From<*mut UnsafeCell<u8>> for NativePtr {
        fn from(value: *mut UnsafeCell<u8>) -> Self {
            Self(value)
        }
    }

    impl NativePtr {
        pub fn from_wasm_ptr<T>(rt_ctx: &mut Caller<'_, T>, wasm_ptr: WasmPtr) -> Self {
            if wasm_ptr.0 == 0 {
                return Self(ptr::null_mut());
            }

            let memory = super::exports::get_exported_memory(rt_ctx);
            let data_ptr = memory.data().as_ptr().cast_mut();
            let data_offset =
                usize::try_from(wasm_ptr.0).expect("WasmPtr size should fit into NativePtr size");
            unsafe { Self(data_ptr.add(data_offset)) }
        }

        #[inline]
        pub fn addr(&self) -> usize {
            self.0.addr()
        }

        #[inline]
        pub fn raw<T>(&self) -> *mut T {
            self.0.cast()
        }
    }

    #[derive(Debug)]
    struct WasmMemoryBlock {
        base_address: NativePtr,
        size: usize,
        is_mapped: bool,
    }

    impl WasmMemoryBlock {
        pub fn unmapped(base_address: NativePtr, size: usize) -> Self {
            Self {
                base_address,
                size,
                is_mapped: false,
            }
        }

        pub fn mapped(base_address: NativePtr, size: usize) -> Self {
            Self {
                base_address,
                size,
                is_mapped: true,
            }
        }
    }

    #[derive(Debug)]
    pub struct MMapManager {
        blocks: BTreeMap<NativePtr, WasmMemoryBlock>,
    }

    /// # Safety
    ///
    /// We implement Send for MMapManager as it _manages_
    /// handles the NativePtr it retrieves when extending
    /// the WebAssembly linear memory.
    ///
    /// One important assumption is that pointers given
    /// to MMapManager in the [`MMapManager::track_mmap`]
    /// method will stay valid for the lifetime of MMapManager
    /// itself as there is no way to unregister the memory blocks
    /// (this is fine as WebAssembly memory can not be shrunk).
    ///
    /// MMapManager never accesses the raw
    /// pointers itself and only hands them out
    /// via [`MMapManager::track_mmap`]. The user of the
    /// API then needs to take care about handling the pointer
    /// as described in the safety section of the method.
    ///
    /// Furthermore, native pointers are only handed out exclusively
    /// by methods the require a mutable borrow of MMapManager. The
    /// contract between the [`MMapManager::track_mmap`] and
    /// [`MMapManager::track_munmap`] ensures that no native pointer
    /// is handed out to multiple threads as long as the API
    /// user adheres to the safety requirements.
    unsafe impl Send for MMapManager {}

    impl MMapManager {
        pub fn new() -> Self {
            Self {
                blocks: BTreeMap::new(),
            }
        }

        /// Registers an unmapped WebAssembly memory block
        fn register_block(&mut self, base_address: NativePtr, size: usize) -> &mut WasmMemoryBlock {
            if let Some((base, block)) = self.blocks.range(..&base_address).next_back()
                && base.addr() + block.size > base_address.addr()
            {
                panic!(
                    "New block would overlap with an existing block. This is an invalid operation!"
                );
            };

            if let Some((base, _)) = self.blocks.range(&base_address..).next()
                && base_address.addr() + size > base.addr()
            {
                panic!(
                    "New block would overlap with an existing block. This is an invalid operation!"
                );
            };

            match self.blocks.entry(base_address) {
                btree_map::Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(WasmMemoryBlock::unmapped(base_address, size))
                }
                btree_map::Entry::Occupied(occupied_entry) => {
                    // This is basically only the case if we insert a block with size 0.
                    // Otherwise, we would run into the above checks.
                    panic!(
                        "Block was already registered at address {:p}",
                        occupied_entry.key().0
                    )
                }
            }
        }

        fn find_first_fit(&mut self, length: usize) -> Option<&mut WasmMemoryBlock> {
            for block in self.blocks.values_mut() {
                if block.is_mapped {
                    continue;
                }

                if block.size < length {
                    continue;
                }

                let raw_base = block.base_address.addr();
                let page_aligned_base = raw_base.next_multiple_of(nc::PAGE_SIZE);
                let offset = page_aligned_base - raw_base;
                if block.size - offset < length {
                    continue;
                }

                return Some(block);
            }

            None
        }

        /// Find an unmapped region of specific length in WebAssembly linear memory
        ///
        /// This function uses a first-fit strategy to identify a matching block.
        ///
        /// # SAFETY
        /// The returned pointer is OS page aligned and it is guaranteed that at least
        /// `length` bytes starting from the pointers address are safe to write
        /// without interfering with other program parts.
        ///
        /// To free the memory described by the returned pointer. It must be unmapped
        /// via [`MMapManager::track_munmap`].
        pub unsafe fn track_mmap<F>(&mut self, length: usize, request_memory: F) -> NativePtr
        where
            F: FnOnce(usize) -> (NativePtr, usize),
        {
            let block = if let Some(slot) = self.find_first_fit(length) {
                slot
            } else {
                let (base_addr, len) = request_memory(length);
                self.register_block(base_addr, len)
            };

            let raw_base = block.base_address.addr();
            let page_aligned_base = raw_base.next_multiple_of(nc::PAGE_SIZE);
            let offset = page_aligned_base - raw_base;

            // Calculate the specs of the new free block.
            let free_base = page_aligned_base + length;
            let free_len = block.size - (free_base - raw_base);

            if offset > 0 {
                // Block was not OS page aligned.
                // There is now a bit of empty space before the actual mmaped area ...
                block.size = offset;

                // ... and we need to register a new mapped block.
                let mmap_ptr = NativePtr(page_aligned_base as *mut UnsafeCell<u8>);
                self.blocks
                    .insert(mmap_ptr, WasmMemoryBlock::mapped(mmap_ptr, length));
            } else {
                // Block was already OS page aligned.
                // We can work with the existing block.
                block.size = length;
                block.is_mapped = true;
            }

            if free_len > 0 {
                // If there is free space after the mmapped region. We add a free block to the block registry.
                let free_ptr = NativePtr(free_base as *mut UnsafeCell<u8>);
                self.blocks
                    .insert(free_ptr, WasmMemoryBlock::unmapped(free_ptr, free_len));
            }

            let page_aligned_ptr = page_aligned_base as *mut UnsafeCell<u8>;
            log::debug!(
                "Tracking 'mmap': handed out pointer {:p} for length {length}",
                page_aligned_ptr
            );
            NativePtr(page_aligned_ptr)
        }

        /// Remove all blocks that are zero-sized.
        fn cleanse(&mut self) {
            self.blocks.retain(|_, block| block.size > 0);
        }

        fn shrink_before(&mut self, base_address: &NativePtr) {
            if let Some((block_base_addr, block)) =
                self.blocks.range_mut(..base_address).next_back()
            {
                // Due to BTree ordering we know that block_base_addr < base_address
                if block_base_addr.addr() + block.size > base_address.addr() {
                    block.size = base_address.addr() - block_base_addr.addr();
                }
            };
        }

        fn shrink_after(&mut self, base_address: &NativePtr, length: usize) {
            for (block_base_addr, block) in self.blocks.range_mut(base_address..) {
                if base_address.addr() + length >= block_base_addr.addr() + block.size {
                    // The unmapped area contains the whole block
                    block.size = 0;
                } else if base_address.addr() + length > block_base_addr.addr() {
                    // The unmapped area contains the block partially.
                    //
                    // The blocks in the tree must not overlap
                    // so this is the last block that is affected by
                    // the unmap operation
                    let base = *block_base_addr;
                    let mut block = self
                        .blocks
                        .remove(&base)
                        .expect("Presence was verified by iterator");
                    let new_base_addr =
                        NativePtr((base_address.addr() + length) as *mut UnsafeCell<u8>);
                    // This will not overflow as we have verified above that:
                    //   base_address.addr() + length < block.base_address.addr() + block.size
                    let new_size = block.base_address.addr() + block.size - new_base_addr.addr();
                    block.base_address = new_base_addr;
                    block.size = new_size;
                    self.blocks.insert(new_base_addr, block);
                    break;
                }
            }
        }

        /// Mark a region in Wasm linear memory as unmapped
        ///
        /// # SAFETY
        /// Any memory blocks that have been previously mapped in the range [base_address, base_address + length)
        /// are considered as unmapped and pointers to this range will be handed out in future
        /// calls to [`MMapManager::track_mmap`]. The caller has to make sure that the program will not access memory in
        /// this range after calling this function.
        pub unsafe fn track_munmap(&mut self, base_address: NativePtr, length: usize) {
            log::debug!(
                "Tracking munmap: freeing all blocks starting from {:p} for length {length} ...",
                base_address.0
            );

            // If the unmapped area overlaps with the block starting before base_address we need to shrink the previous block
            self.shrink_before(&base_address);

            // We need to shrink all blocks in the unmapped range following base_address
            self.shrink_after(&base_address, length);

            // Clean all blocks that are zero sized ...
            self.cleanse();

            // ... and insert a new unmapped one of appropriate size.
            //
            // This should not panic as we removed or shrinked overlapping regions
            // If it does, this is a programming error.
            self.register_block(base_address, length);

            self.coalesce();
        }

        fn prepare_merge(&mut self) -> BTreeMap<NativePtr, usize> {
            let mut merged_blocks: BTreeMap<NativePtr, usize> = BTreeMap::new();
            let mut merged_base = None;
            let mut merged_len = 0;
            let mut needs_merge = false;
            for (base_addr, block) in self.blocks.iter_mut() {
                if block.is_mapped {
                    if let Some(base_addr) = merged_base {
                        if needs_merge {
                            merged_blocks.insert(base_addr, merged_len);
                        }

                        // Reset even if we didn't insert. The scenario:
                        // If we didnt reset the variables in all cases here,
                        // there could be a problem in the following situation:
                        // |----------|--------|----------|
                        // | UNMAPPED | MAPPED | UNMAPPED |
                        // |----------|--------|----------|
                        // The 2nd unmapped area would be resized because the variables
                        // still had the values from the 1st unmapped area.
                        merged_base = None;
                        merged_len = 0;
                        needs_merge = false;
                    } else {
                        // This case is a noop. It happens in the following cases:
                        // - The first block in the tree is mapped
                        // - Two mapped blocks are direct neigbors in the tree
                    }
                } else if let Some(merged_base_addr) = merged_base {
                    // This is the case where the current unmapped block follows
                    // another unmapped block IN THE TREE structure
                    if merged_base_addr.addr() + merged_len == base_addr.addr() {
                        // This is the case where the current unmapped block follows
                        // another unmapped block IN MEMORY
                        merged_len += block.size;
                        block.size = 0;
                        needs_merge = true;
                    } else {
                        // This is the case where there is room between two unmapped blocks IN MEMORY
                        log::warn!(
                            "We do not expect non-connected unmapped memory regions in WALI"
                        );

                        if needs_merge {
                            merged_blocks.insert(merged_base_addr, merged_len);
                        }

                        merged_base = None;
                        merged_len = 0;
                        needs_merge = false;
                    }
                } else {
                    // This branch is executed in two cases:
                    // - The first block in the tree is unmapped
                    // - This block is unmapped and follows a mapped block
                    merged_base = Some(*base_addr);
                    merged_len = block.size;
                }
            }

            if let Some(base_addr) = merged_base
                && needs_merge
            {
                // This is the case when two or more connected blocks at the
                // end of the iterator were unmapped
                merged_blocks.insert(base_addr, merged_len);
            }

            merged_blocks
        }

        fn execute_merge(&mut self, merged_blocks: BTreeMap<NativePtr, usize>) {
            for (merged_base, merged_len) in merged_blocks {
                let block = self
                    .blocks
                    .get_mut(&merged_base)
                    .expect("Presence was verified in 'prepare_merge'");
                block.size = merged_len
            }
        }

        pub fn coalesce(&mut self) {
            // Find connected areas of distinct unmapped blocks ...
            let merged_blocks = self.prepare_merge();

            // ... clean all blocks that are zero sized ...
            self.cleanse();

            // ... and resize each starting block of the cleaned areas
            // such that it takes up the whole unmapped area.
            self.execute_merge(merged_blocks);
        }
    }
}

pub mod exports {
    use wasmtime::{
        AsContext, AsContextMut, Caller, Extern, Func, Instance, SharedMemory, TypedFunc,
    };

    use crate::ctx::WaliCtxInner;

    fn get_ifp_export(
        export: Option<Extern>,
        store: impl AsContext,
    ) -> TypedFunc<u32, Option<Func>> {
        if let Some(ext) = export {
            if let Extern::Func(func) = ext {
                match func.typed(store) {
                    Ok(func) => func,
                    Err(e) => panic!(
                        "Wasmtime WALI depends on an export '{}' with type 'i32 -> funcref'.\nError: {e}",
                        WaliCtxInner::GET_INDIRECT_FUNC_EXPORT_NAME
                    ),
                }
            } else {
                panic!(
                    "Wasmtime WALI depends on an export '{}' that must be a function.",
                    WaliCtxInner::GET_INDIRECT_FUNC_EXPORT_NAME
                )
            }
        } else {
            panic!(
                "Wasmtime WALI depends on an export '{}'.",
                WaliCtxInner::GET_INDIRECT_FUNC_EXPORT_NAME
            )
        }
    }

    pub fn get_ifp_from_caller<T>(rt_ctx: &mut Caller<'_, T>) -> TypedFunc<u32, Option<Func>> {
        get_ifp_export(
            rt_ctx.get_export(WaliCtxInner::GET_INDIRECT_FUNC_EXPORT_NAME),
            rt_ctx,
        )
    }

    pub fn get_ifp_from_instance(
        inst: &Instance,
        mut store: impl AsContextMut,
    ) -> TypedFunc<u32, Option<Func>> {
        get_ifp_export(
            inst.get_export(&mut store, WaliCtxInner::GET_INDIRECT_FUNC_EXPORT_NAME),
            store,
        )
    }

    pub fn get_exported_memory<T>(rt_ctx: &mut Caller<'_, T>) -> SharedMemory {
        if let Some(export) = rt_ctx.get_export(WaliCtxInner::MEMORY_EXPORT_NAME) {
            if let Extern::SharedMemory(memory) = export {
                memory
            } else {
                panic!(
                    "Export '{}' of a WALI module must be a shared linear memory.",
                    WaliCtxInner::MEMORY_EXPORT_NAME
                )
            }
        } else {
            panic!(
                "WALI module must export linear memory via '{}'.",
                WaliCtxInner::MEMORY_EXPORT_NAME
            )
        }
    }
}

pub mod signal {
    use std::{
        collections::HashMap,
        ffi::c_int,
        mem::MaybeUninit,
        sync::{
            OnceLock,
            atomic::{AtomicU64, Ordering},
        },
    };

    use anyhow::{Error, bail};
    use wasmtime::{Engine, StoreContextMut, TypedFunc, UpdateDeadline};

    use crate::{WaliView, ctx::WaliCtxInner};

    // FIXME:
    // Using those two static singletons here prevents
    // users of WaliCtx of running Wasm modules multiple times
    // or with different engines in a single program run.
    //
    // This is not ideal and should probably be revised.
    // If using this library only from the CLI this should
    // not be a problem.

    fn engine_singleton() -> &'static OnceLock<Engine> {
        static WALI_ENGINE: OnceLock<Engine> = OnceLock::new();
        &WALI_ENGINE
    }

    pub fn initialize_engine(engine: &Engine) -> &'static Engine {
        engine_singleton().get_or_init(|| engine.clone())
    }

    pub fn get_engine() -> Option<&'static Engine> {
        engine_singleton().get()
    }

    fn pending_signals() -> &'static AtomicU64 {
        static PENDING_SIGNALS: OnceLock<AtomicU64> = OnceLock::new();
        PENDING_SIGNALS.get_or_init(|| AtomicU64::new(0))
    }

    pub extern "C" fn wali_sigact_handler(signo: c_int) {
        pending_signals().fetch_or(1 << signo, Ordering::SeqCst);
        if let Some(engine) = get_engine() {
            for _ in 0..WaliCtxInner::SIGNAL_POLL_EPOCH {
                // Increment epochs such that it will yield to the signal poll function
                // as soon as possible.
                engine.increment_epoch();
            }
            log::debug!(
                "Signal Handler: Incremented epochs by {}.",
                WaliCtxInner::SIGNAL_POLL_EPOCH
            );
        } else {
            log::warn!(
                "Signal Handler: Could not increment Wasmtime epoch. No static engine initialized!"
            );
        }
    }

    pub fn signal_poll_callback<T: WaliView>()
    -> impl FnMut(StoreContextMut<T>) -> Result<UpdateDeadline, Error> + Send + Sync + 'static {
        |mut store_ctx| {
            // We clone the Arc here as we need Caller later
            let data = store_ctx.data().clone();
            let wali_ctx = data.ctx();
            let sigtable = match wali_ctx.0.sigtable.lock() {
                Ok(guard) => guard,
                Err(e) => bail!("Sigtable lock was poisoned in signal_poll_callback: {e}"),
            };

            let mut signo = MaybeUninit::uninit();
            match pending_signals().fetch_update(Ordering::SeqCst, Ordering::SeqCst, |signals| {
                if signals != 0 {
                    let idx = signals.trailing_zeros() + 1;
                    signo.write((idx as i32) - 1); // Cast is fine. Idx may be at most 64
                    Some(signals & !((1 << idx) - 1))
                } else {
                    None
                }
            }) {
                Ok(prev) => {
                    log::debug!("Signal Poll did find a signal. Prev value {prev:x}");
                    // If we return Some in the fetch_update, we have written the signo before.
                    // So, this is safe.
                    let signo = unsafe { signo.assume_init() };
                    if let Some(sighandler) = sigtable.get_handler_func(signo) {
                        sighandler.call(&mut store_ctx, signo)?;
                        log::debug!("Handled signal with number: {signo}");
                    } else {
                        panic!("No signal handler was registered for signal {signo}")
                    }
                }
                Err(prev) => log::debug!("Signal Poll did not find a signal. Prev value {prev:x}"),
            }

            Ok(UpdateDeadline::Continue(WaliCtxInner::SIGNAL_POLL_EPOCH))
        }
    }

    struct WaliSigEntry {
        /// The Wasmtime function instance representing the signal handler
        function: TypedFunc<i32, ()>,
        /// The index in the 0th table of the WebAssembly module
        /// This is the table-entry that holds the signal handler funcref
        function_table_idx: u32,
        // function_idx: u32, => Present in the reference implementation to patch AotFunctionInstance in case of reassignment. Dont think we need this.
    }

    impl WaliSigEntry {
        fn new(function: TypedFunc<i32, ()>, function_table_idx: u32) -> Self {
            Self {
                function,
                function_table_idx,
            }
        }
    }

    pub struct SigTable {
        table: HashMap<i32, WaliSigEntry>,
    }

    impl SigTable {
        pub fn new() -> Self {
            Self {
                table: HashMap::new(),
            }
        }

        pub fn update(
            &mut self,
            signo: i32,
            function: TypedFunc<i32, ()>,
            function_table_idx: u32,
        ) {
            self.table
                .insert(signo, WaliSigEntry::new(function, function_table_idx));
        }

        pub fn get_handler_table_idx(&self, signo: i32) -> Option<u32> {
            self.table.get(&signo).map(|entry| entry.function_table_idx)
        }

        pub fn get_handler_func(&self, signo: i32) -> Option<&TypedFunc<i32, ()>> {
            self.table.get(&signo).map(|entry| &entry.function)
        }
    }
}
