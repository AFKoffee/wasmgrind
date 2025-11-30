use wasmtime::Caller;

use crate::{
    WaliResult, WaliView,
    ctx::{
        impls::SyscallResult,
        utils::mem::{NativePtr, WasmPtr},
    },
};

#[inline]
pub fn wali_poll<T: WaliView>(
    mut caller: Caller<'_, T>,
    fds: WasmPtr,
    nfds: u32,
    timeout: i32,
) -> WaliResult<SyscallResult<usize>> {
    let fds_ptr = NativePtr::from_wasm_ptr(&mut caller, fds);
    let retval = unsafe {
        nc::syscalls::syscall3(
            nc::SYS_POLL,
            fds_ptr.addr(),
            nfds as usize,
            timeout as usize,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}
