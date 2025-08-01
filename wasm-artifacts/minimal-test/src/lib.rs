use std::sync::atomic::AtomicU32;

use wasm_threadlink::{mutex::TracingMutex, thread};

static SOME_VARIABLE: TracingMutex<i32> = TracingMutex::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn increment_some_variable() {
    *SOME_VARIABLE.lock() += 1;
}

#[unsafe(no_mangle)]
pub extern "C" fn reset_some_variable() {
    *SOME_VARIABLE.lock() = 0;
}

#[unsafe(no_mangle)]
pub extern "C" fn get_some_variable() -> i32 {
    *SOME_VARIABLE.lock()
}

#[unsafe(no_mangle)]
pub extern "C" fn threading_test() -> i32 {
    let t1 = thread::thread_spawn(|| {
        for _ in 0..100 {
            increment_some_variable();
        }
    });

    let t2 = thread::thread_spawn(|| {
        reset_some_variable();
    });

    match t1.join() {
        Ok(()) => (),
        Err(_) => return -1,
    };

    match t2.join() {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn two_nested_threads_test() {
    let t1 = thread::thread_spawn(|| {
        let t2 = thread::thread_spawn(|| {
            for _ in 0..100 {
                increment_some_variable();
            }
        });
        let _ = t2.join();
    });
    let _ = t1.join();
}

#[unsafe(no_mangle)]
pub extern "C" fn two_nested_detached_threads_test() {
    let t1 = thread::thread_spawn(|| {
        thread::thread_spawn(|| {
            for _ in 0..100 {
                increment_some_variable();
            }
        });
    });
    let _ = t1.join();
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn thread_hierarchy_test() {
    let mut threads = Vec::new();
    while COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 50 {
        threads.push(thread::thread_spawn(|| {
            thread_hierarchy_test();
        }));
    }

    for thread in threads {
        let _ = thread.join();
    }
}
