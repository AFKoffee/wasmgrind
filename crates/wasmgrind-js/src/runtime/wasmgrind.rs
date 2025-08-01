use std::sync::Arc;

use js_sys::{
    JsString, Promise, Uint8Array,
    WebAssembly::{Memory, Module},
};
use race_detection::tracing::Tracing;
use wasm_bindgen::{
    JsCast, JsError, JsValue, UnwrapThrowExt,
    prelude::{Closure, wasm_bindgen},
};
use wasmgrind_core::get_memory_limits;
use web_sys::MessageEvent;

use crate::{
    compile_helper,
    coms::{TraceGenerationComs, WasmgrindComs},
    memory_helper,
    message::Message,
    runtime::TraceOutput,
    start_worker,
    tmgmt::SyncedJsTmgmt,
};

/// Enables execution of multithreaded WebAssembly modules _with_ execution tracing support.
#[wasm_bindgen]
pub struct WasmgrindRuntime {
    module: Module,
    memory: Memory,
    tracing: Arc<Tracing>,
    tmgmt: Arc<SyncedJsTmgmt>,
}

impl WasmgrindRuntime {
    pub(crate) async fn new(binary: &[u8]) -> Result<Self, JsValue> {
        let (min, max) = get_memory_limits(binary).map_err(|e| JsError::from(&*e))?;
        let memory = memory_helper(min, max);
        let module = compile_helper(Uint8Array::from(binary)).await;
        let tracing = Arc::new(Tracing::new());
        let tmgmt = Arc::new(SyncedJsTmgmt::new());

        Ok(Self {
            module,
            memory,
            tracing,
            tmgmt,
        })
    }
}

#[wasm_bindgen]
impl WasmgrindRuntime {
    /// Invokes a function, which is exported by the WebAssembly module.
    /// 
    /// This method behaves the same as [`ThreadlinkRuntime::invoke_function`][`super::ThreadlinkRuntime::invoke_function`].
    /// Refer to the docs of this method for further details.
    pub fn invoke_function(&self, function_name: JsString) -> Promise {
        Promise::new(&mut |resolve, _| {
            let msg = Message::RunnerStartup {
                target_module: JsValue::from(&self.module),
                target_memory: JsValue::from(&self.memory),
                target_function: function_name.clone(),
                communications: WasmgrindComs::send(self.tracing.clone(), self.tmgmt.clone())
                    .unwrap_throw(),
            };

            let onmessage_callback = Closure::once_into_js(move |e: MessageEvent| {
                if let Message::RunnerFinished = Message::try_from_json(e.data()).unwrap_throw() {
                    drop(resolve.call0(&JsValue::undefined()))
                }
            });

            Self::start_and_post(onmessage_callback, msg);
        })
    }

    /// Emits the current state of the execution trace in binary format.
    ///
    /// This method will lock the internal execution trace structure 
    /// iterates over all collected events and creates a binary trace in
    /// [RapidBin](https://wasmgrind-d6f2b1.gitlab.io/book/developers_guide/race_detection/rapid_bin.html)
    /// format.
    /// 
    /// The excution trace is returned in form of a [`js_sys::Promise`] that wrapps a [`TraceOutput`] object.
    /// 
    /// Refer to [`race_detection::tracing::Tracing`] for further information with regard to execution tracing
    /// in Wasmgrind.
    pub fn generate_binary_trace(&self) -> Promise {
        Promise::new(&mut |resolve, _| {
            let (coms, receiver) = TraceGenerationComs::send(self.tracing.clone()).unwrap_throw();
            let msg = Message::TraceGenerationStartup {
                communications: coms,
            };

            let onmessage_callback = Closure::once_into_js(move |e: MessageEvent| {
                if let Message::TraceGenerationFinished =
                    Message::try_from_json(e.data()).unwrap_throw()
                {
                    let output = receiver.receive().unwrap_throw();
                    let js_output: TraceOutput = output.try_into().unwrap_throw();
                    drop(resolve.call1(&JsValue::undefined(), &JsValue::from(js_output)))
                }
            });

            Self::start_and_post(onmessage_callback, msg);
        })
    }

    fn start_and_post(onmessage_callback: JsValue, message: Message) {
        let worker = start_worker();
        worker.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        worker
            .post_message(&message.try_to_json().unwrap_throw())
            .unwrap_throw();
    }
}
