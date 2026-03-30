use std::{
    cell::RefCell,
    collections::HashSet,
    path::Path,
    sync::{
        Mutex,
        atomic::{self, AtomicU64},
    },
};

use anyhow::Error;
use bitcode::{Decode, Encode};

use crate::tracing::{
    representation::Event,
    trace::{
        registry::{Registry, RegistryIter, TraceRegistry},
        tls::TlsTrace,
    },
};

mod cursor;
mod registry;
mod tls;

thread_local! {
    static EVENT_BUFFER: RefCell<Option<TlsTrace>> = const { RefCell::new(None) };
}

#[derive(Encode, Decode, Debug)]
pub struct EventRecord {
    id: u64,
    event: Event,
}

#[repr(transparent)]
pub struct EventHandle {
    id: u64,
}

pub struct Trace {
    next_event_id: AtomicU64,
    registry: TraceRegistry,
    invalid: Mutex<HashSet<u64>>,
}

impl Trace {
    pub fn new<P: AsRef<Path>>(cache_dir: P) -> Self {
        Self {
            next_event_id: AtomicU64::new(0),
            registry: TraceRegistry::new(cache_dir).expect("Could not create trace registry"),
            invalid: Mutex::new(HashSet::new()),
        }
    }

    pub fn append_event(&self, event: Event) -> EventHandle {
        let event_id = 0; // self.next_event_id.fetch_add(1, atomic::Ordering::Relaxed);
        let record = EventRecord {
            id: event_id,
            event,
        };

        EVENT_BUFFER.with_borrow_mut(|tls| {
            if let Some(tls_trace) = tls {
                tls_trace
                    .append(record, &self.registry)
                    .expect("Failed to append event to TLS trace");
            } else {
                let mut tls_trace = TlsTrace::new(record.event.t, &self.registry)
                    .expect("Could not initialize TLS trace");
                tls_trace
                    .append(record, &self.registry)
                    .expect("Failed to append event to TLS trace");
                *tls = Some(tls_trace);
            };
        });

        EventHandle { id: event_id }
    }

    /// Invalidate an event with the given global ID
    ///
    /// # Panics
    /// If the id was already invalidated before
    pub fn invalidate(&self, event_handle: EventHandle) {
        let was_invalid = self
            .invalid
            .lock()
            .expect("Invalidation mutex was poisoned!")
            .insert(event_handle.id);

        assert!(!was_invalid, "Event was already invalidated once!");
    }

    pub fn close(self) -> Result<CachedTrace, Error> {
        EVENT_BUFFER.with_borrow_mut(|tls| {
            if let Some(mut tls_trace) = tls.take() {
                tls_trace.flush().and_then(|_| tls_trace.seal())?;
            }

            Ok::<(), Error>(())
        })?;

        Ok(CachedTrace {
            n_events: self.next_event_id.into_inner(),
            cache: self.registry.close(),
            invalid: self
                .invalid
                .into_inner()
                .expect("Invalid event set mutex was poisoned"),
        })
    }
}

pub struct CachedTrace {
    n_events: u64,
    cache: Registry,
    invalid: HashSet<u64>,
}

impl CachedTrace {
    pub fn iter(&self) -> Result<TraceIter<'_>, Error> {
        Ok(TraceIter {
            n_events: self.n_events,
            expected_event_id: 0,
            registry_iter: self.cache.iter()?,
            invalid: &self.invalid,
        })
    }
}

pub struct TraceIter<'a> {
    n_events: u64,
    expected_event_id: u64,
    registry_iter: RegistryIter<'a>,
    invalid: &'a HashSet<u64>,
}

