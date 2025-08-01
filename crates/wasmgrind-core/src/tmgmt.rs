use std::{
    cell::Cell,
    sync::{
        Condvar, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

use anyhow::{Error, anyhow, bail};


static THREAD_COUNTER: AtomicU32 = AtomicU32::new(0);
static MAIN_INITIALIZED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static THREAD_ID: Cell<Option<u32>> = const { Cell::new(None) }
}

/// Generates a new, unique thread-id for this program run.
/// 
/// This function internally increments an [`AtomicU32`].
/// Therefore, with the current implementation there is no
/// way of reusing thread-ids
pub fn next_available_thread_id() -> u32 {
    THREAD_COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Retrieves the thread-id of the current thread
/// 
/// The thread-id must have been set beforehand
/// by the [`set_thread_id`] function. The **only exception**
/// from this rule is the main thread, for whom the thread-id
/// is generated upon first access. Therefore, this function
/// is allowed to be called **once** amongst all threads
/// without having a thread-id set.
/// 
/// # Errors
/// 
/// This function may fail in the following cases:
/// - The thread-local storage, where the thread-id resides
///   can not be accessed.
/// - The function is called the second time whithout having
///   a thread-id set beforehand
pub fn thread_id() -> Result<u32, Error> {
    THREAD_ID.try_with(|maybe_thread_id| {
        if let Some(thread_id) = maybe_thread_id.get() {
            Ok(thread_id)
        } else {
            if MAIN_INITIALIZED.load(Ordering::SeqCst) {
                bail!("Error: Main thread initialized twice.")
            }

            let thread_id = next_available_thread_id();
            maybe_thread_id.replace(Some(thread_id));

            MAIN_INITIALIZED.store(true, Ordering::SeqCst);

            // println!("INFO: Initialized main thread id with {thread_id}");

            Ok(thread_id)
        }
    })? // .expect("Error: Could not access thread-local thread id.")
}

/// Sets the thread-id for the current thread
/// 
/// This operation is only allowed **once** per thread.
/// 
/// # Errors
/// 
/// This function may fail in the following cases:
/// - The thread-local storage, where the thread-id resides
///   can not be accessed.
/// - The function is called the second time from the same
///   thread
pub fn set_thread_id(id: u32) -> Result<(), Error> {
    THREAD_ID.try_with(|maybe_thread_id| {
        if maybe_thread_id.get().is_some() {
            bail!("Thread id was already initialized!")
        } else {
            maybe_thread_id.replace(Some(id));
            Ok(())
        }
    })? // .expect("Error: Could not access thread-local thread id.")
}

/// A structure that allows waiting for a value to become present.
/// 
/// A [`ConditionalHandle`] represents a [`Mutex`]-[`Condvar`] pair,
/// where the mutex wraps an [`Option`] of an arbitrary type.
/// 
/// This structure is intended to send values of a specific type 
/// between a pair of threads and should therefore never be shared between
/// more than two threads - one sender and one receiver.
pub struct ConditionalHandle<T> {
    handle: Mutex<Option<T>>,
    barrier: Condvar,
}

impl<T> ConditionalHandle<T> {
    /// Creates an empty [`ConditionalHandle`].
    /// 
    /// The value will not be present until the sending thread calls
    /// [`set_and_notify`][ConditionalHandle::set_and_notify] on 
    /// this `struct`.
    pub fn new() -> Self {
        Self {
            handle: Mutex::new(None),
            barrier: Condvar::new(),
        }
    }

    /// Creates a [`ConditionalHandle`] that already contains a value.
    /// 
    /// The value is present immediately, which means that a call
    /// to [`take_when_ready`][ConditionalHandle::take_when_ready] by
    /// the receiving thread will return immediately.
    pub fn with_value(val: T) -> Self {
        Self {
            handle: Mutex::new(Some(val)),
            barrier: Condvar::new(),
        }
    }

    /// Sets the value of this [`ConditionalHandle`] and notifies
    /// a thread waiting on it.
    /// 
    /// # Errors
    /// 
    /// This function may fail if the internal [`Mutex`] of this
    /// [`ConditionalHandle`] was poisoned.
    pub fn set_and_notify(&self, val: T) -> Result<(), Error> {
        match self.handle.lock() {
            Ok(mut maybe_val) => {
                *maybe_val = Some(val);
                self.barrier.notify_one();
                Ok(())
            }
            Err(_) => Err(anyhow!("ConditionalHandle Mutex was poisoned!")),
        }
    }

    /// Waits for the value of this [`ConditionalHandle`] to become present.
    /// 
    /// If the value already is present when calling this function, it will
    /// return immediately.
    /// 
    /// # Errors
    /// 
    /// This function may fail if the internal [`Mutex`] of this
    /// [`ConditionalHandle`] was poisoned or the value has not been
    /// present although the internal [`Condvar`] was notified.
    /// The latter case is most likely an internal bug!
    pub fn take_when_ready(&self) -> Result<T, Error> {
        let mut val = match self.handle.lock() {
            Ok(val) => val,
            Err(_) => bail!("ConditionalHandle Mutex was poisoned!"),
        };

        while val.is_none() {
            val = match self.barrier.wait(val) {
                Ok(val) => val,
                Err(_) => bail!("ConditionalHandle Mutex was poisoned!"),
            };
        }

        if let Some(inner) = val.take() {
            Ok(inner)
        } else {
            Err(anyhow!(
                "Taken value was None: This should never happen as we sync this operation through a condition variable."
            ))
        }
    }
}

impl<T> Default for ConditionalHandle<T> {
    fn default() -> Self {
        Self::new()
    }
}
