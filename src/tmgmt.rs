use std::{collections::HashMap, thread::JoinHandle};

use anyhow::{Error, anyhow};
use wasmgrind_core::tmgmt::ConditionalHandle;

pub struct ThreadManagement<T> {
    threads: HashMap<u32, ConditionalHandle<JoinHandle<T>>>,
}

impl<T> ThreadManagement<T> {
    pub fn new() -> Self {
        Self {
            threads: HashMap::new(),
        }
    }

    pub fn register_thread(&mut self) -> u32 {
        let thread_id = wasmgrind_core::tmgmt::next_available_thread_id();
        let handle = ConditionalHandle::new();
        self.threads.insert(thread_id, handle);

        thread_id
    }

    pub fn set_join_handle(&self, thread_id: u32, handle: JoinHandle<T>) -> Result<(), Error> {
        if let Some(cond_handle) = self.threads.get(&thread_id) {
            cond_handle.set_and_notify(handle)
        } else {
            Err(anyhow!(
                "Tried to set a join handle for a non-existing thread."
            ))
        }
    }

    pub fn retrieve_thread(&mut self, thread_id: u32) -> Option<ConditionalHandle<JoinHandle<T>>> {
        self.threads.remove(&thread_id)
    }
}

impl<T> Default for ThreadManagement<T> {
    fn default() -> Self {
        Self::new()
    }
}
