# Wasm-Threadlink

Wasm-Threadlink is an essential crate when it comes to compiling programs to WebAssembly for usage with Wasmgrind. It wraps the internal runtime ABI such that it is imported by the WebAssembly binary and provides convenient wrapper functions for thread management and synchronization.

## Thread Management API
The thread management API of Wasm-Threadlink closely follows the threading API of the Rust standard library. Therefore, the internal implementation does also reuse large parts of the standard library threading implementation. This is a breakdown of the most important concepts.

### Internals of Thread Creation
When spawning a thread a closure of type `F` implementing `F: FnOnce() -> T + Send + 'static, T: Send + 'static` is given to the thread spawning routine.

The routine creates an atomically reference-counted data structure, which will serve as a container for the result. It is defined like this:
```Rust
struct ThreadInternals<T> {
    result: UnsafeCell<Option<ThreadResult<T>>>,
}
```

After that it wraps the closure into closure implementing type `FnOnce() + Send + 'static`, i.e., without any return type. This closure receives a reference to the above data structure, simply calls the wrapped closure and stores the result in the given container before returning and thus dropping the reference to it.

The thread creation routine then calls the `thread_create` runtime API function using a raw boxed pointer to the wrapped closure:
```Rust
// variable `main` holds the wrapped closure
let mut thread_id = MaybeUninit::uninit();
let thread = match unsafe {
    wasm_abi::thread_create(
        thread_id.as_mut_ptr(),
        Box::into_raw(Box::new(Work { func: main })) as usize,
    )
} {
    0 => unsafe { thread_id.assume_init() }, // Zero signals successful completion
    code => unsafe { wasm_abi::panic(code) },
};
```

It also defines the `thread_start` function, which simply reconstructs the boxed pointer and calls the closure:
```Rust
#[unsafe(no_mangle)]
pub extern "C" fn thread_start(main: usize) {
    unsafe { (Box::from_raw(main as *mut Work).func)() };
}
```

Lastly, a `JoinHandle` is returned, which can be used to wait for the result of the threads' closure. It wraps the thread identifier and a reference to the container that will hold the result.

### Internals of Thread Joining
Joining of threads can only be performed by using a `JoinHandle`. 

When calling `join()` on the handle, first, the `thread_join` function of the internal runtime ABI is called using the thread identifier:
```Rust
let ret = unsafe { wasm_abi::thread_join(self.native) };
```

The return value is checked for normal behavior before accessing the result held by the container struct of the `JoinHandle`. Note that this should always succeed as the runtime has to ensure that the thread successfully finished before it returns without an error from `thread_join`. Because we constructed the threads' closure, this means that there should be only _one reference_ to the container and a result should _always_ be set at this point.


## Synchronization Primitives
Wasm-Threadlink does provide a custom mutex synchronization primitive that should be used if Wasmgrinds' execution tracing feature is employed.

The `TracingMutex` implementation is built upon the [parking-lot](https://github.com/Amanieu/parking_lot) library, which offers efficient synchronization primitives accompanied by a low-level API to create custom synchronization primitives.

The concept of Wasm-Threadlinks' `TracingMutex` is simple: It wrapps the standard mutex of parking-lot internally to insert callbacks to the internal runtime ABI tracing extension before and after locking or unlocking the mutext respectively. Here is how the internal functions are implemented:

```Rust
use parking_lot::{
    RawMutex,
    lock_api::{self, Mutex},
};

#[cfg(feature = "tracing")]
use crate::wasm_abi;

pub struct TracingRawMutex {
    inner: RawMutex,
}

unsafe impl lock_api::RawMutex for TracingRawMutex {
    
    // ...

    fn lock(&self) {
        #[cfg(feature = "tracing")]
        unsafe {
            wasm_abi::start_lock(self as *const _ as usize)
        };

        self.inner.lock();

        #[cfg(feature = "tracing")]
        unsafe {
            wasm_abi::finish_lock(self as *const _ as usize)
        };
    }

    // ...

    unsafe fn unlock(&self) {
        #[cfg(feature = "tracing")]
        unsafe {
            wasm_abi::start_unlock(self as *const _ as usize)
        };

        unsafe { self.inner.unlock() };

        #[cfg(feature = "tracing")]
        unsafe {
            wasm_abi::finish_unlock(self as *const _ as usize)
        };
    }
}

pub type TracingMutex<T> = Mutex<TracingRawMutex, T>;
```

**Note:** The _tracing_ feature is explicitly required to include those callbacks. This makes it possible to compile the binary without tracing enabled, e.g. if you want to simply run the binary without execution tracing in Wasmgrind.