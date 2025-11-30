use std::{
    ffi::{CString, c_char},
    str::FromStr,
    sync::atomic::Ordering,
};

use wasmtime::Caller;

use crate::{
    WaliResult, WaliView,
    ctx::utils::mem::{self, NativePtr, WasmPtr},
};

#[inline]
pub fn wali_get_init_envfile<T: WaliView>(
    mut caller: Caller<'_, T>,
    faddr: WasmPtr,
    fsize: u32,
) -> WaliResult<i32> {
    let native_ptr = NativePtr::from_wasm_ptr(&mut caller, faddr);
    let envfile_name = format!("/tmp/wali.env.{}", unsafe { nc::getpid() });
    let envfile = if let Err(e) = std::fs::OpenOptions::new().read(true).open(&envfile_name) {
        match e.kind() {
            std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied => {
                &caller.data().ctx().0.wali_app_env_file
            }
            _ => panic!("Unexpected io-error while trying to access WALI env file:\n{e}"),
        }
    } else {
        &Some(envfile_name)
    };

    let retval = if let Some(envfile_name) = envfile {
        let c_envfile =
            CString::from_str(envfile_name).expect("Could not convert Rust String to C String");
        if c_envfile.count_bytes() + 1
            > usize::try_from(fsize).expect("filename size must fit into 'usize'")
        {
            log::error!(
                "WALI env initialization filepath too large (max length: {fsize}). Defaulting to NULL"
            );
            unsafe { std::ptr::write(native_ptr.raw::<c_char>(), 0) };
        } else {
            unsafe {
                libc::strcpy(native_ptr.raw(), c_envfile.as_ptr());
            }
            log::info!("WALI init env file: '{envfile_name}'");
        }

        1
    } else {
        log::warn!("No WALI environment file provided");
        0
    };

    caller.data().ctx().return_or_exit(retval)
}

#[inline]
pub fn wali_deinit<T: WaliView>(caller: Caller<'_, T>) {
    let wali_ctx = &caller.data().ctx();
    if wali_ctx.0.wali_deinit_called.load(Ordering::Acquire) {
        panic!("WALI __deinit was called multiple times!");
    }

    wali_ctx.0.wali_deinit_called.store(true, Ordering::Release);
}

#[inline]
pub fn wali_init<T: WaliView>(caller: Caller<'_, T>) {
    let wali_ctx = &caller.data().ctx();
    if wali_ctx.0.wali_init_called.load(Ordering::Acquire) {
        panic!("WALI __init was called multiple times!");
    }

    wali_ctx.0.wali_init_called.store(true, Ordering::Release);
}

#[inline]
pub fn wali_cl_get_argc<T: WaliView>(caller: Caller<'_, T>) -> WaliResult<u32> {
    let wali_ctx = &caller.data().ctx();

    let argc = wali_ctx.0.wali_cl_args.len();

    wali_ctx.return_or_exit(
        argc as u32, /* ATTENTION: Possible loss of information (unlikely) */
    )
}

#[inline]
pub fn wali_cl_get_argv_len<T: WaliView>(caller: Caller<'_, T>, arg_idx: u32) -> WaliResult<u32> {
    let wali_ctx = &caller.data().ctx();

    let idx = usize::try_from(arg_idx).expect("Could not index argv array");
    let argv_len = wali_ctx.0.wali_cl_args[idx].count_bytes();

    wali_ctx.return_or_exit(
        argv_len as u32, /* ATTENTION: Possible loss of information (unlikely) */
    )
}

#[inline]
pub fn wali_cl_copy_argv<T: WaliView>(
    mut caller: Caller<'_, T>,
    arg_dst: WasmPtr,
    arg_idx: u32,
) -> WaliResult<u32> {
    let dst_ptr = mem::NativePtr::from_wasm_ptr(&mut caller, arg_dst);
    let wali_ctx = &caller.data().ctx();

    let idx = usize::try_from(arg_idx).expect("Could not index argv array");
    let arg = &wali_ctx.0.wali_cl_args[idx];
    unsafe {
        libc::strcpy(dst_ptr.raw(), arg.as_ptr());
    }

    wali_ctx.return_or_exit(0)
}
