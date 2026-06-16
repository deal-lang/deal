//! Integration tests for `deal evidence baseline <tag>`.
//!
//! REQ: REQ-phase-5-3-evidence-verification
//!
//! Tests verify:
//!   - deal evidence baseline <tag> writes evidence/baselines/<tag>/manifest.json
//!   - manifest contains per-sim content_hash + valid verdict (PASS|FAIL|PARTIAL) (D-82)
//!   - Two baseline runs over identical evidence produce byte-identical manifest.json (D-18)
//!   - manifest keys are alphabetical (BTreeMap, D-18)
//!
//! VALIDATION.md Per-Task Verification Map:
//!   cargo test -p deal --test evidence_baseline

/// Baseline writes manifest.json with per-sim content_hash and valid verdict.
///
/// Creates a fake evidence cache with two sims, runs baseline, and asserts the
/// manifest contains required D-82 fields.
#[test]
fn test_baseline_writes_manifest_with_content_hash_and_verdict() {
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let root = tmpdir.path();

    // Create .deal/evidence/sim_alpha/ with output.json + metadata.json
    let sim_alpha = root.join(".deal").join("evidence").join("sim_alpha");
    std::fs::create_dir_all(&sim_alpha).unwrap();
    let alpha_output = serde_json::json!({
        "deal_sim_protocol": "v0",
        "exit_code": 0,
        "outputs": {"result": {"value": 42.0, "unit": "W"}},
        "v": 1
    });
    std::fs::write(
        sim_alpha.join("output.json"),
        serde_json::to_vec_pretty(&alpha_output).unwrap(),
    )
    .unwrap();
    let alpha_meta = serde_json::json!({
        "deal_sim_protocol": "v0",
        "reproducibility_tier": "strict",
        "tool": "python",
        "tool_version": "3.11.0",
        "v": 1
    });
    std::fs::write(
        sim_alpha.join("metadata.json"),
        serde_json::to_vec_pretty(&alpha_meta).unwrap(),
    )
    .unwrap();

    // Run baseline
    deal::evidence::run_evidence_baseline_in(root, "v1.0.0").expect("baseline should succeed");

    // Assert manifest.json was written
    let manifest_path = root
        .join("evidence")
        .join("baselines")
        .join("v1.0.0")
        .join("manifest.json");
    assert!(
        manifest_path.exists(),
        "manifest.json must exist at evidence/baselines/v1.0.0/manifest.json"
    );

    // Parse manifest
    let manifest_bytes = std::fs::read(&manifest_path).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).expect("manifest.json must parse as JSON");

    // Top-level fields (D-82)
    assert_eq!(manifest["tag"], "v1.0.0", "manifest must record tag");
    assert_eq!(manifest["v"], 1, "manifest v must be 1");
    assert!(
        manifest["timestamp"].as_str().is_some(),
        "manifest must have timestamp"
    );

    // Per-sim fields
    let sims = manifest["sims"]
        .as_object()
        .expect("manifest must have sims object");
    assert!(
        sims.contains_key("sim_alpha"),
        "manifest must contain sim_alpha"
    );

    let sim_entry = &sims["sim_alpha"];

    // content_hash: non-empty hex string (SHA-256 = 64 chars)
    let hash = sim_entry["content_hash"]
        .as_str()
        .expect("content_hash must be a string");
    assert_eq!(hash.len(), 64, "SHA-256 hex must be 64 chars");
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "content_hash must be hex"
    );

    // verdict: must be a valid enum member
    let verdict = sim_entry["verdict"]
        .as_str()
        .expect("verdict must be a string");
    assert!(
        matches!(verdict, "PASS" | "FAIL" | "PARTIAL"),
        "verdict must be PASS, FAIL, or PARTIAL — got: {}",
        verdict
    );

    // Other D-82 fields must be present
    assert!(
        sim_entry["reproducibility_tier"].is_string(),
        "reproducibility_tier must be string"
    );
    assert!(sim_entry["tool"].is_string(), "tool must be string");
    assert!(
        sim_entry["tool_version"].is_string(),
        "tool_version must be string"
    );
}

