//! Shared Rust FFI bindings for the DEAL compiler core (libdeal.a).
//!
//! This crate is the single source of truth for the 9 C ABI exports
//! declared in `deal/include/deal.h`. Consumers (`cli/` for batch operations
//! and `lsp/` for editor integration) depend on this crate rather than
//! redeclaring the extern block — Cargo enforces exactly one `links = "deal"`
//! claim across the workspace, so this crate owns it (Plan 03-03 Task 1
//! extracted this surface from cli/src/ffi.rs into deal-ffi/).
//!
//! ## ABI surface (9 exports — locked at Phase 2 closeout)
//!
//! 1. `deal_parse`            — Parse source bytes; returns owned handle.
//! 2. `deal_free`             — Release the handle's arena.
//! 3. `deal_has_errors`       — Did the parse produce any error diagnostics?
//! 4. `deal_diagnostics_count`— How many diagnostics did the handle accumulate?
//! 5. `deal_diagnostics_json` — Diagnostic envelope (D-32) as UTF-8 JSON.
//! 6. `deal_ast_json`         — AST encoded as UTF-8 JSON.
//! 7. `deal_ir_json`          — IR document as UTF-8 JSON.
//! 8. `deal_format`           — Canonical formatted DEAL source bytes.
//! 9. `deal_index_json`       — Sema workspace index (`.deal/index.json`).
//!
//! ## Ownership invariants (D-11, T-02-29)
//!
//! - The handle owns the arena. After `deal_free`, all `*const u8` pointers
//!   returned by any JSON accessor are invalidated — clone bytes BEFORE
//!   freeing the handle.
//! - Per-handle thread affinity historically (D-13), but Plan 03-03 adds the
//!   `OwnedDealHandle` Send wrapper (D-45) so per-document handles can be
//!   passed across tokio tasks. Safety relies on the LSP holding a per-handle
//!   `tokio::sync::Mutex` so only one task touches a given handle at a time.
//!
//! ## Safe wrapper layer (`safe` module)
//!
//! All clone-before-free machinery (Pitfall 3 / T-02-29) is encapsulated in
//! `deal_ffi::safe::*` helpers. LSP and (incrementally) CLI code should call
//! the safe wrappers instead of touching raw pointers.

#![allow(non_camel_case_types)]

use std::marker::{PhantomData, PhantomPinned};

