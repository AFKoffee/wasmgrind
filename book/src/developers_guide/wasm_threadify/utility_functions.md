# Utility Functions
Despite preparing the WebAssembly module for multithreading, the wasm-threadify crate provides some additional utility functions to query metadata from a given binary:

- `get_shared_memory_size`: A function to query the limits of the shared memory of a module.