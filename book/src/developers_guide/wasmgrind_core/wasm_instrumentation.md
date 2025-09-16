# WebAssembly Instrumentation
Wasmgrind-Core uses Wasabi under the hood to perform binary instrumentation for execution tracing. The following parts of the WebAssembly module are altered in this process:
- specific calls to the internal runtime ABI, i.e., `thread_create`, `thread_join`, `start_lock`, `finish_lock`, `start_unlock`, `finish_unlock`
- memory instructions like `load` and `store` (including atomic memory instructions, i.e., `cmpxchg`, `rmw`, `atomic.load` and `atomic.store`)

## Instrumentation of internal runtime ABI Calls
All calls to the internal runtime ABI stated above are extended by two parameters: the current function index and the current instruction index.

For example: 
- `thread_create` will have signature `(i32, i32, i32, i32) -> (i32)` instead of `(i32, i32) -> (i32)` after the instrumentation.
- `start_lock` will have signature `(i32, i32, i32) -> ()` instead of `(i32) -> ()` after the instrumentation.

## Instrumentation of Memory Instructions
To record memory accesses, wasabi injects two new functions into the internal runtime ABI. These are imported under the `wasabi` namespace and have the following names:

- `read_hook`: A function with four arguments indicating a memory read: the memory address being accessed, the number of accessed bytes, the current function index, the current instruction index
- `write_hook`: A function with four arguments indicating a memory write: the memory address being accessed, the number of accessed bytes, the current function index, the current instruction index

Each memory instruction is patched slightly different. The following sections describe their instrumentation in detail.

### Instrumentation of load instructions (including atomic loads)
A new local is injected into the function of the instrumented instruction to temporarily store the argument of the instruction, i.e. the accessed memory address.

```
... ;; code before the load instruction

local.tee $addr_tmp
<original load instruction>
local.get $addr_tmp
i32.const <offset of the load instruction memory immediate>
i32.add
i32.const <memory access width of the load instruction>
i32.const <current function index>
i32.const <current instruction index>
call <index of the read_hook function>

... ;; code after the load instruction

```

### Instrumentation of store instructions (including atomic store)
Two new locals are injected into the function of the instrumented instruction to temporarily store the arguments of the instruction, i.e. the value to be stored and the memory address being accessed.

```
... ;; code before the store instruction

local.set $value_tmp
local.tee $addr_tmp
local.get $value_tmp
<original store instruction>
local.get $addr_tmp
i32.const <offset of the store instruction memory immediate>
i32.add
i32.const <memory access width of the store instruction>
i32.const <current function index>
i32.const <current instruction index>
call <index of the write_hook function>

... ;; code after the store instruction

```

### Instrumentation of memory.fill instructions
Three new locals are injected into the function of the instrumented instruction to temporarily store the arguments of the instruction, i.e. the memory address being accessed, a single-byte value and the number of bytes at which the given byte value should be set.

```
... ;; code before the memory.fill instruction

local.set $n_bytes_tmp
local.set $byte_value_tmp
local.tee $dst_addr_tmp
local.get $byte_value_tmp
local.get $n_bytes_tmp
<original memory.fill instruction>
local.get $dst_addr_tmp
local.get $n_bytes_tmp
i32.const <current function index>
i32.const <current instruction index>
call <index of the write_hook function>

... ;; code after the memory.fill instruction

```

### Instrumentation of memory.copy instructions
Three new locals are injected into the function of the instrumented instruction to temporarily store the arguments of the instruction, i.e. the source memory address being read from, the destination memory address being written to and the number of bytes being copied.

```
... ;; code before the memory.copy instruction

local.set $n_bytes_tmp
local.set $src_addr_tmp
local.tee $dst_addr_tmp
local.get $src_addr_tmp
local.get $n_bytes_tmp
<original memory.copy instruction>
local.get $src_addr_tmp
local.get $n_bytes_tmp
i32.const <current function index>
i32.const <current instruction index>
call <index of the read_hook function>
local.get $dst_addr_tmp
local.get $n_bytes_tmp
i32.const <current function index>
i32.const <current instruction index>
call <index of the write_hook function>

... ;; code after the memory.copy instruction

```

### Instrumentation of memory.init instructions
Three new locals are injected into the function of the instrumented instruction to temporarily store the arguments of the instruction, i.e. the memory address being accessed, the offset into the data segment being read from and number of bytes being copied from the data segment into memory.

