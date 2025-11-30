use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use wasmtime::Module;

mod provider;
pub use provider::StandaloneCtxProvider;

pub struct WasmgrindStandaloneCtx {
    module: Module,
    tls_size: u32,
    tls_align: u32,
    next_tid: Arc<AtomicU32>,
}

impl Clone for WasmgrindStandaloneCtx {
    fn clone(&self) -> Self {
        Self {
            module: self.module.clone(),
            tls_size: self.tls_size,
            tls_align: self.tls_align,
            next_tid: self.next_tid.clone(),
        }
    }
}

impl WasmgrindStandaloneCtx {
    const MODULE_NAME: &str = "wasmgrind_standalone";
    const MEMORY_IMPORT_NAME: &str = "memory";
    const MEMORY_IMPORT_MODULE: &str = "env";

    pub fn next_available_tid(&self) -> u32 {
        self.next_tid.fetch_add(1, Ordering::Relaxed)
    }
}
