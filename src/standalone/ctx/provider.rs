use std::{
    path::Path,
    sync::{Arc, OnceLock, atomic::AtomicU32},
};

use anyhow::{Error, anyhow};
use wasmtime::{AsContext, Caller, Engine, Extern, Linker, MemoryType, Module, SharedMemory};

use crate::standalone::{StandaloneView, ctx::WasmgrindStandaloneCtx};

pub struct StandaloneCtxProvider<T> {
    module: Module,
    memory_min: u32,
    memory_max: u32,
    tls_size: u32,
    tls_align: u32,
    linker: Arc<OnceLock<Linker<T>>>,
}

impl<T> StandaloneCtxProvider<T> {
    pub fn from_file<P: AsRef<Path>>(
        engine: &Engine,
        file: P,
    ) -> Result<(Self, walrus::Module), Error> {
        let mut module = walrus::Module::from_file(file)?;
        let provider = Self::from_walrus(engine, &mut module)?;
        Ok((provider, module))
    }

    pub fn from_binary(engine: &Engine, wasm: &[u8]) -> Result<(Self, walrus::Module), Error> {
        let mut module = walrus::Module::from_buffer(wasm)?;
        let provider = Self::from_walrus(engine, &mut module)?;
        Ok((provider, module))
    }

    pub fn from_walrus(engine: &Engine, module: &mut walrus::Module) -> Result<Self, Error> {
        wasmgrind_core::threadify::patch(module)?;

        let (memory_min, memory_max) = wasmgrind_core::threadify::get_shared_memory_size(module)?;

        let tls_size = wasmgrind_core::threadify::extract_tls_size(module)?;
        let tls_align = wasmgrind_core::threadify::extract_tls_align(module)?;
        let module = Module::from_binary(engine, &module.emit_wasm())?;

        Ok(Self {
            module,
            memory_min,
            memory_max,
            tls_size,
            tls_align,
            linker: Arc::new(OnceLock::new()),
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn engine(&self) -> &Engine {
        self.module.engine()
    }

    pub fn create_ctx(&self) -> WasmgrindStandaloneCtx {
        WasmgrindStandaloneCtx {
            module: self.module.clone(),
            tls_size: self.tls_size,
            tls_align: self.tls_align,
            next_tid: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn finalize(&self, linker: Linker<T>) -> Result<(), Error> {
        self.linker
            .set(linker)
            .map_err(|_| anyhow!("Linker has already been set for this provider!"))
    }
}

impl<T: StandaloneView + Clone + 'static> StandaloneCtxProvider<T> {
    pub fn add_to_linker(
        &self,
        linker: &mut Linker<T>,
        store: impl AsContext<Data = T>,
    ) -> Result<(), Error> {
        let closure_linker = self.linker.clone();
        let memory = SharedMemory::new(
            self.module.engine(),
            MemoryType::shared(self.memory_min, self.memory_max),
        )?;

        linker
            .define(
                store,
                WasmgrindStandaloneCtx::MEMORY_IMPORT_MODULE,
                WasmgrindStandaloneCtx::MEMORY_IMPORT_NAME,
                memory,
            )?
            .func_wrap(
                WasmgrindStandaloneCtx::MODULE_NAME,
                "clone_instance",
                move |mut caller: Caller<'_, T>,
                      tls_base_ptr: u32,
                      stack_ptr: u32,
                      tid_ptr: u32,
                      start_fn_ptr: u32,
                      start_fn_arg: u32| {
                    const GENERIC_ERROR_CODE: i32 = -1;
                    let data = caller.data().clone();
                    let ctx = data.ctx();
                    let linker = closure_linker.get().expect("Linker was not initialized!");

                    let memory = if let Some(Extern::SharedMemory(memory)) = linker.get(
                        &mut caller,
                        WasmgrindStandaloneCtx::MEMORY_IMPORT_MODULE,
                        WasmgrindStandaloneCtx::MEMORY_IMPORT_NAME,
                    ) {
                        memory
                    } else {
                        return GENERIC_ERROR_CODE;
                    };

                    let engine = caller.engine();
                    let mut store = wasmtime::Store::new(engine, data.clone());
                    let instance = match linker.instantiate(&mut store, &ctx.module) {
                        Ok(instance) => instance,
                        Err(_) => return GENERIC_ERROR_CODE,
                    };

                    let instance_entry = match instance.get_typed_func::<(u32, u32, u32, u32), ()>(
                        &mut store,
                        "__wasmgrind_instance_entry",
                    ) {
                        Ok(instance_entry) => instance_entry,
                        Err(_) => return GENERIC_ERROR_CODE,
                    };

                    let tid = ctx.next_available_tid();
                    let tid_ptr = match usize::try_from(tid_ptr) {
                        Ok(tid_ptr) => tid_ptr,
                        Err(_) => {
                            return GENERIC_ERROR_CODE;
                        }
                    };

                    if memory.data()[tid_ptr..].len() < std::mem::size_of::<u32>() {
                        return GENERIC_ERROR_CODE;
                    } else {
                        unsafe {
                            let native_tid_ptr = memory.data().as_ptr().add(tid_ptr);
                            std::ptr::write(native_tid_ptr.cast::<u32>().cast_mut(), tid);
                        };
                    }

                    std::thread::spawn(move || {
                        let panic_msg = format!("Child {tid} trapped!");
                        instance_entry
                            .call(
                                &mut store,
                                (start_fn_ptr, start_fn_arg, stack_ptr, tls_base_ptr),
                            )
                            .expect(&panic_msg);
                    });

                    0
                },
            )?
            .func_wrap(
                WasmgrindStandaloneCtx::MODULE_NAME,
                "get_tls_size",
                |caller: Caller<'_, T>| caller.data().ctx().tls_size,
            )?
            .func_wrap(
                WasmgrindStandaloneCtx::MODULE_NAME,
                "get_tls_align",
                |caller: Caller<'_, T>| caller.data().ctx().tls_align,
            )?
            .func_wrap(
                WasmgrindStandaloneCtx::MODULE_NAME,
                "exit",
                |_: Caller<'_, T>, exit_code: i32| {
                    panic!("Raw Error Code: {}", exit_code);

                    // We need this here to make the type checker happy.
                    // A non returning function does not implement any wasm-type
                    // as expected by wasmtime, so we have to return something
                    // although we know this function will not resume execution.
                    #[allow(unreachable_code)]
                    ()
                },
            )?;

        Ok(())
    }
}
