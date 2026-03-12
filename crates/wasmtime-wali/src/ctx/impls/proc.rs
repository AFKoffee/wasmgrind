use std::{
    cell::UnsafeCell,
    sync::{Arc, OnceLock, atomic::Ordering, mpsc::RecvTimeoutError},
    time::Duration,
};

use anyhow::Error;
use wasmtime::{Caller, Linker, Store};

use crate::{
    WaliResult, WaliTrap, WaliView,
    ctx::WaliCtxInner,
    ctx::impls::SyscallResult,
    ctx::utils::{
        self,
        exports::get_ifp_from_instance,
        mem::{NativePtr, WasmPtr},
    },
};

#[inline]
pub fn wali_proc_exit<T: WaliView>(caller: Caller<'_, T>, status: i32) -> Error {
    let wali_ctx = &caller.data().ctx();
    if !wali_ctx.0.wali_deinit_called.load(Ordering::Acquire) || status != 0 {
        // Reference implementation throws a wasm-exception here ...
        // ... not quite sure if we want to support that.
        // Just print an error for now:
        log::info!("WALI process exit called prematurely. Exit code: {status}")
    }

    wali_ctx.0.proc_exit_code.store(status, Ordering::Release);

    wali_ctx.0.proc_exit_invoked.store(true, Ordering::Release);

    // NOTE:
    // Ideally we would just terminate all running threads here, but we cant
    // do that. Termination is deferred to calls of 'WaliCtxInner::return_or_exit',
    // which is called in our WALI import implementations.
    Error::new(WaliTrap::ProcessExiting)
}

#[inline]
pub fn wali_thread_exit<T: WaliView>(caller: Caller<'_, T>, status: i32) -> Error {
    if caller
        .data()
        .ctx()
        .0
        .thread_count
        .fetch_sub(1, Ordering::AcqRel)
        == 1
    {
        // Thread count was 1 so we are the last thread to exit.
        // In this case we do some additional bookkeeping.
        wali_proc_exit(caller, status)
    } else {
        // This situation will occur if 'SYS_exit' is called
        // while multiple threads are running.
        // We can't just terminate them like the OS would
        // so we wait for the next safepoint and issue a
        // custom trap to Wasmtime.
        Error::new(WaliTrap::ThreadExiting)
    }
}

#[inline]
pub fn wali_thread_spawn<T: WaliView>(
    linker: Arc<OnceLock<Linker<T>>>,
    caller: Caller<'_, T>,
    setup_fnptr: u32,
    arg_wasm: i32,
) -> WaliResult<i32> {
    let (tx, rx) = std::sync::mpsc::sync_channel::<i32>(1);
    let data = caller.data().clone();
    let engine = caller.engine().clone();
    std::thread::spawn(move || {
        let ctx = data.ctx();
        ctx.0.thread_count.fetch_add(1, Ordering::AcqRel);

        let module = &ctx.0.module;
        let mut thread_store = Store::new(&engine, data.clone());
        thread_store.epoch_deadline_callback(utils::signal::signal_poll_callback());
        thread_store.set_epoch_deadline(WaliCtxInner::SIGNAL_POLL_EPOCH);
        let thread_instance = linker
            .get()
            .expect("Linker was not initialized")
            .instantiate(&mut thread_store, module)
            .expect("Failed to create Wasmtime instance in thread");

        let indirect_function_provider = get_ifp_from_instance(&thread_instance, &mut thread_store);
        let setup_wasm_fn = indirect_function_provider
            .call(&mut thread_store, setup_fnptr)
            .expect("Could not get funcref for thread startup. Should be '__wasm_thread_start_libc' in the binary")
            .expect("funcref for '__wasm_thread_start_libc' was null")
            .typed::<(i32, i32), ()>(&thread_store)
            .expect("Thread startup routine was of wrong function type");

        let tid = unsafe { nc::gettid() };
        tx.send(tid)
            .expect("Failed to send TID to the parent. Channel closed.");

        match setup_wasm_fn.call(&mut thread_store, (tid, arg_wasm)) {
            Ok(()) => log::warn!("Thread {tid} exited without custom Wali trap"),
            Err(e) => match e.downcast::<WaliTrap>() {
                Ok(trap) => match trap {
                    WaliTrap::ThreadExiting => log::info!("Thread {tid} is exiting normally"),
                    WaliTrap::ProcessExiting => log::info!("Thread {tid} triggered process exit"),
                },
                Err(e) => log::error!("Thread {tid} exited with error: {e}"),
            },
        }
    });

    let tid = match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(tid) => tid,
        Err(RecvTimeoutError::Timeout) => {
            log::warn!("TID channel timeouted. Did not receive child thread id.");
            -1
        }
        Err(RecvTimeoutError::Disconnected) => {
            panic!("Failed to receive TID from child thread. Channel closed!")
        }
    };

    caller.data().ctx().return_or_exit(tid)
}

