# Standalone Runtime
The standalone runtime is meant to provide an execution environment for multithreaded WebAssembly without tracing capabilities. It can handle WebAssmebly modules requiring the following functions from the internal runtime ABI:
- `wasm_threadlink` `panic`: `(i32) -> ()`
- `wasm_threadlink` `thread_create`: `(i32, i32) -> (i32)`
- `wasm_threadlink` `thread_join`: `(i32) -> (i32)`

It also expects the module to import a _shared memory_ via `env` `memory`.

## Runtime Builder
A runtime builder has to be used to create a runtime. The builder offers the ability to submit closures that implement addtional imports, which the binary may specify apart from the above ones.

