//! Provides a mutex capable of execution tracing
//! 
//! The [`TracingMutex`] exported by this module will only call
//! the lock and unlock hooks if the `tracing` feature is enabled
//! upon compilation.

use parking_lot::{
    RawMutex,
    lock_api::{self, Mutex},
};

#[cfg(feature = "tracing")]
use crate::wasm_abi;

/// A mutex implementation enabling the tracing of lock and unlock events
/// 
/// This struct wraps and behaves like [`parking_lot::RawMutex`] with the
/// following differences if and only if the `tracing` feature is enabled:
/// - The `start_lock` hook is called before the mutex attempts to aquire the lock
/// - The `finish_lock` hook is called after the mutex successfully aquired the lock
/// - The `start_unlock` hook is called before the mutex attempts to release the lock
/// - The `finish_unlock` hook is called after the mutex sucessfully released the lock
/// 
/// **Note:** If the `tracing` feature is enabled. This struct relies on the 
/// _tracing-extended_ internal runtime ABI of Wasmgrind, which provides the above hooks.
pub struct TracingRawMutex {
    inner: RawMutex,
}

unsafe impl lock_api::RawMutex for TracingRawMutex {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self {
        inner: RawMutex::INIT,
    };

    type GuardMarker = <parking_lot::RawMutex as parking_lot::lock_api::RawMutex>::GuardMarker;

    fn lock(&self) {
        alloc_exposer::link_mem_intrinsics();

        #[cfg(feature = "tracing")]
        unsafe {
            wasm_abi::start_lock(self as *const _ as usize)
        };

        self.inner.lock();

        #[cfg(feature = "tracing")]
        unsafe {
            wasm_abi::finish_lock(self as *const _ as usize)
        };
    }

    fn try_lock(&self) -> bool {
        alloc_exposer::link_mem_intrinsics();

        #[cfg(feature = "tracing")]
        unsafe {
            wasm_abi::start_lock(self as *const _ as usize)
        };

        let is_locked = self.inner.try_lock();

        if is_locked {
            #[cfg(feature = "tracing")]
            unsafe {
                wasm_abi::finish_lock(self as *const _ as usize)
            };
        };

        is_locked
    }

    unsafe fn unlock(&self) {
        alloc_exposer::link_mem_intrinsics();

        #[cfg(feature = "tracing")]
        unsafe {
            wasm_abi::start_unlock(self as *const _ as usize)
        };

        unsafe { self.inner.unlock() };

        #[cfg(feature = "tracing")]
        unsafe {
            wasm_abi::finish_unlock(self as *const _ as usize)
        };
    }
}

/// A mutual exclusion primitive with execution tracing capabilites
/// useful for protecting shared data.
/// 
/// This mutex will behave exactly the same as [`parking_lot::Mutex`] with the
/// exception of its ability to trace lock and unlock events.
/// 
/// For further details with regards to execution tracing, refer to [`TracingRawMutex`].
pub type TracingMutex<T> = Mutex<TracingRawMutex, T>;
