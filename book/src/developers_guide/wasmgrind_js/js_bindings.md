# JS Bindings for Wasmgrind
The wasmgrind-js utility crate bundles everything that is needed to perform binary instrumentation, execution tracing and thread management in the browser. It uses wasm-bindgen to generate JS bindings to expose the following utilites:
- Patching WebAssembly modules via wasm-threadify
- Querying memory limits of a modules' shared memory
- Instrumenting WebAssembly modules via Wasabi
- Thread management, i.e. getter and setter for thread-id, utilites for thread creation and joining.
- Error descriptions for error codes
- Functions to append specific events to the execution trace
- Retrieving the execution trace in RapidBin format along with its metadata