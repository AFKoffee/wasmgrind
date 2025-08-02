# The Internal API

The internal runtime ABI is defined in file: `crates/wasm-threadlink/wasm_abi.rs`. 

**Note:** The functions are subject to change. Furthermore, the API may be separated from the wasm-threadlink library in the future!

## Core Functions

The API contains the following core functions:

```Rust
pub fn thread_create(thread_id: *mut u32, start_routine: usize) -> i32;

pub fn thread_join(thread: u32) -> i32;

pub fn panic(errno: i32) -> !;
```

Furthermore, it is assumed that the binary exposes a function with this signature:

```Rust
pub fn thread_start(start_routine: usize) {
    ...
}
```

### `thread_create(thread_id: *mut u32, start_routine: usize) -> i32`
This function takes the memory address of an `u32` integer and the address of a _parameterless_ function as `usize`. 

It creates a new thread, writes the thread-id to the location specified by `thread_id` and calls the exposed `thread_start` function with `start_routine` as its argument.

The return value is an error code signaling whether an error occurred during thread creation.

### `thread_join(thread: u32) -> i32`
This function takes a thread-id as `u32` and attempts to join this thread.

The return value is an error code signaling whether an error occurred while joining the thread.

### `panic(errno: i32) -> !`
This function takes an `i32` error code signaling a reason to stop program execution. For a list of error codes refer to [wasmgrind-error](../wasmgrind_error.md).

**Note:** The `panic` function should never return but WebAssembly has no way of ensuring this so the panic function will simply have no return type. The implementing runtime is responsible of guaranteeing that the program execution stops after a call to this function.

## Tracing Extension

If you compile wasm-threadlink with tracing, the API is extended with four more functions:

```Rust
pub fn start_lock(mutex: usize);

pub fn finish_lock(mutex: usize);

pub fn start_unlock(mutex: usize);

pub fn finish_unlock(mutex: usize);
```

All of the above functions are callbacks that are expected to be called before and after locking or unlocking a mutex respectively. The argument currently is pointer-sized because wasm-threadlink uses the memory address of the lock as its identifier.

## The WebAssembly Level
These functions have to be present as _imports_ in the WebAssembly binary after compilation. The functions have to be located under the *wasm_threadlink* namespace. 

For example, the `thread_create` function should be defined like this in a WebAssembly binary:

```wasm
(module
    ...
    
    (type (func (param i32 i32) (result i32))) ;; This is the type with index 5
    
    ...

    (import "wasm_threadlink" "thread_create" (func $<internal name> (type 5)))
    
    ...
)
```

The `thread_start` function should be exposed like this:

```wasm
(module
    ...

    (type (func (param i32))) ;; This is the type with index 2
    
    ...
    
    (export "thread_start" (func $thread_start))
    
    ...

    (func $thread_start (type 2) (param i32)
        ;; function code is here
    )
    
    ...
)
```


**Note:** WebAssembly has no notion of a "pointer-sized" type. `usize` and `*mut u32` will be compiled to `i32` or `i64` depending on whether the binary is compiled to 32-bit or 64-bit WebAssembly.

**Important:** Wasmgrind only supports 32-bit WebAssembly at this point in time!

