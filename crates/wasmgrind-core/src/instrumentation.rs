use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use anyhow::{Error, bail};
use rayon::iter::ParallelIterator;
use walrus::{
    FunctionBuilder, FunctionId, Import, InstrLocId, InstrSeqBuilder, LocalFunction, LocalId,
    Module, ModuleLocals, ModuleTypes, TypeId, ValType,
    ir::{
        AtomicRmw, AtomicWait, BinaryOp, Block, Call, Cmpxchg, Const, IfElse, Instr, Load, Loop,
        MemoryCopy, MemoryFill, MemoryInit, Store, Value,
    },
};

struct ReusableLocalProvider<'mutex, 'module> {
    module_locals: &'mutex Mutex<&'module mut ModuleLocals>,
    locals: HashMap<ValType, Vec<LocalId>>,
}

impl<'mutex, 'module> ReusableLocalProvider<'mutex, 'module> {
    fn new(module_locals: &'mutex Mutex<&'module mut ModuleLocals>) -> Self {
        Self {
            module_locals,
            locals: HashMap::new(),
        }
    }

    fn get<'me, const N: usize>(
        &'me mut self,
        types: [ValType; N],
    ) -> ReusableLocals<'me, 'mutex, 'module, N> {
        let mut locals = Vec::new();
        for ty in types {
            let local = self
                .locals
                .get_mut(&ty)
                .and_then(|typed_locals| typed_locals.pop())
                .unwrap_or(
                    self.module_locals
                        .lock()
                        .expect("Locals Lock poisoned!")
                        .add(ty),
                );
            locals.push((ty, local));
        }

        ReusableLocals {
            provider: self,
            locals: locals
                .try_into()
                .expect("Local generation should be always valid!"),
        }
    }
}

struct ReusableLocals<'provider, 'a, 'module, const N: usize> {
    provider: &'provider mut ReusableLocalProvider<'a, 'module>,
    locals: [(ValType, LocalId); N],
}

impl<'provider, 'mutex, 'module, const N: usize> ReusableLocals<'provider, 'mutex, 'module, N> {
    fn locals(&self) -> &[(ValType, LocalId); N] {
        &self.locals
    }
}

impl<'provider, 'mutex, 'module, const N: usize> Drop
    for ReusableLocals<'provider, 'mutex, 'module, N>
{
    fn drop(&mut self) {
        // Reclaim the locals on drop
        // This way we can reuse the same locals throughout the whole function
        for (ty, local) in self.locals {
            self.provider.locals.entry(ty).or_default().push(local);
        }
    }
}

struct WasmgrindInstrumentation<'mutex, 'context, 'module> {
    context: &'context InstrumentationContext,
    /// This should be the byte-offset of the first instruction
    /// of the function that is currently instrumented
    function_loc: InstrLocId,
    local_provider: ReusableLocalProvider<'mutex, 'module>,
}

impl<'mutex, 'context, 'module> WasmgrindInstrumentation<'mutex, 'context, 'module> {
    const ACCESS_WIDTH_32BIT: u32 = 4;
    const ACCESS_WIDTH_64BIT: u32 = 8;
    const ATOMIC_ACCESS: i32 = 1;
    const NON_ATOMIC_ACCESS: i32 = 0;

    fn new(
        context: &'context InstrumentationContext,
        locals: &'mutex Mutex<&'module mut ModuleLocals>,
    ) -> Self {
        Self {
            context,
            function_loc: InstrLocId::default(),
            local_provider: ReusableLocalProvider::new(locals),
        }
    }

