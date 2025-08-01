use js_sys::{BigInt, Function, JsString, Object, Reflect, WebAssembly, global};
use wasm_bindgen::{JsCast, JsError, JsValue, prelude::wasm_bindgen};
use wasm_bindgen_futures::JsFuture;
use web_sys::DedicatedWorkerGlobalScope;

use crate::{
    coms::{ThreadlinkComs, TraceGenerationComs, WasmgrindComs},
    context::{ThreadlinkContext, WasmgrindContext},
};

pub enum Message {
    RunnerStartup {
        target_module: JsValue,
        target_memory: JsValue,
        target_function: JsString,
        communications: WasmgrindComs,
    },
    WorkerStartup {
        target_module: JsValue,
        target_memory: JsValue,
        thread_id: u32,
        start_routine: u32,
        communications: WasmgrindComs,
    },
    ThreadlinkRunnerStartup {
        target_module: JsValue,
        target_memory: JsValue,
        target_function: JsString,
        communications: ThreadlinkComs,
    },
    ThreadlinkWorkerStartup {
        target_module: JsValue,
        target_memory: JsValue,
        thread_id: u32,
        start_routine: u32,
        communications: ThreadlinkComs,
    },
    TraceGenerationStartup {
        communications: TraceGenerationComs,
    },
    TraceGenerationFinished,
    RunnerFinished,
}

impl Message {
    const RUNNER_STARTUP_TYPE: &str = "wgrind_runner_startup";
    const WORKER_STARTUP_TYPE: &str = "wgrind_worker_startup";
    const THREADLINK_RUNNER_STARTUP_TYPE: &str = "wgrind_threadlink_runner_startup";
    const THREADLINK_WORKER_STARTUP_TYPE: &str = "wgrind_threadlink_worker_startup";
    const TRACE_GENERATION_STARTUP_TYPE: &str = "wgrind_trace_generation_startup";
    const TRACE_GENERATION_FINISHED_TYPE: &str = "wgrind_trace_generation_finished";
    const RUNNER_FINISHED_TYPE: &str = "wgrind_runner_finished";

