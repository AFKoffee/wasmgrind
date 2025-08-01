use anyhow::Error;

/// Utilities to patch WebAssembly modules
pub mod patching;

/// Utilities for Wasmgrinds' internal thread management
pub mod tmgmt;

/// Retrieves the memory limits of a binary WebAssembly module
/// 
/// The function expects `wasm_bytes` to be a valid WebAssembly module
/// in binary format that imports a _single shared memory_ and retrieves
/// the limits of that memory.
/// 
/// The module is parsed into a [`walrus::Module`] before being passed to
/// [`wasm_threadify::get_shared_memory_size`].
/// 
/// The function returns a tuple of memory limits: `(min, max)`.
/// 
/// # Errors
/// 
/// This function may fail in the following cases:
/// - The given buffer `wasm_bytes` could not be parsed by [`walrus`].
/// - [`wasm_threadify`] could not determine the memory limits of the module
/// 
/// Refer to [`walrus::Module::from_buffer`] and [`wasm_threadify::get_shared_memory_size`]
/// for details with regard to the possible errors.
pub fn get_memory_limits(wasm_bytes: &[u8]) -> Result<(u32, u32), Error> {
    wasm_threadify::get_shared_memory_size(&walrus::Module::from_buffer(wasm_bytes)?)
}
