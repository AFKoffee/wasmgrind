mod threadlink;
mod wasmgrind;

use anyhow::Error;
use race_detection::tracing::BinaryTraceOutput;
pub use threadlink::ThreadlinkRuntime;
use wasm_bindgen::prelude::wasm_bindgen;
pub use wasmgrind::WasmgrindRuntime;

/// A struct to expose the result of execution tracing to JavaScript environments.
#[allow(dead_code)]
#[wasm_bindgen]
pub struct TraceOutput {
    trace: Vec<u8>,
    metadata: String,
}

#[wasm_bindgen]
impl TraceOutput {
    /// Retrieves an owned version of the binary trace.
    #[wasm_bindgen(getter)]
    pub fn trace(&self) -> Vec<u8> {
        self.trace.clone()
    }

    /// Retrieves an owned version of the trace metadata.
    #[wasm_bindgen(getter)]
    pub fn metadata(&self) -> String {
        self.metadata.clone()
    }
}

impl TryFrom<BinaryTraceOutput> for TraceOutput {
    type Error = Error;

    fn try_from(value: BinaryTraceOutput) -> Result<Self, Self::Error> {
        Ok(Self {
            trace: value.trace,
            metadata: value.metadata.to_json()?,
        })
    }
}
