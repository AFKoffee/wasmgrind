use wasmtime::Caller;

use crate::{
    WaliResult, WaliView,
    ctx::{
        impls::SyscallResult,
        utils::mem::{NativePtr, WasmPtr},
    },
};

#[inline]
pub fn wali_accept<T: WaliView>(
    mut caller: Caller<'_, T>,
    socket: i32,
    sockaddr: WasmPtr,
    address_len: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let sockaddr_ptr = NativePtr::from_wasm_ptr(&mut caller, sockaddr);
    let socklen_ptr = NativePtr::from_wasm_ptr(&mut caller, address_len);
    let retval = unsafe {
        nc::syscalls::syscall3(
            nc::SYS_ACCEPT,
            socket as usize,
            sockaddr_ptr.addr(),
            socklen_ptr.addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_bind<T: WaliView>(
    mut caller: Caller<'_, T>,
    socket: i32,
    sockaddr: WasmPtr,
    address_len: u32,
) -> WaliResult<SyscallResult<()>> {
    let sockaddr_ptr = NativePtr::from_wasm_ptr(&mut caller, sockaddr);
    let retval = unsafe { nc::bind(socket, sockaddr_ptr.raw(), address_len) };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_connect<T: WaliView>(
    mut caller: Caller<'_, T>,
    socket: i32,
    sockaddr: WasmPtr,
    address_len: u32,
) -> WaliResult<SyscallResult<()>> {
    let sockaddr_ptr = NativePtr::from_wasm_ptr(&mut caller, sockaddr);
    let retval = unsafe { nc::connect(socket, sockaddr_ptr.raw(), address_len) };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_listen<T: WaliView>(
    caller: Caller<'_, T>,
    socket: i32,
    backlog: i32,
) -> WaliResult<SyscallResult<()>> {
    let retval = unsafe { nc::listen(socket, backlog) };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_getsockname<T: WaliView>(
    mut caller: Caller<'_, T>,
    socket: i32,
    sockaddr: WasmPtr,
    address_len: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let sockaddr_ptr = NativePtr::from_wasm_ptr(&mut caller, sockaddr);
    let socklen_ptr = NativePtr::from_wasm_ptr(&mut caller, address_len);
    let retval = unsafe {
        nc::syscalls::syscall3(
            nc::SYS_GETSOCKNAME,
            socket as usize,
            sockaddr_ptr.addr(),
            socklen_ptr.addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_recvmsg<T: WaliView>(
    mut caller: Caller<'_, T>,
    socket: i32,
    msg: WasmPtr,
    flags: i32,
) -> WaliResult<SyscallResult<usize>> {
    let msg_ptr = NativePtr::from_wasm_ptr(&mut caller, msg);
    let retval = unsafe {
        nc::syscalls::syscall3(
            nc::SYS_RECVMSG,
            socket as usize,
            msg_ptr.addr(),
            flags as usize,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_sendmsg<T: WaliView>(
    mut caller: Caller<'_, T>,
    socket: i32,
    msg: WasmPtr,
    flags: i32,
) -> WaliResult<SyscallResult<usize>> {
    let msg_ptr = NativePtr::from_wasm_ptr(&mut caller, msg);
    let retval = unsafe {
        nc::syscalls::syscall3(
            nc::SYS_SENDMSG,
            socket as usize,
            msg_ptr.addr(),
            flags as usize,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_sendto<T: WaliView>(
    mut caller: Caller<'_, T>,
    socket: i32,
    buf: WasmPtr,
    len: u32,
    flags: i32,
    dest_addr: WasmPtr,
    addrlen: u32,
) -> WaliResult<SyscallResult<usize>> {
    let buf_ptr = NativePtr::from_wasm_ptr(&mut caller, buf);
    let dest_ptr = NativePtr::from_wasm_ptr(&mut caller, dest_addr);
    let retval = unsafe {
        nc::syscalls::syscall6(
            nc::SYS_SENDTO,
            socket as usize,
            buf_ptr.addr(),
            len as usize,
            flags as usize,
            dest_ptr.addr(),
            addrlen as usize,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_setsockopt<T: WaliView>(
    mut caller: Caller<'_, T>,
    socket: i32,
    level: i32,
    optname: i32,
    optval: WasmPtr,
    optlen: u32,
) -> WaliResult<SyscallResult<()>> {
    let opt_ptr = NativePtr::from_wasm_ptr(&mut caller, optval);
    let retval = unsafe { nc::setsockopt(socket, level, optname, opt_ptr.raw(), optlen) };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_socket<T: WaliView>(
    caller: Caller<'_, T>,
    domain: i32,
    ty: i32,
    protocol: i32,
) -> WaliResult<SyscallResult<i32>> {
    let retval = unsafe { nc::socket(domain, ty, protocol) };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}
