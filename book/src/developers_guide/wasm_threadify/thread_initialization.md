# Thread Initialization
To initialize the thread local memory regions upon instantiation of the WebAssembly module, wasm-threadify injects custom functionality into the `start` function of the module.

It basically performs the following steps:

1. If the module had a start function set previously, call this start function first.
2. Check if we are the first thread being initialized
    - If we are: Do nothing. We can use the compiler provided TLS and stack spaces.
    - If we are not:
        - Lock the temporary stack
        - Set the stack pointer to point to the temporary stack
        - Call `__wasmgrind_malloc` to get memory for our thread local stack
        - Unlock the temporary stack
        - Set the stack pointer to the upper end of the newly allocated memory.
3. Allocate the thread-local storage using the linker-emitted symbols and utility functions (i.e. `__wasm_init_tls`, `__tls_size`, `__tls_align` and `__tls_base`)  
**Note:** This involves a second call to `__wasmgrind_malloc` (just without the temporary stack) because we need a fresh memory region where `__tls_base` can point to.
4. Replace the previous start function (if there was one) with the newly created function.   