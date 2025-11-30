/*
* The code in this file is based on and partly taken from the wasm-bindgen tool:
* https://github.com/rustwasm/wasm-bindgen/blob/dfb9d92efa91a84640cbaebc007c3ebe314c0c8b/crates/threads-xform/src/lib.rs
* https://github.com/rustwasm/wasm-bindgen/blob/73460587d42e7faba0cef672da2bcf754337f384/crates/wasm-conventions/src/lib.rs
*
* Copyright (c) 2014 Alex Crichton
*
* Permission is hereby granted, free of charge, to any
* person obtaining a copy of this software and associated
* documentation files (the "Software"), to deal in the
* Software without restriction, including without
* limitation the rights to use, copy, modify, merge,
* publish, distribute, sublicense, and/or sell copies of
* the Software, and to permit persons to whom the Software
* is furnished to do so, subject to the following
* conditions:
*
* The above copyright notice and this permission notice
* shall be included in all copies or substantial portions
* of the Software.
*
* THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
* ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
* TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
* PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
* SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
* CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
* OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
* IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
* DEALINGS IN THE SOFTWARE.
*/

use anyhow::{Error, anyhow, bail};
use walrus::{
    ConstExpr, ExportItem, FunctionBuilder, FunctionId, GlobalId, GlobalKind, MemoryId, Module,
    ValType, ir::Value,
};

fn get_memory(module: &Module) -> Result<MemoryId, Error> {
    let mut memories = module.memories.iter().map(|m| m.id());
    let memory = memories.next();
    if memories.next().is_some() {
        bail!(
            "expected a single memory, found multiple; multiple memories are currently not supported"
        )
    }
    memory.ok_or_else(|| {
        anyhow!("module does not have a memory; must have a memory to prepare for threading")
    })
}

fn get_stack_pointer(module: &Module) -> Option<GlobalId> {
    if let Some(g) = module
        .globals
        .iter()
        .find(|g| matches!(g.name.as_deref(), Some("__stack_pointer")))
    {
        return Some(g.id());
    }

    let candidates = module
        .globals
        .iter()
        .filter(|g| g.ty == ValType::I32)
        .filter(|g| g.mutable)
        // The stack pointer is guaranteed to not be initialized to 0, and it's
        // guaranteed to have an i32 initializer, so find globals which are
        // locally defined, are an i32, and have a nonzero initializer
        .filter(|g| match g.kind {
            GlobalKind::Local(ConstExpr::Value(Value::I32(n))) => n != 0,
            _ => false,
        })
        .collect::<Vec<_>>();

    match candidates.len() {
        0 => None,
        1 => Some(candidates[0].id()),
        2 => {
            log::warn!("Unable to accurately determine the location of `__stack_pointer`");
            Some(candidates[0].id())
        }
        _ => None,
    }
}

fn delete_synthetic_func(module: &mut Module, name: &str) -> Result<FunctionId, Error> {
    match delete_synthetic_export(module, name)? {
        walrus::ExportItem::Function(f) => Ok(f),
        _ => bail!("`{}` must be a function", name),
    }
}

fn delete_synthetic_global(module: &mut Module, name: &str) -> Result<u32, Error> {
    let id = match delete_synthetic_export(module, name)? {
        walrus::ExportItem::Global(g) => g,
        _ => bail!("`{}` must be a global", name),
    };
    let g = match module.globals.get(id).kind {
        walrus::GlobalKind::Local(g) => g,
        walrus::GlobalKind::Import(_) => bail!("`{}` must not be an imported global", name),
    };
    match g {
        ConstExpr::Value(Value::I32(v)) => Ok(v as u32),
        _ => bail!("`{}` was not an `i32` constant", name),
    }
}

fn delete_synthetic_export(module: &mut Module, name: &str) -> Result<ExportItem, Error> {
    let item = module
        .exports
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| anyhow!("failed to find `{}`", name))?;
    let ret = item.item;
    let id = item.id();
    module.exports.delete(id);
    Ok(ret)
}

