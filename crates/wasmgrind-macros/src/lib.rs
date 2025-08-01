//! **ATTENTION:** This crate is tightly coupled with the `wasmgrind` crate. 
//! Any standalone usage is discouraged!

use proc_macro::TokenStream;

mod thread_create;
mod thread_join;

/// Creates a Rust closure, which implements the `thread_create` function
/// 
/// This macro accepts 5 required and 1 optional arguments:
/// 1. a [`wasmtime::Engine`](https://docs.rs/wasmtime/33.0.0/wasmtime/struct.Engine.html)
/// 2. a [`wasmtime::Module`](https://docs.rs/wasmtime/33.0.0/wasmtime/struct.Module.html)
/// 3. a [`wasmtime::SharedMemory`](https://docs.rs/wasmtime/33.0.0/wasmtime/struct.SharedMemory.html)
/// 4. a [`wasmtime::Linker`](https://docs.rs/wasmtime/33.0.0/wasmtime/struct.Linker.html)
/// 5. a `wasmtime::runtime::Tmgmt` (internal thread management of wasmgrind)
/// 6. Optional: a [`race_detection::Tracing`](https://wasmgrind-d6f2b1.gitlab.io/docs/race_detection/struct.Tracing.html)
///    wrapped in a [`std::sync::Arc`].
/// 
/// The returned Rust closure implements the `thread_create` function of the internal runtime ABI.
/// If the 6th argument was given, the _tracing-extended_ version of the function is emitted.
/// Otherwise, it emits the _standalone_ version of the function.
#[proc_macro]
pub fn thread_create_func(input: TokenStream) -> TokenStream {
    thread_create::thread_create_func_(input)
}

/// Creates a Rust closure, which implements the `join_function` function
/// 
/// This macro accepts 1 required and 1 optional argument:
/// 1. a `wasmtime::runtime::Tmgmt` (internal thread management of wasmgrind)
/// 2. Optional: a [`race_detection::Tracing`](https://wasmgrind-d6f2b1.gitlab.io/docs/race_detection/struct.Tracing.html) 
///    wrapped in a [`std::sync::Arc`].
/// 
/// The returned Rust closure implements the `thread_create` function of the internal runtime ABI.
/// If the 2nd argument was given, the _tracing-extended_ version of the function is emitted.
/// Otherwise, it emits the _standalone_ version of the function.
#[proc_macro]
pub fn thread_join_func(input: TokenStream) -> TokenStream {
    thread_join::thread_join_func_(input)
}
