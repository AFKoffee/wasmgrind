# Alloc Exposer

Exposing memory management functionality for [`wasm-threadify`].
 
The sole purpose of this crate is to expose global allocator functions such that
[`wasm-threadify`] can insert allocation code for thread-local storage and thread-local
stacks while patching the WebAssembly binary. 

While it can be compiled on platforms other than WebAssembly, this crate is intended to
be used only in binaries that will be compiled to WebAssembly and processed by 
[`wasm-threadify`] afterwards.

[`wasm-threadify`]: https://wasmgrind-a64c5a.gitlab.io/docs/wasm_threadify/index.html

## Third Party Materials
The following files in this directory (including its subdirectories) contain code by
other autors. See their respective license headers for more details:
- src/lib.rs, src/link.rs: Copyright (c) 2014 Alex Crichton