use std::{io::Cursor, sync::Mutex};

use anyhow::{Error, bail};
use representation::Event;

use crate::{
    generic::{Encoder, Operation, Parser},
    rapidbin::encoder::RapidBinEncoder,
    tracing::{converter::WasmgrindTraceConverter, metadata::WasmgrindTraceMetadata}, RapidBinParser,
};

mod converter;

/// Utilities to manage metadata of Wasmgrind execution traces.
pub mod metadata;
mod representation;

pub use representation::Op;

/// A mutex-protected execution trace.
///
/// Internally, this is a [`Vec`] of events wrapped by a [`Mutex`]
/// that has to be locked each time any thread wants to read from or
/// write to the trace.
pub struct Tracing {
    events: Mutex<Vec<Event>>,
}

impl Tracing {
    /// Creates an empty execution trace.
    pub const fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    /// Append a new event to the execution trace.
    ///
    /// `tid` is the id of the executing thread, `loc` is the current location
    /// inside the WebAssembly binary and `op` is the recorded operation [`Op`]
    /// that has been executed by the thread identified by `tid`.
    ///
    /// # Errors
    ///
    /// This function may fail if the [`Mutex`] guarding the execution trace
    /// was poisoned.
    #[inline]
    pub fn add_event(&self, tid: u32, op: Op, loc: (u32, u32)) -> Result<(), Error> {
        match self.events.lock() {
            Ok(mut events_guard) => {
                events_guard.push(Event { t: tid, op, loc });
                Ok(())
            }
            Err(_) => bail!("Trace Lock Poisoned: Could not insert new event!"),
        }
    }

    /// Emits the current state of the execution trace in binary format.
    ///
    /// This method will lock the internal execution trace
    /// iterates over all collected events and creates a binary trace in
    /// [RapidBin](https://afkoffee.github.io/wasmgrind/developers_guide/race_detection/rapid_bin.html)
    /// format.
    ///
    /// # Errors
    ///
    /// This function may fail if the [`Mutex`] guarding the execution trace
    /// was poisoned.
    pub fn generate_binary_trace(&self) -> Result<BinaryTraceOutput, Error> {
        let mut converter = WasmgrindTraceConverter::new();

        let binary_trace = match self.events.lock() {
            Ok(events_guard) => {
                let mut encoder = RapidBinEncoder::new();
                let mut output = Cursor::new(Vec::with_capacity(
                    events_guard.len() * RapidBinEncoder::EVENT_SIZE_HINT,
                ));

                encoder.encode(
                    events_guard.iter().map(|e| Ok(converter.convert_event(e))),
                    &mut output,
                )?;

                output.into_inner()
            }
            Err(_) => bail!("Trace Lock Poisoned: Could not generate binary trace!"),
        };

        Ok(BinaryTraceOutput {
            trace: binary_trace,
            metadata: converter.generate_metadata(),
        })
    }
}

impl Default for Tracing {
    fn default() -> Self {
        Self::new()
    }
}

/// A collection of overlapping memory accesses with regard to a single execution trace.
/// 
/// Currently, this struct can only be created using the [`BinaryTraceOutput::find_overlaps`]
/// function. Therefore, the data contained in this struct specifically relates to the instance
/// of [`BinaryTraceOutput`] it was created from.
pub struct Overlaps<'a> {
    overlaps: Vec<metadata::Overlap<'a>>,
    n_memory_events: usize,
    n_overlap_events: usize,
}

impl <'a> Overlaps<'a> {
    /// Returns a reference to a list of all pairwise overlaps of distinct memory accesses.
    /// 
    /// The overlaps in this list are selected by the following criteria:
    /// - The memory accesses have to share at least one byte of targeted memory
    /// - The memory accesses need to occur amongst different threads throughout the
    ///   execution trace
    /// 
    /// Any memory access targeting the same address with the same number of accessed
    /// bytes is only counted **once**.
    pub fn get_overlaps(&self) -> &Vec<metadata::Overlap<'a>> {
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

/// An execution trace in RapidBin format including its metadata.
pub struct BinaryTraceOutput {
    /// The binary execution trace
    pub trace: Vec<u8>,

