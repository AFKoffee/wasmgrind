# Wasmgrind

In this book we present _Wasmgrind_, an analysis framework and runtime for multithreaded WebAssembly programs written in Rust.

## What Wasmgrind Is for

With the [threads-proposal](https://github.com/WebAssembly/threads/blob/main/proposals/threads/Overview.md) of WebAssembly being implemented in most modern browser and native runtimes concurrent access of memory regions as well as the implementation of synchronization primitives like mutexes became possible. 

However, at the time of this writing there is no standardized way of creating and joining threads _from within_ WebAssembly. Because of this, its currently quite difficult and error prone to write multithreaded programs in WebAssembly inducing the need for a convenient way to write, run and test multithreaded programs - ideally in a platform independent way.

## The Core Features of Wasmgrind

Wasmgrind tries to fullfill the above needs by providing three mostly independent core utilities.

### Defining an Embedder-Agnostic Runtime API
Defining an API that allows for thread management from within WebAssembly is the foundation for Wasmgrind. It allows for arbitrary projects to be used with Wasmgrind as long as the resulting binaries implement the provided runtime interface.

### Providing Embedder-Specific Runtime Environments
Because the runtime API is embedder-agnostic, various runtimes can be adapted to provide the required functionality to implement the runtime API. For browser engines, the custom runtime environment needs to utilize JavaScript in combination with WebWorkers, whereas for native runtimes, the environment is able to use os-threads.

### Creation of Execution Traces
In order to identify common concurrency bugs like dataraces or deadlocks, Wasmgrind offers the ability to record important events like operations on mutexes, creation and joining of threads as well as memory accesses during program execution for offline analysis. To this end, Wasmgrind uses binary instrumentation to patch the binary before execution.