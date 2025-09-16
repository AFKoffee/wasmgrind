use std::{collections::{HashMap, HashSet}, hash::Hash};

use crate::generic;

use super::{
    metadata::WasmgrindTraceMetadata,
    representation::{Event, Op},
};

struct WasmgrindToGeneric<T> {
    map: HashMap<T, u64>,
    counter: u64,
}

impl<T: Eq + Hash + Copy> WasmgrindToGeneric<T> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            counter: 0,
        }
    }

    pub fn get_identifier(&mut self, value: &T) -> u64 {
        if let Some(tid) = self.map.get(value) {
            *tid
        } else {
            let tid = self.counter;
            self.map.insert(*value, tid);
            self.counter += 1;
            tid
        }
    }

    pub fn get_map(&self) -> &HashMap<T, u64> {
        &self.map
    }
}

pub struct WasmgrindTraceConverter {
    threads: WasmgrindToGeneric<u32>,
    variables: WasmgrindToGeneric<(u32, u32)>,
    locks: WasmgrindToGeneric<u32>,
    locations: WasmgrindToGeneric<(u32, u32)>,
    shared_variables: HashMap<u64, HashSet<u64>>,
}

impl WasmgrindTraceConverter {
    pub fn new() -> Self {
        Self {
            threads: WasmgrindToGeneric::new(),
            variables: WasmgrindToGeneric::new(),
            locks: WasmgrindToGeneric::new(),
            locations: WasmgrindToGeneric::new(),
            shared_variables: HashMap::new()
        }
    }

    pub fn convert_event(&mut self, event: &Event) -> generic::Event {
        let Event { t, op, loc } = event;

        let thread_id = self.threads.get_identifier(t);
        let operation = match op {
            Op::Read { addr, n } => {
                let variable_id = self.variables.get_identifier(&(*addr, *n));
                self.shared_variables.entry(variable_id)
                    .or_default()
                    .insert(thread_id);
                generic::Operation::Read {
                    memory: variable_id,
                }
            },
            Op::Write { addr, n } => {
                let variable_id = self.variables.get_identifier(&(*addr, *n));
                self.shared_variables.entry(variable_id)
                    .or_default()
                    .insert(thread_id);
                generic::Operation::Write {
                    memory: variable_id,
                }
            },
            Op::Aquire { lock } => generic::Operation::Aquire {
                lock: self.locks.get_identifier(lock),
            },
            Op::Request { lock } => generic::Operation::Request {
                lock: self.locks.get_identifier(lock),
            },
            Op::Release { lock } => generic::Operation::Release {
                lock: self.locks.get_identifier(lock),
            },
            Op::Fork { tid } => generic::Operation::Fork {
                tid: self.threads.get_identifier(tid),
            },
            Op::Join { tid } => generic::Operation::Join {
                tid: self.threads.get_identifier(tid),
            },
        };
        let location = self.locations.get_identifier(loc);

        generic::Event::new(thread_id, operation, location)
    }

    pub fn generate_metadata(&self) -> WasmgrindTraceMetadata {
        let mut metadata = WasmgrindTraceMetadata::new();

        metadata.fill_thread_records(self.threads.get_map());
        metadata.fill_memory_records(self.variables.get_map());
        metadata.fill_lock_records(self.locks.get_map());
        metadata.fill_location_records(self.locations.get_map());
        metadata.fill_shared_variables(&self.shared_variables);

        metadata
    }
}
