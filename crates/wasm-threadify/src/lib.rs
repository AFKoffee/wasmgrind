/*
* The code in this file is mainly based on and taken from the wasm-bindgen tool:
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

use std::cmp;

use anyhow::{Error, anyhow, bail};
use walrus::{
    ConstExpr, ExportItem, FunctionBuilder, FunctionId, FunctionKind, GlobalId, GlobalKind,
    InstrSeqBuilder, MemoryId, Module, ValType,
    ir::{MemArg, Value},
};

const PAGE_SIZE: u32 = 1 << 16;
const DEFAULT_THREAD_STACK_SIZE: u32 = 1 << 21; // 2MB
const ATOMIC_MEM_ARG: MemArg = MemArg {
    align: 4,
    offset: 0,
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

fn get_start(module: &mut Module) -> Result<FunctionId, Option<FunctionId>> {
    match module.start {
        Some(start) => match module.funcs.get_mut(start).kind {
            FunctionKind::Import(_) => Err(Some(start)),
            FunctionKind::Local(_) => Ok(start),
            FunctionKind::Uninitialized(_) => unimplemented!(),
        },
        None => Err(None),
    }
}

/// Patches a WebAssembly module for multithreading support.
pub fn run(module: &mut Module) -> Result<&mut Module, Error> {
    let memory = get_memory(module)?;

    // Now we need to allocate extra static memory for:
    // - A thread id counter.
    // - A temporary stack for calls to `malloc()` and `free()`.
    // - A lock to synchronize usage of the above stack.
    // For this, we allocate 1 extra page of memory (should be enough as temporary
    // stack) and grab the first 2 _aligned_ i32 words to use as counter and lock.
    let static_data_align = 4;
    let static_data_pages = 1;
    let (base, addr) = allocate_static_data(module, memory, static_data_align, static_data_pages)?;

    let mem = module.memories.get(memory);
    assert!(mem.shared);
    assert!(mem.import.is_some());
    assert!(mem.data_segments.is_empty());

    let tls = Tls {
        init: delete_synthetic_func(module, "__wasm_init_tls")?,
        size: delete_synthetic_global(module, "__tls_size")?,
        align: delete_synthetic_global(module, "__tls_align")?,
        base: get_tls_base(module).ok_or_else(|| anyhow!("failed to find tls base"))?,
    };

    let thread_counter_addr = addr as i32;

    let stack_alloc =
        module
            .globals
            .add_local(ValType::I32, true, false, ConstExpr::Value(Value::I32(0)));

    // Make sure the temporary stack is aligned down
    let temp_stack = (base + static_data_pages * PAGE_SIZE) & !(static_data_align - 1);

    const _: () = assert!(DEFAULT_THREAD_STACK_SIZE % PAGE_SIZE == 0);

    let stack = Stack {
        pointer: get_stack_pointer(module)
            .ok_or_else(|| anyhow!("failed to find stack pointer"))?,
        temp: temp_stack as i32,
        temp_lock: thread_counter_addr + 4,
        alloc: stack_alloc,
        size: module.globals.add_local(
            ValType::I32,
            true,
            false,
            ConstExpr::Value(Value::I32(DEFAULT_THREAD_STACK_SIZE as i32)),
        ),
    };

    let _ = module.exports.add("__stack_alloc", stack.alloc);

    inject_start(module, &tls, &stack, thread_counter_addr, memory)?;

    // we expose a `__wbindgen_thread_destroy()` helper function that deallocates stack space.
    //
    // ## Safety
    // After calling this function in a given agent, the instance should be considered
    // "destroyed" and any further invocations into it will trigger UB. This function
    // should not be called from an agent that cannot block (e.g. the main document thread).
    //
    // You can also call it from a "leader" agent, passing appropriate values, if said leader
    // is in charge of cleaning up after a "follower" agent. In that case:
    // - The "appropriate values" are the values of the `__tls_base` and `__stack_alloc` globals
    //   and the stack size from the follower thread, after initialization.
    // - The leader does _not_ need to block.
    // - Similar restrictions apply: the follower thread should be considered unusable afterwards,
    //   the leader should not call this function with the same set of parameters twice.
    // - Moreover, concurrent calls can lead to UB: the follower could be in the middle of a
    //   call while the leader is destroying its stack! You should make sure that this cannot happen.
    inject_destroy(module, &tls, &stack, memory)?;

    Ok(module)
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

fn allocate_static_data(
    module: &mut Module,
    memory: MemoryId,
    pages: u32,
    align: u32,
) -> Result<(u32, u32), Error> {
    // First up, look for a `__heap_base` export which is injected by LLD as
    // part of the linking process. Note that `__heap_base` should in theory be
    // *after* the stack and data, which means it's at the very end of the
    // address space and should be safe for us to inject extra pages of data at.
    let heap_base = module
        .exports
        .iter()
        .filter(|e| e.name == "__heap_base")
        .find_map(|e| match e.item {
            ExportItem::Global(id) => Some(id),
            _ => None,
        });
    let heap_base = match heap_base {
        Some(idx) => idx,
        None => bail!("failed to find `__heap_base` for injecting thread id"),
    };

    // Now we need to bump up `__heap_base` by a few pages. Do lots of validation
    // here to make sure that `__heap_base` is an non-mutable integer, and then do
    // some logic to ensure that the return the correct, aligned `address` as specified
    // by `align`.
    let (base, address) = {
        let global = module.globals.get_mut(heap_base);
        if global.ty != ValType::I32 {
            bail!("the `__heap_base` global doesn't have the type `i32`");
        }
        if global.mutable {
            bail!("the `__heap_base` global is unexpectedly mutable");
        }
        let offset = match &mut global.kind {
            GlobalKind::Local(ConstExpr::Value(Value::I32(n))) => n,
            _ => bail!("`__heap_base` not a locally defined `i32`"),
        };

        let address = (*offset as u32 + (align - 1)) & !(align - 1); // align up
        let base = *offset;

        *offset += (pages * PAGE_SIZE) as i32;

        (base, address)
    };

    let memory = module.memories.get_mut(memory);
    memory.initial += u64::from(pages);
    memory.maximum = memory.maximum.map(|m| cmp::max(m, memory.initial));

    Ok((base as u32, address))
}

struct Tls {
    init: walrus::FunctionId,
    size: u32,
    align: u32,
    base: GlobalId,
}

struct Stack {
    /// The stack pointer global
    pointer: GlobalId,
    /// The address of a small, "scratch-space" stack
    temp: i32,
    /// The address of a lock for the temporary stack
    temp_lock: i32,
    /// A global to store allocated stack
    alloc: GlobalId,
    /// The size of the stack
    size: GlobalId,
}

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

fn inject_start(
    module: &mut Module,
    tls: &Tls,
    stack: &Stack,
    thread_counter_addr: i32,
    memory: MemoryId,
) -> Result<(), Error> {
    use walrus::ir::*;

    let local = module.locals.add(ValType::I32);
    //let thread_count = module.locals.add(ValType::I32);
    //let stack_size = module.locals.add(ValType::I32);

    let malloc = find_function(module, "__wasmgrind_malloc")?;

    let prev_start = get_start(module);
    let mut builder = FunctionBuilder::new(&mut module.types, &[/*ValType::I32*/], &[]);

    if let Ok(prev_start) | Err(Some(prev_start)) = prev_start {
        builder.func_body().call(prev_start);
    }

    let mut body = builder.func_body();

    // Perform an if/else based on whether we're the first thread or not. Our
    // thread ID will be zero if we're the first thread, otherwise it'll be
    // nonzero (assuming we don't overflow...)
    body.i32_const(thread_counter_addr)
        .i32_const(1)
        .atomic_rmw(memory, AtomicOp::Add, AtomicWidth::I32, ATOMIC_MEM_ARG)
        //.local_tee(thread_count)
        .if_else(
            None,
            // If our thread id is nonzero then we're the second or greater thread, so
            // we give ourselves a stack and we update our stack
            // pointer as the default stack pointer is surely wrong for us.
            |body| {
                /*body.local_get(stack_size).if_else(
                    None,
                    |body| {
                        body.local_get(stack_size).global_set(stack.size);
                    },
                    |_| (),
                );*/

                // local = malloc(stack.size, align) [aka base]
                with_temp_stack(body, memory, stack, |body| {
                    body.global_get(stack.size)
                        .i32_const(16)
                        .call(malloc)
                        .local_tee(local);
                });

                // stack.alloc = base
                body.global_set(stack.alloc);

                // stack_pointer = base + stack.size
                body.global_get(stack.alloc)
                    .global_get(stack.size)
                    .binop(BinaryOp::I32Add)
                    .global_set(stack.pointer);
            },
            // If the thread id is zero then the default stack pointer works for
            // us.
            |_| {},
        );

    // Afterwards we need to initialize our thread-local state.
    body.i32_const(tls.size as i32)
        .i32_const(tls.align as i32)
        .call(malloc)
        .global_set(tls.base)
        .global_get(tls.base)
        .call(tls.init);

    let id = builder.finish(vec![/*stack_size*/], &mut module.funcs);
    module.start = Some(id);

    Ok(())
}

