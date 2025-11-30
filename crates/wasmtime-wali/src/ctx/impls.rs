mod event;
mod fs;
mod io;
mod ioctl;
mod mm;
mod net;
mod proc;
mod signal;
mod time;
mod wali;

pub use event::*;
pub use fs::*;
pub use io::*;
pub use ioctl::*;
pub use mm::*;
pub use net::*;
pub use proc::*;
pub use signal::*;
pub use time::*;
pub use wali::*;

use crate::ctx::utils::mem::WasmPtr;

pub struct SyscallResult<T> {
    inner: Result<T, nc::Errno>,
}

impl<T> SyscallResult<T> {
    pub fn map<U, F: FnOnce(T) -> U>(self, op: F) -> SyscallResult<U> {
        SyscallResult {
            inner: self.inner.map(op),
        }
    }
}

impl<T> From<Result<T, nc::Errno>> for SyscallResult<T> {
    fn from(value: Result<T, nc::Errno>) -> Self {
        Self { inner: value }
    }
}

impl From<SyscallResult<usize>> for i64 {
    fn from(value: SyscallResult<usize>) -> Self {
        assert!(
            std::mem::size_of::<usize>() <= std::mem::size_of::<i64>(),
            "'usize' did not fit into 'i64'"
        );
        match value.inner {
            Ok(retval) => retval as i64,
            Err(errno) => -(errno as i64),
        }
    }
}

impl From<SyscallResult<isize>> for i64 {
    fn from(value: SyscallResult<isize>) -> Self {
        assert!(
            std::mem::size_of::<isize>() <= std::mem::size_of::<i64>(),
            "'isize' did not fit into 'i64'"
        );
        match value.inner {
            Ok(retval) => retval as i64,
            Err(errno) => -(errno as i64),
        }
    }
}

impl From<SyscallResult<i32>> for i64 {
    fn from(value: SyscallResult<i32>) -> Self {
        match value.inner {
            Ok(retval) => retval as i64,
            Err(errno) => -(errno as i64),
        }
    }
}

impl From<SyscallResult<()>> for i64 {
    fn from(value: SyscallResult<()>) -> Self {
        match value.inner {
            Ok(()) => 0,
            Err(errno) => -(errno as i64),
        }
    }
}

impl From<SyscallResult<WasmPtr>> for i64 {
    fn from(value: SyscallResult<WasmPtr>) -> Self {
        match value.inner {
            Ok(ptr) => ptr.raw() as i64,
            Err(errno) => -(errno as i64),
        }
    }
}
