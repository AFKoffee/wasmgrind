use std::{
    ffi::CString,
    path::Path,
    str::FromStr,
    sync::{Arc, OnceLock, atomic::Ordering},
};

use anyhow::{Error, bail};
use wasmtime::{Caller, Config, Engine, InstanceAllocationStrategy, Linker, Module, Store};

use crate::{
    WaliTrap, WaliView,
    ctx::{WaliCtx, WaliCtxInner, impls, utils},
    memory::WaliMemoryCreator,
};

pub trait ProviderState {}

pub trait HasLinker<T> {
    fn linker(&self) -> Arc<OnceLock<Linker<T>>>;
}

pub struct Empty<T> {
    linker: Arc<OnceLock<Linker<T>>>,
}

impl<T> ProviderState for Empty<T> {}
impl<T> HasLinker<T> for Empty<T> {
    fn linker(&self) -> Arc<OnceLock<Linker<T>>> {
        self.linker.clone()
    }
}

pub struct Configured<T> {
    linker: Arc<OnceLock<Linker<T>>>,
    engine: Engine,
}
impl<T> ProviderState for Configured<T> {}
impl<T> HasLinker<T> for Configured<T> {
    fn linker(&self) -> Arc<OnceLock<Linker<T>>> {
        self.linker.clone()
    }
}

pub struct Initialized<T> {
    linker: Arc<OnceLock<Linker<T>>>,
    engine: Engine,
    main_module: Module,
    thread_module: Module,
}
impl<T> ProviderState for Initialized<T> {}
impl<T> HasLinker<T> for Initialized<T> {
    fn linker(&self) -> Arc<OnceLock<Linker<T>>> {
        self.linker.clone()
    }
}

pub struct WaliCtxProvider<S: ProviderState> {
    state: S,
}

impl<T> WaliCtxProvider<Empty<T>> {
    pub fn new() -> Self {
        Self {
            state: Empty {
                linker: Arc::new(OnceLock::new()),
            },
        }
    }

    pub fn with_config(self, config: &mut Config) -> Result<WaliCtxProvider<Configured<T>>, Error> {
        Ok(WaliCtxProvider {
            state: Configured {
                linker: self.state.linker,
                engine: Engine::new(Self::patch_config(config))?,
            },
        })
    }

    fn with_default_config(self) -> Result<WaliCtxProvider<Configured<T>>, Error> {
        self.with_config(&mut Config::new())
    }

    pub fn with_file<P: AsRef<Path>>(
        self,
        file: P,
    ) -> Result<WaliCtxProvider<Initialized<T>>, Error> {
        self.with_default_config()?.with_file(file)
    }

    pub fn with_buffer(self, wasm: &[u8]) -> Result<WaliCtxProvider<Initialized<T>>, Error> {
        self.with_default_config()?.with_buffer(wasm)
    }

    pub fn with_walrus(
        self,
        module: &mut walrus::Module,
    ) -> Result<WaliCtxProvider<Initialized<T>>, Error> {
        self.with_default_config()?.with_walrus(module)
    }
}

impl<T> Default for WaliCtxProvider<Empty<T>> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> WaliCtxProvider<Configured<T>> {
    pub fn engine(&self) -> &Engine {
        &self.state.engine
    }

    pub fn from_config(config: &mut Config) -> Result<Self, Error> {
        WaliCtxProvider::new().with_config(config)
    }

    pub fn with_file<P: AsRef<Path>>(
        self,
        file: P,
    ) -> Result<WaliCtxProvider<Initialized<T>>, Error> {
        Self::with_walrus(self, &mut walrus::Module::from_file(file)?)
    }

    pub fn with_buffer(self, wasm: &[u8]) -> Result<WaliCtxProvider<Initialized<T>>, Error> {
        Self::with_walrus(self, &mut walrus::Module::from_buffer(wasm)?)
    }