pub fn extract_tls_size(module: &mut Module) -> Result<u32, Error> {
    delete_synthetic_global(module, "__tls_size")
}

pub fn extract_tls_align(module: &mut Module) -> Result<u32, Error> {
    delete_synthetic_global(module, "__tls_align")
}

pub fn patch(module: &mut Module) -> Result<&mut Module, Error> {
    inject_instance_entry(module)?;
    Ok(module)
}

fn inject_instance_entry(module: &mut Module) -> Result<(), Error> {
    let thread_start_func = delete_synthetic_func(module, "__wasmgrind_thread_start")?;
    let tls_init_func = delete_synthetic_func(module, "__wasm_init_tls")?;
    let stack_ptr_global =
        get_stack_pointer(module).ok_or_else(|| anyhow!("failed to find stack pointer"))?;

    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::I32, ValType::I32, ValType::I32, ValType::I32],
        &[],
    );

    builder.name("__wasmgrind_instance_entry".into());

    let start_fn_ptr = module.locals.add(ValType::I32);
    let start_fn_arg = module.locals.add(ValType::I32);
    let stack_ptr = module.locals.add(ValType::I32);
    let tls_base_ptr = module.locals.add(ValType::I32);

    builder
        .func_body()
        // First we store the exit code at the specified location
        .local_get(stack_ptr)
        .global_set(stack_ptr_global)
        .local_get(tls_base_ptr)
        .call(tls_init_func)
        .local_get(start_fn_ptr)
        .local_get(start_fn_arg)
        .call(thread_start_func);

    let instance_entry_id = builder.finish(
        vec![start_fn_ptr, start_fn_arg, stack_ptr, tls_base_ptr],
        &mut module.funcs,
    );

    module
        .exports
        .add("__wasmgrind_instance_entry", instance_entry_id);

    Ok(())
}

/// Retrieves the memory limits of a binary WebAssembly module
///
/// The given `module` has to fulfill the following requirements:
/// - It must define _exactly one_ memory.
/// - The memory has to be marked as `shared`.
/// - The memory has to be 32bit addressed.
///
/// The function returns a tuple of memory limits: `(min, max)`.
///
/// # Errors
///
/// This function may fail in the following cases:
/// - The given `module` did not define _exactly one_ memory.
/// - The `module` memory was not marked as `shared`.
/// - The `module` memory was 64bit addressed.
/// - The `module` memory had no maximum size associated with it.
///   (although this is disallowed when the memory is marked as `shared`).
pub fn get_shared_memory_size(module: &Module) -> Result<(u32, u32), Error> {
    let memory_id = get_memory(module)?;
    let memory = module.memories.get(memory_id);
    if !memory.shared {
        bail!("Module memory is not shared!");
    }

    if memory.memory64 {
        bail!("Module memory is 64bit. This is unsupported!");
    }

    let min = u32::try_from(memory.initial)?;
    memory
        .maximum
        .map(u32::try_from)
        .transpose()?
        .map(|max| (min, max))
        .ok_or_else(|| anyhow!("Module memory hand no maximum size specified!"))
}

// ================================================================================================
// We might need this code again in the future:

/*
const ATOMIC_MEM_ARG: MemArg = MemArg {
    align: 4,
    offset: 0,
};

fn find_function(module: &Module, name: &str) -> Result<FunctionId, Error> {
    let e = module
        .exports
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| anyhow!("failed to find `{}`", name))?;
    match e.item {
        walrus::ExportItem::Function(f) => Ok(f),
        _ => bail!("`{}` wasn't a function", name),
    }
}

fn get_tls_base(module: &Module) -> Option<GlobalId> {
    let candidates = module
        .exports
        .iter()
        .filter(|ex| ex.name == "__tls_base")
        .filter_map(|ex| match ex.item {
            walrus::ExportItem::Global(id) => Some(id),
            _ => None,
        })
        .filter(|id| {
            let global = module.globals.get(*id);

            global.ty == ValType::I32
        })
        .collect::<Vec<_>>();

    match candidates.len() {
        1 => Some(candidates[0]),
        _ => None,
    }
}
*/
