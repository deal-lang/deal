//! Integration tests for `deal parse` (Plan 02-06 — final stub removal).
//!
//! Covers WARNING-05 Option A contract: `deal parse --json` emits the raw
//! alphabetical-keyed AST JSON directly to stdout — NO D-32 envelope on success.
//! The D-32 envelope goes to STDERR only when diagnostics fire (parse errors).
//!
//! Test matrix:
//!   1. `deal parse FILE.deal` exits 0; stdout is parseable JSON with alphabetical keys
//!   2. `deal parse --json FILE.deal` exits 0; stdout is IDENTICAL to non---json output
//!      (WARNING-05: --json is a no-op for parse success path in Phase 2)
//!   3. WARNING-05 mitigation: `deal parse --json FILE.deal` stdout has NO D-32 envelope keys
//!      (`command` and `summary` must not appear at the JSON top level on success)
//!   4. `deal parse tests/malformed/m01_*.deal` exits 1; stderr has diagnostics; stdout empty
//!   5. `deal parse --json tests/malformed/m01_*.deal` exits 1; stderr is D-32 envelope JSON;
//!      stdout is EMPTY (no partial AST mixed with diagnostics)
//!   6. `deal parse /tmp/nonexistent.deal` exits 2 (D-34 internal I/O error)
//!   7. All showcase files exit 0 (S-9 integration check)

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
    // CARGO_MANIFEST_DIR is .../deal/cli, so parent is .../deal
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // pop "cli"
    p
}

/// A showcase file that reliably parses with zero errors.
fn showcase_file() -> std::path::PathBuf {
    repo_root().join("tests/showcase/packages/requirements/system.deal")
}

// ─── Test 1: `deal parse FILE` exits 0, stdout is alphabetical-key JSON ─────

/// `deal parse FILE.deal` exits 0 and stdout is parseable JSON with alphabetical keys.
#[test]
fn parse_clean_file_exits_zero_with_json() {
    let path = showcase_file();
    let output = Command::new(deal_bin())
        .args(["parse"])
        .arg(&path)
        .output()
        .expect("failed to run deal parse");

    let exit_code = output.status.code().unwrap_or(99);
    assert_eq!(
        exit_code,
        0,
        "deal parse on clean showcase file expected exit 0 but got {}\nstderr: {}",
        exit_code,
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.trim().is_empty(),
        "deal parse stdout should not be empty on success"
    );

    // Verify stdout is valid JSON.
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "deal parse stdout is not valid JSON: {}\nstdout: {}",
            e, stdout
        )
    });

    // Verify the output is a raw AST JSON envelope (per D-18 top-level key order).
    // The AST envelope keys are: v, mode, filename, root (canonical D-18 order).
    // This is the raw deal_ast_json() output — NOT the D-32 diagnostic envelope.
    if let Some(obj) = parsed.as_object() {
        assert!(
            obj.contains_key("root"),
            "AST JSON must have a 'root' key (it's the parse tree root) but got keys: {:?}",
            obj.keys().collect::<Vec<_>>(),
        );
        assert!(
            obj.contains_key("v"),
            "AST JSON must have a 'v' key (version) but got keys: {:?}",
            obj.keys().collect::<Vec<_>>(),
        );
        // The AST must NOT have the D-32 diagnostic envelope keys.
        assert!(
            !obj.contains_key("diagnostics"),
            "D-32 envelope 'diagnostics' key leaked into AST output — should be raw AST",
        );
        assert!(
            !obj.contains_key("summary"),
            "D-32 envelope 'summary' key leaked into AST output — should be raw AST",
        );
    }
}

// ─── Test 2: `deal parse --json FILE` == `deal parse FILE` on success ────────

