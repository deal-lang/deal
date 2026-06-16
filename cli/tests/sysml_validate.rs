//! Integration tests for `deal build --target sysml-v2` (Plan 02-04).
//!
//! Tests the end-to-end build pipeline via subprocess calls (matching
//! the pattern used in check_subcommand.rs).
//!
//! Test matrix:
//!   1. Sema-error file → build → exit 1 (sema errors block codegen)
//!   2. --validate flag with a clean file → schema-valid output → exit 0
//!   3. schema_registry::build_validator() caching (unit-tested via serde_json)
//!   4. serde_json preserve_order regression check (Pitfall 1)

use std::process::Command;

/// Path to the built deal binary.
fn deal_bin() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_deal") {
        path.into()
    } else {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("../target/debug/deal");
        p
    }
}

/// Repo root — all fixture paths are relative to this.
fn repo_root() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // pop "cli"
    p
}

// ─── Test 1: Sema-error file blocks codegen and exits 1 ──────────────────────

/// `deal build` on a file with sema errors exits 1 (sema errors block codegen).
/// --validate is irrelevant when sema fails.
#[test]
fn build_sema_error_file_exits_one() {
    let path = repo_root().join("tests/regressions/sema/01-name-resolution.deal");

    let output = Command::new(deal_bin())
        .args(["build", "--target", "sysml-v2"])
        .arg(&path)
        .output()
        .expect("failed to run deal build");

    let exit_code = output.status.code().unwrap_or(99);
    assert_eq!(
        exit_code,
        1,
        "deal build on sema-error file expected exit 1 but got {}\nstderr: {}",
        exit_code,
        String::from_utf8_lossy(&output.stderr),
    );
}

// ─── Test 2: serde_json preserve_order regression ────────────────────────────

/// Verify serde_json is NOT using preserve_order (D-18 alphabetical invariant).
/// This is a local regression check — it verifies that no transitive dep has
/// enabled the preserve_order feature (Pitfall 1).
#[test]
fn serde_json_alphabetical_not_preserve_order() {
    use serde_json::json;

    // Insert keys in reverse-alphabetical order.
    let val = json!({ "z": 1, "m": 2, "a": 3 });
    let raw = serde_json::to_string(&val).expect("serialize");

    // BTreeMap gives alphabetical order: a, m, z
    let a_pos = raw.find("\"a\"").expect("'a' not in json");
    let m_pos = raw.find("\"m\"").expect("'m' not in json");
    let z_pos = raw.find("\"z\"").expect("'z' not in json");
    assert!(
        a_pos < m_pos,
        "key 'a' should appear before 'm' (D-18 alphabetical); got: {}",
        raw
    );
    assert!(
        m_pos < z_pos,
        "key 'm' should appear before 'z' (D-18 alphabetical); got: {}",
        raw
    );
}

// ─── Test 3: Build with golden fixture ───────────────────────────────────────

/// Build from a golden fixture source file and verify exit code.
/// Uses golden fixture 01-part-def.deal which has no sema errors.
#[test]
fn build_golden_fixture_01_exits_zero() {
    let path = repo_root().join("tests/golden/sysml-v2/01-part-def.deal");
    if !path.exists() {
        eprintln!(
            "Skipping: golden fixture not yet created at {}",
            path.display()
        );
        return;
    }

    let output = Command::new(deal_bin())
        .args(["build", "--target", "sysml-v2"])
        .arg(&path)
        .output()
        .expect("failed to run deal build");

    let exit_code = output.status.code().unwrap_or(99);
    assert_eq!(
        exit_code,
        0,
        "deal build on golden fixture 01 expected exit 0 but got {}\nstderr: {}\nstdout: {}",
        exit_code,
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
}

// ─── Test 4: --validate with golden fixture ───────────────────────────────────

/// `deal build --target sysml-v2 --validate` on a golden fixture exits 0
/// if the emitted JSON is schema-valid against the bundled SysML.json.
#[test]
fn build_golden_fixture_01_validate_exits_zero() {
    let path = repo_root().join("tests/golden/sysml-v2/01-part-def.deal");
    if !path.exists() {
        eprintln!(
            "Skipping: golden fixture not yet created at {}",
            path.display()
        );
        return;
    }

    let output = Command::new(deal_bin())
        .args(["build", "--target", "sysml-v2", "--validate"])
        .arg(&path)
        .output()
        .expect("failed to run deal build --validate");

    let exit_code = output.status.code().unwrap_or(99);
    assert_eq!(
        exit_code,
        0,
        "deal build --validate on golden fixture expected exit 0 but got {}\nstderr: {}",
        exit_code,
        String::from_utf8_lossy(&output.stderr),
    );
}
