# Wasmgrind

Wasmgrind is an analysis framework for multi-threaded WebAssembly programs. It provides an embedder-agnostic tracing API and a WebAssembly instrumentation engine to track concurrency-releated as well as memory access events during runtime.

The ultimate goal of this tool is to enable the analysis of concurrent WebAssembly programs in order to detect deadlocks or dataraces.

## Prerequisites
#### 1. Ensure you have the Rust toolchain installed on your system

    rustup --version
    rustc --version
    cargo --version

Otherwise, install it: https://www.rust-lang.org/tools/install

## Quick Start Guide
The following sections describe how to get up and running with wasmgrind quickly. For more in-depth explainations refer to the [Wasmgrind Book](https://afkoffee.github.io/wasmgrind/).

### Compiling Binaries for Wasmgrind
Currently, the only supported way of compiling binaries for Wasmgrind is to use the WALI toolchain. View the [WALI repository](https://github.com/arjunr2/WALI) for more details.

#### Execution Tracing with WALI Binaries
Wasmgrind's execution tracing relies on the analyzed binary itself to notify it about concurrency-related events. To intercept calls to, e.g. `pthread_create`, made by the WALI binary, you have to inject shims for those functions at compile time. The Wasmgrind Benchmark Suite relies on such mechanisms and can be a great starting point for anyone trying to perform execution tracing on WALI binaries.

### Building Wasmgrind
Assuming Cargo is installed on your system, simply run the following command in the root directory of the project to build the Wasmgrind executable:

    cargo build --release 

To make Wasmgrind available on your PATH, you can also choose to run inside the project root:

    cargo install --path .

### Executing Binaries with Wasmgrind
Wasmgrind provides a simple command-line interface to execute and analyze binaries. For example, to run a WebAssembly binary compiled with WALI, execute the following command:

    wasmgrind run path/to/my-binary.wasm wali [ARGS for my-binary.wasm]...

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
