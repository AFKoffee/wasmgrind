# Thread Destruction
To make thread destruction possible, wasm-threadify injects a new function into the WebAssembly module that is exported under the name `__wasmgrind_thread_destroy`.

This function does take three optional parameters:
- The memory address that points to the TLS to be deallocated
- The memory address that points to the stack to be deallocated
- The size of the stack to be deallocated

This makes it possible for threads to be destroyed from the outside if necessary. If the parameters are not given, the corresponding global variables of the instance are used instead.

Threads are destroyed as follows:

1. Get the values for `__tls_base`, `__tls_size`, `__tls_align`
2. Call `__wasmgrind_free` with those values as arguments
3. Set `__tls_base` global variable to the maximum 32-bit address to trigger invalid memory on future accesses
4. Get the values for `__stack_ptr` and `stack size`
5. Call `__wasmgrind_free` with those values as arguments (stack is 16-byte aligned)
6. Set `__stack_ptr` to zero so future accesses trigger invalid memory