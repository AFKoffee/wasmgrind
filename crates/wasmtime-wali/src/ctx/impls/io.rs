use std::{
    ffi::c_void,
    ptr::{self, NonNull},
};

use wasmtime::Caller;

use crate::{
    WaliResult, WaliView,
    ctx::impls::SyscallResult,
    ctx::utils::mem::{NativePtr, WasmPtr},
    systypes::wasm_iovec_t,
};

#[inline]
pub fn wali_write<T: WaliView>(
    mut caller: Caller<'_, T>,
    fd: i32,
    buf: WasmPtr,
    count: u32,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall3(
            nc::SYS_WRITE,
            fd as usize,
            NativePtr::from_wasm_ptr(&mut caller, buf).addr(),
            count as usize,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_read<T: WaliView>(
    mut caller: Caller<'_, T>,
    fd: i32,
    buf: WasmPtr,
    count: u32,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall3(
            nc::SYS_READ,
            fd as usize,
            NativePtr::from_wasm_ptr(&mut caller, buf).addr(),
            count as usize,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_writev<T: WaliView>(
    mut caller: Caller<'_, T>,
    fd: i32,
    iov: WasmPtr,
    iovcnt: u32,
) -> WaliResult<SyscallResult<isize>> {
    let wasm_iov_ptr = NativePtr::from_wasm_ptr(&mut caller, iov);
    let retval = if let Some(mut wasm_iov_ptr) = NonNull::new(wasm_iov_ptr.raw::<wasm_iovec_t>()) {
        let mut native_iov: Vec<nc::iovec_t> = Vec::with_capacity(iovcnt as usize);
        for _ in 0..iovcnt {
            let wasm_iovec_t { iov_base, iov_len } = unsafe { wasm_iov_ptr.read() };

            native_iov.push(nc::iovec_t {
                iov_base: NativePtr::from_wasm_ptr(&mut caller, iov_base.into()).raw(),
                iov_len: iov_len as usize,
            });

            wasm_iov_ptr = unsafe { wasm_iov_ptr.add(1) };
        }

        unsafe { nc::writev(fd as usize, native_iov.as_slice()) }
    } else {
        // In case of a null pointer we let the OS do whatever it deems to be fit
        // This will probably crash with EFAULT error code if 'iovcnt' is non-zero
        let res = unsafe {
            nc::syscalls::syscall3(
                nc::SYS_WRITEV,
                fd as usize,
                ptr::null_mut::<c_void>().addr(),
                iovcnt as usize,
            )
        };

        res.map(|res| res as nc::ssize_t)
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_readv<T: WaliView>(
    mut caller: Caller<'_, T>,
    fd: i32,
    iov: WasmPtr,
    iovcnt: u32,
) -> WaliResult<SyscallResult<isize>> {
    let wasm_iov_ptr = NativePtr::from_wasm_ptr(&mut caller, iov);
    let retval = if let Some(mut wasm_iov_ptr) = NonNull::new(wasm_iov_ptr.raw::<wasm_iovec_t>()) {
        let mut native_iov: Vec<nc::iovec_t> = Vec::with_capacity(iovcnt as usize);
        for _ in 0..iovcnt {
            let wasm_iovec_t { iov_base, iov_len } = unsafe { wasm_iov_ptr.read() };

            native_iov.push(nc::iovec_t {
                iov_base: NativePtr::from_wasm_ptr(&mut caller, iov_base.into()).raw(),
                iov_len: iov_len as usize,
            });

            wasm_iov_ptr = unsafe { wasm_iov_ptr.add(1) };
        }

        unsafe { nc::readv(fd as usize, native_iov.as_mut_slice()) }
    } else {
        // In case of a null pointer we let the OS do whatever it deems to be fit
        // This will probably crash with EFAULT error code if 'iovcnt' is non-zero
        let res = unsafe {
            nc::syscalls::syscall3(
                nc::SYS_WRITEV,
                fd as usize,
                ptr::null_mut::<c_void>().addr(),
                iovcnt as usize,
            )
        };

        res.map(|res| res as nc::ssize_t)
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_close<T: WaliView>(caller: Caller<'_, T>, fd: i32) -> WaliResult<SyscallResult<()>> {
    let retval = unsafe { nc::close(fd) };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}
