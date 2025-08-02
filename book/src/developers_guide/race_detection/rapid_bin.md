# RapidBin - The Binary Trace Format
The RapidBin trace format is a binary format to represent execution traces. It consists of a header and a list of events. The format is specified as follows (the given ranges are half-open intervals `[start,end)`):

Header:
```
-----------------------------------
| bytes 0-2: number of threads    | 
-----------------------------------
-----------------------------------
| bytes 2-6: number of locks      | 
-----------------------------------
-----------------------------------
| bytes 6-10: number of variables |  
-----------------------------------
-----------------------------------
| bytes 10-18: number of events   |
-----------------------------------
```
**Note:**
- `number of threads` is a 16-bit _signed_ integer in _big-endian_ byte order
- `number of locks` and `number of variables` are 32-bit _signed_ integers in _big-endian_ byte order
- `number of events` is a 64-bit _signed_ integer in _big-endian_ byte order


The header is followed by a series of `n` events as specified in the header. Each event is a 64-bit _signed_ integer in _big-endian_ byte order and has the following format:
```
-------------------------
| bit 0: sign-bit       | 
-------------------------
-------------------------
| bits 1-16: location   | 
-------------------------
-------------------------
| bits 16-50: decor     | 
-------------------------
-------------------------
| bits 50-54: operation | 
-------------------------
-------------------------
| bits 54-64: thread-id |
-------------------------
```

**Note:** Decor can be either a variable or, the id of a child thread or a lock-id depending on the operation.

## Trace Metadata
When generating the binary trace, a list of metadata records is emitted alongside it in JSON format. These metadata records map the original event datapoints to their identifiers in the binary trace. 

This is necessary because the RapidBin format defines a single location identifier and a single identifier for variables inside the decor, whereas Wasmgrinds location is defined by _two_ values (the function index and the instruction index) and Wasmgrinds memory locations is defined by the _memory address_ and the _number of accessed bytes_. The metadata enables users of Wasmgrind to locate the problem inside the original binary, if the analysis of the binary trace with third-party tools reports a bug.