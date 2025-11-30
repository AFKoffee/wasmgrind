#![allow(non_camel_case_types)]

// We hardcoded the datatypes here instead of using c-types
// to exactly match the 32bit wali-musl wasm layout

#[repr(C)]
pub struct wasm_sigaction_t {
    pub wasm_handler: u32,
    pub flags: u64,
    pub wasm_restorer: u32,
    pub mask: [u32; 2],
}

impl wasm_sigaction_t {
    pub const WASM_SIG_ERR: u32 = 0xFFFFFFFF;
    pub const WASM_SIG_DFL: u32 = 0;
    pub const WASM_SIG_IGN: u32 = 0xFFFFFFFE;
}

#[repr(C)]
pub struct wasm_iovec_t {
    pub iov_base: u32,
    pub iov_len: u32,
}
