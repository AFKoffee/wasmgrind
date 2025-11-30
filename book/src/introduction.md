# Introduction

The _Wasmgrind_ project is a collection of various concepts, tools and libraries which interact deeply with low-level concepts of WebAssembly.

> [!CAUTION]
> Most of the content of this book has been removed during the course of switching to WALI as Wasmgrind's main low-level interface. We may re-add it in the future, but for now this book is just an artifact from the past ...

## Background

In the _User_ and _Developer Guides_ we assume, that you are familiar with WebAssembly and the analysis of concurrent programs. We recommend reading the following chapters first, depending on your background: 
- If you are new to WebAssembly, we suggest you to check out chapters [Chapter 1: What is WebAssembly?]() and [Chapter 2: WebAssembly and Multi-Threading]() before moving on to one of the guides. 
- If you want to catch up with the concepts of concurrency analysis and binary instrumentation used in this project, refer to chapters [Chapter 3: Analysis of Concurrent Programs]() and [Chapter 4: Patching WebAssembly Binaries]().
- In case you are wondering if this project fits your usecase you may want to read through [Chapter 5: Project Goals](), where we describe what Wasmgrind is and what it is not. 

## Guides

The [User Guide]() is targeted at users that want to embed Wasmgrind into their projects as a library or want to use the provided packages as in order to run and analyze multithreaded WebAssembly programs.

If you aim to contribute to Wasmgrind or want to explore the internal concepts of this project the [Developers Guide]() will fit your needs.
