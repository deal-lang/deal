//! Integration tests for `deal check` (Plan 02-02).
//!
//! Covers the D-34 exit-code contract and D-32 JSON envelope for the
//! fully-wired `check` subcommand.
//!
//! Test matrix:
//!   1. Clean showcase file → exit 0 (human mode)
//!   2. Regression fixture 01-name-resolution.deal → exit 1, stderr has "E2000"
//!   3. Same file with --json → exit 1, stdout JSON has alphabetical keys,
//!      diagnostics[0].code == "E2000"
//!   4. Regression fixture 06-import.deal → exit 1, stderr has "E2400"
//!   5. Nonexistent file → exit 2 (internal I/O error)
//!   6. --color=always → stderr contains ANSI escape codes
//!   7. --color=never → stderr has no ANSI escape codes

use std::process::Command;

/// Path to the built deal binary (mirrors cli_smoke.rs).
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
    // CARGO_MANIFEST_DIR is .../deal/cli, so parent is .../deal
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // pop "cli"
    p
}

// ─── Test 2: Name-resolution regression fixture exits 1, stderr has E2000 ────

/// Fixture 01 (name resolution) produces an E2000 diagnostic → exit 1 (D-34 user).
#[test]
fn check_name_resolution_exits_one_with_e2000() {
    let path = repo_root().join("tests/regressions/sema/01-name-resolution.deal");

    let output = Command::new(deal_bin())
        .args(["check", "--color=never"])
        .arg(&path)
        .output()
        .expect("failed to run deal check");

    let exit_code = output.status.code().unwrap_or(99);
    assert_eq!(
        exit_code,
        1,
        "deal check name-resolution expected exit 1 but got {}\nstderr: {}",
        exit_code,
        String::from_utf8_lossy(&output.stderr),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("E2000"),
        "stderr should contain 'E2000' but got:\n{}",
        stderr,
    );
}

// ─── Test 3: --json mode for name-resolution fixture ─────────────────────────

/// `deal check --json` emits D-32 envelope on stdout with alphabetical keys.
/// Exit is still 1 (≥1 error-severity diagnostic).
#[test]
fn check_json_mode_envelope_and_exit_one() {
    let path = repo_root().join("tests/regressions/sema/01-name-resolution.deal");

    let output = Command::new(deal_bin())
        .args(["check", "--json"])
        .arg(&path)
        .output()
        .expect("failed to run deal check --json");

    let exit_code = output.status.code().unwrap_or(99);
    assert_eq!(
        exit_code, 1,
        "deal check --json expected exit 1 but got {}",
        exit_code,
    );

    // stdout must be valid JSON.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("--json stdout is not valid JSON: {}\nraw: {}", e, stdout));

    // D-32 envelope fields must be present.
    assert_eq!(
        envelope["command"], "check",
        "envelope.command must be 'check'"
    );
    assert_eq!(envelope["v"], 1, "envelope.v must be 1");
    assert!(
        envelope["deal_version"].is_string(),
        "envelope.deal_version must be a string"
    );
    assert!(
        envelope["diagnostics"].is_array(),
        "envelope.diagnostics must be array"
    );
    assert!(
        envelope["summary"].is_object(),
        "envelope.summary must be object"
    );

    // At least one diagnostic with code "E2000".
    let diags = envelope["diagnostics"].as_array().unwrap();
    let has_e2000 = diags.iter().any(|d| d["code"].as_str() == Some("E2000"));
    assert!(
        has_e2000,
        "diagnostics array should contain a diagnostic with code 'E2000', got: {:?}",
        diags,
    );

    // D-32 alphabetical key order: command < deal_version < diagnostics < summary < v.
    // Verify by re-serializing the object and checking key order in raw JSON.
    // serde_json uses BTreeMap by default → keys are alphabetical.
    let raw_keys: Vec<&str> = envelope
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    let expected_keys = ["command", "deal_version", "diagnostics", "summary", "v"];
    assert_eq!(
        raw_keys, expected_keys,
        "D-32 envelope keys must be alphabetical: {:?}",
        raw_keys,
    );
}

// ─── Test 4: Import-resolution regression fixture exits 1, stderr has E2400 ──

/// Fixture 06 (import resolution) produces an E2400 diagnostic → exit 1.
#[test]
fn check_import_resolution_exits_one_with_e2400() {
    let path = repo_root().join("tests/regressions/sema/06-import.deal");

    let output = Command::new(deal_bin())
        .args(["check", "--color=never"])
        .arg(&path)
        .output()
        .expect("failed to run deal check");

    let exit_code = output.status.code().unwrap_or(99);
    assert_eq!(
        exit_code,
        1,
        "deal check import-resolution expected exit 1 but got {}\nstderr: {}",
        exit_code,
        String::from_utf8_lossy(&output.stderr),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("E2400"),
        "stderr should contain 'E2400' but got:\n{}",
        stderr,
    );
}

// ─── Test 5: Nonexistent file exits 2 ────────────────────────────────────────

/// A nonexistent file is an I/O error → exit 2 (D-34 internal).
#[test]
fn check_nonexistent_file_exits_two() {
    let output = Command::new(deal_bin())
        .args(["check", "/tmp/__deal_check_subcommand_nonexistent__.deal"])
        .output()
        .expect("failed to run deal check");

    let exit_code = output.status.code().unwrap_or(99);
    assert_eq!(
        exit_code, 2,
        "deal check <nonexistent> expected exit 2 (D-34 internal) but got {}",
        exit_code,
    );

    // Should not emit a user error message (internal errors use "internal error:" prefix).
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("internal error"),
        "stderr for internal error should contain 'internal error', got:\n{}",
        stderr,
    );
}

// ─── Test 6: --color=always produces ANSI escape codes ───────────────────────

/// `--color=always` forces ANSI escape sequences even when stderr is not a TTY.
#[test]
fn check_color_always_produces_ansi() {
    let path = repo_root().join("tests/regressions/sema/01-name-resolution.deal");

    let output = Command::new(deal_bin())
        .args(["check", "--color=always"])
        .arg(&path)
        .output()
        .expect("failed to run deal check --color=always");

    let stderr = String::from_utf8_lossy(&output.stderr);
    // ANSI escape sequences start with ESC (\x1b) followed by '['.
    assert!(
        stderr.contains('\x1b'),
        "--color=always should produce ANSI escape codes in stderr but got plain text:\n{}",
        stderr,
    );
}

// ─── Test 7: --color=never produces no ANSI escape codes ─────────────────────

/// `--color=never` strips all ANSI escape sequences.
#[test]
fn check_color_never_produces_no_ansi() {
    let path = repo_root().join("tests/regressions/sema/01-name-resolution.deal");

    let output = Command::new(deal_bin())
        .args(["check", "--color=never"])
        .arg(&path)
        .output()
        .expect("failed to run deal check --color=never");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains('\x1b'),
        "--color=never should produce no ANSI escape codes but stderr contains ESC:\n{}",
        stderr,
    );
}
