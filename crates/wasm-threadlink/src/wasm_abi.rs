#[link(wasm_import_module = "wasm_threadlink")]
unsafe extern "C" {
    /// This function is expected to take a function pointer (as usize) as well as a
    /// MUTABLE pointer to the location where the u32 thread-id should be placed
    /// and run the function in a new thread by calling `thread_start` with it.
    ///
    /// The `thread_start` function is guaranteed to be exported by this module.
    ///
    /// A nonzero return value signals an error during thread creation.
    pub fn thread_create(thread_id: *mut u32, start_routine: usize) -> i32;

    /// This function joins a target thread by supplying its identifier as argument.
    ///
    /// A nonzero return value signals an error during thread joining.
    pub fn thread_join(thread: u32) -> i32;

    /// Callback, which is called whenever a thread tries to aquire a `TracingMutex`.
    #[cfg(feature = "tracing")]
    pub fn start_lock(mutex: usize);

    /// Callback, which is called whenever a thread successfully aquired a `TracingMutex`.
    #[cfg(feature = "tracing")]
    pub fn finish_lock(mutex: usize);

    /// Callback, which is called whenever a thread tries to release a `TracingMutex`.
    #[cfg(feature = "tracing")]
    pub fn start_unlock(mutex: usize);

    /// Callback, which is called whenever a thread successfully released a `TracingMutex`.
    #[cfg(feature = "tracing")]
    pub fn finish_unlock(mutex: usize);

    /// Signals to the runtime that the running program suffered an irrecoverable error
    /// and needs to be aborted.
    ///
    /// It takes a single argument indicating the kind of error.
    ///
    /// This function should NEVER return!
    pub fn panic(errno: i32) -> !;
}
