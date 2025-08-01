# Web Environments
**Attention:** Using Wasmgrind in web-browsers is highly experimental and should be used with care!

In the next sections we assume that you have cloned to your local disk. Paths are given relative to the project root unless stated otherwise.

## Setup
We assume that the WebAssembly binary `target.wasm`, which you want to examine with Wasmgrind, is located in some directory `serve` on your disk and it exports a parameterless function `target_function`. This directory is served via a webserver and accessible via the browser.

**Note:** Wasmgrind on the Web currently does only support the full tracing API with instrumentation. Standalone execution is not yet implemented.

### 1. Setup JS Runtime Environment
Copy the files `wasmgrind.js`, `wasmgrind_worker.js` and `worker.js` from the directory `crates/wasmgrind-js` to your `serve` directory.

### 2. Setup Wasmgrind JS Utility Library
Compile the wasmgrind-js library as it provides core utilities for instrumentation, thread management and execution tracing in form of a WebAssembly module. To build it, navigate to `crates/wasmgrind-js` and run

    wasm-pack build --target web

The output of this command should be located under `crates/wasmgrind-js/pkg`. Copy this folder to your `serve` directory.

### 3. Create an `index.html` to execute Wasmgrind
The usage of Wasmgrind in the web boils down to one simple API function at this point in time: 

```TypeScript
class TraceOutput {
  readonly trace: Uint8Array;
  readonly metadata: string;
}

function wasmgrind(binary_path: string, function_name: string): TraceOutput;
```

This function should only be called inside a WebWorker because it uses atomic wait instructions, which are disallowed on the main browser thread. The web worker proxy is provided via the `wgrind_worker.js` script. To call `wasmgrind`, spin up the worker and post a message with the following content to it:

```JavaScript
{
    binary_path: "./target.wasm",
    function_name: "target_function",
}
```

An example `index.html` would look like this:

```html
<html>

<head>
    <meta content="text/html;charset=utf-8" http-equiv="Content-Type" />
</head>

<body>
    <div id="wrapper">
        <h1>Wasmgrind Test</h1>

        <button id="wasmgrind">Try wasmgrind!</button>
    </div>
    <script type="module">
        let button = document.getElementById("wasmgrind");

        button.onclick = async _event => {
            let worker = new Worker("./wgrind_worker.js", { type: "module" });
            worker.onmessage = (event) => {
                let { trace_url, metadata_url } = event.data;
                let a = document.getElementById("invisible-download-link");
                if (!a) {
                    a = document.createElement('a');
                    a.id = "invisible-download-link";
                    document.body.appendChild(a);
                }
                a.href = trace_url
                a.download = "trace.bin";
                a.click();

                a.href = metadata_url
                a.download = "trace.json";
                a.click();
            };
            worker.postMessage({
                binary_path: "./target.wasm",
                function_name: "target_function"
            })
        }
    </script>
</body>

</html>
```

When the button _"Try wasmgrind!"_ is clicked, Wasmgrind executes `target_function` of `target.wasm`, generates an execution trace and downloads it in RapidBin format (`trace.bin`) alongside with its metadata (`trace.json`).

### 4. Final Directory Structure of the `serve` folder
Your `serve` directory should now contain _at least_ the following files inside their corresponding directories:
```
serve/
|--- pkg/
|    |--- wasmgrind_js_bg.wasm
|    |--- wasmgrind_js_bg.wasm.d.ts
|    |--- wasmgrind_js.d.ts
|    |--- wasmgrind_js.js
|--- index.html
|--- target.wasm
|--- wasmgrind.js
|--- wgrind_worker.js
|--- worker.js
```