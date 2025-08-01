use anyhow::Error;
use race_detection::tracing::Op;
use wasmgrind_macros::{thread_create_func, thread_join_func};
use wasmtime::{Engine, IntoFunc, Linker, Module, SharedMemory};

use crate::runtime::{ArcTracing, SynchronizedLinker, Tmgmt};

pub trait ContextProvider<
    ThreadCreateParams,
    ThreadCreateResults,
    ThreadJoinParams,
    ThreadJoinResults,
>
{
    fn get_thread_create_func(
        &self,
        engine: Engine,
        module: Module,
        memory: SharedMemory,
        linker: SynchronizedLinker,
        tmgmt: Tmgmt,
    ) -> impl IntoFunc<(), ThreadCreateParams, ThreadCreateResults>;

    fn get_thread_join_func(
        &self,
        tmgmt: Tmgmt,
    ) -> impl IntoFunc<(), ThreadJoinParams, ThreadJoinResults>;

    fn finalize(&self, linker: &mut Linker<()>) -> Result<(), Error>;
}

pub struct DefaultContextProvider {}

impl DefaultContextProvider {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for DefaultContextProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextProvider<(u32, u32), i32, u32, i32> for DefaultContextProvider {
    fn get_thread_create_func(
        &self,
        engine: Engine,
        module: Module,
        memory: SharedMemory,
        linker: SynchronizedLinker,
        tmgmt: Tmgmt,
    ) -> impl IntoFunc<(), (u32, u32), i32> {
        thread_create_func! {
            engine: engine,
            module: module,
            memory: memory,
            linker: linker,
            tmgmt: tmgmt
        }
    }

    fn get_thread_join_func(&self, tmgmt: Tmgmt) -> impl IntoFunc<(), u32, i32> {
        thread_join_func! {
            tmgmt: tmgmt
        }
    }

    fn finalize(&self, linker: &mut Linker<()>) -> Result<(), Error> {
        linker.func_wrap("wasm_threadlink", "panic", |error_code: i32| {
            panic!("{}", wasmgrind_error::errno_description(error_code));

            // We need this here to make the type checker happy.
            // A non returning function does not implement any wasm-type
            // as expected by wasmtime, so we have to return something
            // although we know this function will not resume execution.
            #[allow(unreachable_code)]
            ()
        })?;

        Ok(())
    }
}

pub struct TracingContextProvider {
    inner: DefaultContextProvider,
    tracing: ArcTracing,
}

impl TracingContextProvider {
    pub fn new(tracing: ArcTracing) -> Self {
        Self {
            inner: DefaultContextProvider::new(),
            tracing,
        }
    }

    fn get_start_lock_func(tracing: ArcTracing) -> impl Fn(u32, u32, u32) {
        move |lock_id: u32, fidx: u32, iidx: u32| {
            tracing
                .add_event(
                    wasmgrind_core::tmgmt::thread_id().expect("Thread-ID should be accessible"),
                    Op::Request { lock: lock_id },
                    (fidx, iidx),
                )
                .expect("Error while adding event to tracing");
        }
    }

    fn get_finish_lock_func(tracing: ArcTracing) -> impl Fn(u32, u32, u32) {
        move |lock_id: u32, fidx: u32, iidx: u32| {
            tracing
                .add_event(
                    wasmgrind_core::tmgmt::thread_id().expect("Thread-ID should be accessible"),
                    Op::Aquire { lock: lock_id },
                    (fidx, iidx),
                )
                .expect("Error while adding event to tracing");
        }
    }

    fn get_start_unlock_func(tracing: ArcTracing) -> impl Fn(u32, u32, u32) {
        move |lock_id: u32, fidx: u32, iidx: u32| {
            tracing
                .add_event(
                    wasmgrind_core::tmgmt::thread_id().expect("Thread-ID should be accessible"),
                    Op::Release { lock: lock_id },
                    (fidx, iidx),
                )
                .expect("Error while adding event to tracing");
        }
    }

    fn get_read_hook_func(tracing: ArcTracing) -> impl Fn(u32, u32, u32, u32) {
        move |addr: u32, align: u32, fidx: u32, iidx: u32| {
            tracing
                .add_event(
                    wasmgrind_core::tmgmt::thread_id().expect("Thread-ID should be accessible"),
                    Op::Read { addr, n: align },
                    (fidx, iidx),
                )
                .expect("Error while adding event to tracing");
        }
    }

    fn get_write_hook_func(tracing: ArcTracing) -> impl Fn(u32, u32, u32, u32) {
        move |addr: u32, align: u32, fidx: u32, iidx: u32| {
            tracing
                .add_event(
                    wasmgrind_core::tmgmt::thread_id().expect("Thread-ID should be accessible"),
                    Op::Write { addr, n: align },
                    (fidx, iidx),
                )
                .expect("Error while adding event to tracing");
        }
    }
}

impl ContextProvider<(u32, u32, u32, u32), i32, (u32, u32, u32), i32> for TracingContextProvider {
    fn get_thread_create_func(
        &self,
        engine: Engine,
        module: Module,
        memory: SharedMemory,
        linker: SynchronizedLinker,
        tmgmt: Tmgmt,
    ) -> impl IntoFunc<(), (u32, u32, u32, u32), i32> {
        thread_create_func! {
            engine: engine,
            module: module,
            memory: memory,
            linker: linker,
            tmgmt: tmgmt,
            tracing: self.tracing.clone()
        }
    }

    fn get_thread_join_func(&self, tmgmt: Tmgmt) -> impl IntoFunc<(), (u32, u32, u32), i32> {
        thread_join_func! {
            tmgmt: tmgmt,
            tracing: self.tracing.clone()
        }
    }

    fn finalize(&self, linker: &mut Linker<()>) -> Result<(), Error> {
        self.inner.finalize(linker)?;

        linker.func_wrap(
            "wasm_threadlink",
            "start_lock",
            Self::get_start_lock_func(self.tracing.clone()),
        )?;
        linker.func_wrap(
            "wasm_threadlink",
            "finish_lock",
            Self::get_finish_lock_func(self.tracing.clone()),
        )?;
        linker.func_wrap(
            "wasm_threadlink",
            "start_unlock",
            |_lock_id: u32, _fidx: u32, _iidx: u32| {
                // Noop: Not yet in use ...
            },
        )?;
        linker.func_wrap(
            "wasm_threadlink",
            "finish_unlock",
            Self::get_start_unlock_func(self.tracing.clone()),
        )?;
        linker.func_wrap(
            "wasabi",
            "read_hook",
            Self::get_read_hook_func(self.tracing.clone()),
        )?;
        linker.func_wrap(
            "wasabi",
            "write_hook",
            Self::get_write_hook_func(self.tracing.clone()),
        )?;

        Ok(())
    }
}
