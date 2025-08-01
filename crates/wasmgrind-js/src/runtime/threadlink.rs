use std::sync::Arc;

use js_sys::{
    JsString, Promise, Uint8Array,
    WebAssembly::{Memory, Module},
};
use wasm_bindgen::{
    JsCast, JsError, JsValue, UnwrapThrowExt,
    prelude::{Closure, wasm_bindgen},
};
use wasmgrind_core::get_memory_limits;
use web_sys::MessageEvent;

use crate::{
    compile_helper, coms::ThreadlinkComs, memory_helper, message::Message, start_worker,
    tmgmt::SyncedJsTmgmt,
};

/// Enables execution of multithreaded WebAssembly modules _without_ execution tracing support.
#[wasm_bindgen]
pub struct ThreadlinkRuntime {
    module: Module,
    memory: Memory,
    tmgmt: Arc<SyncedJsTmgmt>,
}

impl ThreadlinkRuntime {
    pub(crate) async fn new(binary: &[u8]) -> Result<Self, JsValue> {
        let (min, max) = get_memory_limits(binary).map_err(|e| JsError::from(&*e))?;
        let memory = memory_helper(min, max);
        let module = compile_helper(Uint8Array::from(binary)).await;
        let tmgmt = Arc::new(SyncedJsTmgmt::new());

        Ok(Self {
            module,
            memory,
            tmgmt,
        })
    }
}

#[wasm_bindgen]
impl ThreadlinkRuntime {
    /// Invokes a function, which is exported by the WebAssembly module.
    /// 
    /// The `function_name` argument has to match the name of the exported 
    /// WebAssembly function precisely. The function must not take any
    /// arguments nor return any results.
    /// 
    /// This method calls the specified function in a dedicated WebWorker and returns
    /// a [`js_sys::Promise`], which will resolve once the worker finished executing
    /// the specified function.
    /// 
    /// **Note:** If there is an error during excution in the WebWorker, the Promise
    /// may _never_ resolve.
    pub fn invoke_function(&self, function_name: JsString) -> Promise {
        Promise::new(&mut |resolve, _| {
            let msg = Message::ThreadlinkRunnerStartup {
                target_module: JsValue::from(&self.module),
                target_memory: JsValue::from(&self.memory),
                target_function: function_name.clone(),
                communications: ThreadlinkComs::send(self.tmgmt.clone()).unwrap_throw(),
            };

            let onmessage_callback = Closure::once_into_js(move |e: MessageEvent| {
                if let Message::RunnerFinished = Message::try_from_json(e.data()).unwrap_throw() {
                    drop(resolve.call0(&JsValue::undefined()))
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
