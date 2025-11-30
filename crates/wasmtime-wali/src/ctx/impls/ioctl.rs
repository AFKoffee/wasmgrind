use wasmtime::Caller;

use crate::{
    WaliResult, WaliView,
    ctx::impls::SyscallResult,
    ctx::utils::mem::{NativePtr, WasmPtr},
};

#[inline]
pub fn wali_ioctl<T: WaliView>(
    mut caller: Caller<'_, T>,
    fd: i32,
    cmd: u32,
    arg: WasmPtr,
) -> WaliResult<SyscallResult<i32>> {
    let retval = unsafe { nc::ioctl(fd, cmd, NativePtr::from_wasm_ptr(&mut caller, arg).raw()) };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}
