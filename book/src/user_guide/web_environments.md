# Web Environments
**Attention:** Using Wasmgrind in web-browsers is experimental and should be used with care!

In the next sections we assume that you have cloned the git repository to your local disk. Paths are given relative to the project root unless stated otherwise.

## Setup
We assume that the WebAssembly binary `target.wasm`, which you want to examine with Wasmgrind, exports a parameterless function `target_function`.

### 1. Setup an NPM Package with Webpack
First, setup a new npm package or use an existing one on your disk. To create a new package, you can use the `npm init` command. Then, install the necessary webpack utilities and copy your `target.wasm` file into the package root directory.

An example `package.json` may look like this:
```json
{
  "name": "your-package",
  "version": "0.1.0",
  "main": "index.js",
  "scripts": {
    "build": "webpack build",
    "start": "webpack-dev-server"
  },
  "author": "",
  "license": "MIT OR Apache-2.0",
  "description": "",
  "devDependencies": {
    "copy-webpack-plugin": "^13.0.0",
    "webpack": "^5.100.2",
    "webpack-cli": "^6.0.1",
    "webpack-dev-server": "^5.2.2"
  }
}
```

### 2. Create a Webpack Configuration
The following code snippet shows an example webpack configuration adapted from the [Wasmgrind browser demo](https://gitlab.com/AFKoffee/wasmgrind/-/tree/main/demos/browser-demo). There are a few points to look out for here:
- WebServer Headers: The `Cross-Origin-Opener-Policy` and `Cross-Origin-Embedder-Policy` have to be set in order to use WebAssembly shared memories, which is required for Wasmgrind to work
- The copied files and the entrypoint are tailored to this example and should be adjusted to fit the needs of your project.
```javascript
import CopyPlugin from 'copy-webpack-plugin';
import { dirname, resolve } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));

export default {
  entry: "./index.js",
  output: {
    path: resolve(__dirname, "dist"),
    filename: "bundle.js",
  },
  mode: "development",
  plugins: [
    new CopyPlugin({
      patterns: ['index.html', 'target.wasm']
    })
  ],
  devServer: {
    static: {
      directory: './dist'
    },
    headers: [
      { key: 'Cross-Origin-Opener-Policy', value: 'same-origin' },
      { key: 'Cross-Origin-Embedder-Policy', value: 'require-corp' },
    ],
  },
  module: {
    rules: [
      {
        test: /\.m?js$/,
        resolve: {
          fullySpecified: false
        }
      }
    ]
  }
};
```

### 2. Setup the Wasmgrind JS Package
Compile the wasmgrind-js library as it provides core utilities for instrumentation, thread management and execution tracing in form of a WebAssembly module. To build it, navigate to `crates/wasmgrind-js` and run

    wasm-pack build --target web

The output of this command should be located under `crates/wasmgrind-js/pkg`. Link this npm-package into your project by navigating back into your project and executing

    npm link /path/to/wasmgrind-js/pkg

### 3. Create an `index.html` and `index.js` to execute Wasmgrind
Wasmgrind offers a reduced set of API functions on the web compared to native hosts. Refer to the [wasmgrind-js docs](https://afkoffee.github.io/wasmgrind/wasmgrind-docs-rs/wasmgrind_js/) for further information. The following snippets show how to generate and download a simple execution trace with Wasmgrind on the web.

As we have specified the `index.js` in our webpack configuration, we have to provide one:
```javascript
import init, { wasmgrind } from 'wasmgrind-js';

let button = document.getElementById("wasmgrind");

button.onclick = async _event => {
    await init();

    let runtime = await wasmgrind(new URL("./target.wasm", import.meta.url));
    await runtime.invoke_function("target_function");
    
    let output = await runtime.generate_binary_trace();

    let trace_url = URL.createObjectURL(new Blob([output.trace]));    
    let metadata_url = URL.createObjectURL(new Blob([output.metadata]));

    download_from_url(trace_url, "trace.bin");
    download_from_url(metadata_url, "trace.json");
}

function download_from_url(url, filename) {
    let a = document.getElementById("invisible-download-link");
    if (!a) {
        a = document.createElement('a');
        a.id = "invisible-download-link";
        document.body.appendChild(a);
    }
    a.href = url
    a.download = filename;
    a.click();
}
```

The corresponding `index.html` looks like this:

```html
<html>

<head>
    <meta content="text/html;charset=utf-8" http-equiv="Content-Type" />
</head>

<body>
    <div id="wrapper">
        <h1>Wasmgrind Browser Demo</h1>

        <button id="wasmgrind">Try wasmgrind!</button>
    </div>
    <script src="bundle.js"></script>
</body>

</html>
```

When the button _"Try wasmgrind!"_ is clicked, Wasmgrind executes `target_function` of `target.wasm`, generates an execution trace and downloads it in RapidBin format (`trace.bin`) alongside with its metadata (`trace.json`).

**Note:** We import the `bundle.js` script here as this script is emitted by webpack after bundling.

### 4. Final Directory Structure of the `serve` folder
Your npm-package directory should now contain _at least_ the following files:
```
your-package/
|--- index.html
|--- index.js
|--- target.wasm
|--- package.json
|--- webpack.config.mjs
```

For a working example, refer to the [browser demo](https://gitlab.com/AFKoffee/wasmgrind/-/tree/main/demos/browser-demo).