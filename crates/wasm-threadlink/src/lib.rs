pub mod mutex;
pub mod thread;

/// Defines the imports of the Wasmgrind internal runtime ABI in Rust
/// 
/// The lock and unlock hooks will only be available if the `tracing`
/// feature is enabled upon compilation.
mod wasm_abi;
