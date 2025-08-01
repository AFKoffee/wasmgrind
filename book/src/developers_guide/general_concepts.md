# General Concepts
To provide a framework for multithreaded WebAssembly, different challenges have to be addressed.

The first problem is that WebAssembly does not have a threading model in the first place, which makes it the responsibility of Wasmgrind to provide one. We decided to use the _"one instance per thread"_ threading model, which maps each _instance_ of a WebAssembly module to one unique _host thread_. A host thread in this context can be a _WebWorker_ in browsers or an _os-thread_ on native hosts.

Secondly, while WebAssembly is embedder-agnostic, meaning that the instruction set does not make any assumptions about the platform it is running on, in the context of multithreading we have to deal with two fundamentally different environments: _browsers_ and _native hosts_. Because both environments have different ways of utilizing parallelism, we have to account for both of them if we want to make Wasmgrind applicable for both native platforms and the Web.

## Preparing WebAssembly Modules for Multithreading
Modern compilers already provide the fundamental building blocks to support thread-local-storage (TLS) and thread-local stacks - by _stack_ we mean the implicit call stack built by the compiler not the WebAssembly instruction stack. The _llvm compiler backend_ already emits symbols that indicate the start of heap, stack and TLS. Furthermore, it is assured, that memory regions only get initialized once upon instantiating a new WebAssembly module with a single shared memory multiple times. 

However, there is no mechanism that gives each thread a separate TLS and thread-local stack upon initialization. Without this mechanism, instantiating a WebAssembly module multiple times in distinct threads would lead to a situation where every thread uses the _same_ TLS and stack. Therefore, we need to patch the binary before running it with Wasmgrind in order to give each thread a seperate TLS and stack space upon module instantiation. The [wasm-threadify](wasm_threadify.md) crate was built do address this issue.

**Note:** The wasm-threadify crate at this point in time heavily relies on the _linker symbols emitted by llvm_ and _memory layout configured_ by the Rust compiler to prepare the binary for multithreading. Therefore, it is primary targeted at Rust programs that are compiled to WebAssembly. If you want to assess whether Wasmgrind can handle your binary, check out [Chapter 12: Wasm-Threadify](wasm_threadify.md) for in-depth explainations.

## Runtime API
The [Internal Runtime API](./project_structure/the_internal_api.md) is loosely inspired by the POSIX Threads API but is mainly intended to make multithreading accessible from WebAssembly in the first place rather than implementing Pthreads as a whole.

Another important aspect of this API is that it is embedder independent because it relies only on WebAssembly primitives. This was an important design decision as we wanted to decouple Wasmgrind from a specific runtime environment. Having this API enables us to run binaries on any host as long as we implement the API with host specific mechanisms.

## Runtime Environment
Wasmgrind is built upon existing runtimes without changing their inner workings, but by extending their functionality via the public APIs.

On native hosts, we build upon the _wasmtime_ WebAssembly runtime. Generally speaking, Wasmgrind wraps wasmtime and provides the necessary functions to implement the runtime API upon startup using WebAssembly imports. See [Wasmgrind](wasmgrind.md).

In the browser, we utilize the builtin WebAssembly runtime. The multithreading environment can be developed using JavaScript and WebWorkers and provides the necessary functions upon instantiation to implement the runtime API. See [Wasmgrind-JS](wasmgrind_js.md).