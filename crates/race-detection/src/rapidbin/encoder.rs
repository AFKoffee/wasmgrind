use std::{
    collections::HashSet,
    io::{Seek, SeekFrom, Write},
};

use anyhow::Error;

use crate::{
    generic::{Encoder, Event, EventResult, Operation},
    rapidbin::{
        DECOR_BIT_OFFSET, DECOR_NUM_BITS, LOC_BIT_OFFSET, LOC_NUM_BITS, OP_BIT_OFFSET, OP_NUM_BITS,
        THREAD_BIT_OFFSET, THREAD_NUM_BITS,
    },
};

/// An encoder to emit execution traces in _RapidBin_ format
pub struct RapidBinEncoder {
    threads: HashSet<i64>,
    locks: HashSet<i64>,
    variables: HashSet<i64>,
}

impl RapidBinEncoder {
    const HEADER_LEN: usize =
        std::mem::size_of::<i16>() + 2 * std::mem::size_of::<i32>() + std::mem::size_of::<i64>();

    pub fn new() -> Self {
        Self {
            threads: HashSet::new(),
            locks: HashSet::new(),
            variables: HashSet::new(),
        }
    }

    fn get_n_threads(&self) -> Result<i16, Error> {
        let n_threads = i16::try_from(self.threads.len())?;

        Ok(n_threads)
    }

    fn get_n_locks(&self) -> Result<i32, Error> {
        let n_locks = i32::try_from(self.locks.len())?;

        Ok(n_locks)
    }

    fn get_n_variables(&self) -> Result<i32, Error> {
        let n_variables = i32::try_from(self.variables.len())?;

        Ok(n_variables)
    }

    fn encode_event(&mut self, event: Event) -> Result<i64, Error> {
        let (thread_id, operation, location) = event.into_fields();

        let tid = i64::from(i16::try_from(thread_id)?) & ((1 << THREAD_NUM_BITS) - 1);
        let oid = i64::from(operation.id()) & ((1 << OP_NUM_BITS) - 1);
        let lid = i64::from(i16::try_from(location)?) & ((1 << LOC_NUM_BITS) - 1);

        let decor = match operation {
            Operation::Aquire { lock: decor }
            | Operation::Request { lock: decor }
            | Operation::Release { lock: decor } => {
                let decor = i64::try_from(decor)?;
                self.locks.insert(decor);
                decor
            }
            Operation::Read { memory: decor } | Operation::Write { memory: decor } => {
                let decor = i64::try_from(decor)?;
                self.variables.insert(decor);
                decor
            }
            Operation::Fork { tid: decor } | Operation::Join { tid: decor } => {
                let decor = i64::try_from(decor)?;
                self.threads.insert(decor);
                decor
            }
        } & ((1 << DECOR_NUM_BITS) - 1);

        self.threads.insert(tid);

        Ok((tid << THREAD_BIT_OFFSET)
            | (oid << OP_BIT_OFFSET)
            | (decor << DECOR_BIT_OFFSET)
            | (lid << LOC_BIT_OFFSET))
    }
}

impl Default for RapidBinEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Encoder for RapidBinEncoder {
    const EVENT_SIZE_HINT: usize = 8;

    fn encode<W: Write + Seek, I: IntoIterator<Item = EventResult>>(
        &mut self,
        input: I,
        mut output: W,
    ) -> Result<(), Error> {
        // Reserve empty space for the header information
        output.write_all(&[0u8; Self::HEADER_LEN])?;

        // Write the events of the trace
        let mut n_events = 0_i64;
        for event in input {
            output.write_all(&self.encode_event(event?)?.to_be_bytes())?;
            n_events += 1;
        }

        // Now we can write the header information
        output.seek(SeekFrom::Start(0))?;
        output.write_all(&self.get_n_threads()?.to_be_bytes())?;
        output.write_all(&self.get_n_locks()?.to_be_bytes())?;
        output.write_all(&self.get_n_variables()?.to_be_bytes())?;
        output.write_all(&n_events.to_be_bytes())?;

        Ok(())
    }

    fn format(&self) -> &'static str {
        "RapidBin"
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use anyhow::Error;
    use rand_xoshiro::{
        Xoshiro256PlusPlus,
        rand_core::{RngCore, SeedableRng},
    };

    use crate::generic::{Encoder, Event, EventResult, Operation};

    use super::RapidBinEncoder;

    struct ExampleTraceBuilder {
        invalid_tid: bool,
        invalid_loc: bool,
        invalid_decor: bool,
    }

    impl ExampleTraceBuilder {
        fn new() -> Self {
            Self {
                invalid_tid: false,
                invalid_loc: false,
                invalid_decor: false,
            }
        }

        fn invalid_tid(mut self, invalid: bool) -> Self {
            self.invalid_tid = invalid;
            self
        }

        fn invalid_loc(mut self, invalid: bool) -> Self {
            self.invalid_loc = invalid;
            self
        }

        fn invalid_decor(mut self, invalid: bool) -> Self {
            self.invalid_decor = invalid;
            self
        }

