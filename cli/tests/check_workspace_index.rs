//! Integration test for SPEC §criterion 1 (Phase 2 closeout):
//! `deal check <workspace-dir>` writes `<workspace>/.deal/index.json` and the
//! resulting JSON has alphabetical top-level keys (D-18) and contains
//! non-trivial content (elements + imports_graph).
//!
//! Why this file exists:
//!   The original 02-02-T1 verification command referenced a phantom Zig test
//!   `sema_corpus.index_json_alphabetical_keys` and a non-existent build option
//!   `-DINDEX_OUT_DIR`. The behavior the planner intended to gate — that
//!   `deal check <dir>` writes a workspace-mode `.deal/index.json` with
//!   alphabetical keys — was only asserted narratively in 02-VERIFICATION.md.
//!   This file is the missing automated gate.
//!
//! What we assert (behavioral):
//!   1. `deal check <fixed-fixture-dir>` exits 0
//!   2. The file `<fixture>/.deal/index.json` exists
//!   3. The file parses as valid JSON
//!   4. The top-level keys are in alphabetical order (D-18)
//!   5. `elements` and `imports_graph` keys are present and non-trivial
//!   6. The keys inside `elements` are in alphabetical order (D-18)
//!
//! Mirrors the in-memory pattern in cli/tests/key_order.rs but exercises the
//! full on-disk artifact path via the built `deal` binary (Pitfall 1 / D-18
//! defense on both the FFI return path and the workspace-merge write path).
//!
//! Hermeticity strategy: this test runs `deal check` directly against the
//! committed `tests/showcase/` fixture tree (a deterministic, hardcoded
//! repo-relative path) and cleans up the produced `.deal/index.json` on exit.
//! No untrusted strings are interpolated into any file path.

use std::process::Command;

/// Path to the built deal binary (mirrors cli_smoke.rs / check_subcommand.rs).
fn deal_bin() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_deal") {
        path.into()
    } else {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("../target/debug/deal");
        p
    }
}

/// Repo root — used to locate `tests/showcase`.
fn repo_root() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // pop "cli"
    p
}

/// RAII guard that removes the `.deal/` index dir on drop so the test is
/// self-cleaning even if assertions panic.
struct IndexDirGuard {
    path: std::path::PathBuf,
}

impl Drop for IndexDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// SPEC §criterion 1: `deal check <workspace-dir>` writes
/// `<workspace>/.deal/index.json` with alphabetical top-level keys and
/// non-trivial content.
///
/// ADR-0004 P4: the index write is gated on `!any_errors`. Under strict
/// import-scoped closure loading the showcase emits diagnostics (it is not yet
/// import-clean — see `check_clean_file_exits_zero`), so no index is written.
/// Re-enable once the examples are import-clean (P6); WS-F adds an equivalent
/// assertion over a clean synthetic project.
#[ignore = "ADR-0004 P6: showcase not import-clean → diagnostics gate off index write; re-enable in P6"]
#[test]
fn check_workspace_writes_index_json_with_alphabetical_keys() {
    // Hardcoded fixture path — no caller-supplied data is interpolated.
    let workspace_dir = repo_root().join("tests").join("showcase");
    assert!(
        workspace_dir.is_dir(),
        "fixture directory missing: {:?}",
        workspace_dir,
    );

    // Pre-flight: any prior `.deal/index.json` from a stale run is removed.
    // RAII guard also removes it on test exit (panic-safe).
    let index_dir = workspace_dir.join(".deal");
    let _ = std::fs::remove_dir_all(&index_dir);
    let _guard = IndexDirGuard {
        path: index_dir.clone(),
    };

    // Run `deal check <fixture-dir>`.
    let output = Command::new(deal_bin())
        .args(["check", "--color=never"])
        .arg(&workspace_dir)
        .output()
        .expect("failed to run deal check on showcase fixture");

    let exit_code = output.status.code().unwrap_or(99);
    assert_eq!(
        exit_code,
        0,
        "deal check on showcase expected exit 0 but got {}\nstderr: {}\nstdout: {}",
        exit_code,
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    // Assert 2: the index file exists at <fixture>/.deal/index.json.
    let index_path = index_dir.join("index.json");
    assert!(
        index_path.exists(),
        "expected .deal/index.json under {:?} but file is missing",
        workspace_dir,
    );

    // Assert 3: it parses as JSON.
    let bytes = std::fs::read(&index_path)
        .unwrap_or_else(|e| panic!("cannot read {:?}: {}", index_path, e));
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("index.json is not valid JSON: {}", e));

    // Assert 4 (D-18): top-level keys are in alphabetical order.
    let obj = value
        .as_object()
        .expect("index.json root must be a JSON object");
    let top_keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
    let mut sorted = top_keys.clone();
    sorted.sort();
    assert_eq!(
        top_keys, sorted,
        "D-18 violation: top-level keys of .deal/index.json must be alphabetical, got: {:?}",
        top_keys,
    );

    // Assert 5: presence of structural keys + non-trivial content.
    assert!(
        obj.contains_key("elements"),
        "index.json must contain 'elements' key, got top-level: {:?}",
        top_keys,
    );
    assert!(
        obj.contains_key("imports_graph"),
        "index.json must contain 'imports_graph' key, got top-level: {:?}",
        top_keys,
    );

    let elements = obj["elements"]
        .as_object()
        .expect("'elements' must be a JSON object");
    assert!(
        !elements.is_empty(),
        "expected elements map to be non-empty (showcase has \u{2265}1 declaration), got 0",
    );

    // Assert 6 (D-18): inner keys of `elements` are alphabetical.
    let elem_keys: Vec<&str> = elements.keys().map(|k| k.as_str()).collect();
    let mut sorted_elem = elem_keys.clone();
    sorted_elem.sort();
    assert_eq!(
        elem_keys, sorted_elem,
        "D-18 violation: 'elements' inner keys must be alphabetical, got: {:?}",
        elem_keys,
    );
}
