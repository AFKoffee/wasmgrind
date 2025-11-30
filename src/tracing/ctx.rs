use std::{path::Path, sync::Arc};

use anyhow::Error;
use wasmgrind_core::tracing::{Tid, Tracing, metadata::WasmgrindTraceMetadata};
use wasmtime::{Caller, Linker};

use crate::tracing::TracingView;

pub struct WasmgrindTracingCtx {
    tracing: Arc<Tracing>,
}

impl Clone for WasmgrindTracingCtx {
    fn clone(&self) -> Self {
        Self {
            tracing: self.tracing.clone(),
        }
    }
}

impl WasmgrindTracingCtx {
    const MODULE_NAME: &str = "wasmgrind_tracing";

    pub fn new<P: AsRef<Path>>(tracing_cache_dir: P) -> Self {
        Self {
            tracing: Arc::new(Tracing::new(tracing_cache_dir)),
        }
    }

    pub fn add_to_linker<T: TracingView + 'static>(linker: &mut Linker<T>) -> Result<(), Error> {
        linker
            .func_wrap(Self::MODULE_NAME, "initialize", |caller: Caller<'_, T>| {
                caller.data().ctx().tracing.initialize();
            })?
            .func_wrap(
                Self::MODULE_NAME,
                "thread_ignore_begin",
                |caller: Caller<'_, T>| {
                    caller.data().ctx().tracing.thread_ignore_begin();
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "thread_ignore_end",
                |caller: Caller<'_, T>| {
                    caller.data().ctx().tracing.thread_ignore_end();
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "thread_create",
                |caller: Caller<'_, T>, child_id: u32, flags: u32, fidx: u32, iidx: u32| -> Tid {
                    caller
                        .data()
                        .ctx()
                        .tracing
                        .thread_create(child_id, flags, (fidx, iidx))
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "thread_register",
                |caller: Caller<'_, T>, thread_id: Tid| {
                    caller.data().ctx().tracing.thread_register(thread_id);
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "thread_consume",
                |caller: Caller<'_, T>, thread_id: u32| -> Tid {
                    caller.data().ctx().tracing.thread_consume(thread_id)
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "thread_join",
                |caller: Caller<'_, T>, child_id: Tid, fidx: u32, iidx: u32| {
                    caller
                        .data()
                        .ctx()
                        .tracing
                        .thread_join(child_id, (fidx, iidx));
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "thread_detach",
                |caller: Caller<'_, T>, child_id: Tid| {
                    caller.data().ctx().tracing.thread_detach(child_id);
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "mutex_register",
                |caller: Caller<'_, T>, lock_id: u32, flags: u32| {
                    caller.data().ctx().tracing.mutex_register(lock_id, flags);
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "mutex_unregister",
                |caller: Caller<'_, T>, lock_id: u32| {
                    caller.data().ctx().tracing.mutex_unregister(lock_id);
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "mutex_start_lock",
                |caller: Caller<'_, T>, lock_id: u32, fidx: u32, iidx: u32| {
                    caller
                        .data()
                        .ctx()
                        .tracing
                        .mutex_start_lock(lock_id, (fidx, iidx));
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "mutex_finish_lock",
                |caller: Caller<'_, T>, lock_id: u32, fidx: u32, iidx: u32| {
                    caller
                        .data()
                        .ctx()
                        .tracing
                        .mutex_finish_lock(lock_id, (fidx, iidx));
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "mutex_unlock",
                |caller: Caller<'_, T>, lock_id: u32, fidx: u32, iidx: u32| {
                    caller
                        .data()
                        .ctx()
                        .tracing
                        .mutex_unlock(lock_id, (fidx, iidx));
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "mutex_repair",
                |caller: Caller<'_, T>, lock_id: u32| {
                    caller.data().ctx().tracing.mutex_repair(lock_id);
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "mutex_invalid_access",
                |caller: Caller<'_, T>, lock_id: u32| {
                    caller.data().ctx().tracing.mutex_invalid_access(lock_id);
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "read_hook",
                |caller: Caller<'_, T>,
                 addr: u32,
                 width: u32,
                 atomic: u32,
                 fidx: u32,
                 iidx: u32| {
                    caller.data().ctx().tracing.memory_access_read(
                        addr,
                        width,
                        atomic,
                        (fidx, iidx),
                    );
                },
            )?
            .func_wrap(
                Self::MODULE_NAME,
                "write_hook",
                |caller: Caller<'_, T>,
                 addr: u32,
                 width: u32,
                 atomic: u32,
                 fidx: u32,
                 iidx: u32| {
                    caller.data().ctx().tracing.memory_access_write(
                        addr,
                        width,
                        atomic,
                        (fidx, iidx),
                    );
                },
            )?;

        Ok(())
    }

    pub fn generate_binary_trace<P: AsRef<Path>>(
        self,
        outfile: P,
    ) -> Result<Result<WasmgrindTraceMetadata, Error>, WasmgrindTracingCtx> {
        match Arc::try_unwrap(self.tracing) {
            Ok(tracing) => Ok(tracing.generate_binary_trace(outfile)),
            Err(arc_tracing) => Err(Self {
                tracing: arc_tracing,
            }),
        }
    }
}
