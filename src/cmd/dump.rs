use std::path::PathBuf;

use anyhow::Error;

use crate::cmd::{emit_to_file, load_and_instrument};

pub struct DumpCmd {
    pub binary: PathBuf,
}

impl DumpCmd {
    pub fn exec(self) -> Result<(), Error> {
        let mut module = load_and_instrument(&self.binary)?;
        emit_to_file("tmp", &module.emit_wasm(), "instrumented")?;
        Ok(())
    }
}
