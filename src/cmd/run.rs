use std::path::PathBuf;

use anyhow::{Error, anyhow};
use wasmgrind::standalone::ctx::StandaloneCtxProvider;
use wasmtime::{Config, Engine, Linker, ProfilingStrategy, Store};
use wasmtime_wali::ctx::WaliCtxProvider;

use crate::cmd::{
    ProfilingOptions, RtInterface, RtPhaseMarkers, emit_to_file, run_standalone_binary_func,
};

pub struct RunCmd {
    pub binary: PathBuf,
    pub interface: RtInterface,
}

impl RunCmd {
    pub fn exec(self) -> Result<(), Error> {
        self.exec_with_options(&ProfilingOptions::new())
    }

    pub fn exec_with_options(self, options: &ProfilingOptions) -> Result<(), Error> {
        let mut config = Config::new();
        if let Some(RtPhaseMarkers::Perf) = &options.markers {
            config.profiler(ProfilingStrategy::PerfMap);
        }

        match self.interface {
            RtInterface::Standalone {
                emit_patched,
                function,
            } => run_standalone(self.binary, config, emit_patched, function, options),
            RtInterface::Wali { args } => run_wali(self.binary, config, args, options),
            RtInterface::Wasi => {
                todo!("Support for WASI (wasi-threads-p1) is not yet implemented.")
            }
        }
    }
}

fn run_standalone(
    binary: PathBuf,
    config: Config,
    emit_patched: bool,
    function: String,
    options: &ProfilingOptions,
) -> Result<(), Error> {
    let engine = Engine::new(&config)?;

    let (provider, mut module) = StandaloneCtxProvider::from_file(&engine, &binary)?;

    if emit_patched {
        emit_to_file("tmp", &module.emit_wasm(), "patched")?;
    }

    let linker = Linker::new(provider.engine());

    let ctx = provider.create_ctx();

    run_standalone_binary_func::<_, (), ()>(linker, provider, ctx, function, (), options)
}

fn run_wali(
    binary: PathBuf,
    mut config: Config,
    mut args: Vec<String>,
    profile: &ProfilingOptions,
) -> Result<(), Error> {
    let provider = WaliCtxProvider::from_config(&mut config)?.with_file(&binary)?;

    let mut linker = Linker::new(provider.engine());
    unsafe {
        provider.add_to_linker(&mut linker)?;
    }

    let program_name = binary
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .ok_or(anyhow!(
            "Could not determine program name for binary '{}'",
            binary.display()
        ))?;

    args.insert(0, program_name);
    let ctx = provider.create_ctx(args)?;

    let mut store = Store::new(provider.engine(), ctx);

    if let Some(markers) = &profile.markers {
        markers.begin_wasm()?;
    }

    provider.run(&mut store, linker)?;

    if let Some(markers) = &profile.markers {
        markers.end_wasm()?;
    }

    Ok(())
}
