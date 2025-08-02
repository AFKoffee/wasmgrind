use std::sync::Arc;

use js_sys::{
    JsString, Object, Reflect,
    WebAssembly::{Memory, Module},
};
use wasm_bindgen::{JsError, JsValue};

use crate::{closures::WasmgrindClosures, coms::WasmgrindComs, tmgmt::SyncedJsTmgmt};

pub struct WasmgrindContext {
    module: Module,
    memory: Memory,
    thread_id: Option<u32>,
    closures: WasmgrindClosures,
    tmgmt: Arc<SyncedJsTmgmt>,
}

impl WasmgrindContext {
    fn get_env_imports(&self) -> Result<Object, JsValue> {
        let imports = Object::new();

        Reflect::set(&imports, &JsString::from("memory"), &self.memory)?;

        Ok(imports)
    }
}

impl WasmgrindContext {
    pub fn new(
        module: Module,
        memory: Memory,
        thread_id: Option<u32>,
        coms: WasmgrindComs,
    ) -> Result<Self, JsValue> {
        let (tracing, tmgmt) = coms.receive().map_err(|e| JsError::from(&*e))?;

        if let Some(tid) = thread_id {
            // This only happens for workers
            wasmgrind_core::tmgmt::set_thread_id(tid).map_err(|e| JsError::from(&*e))?;
        } else {
            // In this case we are in the main execution context of the function,
            // i.e., in the WasmgrindRunner WebWorker
            let tid = wasmgrind_core::tmgmt::next_available_thread_id();
            wasmgrind_core::tmgmt::set_thread_id(tid).map_err(|e| JsError::from(&*e))?;
        };

        let closures = WasmgrindClosures::new(&memory, &module, tracing, tmgmt.clone())?;

        Ok(Self {
            module,
            memory,
            thread_id,
            closures,
            tmgmt,
        })
    }

    pub fn get_target_module(&self) -> Module {
        self.module.clone()
    }

    pub fn get_wasm_imports(&self) -> Result<Object, JsValue> {
        let imports = Object::new();

        Reflect::set(
            &imports,
            &JsString::from("env"),
            &self.get_env_imports()?.into(),
        )?;
        Reflect::set(
            &imports,
            &JsString::from("wasm_threadlink"),
            &self.closures.get_wasm_threadlink_imports()?.into(),
        )?;
        Reflect::set(
            &imports,
            &JsString::from("wasabi"),
            &self.closures.get_wasabi_imports()?.into(),
        )?;

        Ok(imports)
    }

    pub fn close(self) -> Result<(), JsError> {
        if let Some(tid) = self.thread_id {
            // Signal that the worker is finished
            self.tmgmt.set_return_val(tid, 0)
        } else {
            // In this case we are on the main thread,
            // which will never appear in thread management
            Ok(())
        }
    }
}
