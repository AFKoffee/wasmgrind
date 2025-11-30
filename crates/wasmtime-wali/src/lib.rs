use std::{fmt::Display, ops::Deref};

use crate::ctx::WaliCtx;

pub mod ctx;
mod memory;
/// holds structs that WALI programs may pass in syscalls
mod systypes;

pub struct WaliCtxView<'ctx> {
    ctx: &'ctx WaliCtx,
}

impl<'ctx> Deref for WaliCtxView<'ctx> {
    type Target = WaliCtx;

    fn deref(&self) -> &Self::Target {
        self.ctx
    }
}

impl<'ctx> From<&'ctx WaliCtx> for WaliCtxView<'ctx> {
    fn from(value: &'ctx WaliCtx) -> Self {
        Self { ctx: value }
    }
}

pub trait WaliView: Send + Sync + Clone {
    fn ctx(&self) -> WaliCtxView<'_>;
}

impl WaliView for WaliCtx {
    fn ctx(&self) -> WaliCtxView<'_> {
        WaliCtxView::from(self)
    }
}

#[derive(Debug)]
pub enum WaliTrap {
    ThreadExiting,
    ProcessExiting,
}

impl Display for WaliTrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WaliTrap::ThreadExiting => write!(
                f,
                "WaliTrap - ThreadExiting: a custom trap used to propagate an 'SYS_exit' call through Wasmtime up to the entry function."
            ),
            WaliTrap::ProcessExiting => write!(
                f,
                "WaliTrap - ProcessExiting: a custom trap used to propagate a process exit through Wasmtime up to the entry function."
            ),
        }
    }
}

impl std::error::Error for WaliTrap {}

type WaliResult<T> = Result<T, WaliTrap>;
