use std::{
    fs::File,
    io::{Seek, SeekFrom, Write},
    sync::Arc,
};

use anyhow::Error;
use bitcode::Buffer;

use crate::tracing::trace::{
    EventRecord,
    registry::{CacheFile, TraceRegistry},
};

pub struct TlsTrace {
    capacity: usize,
    buffer: Vec<EventRecord>,
    metadata: Arc<CacheFile>,
    bitcode: Buffer,
    n_written: u64,
    n_buffers: u64,
    file: File,
    is_sealed: bool,
}

impl Drop for TlsTrace {
    fn drop(&mut self) {
        // Try to finalize the thread-local trace trace on disk.
        // We can't handle the error here anyways so we ignore it
        let _ = self.flush().and_then(|_| self.seal());
    }
}

impl TlsTrace {
    const RECORD_SIZE: usize = std::mem::size_of::<EventRecord>();
    const TLS_CAPACITY: usize = 64 * 1024 * 1024 / Self::RECORD_SIZE; // 64 MiB

    pub fn new(thread_id: u32, registry: &TraceRegistry) -> Result<Self, Error> {
        // Request a new cache file from the registry
        let metadata = registry.request_cache_file(thread_id);
        let mut file = File::create(metadata.path())?;

        // Set the header to zero
        let n_buffers: u64 = 0;
        file.write_all(&n_buffers.to_le_bytes())?;

        Ok(Self {
            capacity: Self::TLS_CAPACITY,
            buffer: Vec::with_capacity(Self::TLS_CAPACITY),
            metadata,
            bitcode: Buffer::new(),
            n_written: 0,
            n_buffers,
            file,
            is_sealed: false,
        })
    }

    pub fn seal(&mut self) -> Result<(), Error> {
        if !self.is_sealed {
            self.file.seek(SeekFrom::Start(0))?;
            self.file.write_all(&self.n_buffers.to_le_bytes())?;

            self.is_sealed = true;
        }

        Ok(())
    }

    fn maybe_swap_cache_file(
        &mut self,
        thread_id: u32,
        registry: &TraceRegistry,
    ) -> Result<(), Error> {
        if self.n_written >= self.metadata.target_size() || self.is_sealed {
            // Seal the file, i.e., write self.n_buffers to the header
            self.seal()?;

            // Retrieve a new cache file from the registry and open it
            let new_file = registry.request_cache_file(thread_id);
            self.file = File::create(new_file.path())?;

            // Set the header to zero
            self.n_buffers = 0;
            self.file.write_all(&self.n_buffers.to_le_bytes())?;

            // Reset state
            self.metadata = new_file;
            self.n_written = 0;
            self.is_sealed = false;
        }

        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), Error> {
        if !self.buffer.is_empty() {
            // Serialize data and determine length on disk
            let binary_data = self.bitcode.encode(&self.buffer);
            let data_len = binary_data.len() as u64; // If usize -> u64 overflows, our disk explodes anyways

            // Write the data to disk and clear the in-memory buffer
            self.file.write_all(&data_len.to_le_bytes())?;
            self.file.write_all(binary_data)?;
            self.buffer.clear();

            // Update state (relevant for `maybe_swap_cache_file()`)
            self.n_written += data_len;
            self.n_buffers += 1;
        }

        Ok(())
    }

    pub fn append(&mut self, record: EventRecord, registry: &TraceRegistry) -> Result<(), Error> {
        self.maybe_swap_cache_file(record.event.t, registry)?;

        self.buffer.push(record);

        if self.buffer.len() >= self.capacity {
            self.flush()?;
        }

        Ok(())
    }
}
