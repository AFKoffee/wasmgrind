import init, { runtime } from 'wasmgrind-js';

let button = document.getElementById("wasmgrind");

button.onclick = async _event => {
    await init();

    let rt = await runtime(new URL("./minimal_test.wasm", import.meta.url));
    await rt.invoke_function("thread_hierarchy_test");
    
    console.log("Function invocation awaited!");
    
    /*let output = await runtime.generate_binary_trace();

    console.log("Binary trace awaited!");

    console.log(output.trace);
    console.log(output.metadata);*/
}