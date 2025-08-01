# Other Useful Wrappers
Wasm-Core wraps the functions provided by [wasm-threadify](../wasm_threadify.md) such that they operate on byte buffers instead of the internal module representation of the `walrus` WebAssembly transformation library.

Those wrappers are mostly convenience functions but they dampen performance because the require to parse the byte buffer into a `walrus` module on every call. There is definitly room for improvement here.