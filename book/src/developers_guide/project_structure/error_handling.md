# Challenges in Error Handling

Currently, WebAssembly has some caveats when it comes to panics in Rust.

Rust currently compiles with option `panic_abort` by default when targeting WebAssembly. This means that the program should abort upon panicking which leads to an `llvm.trap` instruction being executed in case of a panic. Because llvm traps are translated to `unreachable` instructions in the WebAssembly binary the WebAssembly instance will simply stop program execution with an error upon reaching this program location. With `panic_abort` no cleanup code gets executed possibly leaving the memory in an inconsistent state.

This is a problem in the multithreaded environment where we spin up multiple instances working on the same memory at the same time. If one instance fails because of an `unreachable` instruction being executed, this does not imply that other instances will stop execution, too. Hence, it becomes possible that a running instance interacts with corrupted memory.

Because Wasmgrind currently has no way to handle this case, our recommendation to alleviate this issue is to avoid panics in your code all along or use the internal `panic` API function if you want to signal program abortion.