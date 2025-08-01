# Thread Local Storage
Every thread needs to have their own thread local storage (TLS). The llvm linker for wasm emits four essential symbols to manage TLS:
- `__tls_base`: A global variable that should contain a pointer to the memory address where the TLS is located. Accesses to the TLS are resolved relatively to this pointer.
- `__tls_size`: An immutable global variable indicating the total size of the TLS block in memory
- `__tls_align`: An immutable global variable containing the alignment requirement of the thread local block in bytes.
- `__wasm_init_tls`: A function that expects a pointer argument containing the memory block to use as thread local storage. It will initialize the TLS and set `__tls_base` to the address of this memory block.

So to support multiple threads with their own TLS using the same memory, we have to make sure that each thread initializes their TLS in a distinct memory block. The memory structure when multiple threads are active may then look something like this:

```
---------------------------------------------------------------------------------------------------------------------------
|               |                     |   Heap ==========>                                                                |
|  Static Data  | <==== Call Stack    |                                                                                   |
|               |    (Main Thread)    | ... | <== TLS 0 ==> | <== TLS 1 ==> | ... | <== TLS 2 ==> | ... | <== TLS 3 ==>|  |
---------------------------------------------------------------------------------------------------------------------------
```