/// Frozen output.json snapshot is written alongside manifest.json.
#[test]
fn test_baseline_writes_frozen_output_snapshot() {
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let root = tmpdir.path();

    let sim_dir = root.join(".deal").join("evidence").join("sim_beta");
    std::fs::create_dir_all(&sim_dir).unwrap();
    let output =
        serde_json::json!({"deal_sim_protocol": "v0", "exit_code": 0, "outputs": {}, "v": 1});
    std::fs::write(
        sim_dir.join("output.json"),
        serde_json::to_vec(&output).unwrap(),
    )
    .unwrap();

    deal::evidence::run_evidence_baseline_in(root, "v2.0.0").expect("baseline should succeed");

    // Frozen output.json must exist in the baseline dir
    assert!(
        root.join("evidence")
            .join("baselines")
            .join("v2.0.0")
            .join("sim_beta")
            .join("output.json")
            .exists(),
        "frozen output.json must be written to evidence/baselines/v2.0.0/sim_beta/"
    );
}

/// Two baseline runs over identical evidence produce byte-identical manifest.json (D-18).
///
/// This enforces the BTreeMap alphabetical-key invariant: any non-deterministic
/// key ordering would produce different bytes on the second run.
#[test]
fn test_baseline_byte_identical_on_repeated_run() {
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let root = tmpdir.path();

    // Create evidence with multiple sims (to stress key ordering)
    for name in &["zebra_sim", "alpha_sim", "middle_sim"] {
        let sim_dir = root.join(".deal").join("evidence").join(name);
        std::fs::create_dir_all(&sim_dir).unwrap();
        let output = serde_json::json!({
            "deal_sim_protocol": "v0",
            "exit_code": 0,
            "outputs": {"signal": {"value": 1.0, "unit": "N"}},
            "v": 1
        });
        std::fs::write(
            sim_dir.join("output.json"),
            serde_json::to_vec_pretty(&output).unwrap(),
        )
        .unwrap();
        let meta = serde_json::json!({
            "tool": "python",
            "tool_version": "3.11.0",
            "reproducibility_tier": "tolerant",
        });
        std::fs::write(
            sim_dir.join("metadata.json"),
            serde_json::to_vec_pretty(&meta).unwrap(),
        )
        .unwrap();
    }

    // First baseline run
    deal::evidence::run_evidence_baseline_in(root, "v3.0.0").expect("first baseline");
    let manifest1 = std::fs::read(
        root.join("evidence")
            .join("baselines")
            .join("v3.0.0")
            .join("manifest.json"),
    )
    .unwrap();

    // Overwrite baseline dir to get a fresh second run (remove and recreate)
    std::fs::remove_dir_all(root.join("evidence").join("baselines").join("v3.0.0")).unwrap();

    // Second baseline run
    deal::evidence::run_evidence_baseline_in(root, "v3.0.0").expect("second baseline");
    let manifest2 = std::fs::read(
        root.join("evidence")
            .join("baselines")
            .join("v3.0.0")
            .join("manifest.json"),
    )
    .unwrap();

    assert_eq!(
        manifest1, manifest2,
        "Two baseline runs over identical evidence must produce byte-identical manifest.json (D-18)"
    );
}

/// manifest.json keys are in alphabetical order (BTreeMap, D-18).
#[test]
fn test_manifest_keys_are_alphabetical() {
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let root = tmpdir.path();

    // Create evidence with sims in non-alphabetical order
    for name in &["zebra_sim", "alpha_sim"] {
        let sim_dir = root.join(".deal").join("evidence").join(name);
        std::fs::create_dir_all(&sim_dir).unwrap();
        let output = serde_json::json!({"deal_sim_protocol":"v0","exit_code":0,"outputs":{},"v":1});
        std::fs::write(
            sim_dir.join("output.json"),
            serde_json::to_vec_pretty(&output).unwrap(),
        )
        .unwrap();
    }

    deal::evidence::run_evidence_baseline_in(root, "v4.0.0").expect("baseline");

    let manifest_bytes = std::fs::read(
        root.join("evidence")
            .join("baselines")
            .join("v4.0.0")
            .join("manifest.json"),
    )
    .unwrap();
    let manifest_str = std::str::from_utf8(&manifest_bytes).unwrap();

    // alpha_sim must appear before zebra_sim in JSON text (BTreeMap ordering, D-18)
    let alpha_pos = manifest_str.find("alpha_sim").unwrap_or(usize::MAX);
    let zebra_pos = manifest_str.find("zebra_sim").unwrap_or(usize::MAX);
    assert!(
        alpha_pos < zebra_pos,
        "manifest.json sims keys must be alphabetical (D-18): alpha_sim before zebra_sim"
    );
}

