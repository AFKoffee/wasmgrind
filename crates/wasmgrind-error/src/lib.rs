/// Defines all error codes used throughout Wasmgrind
pub mod errno {
    /// Function returned without error
    /// 
    /// **Note:** Should not be used with `panic` of the internal runtime ABI.
    /// This would make no sense!
    pub const NO_ERROR: i32 = 0;

    /// Internal runtime error: The thread with the requested ID did not exist
    /// 
    /// In the native runtime, this error is emitted if the program requested
    /// to join a thread with an ID that had no os-thread associated with it.
    pub const RT_ERROR_THREAD_NOT_FOUND: i32 = 1;

    /// Internal runtime error: The attempt to join the thread failed
    /// 
    /// In the native runtime, this error is emitted if there was an
    /// error while joining the os-thread backing the wasm-thread.
    pub const RT_ERROR_THREAD_JOIN_FAILURE: i32 = 2;

    /// Internal runtime error: There was a failure inside a runtime function
    /// 
    /// In the native runtime, this error is emitted if there was a failure
    /// while creating the WebAssembly instance inside the os-thread or
    /// while calling the threads closure.
    pub const RT_ERROR_THREAD_RUNTIME_FAILURE: i32 = 3;

    /// Internal runtime error: WebAssembly memory was accessed out of bounds
    /// 
    /// In the native runtime, this error is emitted if the shared memory 
    /// of the running WebAssembly instance would have been accessed outside
    /// of its valid address range.
    /// 
    /// Most likely, this occured because an invalid pointer was given to
    /// the `thread_create` function of the internal runtime ABI.
    pub const RT_ERROR_MEMORY_OUT_OF_BOUNDS_ACCESS: i32 = 4;

    /// Internal runtime error: Could not convert TID pointer to internal type
    /// 
    /// In the native runtime, this error is emitted if the pointer given
    /// as a first argument to the `thread_create` function of the internal 
    /// runtime ABI could not be converted to `usize`.
    /// 
    /// This can, for example, happen if you run 32-bit WebAssembly on a
    /// 16-bit host machine - it is very unlikely that this error ever 
    /// happens.
    pub const RT_ERROR_TID_POINTER_CONVERSION_FAILED: i32 = 5;

    /// Internal runtime error: Thread-Management lock was poisoned
    /// 
    /// In the native runtime, this error is emitted if the mutex
    /// guarding the thread management was poisoned. This can happen 
    /// if another thread panicked while holding the lock on the
    /// thread management.
    pub const RT_ERROR_TMGMT_LOCK_POISONED: i32 = 6;

    /// Library error: Thread was successfully joined, but result was not set
    /// 
    /// This error used to call the `panic` function of the internal runtime
    /// ABI from inside wasm-threadlink if the result originating from the
    /// joined thread was not set despite being successfully joined.
    /// 
    /// **Note:** This error is most likely a library bug.
    pub const LIB_ERROR_NO_RESULT_AFTER_JOIN: i32 = 7;

    /// Library error: Thread was successfully joined, but multiple references to the result existed
    /// 
    /// This error used to call the `panic` function of the internal runtime
    /// ABI from inside wasm-threadlink if there existed multiple references to the result structure
    /// of the joined thread despite being successfully joined.
    /// 
    /// **Note:** This error is most likely a library bug.
    pub const LIB_ERROR_MULTIPLE_REFS_AFTER_JOIN: i32 = 8;
}

/// Returns a string describing the error based on the given error code `errno`
pub fn errno_description(errno: i32) -> String {
    match errno {
        0 => "No Error".into(),
        1 => "Runtime Error: Thread not found".into(),
        2 => "Runtime Error: Thread join failed".into(),
        3 => "Runtime Error: Thread failed in runtime context".into(),
        4 => "Runtime Error: Thread tried to access shared memory out of bounds".into(),
        5 => "Runtime Error: Could not convert u32 to pointer-sized type (usize)".into(),
        6 => "Runtime Error: ThreadManagement lock was poisoned".into(),
        7 => "Library Error: Result was not available after thread join".into(),
        8 => "Library Error: Multiple references to the result after thread join".into(),
        _ => "Unknown error!".into(),
    }
}
