//! Integration tests for the DEAL C ABI via Rust FFI.
//!
//! Plan 01-06 extends the Plan 01-01 stub-level smoke tests with real parsing:
//! - ffi_smoke: legacy smoke test (now exercises real parser)
//! - ffi_dealx_mode_detection: extended with real content check
//! - ffi_free_null_is_safe: unchanged
//! - ffi_parse_battery_real: parse a real showcase .deal file
//! - ffi_parse_dealx_real: parse a real showcase .dealx file
//! - ffi_diagnostics_on_malformed: parse a malformed file and read diagnostics
//! - ffi_ast_json_cache_stable: calling deal_ast_json twice returns identical bytes
//! - ffi_invalid_utf8: invalid UTF-8 input returns handle with E0001 diagnostic

use std::ptr;

use deal_ffi_tests::{
    deal_ast_json, deal_diagnostics_count, deal_diagnostics_json, deal_free, deal_has_errors,
    deal_parse, DealHandle,
};

fn parse_into_handle(source: &[u8], filename: &[u8]) -> *mut DealHandle {
    unsafe {
        deal_parse(
            source.as_ptr(),
            source.len(),
            filename.as_ptr(),
            filename.len(),
        )
    }
}

fn read_ast_json(handle: *mut DealHandle) -> String {
    let mut ptr: *const u8 = ptr::null();
    let mut len: usize = 0;
    let ok = unsafe { deal_ast_json(handle, &mut ptr, &mut len) };
    assert!(ok, "deal_ast_json returned false on a non-null handle");
    assert!(!ptr.is_null(), "deal_ast_json wrote a null out_ptr");
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(bytes)
        .expect("AST JSON not valid UTF-8")
        .to_owned()
}

fn read_ast_json_raw(handle: *mut DealHandle) -> (*const u8, usize) {
    let mut ptr: *const u8 = ptr::null();
    let mut len: usize = 0;
    let ok = unsafe { deal_ast_json(handle, &mut ptr, &mut len) };
    assert!(ok, "deal_ast_json returned false");
    (ptr, len)
}

fn read_diag_json(handle: *mut DealHandle) -> Vec<u8> {
    let mut ptr: *const u8 = ptr::null();
    let mut len: usize = 0;
    let ok = unsafe { deal_diagnostics_json(handle, &mut ptr, &mut len) };
    assert!(
        ok,
        "deal_diagnostics_json returned false on a non-null handle"
    );
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    bytes.to_vec()
}

/// Helper: resolve path relative to the deal/ package root (the directory
/// containing Cargo.toml and tests/ffi/).
fn deal_root() -> std::path::PathBuf {
    // The manifest file is tests/ffi/Cargo.toml, so the package root is two
    // levels up from the CARGO_MANIFEST_DIR env var.
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set by Cargo");
    let mut p = std::path::PathBuf::from(manifest_dir);
    p.pop(); // tests/ffi -> tests
    p.pop(); // tests -> deal/
    p
}

// ─── Legacy smoke tests (Plan 01-01, still exercising the real parser) ────────

#[test]
fn ffi_smoke() {
    let source: &[u8] = b"";
    let filename: &[u8] = b"test.deal";

    let handle = parse_into_handle(source, filename);
    assert!(
        !handle.is_null(),
        "deal_parse returned null on empty source"
    );

    let count = unsafe { deal_diagnostics_count(handle) };
    assert_eq!(count, 0, "empty source should produce zero diagnostics");

    let has_err = unsafe { deal_has_errors(handle) };
    assert!(!has_err, "empty source should report no errors");

    let json = read_ast_json(handle);
    assert!(
        json.contains("\"v\":1"),
        "AST JSON missing \"v\":1 — got {json}"
    );
    assert!(
        json.contains("\"mode\":\"deal\""),
        "AST JSON missing \"mode\":\"deal\" — got {json}"
    );
    assert!(
        json.contains("\"filename\":\"test.deal\""),
        "AST JSON missing the filename — got {json}"
    );
    // Empty source produces a real deal_file root node (wired in Plan 03).
    assert!(
        json.contains("\"root\":{"),
        "AST JSON missing real root node — got {json}"
    );

    unsafe { deal_free(handle) };
}

