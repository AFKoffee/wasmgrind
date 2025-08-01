# Getting Started
This and the following chapters will guide you through all necessary steps to get Wasmgrind up and running. In this chapter we will set up the build and execution environment for Wasmgrind.

## 1. Install a Rust Toolchain
Wasmgrind requires a `nightly` toolchain to compile binaries that are _run_ with Wasmgrind and a `stable` toolchain to _build_ the Wasmgrind engine itself. The most straightforward apporach to managing Rust toolchains is by using `rustup`. Refer to the [Rust Website](https://www.rust-lang.org/tools/install) for instructions on how to install it.

Verify, that you have the necessary tools installed:
```
rustup --version
rustc --version
cargo --version
```

## 2. Install `wasm-tools`
The `wasm-tools` package is a CLI for a collection of libraries for low-level manipulation of WebAssembly modules. Currently, the native Wasmgrind engine uses this CLI tool when certain command line options are activated and expects it to be available on your PATH.

Install the tool via:
```
cargo install wasm-tools
```

And verify it is on your PATH:
```
wasm-tools --version
```

## 3. Install `wasm-pack`
The `wasm-pack` tool simplifies the process of building WebAssembly modules to be run in browsers. It integrates seamlessly with `wasm-bindgen` and can bundle the resulting `.wasm` and `.js` files as an npm package.

**Note:** You only need this tool if you plan to compile the `wasmgrind-js` crate in order to run Wasmgrind in the browser.

Install the tool via:
```
cargo install wasm-pack
```

And verify it is on your PATH:
```
wasm-pack --version
```

## 4. Install Python
If you want to run the demo server of the `wasmgrind-js` project, you need to have python installed. You can verify this via:
```
python --version
```

The demo server only requires the `sys` and `http.server` packages so for modern python versions there should be no need to install additional modules appart from python itself.