use std::{collections::HashSet, io::Read};

use anyhow::{Error, bail, ensure};

use crate::generic::{Event, EventResult, Operation, Parser};

use super::{
    DECOR_BIT_OFFSET, DECOR_MASK, LOC_BIT_OFFSET, LOC_MASK, NUMBER_OF_EVENTS_MASK,
    NUMBER_OF_LOCKS_MASK, NUMBER_OF_TRHEADS_MASK, NUMBER_OF_VARS_MASK, OP_BIT_OFFSET, OP_MASK,
    THREAD_BIT_OFFSET, THREAD_MASK,
};

/// A parser for execution traces in _RapidBin_ format.
pub struct RapidBinParser;

impl RapidBinParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for RapidBinParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser for RapidBinParser {
    type Iter<R: Read> = RapidBinIterator<R>;

    fn parse<R: Read>(&mut self, mut input: R) -> Result<Self::Iter<R>, Error> {
        // Parse header info
        let mut n_threads = [0; 2];
        input.read_exact(&mut n_threads)?;
        let n_threads = NUMBER_OF_TRHEADS_MASK & i16::from_be_bytes(n_threads);

        let mut n_locks = [0; 4];
        input.read_exact(&mut n_locks)?;
        let n_locks = NUMBER_OF_LOCKS_MASK & i32::from_be_bytes(n_locks);

        let mut n_vars = [0; 4];
        input.read_exact(&mut n_vars)?;
        let n_vars = NUMBER_OF_VARS_MASK & i32::from_be_bytes(n_vars);

        let mut n_events = [0; 8];
        input.read_exact(&mut n_events)?;
        let n_events = NUMBER_OF_EVENTS_MASK & i64::from_be_bytes(n_events);

        Ok(RapidBinIterator::new(
            input, n_threads, n_locks, n_vars, n_events,
        ))
    }

    fn format(&self) -> &'static str {
        "RapidBin"
    }
}

pub struct RapidBinIterator<R: Read> {
    input: R,
    n_threads: i16,
    n_locks: i32,
    n_variables: i32,
    n_events: i64,
    buffer: [u8; 8],
    event_counter: i64,
    threads: HashSet<u64>,
    locks: HashSet<u64>,
    variables: HashSet<u64>,
}

impl<R: Read> RapidBinIterator<R> {
    fn new(input: R, n_threads: i16, n_locks: i32, n_variables: i32, n_events: i64) -> Self {
        Self {
            input,
            n_threads,
            n_locks,
            n_variables,
            n_events,
            buffer: [0; 8],
            event_counter: 0,
            threads: HashSet::new(),
            locks: HashSet::new(),
            variables: HashSet::new(),
        }
    }

    fn inner_next(&mut self) -> Result<Option<Event>, Error> {
        if let Err(e) = self.input.read_exact(&mut self.buffer) {
            match e.kind() {
                std::io::ErrorKind::UnexpectedEof => {
                    if self.event_counter == self.n_events
                        && u64::try_from(self.threads.len())? == u64::try_from(self.n_threads)?
                        && u64::try_from(self.locks.len())? == u64::try_from(self.n_locks)?
                        && u64::try_from(self.variables.len())? == u64::try_from(self.n_variables)?
                    {
                        return Ok(None);
                    } else {
                        bail!(e)
                    }
                }
                _ => bail!(e),
            }
        }

        let event_integer = i64::from_be_bytes(self.buffer);
        let t = u64::try_from((event_integer & THREAD_MASK) >> THREAD_BIT_OFFSET)?;
        let op = (event_integer & OP_MASK) >> OP_BIT_OFFSET;
        let decor = u64::try_from((event_integer & DECOR_MASK) >> DECOR_BIT_OFFSET)?;
        let operation = Operation::try_from_id(op, decor)?;
        let loc = u64::try_from((event_integer & LOC_MASK) >> LOC_BIT_OFFSET)?;

        self.threads.insert(t);
        match operation {
            Operation::Aquire { lock: decor }
            | Operation::Request { lock: decor }
            | Operation::Release { lock: decor } => {
                self.locks.insert(decor);
            }
            Operation::Read { memory: decor } | Operation::Write { memory: decor } => {
                self.variables.insert(decor);
            }
            Operation::Fork { tid: decor } | Operation::Join { tid: decor } => {
                self.threads.insert(decor);
            }
        }

        let event = Event::new(t, operation, loc);

        self.event_counter += 1;

        ensure!(
            u64::try_from(self.threads.len())? <= u64::try_from(self.n_threads)?,
            "Found more threads than specified!"
        );
        ensure!(
            u64::try_from(self.locks.len())? <= u64::try_from(self.n_locks)?,
            "Found more locks than specified!"
        );
        ensure!(
            u64::try_from(self.variables.len())? <= u64::try_from(self.n_variables)?,
            "Found more variables than specified!"
        );
        ensure!(
            self.event_counter <= self.n_events,
            "Found more events than specified!"
        );

        Ok(Some(event))
    }
}

impl<R: Read> Iterator for RapidBinIterator<R> {
    type Item = EventResult;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner_next().transpose()
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Error;