#[inline]
pub fn wali_sched_setscheduler<T: WaliView>(
    mut caller: Caller<'_, T>,
    pid: i32,
    policy: i32,
    sched_param: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall3(
            nc::SYS_SCHED_SETSCHEDULER,
            pid as usize,
            policy as usize,
            NativePtr::from_wasm_ptr(&mut caller, sched_param).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_set_robust_list<T: WaliView>(
    caller: Caller<'_, T>,
    _head: u32,
    _size: u32,
) -> WaliResult<i64> {
    log::warn!(
        "Syscall 'SYS_set_robust_list' is not implemented yet. This is just a no op. See source code comments for futher details."
    );
    // Implementing this will be quite tricky:
    //
    // The linux kernel uses the robust list when a thread died to wake threads
    // waiting on futexes held by the dying thread. This prevents threads from
    // indefinitly waiting on a dead thread that missed to release a futex.
    //
    // The problem with the passthrough is that the wasm32 side and the native side expect
    // different memory layouts of the robust list type, specifically 'robust_list_head',
    // which is defined like this in the "include/uapi/linux/futex.h" file of the linux kernel:
    //
    // struct robust_list {
    //     struct robust_list __user *next;
    // };
    //
    // struct robust_list_head {
    //     struct robust_list list;
    //     long futex_offset;
    //     struct robust_list __user *list_op_pending;
    // };
    //
    // However, while WALI's musl-libc uses those types correctly, after compilation
    // the size of robust_list_head will be 12 bytes on 32bit WebAssembly, whereas
    // the x86-64 linux kernel expects it to be 24 bytes (due to differnt pointer-size
    // and long being 8 bytes wide rather than 4). So just forwarding this call will
    // provide "bogus data" to the kernel (at least from its point of view).
    //
    // One possible approach of handling this is to record the head of this
    // list in a thread local structure and sync the list into native space
    // before the thread dies, e.g. on 'SYS_exit' calls. Further reasearch
    // has to be made to investigate if this is possible and which points
    // need to be covered to support this call.
    //
    // For now we just intercept and ignore it. The only thing that could
    // happen is a thread deadlocking because another one missed to unlock
    // a futex before exiting - provided the pthread_t is even configured
    // to use the robust list.
    caller.data().ctx().return_or_exit(0)
}

#[inline]
pub fn wali_futex<T: WaliView>(
    mut caller: Caller<'_, T>,
    uaddr: WasmPtr,
    op: i32,
    val: u32,
    utime: u32,
    uaddr2: WasmPtr,
    val3: u32,
) -> WaliResult<SyscallResult<usize>> {
    let wasm_uaddr = uaddr.raw();
    let wasm_uaddr2 = uaddr2.raw();
    let native_uaddr = NativePtr::from_wasm_ptr(&mut caller, uaddr);
    let native_uaddr2 = NativePtr::from_wasm_ptr(&mut caller, uaddr2);

    // Match on operation to find out if `utime` specifies a timeout (and thus is a pointer).
    // See linux sources:
    // - https://github.com/torvalds/linux/blob/af4e9ef3d78420feb8fe58cd9a1ab80c501b3c08/kernel/futex/syscalls.c#L161-L172
    // - https://github.com/torvalds/linux/blob/af4e9ef3d78420feb8fe58cd9a1ab80c501b3c08/include/uapi/linux/futex.h#L26-L28
    let retval = match op & nc::FUTEX_CMD_MASK {
        nc::FUTEX_WAIT |
        nc::FUTEX_LOCK_PI |
        13 | // FUTEX_LOCK_PI2
        nc::FUTEX_WAIT_BITSET |
        nc::FUTEX_WAIT_REQUEUE_PI => unsafe {
            let wasm_utime = utime;
            let native_utime = NativePtr::from_wasm_ptr(&mut caller, WasmPtr::from(utime));
            log::debug!(
                "SYS_futex: uaddr {:#x}=>{:p}, op {op}, val {val}, utime {:#x}=>{:p}, uaddr2 {:#x}=>{:p}, val3: {val3}",
                wasm_uaddr,
                native_uaddr.raw::<UnsafeCell<u8>>(),
                wasm_utime,
                native_utime.raw::<UnsafeCell<u8>>(),
                wasm_uaddr2,
                native_uaddr2.raw::<UnsafeCell<u8>>()
            );
            nc::syscalls::syscall6(
                nc::SYS_FUTEX,
                native_uaddr.addr(),
                op as usize,
                val as usize,
                native_utime.addr(),
                native_uaddr2.addr(),
                val3 as usize,
            )
        },
        _ => unsafe {
            log::debug!(
                "SYS_futex: uaddr {:#x}=>{:p}, op {op}, val {val}, val2 {utime}, uaddr2 {:#x}=>{:p}, val3: {val3}",
                wasm_uaddr,
                native_uaddr.raw::<UnsafeCell<u8>>(),
                wasm_uaddr2,
                native_uaddr2.raw::<UnsafeCell<u8>>()
            );
            nc::syscalls::syscall6(
                nc::SYS_FUTEX,
                native_uaddr.addr(),
                op as usize,
                val as usize,
                utime as usize,
                native_uaddr2.addr(),
                val3 as usize,
            )
        }
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_set_tid_addess<T: WaliView>(
    mut caller: Caller<'_, T>,
    tidptr: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall1(
            nc::SYS_SET_TID_ADDRESS,
            NativePtr::from_wasm_ptr(&mut caller, tidptr).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_getrusage<T: WaliView>(
    mut caller: Caller<'_, T>,
    who: i32,
    usage: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall2(
            nc::SYS_GETRUSAGE,
            who as usize,
            NativePtr::from_wasm_ptr(&mut caller, usage).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_sched_getaffinity<T: WaliView>(
    mut caller: Caller<'_, T>,
    pid: i32,
    cpusetsize: u32,
    mask: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall3(
            nc::SYS_SCHED_GETAFFINITY,
            pid as usize,
            cpusetsize as usize,
            NativePtr::from_wasm_ptr(&mut caller, mask).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_sysinfo<T: WaliView>(
    mut caller: Caller<'_, T>,
    sysinfo: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall1(
            nc::SYS_SYSINFO,
            NativePtr::from_wasm_ptr(&mut caller, sysinfo).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_prlimit64<T: WaliView>(
    mut caller: Caller<'_, T>,
    pid: i32,
    ressource: i32,
    new_limit: WasmPtr,
    old_limit: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall4(
            nc::SYS_PRLIMIT64,
            pid as usize,
            ressource as usize,
            NativePtr::from_wasm_ptr(&mut caller, new_limit).addr(),
            NativePtr::from_wasm_ptr(&mut caller, old_limit).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_getrlimit<T: WaliView>(
    mut caller: Caller<'_, T>,
    ressource: i32,
    rlim: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall2(
            nc::SYS_GETRLIMIT,
            ressource as usize,
            NativePtr::from_wasm_ptr(&mut caller, rlim).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_setpriority<T: WaliView>(
    caller: Caller<'_, T>,
    which: i32,
    who: i32,
    prio: i32,
) -> WaliResult<SyscallResult<()>> {
    let retval = unsafe { nc::setpriority(which, who, prio) };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_nanosleep<T: WaliView>(
    mut caller: Caller<'_, T>,
    duration: WasmPtr,
    rem: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall2(
            nc::SYS_NANOSLEEP,
            NativePtr::from_wasm_ptr(&mut caller, duration).addr(),
            NativePtr::from_wasm_ptr(&mut caller, rem).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_clock_nanosleep<T: WaliView>(
    mut caller: Caller<'_, T>,
    clockid: i32,
    flags: i32,
    duration: WasmPtr,
    rem: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall4(
            nc::SYS_CLOCK_NANOSLEEP,
            clockid as usize,
            flags as usize,
            NativePtr::from_wasm_ptr(&mut caller, duration).addr(),
            NativePtr::from_wasm_ptr(&mut caller, rem).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_uname<T: WaliView>(
    mut caller: Caller<'_, T>,
    buf: WasmPtr,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall1(
            nc::SYS_UNAME,
            NativePtr::from_wasm_ptr(&mut caller, buf).addr(),
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}
