import init, { wasmgrind } from 'wasmgrind-js';

let button = document.getElementById("wasmgrind");

button.onclick = async _event => {
    await init();

    let runtime = await wasmgrind(new URL("./minimal_test_tracing.wasm", import.meta.url));
    await runtime.invoke_function("two_nested_threads_test");
    
    console.log("Function invocation awaited!");
    
    let output = await runtime.generate_binary_trace();

    console.log("Binary trace awaited!");

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