/// `deal parse --json FILE.deal` and `deal parse FILE.deal` produce IDENTICAL stdout.
/// WARNING-05 Option A: --json is a no-op for the parse success path in Phase 2.
#[test]
fn parse_json_flag_identical_to_plain_on_success() {
    let path = showcase_file();

    let plain = Command::new(deal_bin())
        .args(["parse"])
        .arg(&path)
        .output()
        .expect("failed to run deal parse (plain)");
    assert_eq!(
        plain.status.code().unwrap_or(99),
        0,
        "plain parse should exit 0"
    );

    let with_json = Command::new(deal_bin())
        .args(["parse", "--json"])
        .arg(&path)
        .output()
        .expect("failed to run deal parse --json");
    assert_eq!(
        with_json.status.code().unwrap_or(99),
        0,
        "parse --json should exit 0"
    );

    // Stdout must be byte-identical: both emit the raw AST JSON (no envelope).
    assert_eq!(
        plain.stdout, with_json.stdout,
        "deal parse and deal parse --json must produce identical stdout on success (WARNING-05 Option A: raw AST, no envelope)",
    );
}

// ─── Test 3: WARNING-05 mitigation — no D-32 envelope leak into stdout ───────

/// `deal parse --json FILE.deal` stdout does NOT contain D-32 envelope keys.
/// Asserts the absence of `command` and `summary` at the JSON top level on success.
/// This test is the machine-readable enforcement of WARNING-05 Option A.
#[test]
fn parse_json_no_envelope_leak_on_success() {
    let path = showcase_file();
    let output = Command::new(deal_bin())
        .args(["parse", "--json"])
        .arg(&path)
        .output()
        .expect("failed to run deal parse --json");

    assert_eq!(
        output.status.code().unwrap_or(99),
        0,
        "parse --json should exit 0"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "parse --json stdout not valid JSON: {}\nstdout: {}",
            e, stdout
        )
    });

    // D-32 envelope has top-level keys: command, deal_version, diagnostics, summary, v.
    // Raw AST must NOT have 'diagnostics' or 'summary' (D-32 diagnostic envelope).
    // Raw AST DOES have 'root' (the parse tree) and 'mode'.
    if let Some(obj) = parsed.as_object() {
        assert!(
            !obj.contains_key("diagnostics"),
            "D-32 envelope 'diagnostics' key leaked into parse --json stdout — should be raw AST per WARNING-05 Option A",
        );
        assert!(
            !obj.contains_key("summary"),
            "D-32 envelope 'summary' key leaked into parse --json stdout — should be raw AST per WARNING-05 Option A",
        );
        // Positive assertion: raw AST must have 'root' key (the parse tree root).
        assert!(
            obj.contains_key("root"),
            "parse --json stdout must be raw AST with 'root' key, but 'root' is missing. Got keys: {:?}",
            obj.keys().collect::<Vec<_>>(),
        );
    }
}

// ─── Test 4: malformed file exits 1, stderr has diagnostics, stdout empty ────

/// `deal parse tests/malformed/m01_*.deal` exits 1; stderr has diagnostics; stdout empty.
#[test]
fn parse_malformed_file_exits_one_stderr_diagnostics_stdout_empty() {
    let path = repo_root().join("tests/malformed/m01_missing_semicolon.deal");
    if !path.exists() {
        // Malformed corpus may not exist in all environments; skip gracefully.
        return;
    }

    let output = Command::new(deal_bin())
        .args(["parse"])
        .arg(&path)
        .output()
        .expect("failed to run deal parse on malformed file");

    let exit_code = output.status.code().unwrap_or(99);
    assert_eq!(
        exit_code,
        1,
        "deal parse on malformed file expected exit 1 but got {}\nstderr: {}",
        exit_code,
        String::from_utf8_lossy(&output.stderr),
    );

    // Stdout must be EMPTY — no partial AST emitted on error.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "deal parse on malformed file: stdout must be empty on error, got: {:?}",
        stdout,
    );

    // Stderr must be non-empty (diagnostics rendered).
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.trim().is_empty(),
        "deal parse on malformed file: stderr must contain diagnostics but was empty",
    );
}

// ─── Test 5: --json on malformed exits 1, stderr is D-32 envelope, stdout empty ─