/// Baseline fails with User error when no evidence cache exists.
#[test]
fn test_baseline_errors_when_no_evidence_cache() {
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let root = tmpdir.path();

    // No .deal/evidence/ directory — should return a User error
    let result = deal::evidence::run_evidence_baseline_in(root, "v5.0.0");
    assert!(
        result.is_err(),
        "baseline with no evidence cache must return Err"
    );
    // Should be a user error (not internal)
    if let Err(deal::CliError::User(msg)) = result {
        assert!(
            msg.contains("no .deal/evidence/"),
            "error message should mention missing evidence dir: {}",
            msg
        );
    } else {
        panic!("expected CliError::User");
    }
}

/// Verdict field sourced from metadata.json; defaults to PARTIAL when absent (D-82).
#[test]
fn test_baseline_verdict_defaults_to_partial_when_metadata_absent() {
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let root = tmpdir.path();

    // Create sim with output.json but NO metadata.json
    let sim_dir = root.join(".deal").join("evidence").join("no_meta_sim");
    std::fs::create_dir_all(&sim_dir).unwrap();
    let output = serde_json::json!({"deal_sim_protocol":"v0","exit_code":0,"outputs":{},"v":1});
    std::fs::write(
        sim_dir.join("output.json"),
        serde_json::to_vec_pretty(&output).unwrap(),
    )
    .unwrap();

    deal::evidence::run_evidence_baseline_in(root, "v6.0.0").expect("baseline");

    let manifest_bytes = std::fs::read(
        root.join("evidence")
            .join("baselines")
            .join("v6.0.0")
            .join("manifest.json"),
    )
    .unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();

    let verdict = manifest["sims"]["no_meta_sim"]["verdict"]
        .as_str()
        .expect("verdict must be a string");
    assert_eq!(
        verdict, "PARTIAL",
        "verdict must default to PARTIAL when metadata.json is absent (D-82)"
    );
}

/// T-05-13: the D-81 evidence split is enforced by `.gitignore`.
///
/// Mitigation for the "working .deal artifacts accidentally committed" threat:
/// `.deal/` (the gitignored working cache) MUST be ignored, while the promoted
/// `evidence/baselines/` baseline MUST remain tracked. This assertion fails if a
/// future `.gitignore` edit collapses the split in either direction.
///
/// Uses `git check-ignore --quiet` against the real repo root:
///   exit 0 = path is ignored, exit 1 = path is NOT ignored.
#[test]
fn test_gitignore_enforces_d81_evidence_split() {
    // CARGO_MANIFEST_DIR is <repo>/cli; the repo root is its parent.
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root is the parent of the cli crate dir")
        .to_path_buf();

    let check_ignored = |relpath: &str| -> Option<bool> {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo_root)
            .args(["check-ignore", "--quiet", relpath])
            .status();
        match status {
            // 0 → ignored, 1 → not ignored. Anything else (e.g. 128) is an error.
            Ok(s) if s.code() == Some(0) => Some(true),
            Ok(s) if s.code() == Some(1) => Some(false),
            // git missing or not a git repo (sandboxed build) — skip, not a security regression.
            _ => None,
        }
    };

    let Some(working_ignored) = check_ignored(".deal/evidence/battery_thermal/output.json") else {
        eprintln!("test_gitignore_enforces_d81_evidence_split: git unavailable — skipping");
        return;
    };
    assert!(
        working_ignored,
        "T-05-13: `.deal/evidence/` working cache MUST be gitignored (D-81)"
    );

    let baseline_ignored = check_ignored("evidence/baselines/v2.1.0/manifest.json")
        .expect("git availability already confirmed above");
    assert!(
        !baseline_ignored,
        "T-05-13: `evidence/baselines/` baseline MUST stay tracked, not gitignored (D-81)"
    );
}
