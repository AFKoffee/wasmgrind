# Thread Local Stack
Every thread needs to have a seperate call stack. However, in the design of WebAssembly and contemporary runtimes there is no builtin multithreading support and therefore no mechanism to perform context switches. We need to employ our own mechanism to separate distinct call stacks of concurrent threads.

Spilling and restoring data from the stack is resolved relative to the `stack pointer`, a global variable containing the address pointing to the top of the stack. Global variables are _not_ shared between instances and can therefore be considered thread local. 

As described in the last chapter, we allocate memory on the heap for every thread apart from the first to serve as their call stack. Therefore, during execution the distribution of stacks inside the shared memory may look something like this:

```
-------------------------------------------------------------------------------------------------------------------
|               |                     |   Heap ==========>                                                        |
|  Static Data  | <==== Call Stack    |                                                                           |
|               |    (Main Thread)    | ... | <== stack T1 ==> | <== stack T2 ==> | ... | <== stack T3 ==> | ...  |
-------------------------------------------------------------------------------------------------------------------
```

The main thread uses the compiler-reserved stack space whereas the other threads have their stacks located somewhere in the heap.