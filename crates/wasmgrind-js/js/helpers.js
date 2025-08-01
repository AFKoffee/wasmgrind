async function fetch_helper(url) {
    let buf = await fetch(url).then(res => res.arrayBuffer());
    return new Uint8Array(buf);
}

async function compile_helper(wasm) {
    return WebAssembly.compile(wasm)
}

function memory_helper(min, max) {
    return new WebAssembly.Memory({initial: min, maximum: max, shared: true})
}

function set_value(memory, ptr, value) {
    const view = new DataView(memory.buffer);
    view.setUint32(ptr, value, true);
}

export { fetch_helper, compile_helper, memory_helper, set_value }