        fn build(self) -> Vec<Event> {
            let mut tracing = Vec::new();
            let mut rng = Xoshiro256PlusPlus::seed_from_u64(42);
            let i16_max = u32::try_from(i16::MAX).unwrap();
            let i64_max = u64::try_from(i64::MAX).unwrap();
            let valid_ops = [0, 1, 2, 3, 4, 5, 8];

            for i in 0..100 {
                let tid = if self.invalid_tid && i % 54 == 0 {
                    i16_max + 5234
                } else {
                    let next = rng.next_u32() % (i16_max + 1);
                    assert!(next <= i16_max);
                    next
                };

                let loc = if self.invalid_loc && i % 63 == 0 {
                    i16_max + 2362
                } else {
                    let next = rng.next_u32() % (i16_max + 1);
                    assert!(next <= i16_max);
                    next
                };

                let op_id = valid_ops[(rng.next_u32() % 7) as usize];
                let decor = if self.invalid_decor && i % 91 == 0 {
                    i64_max + 9081
                } else {
                    let next = rng.next_u64() % (i64_max + 1);
                    assert!(next <= i64_max);
                    next
                };

                let event = Event::new(
                    u64::from(tid),
                    Operation::try_from_id(op_id, decor).unwrap(),
                    u64::from(loc),
                );
                tracing.push(event);
            }

            tracing
        }
    }

    #[test]
    fn encode_valid_trace() -> Result<(), Error> {
        let generic_trace: Vec<EventResult> = [
            Event::new(0, Operation::Fork { tid: 1 }, 42),
            Event::new(0, Operation::Fork { tid: 2 }, 42),
            Event::new(2, Operation::Fork { tid: 3 }, 123),
            Event::new(0, Operation::Request { lock: 0 }, 362),
            Event::new(0, Operation::Aquire { lock: 0 }, 362),
            Event::new(0, Operation::Read { memory: 200 }, 436),
            Event::new(0, Operation::Write { memory: 200 }, 923),
            Event::new(0, Operation::Release { lock: 0 }, 362),
            Event::new(0, Operation::Join { tid: 1 }, 7382),
        ]
        .into_iter()
        .map(Ok)
        .collect();

        let mut buffer = Cursor::new(Vec::new());
        let mut encoder = RapidBinEncoder::new();
        encoder.encode(generic_trace, &mut buffer)?;

        let encoded_trace = buffer.into_inner();
        let mut binary_trace = Vec::new();
        binary_trace.extend(4_i16.to_be_bytes());
        binary_trace.extend(1_i32.to_be_bytes());
        binary_trace.extend(1_i32.to_be_bytes());
        binary_trace.extend(9_i64.to_be_bytes());

        #[allow(clippy::unusual_byte_groupings)]
        binary_trace.extend(
            [
                0b0_000000000101010_0000000000000000000000000000000001_0100_0000000000__i64
                    .to_be_bytes(),
                0b0_000000000101010_0000000000000000000000000000000010_0100_0000000000__i64
                    .to_be_bytes(),
                0b0_000000001111011_0000000000000000000000000000000011_0100_0000000010__i64
                    .to_be_bytes(),
                0b0_000000101101010_0000000000000000000000000000000000_1000_0000000000__i64
                    .to_be_bytes(),
                0b0_000000101101010_0000000000000000000000000000000000_0000_0000000000__i64
                    .to_be_bytes(),
                0b0_000000110110100_0000000000000000000000000011001000_0010_0000000000__i64
                    .to_be_bytes(),
                0b0_000001110011011_0000000000000000000000000011001000_0011_0000000000__i64
                    .to_be_bytes(),
                0b0_000000101101010_0000000000000000000000000000000000_0001_0000000000__i64
                    .to_be_bytes(),
                0b0_001110011010110_0000000000000000000000000000000001_0101_0000000000__i64
                    .to_be_bytes(),
            ]
            .concat(),
        );

        assert_eq!(binary_trace, encoded_trace);

        Ok(())
    }

    #[test]
    fn fail_on_invalid_trace() {
        let mut encoder = RapidBinEncoder::new();
        let mut buffer = Cursor::new(Vec::new());

        let trace: Vec<EventResult> = ExampleTraceBuilder::new()
            .invalid_decor(true)
            .build()
            .into_iter()
            .map(Ok)
            .collect();
        encoder.encode(trace, &mut buffer).unwrap_err();

        let trace: Vec<EventResult> = ExampleTraceBuilder::new()
            .invalid_loc(true)
            .build()
            .into_iter()
            .map(Ok)
            .collect();
        encoder.encode(trace, &mut buffer).unwrap_err();

        let trace: Vec<EventResult> = ExampleTraceBuilder::new()
            .invalid_tid(true)
            .build()
            .into_iter()
            .map(Ok)
            .collect();
        encoder.encode(trace, &mut buffer).unwrap_err();

        let trace: Vec<EventResult> = ExampleTraceBuilder::new()
            .invalid_decor(true)
            .invalid_tid(true)
            .invalid_loc(true)
            .build()
            .into_iter()
            .map(Ok)
            .collect();
        encoder.encode(trace, &mut buffer).unwrap_err();
    }

    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn encode_valid_event() -> Result<(), Error> {
        let mut encoder = RapidBinEncoder::new();
        let event = Event::new(0, Operation::Write { memory: 200 }, 912);

        let binary_event = encoder.encode_event(event)?;

        assert_eq!(
            binary_event,
            0b0_000001110010000_0000000000000000000000000011001000_0011_0000000000
        );

        Ok(())
    }

    #[test]
    fn fail_on_invalid_event() {
        let mut encoder = RapidBinEncoder::new();

        let invalid_thread = Event::new(u64::MAX - 100, Operation::Read { memory: 200 }, 912);
        encoder.encode_event(invalid_thread).unwrap_err();

        let invalid_decor = Event::new(2, Operation::Join { tid: 4 }, u64::MAX - 235);
        encoder.encode_event(invalid_decor).unwrap_err();

        let invalid_location = Event::new(
            124,
            Operation::Aquire {
                lock: u64::MAX - 100,
            },
            42,
        );
        encoder.encode_event(invalid_location).unwrap_err();
    }
}
