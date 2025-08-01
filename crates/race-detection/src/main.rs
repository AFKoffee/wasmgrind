use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, BufWriter},
    path::PathBuf,
};

use anyhow::Error;
use clap::Parser;
use race_detection::{RapidBinParser, StdFormatEncoder};

#[derive(Parser)]
struct Cli {
    input: PathBuf,
    output: PathBuf,
}

fn main() -> Result<(), Error> {
    let args = Cli::parse();

    let reader = BufReader::new(File::open(&args.input)?);
    let writer = BufWriter::new(
        OpenOptions::new()
            .truncate(true)
            .write(true)
            .create(true)
            .open(&args.output)?,
    );

    let mut parser = RapidBinParser::new();
    let mut encoder = StdFormatEncoder::new();

    race_detection::convert(&mut parser, &mut encoder, reader, writer)?;

    println!("Trace Output: ");
    let reader = BufReader::new(File::open(args.output)?);
    for line in reader.lines() {
        println!("{}", line?)
    }

    Ok(())
}
