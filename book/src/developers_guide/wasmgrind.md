# Wasmgrind
This crate contains the native engine of Wasmgrind. It provides a [Standalone Runtime](./wasmgrind/standalone_runtime.md) and a [Tracing Runtime](./wasmgrind/tracing_runtime.md) with [Native Thread Management](./wasmgrind/native_tmgmt.md).

To minimize code duplication those runtimes utilize [Runtime Contexts](./wasmgrind/runtime_contexts.md) to provide appropriate implementations of the internal runtime API. The [wasmgrind-macros](./wasmgrind_macros.md) crate plays an important part here.