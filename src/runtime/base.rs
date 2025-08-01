use std::{
    sync::{Arc, Mutex, RwLock},
    thread::{self, JoinHandle},
};

use anyhow::{Error, bail};
use wasmgrind_error::errno;
use wasmtime::{
    Engine, IntoFunc, Linker, MemoryType, Module, SharedMemory, Store, WasmParams, WasmResults,
};

use crate::{
    runtime::{
        SynchronizedLinker, Tmgmt,
        context_provider::{ContextProvider, DefaultContextProvider},
    },
    tmgmt::ThreadManagement,
};

/// Enables execution of multithreaded WebAssembly modules _without_ execution tracing support.
pub struct ThreadlinkRuntime {
    engine: Engine,
    module: Module,
    linker: SynchronizedLinker,
}

impl ThreadlinkRuntime {
    /// Invokes a function, which is exported by the WebAssembly module.
    /// 
    /// The `name` argument has to match the name of the exported 
    /// WebAssembly function precisely.
    /// 
    /// The `params` argument should provide the inputs to the
    /// specified function. 
    /// 
    /// This method calls the specified function in a dedicated thread and returns
    /// a [`std::thread::JoinHandle`] to it, which yields the result upon thread termination.
    /// 
    /// **Note:** [`ThreadlinkRuntime`] uses [`wasmtime`] as its core WebAssembly engine.
    /// Therefore, the `params` argument has to implement wasmtimes 
    /// [`WasmParams`] trait and the results of the function have to implement the
    /// [`WasmResults`] trait. Refer to [`wasmtime::Func::typed`] for details.
    /// 
    /// # Errors
    /// This method will return an error in one of the following cases:
    /// - The instantiation of the WebAssembly module failed
    /// - `name` was no function export or the type signature of the exported function
    ///   did not match `Params` or `Results`
    /// 
    /// # Examples
    /// ```no_run
    /// # use anyhow::Error;
    /// # fn main() -> Result<(), Error> {
    /// // The WebAssembly module "target.wasm" is located inside your working directory.
    /// //
    /// // It exports a parameterless function named `run`.
    /// let binary = "target.wasm";
    ///
    /// let runtime = wasmgrind::runtime(binary, false)?;
    /// runtime.invoke_function::<(), ()>(String::from("run"), ())
    ///     .join()
    ///     .expect("Runner Thread Panicked")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn invoke_function<Params, Results>(
        &self,
        name: String,
        params: Params,
    ) -> JoinHandle<Result<Results, Error>>
    where
        Params: WasmParams + 'static,
        Results: WasmResults + 'static,
    {
        let engine = self.engine.clone();
        let module = self.module.clone();
        let linker = self.linker.clone();
        thread::spawn(move || {
            let _ = wasmgrind_core::tmgmt::thread_id();

            let mut store = Store::new(&engine, ());
            let instance = match linker.read() {
                Ok(linker_guard) => linker_guard.instantiate(&mut store, &module)?,
                Err(_) => bail!("Linker Mutex was poisoned!"),
            };
            let function = instance.get_typed_func::<Params, Results>(&mut store, &name)?;
            function.call(&mut store, params)
        })
    }

    pub(crate) fn write_data_to_memory(memory: &SharedMemory, address: usize, data: &[u8]) -> i32 {
        let raw_memory = memory.data();

        if raw_memory[address..].len() < data.len() {
            errno::RT_ERROR_MEMORY_OUT_OF_BOUNDS_ACCESS
        } else {
            for (offset, byte) in data.iter().enumerate() {
                unsafe { *raw_memory[address + offset].get() = *byte };
            }

            errno::NO_ERROR
        }
    }
}

/// A builder for [`ThreadlinkRuntime`] instances.
pub struct ThreadlinkRuntimeBuilder {
    engine: Engine,
    module: Module,
    memory: SharedMemory,
    linker: SynchronizedLinker,
    tmgmt: Tmgmt,
}

impl ThreadlinkRuntimeBuilder {
    pub(crate) fn new(wasm: &[u8]) -> Result<Self, Error> {
        Self::with_contextprovider(wasm, &DefaultContextProvider::new())
    }

