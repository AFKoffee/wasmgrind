# Wasmgrind Core Library
This library contains essential functionality for Wasmgrind. It mainly provides modules for binary instrumentation and execution tracing of WebAssembly binaries.

## Attributions
Initially, this library used a [custom Wasabi fork](https://github.com/AFKoffee/wasabi/tree/embedder-agnostic-api) as its instrumentation engine. While we moved to [walrus](https://github.com/wasm-bindgen/walrus) because its more actively maintained and supports bleeding-edge WebAssembly features, the new walrus-based implementation is still inspired by the original [Wasabi](https://github.com/danleh/wasabi) project.

## Third Party Materials
The following files in this directory (including its subdirectories) contain code by
other autors. See their respective license headers for more details:
- src/threadify.rs: Copyright (c) 2014 Alex Crichton