```
... ;; code before the memory.init instruction

local.set $n_bytes_tmp
local.set $data_offset_tmp
local.tee $dst_addr_tmp
local.get $data_offset_tmp
local.get $n_bytes_tmp
<original memory.init instruction>
local.get $dst_addr_tmp
local.get $n_bytes_tmp
i32.const <current function index>
i32.const <current instruction index>
call <index of the write_hook function>

... ;; code after the memory.init instruction

```

### Instrumentation of atomic rmw instructions
Two new locals are injected into the function of the instrumented instruction to temporarily store the arguments of the instruction, i.e. the value to be stored and the memory address being accessed.

```
... ;; code before the rmw instruction

local.set $value_tmp
local.tee $addr_tmp
local.get $value_tmp
<original rmw instruction>
local.get $addr_tmp
i32.const <offset of the rmw instruction memory immediate>
i32.add
i32.const <memory access width of the rmw instruction>
i32.const <current function index>
i32.const <current instruction index>
call <index of the read_hook function>
local.get $addr_tmp
i32.const <offset of the rmw instruction memory immediate>
i32.add
i32.const <memory access width of the rmw instruction>
i32.const <current function index>
i32.const <current instruction index>
call <index of the write_hook function>

... ;; code after the rmw instruction

```

**Note:** We need to call both hooks here as the atomic rmw instruction reads from _and_ writes to the specified memory location.

### Instrumentation of atomic cmpxchg instructions
Four new locals are injected into the function of the instrumented instruction to temporarily store the arguments and return value of the instruction, i.e. the memory address being accessed, the expected value, the replacement value and the return value.

```
... ;; code before the cmpxchg instruction

local.set $replacement_tmp
local.set $expected_tmp
local.tee $addr_tmp
local.get $expected_tmp
local.set $replacement_tmp
<original cmpxchg instruction>
local.get $addr_tmp
i32.const <offset of the cmpxchg instruction memory immediate>
i32.add
i32.const <memory access width of the cmpxchg instruction>
i32.const <current function index>
i32.const <current instruction index>
call <index of the read_hook function>
local.tee $returned_tmp
local.get $returned_tmp ;; duplicate the return value to use it in the if statement
local.get $expected_tmp
i<nn>.eq                  ;; nn depends on the instrumented cmpxchg instruction
if
    local.get $addr_tmp
    i32.const <offset of the cmpxchg instruction memory immediate>
    i32.add
    i32.const <memory access width of the cmpxchg instruction>
    i32.const <current function index>
    i32.const <current instruction index>
    call <index of the write_hook function>
end

... ;; code after the cmpxchg instruction

```

**Note:** We need to examine the return value of the cmpxchg instruction to determine if a write has occurred. If a write has occured, we call both hooks. Otherwise, we only call the read hook.

### Instrumentation of atomic wait instructions
Three new locals are injected into the function of the instrumented instruction to temporarily store the arguments of the instruction, i.e. the memory address being accessed, the value expected at the given address and a waiting timeout.

```
... ;; code before the atomic wait instruction

local.set $timeout_tmp
local.set $expected_tmp
local.tee $addr_tmp
i32.const <offset of the atomic wait instruction memory immediate>
i32.add
i32.const <memory access width of the atomic wait instruction>
i32.const <current function index>
i32.const <current instruction index>
call <index of the read_hook function>
local.get $addr_tmp
local.get $expected_tmp
local.get $timeout_tmp
<original atomic.wait instruction>

... ;; code after the atomic wait instruction

```

*Note:* The atomic wait hooks are called BEFORE the original instruction is executed! We decided that this is better than doing it after the instruction because the thread may wait indefinitely after executing atomic.wait. This way the event is more likely to be located close to the point in the trace where atomic.wait was actually executed.

### Instrumentation of atomic notify instructions
Two new locals are injected into the function of the instrumented instruction to temporarily store the arguments of the instruction, i.e. the maximum value of waiters to be woken and the memory address being accessed.

```
... ;; code before the atomic notify instruction

local.set $count_tmp
local.tee $addr_tmp
local.get $count_tmp
<original atomic.notify instruction>
local.get $addr_tmp
i32.const <offset of the atomic notify instruction memory immediate>
i32.add
i32.const <memory access width of the atomic notify instruction>
i32.const <current function index>
i32.const <current instruction index>
call <index of the read_hook function>

... ;; code after the atomic notify instruction

```

*Note:* The threads proposal does not state exactly, whether the notify operator reads from the given address, but we will count it as a read access just to be sure. The question here is: Do we rather want to have more false positives or false negatives? Arguments may favor the approach of not instrumenting atomic.notify because missing some errors is better then erode trust in the tool by having too much false positives ... but this needs more thinking.