    fn process_function(&mut self, func: &mut LocalFunction) {
        let start_seq_id = func.entry_block();
        let start_seq = func.block(start_seq_id);
        let func_loc = start_seq
            .first()
            .map(|(_, loc)| loc)
            .unwrap_or(&start_seq.end);
        self.function_loc = *func_loc;

        let mut stack = vec![start_seq_id];
        while let Some(seq_id) = stack.pop() {
            let mut seq = func.builder_mut().instr_seq(seq_id);
            // This is actually a bit dangerous:
            // We mutate the Vector in-place while iterating over it.
            // A few things are important here:
            // - The index always has to be incremented by the amount of
            //   instructions inserted into the sequence such that we do not instrument our instrumentation code recursively
            // - The loop is unconditional and breaks once we reach the end of the sequence.
            //   The length is queried dynamically on each iteration to account for changes in sequence length
            let mut i = 0;
            loop {
                if i >= seq.instrs().len() {
                    break;
                }

                let (instr, loc) = &seq.instrs()[i];
                match instr {
                    Instr::Block(Block { seq }) | Instr::Loop(Loop { seq }) => {
                        stack.push(*seq);
                    }
                    Instr::IfElse(IfElse {
                        consequent,
                        alternative,
                    }) => {
                        stack.push(*alternative);
                        stack.push(*consequent);
                    }
                    Instr::Call(call) => {
                        self.instrument_call(call.clone(), *loc, &mut seq, &mut i);
                    }
                    Instr::CallIndirect(_) => {
                        // TODO: Implement this
                        //
                        // Problem:
                        // How do we find out whether the called function is part of 'self.context.external_hooks'?
                        // Will this be a type signature mismatch error at runtime?
                    }
                    Instr::MemoryInit(memory_init) => {
                        self.instrument_memory_init(memory_init.clone(), *loc, &mut seq, &mut i);
                    }
                    Instr::MemoryCopy(memory_copy) => {
                        self.instrument_memory_copy(memory_copy.clone(), *loc, &mut seq, &mut i);
                    }
                    Instr::MemoryFill(memory_fill) => {
                        self.instrument_memory_fill(memory_fill.clone(), *loc, &mut seq, &mut i);
                    }
                    Instr::Load(load) => {
                        self.instrument_load(load.clone(), *loc, &mut seq, &mut i);
                    }
                    Instr::Store(store) => {
                        self.instrument_store(store.clone(), *loc, &mut seq, &mut i);
                    }
                    Instr::AtomicRmw(atomic_rmw) => {
                        self.instrument_rmw(atomic_rmw.clone(), *loc, &mut seq, &mut i);
                    }
                    Instr::Cmpxchg(cmpxchg) => {
                        self.instrument_cmpxchg(cmpxchg.clone(), *loc, &mut seq, &mut i);
                    }
                    Instr::AtomicWait(atomic_wait) => {
                        self.instrument_atomic_wait(atomic_wait.clone(), *loc, &mut seq, &mut i);
                    }
                    Instr::AtomicNotify(_) => (), // We do not notify this as it does not access memory: https://webassembly.github.io/threads/core/exec/instructions.html#xref-syntax-instructions-syntax-instr-atomic-memory-mathsf-memory-atomic-notify-xref-syntax-instructions-syntax-memarg-mathit-memarg
                    _ => {}
                }

                i += 1;
            }
        }
    }