    pub fn try_to_json(self) -> Result<JsValue, JsValue> {
        let msg = Object::new();

        match self {
            Message::RunnerStartup {
                target_module,
                target_memory,
                target_function,
                communications,
            } => {
                Reflect::set(
                    &msg,
                    &JsValue::from("type"),
                    &JsValue::from(Message::RUNNER_STARTUP_TYPE),
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("wgrind_module"),
                    &wasm_bindgen::module(),
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("wgrind_memory"),
                    &wasm_bindgen::memory(),
                )?;
                Reflect::set(&msg, &JsValue::from("target_module"), &target_module)?;
                Reflect::set(&msg, &JsValue::from("target_memory"), &target_memory)?;
                Reflect::set(&msg, &JsValue::from("target_function"), &target_function)?;
                Reflect::set(
                    &msg,
                    &JsValue::from("communications"),
                    &BigInt::from(Box::into_raw(Box::new(communications)) as usize),
                )?;
            }
            Message::WorkerStartup {
                target_module,
                target_memory,
                thread_id,
                start_routine,
                communications,
            } => {
                Reflect::set(
                    &msg,
                    &JsValue::from("type"),
                    &JsValue::from(Message::WORKER_STARTUP_TYPE),
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("wgrind_module"),
                    &wasm_bindgen::module(),
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("wgrind_memory"),
                    &wasm_bindgen::memory(),
                )?;
                Reflect::set(&msg, &JsValue::from("target_module"), &target_module)?;
                Reflect::set(&msg, &JsValue::from("target_memory"), &target_memory)?;
                Reflect::set(
                    &msg,
                    &JsValue::from("thread_id"),
                    &serde_wasm_bindgen::to_value(&thread_id)?,
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("start_routine"),
                    &serde_wasm_bindgen::to_value(&start_routine)?,
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("communications"),
                    &BigInt::from(Box::into_raw(Box::new(communications)) as usize),
                )?;
            }
            Message::ThreadlinkRunnerStartup {
                target_module,
                target_memory,
                target_function,
                communications,
            } => {
                Reflect::set(
                    &msg,
                    &JsValue::from("type"),
                    &JsValue::from(Message::THREADLINK_RUNNER_STARTUP_TYPE),
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("wgrind_module"),
                    &wasm_bindgen::module(),
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("wgrind_memory"),
                    &wasm_bindgen::memory(),
                )?;
                Reflect::set(&msg, &JsValue::from("target_module"), &target_module)?;
                Reflect::set(&msg, &JsValue::from("target_memory"), &target_memory)?;
                Reflect::set(&msg, &JsValue::from("target_function"), &target_function)?;
                Reflect::set(
                    &msg,
                    &JsValue::from("communications"),
                    &BigInt::from(Box::into_raw(Box::new(communications)) as usize),
                )?;
            }
            Message::ThreadlinkWorkerStartup {
                target_module,
                target_memory,
                thread_id,
                start_routine,
                communications,
            } => {
                Reflect::set(
                    &msg,
                    &JsValue::from("type"),
                    &JsValue::from(Message::THREADLINK_WORKER_STARTUP_TYPE),
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("wgrind_module"),
                    &wasm_bindgen::module(),
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("wgrind_memory"),
                    &wasm_bindgen::memory(),
                )?;
                Reflect::set(&msg, &JsValue::from("target_module"), &target_module)?;
                Reflect::set(&msg, &JsValue::from("target_memory"), &target_memory)?;
                Reflect::set(
                    &msg,
                    &JsValue::from("thread_id"),
                    &serde_wasm_bindgen::to_value(&thread_id)?,
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("start_routine"),
                    &serde_wasm_bindgen::to_value(&start_routine)?,
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("communications"),
                    &BigInt::from(Box::into_raw(Box::new(communications)) as usize),
                )?;
            }
            Message::TraceGenerationStartup { communications } => {
                Reflect::set(
                    &msg,
                    &JsValue::from("type"),
                    &JsValue::from(Message::TRACE_GENERATION_STARTUP_TYPE),
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("wgrind_module"),
                    &wasm_bindgen::module(),
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("wgrind_memory"),
                    &wasm_bindgen::memory(),
                )?;
                Reflect::set(
                    &msg,
                    &JsValue::from("communications"),
                    &BigInt::from(Box::into_raw(Box::new(communications)) as usize),
                )?;
            }
            Message::TraceGenerationFinished => {
                Reflect::set(
                    &msg,
                    &JsValue::from("type"),
                    &JsValue::from(Message::TRACE_GENERATION_FINISHED_TYPE),
                )?;
            }
            Message::RunnerFinished => {
                Reflect::set(
                    &msg,
                    &JsValue::from("type"),
                    &JsValue::from(Message::RUNNER_FINISHED_TYPE),
                )?;
            }
        }

        Ok(msg.into())
    }

