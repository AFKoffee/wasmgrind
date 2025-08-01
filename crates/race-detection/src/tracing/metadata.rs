use std::{collections::HashMap, io::Read};

use anyhow::{Error, anyhow};
use serde::{Deserialize, Serialize};

use crate::{
    generic,
    tracing::{Op, representation::Event},
};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
struct MemoryIdentifier {
    address: u32,
    align: u32,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
struct ThreadRecord {
    wasm_id: u32,
    trace_id: u64,
}

impl ThreadRecord {
    fn into_fields(self) -> (u32, u64) {
        (self.wasm_id, self.trace_id)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
struct MemoryRecord {
    wasm_id: MemoryIdentifier,
    trace_id: u64,
}

impl MemoryRecord {
    fn into_fields(self) -> ((u32, u32), u64) {
        ((self.wasm_id.address, self.wasm_id.align), self.trace_id)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
struct LockRecord {
    wasm_id: u32,
    trace_id: u64,
}

impl LockRecord {
    fn into_fields(self) -> (u32, u64) {
        (self.wasm_id, self.trace_id)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
struct LocationIdentifier {
    fidx: u32,
    iidx: u32,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
struct LocationRecord {
    wasm_id: LocationIdentifier,
    trace_id: u64,
}

impl LocationRecord {
    fn into_fields(self) -> ((u32, u32), u64) {
        ((self.wasm_id.fidx, self.wasm_id.iidx), self.trace_id)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct WasmgrindTraceMetadata {
    thread_records: Vec<ThreadRecord>,
    memory_records: Vec<MemoryRecord>,
    lock_records: Vec<LockRecord>,
    location_records: Vec<LocationRecord>,
}

impl WasmgrindTraceMetadata {
    pub fn new() -> Self {
        Self {
            thread_records: Vec::new(),
            memory_records: Vec::new(),
            lock_records: Vec::new(),
            location_records: Vec::new(),
        }
    }

    pub fn into_converter(self) -> GenericTraceConverter {
        GenericTraceConverter {
            threads: HashMap::from_iter(
                self.thread_records
                    .into_iter()
                    .map(|record| record.into_fields())
                    .map(|(fst, snd)| (snd, fst)),
            ),
            variables: HashMap::from_iter(
                self.memory_records
                    .into_iter()
                    .map(|record| record.into_fields())
                    .map(|(fst, snd)| (snd, fst)),
            ),
            locks: HashMap::from_iter(
                self.lock_records
                    .into_iter()
                    .map(|record| record.into_fields())
                    .map(|(fst, snd)| (snd, fst)),
            ),
            locations: HashMap::from_iter(
                self.location_records
                    .into_iter()
                    .map(|record| record.into_fields())
                    .map(|(fst, snd)| (snd, fst)),
            ),
        }
    }

    pub fn fill_thread_records(&mut self, map: &HashMap<u32, u64>) {
        self.thread_records.clear();

        for (k, v) in map.iter() {
            self.thread_records.push(ThreadRecord {
                wasm_id: *k,
                trace_id: *v,
            });
        }

        self.thread_records
            .sort_by(|r1, r2| r1.trace_id.cmp(&r2.trace_id));
    }

    pub fn fill_memory_records(&mut self, map: &HashMap<(u32, u32), u64>) {
        self.memory_records.clear();

        for ((k1, k2), v) in map.iter() {
            self.memory_records.push(MemoryRecord {
                wasm_id: MemoryIdentifier {
                    address: *k1,
                    align: *k2,
                },
                trace_id: *v,
            });
        }
        self.memory_records
            .sort_by(|r1, r2| r1.trace_id.cmp(&r2.trace_id));
    }

    pub fn fill_lock_records(&mut self, map: &HashMap<u32, u64>) {
        self.lock_records.clear();

        for (k, v) in map.iter() {
            self.lock_records.push(LockRecord {
                wasm_id: *k,
                trace_id: *v,
            });
        }

        self.lock_records
            .sort_by(|r1, r2| r1.trace_id.cmp(&r2.trace_id));
    }

    pub fn fill_location_records(&mut self, map: &HashMap<(u32, u32), u64>) {
        self.location_records.clear();

        for ((k1, k2), v) in map.iter() {
            self.location_records.push(LocationRecord {
                wasm_id: LocationIdentifier {
                    fidx: *k1,
                    iidx: *k2,
                },
                trace_id: *v,
            });
        }

        self.location_records
            .sort_by(|r1, r2| r1.trace_id.cmp(&r2.trace_id));
    }

    pub fn to_json(&self) -> Result<String, Error> {
        serde_json::to_string_pretty(&self).map_err(Error::from)
    }

    pub fn from_json<R: Read>(reader: R) -> Result<Self, Error> {
        serde_json::from_reader(reader).map_err(Error::from)
    }
}

impl Default for WasmgrindTraceMetadata {
    fn default() -> Self {
        Self::new()
    }
}

pub struct GenericTraceConverter {
    threads: HashMap<u64, u32>,
    variables: HashMap<u64, (u32, u32)>,
    locks: HashMap<u64, u32>,
    locations: HashMap<u64, (u32, u32)>,
}

impl GenericTraceConverter {
    pub fn convert_event(&self, event: &generic::Event) -> Result<Event, Error> {
        let (tid, operation, loc) = event.get_fields();

        let thread = self
            .threads
            .get(tid)
            .ok_or(anyhow!("Thread-ID not present in metadata"))?;
        let location = self
            .locations
            .get(loc)
            .ok_or(anyhow!("Location-ID not present in metadata"))?;
        let op = match operation {
            generic::Operation::Aquire { lock } => Op::Aquire {
                lock: *self
                    .locks
                    .get(lock)
                    .ok_or(anyhow!("Lock-ID not present in metadata"))?,
            },
            generic::Operation::Release { lock } => Op::Release {
                lock: *self
                    .locks
                    .get(lock)
                    .ok_or(anyhow!("Lock-ID not present in metadata"))?,
            },
            generic::Operation::Read { memory } => {
                let (addr, n) = *self
                    .variables
                    .get(memory)
                    .ok_or(anyhow!("Variable-ID not present in metadata"))?;
                Op::Read { addr, n }
            }
            generic::Operation::Write { memory } => {
                let (addr, n) = *self
                    .variables
                    .get(memory)
                    .ok_or(anyhow!("Variable-ID not present in metadata"))?;
                Op::Write { addr, n }
            }
            generic::Operation::Fork { tid } => Op::Fork {
                tid: *self
                    .threads
                    .get(tid)
                    .ok_or(anyhow!("Thread-ID not present in metadata"))?,
            },
            generic::Operation::Join { tid } => Op::Join {
                tid: *self
                    .threads
                    .get(tid)
                    .ok_or(anyhow!("Thread-ID not present in metadata"))?,
            },
            generic::Operation::Request { lock } => Op::Request {
                lock: *self
                    .locks
                    .get(lock)
                    .ok_or(anyhow!("Lock-ID not present in metadata"))?,
            },
        };

        Ok(Event {
            t: *thread,
            op,
            loc: *location,
        })
    }
}
