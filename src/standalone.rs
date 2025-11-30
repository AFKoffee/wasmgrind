use std::ops::Deref;

use crate::standalone::ctx::WasmgrindStandaloneCtx;

pub mod ctx;

pub struct StandaloneCtxView<'ctx> {
    ctx: &'ctx WasmgrindStandaloneCtx,
}

impl<'ctx> Deref for StandaloneCtxView<'ctx> {
    type Target = WasmgrindStandaloneCtx;

    fn deref(&self) -> &Self::Target {
        self.ctx
    }
}

impl<'ctx> From<&'ctx WasmgrindStandaloneCtx> for StandaloneCtxView<'ctx> {
    fn from(value: &'ctx WasmgrindStandaloneCtx) -> Self {
        Self { ctx: value }
    }
}

pub trait StandaloneView: Send + Sync + Clone {
    fn ctx(&self) -> StandaloneCtxView<'_>;
}

impl StandaloneView for WasmgrindStandaloneCtx {
    fn ctx(&self) -> StandaloneCtxView<'_> {
        StandaloneCtxView::from(self)
    }
}
