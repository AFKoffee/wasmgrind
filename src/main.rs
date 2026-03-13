use std::path::PathBuf;

use anyhow::Error;
use log::{Level, LevelFilter};
use log4rs::{
    Config as LogConfig,
    append::{
        console::{ConsoleAppender, Target},
        file::FileAppender,
    },
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    filter::threshold::ThresholdFilter,
};

use crate::{
    cli::{Cli, Cmd, ExecCmd},
    cmd::{ProfilingOptions, RtPhaseMarkers, dump::DumpCmd, run::RunCmd, trace::TraceCmd},
};

mod cli;
mod cmd;

fn init_logging(level: Level, dir: Option<PathBuf>) -> Result<(), Error> {
    let console = ConsoleAppender::builder()
        .target(Target::Stderr)
        .encoder(Box::new(PatternEncoder::new("{h({l})} - {m}{n}")))
        .build();

    let mut filter = match level {
        Level::Error => LevelFilter::Error,
        Level::Warn => LevelFilter::Warn,
        Level::Info => LevelFilter::Info,
        Level::Debug => LevelFilter::Debug,
        Level::Trace => LevelFilter::Trace,
    };

    const CONSOLE_APPENDER: &str = "stderr";
    let mut logconfig = LogConfig::builder().appender(
        Appender::builder()
            .filter(Box::new(ThresholdFilter::new(filter)))
            .build(CONSOLE_APPENDER, Box::new(console)),
    );
    let mut rootconfig = Root::builder().appender(CONSOLE_APPENDER);

    if let Some(dir) = dir {
        let logfile = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new(
                "{d} {i} {l}{D( {t} {f} {L})} - {m}{n}",
            )))
            .build(dir.join("wasmgrind-$TIME{%Y-%m-%d}.log"))?;

        filter = filter.increment_severity();

        const LOGFILE_APPENDER: &str = "logfile";
        logconfig = logconfig.appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(filter)))
                .build(LOGFILE_APPENDER, Box::new(logfile)),
        );
        rootconfig = rootconfig.appender(LOGFILE_APPENDER)
    }

    let config = logconfig.build(rootconfig.build(filter))?;

    log4rs::init_config(config)?;

    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    let args = Cli::args();

    if let Some(level) = args.loglevel() {
        init_logging(level, args.logdir)?;
    }

    match args.cmd {
        Cmd::Dump { binary } => DumpCmd { binary }.exec()?,
        Cmd::Profile { markers, exec_cmd } => {
            let markers = markers.map(|marker_option| {
                // Start phase marker timer
                RtPhaseMarkers::timer();
                RtPhaseMarkers::from(marker_option)
            });
            let options = ProfilingOptions {
                markers,
                emit_trace: false,
            };
            match exec_cmd {
                ExecCmd::Run { binary, interface } => {
                    RunCmd {
                        binary,
                        interface: interface.into(),
                    }
                    .exec_with_options(&options)?;
                }
                ExecCmd::Trace {
                    binary,
                    cachedir,
                    emit_instrumented,
                    outdir,
                    outfile,
                    interface,
                } => {
                    TraceCmd {
                        binary,
                        cachedir,
                        emit_instrumented,
                        outdir,
                        outfile,
                        interface: interface.into(),
                    }
                    .exec_with_options(&options)?;
                }
            }
        }
        Cmd::Exec(cmd) => match cmd {
            ExecCmd::Run { binary, interface } => {
                RunCmd {
                    binary,
                    interface: interface.into(),
                }
                .exec()?;
            }
            ExecCmd::Trace {
                binary,
                cachedir,
                emit_instrumented,
                outdir,
                outfile,
                interface,
            } => {
                TraceCmd {
                    binary,
                    cachedir,
                    emit_instrumented,
                    outdir,
                    outfile,
                    interface: interface.into(),
                }
                .exec()?;
            }
        },
    }

    Ok(())
}
