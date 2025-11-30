use std::ops::Deref;

use crate::tracing::ctx::WasmgrindTracingCtx;

pub mod ctx;

pub struct TracingCtxView<'ctx> {
    ctx: &'ctx WasmgrindTracingCtx,
}

impl<'ctx> Deref for TracingCtxView<'ctx> {
    type Target = WasmgrindTracingCtx;

    fn deref(&self) -> &Self::Target {
        self.ctx
    }
}

impl<'ctx> From<&'ctx WasmgrindTracingCtx> for TracingCtxView<'ctx> {
    fn from(value: &'ctx WasmgrindTracingCtx) -> Self {
        Self { ctx: value }
    }
}

pub trait TracingView {
    fn ctx(&self) -> TracingCtxView<'_>;
}

impl TracingView for WasmgrindTracingCtx {
    fn ctx(&self) -> TracingCtxView<'_> {
        TracingCtxView::from(self)
    }
}
