use std::sync::Arc;

use js_sys::{
    Function, Object, Reflect,
    WebAssembly::{Memory, Module},
};
use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt, prelude::Closure, throw_str};

use crate::{
    coms::ThreadlinkComs, message::Message, set_value, start_worker, tmgmt::SyncedJsTmgmt,
};

pub struct ThreadlinkClosures {
    thread_create_fn: Closure<dyn FnMut(u32, u32) -> u32>,
    thread_join_fn: Closure<dyn FnMut(u32) -> u32>,
    panic_fn: Closure<dyn FnMut(i32)>,
}

impl ThreadlinkClosures {
    pub fn new(
        memory: &Memory,
        module: &Module,
        tmgmt: Arc<SyncedJsTmgmt>,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            thread_create_fn: Self::get_thread_create_closure(memory, module, tmgmt.clone())?,
            thread_join_fn: Self::get_thread_join_closure(tmgmt),
            panic_fn: Self::get_panic_closure(),
        })
    }

    pub fn get_wasm_threadlink_imports(&self) -> Result<Object, JsValue> {
        let imports = Object::new();

        Reflect::set(
            &imports,
            &JsValue::from("thread_create"),
            self.thread_create_fn.as_ref().unchecked_ref::<Function>(),
        )?;
        Reflect::set(
            &imports,
            &JsValue::from("thread_join"),
            self.thread_join_fn.as_ref().unchecked_ref::<Function>(),
        )?;
        Reflect::set(
            &imports,
            &JsValue::from("panic"),
            self.panic_fn.as_ref().unchecked_ref::<Function>(),
        )?;

        Ok(imports)
    }

    fn get_panic_closure() -> Closure<dyn FnMut(i32)> {
        Closure::new(|errno| throw_str(&wasmgrind_error::errno_description(errno)))
    }

    fn get_thread_create_closure(
        memory: &Memory,
        module: &Module,
        tmgmt: Arc<SyncedJsTmgmt>,
    ) -> Result<Closure<dyn FnMut(u32, u32) -> u32>, JsValue> {
        let module = JsValue::from(module);
        let memory = JsValue::from(memory);

        Ok(Closure::new(move |tid_ptr, start_routine| {
            let worker = start_worker();
            let child_id = tmgmt.register_thread().unwrap_throw();

            let msg = Message::ThreadlinkWorkerStartup {
                target_module: module.clone(),
                target_memory: memory.clone(),
                thread_id: child_id,
                start_routine,
                communications: ThreadlinkComs::send(tmgmt.clone()).unwrap_throw(),
            };

            worker
                .post_message(&msg.try_to_json().unwrap_throw())
                .unwrap_throw();

            set_value(&memory, tid_ptr, child_id);

            0
        }))
    }

    fn get_thread_join_closure(tmgmt: Arc<SyncedJsTmgmt>) -> Closure<dyn FnMut(u32) -> u32> {
        Closure::new(move |tid| {
            tmgmt.join(tid).unwrap_throw();

            0
        })
    }
}
