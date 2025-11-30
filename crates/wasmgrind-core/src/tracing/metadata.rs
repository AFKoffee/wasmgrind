use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use anyhow::{Error, anyhow};
use serde::{Deserialize, Serialize};
use trace_tools::{
    RapidBinParser,
    generic::{self, Operation, Parser},
};

use crate::tracing::{Op, metadata::analysis::line_sweep_algorithm, representation::Event};

mod analysis;

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Hash)]
struct MemoryIdentifier {
    address: u32,
    access_width: u32,
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

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Hash)]
struct MemoryRecord {
    wasm_id: MemoryIdentifier,
    trace_id: u64,
}

impl MemoryRecord {
    fn into_fields(self) -> ((u32, u32), u64) {
        (
            (self.wasm_id.address, self.wasm_id.access_width),
            self.trace_id,
        )
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
    shared_variables: HashMap<u64, HashSet<u64>>,
}

impl WasmgrindTraceMetadata {
    pub(super) fn new() -> Self {
        Self {
            thread_records: Vec::new(),
            memory_records: Vec::new(),
            lock_records: Vec::new(),
            location_records: Vec::new(),
            shared_variables: HashMap::new(),
        }
    }

    pub(super) fn into_converter(self) -> GenericTraceConverter {
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

    pub(super) fn fill_thread_records(&mut self, map: &HashMap<u32, u64>) {
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

    pub(super) fn fill_memory_records(&mut self, map: &HashMap<(u32, u32), u64>) {
        self.memory_records.clear();

        for ((k1, k2), v) in map.iter() {
            self.memory_records.push(MemoryRecord {
                wasm_id: MemoryIdentifier {
                    address: *k1,
                    access_width: *k2,
                },
                trace_id: *v,
            });
        }
        self.memory_records
            .sort_by(|r1, r2| r1.trace_id.cmp(&r2.trace_id));
    }

    pub(super) fn fill_lock_records(&mut self, map: &HashMap<u32, u64>) {
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

    pub(super) fn fill_location_records(&mut self, map: &HashMap<(u32, u32), u64>) {
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

    pub(super) fn fill_shared_variables(&mut self, map: &HashMap<u64, HashSet<u64>>) {
        self.shared_variables = map
            .iter()
            .filter(|(_, set)| set.len() > 1)
            .map(|(x, y)| (*x, y.clone()))
            .collect();
    }

    /// Attempts to serialize the metadata to JSON format.
    pub fn to_json(&self) -> Result<String, Error> {
        serde_json::to_string_pretty(&self).map_err(Error::from)
    }

    /// Attempts to build a metadata struct from data provided in JSON format.
    ///
    /// This function will not buffer the provided reader. If you
    /// need buffering, you need to apply your own buffering, e.g.,
    /// by using [`std::io::BufReader`].
    pub fn from_json<R: Read>(reader: R) -> Result<Self, Error> {
        serde_json::from_reader(reader).map_err(Error::from)
    }

    fn find_overlaps_internal(&'_ self) -> Vec<Overlap<'_>> {
        // We filter for memory accesses here that are shared amongst different threads.
        // If memory accesses overlap in the same thread we trust the compiler to have
        // it figured out correctly. Anyways, if there is corrupted data due to overlapping
        // memory accesses inside a single thread only, this is no concurrency related error
        // so this does not bother us right now.
        line_sweep_algorithm(
            self.memory_records
                .iter()
                .filter(|record| self.shared_variables.contains_key(&record.trace_id)),
        )
        .into_iter()
        .filter_map(|(access_x, access_y)| {
            let threads_x = self
                .shared_variables
                .get(&access_x.trace_id)
                .expect("Should be present!");
            let threads_y = self
                .shared_variables
                .get(&access_y.trace_id)
                .expect("Should be present!");

            if threads_x.intersection(threads_y).count() > 0 {
                Some(Overlap {
                    threads_x,
                    access_x,
                    threads_y,
                    access_y,
                })
            } else {
                None
            }
        })
        .collect()
    }

    /// Determines all pairwise overlaps of distinct memory accesses in this execution trace.
    ///
    /// This function will collect all pairwise overlaps of distinct memory accesses,
    /// the total number of memory-access events in this trace as well as the proportion
    /// of these memory-access events that contain overlapping memory accesses. The information
    /// can then be queried via the returned [`Overlaps`] instance.
    ///
    /// Refer to [`Overlaps::get_overlaps`] for details on how pairwise overlaps are determined.
    pub fn find_overlaps<P: AsRef<Path>>(
        &'_ self,
        rapid_bin_file: P,
    ) -> Result<Overlaps<'_>, Error> {
        let overlaps = self.find_overlaps_internal();

        let mut parser = RapidBinParser::new();
        let mut n_memory_events = 0;
        let mut n_overlap_events = 0;
        let trace_reader = BufReader::new(File::open(rapid_bin_file)?);
        for event in parser.parse(trace_reader)? {
            let (_, op, _) = event?.into_fields();
            match op {
                Operation::Read { memory } | Operation::Write { memory } => {
                    n_memory_events += 1;
                    if overlaps.iter().any(|overlap| overlap.contains(memory)) {
                        n_overlap_events += 1;
                    }
                }
                _ => continue,
            }
        }

        Ok(Overlaps {
            overlaps,
            n_memory_events,
            n_overlap_events,
        })
    }
}

pub struct Overlaps<'a> {
    overlaps: Vec<Overlap<'a>>,
    n_memory_events: usize,
    n_overlap_events: usize,
}

impl<'a> Overlaps<'a> {
    /// Returns a reference to a list of all pairwise overlaps of distinct memory accesses.
    ///
    /// The overlaps in this list are selected by the following criteria:
    /// - The memory accesses have to share at least one byte of targeted memory
    /// - The memory accesses need to occur amongst different threads throughout the
    ///   execution trace
    ///
    /// Any memory access targeting the same address with the same number of accessed
    /// bytes is only counted **once**.
    pub fn get_overlaps(&self) -> &Vec<Overlap<'a>> {
        &self.overlaps
    }

    /// Returns the proportion of overlaps compared to all memory accesses.
    ///
    /// The function returns a tuple of two values:
    /// - 1st value:  The number of overlapping memory accesses contained in the trace
    /// - 2nd value:  The number of all memory accesses contained in the trace
    ///
    /// The overlapping memory accesses are determined by the same criteria as stated
    /// in the documentation of [`Overlaps::get_overlaps`]. The function counts the
    /// number of **events** that match the criteria. Therefore, if any memory access
    /// targeting the same address with the same number of accessed bytes appears
    /// more than once throughout the execution trace, it will be counted **multiple
    /// times**.
    pub fn get_overlap_ratio(&self) -> (usize, usize) {
        (self.n_overlap_events, self.n_memory_events)
    }
}

/// A pair of two distinct memory accesses that share at least one byte of targeted memory.
#[derive(PartialEq, Eq)]
pub struct Overlap<'a> {
    threads_x: &'a HashSet<u64>,
    access_x: &'a MemoryRecord,
    threads_y: &'a HashSet<u64>,
    access_y: &'a MemoryRecord,
}

impl Overlap<'_> {
    fn is_intersection(&self) -> bool {
        let start_x = self.access_x.wasm_id.address;
        let start_y = self.access_y.wasm_id.address;

        let length_x = self.access_x.wasm_id.access_width;
        let length_y = self.access_y.wasm_id.access_width;

        let end_x = start_x + length_x;
        let end_y = start_y + length_y;

        if start_x == start_y {
            false
        } else if start_x < start_y && end_x > start_y {
            end_x < end_y
        } else if start_y < start_x && end_y > start_x {
            end_y < end_x
        } else {
            panic!("Overlap struct contained non overlapping memory accesses")
        }
    }

