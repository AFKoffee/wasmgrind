# Runtime Contexts
A runtime context in Wasmgrind provides a collection of elements that should be given to the WebAssembly module upon instantiation.

Its main purpose is to provide specific implementations of `thread_create` and `thread_join` such that the tracing runtime can reuse the multithreading environment of the standalone runtime without having to worry about its inner workings.

The design is as follows:
- The standalone runtime builder requires an argument of type `T` where `T: ContextProvider` upon initialization.
- The `ContextProvider` requires the implementation three methods
    - one to generate a `thread_create` closure provided the first five arguments of the [`thread_create_func!`](../wasmgrind_macros.md#the-thread_create_func-macro) macro.
    - one to generate a `thread_join` closure provided the first argument of the [`thread_join_func!`](../wasmgrind_macros.md#the-thread_join_func-macro) macro.
    - one function that receives a mutable reference to the `wasmtime::Linker` of the runtime such that the context provider can add some additional functionality, which does not need access to the runtime internals.
- The runtime builder uses the provided functions to register necessary imports _during initalization_. This has the advantage that runtime-required imports are **always** already registered before the user is able to provide custom imports. This prevents users from being able to overwrite internals, for example, replace the implementation of `thread_create` with their own one.

Employing this design, a tracing runtime builder can easily be built by wrapping the standalone runtime builder but providing a different implementation of `ContextProvider` that satisfies the import requirements of the tracing runtime.
