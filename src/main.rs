use std::{
    io::{Write, stdin, stdout},
    path::PathBuf,
};

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
    interactive: bool,

    #[arg(short, long)]
    tracing: bool,

    binary: PathBuf,

    function: String,
}

fn main() -> Result<(), anyhow::Error> {
    let args = Cli::parse();

    if args.tracing {
        let runtime = wasmgrind::wasmgrind(args.binary, args.emit_patched, args.emit_instrumented)?;

        let runner = runtime.invoke_function::<(), ()>(args.function, ());

        if args.interactive {
            loop {
                print!("wasmgrind> ");
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
        wasmgrind::run(args.binary, args.function, args.emit_patched)?
            .join()
            .expect("Error: Runner Thread panicked!")
    }
}

fn save_trace(output: BinaryTraceOutput) -> Result<(), Error> {
    std::fs::write("trace.bin", output.trace).map_err(Error::from)?;
    std::fs::write("trace.json", output.metadata.to_json()?).map_err(Error::from)
}