    /// Creates a short message describing the overlap.
    ///
    /// Specifically, this message contains the following information:
    /// -   The threads among which each of the two memory accesses is shared
    /// -   The type of overlap: Do the memory accesses only _intersect_ or does
    ///     one access _contain_ the other?
    /// -   The unique ID, target memory address and number of accessed bytes
    ///     for both memory accesses.
    pub fn description(&self) -> String {
        let id_x = self.access_x.trace_id;
        let id_y = self.access_y.trace_id;

        let start_x = self.access_x.wasm_id.address;
        let start_y = self.access_y.wasm_id.address;

        let length_x = self.access_x.wasm_id.access_width;
        let length_y = self.access_y.wasm_id.access_width;

        let general_msg = format!(
            "Memory access {} (threads: {}) overlaps with memory access {} (threads: {}) - ",
            id_x,
            self.threads_x
                .iter()
                .map(|tid| tid.to_string())
                .collect::<Vec<String>>()
                .join(", "),
            id_y,
            self.threads_y
                .iter()
                .map(|tid| tid.to_string())
                .collect::<Vec<String>>()
                .join(", ")
        );

        let specific_msg = if self.is_intersection() {
            format!(
                "Access {} at {} of length {} {} access {} at {} of length {}",
                id_x, start_x, length_x, "intersects with", id_y, start_y, length_y,
            )
        } else if length_x > length_y {
            format!(
                "Access {} at {} of length {} {} access {} at {} of length {}",
                id_x, start_x, length_x, "contains", id_y, start_y, length_y,
            )
        } else if length_x < length_y {
            format!(
                "Access {} at {} of length {} {} access {} at {} of length {}",
                id_y, start_y, length_y, "contains", id_x, start_x, length_x,
            )
        } else {
            format!(
                "Equal memory accesses {} and {} obviously overlap!",
                id_x, id_y
            )
        };

        format!("{general_msg}{specific_msg}")
    }

    pub(super) fn contains(&self, memory_access: u64) -> bool {
        self.access_x.trace_id == memory_access || self.access_y.trace_id == memory_access
    }
}

impl Default for WasmgrindTraceMetadata {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) struct GenericTraceConverter {
    threads: HashMap<u64, u32>,
    variables: HashMap<u64, (u32, u32)>,
    locks: HashMap<u64, u32>,
    locations: HashMap<u64, (u32, u32)>,
}

impl GenericTraceConverter {
    pub(super) fn convert_event(&self, event: &generic::Event) -> Result<Event, Error> {
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
                Op::Read {
                    addr,
                    n,
                    atomic: false,
                } // BEWARE: Trace format has no notion of atomic accesses.
            }
            generic::Operation::Write { memory } => {
                let (addr, n) = *self
                    .variables
                    .get(memory)
                    .ok_or(anyhow!("Variable-ID not present in metadata"))?;
                Op::Write {
                    addr,
                    n,
                    atomic: false,
                } // BEWARE: Trace format has no notion of atomic accesses.
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
