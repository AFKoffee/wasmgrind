use std::{
    ffi::{CString, c_int},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering},
    },
};

use anyhow::Error;
use wasmtime::{Caller, Engine, Linker, Module};

use crate::{
    WaliResult, WaliTrap, WaliView,
    ctx::utils::{mem::MMapManager, signal::SigTable},
};

mod impls;
mod provider;
mod utils;

pub use provider::WaliCtxProvider;

pub struct WaliCtx(Arc<WaliCtxInner>);

impl Clone for WaliCtx {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl WaliCtx {
    fn return_or_exit<T>(&self, retval: T) -> WaliResult<T> {
        // We deviate from the reference implementation here:
        // In WAMR the threads are managed in a thread-pool by the
        // runtime, so the authors could not just kill the thread here.
        //
        // We can also not do that due to not knowing which thread
        // issued the `SYS_exit_group`. If we just kill the thread
        // here we may terminate the host-main thread and
        // therefore the whole process. Therefore, we
        // rely on custom traps.
        if self.0.proc_exit_invoked.load(Ordering::Acquire) {
            // This situation will occur if 'SYS_exit_group' is called
            // while multiple threads are running.
            //
            // We can't just terminate them like the OS would
            // so we wait for the next safepoint and issue a
            // custom trap to Wasmtime.
            Err(WaliTrap::ThreadExiting)
        } else {
            Ok(retval)
        }
    }
}

// This needs to be Sync as it should be shared amongst threads via Arc
struct WaliCtxInner {
    wali_cl_args: Vec<CString>,
    wali_init_called: AtomicBool,
    wali_deinit_called: AtomicBool,
    proc_exit_invoked: AtomicBool,
    proc_exit_code: AtomicI32,
    thread_count: AtomicUsize,
    mmap_lock: Mutex<MMapManager>,
    sigtable: Mutex<SigTable>,
    sighandler: extern "C" fn(c_int),
    // In the original WALI reference implementation
    // this is set via command line arguments
    wali_app_env_file: Option<String>,
    module: Module,
}

impl WaliCtxInner {
    const MEMORY_EXPORT_NAME: &str = "memory";
    const GET_INDIRECT_FUNC_EXPORT_NAME: &str = "__wasmtime_wali_get_indirect_func";
    const MODULE_NAME: &str = "wali";
    const SIGNAL_POLL_EPOCH: u64 = 1000;
    const PROC_EXIT_CODE_INIT: i32 = -1;

    fn new(engine: &Engine, module: Module, args: Vec<CString>) -> Self {
        assert!(
            utils::signal::get_engine().is_none(),
            "Currently, the global engine is only allowed to be initialized once"
        );
        // FIXME:
        // Another thread my have set the engine between the assertion and the next call. We should prevent that.
        utils::signal::initialize_engine(engine);

        Self {
            wali_cl_args: args,
            wali_init_called: AtomicBool::new(false),
            wali_deinit_called: AtomicBool::new(false),
            proc_exit_invoked: AtomicBool::new(false),
            proc_exit_code: AtomicI32::new(Self::PROC_EXIT_CODE_INIT),
            thread_count: AtomicUsize::new(0),
            mmap_lock: Mutex::new(MMapManager::new()),
            sigtable: Mutex::new(SigTable::new()),
            sighandler: utils::signal::wali_sigact_handler,
            wali_app_env_file: None,
            module,
        }
    }