    fn instrument_call<'a>(
        &mut self,
        call: Call,
        instr_loc_id: InstrLocId,
        seq: &mut InstrSeqBuilder<'a>,
        idx: &mut usize,
    ) {
        // We only need to add the location parameters if we call one of our hooks
        //
        // IMPORTANT:
        // Their signatures have to be patched first (see InstrumentationContext::patch_hook_signatures)
        if self.context.external_hooks.contains(&call.func) {
            // NOTE: We insert the instructions backwards here so we can use the same index over and over again
            seq.instr_at(
                *idx,
                Instr::Const(Const {
                    value: Value::I32(instr_loc_id.data() as i32),
                }),
            );
            seq.instr_at(
                *idx,
                Instr::Const(Const {
                    value: Value::I32(self.function_loc.data() as i32),
                }),
            );
            *idx += 2; // We added 2 instructions in total
        }
    }

    fn instrument_memory_init<'a>(
        &mut self,
        _memory_init: MemoryInit,
        instr_loc_id: InstrLocId,
        seq: &mut InstrSeqBuilder<'a>,
        idx: &mut usize,
    ) {
        let reusable_locals = self
            .local_provider
            .get([ValType::I32, ValType::I32, ValType::I32]);
        let [(_, dst_addr_tmp), (_, data_offset_tmp), (_, n_bytes_tmp)] = &reusable_locals.locals();

        // NOTE: We insert the instructions backwards here so we can use the same index over and over again
        seq
            // These are instructions BEFORE the original instruction
            .local_get_at(*idx, *n_bytes_tmp)
            .local_get_at(*idx, *data_offset_tmp)
            .local_tee_at(*idx, *dst_addr_tmp)
            .local_set_at(*idx, *data_offset_tmp)
            .local_set_at(*idx, *n_bytes_tmp)
            // These are instructions AFTER the original instruction
            .call_at(*idx + 6, self.context.write_hook)
            .const_at(*idx + 6, Value::I32(instr_loc_id.data() as i32))
            .const_at(*idx + 6, Value::I32(self.function_loc.data() as i32))
            .const_at(*idx + 6, Value::I32(Self::NON_ATOMIC_ACCESS))
            .local_get_at(*idx + 6, *n_bytes_tmp)
            .local_get_at(*idx + 6, *dst_addr_tmp);

        *idx += 11; // We added 11 instructions in total
    }

    fn instrument_memory_copy<'a>(
        &mut self,
        _memory_copy: MemoryCopy,
        instr_loc_id: InstrLocId,
        seq: &mut InstrSeqBuilder<'a>,
        idx: &mut usize,
    ) {
        let reusable_locals = self
            .local_provider
            .get([ValType::I32, ValType::I32, ValType::I32]);
        let [(_, dst_addr_tmp), (_, src_addr_tmp), (_, n_bytes_tmp)] = &reusable_locals.locals();

        // NOTE: We insert the instructions backwards here so we can use the same index over and over again
        seq
            // These are instructions BEFORE the original instruction
            .local_get_at(*idx, *n_bytes_tmp)
            .local_get_at(*idx, *src_addr_tmp)
            .local_tee_at(*idx, *dst_addr_tmp)
            .local_set_at(*idx, *src_addr_tmp)
            .local_set_at(*idx, *n_bytes_tmp)
            // These are instructions AFTER the original instruction
            .call_at(*idx + 6, self.context.write_hook)
            .const_at(*idx + 6, Value::I32(instr_loc_id.data() as i32))
            .const_at(*idx + 6, Value::I32(self.function_loc.data() as i32))
            .const_at(*idx + 6, Value::I32(Self::NON_ATOMIC_ACCESS))
            .local_get_at(*idx + 6, *n_bytes_tmp)
            .local_get_at(*idx + 6, *dst_addr_tmp)
            .call_at(*idx + 6, self.context.read_hook)
            .const_at(*idx + 6, Value::I32(instr_loc_id.data() as i32))
            .const_at(*idx + 6, Value::I32(self.function_loc.data() as i32))
            .const_at(*idx + 6, Value::I32(Self::NON_ATOMIC_ACCESS))
            .local_get_at(*idx + 6, *n_bytes_tmp)
            .local_get_at(*idx + 6, *src_addr_tmp);

        *idx += 17; // We added 17 instructions in total
    }

    fn instrument_memory_fill<'a>(
        &mut self,
        _memory_fill: MemoryFill,
        instr_loc_id: InstrLocId,
        seq: &mut InstrSeqBuilder<'a>,
        idx: &mut usize,
    ) {
        let reusable_locals = self
            .local_provider
            .get([ValType::I32, ValType::I32, ValType::I32]);
        let [(_, dst_addr_tmp), (_, byte_value_tmp), (_, n_bytes_tmp)] = &reusable_locals.locals();

        // NOTE: We insert the instructions backwards here so we can use the same index over and over again
        seq
            // These are instructions BEFORE the original instruction
            .local_get_at(*idx, *n_bytes_tmp)
            .local_get_at(*idx, *byte_value_tmp)
            .local_tee_at(*idx, *dst_addr_tmp)
            .local_set_at(*idx, *byte_value_tmp)
            .local_set_at(*idx, *n_bytes_tmp)
            // These are instructions AFTER the original instruction
            .call_at(*idx + 6, self.context.write_hook)
            .const_at(*idx + 6, Value::I32(instr_loc_id.data() as i32))
            .const_at(*idx + 6, Value::I32(self.function_loc.data() as i32))
            .const_at(*idx + 6, Value::I32(Self::NON_ATOMIC_ACCESS))
            .local_get_at(*idx + 6, *n_bytes_tmp)
            .local_get_at(*idx + 6, *dst_addr_tmp);

        *idx += 11; // We added 11 instructions in total
    }

    fn instrument_load<'a>(
        &mut self,
        load: Load,
        instr_loc_id: InstrLocId,
        seq: &mut InstrSeqBuilder<'a>,
        idx: &mut usize,
    ) {
        if let walrus::ir::LoadKind::V128 = load.kind {
            // Unsupported: We do not instrument this ...
            return;
        }

        let is_atomic = if load.kind.atomic() {
            Self::ATOMIC_ACCESS
        } else {
            Self::NON_ATOMIC_ACCESS
        };

        let reusable_locals = self.local_provider.get([ValType::I32]);
        let [(_, addr_tmp)] = &reusable_locals.locals();

        // NOTE: We insert the instructions backwards here so we can use the same index over and over again
        seq
            // These are instructions BEFORE the original instruction
            .local_tee_at(*idx, *addr_tmp)
            // These are instructions AFTER the original instruction
            .call_at(*idx + 2, self.context.read_hook)
            .const_at(*idx + 2, Value::I32(instr_loc_id.data() as i32))
            .const_at(*idx + 2, Value::I32(self.function_loc.data() as i32))
            .const_at(*idx + 2, Value::I32(is_atomic))
            .const_at(*idx + 2, Value::I32(load.kind.width() as i32))
            .binop_at(*idx + 2, BinaryOp::I32Add)
            .const_at(*idx + 2, Value::I32(load.arg.offset as i32))
            .local_get_at(*idx + 2, *addr_tmp);

        *idx += 9; // We added 9 instructions in total
    }

    fn instrument_store<'a>(
        &mut self,
        store: Store,
        instr_loc_id: InstrLocId,
        seq: &mut InstrSeqBuilder<'a>,
        idx: &mut usize,
    ) {
        let val_type = match store.kind {
            walrus::ir::StoreKind::I32 { atomic: _ }
            | walrus::ir::StoreKind::I32_8 { atomic: _ }
            | walrus::ir::StoreKind::I32_16 { atomic: _ } => ValType::I32,
            walrus::ir::StoreKind::I64 { atomic: _ }
            | walrus::ir::StoreKind::I64_8 { atomic: _ }
            | walrus::ir::StoreKind::I64_16 { atomic: _ }
            | walrus::ir::StoreKind::I64_32 { atomic: _ } => ValType::I64,
            walrus::ir::StoreKind::F32 => ValType::F32,
            walrus::ir::StoreKind::F64 => ValType::F64,
            walrus::ir::StoreKind::V128 => {
                // Unsupported: We do not instrument this ...
                seq.instr(store.clone());
                return;
            }
        };

        let is_atomic = if store.kind.atomic() {
            Self::ATOMIC_ACCESS
        } else {
            Self::NON_ATOMIC_ACCESS
        };

        let reusable_locals = self.local_provider.get([ValType::I32, val_type]);
        let [(_, addr_tmp), (_, value_tmp)] = &reusable_locals.locals();

        // NOTE: We insert the instructions backwards here so we can use the same index over and over again
        seq
            // These are instructions BEFORE the original instruction
            .local_get_at(*idx, *value_tmp)
            .local_tee_at(*idx, *addr_tmp)
            .local_set_at(*idx, *value_tmp)
            // These are instructions AFTER the original instruction
            .call_at(*idx + 4, self.context.write_hook)
            .const_at(*idx + 4, Value::I32(instr_loc_id.data() as i32))
            .const_at(*idx + 4, Value::I32(self.function_loc.data() as i32))
            .const_at(*idx + 4, Value::I32(is_atomic))
            .const_at(*idx + 4, Value::I32(store.kind.width() as i32))
            .binop_at(*idx + 4, BinaryOp::I32Add)
            .const_at(*idx + 4, Value::I32(store.arg.offset as i32))
            .local_get_at(*idx + 4, *addr_tmp);

        *idx += 11; // We added 11 instructions in total
    }

    fn instrument_rmw<'a>(
        &mut self,
        rmw: AtomicRmw,
        instr_loc_id: InstrLocId,
        seq: &mut InstrSeqBuilder<'a>,
        idx: &mut usize,
    ) {
        let val_type = match rmw.width {
            walrus::ir::AtomicWidth::I32
            | walrus::ir::AtomicWidth::I32_8
            | walrus::ir::AtomicWidth::I32_16 => ValType::I32,
            walrus::ir::AtomicWidth::I64
            | walrus::ir::AtomicWidth::I64_8
            | walrus::ir::AtomicWidth::I64_16
            | walrus::ir::AtomicWidth::I64_32 => ValType::I64,
        };

        let reusable_locals = self.local_provider.get([ValType::I32, val_type]);
        let [(_, addr_tmp), (_, value_tmp)] = &reusable_locals.locals();

        // NOTE: We insert the instructions backwards here so we can use the same index over and over again
        seq
            // These are instructions BEFORE the original instruction
            .local_get_at(*idx, *value_tmp)
            .local_tee_at(*idx, *addr_tmp)
            .local_set_at(*idx, *value_tmp)
            // These are instructions AFTER the original instruction
            .call_at(*idx + 4, self.context.write_hook)
            .const_at(*idx + 4, Value::I32(instr_loc_id.data() as i32))
            .const_at(*idx + 4, Value::I32(self.function_loc.data() as i32))
            .const_at(*idx + 4, Value::I32(Self::ATOMIC_ACCESS))
            .const_at(*idx + 4, Value::I32(rmw.width.bytes() as i32))
            .binop_at(*idx + 4, BinaryOp::I32Add)
            .const_at(*idx + 4, Value::I32(rmw.arg.offset as i32))
            .local_get_at(*idx + 4, *addr_tmp)
            .call_at(*idx + 4, self.context.read_hook)
            .const_at(*idx + 4, Value::I32(instr_loc_id.data() as i32))
            .const_at(*idx + 4, Value::I32(self.function_loc.data() as i32))
            .const_at(*idx + 4, Value::I32(Self::ATOMIC_ACCESS))
            .const_at(*idx + 4, Value::I32(rmw.width.bytes() as i32))
            .binop_at(*idx + 4, BinaryOp::I32Add)
            .const_at(*idx + 4, Value::I32(rmw.arg.offset as i32))
            .local_get_at(*idx + 4, *addr_tmp);

        *idx += 19; // We added 19 instructions in total
    }

    fn instrument_cmpxchg<'a>(
        &mut self,
        cmpxchg: Cmpxchg,
        instr_loc_id: InstrLocId,
        seq: &mut InstrSeqBuilder<'a>,
        idx: &mut usize,
    ) {
        let val_type = match cmpxchg.width {
            walrus::ir::AtomicWidth::I32
            | walrus::ir::AtomicWidth::I32_8
            | walrus::ir::AtomicWidth::I32_16 => ValType::I32,
            walrus::ir::AtomicWidth::I64
            | walrus::ir::AtomicWidth::I64_8
            | walrus::ir::AtomicWidth::I64_16
            | walrus::ir::AtomicWidth::I64_32 => ValType::I64,
        };

        let reusable_locals = self
            .local_provider
            .get([ValType::I32, val_type, val_type, val_type]);
        let [
            (_, addr_tmp),
            (_, expected_tmp),
            (_, replacement_tmp),
            (_, returned_tmp),
        ] = &reusable_locals.locals();

        // NOTE: We insert the instructions backwards here so we can use the same index over and over again
        // This is especially confusing here as we use if-else-blocks and matches. Look out!
        seq
            // These are instructions BEFORE the original instruction
            .local_get_at(*idx, *replacement_tmp)
            .local_get_at(*idx, *expected_tmp)
            .local_tee_at(*idx, *addr_tmp)
            .local_set_at(*idx, *expected_tmp)
            .local_set_at(*idx, *replacement_tmp);

        // These are instructions AFTER the original instruction
        seq.if_else_at(
            *idx + 6,
            None,
            |then| {
                // NOTE: The instructions in the sub-sequence are NOT added in reverse
                // ==> Walrus internally creates a fresh sequence that is referenced by the if-else-blocks so no need for indexing
                then.local_get(*addr_tmp)
                    .i32_const(cmpxchg.arg.offset as i32)
                    .binop(BinaryOp::I32Add)
                    .i32_const(cmpxchg.width.bytes() as i32)
                    .i32_const(Self::ATOMIC_ACCESS)
                    .i32_const(self.function_loc.data() as i32)
                    .i32_const(instr_loc_id.data() as i32)
                    .call(self.context.write_hook);
            },
            |_| {},
        );

        match val_type {
            ValType::I32 => {
                seq.binop_at(*idx + 6, BinaryOp::I32Eq);
            }
            ValType::I64 => {
                seq.binop_at(*idx + 6, BinaryOp::I64Eq);
            }
            _ => unreachable!("memory.atomic.cmpxchg instruction only takes i32 or i64 arguments"),
        }

        seq.local_get_at(*idx + 6, *expected_tmp)
            .local_get_at(*idx + 6, *returned_tmp)
            .local_tee_at(*idx + 6, *returned_tmp)
            .call_at(*idx + 6, self.context.read_hook)
            .const_at(*idx + 6, Value::I32(instr_loc_id.data() as i32))
            .const_at(*idx + 6, Value::I32(self.function_loc.data() as i32))
            .const_at(*idx + 6, Value::I32(Self::ATOMIC_ACCESS))
            .const_at(*idx + 6, Value::I32(cmpxchg.width.bytes() as i32))
            .binop_at(*idx + 6, BinaryOp::I32Add)
            .const_at(*idx + 6, Value::I32(cmpxchg.arg.offset as i32))
            .local_get_at(*idx + 6, *addr_tmp);

        *idx += 18; // We added 18 instructions in total
    }

    fn instrument_atomic_wait<'a>(
        &mut self,
        atomic_wait: AtomicWait,
        instr_loc_id: InstrLocId,
        seq: &mut InstrSeqBuilder<'a>,
        idx: &mut usize,
    ) {
        let (val_type, access_width) = if atomic_wait.sixty_four {
            (ValType::I64, Self::ACCESS_WIDTH_64BIT)
        } else {
            (ValType::I32, Self::ACCESS_WIDTH_32BIT)
        };

        let reusable_locals = self
            .local_provider
            .get([ValType::I32, val_type, ValType::I64]);
        let [(_, addr_tmp), (_, expected_tmp), (_, timeout_tmp)] = &reusable_locals.locals();

        // NOTE: We insert the instructions backwards here so we can use the same index over and over again
        seq
            // These are instructions BEFORE the original instruction
            .local_get_at(*idx, *timeout_tmp)
            .local_get_at(*idx, *expected_tmp)
            .local_get_at(*idx, *addr_tmp)
            .call_at(*idx, self.context.read_hook)
            .const_at(*idx, Value::I32(instr_loc_id.data() as i32))
            .const_at(*idx, Value::I32(self.function_loc.data() as i32))
            .const_at(*idx, Value::I32(Self::ATOMIC_ACCESS))
            .const_at(*idx, Value::I32(access_width as i32))
            .binop_at(*idx, BinaryOp::I32Add)
            .const_at(*idx, Value::I32(atomic_wait.arg.offset as i32))
            .local_tee_at(*idx, *addr_tmp)
            .local_set_at(*idx, *expected_tmp)
            .local_set_at(*idx, *timeout_tmp);
        // Original atomic.wait instruction comes here ...

        *idx += 13 // We added 13 instructions in total
    }
}

