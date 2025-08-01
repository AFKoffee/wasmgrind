use std::sync::Arc;

use anyhow::Error;
use crossbeam_channel::Receiver;

use crate::tmgmt::SyncedJsTmgmt;

pub struct ThreadlinkComs {
    tmgmt_receiver: Receiver<Arc<SyncedJsTmgmt>>,
}

impl ThreadlinkComs {
    pub fn send(tmgmt: Arc<SyncedJsTmgmt>) -> Result<Self, Error> {
        let (tmgmt_sender, tmgmt_receiver) = crossbeam_channel::unbounded::<Arc<SyncedJsTmgmt>>();

        tmgmt_sender.send(tmgmt)?;

        Ok(Self { tmgmt_receiver })
    }

    pub fn receive(self) -> Result<Arc<SyncedJsTmgmt>, Error> {
        let tmgmt = self.tmgmt_receiver.recv()?;

        Ok(tmgmt)
    }
}
