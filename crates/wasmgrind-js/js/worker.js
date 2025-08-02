import init, { handle_message } from "../../..";

self.addEventListener("message", async event => {
    if (event.data.type && event.data.type.startsWith("wgrind_")) {
        let {
            wgrind_module,
            wgrind_memory,
        } = event.data;

        await init({
            module: wgrind_module, 
            memory: wgrind_memory, 
            thread_stack_size: undefined
        });

        console.log("Received message: ", event.data);

        handle_message(event.data);
    } else {
        console.warn("Unknown message: ", event.data);
    }
})

function start_worker() {
    let worker = new Worker(
        new URL("./worker.js", import.meta.url),
        {
            type: "module"
        }
    );

    return worker;
}

export { start_worker }