#[test]
fn ffi_dealx_mode_detection() {
    let source: &[u8] = b"";
    let filename: &[u8] = b"model/example.dealx";

    let handle = parse_into_handle(source, filename);
    assert!(!handle.is_null());

    let json = read_ast_json(handle);
    assert!(
        json.contains("\"mode\":\"dealx\""),
        "filename ending in .dealx should select dealx mode — got {json}"
    );
    assert!(
        json.contains("\"filename\":\"model/example.dealx\""),
        "AST JSON missing the .dealx filename — got {json}"
    );

    unsafe { deal_free(handle) };
}

#[test]
fn ffi_free_null_is_safe() {
    // `deal_free(NULL)` must be a no-op (matches the documented contract in
    // include/deal.h). Verified once here so future plans can rely on it.
    unsafe { deal_free(ptr::null_mut()) };
}

// ─── Plan 06 real-parser tests ────────────────────────────────────────────────

#[test]
fn ffi_parse_battery_real() {
    let root = deal_root();
    let battery_path = root.join("tests/showcase/packages/vehicle/battery.deal");
    let source = std::fs::read(&battery_path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", battery_path.display()));

    let filename = b"battery.deal";
    let handle = parse_into_handle(&source, filename);
    assert!(!handle.is_null(), "deal_parse returned null");

    // battery.deal is well-formed — no parse errors expected.
    let has_err = unsafe { deal_has_errors(handle) };
    assert!(!has_err, "battery.deal is well-formed; expected no errors");

    let json = read_ast_json(handle);

    // D-04 envelope.
    assert!(json.contains("\"v\":1"), "missing v:1 — got {json}");
    assert!(
        json.contains("\"mode\":\"deal\""),
        "missing mode:deal — got {json}"
    );
    assert!(
        json.contains("\"filename\":\"battery.deal\""),
        "missing filename — got {json}"
    );
    // Deal file root node.
    assert!(
        json.contains("\"k\":\"deal_file\""),
        "missing k:deal_file — got {json}"
    );

    unsafe { deal_free(handle) };
}

#[test]
fn ffi_parse_dealx_real() {
    let root = deal_root();
    let vehicle_path = root.join("tests/showcase/model/vehicle.dealx");
    let source = std::fs::read(&vehicle_path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", vehicle_path.display()));

    let filename = b"vehicle.dealx";
    let handle = parse_into_handle(&source, filename);
    assert!(!handle.is_null(), "deal_parse returned null");

    let has_err = unsafe { deal_has_errors(handle) };
    assert!(!has_err, "vehicle.dealx is well-formed; expected no errors");

    let json = read_ast_json(handle);

    assert!(
        json.contains("\"mode\":\"dealx\""),
        "missing mode:dealx — got {json}"
    );
    assert!(
        json.contains("\"k\":\"dealx_file\""),
        "missing k:dealx_file — got {json}"
    );
    // vehicle.dealx contains the canonical connect example (D-09 first-class kind).
    assert!(
        json.contains("\"k\":\"comp_connect\""),
        "vehicle.dealx should have at least one comp_connect node — got {json}"
    );

    unsafe { deal_free(handle) };
}

#[test]
fn ffi_diagnostics_on_malformed() {
    let root = deal_root();
    let malformed_path = root.join("tests/malformed/m02_unclosed_brace.deal");
    let source = std::fs::read(&malformed_path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", malformed_path.display()));

    let filename = b"m02_unclosed_brace.deal";
    let handle = parse_into_handle(&source, filename);
    assert!(!handle.is_null(), "deal_parse returned null");

    // Malformed input must produce at least one diagnostic.
    let has_err = unsafe { deal_has_errors(handle) };
    assert!(has_err, "malformed input should produce errors");

    let count = unsafe { deal_diagnostics_count(handle) };
    assert!(count >= 1, "expected ≥1 diagnostic, got {count}");

    // Parse diagnostics JSON via serde_json and verify structure.
    let diag_bytes = read_diag_json(handle);
    let diags: serde_json::Value = serde_json::from_slice(&diag_bytes).unwrap_or_else(|e| {
        panic!(
            "diagnostics JSON parse failed: {e}\nJSON: {:?}",
            std::str::from_utf8(&diag_bytes)
        )
    });

    let arr = diags.as_array().expect("diagnostics JSON must be an array");
    assert!(!arr.is_empty(), "diagnostics array must not be empty");

    // At least one diagnostic code must be in the E0100..E0499 parser range.
    let has_parser_error = arr.iter().any(|d| {
        if let Some(code) = d.get("code").and_then(|c| c.as_str()) {
            // Parser error codes are E0100..E0499.
            if let Some(num_str) = code.strip_prefix('E') {
                if let Ok(num) = num_str.parse::<u32>() {
                    return (100..500).contains(&num);
                }
            }
        }
        false
    });
    assert!(
        has_parser_error,
        "expected at least one parser error code (E0100..E0499) in diagnostics: {diags}"
    );

    unsafe { deal_free(handle) };
}

#[test]
fn ffi_ast_json_cache_stable() {
    // D-11: calling deal_ast_json twice returns identical (ptr, len) — same
    // pointer, same bytes. The buffer is cached in the handle's arena.
    let source: &[u8] = b"";
    let filename: &[u8] = b"cache_test.deal";

    let handle = parse_into_handle(source, filename);
    assert!(!handle.is_null());

    let (ptr1, len1) = read_ast_json_raw(handle);
    let (ptr2, len2) = read_ast_json_raw(handle);

    assert_eq!(
        ptr1, ptr2,
        "deal_ast_json must return the same pointer on second call (D-11 caching)"
    );
    assert_eq!(
        len1, len2,
        "deal_ast_json must return the same length on second call"
    );

    // Also verify the bytes are identical.
    let bytes1 = unsafe { std::slice::from_raw_parts(ptr1, len1) };
    let bytes2 = unsafe { std::slice::from_raw_parts(ptr2, len2) };
    assert_eq!(
        bytes1, bytes2,
        "deal_ast_json must return identical bytes on second call"
    );

    unsafe { deal_free(handle) };
}

#[test]
fn ffi_invalid_utf8() {
    // T-06-01: invalid UTF-8 input returns a non-null handle carrying exactly
    // one diagnostic with code "E0001". The AST JSON root must be null.
    let invalid_bytes: &[u8] = &[0xff, 0xfe, 0xfd];
    let filename: &[u8] = b"invalid.deal";

    let handle = parse_into_handle(invalid_bytes, filename);
    assert!(
        !handle.is_null(),
        "deal_parse must return non-null for invalid UTF-8 (D-10)"
    );

    let has_err = unsafe { deal_has_errors(handle) };
    assert!(has_err, "invalid UTF-8 must produce at least one error");

    let count = unsafe { deal_diagnostics_count(handle) };
    assert_eq!(
        count, 1,
        "invalid UTF-8 must produce exactly one diagnostic (E0001)"
    );

    let diag_bytes = read_diag_json(handle);
    let diags: serde_json::Value = serde_json::from_slice(&diag_bytes)
        .unwrap_or_else(|e| panic!("diagnostics JSON parse failed: {e}"));
    let arr = diags.as_array().expect("must be array");
    assert_eq!(arr.len(), 1, "must have exactly one diagnostic");
    let code = arr[0]["code"].as_str().expect("code must be string");
    assert_eq!(
        code, "E0001",
        "invalid UTF-8 must produce E0001, got {code}"
    );

    // AST JSON: root must be null (parser not invoked on invalid UTF-8).
    let ast_json = read_ast_json(handle);
    assert!(
        ast_json.contains("\"root\":null"),
        "invalid UTF-8 handle must have null root in AST JSON — got {ast_json}"
    );

    unsafe { deal_free(handle) };
}