    pub fn try_from_json(msg: JsValue) -> Result<Self, JsValue> {
        let msg_type: String = Reflect::get(&msg, &JsValue::from("type"))?
            .dyn_into::<JsString>()?
            .into();

        match msg_type.as_str() {
            Message::RUNNER_STARTUP_TYPE => {
                let target_module = Reflect::get(&msg, &JsValue::from("target_module"))?;
                let target_memory = Reflect::get(&msg, &JsValue::from("target_memory"))?;
                let target_function = Reflect::get(&msg, &JsValue::from("target_function"))?
                    .dyn_into::<JsString>()?;
                let com_ptr_js =
                    Reflect::get(&msg, &JsValue::from("communications"))?.dyn_into::<BigInt>()?;
                let com_ptr = u128::try_from(com_ptr_js)? as usize as *mut WasmgrindComs; // usize has no From<BigInt> so we use the biggest unsigned int possible
                let communications = unsafe { *Box::from_raw(com_ptr) };
                Ok(Message::RunnerStartup {
                    target_module,
                    target_memory,
                    target_function,
                    communications,
                })
            }
            Message::WORKER_STARTUP_TYPE => {
                let target_module = Reflect::get(&msg, &JsValue::from("target_module"))?;
                let target_memory = Reflect::get(&msg, &JsValue::from("target_memory"))?;
                let thread_id = serde_wasm_bindgen::from_value(Reflect::get(
                    &msg,
                    &JsValue::from("thread_id"),
                )?)?;
                let start_routine = serde_wasm_bindgen::from_value(Reflect::get(
                    &msg,
                    &JsValue::from("start_routine"),
                )?)?;
                let com_ptr_js =
                    Reflect::get(&msg, &JsValue::from("communications"))?.dyn_into::<BigInt>()?;
                let com_ptr = u128::try_from(com_ptr_js)? as usize as *mut WasmgrindComs; // usize has no From<BigInt> so we use the biggest unsigned int possible
                let communications = unsafe { *Box::from_raw(com_ptr) };
                Ok(Message::WorkerStartup {
                    target_module,
                    target_memory,
                    thread_id,
                    start_routine,
                    communications,
                })
            }
            Message::THREADLINK_RUNNER_STARTUP_TYPE => {
                let target_module = Reflect::get(&msg, &JsValue::from("target_module"))?;
                let target_memory = Reflect::get(&msg, &JsValue::from("target_memory"))?;
                let target_function = Reflect::get(&msg, &JsValue::from("target_function"))?
                    .dyn_into::<JsString>()?;
                let com_ptr_js =
                    Reflect::get(&msg, &JsValue::from("communications"))?.dyn_into::<BigInt>()?;
                let com_ptr = u128::try_from(com_ptr_js)? as usize as *mut ThreadlinkComs; // usize has no From<BigInt> so we use the biggest unsigned int possible
                let communications = unsafe { *Box::from_raw(com_ptr) };
                Ok(Message::ThreadlinkRunnerStartup {
                    target_module,
                    target_memory,
                    target_function,
                    communications,
                })
            }
            Message::THREADLINK_WORKER_STARTUP_TYPE => {
                let target_module = Reflect::get(&msg, &JsValue::from("target_module"))?;
                let target_memory = Reflect::get(&msg, &JsValue::from("target_memory"))?;
                let thread_id = serde_wasm_bindgen::from_value(Reflect::get(
                    &msg,
                    &JsValue::from("thread_id"),
                )?)?;
                let start_routine = serde_wasm_bindgen::from_value(Reflect::get(
                    &msg,
                    &JsValue::from("start_routine"),
                )?)?;
                let com_ptr_js =
                    Reflect::get(&msg, &JsValue::from("communications"))?.dyn_into::<BigInt>()?;
                let com_ptr = u128::try_from(com_ptr_js)? as usize as *mut ThreadlinkComs; // usize has no From<BigInt> so we use the biggest unsigned int possible
                let communications = unsafe { *Box::from_raw(com_ptr) };
                Ok(Message::ThreadlinkWorkerStartup {
                    target_module,
                    target_memory,
                    thread_id,
                    start_routine,
                    communications,
                })
            }
            Message::TRACE_GENERATION_STARTUP_TYPE => {
                let com_ptr_js =
                    Reflect::get(&msg, &JsValue::from("communications"))?.dyn_into::<BigInt>()?;
                let com_ptr = u128::try_from(com_ptr_js)? as usize as *mut TraceGenerationComs; // usize has no From<BigInt> so we use the biggest unsigned int possible
                let communications = unsafe { *Box::from_raw(com_ptr) };
                Ok(Message::TraceGenerationStartup { communications })
            }
            Message::TRACE_GENERATION_FINISHED_TYPE => Ok(Message::TraceGenerationFinished),
            Message::RUNNER_FINISHED_TYPE => Ok(Message::RunnerFinished),
            _ => Err(JsError::new(&format!(
                "Tried to parse message of unknown type {}",
                msg_type
            ))
            .into()),
        }
    }
}

