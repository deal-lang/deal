//! Rust FFI bindings for the DEAL compiler core.
//!
//! Wave 0 surface: the six C ABI exports declared in `include/deal.h`.
//! Higher-level safe wrappers live in Phase 2 (CLI) and Phase 3 (LSP);
//! this crate exposes only the raw `extern "C"` declarations so the
//! integration tests under `tests/parse.rs` can call directly.

#![allow(non_camel_case_types)]

#[repr(C)]
pub struct DealHandle {
    _opaque: [u8; 0],
    // Mark !Send / !Sync — per-handle thread affinity per D-13.
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

extern "C" {
    pub fn deal_parse(
        source_ptr: *const u8,
        source_len: usize,
        filename_ptr: *const u8,
        filename_len: usize,
    ) -> *mut DealHandle;

    pub fn deal_free(handle: *mut DealHandle);

    pub fn deal_has_errors(handle: *mut DealHandle) -> bool;

    pub fn deal_diagnostics_count(handle: *mut DealHandle) -> u32;

    pub fn deal_ast_json(
        handle: *mut DealHandle,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    ) -> bool;

    pub fn deal_diagnostics_json(
        handle: *mut DealHandle,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    ) -> bool;
}