impl<'a> Iterator for TraceIter<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Self::Item> {
        let record = self
            .registry_iter
            .next()?
            .expect("Failed to load event from cache!");

        debug_assert_eq!(
            self.expected_event_id,
            record.id,
            "Trace did not contain {}-th event of {}",
            self.expected_event_id + 1,
            self.n_events
        );

        self.expected_event_id += 1;

        // Skip the event if it has been invalidated
        if self.invalid.contains(&record.id) {
            self.next()
        } else {
            Some(record.event)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc, vec::IntoIter};

    use anyhow::Error;
    use rand_xoshiro::{
        Xoshiro256PlusPlus,
        rand_core::{RngCore, SeedableRng},
    };
    use tempfile::tempdir;

    use crate::tracing::{Op, representation::Event, trace::Trace};

    fn generate_op(rng: &mut Xoshiro256PlusPlus) -> Op {
        const MAX_N_BYTES_ACCESSED: u32 = 8;
        match rng.next_u32() % 7 {
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
                atomic: rng.next_u32().is_multiple_of(2),
            },
            6 => Op::Write {
                addr: rng.next_u32(),
                n: rng.next_u32() % (MAX_N_BYTES_ACCESSED + 1),
                atomic: rng.next_u32().is_multiple_of(2),
            },
            _ => unreachable!(),
        }
    }

    #[test]
    fn trace_insert_one() -> Result<(), Error> {
        let tmp = tempdir().expect("Could not create cache dir for trace!");
        let trace = Trace::new(tmp.path().join("trace-cache"));
        let event = Event {
            t: 0,
            op: Op::Read {
                addr: 12312,
                n: 4,
                atomic: false,
            },
            loc: (5, 42),
        };
        trace.append_event(event.clone());

        let cache = trace.close()?;
        let mut iter = cache.iter()?;
        assert_eq!(
            iter.next(),
            Some(event),
            "Head and tail pointers were not equal after one insert!"
        );

        Ok(())
    }

    #[test]
    fn trace_insert_many() -> Result<(), Error> {
        const N_INSERT: usize = 1_000_000;

        let tmp = tempdir().expect("Could not create cache dir for trace!");
        let trace = Trace::new(tmp.path().join("trace-cache"));
        let mut trace_cmp = Vec::with_capacity(N_INSERT);

        let mut rng = Xoshiro256PlusPlus::seed_from_u64(42);

        for _ in 0..N_INSERT {
            let event = Event {
                t: rng.next_u32(),
                op: generate_op(&mut rng),
                loc: (rng.next_u32(), rng.next_u32()),
            };
            trace.append_event(event.clone());
            trace_cmp.push(event);
        }

        let mut len_equal = false;
        for (idx, event) in trace.close()?.iter()?.enumerate() {
            let reference = &trace_cmp[idx];
            assert_eq!(
                &event, reference,
                "Iterator yielded event that was not equal to reference event!"
            );

            if idx == trace_cmp.len() - 1 {
                len_equal = true;
            }
        }

        assert!(
            len_equal,
            "Iterator yielded less events than the reference trace contained!"
        );

        Ok(())
    }

    #[test]
    fn trace_insert_mt() -> Result<(), Error> {
        const N_INSERT_PER_THREAD: usize = 10_000_000;
        const N_THREADS: u64 = 10;

        let tmp = tempdir().expect("Could not create cache dir for trace!");
        let trace = Arc::new(Trace::new(tmp.path().join("trace-cache")));
        let mut threads = Vec::new();

        for i in 0..N_THREADS {
            let trace = trace.clone();
            let handle = std::thread::spawn(move || {
                let mut rng = Xoshiro256PlusPlus::seed_from_u64(i);
                let mut thread_events = Vec::new();

                let tid = rng.next_u32();
                for _ in 0..N_INSERT_PER_THREAD {
                    let event = Event {
                        t: tid,
                        op: generate_op(&mut rng),
                        loc: (rng.next_u32(), rng.next_u32()),
                    };
                    trace.append_event(event.clone());
                    thread_events.push(event);
                }

                thread_events
            });

            threads.push(handle);
        }

        let mut nexts: HashMap<Event, IntoIter<Event>> = HashMap::new();
        for (idx, thread) in threads.into_iter().enumerate() {
            let mut iter = thread
                .join()
                .unwrap_or_else(|_| panic!("Thread {idx} panicked!"))
                .into_iter();
            if let Some(next) = iter.next() {
                nexts.insert(next, iter);
            }
        }

        for event in Arc::into_inner(trace).unwrap().close()?.iter()? {
            if let Some(mut iter) = nexts.remove(&event) {
                if let Some(next) = iter.next() {
                    nexts.insert(next, iter);
                }
            } else {
                panic!("Could not find event {event:?} in any of the threads traces.")
            }
        }

        if !nexts.is_empty() {
            panic!(
                "Global trace did not contain all events of the threads. Remaining events: {nexts:?}"
            );
        }

        Ok(())
    }
}
