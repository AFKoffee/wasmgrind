use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use anyhow::{Error, anyhow};
use wasm_bindgen::JsError;
use wasmgrind_core::tmgmt::ConditionalHandle;

struct JsThreadManagement {
    running: HashSet<u32>,
    pending: HashMap<u32, Arc<ConditionalHandle<i32>>>,
    done: HashMap<u32, Arc<ConditionalHandle<i32>>>,
}

impl JsThreadManagement {
    pub fn new() -> Self {
        Self {
            running: HashSet::new(),
            pending: HashMap::new(),
            done: HashMap::new(),
        }
    }

    pub fn register_thread(&mut self) -> u32 {
        let thread_id = wasmgrind_core::tmgmt::next_available_thread_id();
        self.running.insert(thread_id);

        thread_id
    }

    pub fn set_return_val(&mut self, thread_id: u32, return_val: i32) -> Result<(), Error> {
        if self.running.remove(&thread_id) {
            if let Some(handle) = self.pending.remove(&thread_id) {
                handle.set_and_notify(return_val)
            } else {
                self.done.insert(
                    thread_id,
                    Arc::new(ConditionalHandle::with_value(return_val)),
                );
                Ok(())
            }
        } else {
            Err(anyhow!(
                "Tried to set a return value for a non existing thread!"
            ))
        }
    }

    pub fn retrieve_thread(&mut self, thread_id: u32) -> Option<Arc<ConditionalHandle<i32>>> {
        if let Some(handle) = self.done.remove(&thread_id) {
            Some(handle)
        } else if self.running.contains(&thread_id) {
            let handle = Arc::new(ConditionalHandle::new());
            self.pending.insert(thread_id, handle.clone());

            Some(handle)
        } else {
            None
        }
    }
}

pub struct SyncedJsTmgmt(Mutex<JsThreadManagement>);

impl SyncedJsTmgmt {
    pub fn new() -> Self {
        Self(Mutex::new(JsThreadManagement::new()))
    }

    pub fn register_thread(&self) -> Result<u32, JsError> {
        match self.0.lock() {
            Ok(mut tmgmt) => Ok(tmgmt.register_thread()),
            Err(_) => Err(JsError::new("JsThreadManagement Mutex was poisoned!")),
        }
    }

    pub fn set_return_val(&self, thread_id: u32, return_val: i32) -> Result<(), JsError> {
        match self.0.lock() {
            Ok(mut tmgmt) => tmgmt
                .set_return_val(thread_id, return_val)
                .map_err(|e| JsError::from(&*e)),
            Err(_) => Err(JsError::new("JsThreadManagement Mutex was poisoned!")),
        }
    }

    pub fn join(&self, thread_id: u32) -> Result<i32, JsError> {
        let mut tmgmt = match self.0.lock() {
            Ok(tmgmt) => tmgmt,
            Err(_) => return Err(JsError::new("JsThreadManagement Mutex was poisoned!")),
        };

        let handle = match tmgmt.retrieve_thread(thread_id) {
            Some(handle) => handle,
            None => {
                return Err(JsError::new(
                    "Given thread-id did not belong to a registered thread!",
                ));
            }
        };

        // Its important to drop the guard early here in order
        // to prevent deadlocks on the `JsThreadManagement` struct here
        drop(tmgmt);

        handle.take_when_ready().map_err(|e| JsError::from(&*e))
    }
}

pub fn thread_id() -> Result<u32, JsError> {
    wasmgrind_core::tmgmt::thread_id().map_err(|e| JsError::from(&*e))
}
