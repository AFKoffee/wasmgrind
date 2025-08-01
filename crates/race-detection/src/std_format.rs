use crate::generic::{Encoder, Event, EventResult, Operation};
use std::io::{Seek, Write};

/// An encoder to emit execution traces in _RapidBin_ format
pub struct StdFormatEncoder;

impl StdFormatEncoder {
    pub fn new() -> Self {
        Self {}
    }

    fn encode_event(&self, event: Event) -> String {
        let (thread_id, operation, location) = event.into_fields();

        let op_and_decor = match operation {
            Operation::Aquire { lock } => format!("acq(L{})", lock),
            Operation::Release { lock } => format!("rel(L{})", lock),
            Operation::Read { memory } => format!("r(V{})", memory),
            Operation::Write { memory } => format!("w(V{})", memory),
            Operation::Fork { tid } => format!("fork(T{})", tid),
            Operation::Join { tid } => format!("join(T{})", tid),
            Operation::Request { lock } => format!("req(L{})", lock),
        };

        format!("T{}|{}|{}", thread_id, op_and_decor, location)
    }
}

impl Default for StdFormatEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Encoder for StdFormatEncoder {
    const EVENT_SIZE_HINT: usize = 1;

    fn encode<W: Write + Seek, I: IntoIterator<Item = EventResult>>(
        &mut self,
        input: I,
        mut output: W,
    ) -> Result<(), anyhow::Error> {
        for event in input {
            writeln!(output, "{}", self.encode_event(event?))?
        }

        Ok(())
    }

    fn format(&self) -> &'static str {
        "STD"
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use anyhow::Error;

    use crate::generic::{Encoder, Event, EventResult, Operation};

    use super::StdFormatEncoder;

    #[test]
    fn encode_valid_trace() -> Result<(), Error> {
        let generic_trace: Vec<EventResult> = vec![
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
        let mut encoder = StdFormatEncoder::new();
        encoder.encode(generic_trace, &mut buffer)?;

        let encoded_trace = String::from_utf8(buffer.into_inner())?;
        let std_trace: String = [
            "T0|fork(T1)|42",
            "T0|fork(T2)|42",
            "T2|fork(T3)|123",
            "T0|req(L0)|362",
            "T0|acq(L0)|362",
            "T0|r(V200)|436",
            "T0|w(V200)|923",
            "T0|rel(L0)|362",
            "T0|join(T1)|7382\n",
        ]
        .join("\n");

        assert_eq!(std_trace, encoded_trace);

        Ok(())
    }
}