    pub fn with_walrus(
        self,
        module: &mut walrus::Module,
    ) -> Result<WaliCtxProvider<Initialized<T>>, Error> {
        Self::patch_binary(module)?;

        let engine = self.state.engine;

        let main_module = Module::new(&engine, module.emit_wasm())?;

        // We assume only one memory is present here
        let memory_id = module.get_memory_id()?;
        let import_id = module
            .imports
            .add("wali", "memory", walrus::ImportKind::Memory(memory_id));
        module.memories.get_mut(memory_id).import = Some(import_id);
        let thread_module = Module::new(&engine, module.emit_wasm())?;

        Ok(WaliCtxProvider {
            state: Initialized {
                linker: self.state.linker,
                engine,
                main_module,
                thread_module,
            },
        })
    }
}

impl<T> WaliCtxProvider<Initialized<T>> {
    pub fn from_file<P: AsRef<Path>>(self, file: P) -> Result<Self, Error> {
        WaliCtxProvider::new().with_file(file)
    }

    pub fn from_buffer<P: AsRef<Path>>(self, wasm: &[u8]) -> Result<Self, Error> {
        WaliCtxProvider::new().with_buffer(wasm)
    }

    pub fn from_walrus(module: &mut walrus::Module) -> Result<Self, Error> {
        WaliCtxProvider::new().with_walrus(module)
    }

    pub fn engine(&self) -> &Engine {
        &self.state.engine
    }

    pub fn module(&self) -> &Module {
        &self.state.main_module
    }

    pub fn create_ctx(&self, args: Vec<String>) -> Result<WaliCtx, Error> {
        let argv = args
            .into_iter()
            .map(|arg| CString::from_str(&arg).map_err(Error::from))
            .collect::<Result<Vec<CString>, Error>>()?;

        Ok(WaliCtx(Arc::new(WaliCtxInner::new(
            &self.state.engine,
            self.state.thread_module.clone(),
            argv,
        ))))
    }
}

impl<T: WaliView> WaliCtxProvider<Initialized<T>> {
    pub fn run(self, store: &mut Store<T>, mut linker: Linker<T>) -> Result<(), Error> {
        // First, we need to set up the signal polling mechanism on the given store
        store.epoch_deadline_callback(utils::signal::signal_poll_callback());
        store.set_epoch_deadline(WaliCtxInner::SIGNAL_POLL_EPOCH);

        // Then we instantiate the module for the MAIN thread
        let instance = linker.instantiate(&mut *store, &self.state.main_module)?;

        // We retrieve the memory to serve as import for the CHILD threads.
        let memory = instance
            .get_shared_memory(&mut *store, "memory")
            .expect("Shared memory export needs to be present");

        // We register the shared memory with the linker
        linker.define(&mut *store, "wali", "memory", memory)?;

        // We initialize the linker inside the WaliCtx
        match self.state.linker.set(linker) {
            Ok(()) => (),
            Err(_) => bail!("Linker has already been initialized on the given WaliCtx!"),
        }

        // Retrieve the '_start' function of the WebAssembly binary.
        let wali_start = instance.get_typed_func::<(), ()>(&mut *store, "_start")?;

        // Increment the thread count to signal that an additional thread runs inside the WaliCtx
        store
            .data()
            .ctx()
            .0
            .thread_count
            .fetch_add(1, Ordering::AcqRel);

        // NOW we start the MAIN thread. This is important!
        // We can only start the main thread once the linker has been registered inside the WaliCtx
        // because CHILD threads may need it.
        //
        // If we registered AFTER we called '_start', there would be a race between the first
        // invocation of '__wasm_thread_start' and registering the linker to the WaliCtx.
        match wali_start.call(store, ()) {
            Ok(()) => log::warn!("Start function exited without custom Wali trap"),
            Err(e) => match e.downcast::<WaliTrap>() {
                Ok(trap) => match trap {
                    WaliTrap::ThreadExiting => log::info!("Start function is exiting normally"),
                    WaliTrap::ProcessExiting => log::info!("Start function triggered process exit"),
                },
                Err(e) => bail!("Start function exited with error: {e}"),
            },
        }

        Ok(())
    }
}

