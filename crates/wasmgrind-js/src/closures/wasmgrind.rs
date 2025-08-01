use std::sync::Arc;

use js_sys::{
    Function, Object, Reflect,
    WebAssembly::{Memory, Module},
};
use race_detection::tracing::{Op, Tracing};
use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt, prelude::Closure, throw_str};

use crate::{
    coms::WasmgrindComs,
    message::Message,
    set_value, start_worker,
    tmgmt::{SyncedJsTmgmt, thread_id},
};

pub struct WasmgrindClosures {
    thread_create_fn: Closure<dyn FnMut(u32, u32, u32, u32) -> u32>,
    thread_join_fn: Closure<dyn FnMut(u32, u32, u32) -> u32>,
    panic_fn: Closure<dyn FnMut(i32)>,
    start_lock_fn: Closure<dyn FnMut(u32, u32, u32)>,
    finish_lock_fn: Closure<dyn FnMut(u32, u32, u32)>,
    start_unlock_fn: Closure<dyn FnMut(u32, u32, u32)>,
    finish_unlock_fn: Closure<dyn FnMut(u32, u32, u32)>,
    read_hook_fn: Closure<dyn FnMut(u32, u32, u32, u32)>,
    write_hook_fn: Closure<dyn FnMut(u32, u32, u32, u32)>,
}

impl WasmgrindClosures {
    pub fn new(
        memory: &Memory,
        module: &Module,
        tracing: Arc<Tracing>,
        tmgmt: Arc<SyncedJsTmgmt>,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            thread_create_fn: Self::get_thread_create_closure(
                memory,
                module,
                tracing.clone(),
                tmgmt.clone(),
            )?,
            thread_join_fn: Self::get_thread_join_closure(tracing.clone(), tmgmt),
            panic_fn: Self::get_panic_closure(),
            start_lock_fn: Self::get_start_lock_closure(tracing.clone()),
            finish_lock_fn: Self::get_finish_lock_closure(tracing.clone()),
            start_unlock_fn: Self::get_start_unlock_closure(tracing.clone()),
            finish_unlock_fn: Self::get_finish_unlock_closure(tracing.clone()),
            read_hook_fn: Self::get_read_hook_closure(tracing.clone()),
            write_hook_fn: Self::get_write_hook_closure(tracing),
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
        Reflect::set(
            &imports,
            &JsValue::from("start_lock"),
            self.start_lock_fn.as_ref().unchecked_ref::<Function>(),
        )?;
        Reflect::set(
            &imports,
            &JsValue::from("finish_lock"),
            self.finish_lock_fn.as_ref().unchecked_ref::<Function>(),
        )?;
        Reflect::set(
            &imports,
            &JsValue::from("start_unlock"),
            self.start_unlock_fn.as_ref().unchecked_ref::<Function>(),
        )?;
        Reflect::set(
            &imports,
            &JsValue::from("finish_unlock"),
            self.finish_unlock_fn.as_ref().unchecked_ref::<Function>(),
        )?;

        Ok(imports)
    }

    pub fn get_wasabi_imports(&self) -> Result<Object, JsValue> {
        let imports = Object::new();

        Reflect::set(
            &imports,
            &JsValue::from("read_hook"),
            self.read_hook_fn.as_ref().unchecked_ref::<Function>(),
        )?;
        Reflect::set(
            &imports,
            &JsValue::from("write_hook"),
            self.write_hook_fn.as_ref().unchecked_ref::<Function>(),
        )?;

        Ok(imports)
    }

    fn get_read_hook_closure(tracing: Arc<Tracing>) -> Closure<dyn FnMut(u32, u32, u32, u32)> {
        Closure::new(move |addr, n, fidx, iidx| {
            tracing
                .add_event(
                    thread_id().unwrap_throw(),
                    Op::Read { addr, n },
                    (fidx, iidx),
                )
                .unwrap_throw();
        })
    }

    fn get_write_hook_closure(tracing: Arc<Tracing>) -> Closure<dyn FnMut(u32, u32, u32, u32)> {
        Closure::new(move |addr, n, fidx, iidx| {
            tracing
                .add_event(
                    thread_id().unwrap_throw(),
                    Op::Write { addr, n },
                    (fidx, iidx),
                )
                .unwrap_throw();
        })
    }

    fn get_panic_closure() -> Closure<dyn FnMut(i32)> {
        Closure::new(|errno| throw_str(&wasmgrind_error::errno_description(errno)))
    }

    fn get_thread_create_closure(
        memory: &Memory,
        module: &Module,
        tracing: Arc<Tracing>,
        tmgmt: Arc<SyncedJsTmgmt>,
    ) -> Result<Closure<dyn FnMut(u32, u32, u32, u32) -> u32>, JsValue> {
        let module = JsValue::from(module);
        let memory = JsValue::from(memory);

        Ok(Closure::new(move |tid_ptr, start_routine, fidx, iidx| {
            let worker = start_worker();
            let child_id = tmgmt.register_thread().unwrap_throw();
            tracing
                .add_event(
                    thread_id().unwrap_throw(),
                    Op::Fork { tid: child_id },
                    (fidx, iidx),
                )
                .unwrap_throw();

            let msg = Message::WorkerStartup {
                target_module: module.clone(),
                target_memory: memory.clone(),
                thread_id: child_id,
                start_routine,
                communications: WasmgrindComs::send(tracing.clone(), tmgmt.clone()).unwrap_throw(),
            };

            worker
                .post_message(&msg.try_to_json().unwrap_throw())
                .unwrap_throw();

            set_value(&memory, tid_ptr, child_id);

            0
        }))
    }

    fn get_thread_join_closure(
        tracing: Arc<Tracing>,
        tmgmt: Arc<SyncedJsTmgmt>,
    ) -> Closure<dyn FnMut(u32, u32, u32) -> u32> {
        Closure::new(move |tid, fidx, iidx| {
            tmgmt.join(tid).unwrap_throw();

            tracing
                .add_event(thread_id().unwrap_throw(), Op::Join { tid }, (fidx, iidx))
                .unwrap_throw();

            0
        })
    }

    fn get_start_lock_closure(tracing: Arc<Tracing>) -> Closure<dyn FnMut(u32, u32, u32)> {
        Closure::new(move |lock_id, fidx, iidx| {
            tracing
                .add_event(
                    thread_id().unwrap_throw(),
                    Op::Request { lock: lock_id },
                    (fidx, iidx),
                )
                .unwrap_throw();
        })
    }

    fn get_finish_lock_closure(tracing: Arc<Tracing>) -> Closure<dyn FnMut(u32, u32, u32)> {
        Closure::new(move |lock_id, fidx, iidx| {
            tracing
                .add_event(
                    thread_id().unwrap_throw(),
                    Op::Request { lock: lock_id },
                    (fidx, iidx),
                )
                .unwrap_throw();
        })
    }

    fn get_start_unlock_closure(tracing: Arc<Tracing>) -> Closure<dyn FnMut(u32, u32, u32)> {
        Closure::new(move |lock_id, fidx, iidx| {
            tracing
                .add_event(
                    thread_id().unwrap_throw(),
                    Op::Request { lock: lock_id },
                    (fidx, iidx),
                )
                .unwrap_throw();
        })
    }

    fn get_finish_unlock_closure(tracing: Arc<Tracing>) -> Closure<dyn FnMut(u32, u32, u32)> {
        Closure::new(move |lock_id, fidx, iidx| {
            tracing
                .add_event(
                    thread_id().unwrap_throw(),
                    Op::Request { lock: lock_id },
                    (fidx, iidx),
                )
                .unwrap_throw();
        })
    }
}
