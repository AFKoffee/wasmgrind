# Summary

[Wasmgrind](./wasmgrind.md)
[Introduction](./introduction.md)

---

# Project Background

- [What is WebAssembly?]()
- [WebAssembly and Multi-Threading]()
- [Analysis of Concurrent Programs]()
- [Patching WebAssembly Binaries]()
- [Project Goals]()

---

# User Guide

- [Getting Started](./user_guide/getting_started.md)
- [Compiling the Binary](./user_guide/compiling_the_binary.md)
    - [Using Wasm-Threadlink](./user_guide/compiling_the_binary/using_wasm_threadlink.md)
    - [Using a modified Rust Standard Library]()
    - [Alternative Approaches]()
- [Native Environments](./user_guide/native_environments.md)
- [Web Environments](./user_guide/web_environments.md)

---

# Developers Guide

- [General Concepts](./developers_guide/general_concepts.md)
- [Project Structure](./developers_guide/project_structure.md)
    - [The Internal API](./developers_guide/project_structure/the_internal_api.md)
    - [Challenges in Error Handling](./developers_guide/project_structure/error_handling.md)
- [Wasm-Threadify](./developers_guide/wasm_threadify.md)
    - [General Precautions](./developers_guide/wasm_threadify/general_precautions.md)
    - [Thread Initialization](./developers_guide/wasm_threadify/thread_initialization.md)
    - [Thread Local Stack](./developers_guide/wasm_threadify/thread_local_stack.md)
    - [Thread Local Storage](./developers_guide/wasm_threadify/thread_local_storage.md)
    - [Thread Destruction](./developers_guide/wasm_threadify/thread_destruction.md)
    - [Utility Functions](./developers_guide/wasm_threadify/utility_functions.md)
- [Wasm-Threadlink](./developers_guide/wasm_threadlink.md)
- [Alloc Exposer](./developers_guide/alloc_exposer.md)
- [Race Detection](./developers_guide/race_detection.md)
    - [Tracing](./developers_guide/race_detection/tracing.md)
    - [RapidBin - The Binary Trace Format](./developers_guide/race_detection/rapid_bin.md)
- [Wasmgrind Core](./developers_guide/wasmgrind_core.md)
    - [Thread Management Utilities](./developers_guide/wasmgrind_core/thread_management.md)
    - [WebAssembly Instrumentation](./developers_guide/wasmgrind_core/wasm_instrumentation.md)
    - [Other Useful Wrappers](./developers_guide/wasmgrind_core/other_wrappers.md)
- [Wasmgrind Error](./developers_guide/wasmgrind_error.md)
- [Wasmgrind JS]()
- [Wasmgrind Macros](./developers_guide/wasmgrind_macros.md)
- [Wasmgrind](./developers_guide/wasmgrind.md)
    - [Native Thread Management](./developers_guide/wasmgrind/native_tmgmt.md)
    - [Standalone Runtime](./developers_guide/wasmgrind/standalone_runtime.md)
    - [Tracing Runtime](./developers_guide/wasmgrind/tracing_runtime.md)
    - [Runtime Contexts](./developers_guide/wasmgrind/runtime_contexts.md)
- [Further Reading](./developers_guide/further_reading.md)

---

[Concluding Remarks]()