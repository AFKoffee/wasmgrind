use anyhow::{Error, anyhow};
use wasabi::{
    instrument::add_hooks,
    options::{Hook, HookSet},
};

/// Patches a binary WebAssembly module for multithreading support.
/// 
/// The function expects `wasm_bytes` to be a valid WebAssembly module
/// in binary format and returns a modified WebAssembly binary - again in
/// binary format - which has been adapted such that it supports multithreading.
/// 
/// The module is parsed into a [`walrus::Module`] before being passed to
/// [`wasm_threadify::run`].
/// 
/// # Errors
/// 
/// This function may fail in the following cases:
/// - The given buffer `wasm_bytes` could not be parsed by [`walrus`].
/// - [`wasm_threadify`] could not patch this WebAssembly module for multithreading
/// 
/// Refer to [`walrus::Module::from_buffer`] and [`wasm_threadify::run`]
/// for details with regard to the possible errors.
pub fn threadify(wasm_bytes: &[u8]) -> Result<Vec<u8>, Error> {
    Ok(wasm_threadify::run(&mut walrus::Module::from_buffer(wasm_bytes)?)?.emit_wasm())
}

/// Instruments a binary WebAssembly module for execution tracing.
/// 
/// The function expects `wasm_bytes` to be a valid WebAssembly module
/// in binary format and returns a modified WebAssembly binary - again in
/// binary format - which has been instrumented to support execution tracing
/// with Wasmgrind.
/// 
/// The WebAssembly module is parsed and instrumented by a [self-extended
/// Wasabi version](https://github.com/AFKoffee/wasabi.git).
/// 
/// For details with regard to the exact instrumentation performed, refer to the
/// [Wasmgrind Book](https://wasmgrind-a64c5a.gitlab.io/book/developers_guide/wasmgrind_core/wasm_instrumentation.html).
/// 
/// This function may fail in the following cases:
/// - The given buffer `wasm_bytes` could not be parsed by Wasabi.
/// - Wasabi failed to instrument the WebAssembly module.
/// - Wasabi failed to encode the WebAssembly module back to binary format.
pub fn instrument(wasm_bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let mut enabled_hooks = HookSet::new();
    enabled_hooks.insert(Hook::DeadlockDetection);
    // TODO: Wasabi may panic. This is bad in wasm code.
    // Should we wrap it in panic-catch-unwind??
    let (mut module, _, _) = wasabi_wasm::Module::from_bytes(wasm_bytes)?;
    if let Some((_, _)) = add_hooks(&mut module, enabled_hooks, false) {
        module.to_bytes().map_err(Error::from)
    } else {
        Err(anyhow!("Wasabi failed to instrument the wasm binary!"))
    }
}
