//! Exposing memory management functionality for [`wasm-threadify`].
//! 
//! The sole purpose of this crate is to expose global allocator functions such that
//! [`wasm-threadify`] can insert allocation code for thread-local storage and thread-local
//! stacks while patching the WebAssembly binary. 
//! 
//! While it can be compiled on platforms other than WebAssembly, this crate is intended to
//! be used only in binaries that will be compiled to WebAssembly and processed by 
//! [`wasm-threadify`] afterwards.
//! 
//! [`wasm-threadify`]: https://wasmgrind-d6f2b1.gitlab.io/docs/wasm_threadify/index.html

/*
* The code in this file is mainly based on and taken from the wasm-bindgen tool:
* https://github.com/rustwasm/wasm-bindgen/blob/8fa299f37ae1078db8dbc24ce79f5f071883c87f/src/rt/mod.rs
* 
* Copyright (c) 2014 Alex Crichton
* 
* Permission is hereby granted, free of charge, to any
* person obtaining a copy of this software and associated
* documentation files (the "Software"), to deal in the
* Software without restriction, including without
* limitation the rights to use, copy, modify, merge,
* publish, distribute, sublicense, and/or sell copies of
* the Software, and to permit persons to whom the Software
* is furnished to do so, subject to the following
* conditions:
* 
* The above copyright notice and this permission notice
* shall be included in all copies or substantial portions
* of the Software.
* 
* THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
* ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
* TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
* PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
* SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
* CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
* OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
* IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
* DEALINGS IN THE SOFTWARE.
*/

extern crate alloc;
use alloc::alloc::{Layout, alloc, dealloc, realloc};

mod link;

/// Allocates a block of memory using the global allocator
/// 
/// This function is intended to be used by external tools that intend to manage
/// memory on the heap using the internal global allocator of the binary.
/// 
/// **Attention:** If the `size` argument is equal to zero, the returned pointer
/// has the value of the `align` argument and is therefore bogus.
/// 
/// # Panics
/// 
/// This function may fail, i.e. result in immediate abortion of the process,
/// if either of the following cases occurs:
/// - the specified `size` and `align` could not be converted into a valid
///   [`Layout`].
/// - The global allocator returned a null-pointer upon calling 
///   [`function@alloc`].
#[unsafe(no_mangle)]
pub extern "C" fn __wasmgrind_malloc(size: usize, align: usize) -> *mut u8 {
    if let Ok(layout) = Layout::from_size_align(size, align) {
        unsafe {
            if layout.size() > 0 {
                let ptr = alloc(layout);
                if !ptr.is_null() {
                    return ptr;
                }
            } else {
                return align as *mut u8;
            }
        }
    }

    malloc_failure();
}

/// Reallocates an existing memory block with a new size using the global allocator.
/// 
/// This function is intended to be used by external tools that intend to manage
/// memory on the heap using the internal global allocator of the binary.
/// 
/// # Safety
/// 
/// The `ptr` argument has to be a valid pointer to a memory block of size `old_size`
/// with alignment `align` that has been allocated by the global allocator,
/// i.e., it has been allocated via [`__wasmgrind_malloc`] or [`__wasmgrind_realloc`].
/// 
/// # Panics
/// 
/// This function may fail, i.e. result in immediate abortion of the process,
/// if either of the following cases occurs:
/// - the specified `old_size` and `align` could not be converted into a valid
///   [`Layout`].
/// - The global allocator returned a null-pointer upon calling 
///   [`function@realloc`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wasmgrind_realloc(
    ptr: *mut u8,
    old_size: usize,
    new_size: usize,
    align: usize,
) -> *mut u8 {
    debug_assert!(old_size > 0);
    debug_assert!(new_size > 0);
    if let Ok(layout) = Layout::from_size_align(old_size, align) {
        let ptr = unsafe { realloc(ptr, layout, new_size) };
        if !ptr.is_null() {
            return ptr;
        }
    }
    malloc_failure();
}

#[cold]
fn malloc_failure() -> ! {
    cfg_if::cfg_if! {
        if #[cfg(all(
            target_arch = "wasm32",
            any(target_os = "unknown", target_os = "none")
        ))] {
            core::arch::wasm32::unreachable();
        } else {
            std::process::abort();
        }
    }
}

/// Frees an existing memory block using the global allocator.
/// 
/// This function is intended to be used by external tools that intend to manage
/// memory on the heap using the internal global allocator of the binary.
/// 
/// # Safety
/// 
/// The `ptr` argument has to be a valid pointer to a memory block of size `size`
/// with alignment `align` that has been allocated by the global allocator,
/// i.e., it has been allocated via [`__wasmgrind_malloc`] or [`__wasmgrind_realloc`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wasmgrind_free(ptr: *mut u8, size: usize, align: usize) {
    // This happens for zero-length slices, and in that case `ptr` is
    // likely bogus so don't actually send this to the system allocator
    if size == 0 {
        return;
    }
    let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
    unsafe { dealloc(ptr, layout) };
}

/// Empty internal helper function. Only useful for linking purposes.
/// 
/// This function serves as an unfortunate hack to resolve the linker issues,
/// which are also present in the wasm-bindgen tool. Refer to 
/// [this](https://github.com/rustwasm/wasm-bindgen/blob/8fa299f37ae1078db8dbc24ce79f5f071883c87f/src/rt/mod.rs#L490-L522)
/// comment for more details.
pub fn link_mem_intrinsics() {
    crate::link::link_intrinsics();
}