/// Opaque handle returned by `deal_parse`. The handle owns the arena that
/// holds the parsed AST.
///
/// !Send / !Sync at the raw-pointer level — Plan 03-03 adds the
/// `OwnedDealHandle` newtype below for the Send-with-Mutex guarantee.
#[repr(C)]
pub struct DealHandle {
    _opaque: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

extern "C" {
    /// Parse `source` bytes (UTF-8, length-prefixed) with the given filename.
    /// Returns an opaque handle owning the resulting AST + diagnostics.
    /// Caller must eventually call `deal_free` on the returned handle.
    pub fn deal_parse(
        source_ptr: *const u8,
        source_len: usize,
        filename_ptr: *const u8,
        filename_len: usize,
    ) -> *mut DealHandle;

    /// Free the handle and release the arena.
    pub fn deal_free(handle: *mut DealHandle);

    /// Returns `true` if the parse produced any error-severity diagnostics.
    pub fn deal_has_errors(handle: *mut DealHandle) -> bool;

    /// Returns the number of diagnostics attached to this handle.
    pub fn deal_diagnostics_count(handle: *mut DealHandle) -> u32;

    /// Write a JSON encoding of the AST to `*out_ptr` (length `*out_len`).
    /// Arena-owned bytes; clone before `deal_free`.
    pub fn deal_ast_json(
        handle: *mut DealHandle,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    ) -> bool;

    /// Write the diagnostic envelope (D-32) as UTF-8 JSON to `*out_ptr`.
    pub fn deal_diagnostics_json(
        handle: *mut DealHandle,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    ) -> bool;

    /// Emit the IR Document as UTF-8 JSON. (D-22 — Plan 02-03 export #7)
    pub fn deal_ir_json(
        handle: *mut DealHandle,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    ) -> bool;

    /// Emit canonical formatted DEAL source bytes (UTF-8, NOT JSON).
    /// (D-21 — Plan 02-05 export #8)
    pub fn deal_format(
        handle: *mut DealHandle,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    ) -> bool;

    /// Emit `.deal/index.json` bytes (sema symbol-table workspace index).
    /// (Phase 2 closeout export #9)
    pub fn deal_index_json(
        handle: *mut DealHandle,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    ) -> bool;

    /// Analyze DEAL source bytes with a stdlib seed and emit diagnostics JSON.
    ///
    /// Phase 5 C ABI export #10 (D-85, D-88). Exposes `analyzeWithExternalTable`
    /// across the C ABI boundary so the Rust orchestrator can check DEAL files
    /// with the stdlib dimension/unit table seeded.
    ///
    /// The `stdlib_ir_ptr`/`stdlib_ir_len` parameter accepts stdlib DEAL source
    /// bytes (used to seed the external symbol table with dimension/unit entries).
    ///
    /// Returns a non-null opaque handle on success; NULL on allocator failure.
    /// The out_diag_ptr/out_diag_len pair points to arena-owned diagnostics JSON
    /// that MUST be cloned BEFORE calling `deal_free` (Pitfall 3 / T-02-29).
    /// Caller MUST call `deal_free(handle)` to release the arena.
    ///
    /// Diagnostics (has_errors) are also queryable via `deal_has_errors(handle)`.
    pub fn deal_check_with_stdlib(
        source_ptr: *const u8,
        source_len: usize,
        filename_ptr: *const u8,
        filename_len: usize,
        stdlib_ir_ptr: *const u8,
        stdlib_ir_len: usize,
        out_diag_ptr: *mut *const u8,
        out_diag_len: *mut usize,
    ) -> *mut DealHandle;
}

/// Owning wrapper for `*mut DealHandle` introduced in Plan 03-03 (D-45).
///
/// - Drop frees the arena automatically — panics on tokio task paths cannot
///   leak the handle.
/// - `unsafe impl Send` is justified by D-02/D-13: each handle owns an
///   independent arena, no shared state with other handles. The LSP must
///   wrap each handle in a `tokio::sync::Mutex` to enforce single-threaded
///   access per handle (the historical D-13 invariant remains).
/// - NOT `Sync` — only one task may hold a `&OwnedDealHandle` at a time,
///   which the Mutex naturally enforces.
pub struct OwnedDealHandle(*mut DealHandle);

// SAFETY: Each DealHandle owns a private arena (D-02). Moving the handle to
// another thread is safe as long as no two threads dereference the inner
// pointer simultaneously. LSP enforces this by wrapping each handle in a
// per-URI `tokio::sync::Mutex` (D-45).
unsafe impl Send for OwnedDealHandle {}

impl OwnedDealHandle {
    /// Construct from a raw pointer. SAFETY: caller must own the handle and
    /// must not call `deal_free` on it separately — Drop does that.
    ///
    /// Returns `None` if the pointer is null (e.g. `deal_parse` OOM).
    pub fn from_raw(ptr: *mut DealHandle) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self(ptr))
        }
    }

    /// Borrow the raw pointer for an unsafe extern call.
    pub fn as_ptr(&self) -> *mut DealHandle {
        self.0
    }
}

impl Drop for OwnedDealHandle {
    fn drop(&mut self) {
        // SAFETY: We constructed Self only from a non-null pointer obtained
        // via `deal_parse`, and the contract forbids external `deal_free`.
        unsafe {
            deal_free(self.0);
        }
    }
}

/// Safe wrapper helpers that encapsulate the Pitfall 3 / T-02-29
/// clone-before-free invariant. Downstream crates (LSP, future CLI cleanup)
/// should prefer these over the raw extern functions.
pub mod safe {
    use super::*;

