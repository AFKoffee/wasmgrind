use wasm_threadlink::{mutex::TracingMutex, thread::thread_spawn};

static DATA_1: TracingMutex<i64> = TracingMutex::new(0);
static DATA_2: TracingMutex<i64> = TracingMutex::new(0);

#[unsafe(no_mangle)]
pub fn create_deadlock() {
    // Run a detached thread to to use join without freezing the main thread
    //let _ = thread_spawn(|| {
    let t1 = thread_spawn(move || deadlock_prone_task(&DATA_1, &DATA_2));

    let t2 = thread_spawn(move || deadlock_prone_task(&DATA_2, &DATA_1));

    let _ = t1.join();

    let _ = t2.join();
    //});
}

fn deadlock_prone_task(first: &TracingMutex<i64>, second: &TracingMutex<i64>) {
    loop {
        increment_decrement(first, second);
    }
}

fn increment_decrement(first: &TracingMutex<i64>, second: &TracingMutex<i64>) {
    let mut first_data = first.lock();
    let mut second_data = second.lock();
    *first_data += 1;
    *second_data -= 1;
    drop(first_data);
    drop(second_data);
}