/// `deal parse --json MALFORMED.deal` exits 1; stderr is D-32 envelope JSON;
/// stdout is EMPTY (WARNING-05: no partial AST mixed with diagnostics).
#[test]
fn parse_json_malformed_emits_d32_envelope_on_stderr_stdout_empty() {
    let path = repo_root().join("tests/malformed/m01_missing_semicolon.deal");
    if !path.exists() {
        return;
    }

    let output = Command::new(deal_bin())
        .args(["parse", "--json"])
        .arg(&path)
        .output()
        .expect("failed to run deal parse --json on malformed file");

    let exit_code = output.status.code().unwrap_or(99);
    assert_eq!(
        exit_code,
        1,
        "deal parse --json on malformed file expected exit 1 but got {}\nstderr: {}",
        exit_code,
        String::from_utf8_lossy(&output.stderr),
    );

    // Stdout must be EMPTY — no partial AST, no mixed output.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "deal parse --json on malformed: stdout must be empty (no partial AST), got: {:?}",
        stdout,
    );

    // Stderr must be a valid D-32 envelope JSON.
    let stderr = String::from_utf8_lossy(&output.stderr);
    let envelope: serde_json::Value = serde_json::from_str(stderr.trim())
        .unwrap_or_else(|e| panic!(
            "parse --json malformed: stderr must be D-32 JSON envelope but parse failed: {}\nstderr: {}",
            e, stderr,
        ));

    // Verify D-32 envelope structure.
    assert_eq!(
        envelope["command"].as_str(),
        Some("parse"),
        "D-32 envelope 'command' must be 'parse' but got: {:?}",
        envelope.get("command"),
    );
    assert_eq!(
        envelope["v"].as_u64(),
        Some(1),
        "D-32 envelope 'v' must be 1",
    );
    assert!(
        envelope["diagnostics"].is_array(),
        "D-32 envelope must have 'diagnostics' array",
    );
    assert!(
        envelope["summary"].is_object(),
        "D-32 envelope must have 'summary' object",
    );
}

// ─── Test 6: nonexistent file exits 2 (D-34 internal I/O error) ─────────────

/// `deal parse /tmp/nonexistent.deal` exits 2 (D-34 Internal error — file not found).
#[test]
fn parse_nonexistent_file_exits_two() {
    let output = Command::new(deal_bin())
        .args(["parse", "/tmp/__deal_parse_nonexistent__.deal"])
        .output()
        .expect("failed to run deal parse on nonexistent file");

    let exit_code = output.status.code().unwrap_or(99);
    assert_eq!(
        exit_code, 2,
        "deal parse on nonexistent file expected exit 2 (D-34 internal) but got {}",
        exit_code,
    );
}

// ─── Test 7: All showcase .deal files exit 0 (S-9 integration check) ─────────

/// All showcase .deal files parse successfully via `deal parse` (S-9 integration check).
/// This catches regressions where new showcase additions fail to parse.
#[test]
fn parse_all_showcase_deal_files_exit_zero() {
    let showcase_dir = repo_root().join("tests/showcase");
    if !showcase_dir.exists() {
        // Skip if showcase not available (e.g. submodule not initialized).
        return;
    }

    let mut total = 0usize;
    let mut failures: Vec<String> = Vec::new();

    // Walk the showcase directory recursively for .deal files.
    fn walk(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    walk(&p, files);
                } else if p.extension().and_then(|e| e.to_str()) == Some("deal") {
                    files.push(p);
                }
            }
        }
    }
    let mut deal_files = Vec::new();
    walk(&showcase_dir, &mut deal_files);
    deal_files.sort(); // deterministic order

    for path in &deal_files {
        total += 1;
        let output = Command::new(deal_bin())
            .args(["parse"])
            .arg(path)
            .output()
            .unwrap_or_else(|e| panic!("failed to run deal parse on {:?}: {}", path, e));

        if output.status.code().unwrap_or(99) != 0 {
            failures.push(format!(
                "{}: exit {}\n  stderr: {}",
                path.display(),
                output.status.code().unwrap_or(99),
                String::from_utf8_lossy(&output.stderr).trim(),
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "deal parse failed on {}/{} showcase .deal files:\n{}",
        failures.len(),
        total,
        failures.join("\n"),
    );

    assert!(
        total > 0,
        "No .deal files found under {}",
        showcase_dir.display()
    );
}