    /// Parse `source` for `filename`. Returns `None` on null pointer (OOM).
    ///
    /// The returned handle's arena is freed when the `OwnedDealHandle` is
    /// dropped — there is no manual free step.
    pub fn parse(source: &[u8], filename: &str) -> Option<OwnedDealHandle> {
        // SAFETY: Both slices live for the duration of this call. The Zig
        // side copies the data into the handle's arena, so dangling after
        // return is not a concern.
        let ptr = unsafe {
            deal_parse(
                source.as_ptr(),
                source.len(),
                filename.as_bytes().as_ptr(),
                filename.len(),
            )
        };
        OwnedDealHandle::from_raw(ptr)
    }

    /// Analyze `source` (named `filename`) with an external symbol table
    /// seeded from `external_sources` — stdlib and/or sibling workspace files.
    ///
    /// P2 WS-A / ADR-0003: enables cross-file name resolution. References in
    /// `source` (e.g. a `<<specializes>>` target imported from another file)
    /// resolve against the merged declarations of every provided source, and
    /// the resulting handle's `index_json` carries `references[]` keyed by the
    /// declaring file's fully-qualified id.
    ///
    /// Each source's declarations are keyed by its own `package` declaration,
    /// so the synthesized chunk filenames are irrelevant to resolution.
    /// (Limitation: chunks are parsed in `.deal` mode; `.dealx` sources in the
    /// external set do not currently contribute declarations — follow-up.)
    ///
    /// Returns `None` on null pointer (OOM). The handle frees on drop.
    pub fn check_with_external(
        source: &[u8],
        filename: &str,
        external_sources: &[&[u8]],
    ) -> Option<OwnedDealHandle> {
        // The extern splits the blob on NUL (0x00) — never valid in UTF-8 DEAL
        // source — into one chunk per source.
        let mut blob: Vec<u8> = Vec::new();
        for (i, src) in external_sources.iter().enumerate() {
            if i > 0 {
                blob.push(0);
            }
            blob.extend_from_slice(src);
        }
        let mut diag_ptr: *const u8 = std::ptr::null();
        let mut diag_len: usize = 0;
        // SAFETY: all slices live for the call; Zig copies what it needs into
        // the handle arena. out pointers are valid &mut.
        let ptr = unsafe {
            deal_check_with_stdlib(
                source.as_ptr(),
                source.len(),
                filename.as_bytes().as_ptr(),
                filename.len(),
                blob.as_ptr(),
                blob.len(),
                &mut diag_ptr,
                &mut diag_len,
            )
        };
        OwnedDealHandle::from_raw(ptr)
    }

    /// `true` if the handle's parse produced at least one error-severity
    /// diagnostic.
    pub fn has_errors(handle: &OwnedDealHandle) -> bool {
        // SAFETY: handle owns a valid arena (Drop hasn't run yet).
        unsafe { deal_has_errors(handle.as_ptr()) }
    }

    /// Count of diagnostics on this handle (any severity).
    pub fn diagnostics_count(handle: &OwnedDealHandle) -> u32 {
        // SAFETY: handle owns a valid arena (Drop hasn't run yet).
        unsafe { deal_diagnostics_count(handle.as_ptr()) }
    }

    /// Clone the diagnostic-envelope JSON bytes (D-32) out of the arena.
    /// Returns `None` on Zig-side failure (OOM).
    pub fn diagnostics_json(handle: &OwnedDealHandle) -> Option<Vec<u8>> {
        json_export(handle, deal_diagnostics_json)
    }

    /// Clone the AST JSON bytes out of the arena.
    pub fn ast_json(handle: &OwnedDealHandle) -> Option<Vec<u8>> {
        json_export(handle, deal_ast_json)
    }

    /// Clone the IR JSON bytes out of the arena.
    pub fn ir_json(handle: &OwnedDealHandle) -> Option<Vec<u8>> {
        json_export(handle, deal_ir_json)
    }

    /// Clone the workspace-index JSON bytes out of the arena.
    pub fn index_json(handle: &OwnedDealHandle) -> Option<Vec<u8>> {
        json_export(handle, deal_index_json)
    }

    /// Clone the formatter output bytes (UTF-8 DEAL source, NOT JSON)
    /// out of the arena.
    pub fn format(handle: &OwnedDealHandle) -> Option<Vec<u8>> {
        json_export(handle, deal_format)
    }

