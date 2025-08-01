use std::{fs, io::{stdin, stdout, Write}};

use anyhow::Error;
use clap::Parser;
use race_detection::tracing::BinaryTraceOutput;

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    emit_patched: bool,

    #[arg(long)]
    emit_instrumented: bool,

    #[arg(short, long)]
    tracing: bool,

    #[arg(short, long)]
    interactive: bool,
}

const WASM_MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/deadlock_test.wasm"));
const WASM_MODULE_WITH_TRACING: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/deadlock_test_tracing.wasm"));

const DEADLOCKING_FUNCTION: &str = "create_deadlock";

fn main() -> Result<(), Error> {
    let args = Cli::parse();

    let dir = tempfile::tempdir()?;
    let file = dir.path().join("binary.wasm");
    fs::write(&file, if args.tracing { WASM_MODULE_WITH_TRACING } else { WASM_MODULE })?;

    if args.tracing {
        let runtime = wasmgrind::wasmgrind(file, args.emit_patched, args.emit_instrumented)?;
        let runner = runtime.invoke_function::<(), ()>(DEADLOCKING_FUNCTION.into(), ());
        
        if args.interactive {
            loop {
                print!("deadlock-example> ");
                stdout().flush().unwrap();
                match stdin().lines().next() {
                    Some(Ok(input)) => match input.trim() {
                        "finish" => {
                            println!("Exited interactive mode.");
                            break;
                        }
                        "save" => {
                            println!("Saving trace to file ...");
                            save_trace(runtime.generate_binary_trace()?)?;
                            println!("... saved sucessfully.");
                        }
                        "exit" => {
                            println!("Terminating ...");
                            return Ok(());
                        }
                        _ => println!("Invalid input. Try again!"),
                    },
                    _ => { /* Noop */ }
                }
            }
        }
        
        runner.join().expect("Error: Runner Thread panicked!")?;
        save_trace(runtime.generate_binary_trace()?)
    } else {
        let runtime = wasmgrind::runtime(file, args.emit_patched)?;
        let runner = runtime.invoke_function::<(), ()>(DEADLOCKING_FUNCTION.into(), ());
        
        if args.interactive {
            loop {
                print!("deadlock-example> ");
                stdout().flush().unwrap();
                match stdin().lines().next() {
                    Some(Ok(input)) => match input.trim() {
                        "finish" => {
                            println!("Exited interactive mode.");
                            break;
                        }
                        "ready" => {
                            println!("Function did {} terminate.", if runner.is_finished() { "already" } else  { "not yet"});
                        }
                        "exit" => {
                            println!("Terminating ...");
                            return Ok(());
                        }
                        _ => println!("Invalid input. Try again!"),
                    },
                    _ => { /* Noop */ }
                }
            }
        }
        
        runner.join().expect("Error: Runner Thread panicked!")
    }
} 

fn save_trace(output: BinaryTraceOutput) -> Result<(), Error> {
    std::fs::write("trace.bin", output.trace).map_err(Error::from)?;
    std::fs::write("trace.json", output.metadata.to_json()?).map_err(Error::from)
}