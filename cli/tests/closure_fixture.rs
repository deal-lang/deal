//! ADR-0004 P4 WS-F: end-to-end closure-loading guarantees over a synthetic
//! import-clean project (`tests/fixtures/closure-proj`).
//!
//! Replaces the pre-ADR-0004 premises that `check_subcommand::check_clean_file_
//! exits_zero` and `check_workspace_index::…` encoded (both #[ignore]'d until the
//! examples are import-clean in P6) with assertions over a fixture that IS
//! import-clean today.
//!
//! Fixture shape:
//!   app/main.deal            entry — imports the app.geo barrel, uses Point/Vector
//!   app/geo/index.deal       barrel — `export shapes.{Point, Vector}`
//!   app/geo/shapes.deal      sub-package app.geo.shapes — the concrete defs
//!   app/geo/extra.deal       SAME-package sibling of app.geo (package-complete)
//!   app/orphan/dead.deal     UNREACHABLE + broken (undefined type → E2100)
//!   app/bad/uses_unimported.deal  uses Point without importing it (E2100/E2000)

use std::process::Command;

fn deal_bin() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_deal") {
        path.into()
    } else {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("../target/debug/deal");
        p
    }
}

/// `<crate>/tests/fixtures/closure-proj`.
fn fixture_dir() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/closure-proj");
    p
}

fn check(path: &std::path::Path) -> std::process::Output {
    Command::new(deal_bin())
        .args(["check", "--color=never"])
        .arg(path)
        .output()
        .expect("failed to run deal check")
}

/// RAII guard removing a generated `.deal/` index dir on drop (self-cleaning
/// even if assertions panic).
struct IndexGuard(std::path::PathBuf);
impl Drop for IndexGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The core guarantee: checking the reachable entry exits 0 — the unreachable,
/// deliberately-broken `app.orphan` file's error does NOT surface, and the
/// same-package sibling + transitive barrel target both resolve cleanly.
#[test]
fn entry_closure_is_clean_and_excludes_unreachable() {
    let out = check(&fixture_dir().join("app/main.deal"));
    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        0,
        "checking the app.main entry should exit 0 (unreachable broken file must not surface)\nstderr: {}",
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("NoSuchType") && !stderr.contains("E2100"),
        "the unreachable app.orphan error must not surface; stderr: {stderr}"
    );
}

/// Control for the test above: the orphan file IS genuinely broken — checking
/// it directly surfaces the error and exits 1. (So the exit-0 above means
/// "not analyzed", not "happens to be clean".)
#[test]
fn unreachable_file_checked_directly_still_errors() {
    let out = check(&fixture_dir().join("app/orphan/dead.deal"));
    assert_eq!(
        out.status.code().unwrap_or(99),
        1,
        "checking the broken file directly must exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr),
    );
}

/// Strict import-scoping still flags a genuine un-imported cross-package
/// reference (the checker is not simply permissive).
#[test]
fn unimported_reference_is_rejected() {
    let out = check(&fixture_dir().join("app/bad/uses_unimported.deal"));
    assert_eq!(
        out.status.code().unwrap_or(99),
        1,
        "an un-imported cross-package reference must exit 1\nstderr: {}",
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("E2100") || stderr.contains("E2000"),
        "expected E2100/E2000 for the un-imported reference; stderr: {stderr}"
    );
}

/// Criterion-1 replacement: `deal check <clean-package-dir>` exits 0 AND writes
/// `<dir>/.deal/index.json` with alphabetical top-level keys. The dir holds the
/// barrel, its sub-package, and the same-package sibling — all import-clean.
#[test]
fn clean_package_dir_writes_index_json() {
    let dir = fixture_dir().join("app/geo");
    let _guard = IndexGuard(dir.join(".deal"));

    let out = check(&dir);
    let code = out.status.code().unwrap_or(99);
    assert_eq!(
        code,
        0,
        "checking the clean app.geo package dir should exit 0\nstderr: {}",
        String::from_utf8_lossy(&out.stderr),
    );

    let index_path = dir.join(".deal").join("index.json");
    assert!(
        index_path.exists(),
        "deal check <dir> must write {}",
        index_path.display()
    );

    let bytes = std::fs::read(&index_path).expect("read index.json");
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("index.json is valid JSON");
    // Top-level keys alphabetical (D-18): deal_version, elements, imports_graph, v.
    let obj = v.as_object().expect("index.json is a JSON object");
    let keys: Vec<&String> = obj.keys().collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "index.json top-level keys must be alphabetical");
    assert!(
        obj.contains_key("elements"),
        "index.json must carry an elements map"
    );
}
