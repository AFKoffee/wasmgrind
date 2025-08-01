# Compiling the Binary
Binaries that are intended to be executed using Wasmgrind runtimes should import and export a set of predefined API functions. If you do not know, what _imports_ and _exports_ are in the context of WebAssembly modules, please check out [Chapter 1: What is WebAssembly?]() first.

Depending on the runtime used, the following functions are expected to be imported:

| Runtime                   | Module Name       | Function Name         | Function Signature        |
| -------                   | -------------     | --------------------  | ------------------        |
| Standalone + Tracing      | `wasm_threadlink` | `thread_create`       | `(i32, i32) -> (i32)`     |
| Standalone + Tracing      | `wasm_threadlink` | `thread_join`         | `(i32) -> (i32)`          |
| Standalone + Tracing      | `wasm_threadlink` | `panic`               | `(i32) -> ()`             |
| Tracing                   | `wasm_threadlink` | `start_lock`          | `(i32) -> ()`             |
| Tracing                   | `wasm_threadlink` | `finish_lock`         | `(i32) -> ()`             |
| Tracing                   | `wasm_threadlink` | `start_unlock`        | `(i32) -> ()`             |
| Tracing                   | `wasm_threadlink` | `finish_unlock`       | `(i32) -> ()`             |

Furthermore, the following functions are expected to be exported:

| Runtime                   | Exported Function Name    | Function Signature        |
| -------                   | -------------             | ------------------        |
| Standalone + Tracing      | `thread_start`            | `(i32) -> ()`             |
| Standalone + Tracing      | `__wasmgrind_malloc`      | `(i32, i32) -> (i32)`             |
| Standalone + Tracing      | `__wasmgrind_free`        | `(i32, i32, i32) -> ()`             |

Refer to chapters [11.1](../developers_guide/project_structure/the_internal_api.md) and [12.1](../developers_guide/wasm_threadify/general_precautions.md) for in-depth explainations on how this API is intended to be used or implemented respectively.

If you are compiling your WebAssembly module from Rust, we recommend to use the threading and synchronization API provided by the `wasm-threadlink` crate. It makes sure that the above imports and exports are present in the output binary (depending on whether you compiled for tracing or not) while providing a Rust interface simpilar to the Rust standard library. The [next chapter](./compiling_the_binary/using_wasm_threadlink.md) gives an overview about how to use wasm-threadlink.