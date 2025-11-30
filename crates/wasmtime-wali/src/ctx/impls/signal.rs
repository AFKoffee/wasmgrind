use std::{
    mem::MaybeUninit,
    ptr::{self, NonNull},
};

use nc::sigset_t;
use wasmtime::Caller;

use crate::{
    WaliResult, WaliView,
    ctx::impls::SyscallResult,
    ctx::utils::{
        exports::get_ifp_from_caller,
        mem::{NativePtr, WasmPtr},
    },
    systypes::wasm_sigaction_t,
};

#[inline]
pub fn wali_rt_sigprocmask<T: WaliView>(
    mut caller: Caller<'_, T>,
    how: i32,
    set: WasmPtr,
    oldset: WasmPtr,
    sigsetsize: u32,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe {
        nc::syscalls::syscall4(
            nc::SYS_RT_SIGPROCMASK,
            how as usize,
            NativePtr::from_wasm_ptr(&mut caller, set).addr(),
            NativePtr::from_wasm_ptr(&mut caller, oldset).addr(),
            sigsetsize as usize,
        )
    };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_tkill<T: WaliView>(
    caller: Caller<'_, T>,
    tid: i32,
    sig: i32,
) -> WaliResult<SyscallResult<usize>> {
    let retval = unsafe { nc::syscalls::syscall2(nc::SYS_TKILL, tid as usize, sig as usize) };

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}

#[inline]
pub fn wali_rt_sigaction<T: WaliView>(
    mut caller: Caller<'_, T>,
    signum: i32,
    act: WasmPtr,
    oldact: WasmPtr,
    sigsetsize: u32,
) -> WaliResult<SyscallResult<()>> {
    assert_eq!(
        std::mem::size_of::<sigset_t>(),
        sigsetsize as usize,
        "native 'sigset_t' type and 'sigsetsize' param do not match!"
    );
    assert_eq!(
        std::mem::size_of::<sigset_t>(),
        std::mem::size_of::<[u32; 2]>(),
        "'sigset_t' types of Wasm and Native sides are not equal sized!"
    );

    let wasm_act_ptr = NativePtr::from_wasm_ptr(&mut caller, act).raw::<wasm_sigaction_t>();
    let wasm_oldact_ptr = NativePtr::from_wasm_ptr(&mut caller, oldact).raw::<wasm_sigaction_t>();

    // We clone the Arc here as we need Caller later
    let data = caller.data().clone();
    let wali_ctx = data.ctx();
    let mut wali_sigtable = wali_ctx
        .0
        .sigtable
        .lock()
        .expect("WALI sigtable lock was poisoned!");

    let mut target_wasm_funcptr = MaybeUninit::uninit();
    let act = NonNull::new(wasm_act_ptr).map(|wasm_act_ptr| {
        let wasm_sigaction = unsafe { wasm_act_ptr.as_ref() };
        let sa_handler: nc::sighandler_t = match wasm_sigaction.wasm_handler {
            wasm_sigaction_t::WASM_SIG_DFL => nc::SIG_DFL,
            wasm_sigaction_t::WASM_SIG_IGN => nc::SIG_IGN,
            wasm_sigaction_t::WASM_SIG_ERR => nc::SIG_ERR,
            _ => {
                target_wasm_funcptr.write(wasm_sigaction.wasm_handler);
                wali_ctx.0.sighandler as nc::sighandler_t
            }
        };

        let sa_mask: sigset_t = sigset_t {
            sig: unsafe {
                // Relevant assertions for this transmutation are at the beginning of this function
                std::mem::transmute::<[u32; 2], [usize; 1]>(wasm_sigaction.mask)
            },
        };

        nc::sigaction_t {
            sa_handler,
            sa_flags: wasm_sigaction.flags as usize,
            sa_restorer: nc::restore::get_sa_restorer(),
            sa_mask,
        }
    });

    let mut oldact = if wasm_oldact_ptr.is_null() {
        None
    } else {
        Some(nc::sigaction_t::default())
    };

    let retval = unsafe { nc::rt_sigaction(signum, act.as_ref(), oldact.as_mut()) };
    if retval.is_ok() {
        if let Some(oldact) = oldact {
            // kernel syscall returned successfully, so we can assume
            // that the default values have been replaced by the actual values
            let nc::sigaction_t {
                sa_handler,
                sa_flags,
                sa_restorer,
                sa_mask,
            } = oldact;

            let wasm_handler = match sa_handler {
                nc::SIG_DFL => wasm_sigaction_t::WASM_SIG_DFL,
                nc::SIG_IGN => wasm_sigaction_t::WASM_SIG_IGN,
                nc::SIG_ERR => wasm_sigaction_t::WASM_SIG_ERR,
                _ => wali_sigtable
                    .get_handler_table_idx(signum)
                    .expect("Sigaction handler should have been set previously"),
            };

            // Relevant assertions for this transmutation are at the beginning of this function
            let mask: [u32; 2] =
                unsafe { std::mem::transmute::<[usize; 1], [u32; 2]>(sa_mask.sig) };

            let wasm_oldact = wasm_sigaction_t {
                wasm_handler,
                flags: sa_flags as u64,
                // ATTENTION: Reference implementation does a similar thing.
                //
                // A little bit of explaination:
                // The "restorer" callback from Wasm will actually never be used as we replace
                // the restorer with 'nc::restore::get_sa_restorer()' when passing through the `SYS_rt_sigaction` call.
                //
                // So, either 'sa_restorer' is a pointer to 'nc::restore::get_sa_restorer()' if the sigaction was set by
                // our passthrough implementation or it is an unknown restore handler, which has been set previously.
                //
                // Anyway, as long as the Wasm program does not use it or rely on it in any way. This should be
                // fine - although it seems a bit dangerous and could probably be refined.
                wasm_restorer: sa_restorer
                    .map(|restorer_fn| restorer_fn as u32)
                    .unwrap_or(0),
                mask,
            };

            // We only create the MaybeUninit struct if wasm_oldact_ptr is non-null
            // So, if we made it till here, we can write to the pointer safely.
            unsafe { ptr::write(wasm_oldact_ptr, wasm_oldact) };
        }

        if let Some(act) = act
            && ![nc::SIG_DFL, nc::SIG_IGN, nc::SIG_ERR].contains(&act.sa_handler)
        {
            // If we created a nc::sigaction_t struct successfully,
            // the target_wasm_funcptr has been initialized as well.
            let wasm_handler_funcptr = unsafe { target_wasm_funcptr.assume_init() };
            let get_indirect_func = get_ifp_from_caller(&mut caller);
            let wasm_handler = get_indirect_func
                .call(&mut caller, wasm_handler_funcptr)
                .expect("Could not get funcref for wasm_handler_funcptr")
                .expect("Funcref for wasm_handler_funcptr was null")
                .typed(&caller)
                .expect("Wasm Signal handler was of wrong function type");
            wali_sigtable.update(signum, wasm_handler, wasm_handler_funcptr);
        }

        drop(wali_sigtable);
    }

    caller
        .data()
        .ctx()
        .return_or_exit(SyscallResult::from(retval))
}
