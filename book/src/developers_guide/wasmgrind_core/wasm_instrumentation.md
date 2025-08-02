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
i32.const <offset of the load instruction memary immediate>
i32.add
i32.const <align of the load instruction memary immediate>
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
i32.const <offset of the store instruction memary immediate>
i32.add
i32.const <align of the store instruction memary immediate>
i32.const <current function index>
i32.const <current instruction index>
call <index of the write_hook function>

... ;; code after the store instruction

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
i32.const <offset of the rmw instruction memary immediate>
i32.add
i32.const <align of the rmw instruction memary immediate>
i32.const <current function index>
i32.const <current instruction index>
call <index of the read_hook function>
local.get $addr_tmp
i32.const <offset of the rmw instruction memary immediate>
i32.add
i32.const <align of the rmw instruction memary immediate>
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
i32.const <offset of the cmpxchg instruction memary immediate>
i32.add
i32.const <align of the cmpxchg instruction memary immediate>
i32.const <current function index>
i32.const <current instruction index>
call <index of the read_hook function>
local.tee $returned_tmp
local.get $returned_tmp ;; duplicate the return value to use it in the if statement
local.get $expected_tmp
i<nn>.eq                  ;; nn depends on the instrumented cmpxchg instruction
if
    local.get $addr_tmp
    i32.const <offset of the cmpxchg instruction memary immediate>
    i32.add
    i32.const <align of the cmpxchg instruction memary immediate>
    i32.const <current function index>
    i32.const <current instruction index>
    call <index of the write_hook function>
end

... ;; code after the cmpxchg instruction

```

**Note:** We need to examine the return value of the cmpxchg instruction to determine if a write has occurred. If a write has occured, we call both hooks. Otherwise, we only call the read hook.