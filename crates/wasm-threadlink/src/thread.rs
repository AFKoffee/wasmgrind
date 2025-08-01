//! WebAssembly Threads
//! 
//! This module provides an API to spawn and join threads from within
//! WebAssembly. It aims to imitate the behavior of the Rust standard
//! library but with the functionality reduced to a minimum.
//! 
//! ## The threading model
//! 
//! This module wraps the threading-related functions of the Wasmgrind internal
//! runtime ABI and therefore follows Wasmgrinds threading model.
//! 
//! An executing program consists of a collection of Wasmgrind managed threads
//! each with their own stack and local state. Moreover, each thread is backed
//! by its own WebAssembly instance while sharing the memory address space
//! with other threads of the running program. Currently, this comes with some 
//! caveats to look out for:
//! - **Threads can outlive the "main" thread:** WebAssembly does not implement
//!   concept of a "main" function but rather offers functionality via
//!   exports - more like a library rather than an executable. Wasmgrind 
//!   does currently not enforce termination of all child threads created
//!   during an invocation of a function export. Therefore, these threads
//!   may run until Wasmgrind itself terminates causing unexpected side effects.
//! - **Panics result in immediate aborts in WebAssembly:**
//!   This means, that there is currently no way to catch panics in WebAssembly.
//!   Therefore, If any thread panics, the backing WebAssembly instance will report 
//!   an error and terminate, possibly leaving the memory in an inconsistent state. 
//!   **BUT** instances of other threads may stay intact and continue to operate
//!   on this memory possibly leading to unexpected behavior.
//! 
//! ## Spawning a thread
//! 
//! A new thread can be spawned using the [`thread::thread_spawn`][`thread_spawn`] function:
//! 
//! ```no_run
//! use wasm_threadlink::thread;
//! 
//! thread::thread_spawn(move || {
//!     // some work here
//! });
//! ```
//! 
//! In this example, the spawned thread is "detached," which means that there is
//! no way for the program to learn when the spawned thread completes or otherwise
//! terminates.
//! 
//! To learn when a thread completes, it is necessary to capture the [`JoinHandle`]
//! object that is returned by the call to [`thread_spawn`], which provides
//! a `join` method that allows the caller to wait for the completion of the
//! spawned thread:
//!
//! ```no_run
//! use wasm_threadlink::thread;
//!
//! let thread_join_handle = thread::thread_spawn(move || {
//!     // some work here
//! });
//! // some work here
//! let res = thread_join_handle.join();
//! ```
//!
//! The [`join`][`JoinHandle::join`] method returns a [`Result`] containing [`Ok`] of the final
//! value produced by the spawned thread, or [`Err`] if an irrecoverable error
//! happened during thread execution.
//! 
//! ## Thread-local storage
//! 
//! Thread-local storage works out-of-the-box as provided by the Rust standard library through
//! the [`thread_local!`] macro.
//! 
//! ## Message passing
//! 
//! The Rust standard library recommends using [`channels`] to pass messages between threads.
//! However, the implementation of those components needs [`std::time`] being available, which
//! is not the case for the plain WebAssembly (`wasm32-unknown-unknown`) target.
//! 
//! Be aware of the specific target you are compiling for in order to assess whether channels
//! can be used in your cased or not.
//! 
//! If channels are not available to you, communication has to happen via standard synchronization
//! primitives like [`std::sync::Mutex`]. These structs work out-of-the-box with WebAssembly, if
//! you compile with the nightly `atomics` feature of Rust being enabled.
//! 
//! [`channels`]: std::sync::mpsc

/*
* The code in this file is mainly based on and taken from the Rust standard library:
* https://github.com/rust-lang/rust/blob/e27f16a499074ba9a87f7f7641d9f64c572863bc/library/std/src/thread/mod.rs
* 
* Copyright (c) The Rust Project Contributors
* 
* Permission is hereby granted, free of charge, to any
* person obtaining a copy of this software and associated
* documentation files (the "Software"), to deal in the
* Software without restriction, including without
* limitation the rights to use, copy, modify, merge,
* publish, distribute, sublicense, and/or sell copies of
* the Software, and to permit persons to whom the Software
* is furnished to do so, subject to the following
* conditions:
* 
* The above copyright notice and this permission notice
* shall be included in all copies or substantial portions
* of the Software.
* 
* THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
* ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
* TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
* PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
* SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
* CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
* OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
* IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
* DEALINGS IN THE SOFTWARE.
*/

use std::{any::Any, cell::UnsafeCell, mem::MaybeUninit, panic, sync::Arc};

use crate::wasm_abi;
use wasmgrind_error::errno;

