# Tracing Runtime
The tracing runtime is meant to provide an execution environment for multithreaded WebAssembly including tracing capabilities. It can handle WebAssembly modules requiring the following functions from the internal runtime API and wasabi instrumentation:
- `wasm_threadlink` `panic`: `(i32) -> ()`
- `wasm_threadlink` `thread_create`: `(i32, i32, i32, i32) -> (i32)`
- `wasm_threadlink` `thread_join`: `(i32, i32, i32) -> (i32)`
- `wasm_threadlink` `start_lock`: `(i32, i32, i32) -> ()`
- `wasm_threadlink` `finish_lock`: `(i32, i32, i32) -> ()`
- `wasm_threadlink` `start_unlock`: `(i32, i32, i32) -> ()`
- `wasm_threadlink` `finish_unlock`: `(i32, i32, i32) -> ()`
- `wasabi` `read_hook`: `(i32, i32, i32, i32) -> ()`
- `wasabi` `write_hook`: `(i32, i32, i32, i32) -> ()`

It also expects the module to import a _shared memory_ via `env` `memory`.

In contrast to the standalone runtime, the tracing runtime offers the ability to emit the current state of the execution trace in binary format.

## Tracing Runtime Builder
A runtime builder has to be used to create a runtime. The builder offers the ability to submit closures that implement addtional imports, which the binary may specify apart from the above ones.