fn inject_destroy(
    module: &mut Module,
    tls: &Tls,
    stack: &Stack,
    memory: MemoryId,
) -> Result<(), Error> {
    let free = find_function(module, "__wasmgrind_free")?;

    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::I32, ValType::I32, ValType::I32],
        &[],
    );

    builder.name("__wasmgrind_thread_destroy".into());

    let mut body = builder.func_body();

    // if no explicit parameters are passed (i.e. their value is 0) then we assume
    // we're being called from the agent that must be destroyed and rely on its globals
    let tls_base = module.locals.add(ValType::I32);
    let stack_alloc = module.locals.add(ValType::I32);
    let stack_size = module.locals.add(ValType::I32);

    // Ideally, at this point, we would destroy the values stored in TLS.
    // We can't really do that without help from the standard library.
    // See https://github.com/rustwasm/wasm-bindgen/pull/2769#issuecomment-1015775467.

    body.local_get(tls_base).if_else(
        None,
        |body| {
            body.local_get(tls_base)
                .i32_const(tls.size as i32)
                .i32_const(tls.align as i32)
                .call(free);
        },
        |body| {
            body.global_get(tls.base)
                .i32_const(tls.size as i32)
                .i32_const(tls.align as i32)
                .call(free);

            // set tls.base = i32::MIN to trigger invalid memory
            body.i32_const(i32::MIN).global_set(tls.base);
        },
    );

    // free the stack calling `__wbindgen_free(stack.alloc, stack.size)`
    body.local_get(stack_alloc).if_else(
        None,
        |body| {
            // we're destroying somebody else's stack, so we can use our own
            body.local_get(stack_alloc)
                .local_get(stack_size)
                .i32_const(DEFAULT_THREAD_STACK_SIZE as i32)
                .local_get(stack_size)
                .select(None)
                .i32_const(16)
                .call(free);
        },
        |body| {
            with_temp_stack(body, memory, stack, |body| {
                body.global_get(stack.alloc)
                    .global_get(stack.size)
                    .i32_const(16)
                    .call(free);
            });

            // set stack.alloc = 0 to trigger invalid memory
            body.i32_const(0).global_set(stack.alloc);
        },
    );

    let destroy_id = builder.finish(vec![tls_base, stack_alloc, stack_size], &mut module.funcs);

    module.exports.add("__wasmgrind_thread_destroy", destroy_id);

    Ok(())
}