struct InstrumentationContext {
    external_hooks: HashSet<FunctionId>,
    initialize: FunctionId,
    read_hook: FunctionId,
    write_hook: FunctionId,
}

impl InstrumentationContext {
    fn new(module: &mut Module) -> Self {
        let hook_params = [
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
        ];
        let hook_type = Self::get_or_create_type(&mut module.types, &hook_params, &[]);

        let read_hook = Self::create_or_replace_function_import(
            module,
            "wasmgrind_tracing",
            "read_hook",
            hook_type,
        );

        let write_hook = Self::create_or_replace_function_import(
            module,
            "wasmgrind_tracing",
            "write_hook",
            hook_type,
        );

        let init_fn_type = Self::get_or_create_type(&mut module.types, &[], &[]);
        let initialize = Self::create_or_replace_function_import(
            module,
            "wasmgrind_tracing",
            "initialize",
            init_fn_type,
        );

        Self {
            external_hooks: HashSet::new(),
            initialize,
            read_hook,
            write_hook,
        }
    }

    fn create_or_replace_function_import(
        module: &mut Module,
        import_module: &str,
        import_name: &str,
        import_type: TypeId,
    ) -> FunctionId {
        if let Some(import) = module.imports.find(import_module, import_name) {
            module.imports.delete(import);
        }
        let (fidx, _) = module.add_import_func(import_module, import_name, import_type);
        module.funcs.get_mut(fidx).name = Some(import_name.to_string());
        fidx
    }

