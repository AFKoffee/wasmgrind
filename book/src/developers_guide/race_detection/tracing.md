# Tracing
The internal tracing datastructures are designed to precisely match the arguments provided by the tracing hooks, which are creating during the instrumentation phase. For more information about the employed instrumentation refer to [Chapter 16.2: WebAssembly Instrumentation](../wasmgrind_core/wasm_instrumentation.md).

## Operations
Wasmgrind records different _operations_ while building an execution trace. Each operation has specific data associated with it, which is necessary for later analysis. It is defined as an enum:
```Rust
pub enum Op {
    Read { addr: u32, n: u32 },
    Write { addr: u32, n: u32 },
    Aquire { lock: u32 },
    Request { lock: u32 },
    Release { lock: u32 },
    Fork { tid: u32 },
    Join { tid: u32 },
}
```

## Events
An _event_ is a single element of a trace. It bundles the ID if the thread, which is being executed, the performed operation and the location in the binary where the event happened.
```Rust
struct Event {
    t: u32,          // ID of the executing thread
    op: Op,          // executed operation
    loc: (u32, u32), // location in the binary: (function_idx, instr_idx)
}
```

## The Trace
The trace is internally represented as a mutex-synchronized vector of events.
```Rust
pub struct Tracing {
    events: Mutex<Vec<Event>>,
}
```

The `Tracing` struct manages the trace and provides two methods to interact with it:

```Rust
impl Tracing {
    pub fn add_event(&self, tid: u32, op: Op, loc: (u32, u32)) -> Result<(), Error> {
        match self.events.lock() {
            Ok(mut events_guard) => {
                events_guard.push(Event { t: tid, op, loc });
                Ok(())
            }
            Err(_) => bail!("Trace Lock Poisoned: Could not insert new event!"),
        }
    }

    pub fn generate_binary_trace(&self) -> Result<BinaryTraceOutput, Error> {
        let mut converter = WasmgrindTraceConverter::new();

        let binary_trace = match self.events.lock() {
            Ok(events_guard) => {
                let mut encoder = RapidBinEncoder::new();
                let mut output = Cursor::new(Vec::with_capacity(
                    events_guard.len() * RapidBinEncoder::EVENT_SIZE_HINT,
                ));

                encoder.encode(
                    events_guard.iter().map(|e| Ok(converter.convert_event(e))),
                    &mut output,
                )?;

                output.into_inner()
            }
            Err(_) => bail!("Trace Lock Poisoned: Could not generate binary trace!"),
        };

        Ok(BinaryTraceOutput {
            trace: binary_trace,
            metadata: converter.genrate_metadata(),
        })
    }
}
```

The *add_event* function appends a new event to the end of the trace. It takes the given data, locks the trace, wrapps the data in the corresponding structure and pushes the struct onto the vector. The operation is very simple, thus making it very fast, which results in low runtime overhead. According to the [documentation of the Rust standard library](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.push), the vectors push operation takes amortized O(1) time.

The *generate_binary_trace* function reads the trace and generates a RapidBin representation of it. First, the Wasmgrind-specific trace data structure is converted into a generic trace representation. During this process, the trace metadata is generated that maps the generic representation back to the original Wasmgrind-specific data structure. Then, the generic trace is encoded into RapidBin format. 

The [next chapter](./rapid_bin.md) explains the binary trace format in detail.