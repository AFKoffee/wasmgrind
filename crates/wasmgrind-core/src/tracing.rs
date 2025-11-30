use std::{
    cell::RefCell,
    collections::HashMap,
    fs::File,
    io::BufWriter,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

use anyhow::Error;
use representation::Event;
use trace_tools::{generic::Encoder, rapidbin::encoder::RapidBinEncoder};

use crate::tracing::{
    converter::WasmgrindTraceConverter,
    metadata::WasmgrindTraceMetadata,
    trace::{EventHandle, Trace},
};

mod converter;

/// Utilities to manage metadata of Wasmgrind execution traces.
pub mod metadata;
mod representation;
mod trace;

pub use representation::Op;

thread_local! {
    static THREAD_STATE: RefCell<ThreadState> = const { RefCell::new(ThreadState { id: None, ignore_memory_events: false }) };
}

pub type Tid = u32;

struct ThreadState {
    id: Option<Tid>,
    ignore_memory_events: bool,
}

struct ThreadRecord {
    id: Tid,
}

struct MutexRecord {
    id: u32,
    owner: Tid,
    last_event: Option<EventHandle>,
}

pub struct Tracing {
    tid_counter: AtomicU32,
    mutex_counter: AtomicU32,
    initialized: AtomicBool,
    events: Trace,
    threads: Mutex<HashMap<u32, ThreadRecord>>,
    mutexes: Mutex<HashMap<u32, MutexRecord>>,
}

impl Tracing {
    pub const THREAD_CREATE_JOINABLE: u32 = 0;
    pub const THREAD_CREATE_DETACHED: u32 = 1;
    pub const MUTEX_INIT_NORMAL: u32 = 0;
    pub const MUTEX_INIT_RECURSIVE: u32 = 1;

    /// Creates an empty execution trace.
    pub fn new<P: AsRef<Path>>(cache_dir: P) -> Self {
        Self {
            tid_counter: AtomicU32::new(0),
            mutex_counter: AtomicU32::new(0),
            initialized: AtomicBool::new(false),
            events: Trace::new(cache_dir),
            threads: Mutex::new(HashMap::new()),
            mutexes: Mutex::new(HashMap::new()),
        }
    }

    #[inline]
    pub fn initialize(&self) {
        if !self.initialized.load(Ordering::Relaxed) {
            let tid = self.tid_counter.fetch_add(1, Ordering::Relaxed);
            assert_eq!(
                tid, 0,
                "initialize should be called from the first thread, i.e., the main thread!"
            );

            let prev_tid = THREAD_STATE.with_borrow_mut(|thread_state| {
                if thread_state.ignore_memory_events {
                    log::warn!(
                        "Recording of memory access events was disabled at initialization time."
                    )
                }
                thread_state.id.replace(tid)
            });
            assert_eq!(
                prev_tid, None,
                "thread-local TID must not be set before initialization"
            );

            self.initialized.store(true, Ordering::Relaxed);

            log::info!("Successfully initialized Tracing.");
        } else {
            log::debug!(
                "Initialize called from a thread other than the first. Starting to ignore memory accesses ..."
            );
            self.thread_ignore_begin();
        }
    }

    #[inline]
    pub fn thread_ignore_begin(&self) {
        THREAD_STATE.with_borrow_mut(|thread_state| {
            if std::mem::replace(&mut thread_state.ignore_memory_events, true) {
                log::warn!("Memory access event ignore flag was set although already being set! Did you forgot to call thread_ignore_end?")
            } else {
                log::debug!("Starting to ignore memory access events for the current thread.")
            }
        });
    }

    #[inline]
    pub fn thread_ignore_end(&self) {
        THREAD_STATE.with_borrow_mut(|thread_state| {
            if !std::mem::replace(&mut thread_state.ignore_memory_events, false) {
                log::warn!("Memory access event ignore flag was unset although already being unset! Did you forgot to call thread_ignore_start?")
            } else {
                log::debug!("Finished to ignoring memory access events for the current thread.")
            }
        });
    }

    /// Append a new event to the execution trace.
    #[inline]
    fn add_event(&self, tid: u32, op: Op, loc: (u32, u32)) -> EventHandle {
        self.events.append_event(Event { t: tid, op, loc })
    }

    #[inline]
    pub fn memory_access_read(&self, addr: u32, width: u32, atomic: u32, loc: (u32, u32)) {
        THREAD_STATE.with_borrow(|thread_state| {
            if !thread_state.ignore_memory_events {
                if let Some(current_id) = thread_state.id {
                    self.add_event(current_id, Op::Read { addr, n: width, atomic: atomic != 0 }, loc);
                } else {
                    log::warn!(
                        "Local TID was not yet initialized. Ignoring memory read event (addr {addr:x}, width: {width}, loc ({}, {})) ...", 
                        loc.0, loc.1
                    )
                }
            } else {
                log::debug!("Ignoring memory read event (addr {addr:x}, width: {width}, loc ({}, {})) ...", loc.0, loc.1)
            }
        });
    }

    #[inline]
    pub fn memory_access_write(&self, addr: u32, width: u32, atomic: u32, loc: (u32, u32)) {
        THREAD_STATE.with_borrow(|thread_state| {
            if !thread_state.ignore_memory_events {
                if let Some(current_id) = thread_state.id {
                    self.add_event(current_id, Op::Write { addr, n: width, atomic: atomic != 0 }, loc);
                } else {
                    log::warn!(
                        "Local TID was not yet initialized. Ignoring memory write event (addr {addr:x}, width: {width}, loc ({}, {})) ...", 
                        loc.0, loc.1
                    )
                }
            } else {
                log::debug!("Ignoring memory write event (addr {addr:x}, width: {width}, loc ({}, {})) ...", loc.0, loc.1)
            }
        });
    }

    #[inline]
    pub fn thread_create(&self, userspace_child_id: u32, flags: u32, loc: (u32, u32)) -> Tid {
        THREAD_STATE.with_borrow(|thread_state| {
            if let Some(current_tid) = thread_state.id {
                let tid = self.tid_counter.fetch_add(1, Ordering::Relaxed);

                if flags & Self::THREAD_CREATE_DETACHED == 0 {
                    debug_assert_eq!(
                        flags,
                        Self::THREAD_CREATE_JOINABLE,
                        "The only supported thread_create flag is 0b1 => CREATE_DETACHED. 'flags' should be {} otherwise!",
                        Self::THREAD_CREATE_JOINABLE
                    );
                    let prev_mapping = self.threads
                        .lock()
                        .expect("Could not lock thread registry!")
                        .insert(userspace_child_id, ThreadRecord { id: tid });

                    if let Some(prev_mapping) = prev_mapping {
                        log::warn!(
                            "Created a new thread for (userspace) TID '{userspace_child_id}' while the registry still contained \
                            a mapping to the TID '{}'. Have you forgotten to join/detach this thread?",
                            prev_mapping.id
                        );
                    }
                }

                self.add_event(current_tid, Op::Fork { tid }, loc);

                tid
            } else {
                panic!("Local TID not yet initialized. Can not create a new child from an unregistered parent")
            }
        })
    }

    #[inline]
    pub fn thread_register(&self, tid: Tid) {
        let prev_tid = THREAD_STATE.with_borrow_mut(|thread_state| thread_state.id.replace(tid));
        assert_eq!(
            prev_tid, None,
            "Thread-local TID may only be initialized once per thread!"
        );
        log::debug!("Registered thread-local TID. Starting to record memory accesses ...");
        self.thread_ignore_end();
    }

    #[inline]
    pub fn thread_consume(&self, userspace_child_id: u32) -> Tid {
        self.threads
            .lock()
            .expect("Could not lock thread registry!")
            .remove(&userspace_child_id)
            .unwrap_or_else(|| panic!("Thread registry did not contain a mapping for given userspace tid '{userspace_child_id}'"))
            .id
    }

    #[inline]
    pub fn thread_join(&self, tid: Tid, loc: (u32, u32)) {
        THREAD_STATE.with_borrow(|thread_state| {
            if let Some(current_tid) = thread_state.id {
                self.add_event(current_tid, Op::Join { tid }, loc);
            } else {
                log::warn!("Local TID was not yet initialized. Ignoring thread join event ...")
            }
        });
    }

    #[inline]
    pub fn thread_detach(&self, tid: Tid) {
        THREAD_STATE.with_borrow(|thread_state| {
            if let Some(current_tid) = thread_state.id {
                // We enable this check only in debug builds to save us the locking overhead in release builds
                debug_assert!(
                    !self.threads
                        .lock()
                        .expect("Could not lock thread registry!")
                        .contains_key(&tid),
                    "Thread registry still contained TID '{tid}' when emitting a 'thread_detach' event from thread '{current_tid}'. \
                    Did you forget to call 'thread_consume' first?"
                );
            } else {
                log::warn!("Local TID was not yet initialized. Ignoring thread detach event ...");
            }
        });
    }

    #[inline]
    pub fn mutex_register(&self, userspace_mutex_id: u32, flags: u32) {
        THREAD_STATE.with_borrow(|thread_state| {
            if let Some(current_tid) = thread_state.id {
                let mutex_id = self.mutex_counter.fetch_add(1, Ordering::Relaxed);

                if flags & Self::MUTEX_INIT_RECURSIVE != 0 {
                    panic!("Recursive Mutexes are not yet supported!");
                }

                debug_assert_eq!(
                    flags,
                    Self::MUTEX_INIT_NORMAL,
                    "The only supported thread_create flag is 0b1 => INIT_RECURSIVE. 'flags' should be {} otherwise!",
                    Self::MUTEX_INIT_NORMAL
                );
                let prev_mapping = self.mutexes
                    .lock()
                    .expect("Could not lock mutex registry!")
                    .insert(userspace_mutex_id, MutexRecord { id: mutex_id, owner: current_tid, last_event: None });

                if prev_mapping.is_some() {
                    log::warn!(
                        "Registered (userspace) mutex '{userspace_mutex_id:x}' while the registry still contained \
                        an existing mapping for it. Have you forgotten to unregister this mutex first?"
                    );
                }
            } else {
                log::warn!("Local TID was not yet initialized. Skipping registration for mutex '{userspace_mutex_id:x}' ...");
            }
        });
    }

    #[inline]
    pub fn mutex_unregister(&self, userspace_mutex_id: u32) {
        self.mutexes
            .lock()
            .expect("Could not lock mutex registry!")
            .remove(&userspace_mutex_id)
            .unwrap_or_else(|| panic!("Thread registry did not contain a mapping for given userspace mutex '{userspace_mutex_id:x}'"));
    }

    #[inline]
    pub fn mutex_start_lock(&self, userspace_mutex_id: u32, loc: (u32, u32)) {
        THREAD_STATE.with_borrow(|thread_state| {
            if let Some(current_tid) = thread_state.id {
                self.mutexes
                    .lock()
                    .expect("Could not lock mutex registry!")
                    .entry(userspace_mutex_id)
                    .and_modify(|mutex_record| {
                        let event_record = self.add_event(
                            current_tid,
                            Op::Request {
                                lock: mutex_record.id,
                            },
                            loc,
                        );
                        mutex_record.last_event = Some(event_record);
                    })
                    .or_insert_with(|| {
                        let mutex_id = self.mutex_counter.fetch_add(1, Ordering::Relaxed);
                        let event_record =
                            self.add_event(current_tid, Op::Request { lock: mutex_id }, loc);
                        MutexRecord {
                            id: mutex_id,
                            owner: current_tid,
                            last_event: Some(event_record),
                        }
                    });
            } else {
                log::warn!(
                    "Local TID was not yet initialized. Ignoring mutex start lock event ..."
                );
            }
        });
    }

    #[inline]
    pub fn mutex_finish_lock(&self, userspace_mutex_id: u32, loc: (u32, u32)) {
        THREAD_STATE.with_borrow(|thread_state| {
            if let Some(current_tid) = thread_state.id {
                self.mutexes
                    .lock()
                    .expect("Could not lock mutex registry!")
                    .get_mut(&userspace_mutex_id)
                    .map(|mutex_record| {
                        let event_record = self.add_event(current_tid, Op::Aquire { lock: mutex_record.id }, loc);
                        mutex_record.last_event = Some(event_record);
                    })
                    .unwrap_or_else(|| panic!("Tried to register an aquire event for a mutex that could not be found in the mutex registry!"));
            } else {
                log::warn!("Local TID was not yet initialized. Ignoring mutex finish lock event ...");
            }
        });
    }

    #[inline]
    pub fn mutex_unlock(&self, userspace_mutex_id: u32, loc: (u32, u32)) {
        THREAD_STATE.with_borrow(|thread_state| {
            if let Some(current_tid) = thread_state.id {
                self.mutexes
                    .lock()
                    .expect("Could not lock mutex registry!")
                    .get_mut(&userspace_mutex_id)
                    .map(|mutex_record| {
                        let event_record = self.add_event(current_tid, Op::Release { lock: mutex_record.id }, loc);
                        mutex_record.last_event = Some(event_record);
                    })
                    .unwrap_or_else(|| panic!("Tried to register an unlock event for a mutex that could not be found in the mutex registry!"));
            } else {
                log::warn!("Local TID was not yet initialized. Ignoring mutex finish unlock event ...");
            }
        });
    }

    #[inline]
    pub fn mutex_repair(&self, userspace_mutex_id: u32) {
        THREAD_STATE.with_borrow(|thread_state| {
            if let Some(current_tid) = thread_state.id {
                self.mutexes
                    .lock()
                    .expect("Could not lock mutex registry!")
                    .get_mut(&userspace_mutex_id)
                    .map(|mutex_record| {
                        mutex_record.owner = current_tid;
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "Tried to repair a mutex that could not be found in the mutex registry!"
                        )
                    });
            } else {
                log::warn!("Local TID was not yet initialized. Ignoring mutex repair event ...");
            }
        });
    }

    #[inline]
    pub fn mutex_invalid_access(&self, userspace_mutex_id: u32) {
        let event_handle = self.mutexes
            .lock()
            .expect("Could not lock mutex registry!")
            .get_mut(&userspace_mutex_id)
            .map(|mutex_record| {
                mutex_record
                    .last_event
                    .take()
                    .unwrap_or_else(|| panic!("Invalid access has been issued before any event for mutex '{userspace_mutex_id:x}' has been recorded!"))
            })
            .unwrap_or_else(|| panic!("Tried to repair a mutex that could not be found in the mutex registry!"));

        self.events.invalidate(event_handle);
    }

    /// Emits the current state of the execution trace in RapidBin format.
    pub fn generate_binary_trace<P: AsRef<Path>>(
        self,
        outfile: P,
    ) -> Result<WasmgrindTraceMetadata, Error> {
        log::info!("Starting to generate binary trace ...");
        let mut converter = WasmgrindTraceConverter::new();

        let mut encoder = RapidBinEncoder::new();
        let outfile = BufWriter::new(File::create(outfile)?);

        encoder.encode(
            self.events
                .close()?
                .iter()?
                .map(|e| Ok(converter.convert_event(&e))),
            outfile,
        )?;

        Ok(converter.generate_metadata())
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::PathBuf};

    use anyhow::Error;
    use rand_xoshiro::{
        Xoshiro256PlusPlus,
        rand_core::{RngCore, SeedableRng},
    };
    use tempfile::tempdir;
    use trace_tools::{RapidBinParser, generic::Parser};

    use crate::tracing::{Op, metadata::WasmgrindTraceMetadata, trace::Trace};

    use super::Tracing;

    fn example_trace(trace_cache: PathBuf) -> Tracing {
        let tracing = Tracing::new(trace_cache);
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(42);
        const MAX_N_BYTES_ACCESSED: u32 = 8;

        for _ in 0..100 {
            let op = match rng.next_u32() % 7 {
                0 => Op::Aquire {
                    lock: rng.next_u32(),
                },
                1 => Op::Release {
                    lock: rng.next_u32(),
                },
                2 => Op::Request {
                    lock: rng.next_u32(),
                },
                3 => Op::Fork {
                    tid: rng.next_u32(),
                },
                4 => Op::Join {
                    tid: rng.next_u32(),
                },
                5 => Op::Read {
                    addr: rng.next_u32(),
                    n: rng.next_u32() % (MAX_N_BYTES_ACCESSED + 1),
                    atomic: rng.next_u32() % 2 != 0,
                },
                6 => Op::Write {
                    addr: rng.next_u32(),
                    n: rng.next_u32() % (MAX_N_BYTES_ACCESSED + 1),
                    atomic: rng.next_u32() % 2 != 0,
                },
                _ => unreachable!(),
            };

            tracing.add_event(rng.next_u32(), op, (rng.next_u32(), rng.next_u32()));
        }

        tracing
    }

    #[test]
    fn wasmgrind_trace_roundtrip() -> Result<(), Error> {
        let tmp = tempdir().expect("Could not create out dir for trace!");
        let tracing = example_trace(tmp.path().join("trace-cache"));

        let trace_file = tmp.path().join("trace.data");
        let trace_metadata = tracing.generate_binary_trace(&trace_file)?;
        let converter = trace_metadata.into_converter();

        let mut parser = RapidBinParser::new();
        let reader = BufReader::new(File::open(&trace_file)?);

        let trace = Trace::new(tmp.path().join("trace-cache"));
        for event in parser.parse(reader)? {
            trace.append_event(converter.convert_event(&event?)?);
        }

        let mut parser = RapidBinParser::new();
        let reader = BufReader::new(File::open(trace_file)?);
        let mut original_iter = parser.parse(reader)?;
        let cache = trace.close()?;
        let mut reconstructed_iter = cache.iter()?;

        loop {
            match (original_iter.next(), reconstructed_iter.next()) {
                (None, None) => break,
                (None, Some(_)) | (Some(_), None) => {
                    panic!("Original trace and reconstructed trace did not have equal length!")
                }
                (Some(orig_event), Some(recons_event)) => {
                    assert_eq!(
                        converter.convert_event(&orig_event?)?,
                        recons_event,
                        "Original trace and reconstructed trace were not equal!"
                    );
                }
            }
        }

        Ok(())
    }

    #[test]
    fn wasmgrind_metadata_roundtrip() -> Result<(), Error> {
        let tmp = tempdir().expect("Could not create out dir for trace!");
        let trace_metadata = example_trace(tmp.path().join("trace-cache"))
            .generate_binary_trace(tmp.path().join("trace.data"))?;
        let json_metadata = trace_metadata.to_json()?;
        assert_eq!(
            trace_metadata,
            WasmgrindTraceMetadata::from_json(json_metadata.as_bytes())?
        );

        Ok(())
    }
}
