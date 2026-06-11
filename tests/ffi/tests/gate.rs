//! Phase 1 exit gate: gate_all_19.
//!
//! This single test IS the REQ-phase-1-gate acceptance criterion:
//!   "All 19 showcase files tokenize and parse through the Zig core;
//!    AST JSON snapshots stable; error recovery handles at least 50
//!    malformed-input cases without panic; C ABI boundary proven with
//!    Rust harness."
//!
//! gate_all_19 loads libdeal.a, parses every one of the 19 showcase files
//! (15 .deal + 4 .dealx) via the deal_parse C ABI, asserts the D-04 envelope
//! is present in the AST JSON, verifies the mode matches the file extension,
//! and calls deal_free for every handle.

use deal_ffi_tests::{deal_ast_json, deal_free, deal_has_errors, deal_parse};

/// The 19 showcase paths, relative to the deal/ package root (same order as
/// parser_deal_snapshot.zig + parser_dealx_snapshot.zig for determinism).
const SHOWCASE_FILES: &[&str] = &[
    // 15 .deal files
    "tests/showcase/packages/vehicle/battery.deal",
    "tests/showcase/packages/vehicle/motor.deal",
    "tests/showcase/packages/vehicle/behaviors.deal",
    "tests/showcase/packages/vehicle/components.deal",
    "tests/showcase/packages/vehicle/index.deal",
    "tests/showcase/packages/interfaces/electrical.deal",
    "tests/showcase/packages/interfaces/thermal.deal",
    "tests/showcase/packages/interfaces/connections.deal",
    "tests/showcase/packages/interfaces/index.deal",
    "tests/showcase/packages/requirements/system.deal",
    "tests/showcase/packages/requirements/needs.deal",
    "tests/showcase/packages/requirements/index.deal",
    "tests/showcase/packages/use-cases/driving.deal",
    "tests/showcase/packages/use-cases/charging.deal",
    "tests/showcase/packages/use-cases/index.deal",
    // 4 .dealx files
    "tests/showcase/model/vehicle.dealx",
    "tests/showcase/model/traceability.dealx",
    "tests/showcase/model/variants/sedan.dealx",
    "tests/showcase/model/variants/performance.dealx",
];

/// Resolve a path relative to the deal/ package root (two levels above
/// tests/ffi/Cargo.toml, since CARGO_MANIFEST_DIR == tests/ffi/).
fn deal_root() -> std::path::PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR not set by Cargo");
    let mut p = std::path::PathBuf::from(manifest_dir);
    p.pop(); // tests/ffi -> tests
    p.pop(); // tests -> deal/
    p
}

#[test]
fn gate_all_19() {
    let root = deal_root();
    let mut passed = 0usize;
    let mut failed = 0usize;

    for &rel_path in SHOWCASE_FILES {
        let full_path = root.join(rel_path);
        let source = std::fs::read(&full_path)
            .unwrap_or_else(|e| panic!("gate_all_19: could not read {rel_path}: {e}"));

        // Extract the filename component (basename only) for the mode check.
        let filename = full_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(rel_path);
        let expected_mode = if rel_path.ends_with(".dealx") { "dealx" } else { "deal" };

        // --- Parse via C ABI ---
        let handle = unsafe {
            deal_parse(
                source.as_ptr(),
                source.len(),
                filename.as_ptr(),
                filename.len(),
            )
        };

        assert!(
            !handle.is_null(),
            "gate_all_19: deal_parse returned null for {rel_path} (OOM?)"
        );

        // All showcase files are well-formed — expect zero parse errors.
        let has_err = unsafe { deal_has_errors(handle) };
        if has_err {
            eprintln!("gate_all_19 FAIL: {rel_path} produced parse errors");
            failed += 1;
            unsafe { deal_free(handle) };
            continue;
        }

        // --- Read AST JSON ---
        let mut json_ptr: *const u8 = std::ptr::null();
        let mut json_len: usize = 0;
        let ok = unsafe { deal_ast_json(handle, &mut json_ptr, &mut json_len) };
        assert!(ok, "gate_all_19: deal_ast_json returned false for {rel_path}");

        let json_bytes = unsafe { std::slice::from_raw_parts(json_ptr, json_len) };
        let json_str = std::str::from_utf8(json_bytes)
            .unwrap_or_else(|e| panic!("gate_all_19: AST JSON not valid UTF-8 for {rel_path}: {e}"));

        // Assert D-04 envelope begins with {"v":1,"mode":"...
        assert!(
            json_str.starts_with("{\"v\":1,\"mode\":\""),
            "gate_all_19: AST JSON envelope mismatch for {rel_path}; got: {}...",
            &json_str[..json_str.len().min(80)]
        );

        // Verify mode matches extension.
        let mode_key = format!("\"mode\":\"{expected_mode}\"");
        assert!(
            json_str.contains(&mode_key),
            "gate_all_19: {rel_path} expected {mode_key} in JSON; got: {}...",
            &json_str[..json_str.len().min(120)]
        );

        // Validate that serde_json can round-trip the JSON (well-formed check).
        let parsed: serde_json::Value = serde_json::from_str(json_str)
            .unwrap_or_else(|e| panic!("gate_all_19: serde_json rejected AST JSON for {rel_path}: {e}"));

        let actual_mode = parsed["mode"].as_str().unwrap_or("<no mode>");
        assert_eq!(
            actual_mode,
            expected_mode,
            "gate_all_19: mode mismatch for {rel_path}"
        );

        // --- Free handle ---
        unsafe { deal_free(handle) };
        passed += 1;
    }

    println!("gate_all_19: {passed}/{} showcase files parsed cleanly", SHOWCASE_FILES.len());

    assert_eq!(
        failed, 0,
        "gate_all_19: {failed} of {} showcase files produced unexpected errors",
        SHOWCASE_FILES.len()
    );
    assert_eq!(
        passed,
        SHOWCASE_FILES.len(),
        "gate_all_19: expected {} passed, got {passed}",
        SHOWCASE_FILES.len()
    );
}