/// An owned permission to join on a thread (block on its termination).
/// 
/// A `JoinHandle` *detaches* the associated thread when it is dropped, which means
/// that there is no longer any handle to the thread and no way to `join` on it.
/// 
/// This `struct` is created by the [`thread::thread_spawn`] function.
/// 
/// # Examples
/// 
/// Creating a thread:
/// 
/// ```no_run
/// use wasm_threadlink::thread;
///
/// let thread_join_handle = thread::thread_spawn(move || {
///     // some work here
/// });
/// ```
/// 
/// Creating a thread and detaching it:
/// 
/// ```no_run
/// use wasm_threadlink::thread;
///
/// thread::thread_spawn(move || {
///     // thread code
/// });
/// // Spawning thread performs more work here
/// ```
/// 
/// [`thread::thread_spawn`]: thread_spawn
pub struct JoinHandle<T> {
    native: u32,
    internals: Arc<ThreadInternals<T>>,
}

impl<T> JoinHandle<T> {

    /// Waits for the associated thread to finish
    /// 
    /// This function will return immediately if the associated thread has already finished.
    /// 
    /// In terms of [atomic memory orderings], the completion of the associated
    /// thread synchronizes with this function returning. In other words, all
    /// operations performed by that thread [happen
    /// before](https://doc.rust-lang.org/nomicon/atomics.html#data-accesses) all
    /// operations that happen after `join` returns.
    /// 
    /// [atomic memory orderings]: std::sync::atomic
    /// 
    /// # Panics
    /// 
    /// This function may panic if the embedder emits an error that happened while
    /// joining the thread.
    /// 
    /// **Note:** In this case the method does not issue a Rust panic but calls 
    /// the `panic` function of Wasmgrinds internal runtime ABI to signal program abortion.
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// use wasm_threadlink::thread;
    ///
    /// let thread_join_handle = thread::thread_spawn(move || {
    ///     // thread code
    /// });
    /// let res = thread_join_handle.join();
    /// match res {
    ///     Ok(value) => (), // do something with result here
    ///     Err(e) => (), // handle error here
    /// }
    /// ```
    pub fn join(mut self) -> Result<T, Box<dyn Any + Send + 'static>> {
        let ret = unsafe { wasm_abi::thread_join(self.native) };
        if errno::NO_ERROR != ret {
            unsafe { wasm_abi::panic(ret) };
        }

        // Both cases below, where this function may panic are considered internal bugs.
        //
        // This can only happen if the runtime reports a successful join of the thread but 
        // the resources of the thread have not yet been properly freed. However, this should
        // be guaranteed by the implementation of the internal runtime ABI.
        //
        // Therefore, either the runtime falsely reported a successful join or there
        // is an implementation mistake inside of the thread creation routine of this library.
        if let Some(internals_mut) = Arc::get_mut(&mut self.internals) {
            if let Some(result) = internals_mut.take_result() {
                result
            } else {
                unsafe { wasm_abi::panic(errno::LIB_ERROR_NO_RESULT_AFTER_JOIN) };
            }
        } else {
            unsafe { wasm_abi::panic(errno::LIB_ERROR_MULTIPLE_REFS_AFTER_JOIN) };
        }
    }
}

type ThreadResult<T> = Result<T, Box<dyn Any + Send>>;

struct ThreadInternals<T> {
    result: UnsafeCell<Option<ThreadResult<T>>>,
}

// SAFTETY: Same reasoning as in the Rust std library:
//
// Due to the usage of `UnsafeCell` we need to manually implement Sync.
// The type `T` should already always be Send (otherwise the thread could not
// have been created) and the ThreadInternals struct is Sync because all access to the
// `UnsafeCell` is synchronized (by the `join()` boundary).
unsafe impl<T: Send> Sync for ThreadInternals<T> {}

impl<T> ThreadInternals<T> {
    fn new() -> Self {
        Self {
            result: UnsafeCell::new(None),
        }
    }

    // SAFETY: Callers of this function have to ensure, that no other
    // function accesses the result attribute at the same time
    // (i.e., when using the structs' API this means calling this function from another thread)
    unsafe fn set_result(&self, result: ThreadResult<T>) {
        unsafe { *self.result.get() = Some(result) }
    }

    fn take_result(&mut self) -> Option<ThreadResult<T>> {
        self.result.get_mut().take()
    }
}