impl<S: ProviderState> WaliCtxProvider<S> {
    /// Adds WALI host imports to this provider's linker
    ///
    /// # Safety
    ///
    /// This function requires that the [`wasmtime::Engine`] used to create the
    /// supplied [`Linker<T>`] was constructed with options patched via
    /// [`WaliCtxProvider::patch_config`].
    ///
    /// The affected options must not be overridden after patching.
    /// Otherwise the execution of some WALI imports may result in undefined behavior.
    ///
    /// TLDR: Just use the engine from [`WaliCtxProvider::engine`].
    pub unsafe fn add_to_linker<T: WaliView + 'static>(
        &self,
        linker: &mut Linker<T>,
    ) -> Result<(), Error>
    where
        S: HasLinker<T>,
    {
        unsafe { WaliCtxInner::add_to_linker(linker)? };

        let thread_linker = self.state.linker();
        linker.func_wrap(
            WaliCtxInner::MODULE_NAME,
            "__wasm_thread_spawn",
            move |caller: Caller<'_, T>, setup_fnptr: u32, arg_wasm: i32| -> Result<i32, Error> {
                impls::wali_thread_spawn(thread_linker.clone(), caller, setup_fnptr, arg_wasm)
                    .map_err(Error::new)
            },
        )?;

        Ok(())
    }

    /// Configure the Wasmtime engine for WALI.
    ///
    /// Specifically, this sets the _allocation strategy_ of Wasmtime to
    /// [`InstanceAllocationStrategy::OnDemand`] and configures Wasmtime
    /// to use a custom [`wasmtime::MemoryCreator`].
    ///
    /// Having control over Wasmtimes linear memory is crucial for syscalls
    /// like _mmap_ to behave correctly. Therefore, you **must not change**
    /// [`Config::allocation_strategy`] or [`Config::with_host_memory`] settings
    /// after calling this function if you want to run a WebAssembly binary
    /// compiled for WALI. This is the reason why [`WaliCtx::add_to_linker`]
    /// is marked as _unsafe_.
    ///
    /// Lastly, WALIs signal handling depends on epoch-interruption being
    /// enabled. Disabling this option will result in a runtime error.
    fn patch_config(config: &mut Config) -> &mut Config {
        config
            .allocation_strategy(InstanceAllocationStrategy::OnDemand)
            .with_host_memory(Arc::new(WaliMemoryCreator))
            .epoch_interruption(true)
    }

    fn patch_binary(module: &mut walrus::Module) -> Result<&mut walrus::Module, Error> {
        use walrus::FunctionBuilder;
        use walrus::RefType;
        use walrus::ValType;

        let table_idx = match module.tables.main_function_table() {
            Ok(Some(table_idx)) => table_idx,
            Ok(None) => bail!("Could not find a function table for indirect function calls"),
            Err(e) => bail!(
                "WALI expects one single function table to be present for indirect function calls.\n{e}"
            ),
        };

        let mut builder = FunctionBuilder::new(
            &mut module.types,
            &[ValType::I32],
            &[ValType::Ref(RefType::Funcref)],
        );

        builder.name("__wasmtime_wali_get_indirect_func".into());
        let table_fn_idx = module.locals.add(ValType::I32);

        builder
            .func_body()
            .local_get(table_fn_idx)
            .table_get(table_idx);

        let indirect_function_provider = builder.finish(vec![table_fn_idx], &mut module.funcs);

        module.exports.add(
            "__wasmtime_wali_get_indirect_func",
            indirect_function_provider,
        );

        Ok(module)
    }
}

impl<S: ProviderState> WaliCtxProvider<S> {}
