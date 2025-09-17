use std::{sync::Arc, thread::JoinHandle};

use anyhow::Error;
use race_detection::tracing::{BinaryTraceOutput, Tracing};
use wasmtime::{IntoFunc, WasmParams, WasmResults};

use crate::runtime::{
    ArcTracing,
    base::{ThreadlinkRuntime, ThreadlinkRuntimeBuilder},
    context_provider::TracingContextProvider,
};

/// Enables execution of multithreaded WebAssembly modules _with_ execution tracing support.
/// 
/// This struct is basically a wrapper for [`ThreadlinkRuntime`] extending it with 
/// execution tracing capabilities.
pub struct WasmgrindRuntime {
    inner: ThreadlinkRuntime,
    tracing: ArcTracing,
}

impl WasmgrindRuntime {
    /// Invokes a function, which is exported by the WebAssembly module.
    /// 
    /// This method simply forwards the call to 
    /// [`ThreadlinkRuntime::invoke_function`].
    /// Refer to the docs of this method for further details.
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
    /// let runtime = wasmgrind::wasmgrind(binary, false, false)?;
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
        self.inner.invoke_function(name, params)
    }

    /// Emits the current state of the execution trace in binary format.
    ///
    /// This method will lock the internal execution trace structure 
    /// iterates over all collected events and creates a binary trace in
    /// [RapidBin](https://afkoffee.github.io/wasmgrind/developers_guide/race_detection/rapid_bin.html)
    /// format.
    /// 
    /// Refer to [`race_detection::tracing::Tracing`] for further information with regard to execution tracing
    /// in Wasmgrind.
    pub fn generate_binary_trace(&self) -> Result<BinaryTraceOutput, Error> {
        self.tracing.generate_binary_trace()
    }
}

/// A builder for [`WasmgrindRuntime`] instances.
pub struct WasmgrindRuntimeBuilder {
    inner: ThreadlinkRuntimeBuilder,
    tracing: ArcTracing,
}

impl WasmgrindRuntimeBuilder {
    pub(crate) fn new(wasm: &[u8]) -> Result<Self, Error> {
        let tracing = Arc::new(Tracing::new());
        let ctx = TracingContextProvider::new(tracing.clone());
        Ok(Self {
            inner: ThreadlinkRuntimeBuilder::with_contextprovider(wasm, &ctx)?,
            tracing,
        })
    }

    /// Registers a custom import for the WebAssembly module.
    /// 
    /// This method simply forwards the call to 
    /// [`ThreadlinkRuntimeBuilder::register_custom_import`].
    /// Refer to the docs of this method for further details.
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
    /// let builder = wasmgrind::wasmgrind_builder(binary, false, false)?;
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
        mut self,
        module: &str,
        name: &str,
        func: impl IntoFunc<(), Params, Results>,
    ) -> Result<Self, Error> {
        self.inner = self.inner.register_custom_import(module, name, func)?;

        Ok(self)
    }

    /// Consumes this builder to create a new [`WasmgrindRuntime`] instance.
    pub fn build(self) -> WasmgrindRuntime {
        WasmgrindRuntime {
            inner: self.inner.build(),
            tracing: self.tracing,
        }
    }
}
