use std::path::PathBuf;

use anyhow::{Error, anyhow, bail};
use walrus::Module;
use wasmgrind::{
    standalone::{
        StandaloneCtxView, StandaloneView,
        ctx::{StandaloneCtxProvider, WasmgrindStandaloneCtx},
    },
    tracing::{TracingCtxView, TracingView, ctx::WasmgrindTracingCtx},
};
use wasmtime::{Config, Engine, Linker, ProfilingStrategy, Store};
use wasmtime_wali::{
    WaliCtxView, WaliView,
    ctx::{WaliCtx, WaliCtxProvider},
};

use crate::cmd::{
    ProfilingOptions, RtInterface, RtPhaseMarkers, emit_to_file, load_and_instrument,
    run_standalone_binary_func,
};

pub struct TraceCmd {
    pub binary: PathBuf,
    pub cachedir: PathBuf,
    pub emit_instrumented: bool,
    pub outdir: PathBuf,
    pub outfile: PathBuf,
    pub interface: RtInterface,
}

impl TraceCmd {
    pub fn exec(self) -> Result<(), Error> {
        self.exec_with_options(&ProfilingOptions::new())
    }

    pub fn exec_with_options(self, options: &ProfilingOptions) -> Result<(), Error> {
        let mut config = Config::new();
        if let Some(RtPhaseMarkers::Perf) = options.markers {
            config.profiler(ProfilingStrategy::PerfMap);
        }

        let program_name = self
            .binary
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .ok_or(anyhow!(
                "Could not determine program name for binary '{}'",
                self.binary.display()
            ))?;

        let mut module = load_and_instrument(self.binary)?;

        if self.emit_instrumented {
            emit_to_file("tmp", &module.emit_wasm(), "instrumented")?;
        }

        let tracing_ctx = match self.interface {
            RtInterface::Standalone {
                emit_patched,
                function,
            } => trace_standalone(
                module,
                config,
                emit_patched,
                self.cachedir,
                function,
                options,
            )?,
            RtInterface::Wali { mut args } => {
                args.insert(0, program_name);
                trace_wali(module, config, self.cachedir, args, options)?
            }
            RtInterface::Wasi => todo!(),
        };

        if options.emit_trace {
            std::fs::create_dir_all(&self.outdir)?;
            let outfile = self.outdir.join(self.outfile);
            let trace_file = outfile.with_extension("data");
            match tracing_ctx.generate_binary_trace(&trace_file) {
                Ok(metadata) => {
                    std::fs::write(outfile.with_extension("json"), metadata?.to_json()?)
                        .map_err(Error::from)?;
                }
                Err(_) => bail!(
                    "Could not generate binary trace. Some thread still holds a reference to the trace!"
                ),
            };
        }

        Ok(())
    }
}

#[derive(Clone)]
struct StandaloneTracingCtx {
    standalone_ctx: WasmgrindStandaloneCtx,
    tracing_ctx: WasmgrindTracingCtx,
}

impl StandaloneView for StandaloneTracingCtx {
    fn ctx(&self) -> wasmgrind::standalone::StandaloneCtxView<'_> {
        StandaloneCtxView::from(&self.standalone_ctx)
    }
}

impl TracingView for StandaloneTracingCtx {
    fn ctx(&self) -> wasmgrind::tracing::TracingCtxView<'_> {
        TracingCtxView::from(&self.tracing_ctx)
    }
}

#[derive(Clone)]
struct WALITracingCtx {
    wali_ctx: WaliCtx,
    tracing_ctx: WasmgrindTracingCtx,
}

impl WaliView for WALITracingCtx {
    fn ctx(&self) -> wasmtime_wali::WaliCtxView<'_> {
        WaliCtxView::from(&self.wali_ctx)
    }
}

impl TracingView for WALITracingCtx {
    fn ctx(&self) -> wasmgrind::tracing::TracingCtxView<'_> {
        TracingCtxView::from(&self.tracing_ctx)
    }
}

fn trace_standalone(
    mut binary: Module,
    config: Config,
    emit_patched: bool,
    cachedir: PathBuf,
    function: String,
    options: &ProfilingOptions,
) -> Result<WasmgrindTracingCtx, Error> {
    let engine = Engine::new(&config)?;

    let provider = StandaloneCtxProvider::from_walrus(&engine, &mut binary)?;

    if emit_patched {
        emit_to_file("tmp", &binary.emit_wasm(), "patched")?;
    }

    let mut linker = Linker::new(provider.engine());
    WasmgrindTracingCtx::add_to_linker(&mut linker)?;

    let ctx = StandaloneTracingCtx {
        standalone_ctx: provider.create_ctx(),
        tracing_ctx: WasmgrindTracingCtx::new(cachedir),
    };

    run_standalone_binary_func::<_, (), ()>(linker, provider, ctx.clone(), function, (), options)?;

    Ok(ctx.tracing_ctx)
}

fn trace_wali(
    mut binary: Module,
    mut config: Config,
    cachedir: PathBuf,
    args: Vec<String>,
    options: &ProfilingOptions,
) -> Result<WasmgrindTracingCtx, Error> {
    let provider = WaliCtxProvider::from_config(&mut config)?.with_walrus(&mut binary)?;

    let mut linker = Linker::new(provider.engine());
    WasmgrindTracingCtx::add_to_linker(&mut linker)?;
    unsafe {
        provider.add_to_linker(&mut linker)?;
    }

    let ctx = WALITracingCtx {
        wali_ctx: provider.create_ctx(args)?,
        tracing_ctx: WasmgrindTracingCtx::new(cachedir),
    };

    let mut store = Store::new(provider.engine(), ctx.clone());

    if let Some(markers) = &options.markers {
        markers.begin_wasm()?;
    }

    provider.run(&mut store, linker)?;

    if let Some(markers) = &options.markers {
        markers.end_wasm()?;
    }

    Ok(ctx.tracing_ctx)
}
