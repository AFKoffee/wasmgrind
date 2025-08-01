# Wasmgrind JS
WasmgrindJS is an experimental project trying to make Wasmgrind work in browsers. 

**Note:** This project is currently neither performant nor ergonomic to use. It was meant as a proof of concept rather than a usable product but this may change in the future.

## The WasmgrindJS Utility Crate
This is a library written in Rust that offers JavaScript bindings to various helper functions in order to enable thread management, execution tracing and binary instrumentation from JavaScript. It should be compiled using `wasm-pack` with a _nightly toolchain_ because it depends atomic WebAssembly instructions and shared memory.

The next two chapters provide insights about the internals of this crate.

## JavaScript Runtime Environment ([Details...](./wasmgrind_js/js_runtime_environment.md))
This is a collection of JavaScript files, which represent a working multihreaded runtime environment inside the browser. The code in these files takes care of the instantiation and instrumentation of WebAssembly modules, execution tracing, the implementation of the internal runtime API and the management of threads using WebWorkers.

## Demo Server
The project also contains an `index.html` and a `server.py`, which can be used to spin up a demo server in order to examine how the above parts are put together.