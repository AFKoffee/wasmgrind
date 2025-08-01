use std::fs;

use anyhow::Error;
use clap::{Parser, ValueEnum};
use race_detection::tracing::BinaryTraceOutput;

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    emit_patched: bool,

    #[arg(long)]
    emit_instrumented: bool,

    #[arg(short, long)]
    tracing: bool,

    #[arg(value_enum)]
    example: Example,
}

#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord, ValueEnum)]
enum Example {
    TwoNestedThreads,
    TwoNestedThreadsDetached,
    ThreadHierarchy
}

const WASM_MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/minimal_test.wasm"));
const WASM_MODULE_WITH_TRACING: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/minimal_test_tracing.wasm"));

fn main() -> Result<(), Error> {
    let args = Cli::parse();

    let dir = tempfile::tempdir()?;
    let file = dir.path().join("binary.wasm");
    fs::write(&file, if args.tracing { WASM_MODULE_WITH_TRACING } else { WASM_MODULE })?;

    let function_name = match args.example {
        Example::TwoNestedThreads => "two_nested_threads_test",
        Example::TwoNestedThreadsDetached => "two_nested_detached_threads_test",
        Example::ThreadHierarchy => "thread_hierarchy_test",
    }.into();

    if args.tracing {
        let runtime = wasmgrind::wasmgrind(file, args.emit_patched, args.emit_instrumented)?;
        let runner = runtime.invoke_function::<(), ()>(function_name, ());
        runner.join().expect("Error: Runner Thread panicked!")?;
        save_trace(runtime.generate_binary_trace()?)
    } else {
        wasmgrind::run(file, function_name, args.emit_patched)?
            .join()
            .expect("Error: Runner Thread panicked!")
    }
} 

fn save_trace(output: BinaryTraceOutput) -> Result<(), Error> {
    std::fs::write("trace.bin", output.trace).map_err(Error::from)?;
    std::fs::write("trace.json", output.metadata.to_json()?).map_err(Error::from)
}