    pub(crate) fn with_contextprovider<
        TCP,
        TCR,
        TJP,
        TJR,
        T: ContextProvider<TCP, TCR, TJP, TJR>,
    >(
        wasm: &[u8],
        context: &T,
    ) -> Result<Self, Error> {
        let engine = Engine::default();
        let module = Module::from_binary(&engine, wasm)?;
        let (min, max) = wasmgrind_core::get_memory_limits(wasm)?;
        let memory = SharedMemory::new(&engine, MemoryType::shared(min, max))?;
        let linker = Arc::new(RwLock::new(Linker::new(&engine)));
        let tmgmt = Arc::new(Mutex::new(ThreadManagement::new()));

        let builder = Self {
            engine,
            module,
            memory,
            linker,
            tmgmt,
        };

        builder.register_default_imports(context)?;

        Ok(builder)
    }

    fn register_default_imports<TCP, TCR, TJP, TJR, T: ContextProvider<TCP, TCR, TJP, TJR>>(
        &self,
        context: &T,
    ) -> Result<(), Error> {
        let store = Store::new(&self.engine, ());
        let mut linker_mut = match self.linker.write() {
            Ok(linker_guard) => linker_guard,
            Err(_) => bail!("Linker Mutex was poisoned!"),
        };
        linker_mut.define(&store, "env", "memory", self.memory.clone())?;
        linker_mut.func_wrap(
            "wasm_threadlink",
            "thread_create",
            context.get_thread_create_func(
                self.engine.clone(),
                self.module.clone(),
                self.memory.clone(),
                self.linker.clone(),
                self.tmgmt.clone(),
            ),
        )?;
        linker_mut.func_wrap(
            "wasm_threadlink",
            "thread_join",
            context.get_thread_join_func(self.tmgmt.clone()),
        )?;

        context.finalize(&mut linker_mut)?;

        Ok(())
    }

    /// Registers a custom import for the WebAssembly module.
    /// 
    /// The `module` and `name` arguments identify the custom import inside
    /// the WebAssembly module while `func` provides its implementation.
    /// 
    /// **Note:** The [`ThreadlinkRuntime`] uses [`wasmtime`] as its core WebAssembly engine.
    /// This function is basically a wrapper for [`wasmtime::Linker::func_wrap`], which
    /// means that it imposes the same restrictions on its arguments.
    /// 
    /// # Errors
    /// This method will return an error in one of the following cases:
    /// - The internal linker struct is not accessible (although this is considered 
    ///   a bug and should never happen).
    /// - `module` and `name` identify an import that has been registered already.
    /// 
    /// # Examples
    /// ```no_run
    /// # use anyhow::Error;
    /// # fn main() -> Result<(), Error> {
    /// // The WebAssembly module "target.wasm" is located inside your working directory.
    /// //
    /// // It defines a custom import `custom_module` `custom_function`
    /// // with the signature (i32, i32) -> i32.
    /// let binary = "target.wasm";
    ///
    /// let builder = wasmgrind::runtime_builder(binary, false)?;
    /// let runtime = builder
    ///     .register_custom_import::<(i32, i32), (i32)>(
    ///         "custom_module",
    ///         "custom_function",
    ///         |x, y| {
    ///             println!("Custom function received two integers: x = {x} and y = {y}!");
    ///             println!("Returning their sum ...");
    ///             x + y
    ///         }
    ///     )?
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn register_custom_import<Params, Results>(
        self,
        module: &str,
        name: &str,
        func: impl IntoFunc<(), Params, Results>,
    ) -> Result<Self, Error> {
        let mut linker_mut = match self.linker.write() {
            Ok(linker_guard) => linker_guard,
            Err(_) => bail!("Linker Mutex was poisoned!"),
        };

        linker_mut.func_wrap(module, name, func)?;

        drop(linker_mut);

        Ok(self)
    }

    /// Consumes this builder to create a new [`ThreadlinkRuntime`] instance.
    pub fn build(self) -> ThreadlinkRuntime {
        ThreadlinkRuntime {
            engine: self.engine,
            module: self.module,
            linker: self.linker,
        }
    }
}
