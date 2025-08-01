# Wasmgrind Error
This crate simply defines all error codes that are used inside Wasmgrind along with descriptions for them. Users of the `panic` function of the internal runtime API should use one of the specified error codes to clarify the reason for abortion.

Currently, this library consists of a single file of code:
```Rust
{{#include ../../../crates/wasmgrind-error/src/lib.rs}}
```