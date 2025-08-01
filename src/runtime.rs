use std::sync::{Arc, Mutex, RwLock};

use anyhow::Error;
use race_detection::tracing::Tracing;
use wasmtime::Linker;

use crate::tmgmt::ThreadManagement;

mod base;
mod context_provider;
mod tracing;

pub use base::{ThreadlinkRuntime, ThreadlinkRuntimeBuilder};
pub use tracing::{WasmgrindRuntime, WasmgrindRuntimeBuilder};

type SynchronizedLinker = Arc<RwLock<Linker<()>>>;
type Tmgmt = Arc<Mutex<ThreadManagement<Result<(), Error>>>>;
type ArcTracing = Arc<Tracing>;
