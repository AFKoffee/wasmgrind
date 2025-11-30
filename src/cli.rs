use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand, ValueEnum};

use crate::cmd::{RtInterface, RtPhaseMarkers};

#[derive(Parser)]
pub struct Cli {
    /// Increase console output
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,

    /// Decrease console output
    #[arg(short, long, action = ArgAction::Count)]
    quiet: u8,

    /// Write a logfile to a specific dir (console log-level + 1)
    #[arg(short, long)]
    pub logdir: Option<PathBuf>,

    /// The Wasmgrind command to be executed
    #[command(subcommand)]
    pub cmd: Cmd,
}

impl Cli {
    pub fn args() -> Self {
        Self::parse()
    }

    pub fn loglevel(&self) -> Option<log::Level> {
        let verbosity = self.verbose.saturating_add(2).saturating_sub(self.quiet);

        match verbosity {
            0 => None,
            1 => Some(log::Level::Error),
            2 => Some(log::Level::Warn),
            3 => Some(log::Level::Info),
            4 => Some(log::Level::Debug),
            _ => Some(log::Level::Trace),
        }
    }
}

#[derive(Subcommand)]
pub enum ExecCmd {
    /// Execute a multithreaded WebAssembly binary
    Run {
        /// The binary to be run
        binary: PathBuf,

        /// The interface used to enable threading
        #[command(subcommand)]
        interface: Interface,
    },
    /// Trace the execution of a multithreaded WebAssembly binary
    Trace {
        /// The binary to be traced
        binary: PathBuf,

        /// Directory where the on-disk cache of the trace should be located
        #[arg(long, default_value = ".wasmgrind-cache")]
        cachedir: PathBuf,

        /// Emit *.wasm and *.wat of the binary after instrumentation
        #[arg(long)]
        emit_instrumented: bool,

        /// Directory where the generated *.data/*.json files are placed
        #[arg(long, default_value = ".")]
        outdir: PathBuf,

        /// Name of the generated *.data/*.json files
        #[arg(long, default_value = "trace")]
        outfile: PathBuf,

        /// The interface used to enable threading
        #[command(subcommand)]
        interface: Interface,
    },
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Dump instrumented WebAssembly binary to file
    Dump {
        /// The binary to be instrumented
        binary: PathBuf,
    },
    /// Run Wasmgrind with profiling options
    Profile {
        /// Specifies, how phase markers are emitted
        #[arg(short, long, value_enum)]
        markers: Option<PhaseMarkers>,

        #[command(subcommand)]
        exec_cmd: ExecCmd,
    },
    #[command(flatten)]
    Exec(ExecCmd),
}

#[derive(Clone, ValueEnum)]
pub enum PhaseMarkers {
    Perf,
    Stdout,
}

#[derive(Subcommand)]
pub enum Interface {
    /// Use Wasmgrind's standalone interface
    Standalone {
        /// Emit *.wasm and *.wat after inserting patching code
        #[arg(long)]
        emit_patched: bool,

        /// The function to execute (needs to be of type () -> ())
        function: String,
    },
    /// Use the WebAssembly Linux Interface
    Wali {
        /// Command line arguments for the WALI application
        args: Vec<String>,
    },
    /// Use the WebAssembly System Interface (P1 with wasi-threads)
    Wasi,
}

impl From<Interface> for RtInterface {
    fn from(value: Interface) -> Self {
        match value {
            Interface::Standalone {
                emit_patched,
                function,
            } => Self::Standalone {
                emit_patched,
                function,
            },
            Interface::Wali { args } => Self::Wali { args },
            Interface::Wasi => Self::Wasi,
        }
    }
}

impl From<PhaseMarkers> for RtPhaseMarkers {
    fn from(value: PhaseMarkers) -> Self {
        match value {
            PhaseMarkers::Perf => RtPhaseMarkers::Perf,
            PhaseMarkers::Stdout => RtPhaseMarkers::MarkersOnly,
        }
    }
}