#[wasm_bindgen]
pub async fn handle_message(msg: JsValue) -> Result<(), JsValue> {
    let message = match Message::try_from_json(msg) {
        Ok(message) => message,
        Err(_) => return Ok(()),
    };

    match message {
        Message::RunnerStartup {
            target_module,
            target_memory,
            target_function,
            communications,
        } => {
            let context = WasmgrindContext::new(
                target_module.dyn_into()?,
                target_memory.dyn_into()?,
                None,
                communications,
            )?;

            let instance: WebAssembly::Instance = JsFuture::from(WebAssembly::instantiate_module(
                &context.get_target_module(),
                &context.get_wasm_imports()?,
            ))
            .await?
            .dyn_into()?;

            let func =
                Reflect::get(&instance.exports(), &target_function)?.dyn_into::<Function>()?;
            func.call0(&JsValue::undefined())?;

            let global = global().dyn_into::<DedicatedWorkerGlobalScope>()?;
            global.post_message(&Message::RunnerFinished.try_to_json()?)?;

            context.close()?;
            global.close();
        }
        Message::WorkerStartup {
            target_module,
            target_memory,
            thread_id,
            start_routine,
            communications,
        } => {
            let context = WasmgrindContext::new(
                target_module.dyn_into()?,
                target_memory.dyn_into()?,
                Some(thread_id),
                communications,
            )?;

            let instance: WebAssembly::Instance = JsFuture::from(WebAssembly::instantiate_module(
                &context.get_target_module(),
                &context.get_wasm_imports()?,
            ))
            .await?
            .dyn_into()?;

            let func = Reflect::get(&instance.exports(), &JsValue::from("thread_start"))?
                .dyn_into::<Function>()?;
            func.call1(&JsValue::undefined(), &JsValue::from(start_routine))?;

            let global = global().dyn_into::<DedicatedWorkerGlobalScope>()?;

            context.close()?;
            global.close();
        }
        Message::ThreadlinkRunnerStartup {
            target_module,
            target_memory,
            target_function,
            communications,
        } => {
            let context = ThreadlinkContext::new(
                target_module.dyn_into()?,
                target_memory.dyn_into()?,
                None,
                communications,
            )?;

            let instance: WebAssembly::Instance = JsFuture::from(WebAssembly::instantiate_module(
                &context.get_target_module(),
                &context.get_wasm_imports()?,
            ))
            .await?
            .dyn_into()?;

            let func =
                Reflect::get(&instance.exports(), &target_function)?.dyn_into::<Function>()?;
            func.call0(&JsValue::undefined())?;

            let global = global().dyn_into::<DedicatedWorkerGlobalScope>()?;
            global.post_message(&Message::RunnerFinished.try_to_json()?)?;

            context.close()?;
            global.close();
        }
        Message::ThreadlinkWorkerStartup {
            target_module,
            target_memory,
            thread_id,
            start_routine,
            communications,
        } => {
            let context = ThreadlinkContext::new(
                target_module.dyn_into()?,
                target_memory.dyn_into()?,
                Some(thread_id),
                communications,
            )?;

            let instance: WebAssembly::Instance = JsFuture::from(WebAssembly::instantiate_module(
                &context.get_target_module(),
                &context.get_wasm_imports()?,
            ))
            .await?
            .dyn_into()?;

            let func = Reflect::get(&instance.exports(), &JsValue::from("thread_start"))?
                .dyn_into::<Function>()?;
            func.call1(&JsValue::undefined(), &JsValue::from(start_routine))?;

            let global = global().dyn_into::<DedicatedWorkerGlobalScope>()?;

            context.close()?;
            global.close();
        }
        Message::TraceGenerationStartup { communications } => {
            communications
                .receive_and_reply(|tracing| tracing.generate_binary_trace())
                .map_err(|e| JsError::from(&*e))?;

            let global = global().dyn_into::<DedicatedWorkerGlobalScope>()?;
            global.post_message(&Message::TraceGenerationFinished.try_to_json()?)?;
            global.close();
        }
        Message::TraceGenerationFinished | Message::RunnerFinished => {
            return Err(JsError::new("Response messages are not handled by worker!").into());
        }
    }

    Ok(())
}
