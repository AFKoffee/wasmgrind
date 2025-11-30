use std::io::{Read, Seek, Write};

use anyhow::{Error, anyhow};

/// The generic (format-independent) representation of an operation
#[derive(Debug, PartialEq, Eq)]
pub enum Operation {
    Aquire { lock: u64 },
    Release { lock: u64 },
    Read { memory: u64 },
    Write { memory: u64 },
    Fork { tid: u64 },
    Join { tid: u64 },
    Request { lock: u64 },
}

impl Operation {
    /// Returns an integer that uniquely identifies the type of the operation
    pub fn id(&self) -> u8 {
        match self {
            Operation::Aquire { lock: _ } => 0,
            Operation::Release { lock: _ } => 1,
            Operation::Read { memory: _ } => 2,
            Operation::Write { memory: _ } => 3,
            Operation::Fork { tid: _ } => 4,
            Operation::Join { tid: _ } => 5,
            Operation::Request { lock: _ } => 8,
        }
    }

    pub fn try_from_id(id: i64, decor: u64) -> Result<Self, Error> {
        match id {
            0 => Ok(Operation::Aquire { lock: decor }),
            1 => Ok(Operation::Release { lock: decor }),
            2 => Ok(Operation::Read { memory: decor }),
            3 => Ok(Operation::Write { memory: decor }),
            4 => Ok(Operation::Fork { tid: decor }),
            5 => Ok(Operation::Join { tid: decor }),
            8 => Ok(Operation::Request { lock: decor }),
            _ => Err(anyhow!("Operation-ID was not recognized")),
        }
    }
}

/// The generic (format-independent) representation of an event
#[derive(Debug, PartialEq, Eq)]
pub struct Event {
    thread_id: u64,
    operation: Operation,
    location: u64,
}

impl Event {
    pub fn new(thread_id: u64, operation: Operation, location: u64) -> Self {
        Self {
            thread_id,
            operation,
            location,
        }
    }

    pub fn into_fields(self) -> (u64, Operation, u64) {
        (self.thread_id, self.operation, self.location)
    }

    pub fn get_fields(&self) -> (&u64, &Operation, &u64) {
        (&self.thread_id, &self.operation, &self.location)
    }
}

/// Shared iterator item type for [`Parser`] and [`Encoder`] implementations.
pub type EventResult = Result<Event, Error>;

/// Common trait for parsers of execution traces
pub trait Parser {
    type Iter<R: Read>: Iterator<Item = EventResult>;

    /// Parses an execution trace of some specific format.
    fn parse<R: Read>(&mut self, input: R) -> Result<Self::Iter<R>, Error>;

    /// Returns a string identifying the execution trace format of this parser.
    fn format(&self) -> &'static str;
}

/// Common trait for encoders of execution traces
pub trait Encoder {
    /// A constant that indicates the approximate space in bytes an event will occupy.
    const EVENT_SIZE_HINT: usize;

    /// Encodes an execution trace into some specific format.
    fn encode<W: Write + Seek, I: IntoIterator<Item = EventResult>>(
        &mut self,
        input: I,
        output: W,
    ) -> Result<(), Error>;

    /// Returns a string identifying the execution trace format of this encoder.
    fn format(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::Operation;

    #[test]
    fn fail_on_invalid_operation_id() {
        let valid_decor = 42;
        let valid_ids = [0, 1, 2, 3, 4, 5, 8];

        for id in (-100..100).filter(|id| !valid_ids.contains(id)) {
            Operation::try_from_id(id, valid_decor).unwrap_err();
        }
    }

    #[test]
    fn validate_correct_operation_ids() {
        use super::Operation::*;

        let valid_decor = 42;
        let valid_ids = [0, 1, 2, 3, 4, 5, 8];
        let valid_ops = [
            Aquire { lock: valid_decor },
            Release { lock: valid_decor },
            Read {
                memory: valid_decor,
            },
            Write {
                memory: valid_decor,
            },
            Fork { tid: valid_decor },
            Join { tid: valid_decor },
            Request { lock: valid_decor },
        ];

        for (idx, id) in valid_ids.into_iter().enumerate() {
            let op = Operation::try_from_id(id, valid_decor).unwrap();
            assert_eq!(op, valid_ops[idx])
        }
    }
}
