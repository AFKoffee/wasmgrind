# General Precautions
To make initialization and destruction of threads possible, some preparations have to be made.

## Alloc and Free
Wasm-Threadify needs functions that are able to allocate and deallocate memory in order to work properly. Therefore, it assumes that bindings to an allocator (ideally the global allocator) are accessible via WebAssembly exports:

- `__wasmgrind_malloc`:  
A function that takes the `size` and `alignment` as arguments and returns a pointer to the beginning of the allocated memory with specified size and alignment.
- `__wasmgrind_free`:  
A function that takes a `ptr` to a memory location as well as its `size` and `align` and frees the specified memory region.

## Temporary Stack Space
We have to allocate extra static memory that is used during thread initialization and destruction to ensure two things:

1. We are able to identify whether the thread that is currently being initialized is the first thread or not (because the first thread can use the compiler provided stack and TLS spaces).
2. Threads do not interfere with the stack space of other threads during initialization and destruction. I.e. they need a temporary stack where they can spill values onto while allocating/deallocating their own stack.

Therefore, we allocate one page of memory where the first two _aligned_ i32 words serve as a _thread counter_ and a _lock_ respectively. The rest of the page can be used as a _"temporary stack"_ for threads whenever they have no local stack. The _temporary stack_ is synchronized via the _lock_ mentioned above.