    fn validate_function_import(import: &Import) -> Result<FunctionId, Error> {
        match import.kind {
            walrus::ImportKind::Function(id) => Ok(id),
            _ => bail!(
                "Import '{} {}' was not a function!",
                import.module,
                import.name
            ),
        }
    }

    fn get_or_create_type(
        types: &mut ModuleTypes,
        params: &[ValType],
        results: &[ValType],
    ) -> TypeId {
        if let Some(tidx) = types.find(params, results) {
            tidx
        } else {
            types.add(params, results)
        }
    }

    fn accept_import(&mut self, import: &Import) -> Result<bool, Error> {
        match import.module.as_str() {
            "wasmgrind_tracing" => match import.name.as_str() {
                "thread_create" | "thread_join" | "mutex_start_lock" | "mutex_finish_lock"
                | "mutex_unlock" => {
                    let fidx = Self::validate_function_import(import)?;
                    self.external_hooks.insert(fidx);
                    Ok(true)
                }
                _ => Ok(false),
            },
            _ => Ok(false),
        }
    }

    fn patch_hook_signatures(&self, module: &mut Module) -> Result<(), Error> {
        for fidx in &self.external_hooks {
            let func = module.funcs.get_mut(*fidx);
            match &mut func.kind {
                walrus::FunctionKind::Import(imported_function) => {
                    let ty = module.types.get(imported_function.ty);

                    let results = ty.results().to_vec();
                    let mut params = ty.params().to_vec();
                    params.extend([ValType::I32, ValType::I32]);

                    imported_function.ty = InstrumentationContext::get_or_create_type(
                        &mut module.types,
                        &params,
                        &results,
                    )
                }
                _ => bail!("Imported function did not have FunctionKind 'Import'"),
            }
        }

        Ok(())
    }
}

fn patch_start_fn(module: &mut Module, context: &InstrumentationContext) {
    let mut builder = FunctionBuilder::new(&mut module.types, &[], &[]);
    builder.name("__wasmgrind_init".to_string());

    let mut body = builder.func_body();
    body.call(context.initialize);

    if let Some(start) = &module.start {
        body.call(*start);
    }

    let id = builder.finish(vec![], &mut module.funcs);
    module.start = Some(id);
}

pub fn instrument(module: &mut Module) -> Result<&mut Module, Error> {
    for memory in module.memories.iter() {
        if memory.memory64 {
            bail!("Wasmgrind instrumentation does not support 64bit WebAssembly memories")
        }
    }

    let mut context = InstrumentationContext::new(module);
    for import in module.imports.iter() {
        context.accept_import(import)?;
    }

    context.patch_hook_signatures(module)?;

    patch_start_fn(module, &context);

    let module_locals = Mutex::new(&mut module.locals);
    module.funcs.par_iter_local_mut().for_each(|(_, f_mut)| {
        let mut instrumentation = WasmgrindInstrumentation::new(&context, &module_locals);
        instrumentation.process_function(f_mut);
    });

    Ok(module)
}
