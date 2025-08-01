use std::sync::{atomic::{AtomicU32, Ordering}, Arc};

use wasm_threadlink::thread::thread_spawn;
use wasm_threadlink::mutex::TracingMutex;


#[link(wasm_import_module = "custom_import")]
unsafe extern "C" {
    pub fn multiply(arg1: u32, arg2: u32) -> u32;
    pub fn add(arg1: u32, arg2: u32) -> u32;
}

static COUNTER: TracingMutex<u32> = TracingMutex::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn run() -> u32 {
    let result = Arc::new(AtomicU32::new(0));

    let r1 = result.clone();
    let t1 = thread_spawn(move || {
        for _ in 0..50_000 {
            let mut counter = COUNTER.lock();
            let arg1 = *counter;
            *counter += 2;
            drop(counter);

            let mut counter = COUNTER.lock();
            let arg2 = *counter;
            *counter -= 1;
            drop(counter);
            let res = unsafe { multiply(arg1, arg2) };
            r1.store(res, Ordering::SeqCst);
        }
    });

    let r2 = result.clone();
    let t2 = thread_spawn(move || {
        for _ in 0..50_000 {
            let mut counter = COUNTER.lock();
            let arg1 = *counter;
            *counter += 2;
            drop(counter);

            let mut counter = COUNTER.lock();
            let arg2 = *counter;
            *counter -= 1;
            drop(counter);
            let res = unsafe { add(arg1, arg2) };
            r2.store(res, Ordering::SeqCst);
        }
    });

    let _ = t1.join();
    let _ = t2.join();

    result.load(Ordering::SeqCst)
} 