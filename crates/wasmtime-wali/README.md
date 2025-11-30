# Wasmtime Wali
This crate provides WALI support for the Wasmtime WebAssembly runtime such that ordinary C programs compiled with the WALI toolchain can be run with Wasmgrind. It is in _early development stage_ and considered **highly experimental** at this point.

## What is WALI?
The WebAssembly Linux Interface is a low-level abstraction layer provides access to Linux system calls to WebAssembly binaries. See the [WALI repository](https://github.com/arjunr2/WALI) for details.

## Attributions
This library is at its heart a port of the [reference implementation](https://github.com/SilverLineFramework/wasm-micro-runtime/tree/9b8b393be751b16ee4bf507b4c69f2fb20c5bd62) written by the WALI authors to Rust. However, it has a more sophisticated memory management, is targeted at Wasmtime and currently only supports the subset of system calls necessary to run the programs of the Wasmgrind Benchmark Suite.