/// Internal wrapper of the closure passed to a thread
/// 
/// This structure serves as a helper to convert the
/// boxed closure into a _thin pointer_ upon passing
/// it to the embedder.
/// 
/// A _fat pointer_ would take up two `usize` to identify the
/// object it points to: its address and its size. However,
/// we need to pass a single `usize` to the Wasmgrind
/// internal runtime ABI so we need a thin pointer.
struct Work {
    func: Box<dyn FnOnce() + Send + 'static>,
}

/// Spawns a new thread, returning a [`JoinHandle`] for it.
/// 
/// The join handle provides a [`join`] function that can be used
/// to join the spawned thread.
/// 
/// If the join handle is dropped, the spawned thread will implicitly be
/// *detached*. In this case, the spawned thread may no longer be joined.
/// 
/// As you can see in the signature of `spawn` there are two constraints on
/// both the closure given to `spawn` and its return value, let's explain them:
///
/// - The `'static` constraint means that the closure and its return value
///   must have a lifetime of the whole program execution. The reason for this
///   is that threads can outlive the lifetime they have been created in.
///
///   Indeed if the thread, and by extension its return value, can outlive their
///   caller, we need to make sure that they will be valid afterwards, and since
///   we *can't* know when it will return we need to have them valid as long as
///   possible, that is until the end of the program, hence the `'static`
///   lifetime.
/// - The [`Send`] constraint is because the closure will need to be passed
///   *by value* from the thread where it is spawned to the new thread. Its
///   return value will need to be passed from the new thread to the thread
///   where it is `join`ed.
///   As a reminder, the [`Send`] marker trait expresses that it is safe to be
///   passed from thread to thread. [`Sync`] expresses that it is safe to have a
///   reference be passed from thread to thread.
/// 
/// # Panics
/// 
/// Panics if the embedder fails to create a thread.
/// 
/// **Note:** In this case the method does not issue a Rust panic but calls 
/// the `panic` function of Wasmgrinds internal runtime ABI to signal program abortion.
/// 
/// # Examples
/// 
/// Creating a thread:
/// 
/// ```no_run
/// use wasm_threadlink::thread;
///
/// let thread_join_handle = thread::thread_spawn(move || {
///     // thread code
/// });
/// let res = thread_join_handle.join();
/// // handle the result here ...
/// ```
/// 
/// [`join`]: JoinHandle::join
pub fn thread_spawn<F: FnOnce() -> T + Send + 'static, T: Send + 'static>(f: F) -> JoinHandle<T> {
    alloc_exposer::link_mem_intrinsics();

    let read_internals = Arc::new(ThreadInternals::new());
    let write_internals = read_internals.clone();

    let main = move || {
        // FIXME: This is probably unnecessary because panics in wasm code do not unwind anyway ...
        let try_result = panic::catch_unwind(panic::AssertUnwindSafe(f));

        // SAFETY: `write_internals` has been defined just above and moved by the closure (being an Arc<...>).
        // `read_internals` is only given to the returned JoinHandle, which only allows accessing its data
        // after the thread has terminated. Therefore, this mutation is safe.
        unsafe {
            write_internals.set_result(try_result);
        }

        // Finish thread operation
        drop(write_internals); // We drop explicitly here to emphasize whats going on ...
    };

    let main = Box::new(main);
    // SAFETY: dynamic size and alignment of the Box remain the same. The lifetime change is
    // justified, because the closure is passed over a ffi-boundary (in this case to the 
    // WebAssembly embedder, which may operate in Rust, JavaScript, etc.), where there 
    // is no way to enforce lifetimes of the closure.
    //
    // The caller of this function has to ensure, that the thread will not outlive any variables
    // bound by the closure (or the reference to the closure itself). This is enforced statically
    // by the 'static trait bound of the public `thread_spawn()` function.
    //
    // The thread execution mechanism inside the embedder has to ensure, that there are no
    // references to the closure after the thread has terminated (when `join()` returns).
    let main =
        unsafe { Box::from_raw(Box::into_raw(main) as *mut (dyn FnOnce() + Send + 'static)) };

    let mut thread_id = MaybeUninit::uninit();
    let start_routine = Box::into_raw(Box::new(Work { func: main }));
    let thread = match unsafe {
        wasm_abi::thread_create(
            thread_id.as_mut_ptr(),
            start_routine as usize,
        )
    } {
        errno::NO_ERROR => unsafe { thread_id.assume_init() },
        code => {
            drop(unsafe { Box::from_raw(start_routine) });
            unsafe { wasm_abi::panic(code) }
        },
    };

    #[unsafe(no_mangle)]
    pub extern "C" fn thread_start(main: usize) {
        unsafe { (Box::from_raw(main as *mut Work).func)() };
    }

    JoinHandle {
        native: thread,
        internals: read_internals,
    }
}
