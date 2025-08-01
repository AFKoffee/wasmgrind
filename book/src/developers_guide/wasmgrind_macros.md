# Wasmgrind Macros
The wasmgrind-macros crate contains procedural macros used by the native wasmgrind engine.

**ATTENTION:** This crate is currently tightly coupled with the wasmgrind crate. Changes in the wasmgrind crate can lead to errors with regard to the macros and vice versa. Both crates should be seen as one single crate.

## Assumptions
The macros assume that the following crates, modules and functions are accessible in the context of invocation:
- `wasmgrind_core`
- `wasmgrind_error`
- `race_detection`
- `wasmtime`
- `anyhow`
- `crate::runtime::base::ThreadlinkRuntime::write_data_to_memory`
- `std::thread`

## The `thread_create_func!` Macro
This macro creates a closure that implements the `thread_create` function of the internal runtime API. It emits different types of closures depending on the number of arguments received:
- 5 arguments: emits a closure that expects 2 parameters and does not include tracing of `fork` events.
- 6 arguments: emits a closure that expects 4 parameters and does include tracing of `fork` events.

In both cases the first 5 arguments are expected to be ...
- ... of type `wasmtime::Engine`
- ... of type `wasmtime::Module`
- ... of type `wasmtime::SharedMemory`
- ... of type `wasmtime::Linker`
- ... of type `wasmgrind::runtime::Tmgmt`

The optional 6th argument is expected to be of type `wasmgrind::runtime::ArcTracing`.

## The `thread_join_func!` Macro
This macro creates a closure that implements the `thread_join` function of the internal runtime API. It emits different types of closures depending on the number of arguments received:
- 1 argument: emits a closure that expects 1 parameter and does not include tracing of `join` events.
- 2 arguments: emits a closure that expects 3 parameters and does include tracing of `join` events.

In both cases the first argument is expected to be of type `wasmgrind::runtime::Tmgmt`.

The optional second argument is expected to be of type `wasmgrind::runtime::ArcTracing`.