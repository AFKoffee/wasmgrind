# Using Wasm-Threadlink
The _wasm-threadlink_ Rust library offers an ideomatic way of accessing the internal runtime ABI of Wasmgrind from Rust program. It provides two main utilities: a modified mutex structure, which enables tracing of lock events, and functions for thread creation and joining. Refer to [docs.rs](https://afkoffee.github.io/wasmgrind/wasmgrind-docs-rs/wasmgrind/) for a more detailed API documentation.

**Note:** If the `tracing` feature of the crate is enabled, it wraps the _tracing extended_ internal runtime ABI. Otherwise, it just wraps the _standalone_ internal runtime ABI.

**IMPORTANT:** 
- To compile working WebAssembly modules with wasm-threadlink, a nightly rust toolchain is required and the following target-features have to be active: `atomics`, `bulk-memory` and `mutable-globals`. 
- Furthermore, the compiled crate has to be labelled as `crate-type = ["cdylib", "rlib"]` in `Cargo.toml`.

An example program using wasm-threadlink may look like this:
```Rust
use std::sync::atomic::AtomicU32;

use wasm_threadlink::{mutex::TracingMutex, thread};

static SOME_VARIABLE: TracingMutex<i32> = TracingMutex::new(0);


fn increment_some_variable() {
    *SOME_VARIABLE.lock() += 1;
}


fn reset_some_variable() {
    *SOME_VARIABLE.lock() = 0;
}

#[unsafe(no_mangle)]
pub extern "C" fn example_function() {
    let t1 = thread::thread_spawn(|| {
        for _ in 0..100 {
            increment_some_variable();
        }
    });

    let t2 = thread::thread_spawn(|| {
        reset_some_variable();
    });

    t1.join();

    t2.join();
}
```

It compiles to a WebAssembly binary that exports a single function `example_function`. If the binary is compiled with `tracing` enabled, Wasmgrind has now the ability to trace `thread_spawn`, `thread_join`, `mutex_lock` and `mutex_unlock` events during program runs.