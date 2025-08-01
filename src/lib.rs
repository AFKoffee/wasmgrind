use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
    thread::JoinHandle,
};

use anyhow::Error;

use crate::runtime::{
    ThreadlinkRuntime, ThreadlinkRuntimeBuilder, WasmgrindRuntime, WasmgrindRuntimeBuilder,
};

/// Defines the different runtimes of Wasmgrind.
pub mod runtime;
mod tmgmt;

fn emit_to_file(wasm: &[u8], name: &str) -> Result<(), Error> {
    let path_buf = PathBuf::from("tmp");
    std::fs::create_dir_all(path_buf.as_path())?;
    let wasm_file = path_buf.join(format!("{name}.wasm"));
    let wat_file = path_buf.join(format!("{name}.wat"));
    std::fs::write(&wasm_file, wasm)?;
    Command::new("wasm-tools")
        .args([
            OsStr::new("print"),
            wasm_file.as_os_str(),
            OsStr::new("-o"),
            wat_file.as_os_str(),
        ])
        .output()?;

    Ok(())
}

fn patch_binary<P: AsRef<Path>>(binary: P, emit_patched: bool) -> Result<Vec<u8>, Error> {
    let wasm = wasmgrind_core::patching::threadify(&std::fs::read(binary)?)?;

    if emit_patched {
        emit_to_file(&wasm, "patched")?;
    }

    Ok(wasm)
}

/// Patches the given WebAssembly binary for multithreading support, performs binary instrumentation
/// and returns a [`WasmgrindRuntimeBuilder`] wrapping the modified binary.
///
/// The use of this function is only recommended if your provided binary imports custom
/// functions other than Wasmgrinds internal ABI. If this is not the case, [`wasmgrind`]
/// serves as a convenience function to create a [`WasmgrindRuntime`] directly.
///
/// The `binary` argument should be a path to a valid WebAssembly module in binary format, which
/// was compiled against the _tracing-extended_ Wasmgrind ABI. For details refer to the
/// [Wasmgrind Book](https://wasmgrind-d6f2b1.gitlab.io/book/user_guide/compiling_the_binary.html).
///
/// If the `emit_patched` flag is set to `true`, the state of the WebAssembly module after
/// patching will be emitted to a `tmp` folder relative to the current working directory.
/// The folder is created if it does not exist yet.
///
/// If the `emit_instrumented` flag is set to `true`, the state of the WebAssembly module after
/// patching and instrumentation will be emitted to a `tmp` folder relative to the current working
/// directory. The folder is created if it does not exist yet.
///
/// # Panics
/// The function will panic if the provided WebAssembly binary could not be patched or instrumented
/// for any reason.
///
/// Refer to the docs or [`wasmgrind_core::patching::threadify`] and
/// [`wasmgrind_core::patching::instrument`] for further details.
///
/// # Examples
/// ```no_run
/// # use anyhow::Error;
/// # fn main() -> Result<(), Error> {
/// // The WebAssembly module "target.wasm" is located inside your working directory.
/// //
/// // It defines a custom import `custom_module` `custom_function`
/// // with the signature (i32, i32) -> i32.
/// //
/// // Furthermore, it exports a parameterless function named `run`.
/// let binary = "target.wasm";
///
/// let builder = wasmgrind::wasmgrind_builder(binary, false, false)?;
/// let runtime = builder
///     .register_custom_import::<(i32, i32), (i32)>(
///         "custom_module",
///         "custom_function",
///         |x, y| {
///             println!("Custom function received two integers: x = {x} and y = {y}!");
///             println!("Returning their sum ...");
///             x + y
///         }
///     )?
///     .build();
///
/// runtime.invoke_function::<(), ()>(String::from("run"), ())
///     .join()
///     .expect("Runner Thread Panicked")?;
/// # Ok(())
/// # }
/// ```
pub fn wasmgrind_builder<P: AsRef<Path>>(
    binary: P,
    emit_patched: bool,
    emit_instrumented: bool,
) -> Result<WasmgrindRuntimeBuilder, Error> {
    let wasm = wasmgrind_core::patching::instrument(&patch_binary(binary, emit_patched)?)?;

    if emit_instrumented {
        emit_to_file(&wasm, "instrumented")?;
    }

    WasmgrindRuntimeBuilder::new(&wasm)
}

/// Creates a [`WasmgrindRuntime`] for a given WebAssembly binary.
///
/// It serves as a shortcut for:
/// `wasmgrind::wasmgrind_builder(binary, emit_patched, emit_instrumented)?.build()`.
///
/// Refer to [`wasmgrind_builder`] for details.
pub fn wasmgrind<P: AsRef<Path>>(
    binary: P,
    emit_patched: bool,
    emit_instrumented: bool,
) -> Result<WasmgrindRuntime, Error> {
    Ok(wasmgrind_builder(binary, emit_patched, emit_instrumented)?.build())
}

