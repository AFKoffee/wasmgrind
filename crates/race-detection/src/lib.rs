use std::io::{Read, Seek, Write};

use anyhow::Error;

use crate::generic::{Encoder, Parser};

/// Generic traits and structs for parsing and encoding of execution traces
pub mod generic;
mod rapidbin;
mod std_format;

/// Execution tracing utilities specifically for Wasmgrind
pub mod tracing;

pub use rapidbin::{encoder::RapidBinEncoder, parser::RapidBinParser};
pub use std_format::StdFormatEncoder;

/// Converts an execution trace from one format into another
pub fn convert<P: Parser, E: Encoder, I: Read, O: Write + Seek>(
    parser: &mut P,
    encoder: &mut E,
    input: I,
    mut output: O,
) -> Result<(), Error> {
    encoder.encode(parser.parse(input)?, &mut output)?;

    output.flush()?;

    Ok(())
}