    /// The trace metadata
    pub metadata: WasmgrindTraceMetadata,
}

impl BinaryTraceOutput {
    /// Determines all pairwise overlaps of distinct memory accesses in this execution trace.
    /// 
    /// This function will collect all pairwise overlaps of distinct memory accesses,
    /// the total number of memory-access events in this trace as well as the proportion
    /// of these memory-access events that contain overlapping memory accesses. The information
    /// can then be queried via the returned [`Overlaps`] instance.
    /// 
    /// Refer to [`Overlaps::get_overlaps`] for details on how pairwise overlaps are determined.
    pub fn find_overlaps(&self) -> Result<Overlaps, Error> {
        let overlaps = self.metadata.find_overlaps();
        
        let mut parser = RapidBinParser::new();
        let mut n_memory_events = 0;
        let mut n_overlap_events = 0;
        for event in parser.parse(&self.trace[..])? {
            let (_, op, _) = event?.into_fields();
            match op {
                Operation::Read { memory } |
                Operation::Write { memory } => {
                    n_memory_events += 1;
                    if overlaps.iter().any(|overlap| overlap.contains(memory)) {
                        n_overlap_events += 1;
                    }
                },
                _ => continue
            }
        }

        Ok(Overlaps { overlaps, n_memory_events, n_overlap_events })
    }
}

impl TryFrom<BinaryTraceOutput> for Tracing {
    type Error = Error;

    fn try_from(value: BinaryTraceOutput) -> Result<Self, Self::Error> {
        let converter = value.metadata.into_converter();
        let mut trace = Vec::new();

        let mut parser = RapidBinParser::new();
        for event in parser.parse(&value.trace[..])? {
            trace.push(converter.convert_event(&event?)?);
        }

        Ok(Tracing { events: Mutex::new(trace) })
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Error;
    use rand_xoshiro::{
        Xoshiro256PlusPlus,
        rand_core::{RngCore, SeedableRng},
    };

    use crate::{
        RapidBinParser,
        generic::{Event, Parser},
        tracing::{Op, metadata::WasmgrindTraceMetadata},
    };

    use super::Tracing;

    fn example_trace() -> Tracing {
        let tracing = Tracing::new();
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
                },
                6 => Op::Write {
                    addr: rng.next_u32(),
                    n: rng.next_u32() % (MAX_N_BYTES_ACCESSED + 1),
                },
                _ => unreachable!(),
            };
            tracing
                .add_event(rng.next_u32(), op, (rng.next_u32(), rng.next_u32()))
                .unwrap();
        }

        tracing
    }

    #[test]
    fn wasmgrind_trace_roundtrip() -> Result<(), Error> {
        let tracing = example_trace();
        let output = tracing.generate_binary_trace()?;
        let (trace, metadata) = (output.trace, output.metadata);

        let mut parser = RapidBinParser::new();
        let maybe_trace: Result<Vec<Event>, Error> = parser.parse(trace.as_slice())?.collect();
        let converter = metadata.into_converter();

        let mut trace = Vec::new();
        for event in maybe_trace? {
            trace.push(converter.convert_event(&event)?);
        }

        assert_eq!(*tracing.events.lock().unwrap(), trace);

        Ok(())
    }

    #[test]
    fn wasmgrind_metadata_roundtrip() -> Result<(), Error> {
        let trace_metadata = example_trace().generate_binary_trace()?.metadata;
        let json_metadata = trace_metadata.to_json()?;
        assert_eq!(
            trace_metadata,
            WasmgrindTraceMetadata::from_json(json_metadata.as_bytes())?
        );

        Ok(())
    }
}
