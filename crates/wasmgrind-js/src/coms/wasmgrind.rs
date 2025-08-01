use std::sync::Arc;

use anyhow::Error;
use crossbeam_channel::Receiver;
use race_detection::tracing::Tracing;

use crate::tmgmt::SyncedJsTmgmt;

pub struct WasmgrindComs {
    tracing_receiver: Receiver<Arc<Tracing>>,
    tmgmt_receiver: Receiver<Arc<SyncedJsTmgmt>>,
}

impl WasmgrindComs {
    pub fn send(tracing: Arc<Tracing>, tmgmt: Arc<SyncedJsTmgmt>) -> Result<Self, Error> {
        let (tracing_sender, tracing_receiver) = crossbeam_channel::unbounded::<Arc<Tracing>>();
        let (tmgmt_sender, tmgmt_receiver) = crossbeam_channel::unbounded::<Arc<SyncedJsTmgmt>>();

        tracing_sender.send(tracing)?;
        tmgmt_sender.send(tmgmt)?;

        Ok(Self {
            tracing_receiver,
            tmgmt_receiver,
        })
    }

    pub fn receive(self) -> Result<(Arc<Tracing>, Arc<SyncedJsTmgmt>), Error> {
        let tracing = self.tracing_receiver.recv()?;
        let tmgmt = self.tmgmt_receiver.recv()?;

        Ok((tracing, tmgmt))
    }
}