    use super::{RapidBinIterator, RapidBinParser};
    use crate::generic::{Event, Operation, Parser};

    #[test]
    fn parse_valid_trace() -> Result<(), Error> {
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

        let mut parser = RapidBinParser::new();
        let parsed_trace: Result<Vec<Event>, Error> =
            parser.parse(binary_trace.as_slice())?.collect();
        let generic_trace: Vec<Event> = vec![
            Event::new(0, Operation::Fork { tid: 1 }, 42),
            Event::new(0, Operation::Fork { tid: 2 }, 42),
            Event::new(2, Operation::Fork { tid: 3 }, 123),
            Event::new(0, Operation::Request { lock: 0 }, 362),
            Event::new(0, Operation::Aquire { lock: 0 }, 362),
            Event::new(0, Operation::Read { memory: 200 }, 436),
            Event::new(0, Operation::Write { memory: 200 }, 923),
            Event::new(0, Operation::Release { lock: 0 }, 362),
            Event::new(0, Operation::Join { tid: 1 }, 7382),
        ];

        assert_eq!(generic_trace, parsed_trace?);

        Ok(())
    }

    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn parse_valid_event() -> Result<(), Error> {
        let input = 0b0_000000000101010_0000000000000000000000000000000001_0100_0000000000__i64
            .to_be_bytes();
        let mut iter = RapidBinIterator::new(&input[..], 2, 0, 0, 1);

        assert_eq!(
            iter.next().unwrap().unwrap(),
            Event::new(0, Operation::Fork { tid: 1 }, 42)
        );
        assert!(iter.next().is_none());

        Ok(())
    }

    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn fail_on_too_few_events() -> Result<(), Error> {
        let input = 0b0_000000000101010_0000000000000000000000000000000001_0100_0000000000__i64
            .to_be_bytes();
        let mut iter = RapidBinIterator::new(&input[..], 2, 0, 0, 2);
        assert_eq!(
            iter.next().unwrap().unwrap(),
            Event::new(0, Operation::Fork { tid: 1 }, 42)
        );
        iter.next().unwrap().unwrap_err();

        Ok(())
    }

    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn fail_on_too_many_events() -> Result<(), Error> {
        let input = 0b0_000000000101010_0000000000000000000000000000000001_0100_0000000000__i64
            .to_be_bytes();
        let mut iter = RapidBinIterator::new(&input[..], 2, 0, 0, 0);
        iter.next().unwrap().unwrap_err();

        Ok(())
    }

    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn fail_on_too_few_threads() -> Result<(), Error> {
        let input = 0b0_000000110110100_0000000000000000000000000011001000_0010_0000000000__i64
            .to_be_bytes();
        let mut iter = RapidBinIterator::new(&input[..], 2, 0, 1, 1);
        assert_eq!(
            iter.next().unwrap().unwrap(),
            Event::new(0, Operation::Read { memory: 200 }, 436)
        );
        iter.next().unwrap().unwrap_err();

        Ok(())
    }

    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn fail_on_too_many_threads() -> Result<(), Error> {
        let input = 0b0_000000110110100_0000000000000000000000000011001000_0010_0000000000__i64
            .to_be_bytes();
        let mut iter = RapidBinIterator::new(&input[..], 0, 0, 1, 1);
        iter.next().unwrap().unwrap_err();

        Ok(())
    }

    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn fail_on_too_few_locks() -> Result<(), Error> {
        let input = 0b0_000000101101010_0000000000000000000000000000000000_0000_0000000000__i64
            .to_be_bytes();
        let mut iter = RapidBinIterator::new(&input[..], 2, 2, 0, 1);
        assert_eq!(
            iter.next().unwrap().unwrap(),
            Event::new(0, Operation::Aquire { lock: 0 }, 362)
        );
        iter.next().unwrap().unwrap_err();

        Ok(())
    }

    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn fail_on_too_many_locks() -> Result<(), Error> {
        let input = 0b0_000000101101010_0000000000000000000000000000000000_0000_0000000000__i64
            .to_be_bytes();
        let mut iter = RapidBinIterator::new(&input[..], 0, 0, 0, 1);
        iter.next().unwrap().unwrap_err();

        Ok(())
    }

    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn fail_on_too_few_variables() -> Result<(), Error> {
        let input = 0b0_000001110011011_0000000000000000000000000011001000_0011_0000000000__i64
            .to_be_bytes();
        let mut iter = RapidBinIterator::new(&input[..], 1, 0, 2, 1);
        assert_eq!(
            iter.next().unwrap().unwrap(),
            Event::new(0, Operation::Write { memory: 200 }, 923)
        );
        iter.next().unwrap().unwrap_err();

        Ok(())
    }

    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn fail_on_too_many_variables() -> Result<(), Error> {
        let input = 0b0_000001110011011_0000000000000000000000000011001000_0011_0000000000__i64
            .to_be_bytes();
        let mut iter = RapidBinIterator::new(&input[..], 1, 0, 0, 1);
        iter.next().unwrap().unwrap_err();

        Ok(())
    }
}
