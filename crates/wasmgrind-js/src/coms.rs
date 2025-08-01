use std::sync::Arc;

use anyhow::Error;
use crossbeam_channel::{Receiver, Sender};
use race_detection::tracing::{BinaryTraceOutput, Tracing};

mod threadlink;
mod wasmgrind;

pub use threadlink::ThreadlinkComs;
pub use wasmgrind::WasmgrindComs;

pub struct TraceGenerationComs {
    tracing_receiver: Receiver<Arc<Tracing>>,
    output_sender: Sender<BinaryTraceOutput>,
}

pub struct TraceOutputReceiver {
    output_receiver: Receiver<BinaryTraceOutput>,
}

impl TraceOutputReceiver {
    pub fn receive(self) -> Result<BinaryTraceOutput, Error> {
        let output = self.output_receiver.recv()?;

        Ok(output)
    }
}

impl TraceGenerationComs {
    pub fn send(tracing: Arc<Tracing>) -> Result<(Self, TraceOutputReceiver), Error> {
        let (tracing_sender, tracing_receiver) = crossbeam_channel::unbounded::<Arc<Tracing>>();
        let (output_sender, output_receiver) = crossbeam_channel::unbounded::<BinaryTraceOutput>();

        tracing_sender.send(tracing)?;

        Ok((
            Self {
                tracing_receiver,
                output_sender,
            },
            TraceOutputReceiver { output_receiver },
        ))
    }

    pub fn receive_and_reply<F: FnOnce(Arc<Tracing>) -> Result<BinaryTraceOutput, Error>>(
        self,
        callback: F,
    ) -> Result<(), Error> {
        let tracing = self.tracing_receiver.recv()?;
        self.output_sender.send(callback(tracing)?)?;

        Ok(())
    }
}