    /// Internal: shared shape for any extern that writes `(*out_ptr, *out_len)`
    /// into the handle's arena. Clones the bytes into an owned `Vec<u8>`
    /// while the arena is still alive.
    fn json_export(
        handle: &OwnedDealHandle,
        extern_fn: unsafe extern "C" fn(*mut DealHandle, *mut *const u8, *mut usize) -> bool,
    ) -> Option<Vec<u8>> {
        let mut out_ptr: *const u8 = std::ptr::null();
        let mut out_len: usize = 0;
        // SAFETY: handle is valid (caller borrows &OwnedDealHandle so Drop
        // hasn't run); out_ptr / out_len are valid &mut. Clone happens before
        // returning so we never hand out arena-owned pointers.
        let ok = unsafe { extern_fn(handle.as_ptr(), &mut out_ptr, &mut out_len) };
        if !ok {
            return None;
        }
        if out_ptr.is_null() || out_len == 0 {
            return Some(Vec::new());
        }
        // SAFETY: Zig wrote a valid (ptr, len) pair; arena is still alive.
        let bytes = unsafe { std::slice::from_raw_parts(out_ptr, out_len) }.to_vec();
        Some(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Smoke test: round-trip a trivially-valid DEAL source through the safe
    // wrapper to prove the extraction wires up correctly. This relies on
    // libdeal.a being built and linked — handled by build.rs.
    #[test]
    fn safe_parse_and_diagnostics_round_trip() {
        let source = b"package empty;";
        let handle = safe::parse(source, "smoke.deal").expect("parse returned null");
        // Even a trivial file produces a parseable handle with zero errors.
        assert!(!safe::has_errors(&handle), "trivial source produced errors");
        let diag = safe::diagnostics_json(&handle).expect("diagnostics_json failed");
        // The envelope is a JSON array (empty or otherwise) — verify it's
        // valid UTF-8 starting with `[` (D-32 envelope shape).
        assert!(!diag.is_empty(), "diagnostics_json returned empty bytes");
        assert_eq!(diag[0], b'[', "diagnostics envelope should start with '['");
        // Drop releases the arena.
    }

    #[test]
    fn safe_format_round_trip() {
        let source = b"package fmt_test;";
        let handle = safe::parse(source, "fmt.deal").expect("parse returned null");
        let formatted = safe::format(&handle).expect("format failed");
        // Formatter output is UTF-8 DEAL source — must contain the package keyword.
        let s = std::str::from_utf8(&formatted).expect("formatter emitted non-UTF8");
        assert!(
            s.contains("package"),
            "formatted output missing 'package' keyword"
        );
    }

    // P2 WS-A / ADR-0003: cross-file name resolution across the FFI + JSON
    // boundary. `interfaces.thermal` declares ThermallyManaged; `battery`
    // imports it via the PREFIX package `interfaces` and specializes it (the
    // showcase's loose-import shape). The reference binding must resolve to the
    // declaring file's FQ id and surface in the index envelope's references[].
    #[test]
    fn safe_check_with_external_resolves_cross_file_reference() {
        let interfaces: &[u8] =
            b"package interfaces.thermal;\ninterface def ThermallyManaged { }\n";
        let battery: &[u8] = b"package vehicle.battery;\n\
            import interfaces.{ThermallyManaged};\n\
            part def BatteryPack <<specializes>> ThermallyManaged { }\n";

        let handle = safe::check_with_external(battery, "vehicle/battery.deal", &[interfaces])
            .expect("check_with_external returned null");
        let index = safe::index_json(&handle).expect("index_json failed");
        let s = std::str::from_utf8(&index).expect("index json not UTF-8");

        assert!(
            s.contains("\"resolved_path\":\"interfaces.thermal.ThermallyManaged\""),
            "cross-file <<specializes>> did not resolve to the declaring FQ id; index = {s}"
        );
        assert!(
            s.contains("\"ref_kind\":\"specializes\""),
            "references[] missing the specializes binding; index = {s}"
        );
    }
}