/// Creates a [`WasmgrindRuntime`] for a given WebAssembly binary and executes
/// a single exported function of the module.
///
/// The function specified by `name` has to be a parameterless function without
/// any return value that is a valid export of the WebAssembly module.
///
/// It serves as a shortcut for:
/// `wasmgrind::wasmgrind(binary, emit_patched, emit_instrumented)?.invoke_function::<(), ()>(name, ())`.
///
/// Refer to [`wasmgrind`] and [`wasmgrind_builder`] for details.
pub fn grind<P: AsRef<Path>>(
    binary: P,
    name: String,
    emit_patched: bool,
    emit_instrumented: bool,
) -> Result<JoinHandle<Result<(), Error>>, Error> {
    let runtime = wasmgrind(binary, emit_patched, emit_instrumented)?;
    Ok(runtime.invoke_function(name, ()))
}

/// Patches the given WebAssembly binary for multithreading support and returns
/// a [`ThreadlinkRuntimeBuilder`] wrapping the modified binary.
///
/// The use of this function is only recommended if your provided binary imports custom
/// functions other than Wasmgrinds internal ABI. If this is not the case, [`function@runtime`]
/// serves as a convenience function to create a [`ThreadlinkRuntime`] directly.
///
/// The `binary` argument should be a path to a valid WebAssembly module in binary format, which
/// was compiled against the _standalone_ Wasmgrind ABI. For details refer to the
/// [Wasmgrind Book](https://wasmgrind-d6f2b1.gitlab.io/book/user_guide/compiling_the_binary.html).
///
/// If the `emit_patched` flag is set to `true`, the state of the WebAssembly module after
/// patching will be emitted to a `tmp` folder relative to the current working directory.
/// The folder is created if it does not exist yet.
///
/// # Panics
/// The function will panic if the provided WebAssembly binary could not be patched for any reason.
///
/// Refer to the docs or [`wasmgrind_core::patching::threadify`] for further details.
///
/// # Examples
/// ```no_run
/// # use anyhow::Error;
/// # fn main() -> Result<(), Error> {
/// // The WebAssembly module "target.wasm" is located inside your working directory.
/// //
/// // It defines a custom import `custom_module` `custom_function`
/// // with the signature (i32, i32) -> i32.
/// //
/// // Furthermore, it exports a parameterless function named `run`.
/// let binary = "target.wasm";
///
/// let builder = wasmgrind::wasmgrind_builder(binary, false, false)?;
/// let runtime = builder
///     .register_custom_import::<(i32, i32), (i32)>(
///         "custom_module",
///         "custom_function",
///         |x, y| {
///             println!("Custom function received two integers: x = {x} and y = {y}!");
///             println!("Returning their sum ...");
///             x + y
///         }
///     )?
///     .build();
///
/// runtime.invoke_function::<(), ()>(String::from("run"), ())
///     .join()
///     .expect("Runner Thread Panicked")?;
/// # Ok(())
/// # }
/// ```
pub fn runtime_builder<P: AsRef<Path>>(
    binary: P,
    emit_patched: bool,
) -> Result<ThreadlinkRuntimeBuilder, Error> {
    ThreadlinkRuntimeBuilder::new(&patch_binary(binary, emit_patched)?)
}

/// Creates a [`ThreadlinkRuntime`] for a given WebAssembly binary.
///
/// It serves as a shortcut for:
/// `wasmgrind::runtime_builder(binary, emit_patched)?.build()`.
///
/// Refer to [`runtime_builder`] for details.
pub fn runtime<P: AsRef<Path>>(binary: P, emit_patched: bool) -> Result<ThreadlinkRuntime, Error> {
    Ok(runtime_builder(binary, emit_patched)?.build())
}

/// Creates a [`ThreadlinkRuntime`] for a given WebAssembly binary and executes a
/// single exported function of the module.
///
/// The function specified by `name` has to be a parameterless function without
/// any return value that is a valid export of the WebAssembly module.
///
/// It serves as a shortcut for:
/// `wasmgrind::runtime(binary, emit_patched)?.invoke_function::<(), ()>(name, ())`.
///
/// Refer to [`function@runtime`] and [`runtime_builder`] for details.
pub fn run<P: AsRef<Path>>(
    binary: P,
    name: String,
    emit_patched: bool,
) -> Result<JoinHandle<Result<(), Error>>, Error> {
    let runtime = runtime(binary, emit_patched)?;
    Ok(runtime.invoke_function(name, ()))
}
