# WebWorkers and Wasmgrind
Wasmgrind does not only use WebWorkers as containers for the threads created from WebAssembly modules. It also uses them as proxies in two use cases
- Target Function Execution
- Execution Trace Encoding

All use cases of WebWorkers utilize the same `worker.js` file:
```javascript
{{#include ../../../../crates/wasmgrind-js/js/worker.js}}
```

The `handle_message` method is an _internal_ function exported by the wasmgrind-js npm-package and performs different actions based on the message type. It should **never** be used by external libraries or applications.

The different message types and the `handle_message` method are defined in the `crates/wasmgrind-js/src/message.rs` file.

## Target Function Execution
_Target Function Execution_ denotes the execution of a function that is exported by a WebAssembly module. Because the main browser thread must never block, `atomic.wait` instructions can not be executed in this context. As Wasmgrind does not know whether a target binary uses these instructions inside its function, it needs to conduct the function execution in a separate WebWorker and posts a message to the main thread when the function returns.

## Execution Trace Encoding
Wasmgrind needs to acquire a lock on the shared execution trace data structure to iterate over it and encode it into RapidBin format. Because the main browser thread must not block, this operation has to be conducted in a separate WebWorker.