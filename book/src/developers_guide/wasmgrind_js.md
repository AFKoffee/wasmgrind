# Wasmgrind JS
Wasmgrind JS is an experimental project trying to make Wasmgrind work in browsers. 

It is a library written in Rust that offers JavaScript bindings to various functions in order to enable thread management, execution tracing and binary instrumentation from JavaScript. It should be compiled using `wasm-pack` with a _nightly toolchain_ because it depends atomic WebAssembly instructions and shared memory.

The next chapters provide insights about the internals of this crate.