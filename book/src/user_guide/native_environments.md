# Native Environments
There are two ways of using Wasmgrind on native platforms: as _binary_ or _library_.

## Wasmgrind as a Binary
The binary offers a simple command line interface to Wasmgrind that should not yet be considered stable. It is intended to be used with binaries that do **not** require any imports besides the internal runtime ABI.

Wasmgrind currently offers the following CLI:

    wasmgrind [OPTIONS] <BINARY> <FUNCTION>

Where `<BINARY>` is the path to a binary WebAssembly module that imports the _standalone_ internal runtime ABI, i.e., that is compiled as described in the last chapter and `FUNCTION` is the name of an exported function of the binary, which takes no parameters and returns no result.

The `[OPTIONS]` can be any number of the following:
- **`-t`** or **`--tracing`**:  
Executes the binary with execution tracing. This option assumes that the WebAssembly module imports the _tracing_ internal runtime ABI. It emits the trace in RapidBin format and its corresponding metadata as `trace.bin` and `trace.json` file inside the working directory after the specified function terminates.
- **`-i`** or **`--interactive`**:  
Executes the binary in interactive mode. This option is aimed at cases, where the called function deadlocks or does not return on purpose - e.g. an internal event loop. The program will run a simple repl offering the following commands while the specified function is executed:
    - `finish`: Exit interactive mode and resume to standard execution.
    - `save`: Save the _current_ state of the execution trace.
    - `exit`: Terminate program execution.  
- **`--emit-patched`**:  
Emits the WebAssembly binary into a `tmp` folder (creating it if necessary) after it has been patched by `wasm-threadify`. The module is emitted in WebAssembly Text Format (`patched.wat`) and WebAssembly Binary Format (`patched.wasm`).
- **`--emit-instrumented`**:  
Emits the WebAssembly binary into a `tmp` folder (creating it if necessary) after it has been instrumented by `wasabi`. The module is emitted in WebAssembly Text Format (`instrumented.wat`) and WebAssembly Binary Format (`instrumented.wasm`). This option has no effect if the `--tracing` option is not set.
- **`-h`** or **`--help`**: Print usage information to the console.

**Note:** If you are running Wasmgrind from inside the project directory, use:
    
    cargo run -- [OPTIONS] <BINARY> <FUNCTION>

## Wasmgrind as a Library
If the WebAssembly binary to be examined depends on custom function imports, you have to use Wasmgrind as a library and embedd it into your project. This gives you control over additional imports that should be present upon instantiating your module.

For usage instructions with regard to the Rust library API, refer to the [wasmgrind docs.rs](https://afkoffee.github.io/wasmgrind/wasmgrind-docs-rs/wasmgrind/) site.