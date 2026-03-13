use std::{
    io::{Write, stdout},
    path::Path,
    sync::{OnceLock, atomic::Ordering},
    time::Instant,
};

use anyhow::{Error, anyhow};
use wasmgrind::standalone::{StandaloneView, ctx::StandaloneCtxProvider};
use wasmtime::{Linker, Store, WasmParams, WasmResults};

pub mod dump;
pub mod run;
pub mod trace;

pub enum RtInterface {
    Standalone {
        emit_patched: bool,
        function: String,
    },
    Wali {
        args: Vec<String>,
    },
    Wasi,
}

pub enum RtPhaseMarkers {
    Perf,
    MarkersOnly,
}

impl RtPhaseMarkers {
    pub fn timer() -> &'static Instant {
        static START: OnceLock<Instant> = OnceLock::new();

        START.get_or_init(Instant::now)
    }

    fn emit_marker(&self, name: &str) -> Result<(), Error> {
        // The compiler fences are here to prevent compiler re-ordering
        // of memory operations accross marker emission
        //
        // See: https://rust.docs.kernel.org/6.8/core/sync/atomic/fn.compiler_fence.html
        std::sync::atomic::compiler_fence(Ordering::SeqCst);

        let mut out = stdout().lock();
        writeln!(
            out,
            "@@WASMGRIND:{name}:{}ns@@",
            RtPhaseMarkers::timer().elapsed().as_nanos()
        )?;
        out.flush()?;

        std::sync::atomic::compiler_fence(Ordering::SeqCst);
        Ok(())
    }

    #[inline(never)]
    #[unsafe(no_mangle)]
    pub fn begin_wasm(&self) -> Result<(), Error> {
        self.emit_marker("begin-wasm")
    }

    #[inline(never)]
    #[unsafe(no_mangle)]
    pub fn end_wasm(&self) -> Result<(), Error> {
        self.emit_marker("end-wasm")
    }
}

pub struct ProfilingOptions {
    pub markers: Option<RtPhaseMarkers>,
    pub emit_trace: bool,
}

impl ProfilingOptions {
    pub fn new() -> Self {
        Self {
            markers: None,
            emit_trace: true,
        }
    }
}

fn load_and_instrument<P: AsRef<Path>>(binary: P) -> Result<walrus::Module, Error> {
    let mut module = walrus::Module::from_file(binary)?;
    wasmgrind_core::instrumentation::instrument(&mut module)?;
    Ok(module)
}

fn emit_to_file<P: AsRef<Path>>(parent_dir: P, wasm: &[u8], name: &str) -> Result<(), Error> {
    std::fs::create_dir_all(&parent_dir)?;

    let file = parent_dir.as_ref().join(name);
    let wasm_file = file.with_extension("wasm");
    let wat_file = file.with_extension("wat");

    std::fs::write(&wasm_file, wasm)?;
    std::fs::write(&wat_file, wasmprinter::print_bytes(wasm)?)?;

    Ok(())
}

fn run_standalone_binary_func<T, Params, Results>(
    mut linker: Linker<T>,
    provider: StandaloneCtxProvider<T>,
    ctx: T,
    function: String,
    params: Params,
    options: &ProfilingOptions,
) -> Result<Results, Error>
where
    T: StandaloneView + Clone + 'static,
    Params: WasmParams,
    Results: WasmResults,
{
    log::warn!(
        "The Wasmgrind Standalone interface is outdated and untested. Prepare for runtime errors!"
    );

    let main_tid = ctx.ctx().next_available_tid();
    let mut store = Store::new(provider.engine(), ctx);
    provider.add_to_linker(&mut linker, &store)?;

    if let Some(markers) = &options.markers {
        markers.begin_wasm()?;
    }

    let instance = linker.instantiate(&mut store, provider.module())?;
    provider.finalize(linker)?;

    instance
        .get_func(&mut store, "__wasmgrind_bootstrap")
        .expect("Wasmgrind standalone needs an exported function named '__wasmgrind_bootstrap'")
        .typed::<u32, ()>(&store)?
        .call(&mut store, main_tid)?;

    let results = instance
        .get_func(&mut store, &function)
        .ok_or(anyhow!("No function export named '{function}'"))?
        .typed::<Params, Results>(&store)?
        .call(&mut store, params)?;

    if let Some(markers) = &options.markers {
        markers.end_wasm()?;
    }

    Ok(results)
}
