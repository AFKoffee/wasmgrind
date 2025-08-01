# JS Runtime Environment
The JS Runtime Environment consists of three distinct JavaScript files: `wasmgrind.js`, `wgrind_worker.js` and `worker.js`.

## wasmgrind.js
This script exports a single function `wasmgrind` that takes the path to a WebAssembly binary and the name of a parameterless function exported by that binary. When this function is called it performs the following steps:
1. It instantiates the wasmgrind-js utilities WebAssembly module
2. It fetches the specified binary
3. It patches the fetched binary for multithreading
4. It instruments the patched binary
5. It compiles the instrumented binary into a `WebAssembly.Module` object
6. It initalizes a shared memory object with limits extraced from the instrumented binary
7. It instantiates the compiled WebAssembly module by providing implementations for all functions required by the internal runtime API (including lock tracing callbacks)
8. It calls the specified function on the instance that is expected to be exported by the WebAssembly module

## wgrind_worker.js
Because the main thread in browsers is not allowed to block, we can not use wasmgrind.js directly in the main thread of the browser. `wgrind_worker.js` is a worker script that is intended to be used as a proxy for calls to the `wasmgrind` function from the main browser thead. Its implementation is straightforward:
```JavaScript
{{#include ../../../../crates/wasmgrind-js/wgrind_worker.js}}
```

## worker.js
This worker script is reponsible to instantiate WebAssembly modules in all threads apart from the first thread. It expects an initalization message containing the following elements:
- The compiled wasmgrind-js utility library
- The shared memory of the wasmgrind-js utility library
- The compiled WebAssembly module, which contains the code for the thread to be run
- The shared memory module, which is used by all threads
- The thread-id of to be set for this child thread
- A pointer to the closure that this thread should run

Upon receiving that message, it performs the following steps:
1. It instantiates the wasmgrind-js utilities WebAssembly module using the given memory and module objects
2. It sets the tread-id to the specified value
3. It instantiates the compiled WebAssembly module by providing implementations for all functions required by the internal runtime API (same implementation as in `wasmgrind.js`)
4. It calls the `thread_start` function exposed by the instantiated WebAssembly module
5. After `thread_start` returns, it sets the return value of the thread to zero to signal termination
6. It calls `self.close()` such that the worker resources can be freed by the browser.

**Note:** Between steps 5 and 6, `__wbindgen_thread_destroy()` and `__wasmgrind_thread_destroy()` should be called on the wasmgrind-core utilities and target module instances. But this currently leads to errors for reasons we do not quite understand so we accept leaking memory here ...