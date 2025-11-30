use wasmtime::Caller;

use crate::{
    WaliResult, WaliView,
    ctx::impls::SyscallResult,
    ctx::utils::mem::{NativePtr, WasmPtr},
};

#[inline]
pub fn wali_gettimeofday<T: WaliView>(
    mut caller: Caller<'_, T>,
    timeval: WasmPtr,
    timezone: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall2(
            nc::SYS_GETTIMEOFDAY,
            NativePtr::from_wasm_ptr(&mut caller, timeval).addr(),
            NativePtr::from_wasm_ptr(&mut caller, timezone).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_clock_gettime<T: WaliView>(
    mut caller: Caller<'_, T>,
    clockid: u32,
    tp: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall2(
            nc::SYS_CLOCK_GETTIME,
            clockid as usize,
            NativePtr::from_wasm_ptr(&mut caller, tp).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}
