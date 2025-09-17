//! # Wasmgrind JS
//! The wasmgrind-js crate contains the web-based Wasmgrind engine.
//!
//! It provides JS bindings of functions to prepare WebAssembly modules for multithreading,
//! instrument them for execution tracing and support
//! their multithreaded execution.

use js_sys::{
    JsString, Uint8Array,
    WebAssembly::{self},
};
use wasm_bindgen::{JsError, JsValue, prelude::*};
use wasm_bindgen_futures::JsFuture;
use wasmgrind_core::patching::{instrument, threadify};
use web_sys::{Url, Worker};

use crate::runtime::{ThreadlinkRuntime, WasmgrindRuntime};

/*
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(a: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}*/

mod closures;
mod coms;
mod context;
mod message;
/// Defines the different runtimes of Wasmgrind.
pub mod runtime;
mod tmgmt;

#[wasm_bindgen(module = "/js/helpers.js")]
extern "C" {
    async fn fetch_helper(url: Url) -> Uint8Array;
    async fn compile_helper(wasm: Uint8Array) -> WebAssembly::Module;
    fn memory_helper(min: u32, max: u32) -> WebAssembly::Memory;
    fn set_value(memory: &JsValue, ptr: u32, value: u32);
}

#[wasm_bindgen(module = "/js/worker.js")]
extern "C" {
    fn start_worker() -> Worker;
}

/// Patches the given WebAssembly binary for multithreading support, performs binary instrumentation
/// and returns a [`WasmgrindRuntime`] wrapping the modified binary.
///
/// The `binary` argument should be an URL to a valid WebAssembly module in binary format, which
/// was compiled against the _tracing-extended_ Wasmgrind ABI. For details refer to the
/// [Wasmgrind Book](https://afkoffee.github.io/wasmgrind/user_guide/compiling_the_binary.html).
///
/// # Errors
/// The function will fail if the provided WebAssembly binary could not be patched or instrumented
/// for any reason.
///
/// Refer to the docs or [`wasmgrind_core::patching::threadify`] and
/// [`wasmgrind_core::patching::instrument`] for further details.
#[wasm_bindgen]
pub async fn wasmgrind(binary: Url) -> Result<WasmgrindRuntime, JsValue> {
    let wasm = fetch_helper(binary).await.to_vec();
    let patched = threadify(&wasm).map_err(|e| JsError::from(&*e))?;
    let instrumented = instrument(&patched).map_err(|e| JsError::from(&*e))?;

    WasmgrindRuntime::new(&instrumented).await
}

/// Creates a [`WasmgrindRuntime`] for a given WebAssembly binary and executes
/// a single exported function of the module.
///
/// The function specified by `name` has to be a parameterless function without
/// any return value that is a valid export of the WebAssembly module.
///
/// The function waits for the function to return an generates a binary trace,
/// which is returned as a result of the JavaScript promise.
///
/// Refer to [`wasmgrind`] for details regarding runtime creation.
#[wasm_bindgen]
pub async fn grind(binary: Url, function_name: JsString) -> Result<JsValue, JsValue> {
    let runtime = wasmgrind(binary).await?;
    JsFuture::from(runtime.invoke_function(function_name)).await?;

    let output = JsFuture::from(runtime.generate_binary_trace()).await?;
    Ok(output)
}

/// Patches the given WebAssembly binary for multithreading support and returns
/// a [`ThreadlinkRuntime`] wrapping the modified binary.
///
/// The `binary` argument should be an URL to a valid WebAssembly module in binary format, which
/// was compiled against the _standalone_ Wasmgrind ABI. For details refer to the
/// [Wasmgrind Book](https://afkoffee.github.io/wasmgrind/user_guide/compiling_the_binary.html).
///
/// # Errors
/// The function will fail if the provided WebAssembly binary could not be patched for any reason.
///
/// Refer to the docs or [`wasmgrind_core::patching::threadify`] for further details.
#[wasm_bindgen]
pub async fn runtime(binary: Url) -> Result<ThreadlinkRuntime, JsValue> {
    let wasm = fetch_helper(binary).await.to_vec();
    let patched = threadify(&wasm).map_err(|e| JsError::from(&*e))?;

    ThreadlinkRuntime::new(&patched).await
}

/// Creates a [`ThreadlinkRuntime`] for a given WebAssembly binary and executes a
/// single exported function of the module.
///
/// The function specified by `name` has to be a parameterless function without
/// any return value that is a valid export of the WebAssembly module.
///
/// Refer to [`function@runtime`] for details.
#[wasm_bindgen]
pub async fn run(binary: Url, function_name: JsString) -> Result<(), JsValue> {
    let runtime = runtime(binary).await?;
    JsFuture::from(runtime.invoke_function(function_name)).await?;

    Ok(())
}
