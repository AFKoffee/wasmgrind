# Wasmgrind

Wasmgrind is an analysis framework for multi-threaded WebAssembly programs.

It provides an embedder-agnostic runtime API to enable basic thread management from within WebAssembly, the ability to execute WebAssembly programs compiled against this API as well as tracing capabilities to track events related to concurrency during program runs.

The ultimate goal of this tool is to enable the analysis of concurrent WebAssembly programs in order to detect deadlocks or dataraces.

## Prerequisites
#### 1. Ensure you have the Rust toolchain installed on your system

    rustup --version
    rustc --version
    cargo --version

Otherwise, install it: https://www.rust-lang.org/tools/install

**Important Note:** Wasmgrinds `build.rs` requires `rustup` to work properly. Make sure not only plain `rustc` and `cargo` are installed!

#### 2. Ensure you have `wasm-tools` installed (if you want to fully use the wasmgrind CLI)

    wasm-tools --version

Otherwise install it:

    cargo install wasm-tools

#### 3. Ensure you have `wasm-pack` installed (if you want to try wasmgrind-js)

    wasm-pack --version

Otherwise install it:

    cargo install wasm-pack

## Quick Start Guide
The following sections describe how to get up and running with wasmgrind quickly. For more in-depth explainations refer to the [Wasmgrind Book](https://wasmgrind-d6f2b1.gitlab.io/book/).

### Compiling Binaries for Wasmgrind
Wasmgrind assumes that the provided WebAssembly binary imports a set of API functions needed to create and join threads as well as to record important events for tracing.  Currently, the only two ways to utilize this API is to either use the [wasm-threadlink](crates/wasm-threadlink/) crate in your project or to wrap the internal API using your own code. 

View the projects in the [wasm-artifacts](wasm-artifacts) folder to see how the _wasm-threadlink_ library functions can be used to create and join threads from Rust code in a way that mimics the Rust standard library threading API.

**Note:** Wasmgrind assumes that the WebAssembly binary has beed compiled with the `atomics` feature enabled, which requires a nightly Rust toolchain at the time of this writing. Otherwise, the module can not utilize atomic instructions and shared memory that are the fundamental building blocks of multithreaded WebAssembly.

### Using Wasmgrind in Native Environments
To use wasmgrind on your host machine, navigate to the root directory of this project and type

    cargo run --release -- </path/to/wasm/module> <exported_function_name>

where `/path/to/wasm/module` should be the path to a WebAssembly module compiled against the internal threading API and `exported_function_name` should be an function **without arguments and return types** that is exported by the given module.

### Using Wasmgrind in Browser Environments
The JS version of wasmgrind has been a proof of concept work and will have a lower priority in development than the native runtime. However, if you want to try out wasmgrind in the web, checkout the [browser demo](demos/browser-demo/README.md) to see how to set it up with your own compiled binary rather than our example module.

### Running the Examples
Wasmgrind provides some examples that show how to use its native Rust API. These examples are located in the [examples](examples) folder and can be executed via

    cargo run --example [example-name] -- [OPTIONS]

Each example has its own set of options. Refer to the CLI defined in the source files for detailed information.

### Example Setup - Step By Step
This section will provide a simple step-by-step guide on how to replicate the behavior of `examples/minimal-test` with the standard Wasmgrind CLI. It aims to provide insights into the inner workings of the examples.

#### 1. Compiling the example binary
Assuming you have cloned the repository and are located at the project root, navigate to the example project:

    cd wasm-artifacts/minimal-test

For compilation you have two options.

**Option A:** Compile without tracing

    cargo build --release

**Option B:** Compile with tracing

    cargo build --release --features tracing

The artifacts will be located under `wasm-artifacts/target/wasm32-unknown-unknown/release` unless you configured the target output directory to point to another location.

#### 2. Running the example binary

Navigate back to the root directory:

    cd ../..

We will now run a single test from the WebAssembly module called `two_nested_threads_test`.

If you chose **Option A** for compilation:

    cargo run --release -- /path/to/minimal_test.wasm two_nested_threads_test

If you chose **Option B** for compilation:

    cargo run --release -- /path/to/minimal_test.wasm two_nested_threads_test --tracing

In the latter case you should see a `trace.bin` and a `trace.json` file in your project directory after running the command.

#### 3. Comparison with the Rust example
Assuming you are located at the project root directory:

Following steps 1 and 2 with **Option A** shows the same behavior as running

    cargo run --release --example minimal_test -- two-nested-threads

Following steps 1 and 2 with **Option B** shows the same behavior as running

    cargo run --release --example minimal_test -- two-nested-threads --tracing

## License

This project is licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
