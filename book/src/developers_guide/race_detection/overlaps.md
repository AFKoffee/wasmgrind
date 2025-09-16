# Overlapping Memory Accesses
Wasmgrind records memory accesses as tuples: `(memory address, number of accessed bytes)`. However, the RapidBin format requires read and write events to refer to a single ID. Wasmgrind bridges that gap by assigning a unique ID to each unique memory access that occurs in the execution trace.

This introduces some caveats. For example, one memory access may target memory address `x` and accesses 4 bytes at that address while another memory
access may target the same memory address `x` but accesses 8 bytes at that address. These two accesses will have different IDs despite targeting (at least partly) the same bytes in memory. Having different IDs, concurrency algorithms working with the RapidBin trace will treat both memory accesses as completely unrelated, which may result in undetected concurrency errors.

The race-detection crate provides utilities to analyze a binary execution trace using its metadata to find all memory accesses that may potentially overlap with other memory accesses amongst different threads such that the extent of possibly undetected errors can be assessed. 

## Special considerations with regard to wasm-threadlink
During development, we found that overlaps occurr in correlation with forking and joining threads when using the wasm-threadlink crate. 

For example, when executing the _minimal-test_ example with arguments _two-nested-threads_ and _--tracing_, we get the following output:
```
WARNING: Memory access 116 (threads: 2, 1) overlaps with memory access 137 (threads: 1, 2) - Access 137 at 1376384 of length 8 contains access 116 at 1376384 of length 4
WARNING: Memory access 92 (threads: 0, 1) overlaps with memory access 149 (threads: 0, 1) - Access 149 at 1376328 of length 8 contains access 92 at 1376328 of length 4
Overlap Ratio of the Trace (Overlaps / Memory Accesses): 12 / 1173
```

When we look at the execution trace in STD format and filter for fork, join, and memory access events with regard to the emitted warnings, we get:
```
T0|w(V92)|114
T0|fork(T1)|122
                    T1|w(V116)|177
                    T1|fork(T2)|185
                                        T2|r(V116)|393
                                        T2|w(V137)|394
                    T1|join(T2)|424
                    T1|r(V137)|431
                    T1|w(V116)|432
                    T1|r(V116)|438
                    T1|r(V92)|443
                    T1|w(V149)|444
T0|join(T1)|424
T0|r(V149)|431
T0|w(V92)|432
T0|r(V92)|438
```

We assume that this has something to do with the mechanism, which wasm-threadlink uses to pass the result of the childthread's closure back to the parent thread. Specifically, we assume that this behavior relates to Rusts memory management for the `Option` enum, which is used by wasm-threadlink internally during the process of passing data to the parent thread when joining. Nevertheless, as we see above, the reads and writes to the overlapping memory regions seem to be synchronized correctly via the join-boundary so we deem them to be safe for now.