    unsafe fn add_to_linker<T: WaliView + 'static>(linker: &mut Linker<T>) -> Result<(), Error> {
        linker
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "__init",
                |caller: Caller<'_, T>| {
                    log::debug!("Before '{}'", "__init");
                    impls::wali_init(caller);
                    log::debug!("After '{}'", "__init");
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "__deinit",
                |caller: Caller<'_, T>| {
                    log::debug!("Before '{}'", "__deinit");
                    impls::wali_deinit(caller);
                    log::debug!("After '{}'", "__deinit");
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "__proc_exit",
                |caller: Caller<'_, T>, code: i32| -> Result<(), Error> {
                    log::debug!("Before '{}'", "__deinit");
                    Err::<(), _>(impls::wali_proc_exit(caller, code))?;
                    log::debug!("After '{}'", "__deinit");
                    Ok(())
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "__cl_get_argc",
                |caller: Caller<'_, T>| -> Result<u32, Error> {
                    log::debug!("Before '{}'", "__cl_get_argc");
                    let res = impls::wali_cl_get_argc(caller).map_err(Error::new);
                    log::debug!("After '{}'", "__cl_get_argc");
                    res
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "__cl_get_argv_len",
                |caller: Caller<'_, T>, arg_idx: u32| -> Result<u32, Error> {
                    log::debug!("Before '{}'", "__cl_get_argv_len");
                    let res = impls::wali_cl_get_argv_len(caller, arg_idx).map_err(Error::new)?;
                    log::debug!("After '{}'", "__cl_get_argv_len");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "__cl_copy_argv",
                |caller: Caller<'_, T>, arg_dst: u32, arg_idx: u32| -> Result<u32, Error> {
                    log::debug!("Before '{}'", "__cl_copy_argv");
                    let res = impls::wali_cl_copy_argv(caller, arg_dst.into(), arg_idx)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "__cl_copy_argv");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_set_tid_address",
                |caller: Caller<'_, T>, tidptr: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_set_tid_address");
                    let res = impls::wali_set_tid_addess(caller, tidptr.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_set_tid_address");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_mmap",
                |caller: Caller<'_, T>,
                 addr: u32,
                 len: u32,
                 prot: i32,
                 flags: i32,
                 fd: i32,
                 offset: i64|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_mmap");
                    let res = impls::wali_mmap(caller, addr, len, prot, flags, fd, offset)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_mmap");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "__get_init_envfile",
                |caller: Caller<'_, T>, faddr: u32, fsize: u32| -> Result<i32, Error> {
                    log::debug!("Before '{}'", "__get_init_envfile");
                    let res = impls::wali_get_init_envfile(caller, faddr.into(), fsize)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "__get_init_envfile");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_rt_sigaction",
                |caller: Caller<'_, T>,
                 signum: i32,
                 act: u32,
                 oldact: u32,
                 sigsetsize: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_rt_sigaction");
                    let res = impls::wali_rt_sigaction(
                        caller,
                        signum,
                        act.into(),
                        oldact.into(),
                        sigsetsize,
                    )
                    .map(i64::from)
                    .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_rt_sigaction");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_tkill",
                |caller: Caller<'_, T>, tid: i32, sig: i32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_tkill");
                    let res = impls::wali_tkill(caller, tid, sig)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_tkill");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_rt_sigprocmask",
                |caller: Caller<'_, T>,
                 how: i32,
                 set: u32,
                 oldset: u32,
                 sigsetsize: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_rt_sigprocmask");
                    let res = impls::wali_rt_sigprocmask(
                        caller,
                        how,
                        set.into(),
                        oldset.into(),
                        sigsetsize,
                    )
                    .map(i64::from)
                    .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_rt_sigprocmask");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_open",
                |caller: Caller<'_, T>, path: u32, flags: u32, mode: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_open");
                    let res = impls::wali_open(caller, path.into(), flags, mode)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_open");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_fcntl",
                |caller: Caller<'_, T>, fd: i32, op: u32, arg: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_fcntl");
                    let res = impls::wali_fcntl(caller, fd, op, arg.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_fcntl");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_brk",
                |caller: Caller<'_, T>, addr: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_brk");
                    let res = impls::wali_brk(caller, addr).map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_brk");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_mprotect",
                |caller: Caller<'_, T>, addr: u32, size: u32, prot: i32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_mprotect");
                    let res = impls::wali_mprotect(caller, addr.into(), size, prot)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_mprotect");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_mremap",
                |caller: Caller<'_, T>,
                 old_address: u32,
                 old_size: u32,
                 new_size: u32,
                 flags: u32,
                 new_address: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_mremap");
                    let res = impls::wali_mremap(
                        caller,
                        old_address.into(),
                        old_size,
                        new_size,
                        flags,
                        new_address.into(),
                    )
                    .map(i64::from)
                    .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_mremap");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_munmap",
                |caller: Caller<'_, T>, addr: u32, len: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_munmap");
                    let res = impls::wali_munmap(caller, addr.into(), len)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_munmap");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_fstat",
                |caller: Caller<'_, T>, fd: i32, buf: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_fstat");
                    let res = impls::wali_fstat(caller, fd, buf.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_fstat");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_fstatat",
                |caller: Caller<'_, T>,
                 fd: i32,
                 path: u32,
                 buf: u32,
                 flags: i32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_fstatat");
                    let res = impls::wali_fstatat(caller, fd, path.into(), buf.into(), flags)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_fstatat");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_stat",
                |caller: Caller<'_, T>, path: u32, buf: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_stat");
                    let res = impls::wali_stat(caller, path.into(), buf.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_stat");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_lstat",
                |caller: Caller<'_, T>, path: u32, buf: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_lstat");
                    let res = impls::wali_lstat(caller, path.into(), buf.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_lstat");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_close",
                |caller: Caller<'_, T>, fd: i32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_close");
                    let res = impls::wali_close(caller, fd)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_close");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_writev",
                |caller: Caller<'_, T>, fd: i32, iov: u32, iovcnt: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_writev");
                    let res = impls::wali_writev(caller, fd, iov.into(), iovcnt)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_writev");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_readv",
                |caller: Caller<'_, T>, fd: i32, iov: u32, iovcnt: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_readv");
                    let res = impls::wali_readv(caller, fd, iov.into(), iovcnt)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_readv");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_ioctl",
                |caller: Caller<'_, T>, fd: i32, cmd: u32, arg: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_ioctl");
                    let res = impls::wali_ioctl(caller, fd, cmd, arg.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_ioctl");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_futex",
                // wali-musl defines futex syscall with the following function signature:
                // CASE_SYSCALL (futex, futex, (int*)a1,(int)a2,(int)a3,(void*)a4,(int*)a5,(int)a6);
                |caller: Caller<'_, T>,
                 uaddr: u32,
                 op: i32,
                 val: u32,
                 utime: u32,
                 uaddr2: u32,
                 val3: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_futex");
                    let res = impls::wali_futex(
                        caller,
                        uaddr.into(),
                        op,
                        val,
                        utime,
                        uaddr2.into(),
                        val3,
                    )
                    .map(i64::from)
                    .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_futex");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_set_robust_list",
                |caller: Caller<'_, T>, head: u32, size: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_set_robust_list");
                    let res =
                        impls::wali_set_robust_list(caller, head, size).map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_set_robust_list");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_exit",
                |caller: Caller<'_, T>, status: i32| {
                    log::debug!("Before '{}'", "SYS_exit");
                    let res = Err::<i64, _>(impls::wali_thread_exit(caller, status))?;
                    log::debug!("After '{}'", "SYS_exit");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_exit_group",
                |caller: Caller<'_, T>, status: i32| {
                    log::debug!("Before '{}'", "SYS_exit_group");
                    let res = Err::<i64, _>(impls::wali_proc_exit(caller, status))?;
                    log::debug!("After '{}'", "SYS_exit_group");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_sched_setscheduler",
                |caller: Caller<'_, T>,
                 pid: i32,
                 policy: i32,
                 sched_param: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_sched_setscheduler");
                    let res =
                        impls::wali_sched_setscheduler(caller, pid, policy, sched_param.into())
                            .map(i64::from)
                            .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_sched_setscheduler");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_clock_gettime",
                |caller: Caller<'_, T>, clockid: u32, tp: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_clock_gettime");
                    let res = impls::wali_clock_gettime(caller, clockid, tp.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_clock_gettime");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_gettimeofday",
                |caller: Caller<'_, T>, timeval: u32, timezone: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_gettimeofday");
                    let res = impls::wali_gettimeofday(caller, timeval.into(), timezone.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_gettimeofday");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_access",
                |caller: Caller<'_, T>, path: u32, mode: i32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_access");
                    let res = impls::wali_access(caller, path.into(), mode)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_access");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_lseek",
                |caller: Caller<'_, T>, fd: i32, offset: i64, whence: i32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_lseek");
                    let res = impls::wali_lseek(caller, fd, offset, whence)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_lseek");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_read",
                |caller: Caller<'_, T>, fd: i32, buf: u32, count: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_read");
                    let res = impls::wali_read(caller, fd, buf.into(), count)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_read");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_write",
                |caller: Caller<'_, T>, fd: i32, buf: u32, count: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_write");
                    let res = impls::wali_write(caller, fd, buf.into(), count)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_write");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_accept",
                |caller: Caller<'_, T>,
                 sockfd: i32,
                 sockaddr: u32,
                 address_len: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_accept");
                    let res =
                        impls::wali_accept(caller, sockfd, sockaddr.into(), address_len.into())
                            .map(i64::from)
                            .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_accept");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_bind",
                |caller: Caller<'_, T>,
                 sockfd: i32,
                 sockaddr: u32,
                 address_len: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_bind");
                    let res = impls::wali_bind(caller, sockfd, sockaddr.into(), address_len)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_bind");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_connect",
                |caller: Caller<'_, T>,
                 sockfd: i32,
                 sockaddr: u32,
                 address_len: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_connect");
                    let res = impls::wali_connect(caller, sockfd, sockaddr.into(), address_len)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_connect");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_listen",
                |caller: Caller<'_, T>, sockfd: i32, backlog: i32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_listen");
                    let res = impls::wali_listen(caller, sockfd, backlog)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_listen");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_getsockname",
                |caller: Caller<'_, T>,
                 sockfd: i32,
                 sockaddr: u32,
                 address_len: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_getsockname");
                    let res = impls::wali_getsockname(
                        caller,
                        sockfd,
                        sockaddr.into(),
                        address_len.into(),
                    )
                    .map(i64::from)
                    .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_getsockname");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_recvmsg",
                |caller: Caller<'_, T>, sockfd: i32, msg: u32, flags: i32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_recvmsg");
                    let res = impls::wali_recvmsg(caller, sockfd, msg.into(), flags)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_recvmsg");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_sendmsg",
                |caller: Caller<'_, T>, sockfd: i32, msg: u32, flags: i32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_sendmsg");
                    let res = impls::wali_sendmsg(caller, sockfd, msg.into(), flags)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_sendmsg");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_sendto",
                |caller: Caller<'_, T>,
                 sockfd: i32,
                 buf: u32,
                 len: u32,
                 flags: i32,
                 dest_addr: u32,
                 addrlen: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_sendto");
                    let res = impls::wali_sendto(
                        caller,
                        sockfd,
                        buf.into(),
                        len,
                        flags,
                        dest_addr.into(),
                        addrlen,
                    )
                    .map(i64::from)
                    .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_sendto");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_setsockopt",
                |caller: Caller<'_, T>,
                 sockfd: i32,
                 level: i32,
                 optname: i32,
                 optval: u32,
                 optlen: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_setsockopt");
                    let res = impls::wali_setsockopt(
                        caller,
                        sockfd,
                        level,
                        optname,
                        optval.into(),
                        optlen,
                    )
                    .map(i64::from)
                    .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_setsockopt");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_socket",
                |caller: Caller<'_, T>,
                 domain: i32,
                 ty: i32,
                 protocol: i32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_socket");
                    let res = impls::wali_socket(caller, domain, ty, protocol)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_socket");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_poll",
                |caller: Caller<'_, T>, fds: u32, nfds: u32, timeout: i32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_poll");
                    let res = impls::wali_poll(caller, fds.into(), nfds, timeout)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_poll");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_getrusage",
                |caller: Caller<'_, T>, who: i32, usage: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_getrusage");
                    let res = impls::wali_getrusage(caller, who, usage.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_getrusage");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_sched_getaffinity",
                |caller: Caller<'_, T>,
                 pid: i32,
                 cpusetsize: u32,
                 mask: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_sched_getaffinity");
                    let res = impls::wali_sched_getaffinity(caller, pid, cpusetsize, mask.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_sched_getaffinity");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_getdents64",
                |caller: Caller<'_, T>, fd: i32, dirp: u32, count: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_getdents64");
                    let res = impls::wali_getdents64(caller, fd, dirp.into(), count)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_getdents64");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_sysinfo",
                |caller: Caller<'_, T>, sysinfo: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_sysinfo");
                    let res = impls::wali_sysinfo(caller, sysinfo.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_sysinfo");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_prlimit64",
                |caller: Caller<'_, T>,
                 pid: i32,
                 ressource: i32,
                 new_limit: u32,
                 old_limit: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_prlimit64");
                    let res = impls::wali_prlimit64(
                        caller,
                        pid,
                        ressource,
                        new_limit.into(),
                        old_limit.into(),
                    )
                    .map(i64::from)
                    .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_prlimit64");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_getrlimit",
                |caller: Caller<'_, T>, ressource: i32, rlim: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_getrlimit");
                    let res = impls::wali_getrlimit(caller, ressource, rlim.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_getrlimit");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_setpriority",
                |caller: Caller<'_, T>, which: i32, who: i32, prio: i32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_setpriority");
                    let res = impls::wali_setpriority(caller, which, who, prio)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_setpriority");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_chmod",
                |caller: Caller<'_, T>, path: u32, mode: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_chmod");
                    let res = impls::wali_chmod(caller, path.into(), mode)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_chmod");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_chown",
                |caller: Caller<'_, T>, path: u32, owner: u32, group: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_chown");
                    let res = impls::wali_chown(caller, path.into(), owner, group)
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_chown");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_utimensat",
                |caller: Caller<'_, T>,
                 dirfd: i32,
                 path: u32,
                 timespec: u32,
                 flags: i32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_utimensat");
                    let res =
                        impls::wali_utimensat(caller, dirfd, path.into(), timespec.into(), flags)
                            .map(i64::from)
                            .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_utimensat");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_futimesat",
                |caller: Caller<'_, T>,
                 dirfd: i32,
                 path: u32,
                 timespec: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_futimesat");
                    let res = impls::wali_futimesat(caller, dirfd, path.into(), timespec.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_futimesat");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_utimes",
                |caller: Caller<'_, T>, path: u32, timespec: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_utimes");
                    let res = impls::wali_utimes(caller, path.into(), timespec.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_utimes");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_unlink",
                |caller: Caller<'_, T>, path: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_unlink");
                    let res = impls::wali_unlink(caller, path.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_unlink");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_rmdir",
                |caller: Caller<'_, T>, path: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_rmdir");
                    let res = impls::wali_rmdir(caller, path.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_rmdir");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_nanosleep",
                |caller: Caller<'_, T>, timespec: u32, rem: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_nanosleep");
                    let res = impls::wali_nanosleep(caller, timespec.into(), rem.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_nanosleep");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_clock_nanosleep",
                |caller: Caller<'_, T>,
                 clockid: i32,
                 flags: i32,
                 timespec: u32,
                 rem: u32|
                 -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_clock_nanosleep");
                    let res = impls::wali_clock_nanosleep(
                        caller,
                        clockid,
                        flags,
                        timespec.into(),
                        rem.into(),
                    )
                    .map(i64::from)
                    .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_clock_nanosleep");
                    Ok(res)
                },
            )?
            .func_wrap(
                WaliCtxInner::MODULE_NAME,
                "SYS_uname",
                |caller: Caller<'_, T>, utsname: u32| -> Result<i64, Error> {
                    log::debug!("Before '{}'", "SYS_uname");
                    let res = impls::wali_uname(caller, utsname.into())
                        .map(i64::from)
                        .map_err(Error::new)?;
                    log::debug!("After '{}'", "SYS_uname");
                    Ok(res)
                },
            )?;

        Ok(())
    }
}
