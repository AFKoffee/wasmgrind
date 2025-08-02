# Web-Targeted Thread Management
Thread management in the browser is based on _WebWorkers_. These are asynchronous in nature and need to be handled differently compared to standard os-threads. The following sections aim to explain the problems and their solutions.

## Thread Creation in the Browser
Unlike os-threads, a web worker can not receive a specific closure upon startup. It rather expects a JavaScript file to run. The sole purpose of this file (`worker.js`) is to set up an event handler such that the parent thread can communicate with its child.

Every WebWorker has its own execution context, which means that objects are not shared across workers but have to be sent to them explicitly. Therefore, the parent thread posts an initialization message to the newly created worker just after startup. This message contains:
- The compiled wasmgrind-js WebAssembly module
- The shared memory for use with the wasmgrind-js WebAssembly module
- The target WebAssembly module, which should be run with Wasmgrind
- The memory module, which is shared between all instances of target WebAssembly modules.
- The thread-id of to be set for this child thread
- A pointer to the closure that this thread should run
- A pointer to a channel struct that is used to send execution context information to the thread (e.g. shared data structures for thread management and exeuction tracing)

Upon receiving the message, the worker instantiates the wasmgrind-js and target WebAssembly module providing implementations of the internal runtime ABI functions and calls `thread_start` with the pointer to the closure.

## Signaling Thread Termination
On native platforms the operating system provides the information whether a thread has finished. Because WebWorker wait for messages rather than simply executing a closure and then exit, we have to implement such a mechanism ourselves.

Currently, a thread signals to be finished by setting a return value of zero in the thread management structure. This is not optimal, but it works for now.

## Thread Joining in the Browser
The thread termination signaling mechanism heavily influences the thread management design with respect to its joining behavior. We can identify two different cases:

1. The parent thread joins the child thread _before_ it has terminated
2. The parent thread joins the child thread _after_ it has terminated.

Currently, the thread management is implemented by using the `ConditionalHandle` from _wasmgrind-core_. When creating a thread, an empty condtitional handle is registered with a new thread-id and pushed into a map of running threads. When a thread is joined, its corresponding ConditionalHandle is should be removed from the thread management and handed to the caller such that he can wait on the result without blocking access to the thread management datastructure. 

If we only had one map of running threads, this would lead to problems if the parent thread joins the child before it terminates because the conditional handle whould be removed from the thread management and there would be no way to signal termination for the child thread anymore. 

Therefore, we emply the following mechanism to manage ConditionalHandles:
- a set for running threads
- one map for terminated threads
- one map for pending joins

The set of running threads holds the IDs of all running threads. 

If a thread signals termination, it is removed from the set of running threads. If there is an entry of pending joins for that thread in the corresponding map, this handle is removed from the map, the result is set for that handle and the pending thread is notified. If there is no entry of pending joins, a new ConditionalHandle is created and inserted into the map of terminated threads.

If a _running_ thread is joined, a new conditional handle is created and inserted into the map of pending joins associated with the _id of the thread to be joined_. A reference to this handle is returned to the caller. If a _terminated_ thread is joined, its conditional handle is removed from the map of terminated threads and returned to the caller.

**Note:** This structure seems suboptimal and can probably be improved but it works for now.