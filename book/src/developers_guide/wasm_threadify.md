# Wasm-Threadify

The Wasm-Threadify crate is responsible of patching a compiled WebAssembly binary to make it suitable for multithreading with a _"one instance per thread"_ threading model. The code for this crate is mainly taken from the [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen) tool, which includes a specific pass to patch a WebAssembly module for multithreading: [threads-xform](https://github.com/rustwasm/wasm-bindgen/tree/main/crates/threads-xform). It uses the [walrus](https://github.com/rustwasm/walrus) transformation library for WebAssembly under the hood.

The crate consists of four central building blocks:

1. **General Precauitions**:  
    Prepares the binary such that the initialization and destruction routines can operate properly. This includes the allocation of utility memory, the identification of linker symbols, etc. [Details...](./wasm_threadify/general_precautions.md)

1. **A Thread Initialization Routine**:  
    Allocates memory for TLS and thread local stack upon module instantiation such that each thread receives its own space. [Details...](./wasm_threadify/thread_initialization.md)

2. **Thread Local Stack Management**:  
    Makes sure, that references to the stack point to the proper memory locations. [Details...](./wasm_threadify/thread_local_stack.md)

3. **Thread Local Storage (TLS) Management**:  
    Makes sure, that references to the TLS point to the proper memory locations. [Details...](./wasm_threadify/thread_local_storage.md)

4. **A Thread Destruction Routine**:  
    Deallocates memory for TLS and thread local stack. Note, that this function _is not called automatically_. The runtime is responsible to deallocate thread local memory via this function. [Details...](./wasm_threadify/thread_destruction.md)

Furthermore, it provides some general [utility functions](./wasm_threadify/utility_functions.md) to query metadata from WebAssembly modules.

## Important Assumptions and Conventions
Wasm-Threadify builds upon the linker symbols that are emitted by llvm and the memory layout configured by the rust compiler. 

### Linker Symbols
Please read through [this document](https://github.com/WebAssembly/tool-conventions/blob/main/Linking.md#experimental-threading-support) to make yourself familiar with the linking conventions, which the wasm-lld linker of llvm employs. 

The definition of the linker-generated symbols in the llvm source code can be viewed [here](https://github.com/llvm/llvm-project/blob/9cbbb74d370c09e13b8412f21dccb7d2c4afc6a4/lld/wasm/Config.h#L150) at the time of this writing.

Rust specifically configures to export some linker symbols as defined [here](https://github.com/rust-lang/rust/blob/556d20a834126d2d0ac20743b9792b8474d6d03c/compiler/rustc_codegen_ssa/src/back/linker.rs#L1463-L1469) that are crucial for wasm-threadify to work.


### Memory Layout
The wasm-lld linker currently employs the following memory layout for wasm-binaries:

```
-------------------------------------------------------------
|               |                   |                       |
|  Static Data  | <==== Call Stack  |   Heap ==========>    |
|               |                   |                       |
-------------------------------------------------------------
```

View the source code where this is defined [here](https://github.com/llvm/llvm-project/blob/9cbbb74d370c09e13b8412f21dccb7d2c4afc6a4/lld/wasm/Writer.cpp#L329-L341).

However, the Rust compiles calls wasm-lld with the `--stack-first` option, which means the linker will layout memory like this: 

```
-------------------------------------------------------------
|                   |                 |                     |
|  <==== Call Stack |   Static Data   | Heap ==========>    |
|                   |                 |                     |
-------------------------------------------------------------
```

View the source code where this is defined [here](https://github.com/rust-lang/rust/blob/556d20a834126d2d0ac20743b9792b8474d6d03c/compiler/rustc_target/src/spec/base/wasm.rs#L30).