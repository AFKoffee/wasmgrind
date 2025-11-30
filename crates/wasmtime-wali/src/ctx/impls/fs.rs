use wasmtime::Caller;

use crate::{
    WaliResult, WaliView,
    ctx::impls::SyscallResult,
    ctx::utils::mem::{NativePtr, WasmPtr},
};

#[inline]
pub fn wali_lseek<T: WaliView>(
    caller: Caller<'_, T>,
    fd: i32,
    offset: i64,
    whence: i32,
) -> WaliResult<SyscallResult<()>> {
    let retval = unsafe { nc::lseek(fd, offset as isize, whence) };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_access<T: WaliView>(
    mut caller: Caller<'_, T>,
    path: WasmPtr,
    mode: i32,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall2(
            nc::SYS_ACCESS,
            NativePtr::from_wasm_ptr(&mut caller, path).addr(),
            mode as usize,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_lstat<T: WaliView>(
    mut caller: Caller<'_, T>,
    path: WasmPtr,
    buf: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall2(
            nc::SYS_LSTAT,
            NativePtr::from_wasm_ptr(&mut caller, path).addr(),
            NativePtr::from_wasm_ptr(&mut caller, buf).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_stat<T: WaliView>(
    mut caller: Caller<'_, T>,
    path: WasmPtr,
    buf: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall2(
            nc::SYS_STAT,
            NativePtr::from_wasm_ptr(&mut caller, path).addr(),
            NativePtr::from_wasm_ptr(&mut caller, buf).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_fstatat<T: WaliView>(
    mut caller: Caller<'_, T>,
    fd: i32,
    path: WasmPtr,
    buf: WasmPtr,
    flags: i32,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall4(
            nc::SYS_NEWFSTATAT,
            fd as usize,
            NativePtr::from_wasm_ptr(&mut caller, path).addr(),
            NativePtr::from_wasm_ptr(&mut caller, buf).addr(),
            flags as usize,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_fstat<T: WaliView>(
    mut caller: Caller<'_, T>,
    fd: i32,
    buf: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall2(
            nc::SYS_FSTAT,
            fd as usize,
            NativePtr::from_wasm_ptr(&mut caller, buf).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_fcntl<T: WaliView>(
    mut caller: Caller<'_, T>,
    fd: i32,
    op: u32,
    arg: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        match op {
            nc::F_GETLK | nc::F_SETLK | nc::F_GETOWN | nc::F_SETOWN => nc::syscalls::syscall3(
                nc::SYS_FCNTL,
                fd as usize,
                op as usize,
                NativePtr::from_wasm_ptr(&mut caller, arg).addr(),
            ),
            _ => {
                nc::syscalls::syscall3(nc::SYS_FCNTL, fd as usize, op as usize, arg.raw() as usize)
            }
        }
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_open<T: WaliView>(
    mut caller: Caller<'_, T>,
    path: WasmPtr,
    flags: u32,
    mode: u32,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall3(
            nc::SYS_OPEN,
            NativePtr::from_wasm_ptr(&mut caller, path).addr(),
            flags as usize,
            mode as usize,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_getdents64<T: WaliView>(
    mut caller: Caller<'_, T>,
    fd: i32,
    dirp: WasmPtr,
    count: u32,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall3(
            nc::SYS_GETDENTS64,
            fd as usize,
            NativePtr::from_wasm_ptr(&mut caller, dirp).addr(),
            count as usize,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_chmod<T: WaliView>(
    mut caller: Caller<'_, T>,
    path: WasmPtr,
    mode: u32,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall2(
            nc::SYS_CHMOD,
            NativePtr::from_wasm_ptr(&mut caller, path).addr(),
            mode as usize,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_chown<T: WaliView>(
    mut caller: Caller<'_, T>,
    path: WasmPtr,
    owner: u32,
    group: u32,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall3(
            nc::SYS_CHOWN,
            NativePtr::from_wasm_ptr(&mut caller, path).addr(),
            owner as usize,
            group as usize,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_utimensat<T: WaliView>(
    mut caller: Caller<'_, T>,
    dirfd: i32,
    path: WasmPtr,
    timespec: WasmPtr,
    flags: i32,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall4(
            nc::SYS_UTIMENSAT,
            dirfd as usize,
            NativePtr::from_wasm_ptr(&mut caller, path).addr(),
            NativePtr::from_wasm_ptr(&mut caller, timespec).addr(),
            flags as usize,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_futimesat<T: WaliView>(
    mut caller: Caller<'_, T>,
    dirfd: i32,
    path: WasmPtr,
    timespec: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall3(
            nc::SYS_FUTIMESAT,
            dirfd as usize,
            NativePtr::from_wasm_ptr(&mut caller, path).addr(),
            NativePtr::from_wasm_ptr(&mut caller, timespec).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_utimes<T: WaliView>(
    mut caller: Caller<'_, T>,
    path: WasmPtr,
    timespec: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall2(
            nc::SYS_UTIMES,
            NativePtr::from_wasm_ptr(&mut caller, path).addr(),
            NativePtr::from_wasm_ptr(&mut caller, timespec).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_unlink<T: WaliView>(
    mut caller: Caller<'_, T>,
    path: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall1(
            nc::SYS_UNLINK,
            NativePtr::from_wasm_ptr(&mut caller, path).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_rmdir<T: WaliView>(
    mut caller: Caller<'_, T>,
    path: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall1(
            nc::SYS_RMDIR,
            NativePtr::from_wasm_ptr(&mut caller, path).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}