/// Wraps the instructions fed by `block()` so that they can assume that the temporary, scratch
/// stack is usable. Clobbers `stack.pointer`.
fn with_temp_stack(
    body: &mut InstrSeqBuilder<'_>,
    memory: MemoryId,
    stack: &Stack,
    block: impl Fn(&mut InstrSeqBuilder<'_>),
) {
    use walrus::ir::*;

    body.i32_const(stack.temp).global_set(stack.pointer);

    body.loop_(None, |loop_| {
        let loop_id = loop_.id();

        loop_
            .i32_const(stack.temp_lock)
            .i32_const(0)
            .i32_const(1)
            .cmpxchg(memory, AtomicWidth::I32, ATOMIC_MEM_ARG)
            .if_else(
                None,
                |body| {
                    body.i32_const(stack.temp_lock)
                        .i32_const(1)
                        .i64_const(-1)
                        .atomic_wait(memory, ATOMIC_MEM_ARG, false)
                        .drop()
                        .br(loop_id);
                },
                |_| {},
            );
    });

    block(body);

    body.i32_const(stack.temp_lock)
        .i32_const(0)
        .store(memory, StoreKind::I32 { atomic: true }, ATOMIC_MEM_ARG)
        .i32_const(stack.temp_lock)
        .i32_const(1)
        .atomic_notify(memory, ATOMIC_MEM_ARG)
        .drop();
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
