use std::fs;

use anyhow::bail;
use clap::Parser;
use wasmgrind::{runtime_builder, wasmgrind_builder};

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    emit_patched: bool,

    #[arg(long)]
    emit_instrumented: bool,

    #[arg(short, long)]
    tracing: bool,

}

const WASM_MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/custom_import.wasm"));
const WASM_MODULE_WITH_TRACING: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/custom_import_tracing.wasm"));

const TARGET_FUNCTION: &str = "run";

pub fn main() -> Result<(), anyhow::Error> {
    let args = Cli::parse();


    let dir = tempfile::tempdir()?;
    let file = dir.path().join("binary.wasm");
    fs::write(&file, if args.tracing { WASM_MODULE_WITH_TRACING } else { WASM_MODULE })?;

    if args.tracing {
        let runtime = wasmgrind_builder(file, args.emit_patched, args.emit_instrumented)?
            .register_custom_import("custom_import", "multiply", |arg1: u32, arg2: u32| {
                println!("Multiplying: {} + {}", arg1, arg2);
                arg1.saturating_mul(arg2)
            })?
            .register_custom_import("custom_import", "add", |arg1: u32, arg2: u32| {
                println!("Adding: {} + {}", arg1, arg2);
                arg1.saturating_add(arg2)
            })?
            .build();

        match runtime.invoke_function::<(), u32>(TARGET_FUNCTION.into(), ()).join() {
            Ok(val) => println!("Result {}", val?),
            Err(e) => bail!("{e:?}"),
        };
    } else {
        let runtime = runtime_builder(file, args.emit_patched)?
            .register_custom_import("custom_import", "multiply", |arg1: u32, arg2: u32| {
                println!("Multiplying: {} + {}", arg1, arg2);
                arg1.saturating_mul(arg2)
            })?
            .register_custom_import("custom_import", "add", |arg1: u32, arg2: u32| {
                println!("Adding: {} + {}", arg1, arg2);
                arg1.saturating_add(arg2)
            })?
            .build();

        match runtime.invoke_function::<(), u32>(TARGET_FUNCTION.into(), ()).join() {
            Ok(val) => println!("Result {}", val?),
            Err(e) => bail!("{e:?}"),
        };
    }
    Ok(())
}