# The Wasmgrind NPM Package
The wasmgrind-js npm-package bundles everything that is needed to perform binary instrumentation, execution tracing and thread management in the browser. It uses wasm-bindgen to generate JS bindings.

Wasm-Bindgen emits an initalization function, which is exported by the npm-package as default. To use functions from the `wasmgrind-js` package, the internal WebAssembly module needs to be instantiated first. This can be achieved using the initialization function:
```javascript
import init, { wasmgrind } from 'wasmgrind-js';

function do_something_with_wasmgrind_js() {
    await init();

    let runtime = await wasmgrind(new URL(/* path to some wasm file ...*/));

    // Do something with the wasmgrind runtime ...
}
```

The Wasmgrind-specific functions and classes that are exposed by wasmgrind-js have the following type signatures:

```typescript
export function wasmgrind(binary: URL): Promise<WasmgrindRuntime>;
export function grind(binary: URL, function_name: string): Promise<any>;
export function runtime(binary: URL): Promise<ThreadlinkRuntime>;
export function run(binary: URL, function_name: string): Promise<void>;

export class ThreadlinkRuntime {
  private constructor();
  free(): void;
  invoke_function(function_name: string): Promise<any>;
}

export class WasmgrindRuntime {
  private constructor();
  free(): void;
  invoke_function(function_name: string): Promise<any>;
  generate_binary_trace(): Promise<any>;
}

export class TraceOutput {
  private constructor();
  free(): void;
  readonly trace: Uint8Array;
  readonly metadata: string;
}
```

These exports refer to Rust functions and structs that are exported by the wasmgrind-js crate. See the [wasmgrind-js docs](https://wasmgrind-d6f2b1.gitlab.io/docs/wasmgrind_js/) for further information.