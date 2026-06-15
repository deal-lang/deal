//! Phase 5 end-to-end integration test.
//!
//! REQ: REQ-phase-5-gate, REQ-phase-5-simulation, REQ-phase-5-2-orchestration,
//!      REQ-phase-5-3-evidence-verification
//!
//! Asserts the full Phase 5 showcase verification chain:
//!   1. deal simulate battery_thermal (Python oracle) — produces schema-valid output.json
//!   2. deal evidence capture — snapshots working runs
//!   3. deal evidence baseline v2.1.0 — frozen manifest.json with SHA-256 hashes
//!   4. deal check --simulations — structural MATLAB binding validation (exit 0)
//!   5. deal check --verify — per-REQ report contains REQ_BAT_001 + REQ_BAT_002 PARTIAL
//!
//! Hard-gate model (D-72):
//!   - battery_thermal MUST produce output.json (Python oracle runs for real)
//!   - MATLAB sims MUST graceful-skip (skip.json, no hard error)
//!   - deal check --simulations MUST exit 0
//!   - deal check --verify MUST contain REQ_BAT_001 and REQ_BAT_002
//!
//! Deviation from plan: The test uses `CARGO_BIN_EXE_deal` which resolves to
//! the debug build under `cargo test`. For the smoke gate, the release binary is
//! used by scripts/phase-5-smoke.sh. Both paths exercise the same code.

use std::path::PathBuf;
use std::process::Command;

/// Resolve the path to the deal binary.
fn deal_bin() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_deal") {
        PathBuf::from(path)
    } else {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("../target/debug/deal");
        p
    }
}

/// Resolve the path to the showcase directory via the spec submodule.
fn showcase_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../spec/examples/showcase");
    p
}

/// Copy showcase to a temp dir so we don't pollute the spec/ submodule.
///
/// Uses a hardcoded static list of showcase-relative paths (compile-time literals)
/// to avoid any dynamic path construction from filesystem data (CWE-22 / T-05-06).
fn copy_showcase_to_temp() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let base = showcase_dir();
    let dst = tmp.path();

    // Static file list — every path component is a compile-time string literal.
    // Directories are created first; files are copied using only literal sub-paths.
    //
    // CWE-22 / T-05-06 mitigation: there is NO dynamic path construction here. No
    // read_dir, no filesystem-derived path components. The complete set of showcase
    // files the Phase 5 chain needs is frozen as a hardcoded list of compile-time
    // string literals (the project's "frozen Record<fixture, absPath>" pattern — see
    // STATE.md "CWE-22 taint-free path construction" decision). Each `dst.join(literal)`
    // and `base.join(literal)` is taint-free because both operands are constants.
    static DIRS: &[&str] = &[
        "model",
        "model/variants",
        "packages",
        "packages/.deal",
        "packages/vehicle",
        "packages/interfaces",
        "packages/requirements",
        "packages/use-cases",
        "simulations",
        "simulations/thermal",
        "simulations/dynamics",
        "test",
        "test/data",
    ];
    static FILES: &[&str] = &[
        "deal.toml",
        // model + traceability (deal check --verify target)
        "model/traceability.dealx",
        "model/vehicle.dealx",
        "model/variants/performance.dealx",
        "model/variants/sedan.dealx",
        // package index (resolved by deal check / package loader)
        "packages/.deal/index.json",
        // vehicle package
        "packages/vehicle/battery.deal",
        "packages/vehicle/behaviors.deal",
        "packages/vehicle/components.deal",
        "packages/vehicle/index.deal",
        "packages/vehicle/motor.deal",
        // interfaces package
        "packages/interfaces/connections.deal",
        "packages/interfaces/electrical.deal",
        "packages/interfaces/index.deal",
        "packages/interfaces/thermal.deal",
        // requirements package (REQ_BAT_001 / REQ_BAT_002 definitions)
        "packages/requirements/index.deal",
        "packages/requirements/needs.deal",
        "packages/requirements/system.deal",
        // use-cases package
        "packages/use-cases/charging.deal",
        "packages/use-cases/driving.deal",
        "packages/use-cases/index.deal",
        // simulation manifest + adapters
        "simulations/deal.sims.toml",
        "simulations/thermal/battery_thermal.py",
        "simulations/thermal/motor_thermal.py",
        "simulations/dynamics/range_model.py",
        "simulations/dynamics/motor_efficiency.m",
        "simulations/dynamics/vehicle_dynamics.m",
        // test data referenced by traceability evidence links
        "test/data/charge-cycle-data.csv",
        "test/data/dyno-results-2026-q2.csv",
        "test/data/thermal-cycle-report.pdf",
    ];

    for rel_dir in DIRS {
        let dest_dir = dst.join(rel_dir);
        std::fs::create_dir_all(&dest_dir)
            .unwrap_or_else(|e| panic!("create dir {}: {}", rel_dir, e));
    }
    for rel_file in FILES {
        let src_file = base.join(rel_file);
        if src_file.exists() {
            let dest_file = dst.join(rel_file);
            std::fs::copy(&src_file, &dest_file)
                .unwrap_or_else(|e| panic!("copy {}: {}", rel_file, e));
        }
    }
    tmp
}

/// Pre-seed input.json for battery_thermal with physics-valid values.
///
/// IR model_path resolution returns null values when deal-std is not installed.
/// Pre-seeding is the standard approach (same as simulate_integration.rs).
/// Values: packResistance=0.05 ohm, totalCurrent=250 A, coolantFlowRate=30 L/min
/// Expected: Q = I²R = 3125 W heat generated, coolantOutTemp ≈ 29°C.
fn seed_battery_thermal_inputs(showcase_root: &std::path::Path) {
    let ev_dir = showcase_root
        .join(".deal")
        .join("evidence")
        .join("battery_thermal");
    std::fs::create_dir_all(&ev_dir).expect("create battery_thermal evidence dir");

    let input_json = serde_json::json!({
        "deal_sim_protocol": "v0",
        "inputs": {
            "coolantFlowRate": {"unit": "L/min", "value": 30.0},
            "packResistance": {"unit": "ohm", "value": 0.05},
            "totalCurrent": {"unit": "A", "value": 250.0}
        },
        "v": 1
    });
    let input_path = ev_dir.join("input.json");
    std::fs::write(
        &input_path,
        serde_json::to_vec_pretty(&input_json).unwrap(),
    )
    .expect("write battery_thermal input.json");
}

/// Run a deal command from a working directory; return (success, stdout, stderr).
fn run_deal(args: &[&str], cwd: &std::path::Path) -> (bool, String, String) {
    let output = Command::new(deal_bin())
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|e| panic!("failed to run deal {:?}: {}", args, e));

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stdout, stderr)
}

// ─── Step 1: deal simulate battery_thermal (Python oracle) ───────────────────

/// Simulate battery_thermal produces schema-valid output.json.
#[test]
fn test_phase5_simulate_battery_thermal_produces_output_json() {
    let tmp = copy_showcase_to_temp();
    let root = tmp.path();
    seed_battery_thermal_inputs(root);

    let (ok, _stdout, stderr) = run_deal(&["simulate", "battery_thermal"], root);

    // Python may not be available on all CI environments — graceful skip
    if !ok && stderr.contains("python") && stderr.contains("not found") {
        eprintln!("test_phase5_simulate_battery_thermal: Python not available — skipping");
        return;
    }

    assert!(
        ok,
        "deal simulate battery_thermal should succeed\nstderr: {}",
        stderr
    );

    let output_path = root
        .join(".deal")
        .join("evidence")
        .join("battery_thermal")
        .join("output.json");

    assert!(
        output_path.exists(),
        "battery_thermal output.json not found at {:?}",
        output_path
    );

    // Validate output.json is valid JSON with simulation outputs
    let output_bytes = std::fs::read(&output_path).expect("read output.json");
    let output_val: serde_json::Value =
        serde_json::from_slice(&output_bytes).expect("parse output.json");

    // output.json should have either a flat outputs object or spec/sims/v0 envelope
    let has_outputs = output_val.get("outputs").is_some()
        || output_val.get("heatGenerated").is_some();
    assert!(
        has_outputs,
        "output.json should contain 'outputs' or top-level sim keys\noutput: {:?}",
        output_val
    );
}

// ─── Step 2: deal simulate --all (MATLAB graceful-skip) ──────────────────────

/// MATLAB sims graceful-skip (D-72): skip.json written, no hard fail.
///
/// IGNORED (Phase 1a): the showcase `.m` sims are empty placeholders. With MATLAB
/// absent they graceful-skip (skip.json) and this passes; with MATLAB INSTALLED they
/// run but emit no output.json, so neither artifact exists. The graceful-skip core is
/// covered by `simulate::tests::graceful_skip_writes_skip_json`. Re-enable when real
/// MATLAB sims are authored (MVP real-sims step).
#[ignore = "showcase .m sims are empty placeholders; re-enable with real MATLAB sims (MVP)"]
#[test]
fn test_phase5_simulate_all_matlab_graceful_skip() {
    let tmp = copy_showcase_to_temp();
    let root = tmp.path();
    seed_battery_thermal_inputs(root);

    let (ok, _stdout, stderr) = run_deal(&["simulate", "--all"], root);

    // Python may not be available — if battery_thermal can't run, skip gracefully
    if !ok && stderr.contains("python") && stderr.contains("not found") {
        eprintln!("test_phase5_simulate_all_matlab_graceful_skip: Python not available — skipping");
        return;
    }

    assert!(
        ok,
        "deal simulate --all should succeed with MATLAB graceful-skip\nstderr: {}",
        stderr
    );

    // Assert MATLAB sims wrote skip.json (not output.json) — graceful-skip D-72
    for matlab_sim in &["motor_efficiency", "vehicle_dynamics"] {
        let sim_ev_dir = root.join(".deal").join("evidence").join(matlab_sim);
        let skip_path = sim_ev_dir.join("skip.json");
        let output_path = sim_ev_dir.join("output.json");

        // Either skip.json (MATLAB absent) OR output.json (MATLAB available)
        let gracefully_handled = skip_path.exists() || output_path.exists();
        assert!(
            gracefully_handled,
            "MATLAB sim '{}' should have skip.json or output.json — neither found\nstderr: {}",
            matlab_sim, stderr
        );

        if skip_path.exists() {
            // Verify skip.json is valid JSON with skipped=true
            let skip_bytes = std::fs::read(&skip_path).expect("read skip.json");
            let skip_val: serde_json::Value =
                serde_json::from_slice(&skip_bytes).expect("parse skip.json");
            assert_eq!(
                skip_val.get("skipped").and_then(|v| v.as_bool()),
                Some(true),
                "skip.json should have skipped=true for {}",
                matlab_sim
            );
        }
    }
}

// ─── Step 3: deal evidence capture ───────────────────────────────────────────

/// deal evidence capture succeeds after simulations ran.
#[test]
fn test_phase5_evidence_capture_succeeds() {
    let tmp = copy_showcase_to_temp();
    let root = tmp.path();
    seed_battery_thermal_inputs(root);

    // Run battery_thermal first to produce evidence
    let (sim_ok, _stdout, sim_stderr) = run_deal(&["simulate", "battery_thermal"], root);
    if !sim_ok {
        if sim_stderr.contains("python") && sim_stderr.contains("not found") {
            eprintln!("test_phase5_evidence_capture_succeeds: Python not available — skipping");
            return;
        }
        panic!("simulate failed: {}", sim_stderr);
    }

    let (ok, _stdout, stderr) = run_deal(&["evidence", "capture"], root);
    assert!(
        ok,
        "deal evidence capture should exit 0\nstderr: {}",
        stderr
    );
}

// ─── Step 4: deal evidence baseline v2.1.0 ───────────────────────────────────

/// deal evidence baseline v2.1.0 writes manifest.json with SHA-256 hashes.
#[test]
fn test_phase5_evidence_baseline_writes_manifest() {
    let tmp = copy_showcase_to_temp();
    let root = tmp.path();
    seed_battery_thermal_inputs(root);

    // Run battery_thermal to produce evidence cache
    let (sim_ok, _stdout, sim_stderr) = run_deal(&["simulate", "battery_thermal"], root);
    if !sim_ok {
        if sim_stderr.contains("python") && sim_stderr.contains("not found") {
            eprintln!("test_phase5_evidence_baseline_writes_manifest: Python not available — skipping");
            return;
        }
        panic!("simulate failed: {}", sim_stderr);
    }

    // Create baseline
    let (ok, _stdout, stderr) =
        run_deal(&["evidence", "baseline", "v2.1.0"], root);
    assert!(
        ok,
        "deal evidence baseline v2.1.0 should exit 0\nstderr: {}",
        stderr
    );

    // Assert manifest.json was written
    let manifest_path = root
        .join("evidence")
        .join("baselines")
        .join("v2.1.0")
        .join("manifest.json");
    assert!(
        manifest_path.exists(),
        "evidence/baselines/v2.1.0/manifest.json not found after baseline command"
    );

    // Parse manifest and check structure (D-82)
    let manifest_bytes = std::fs::read(&manifest_path).expect("read manifest.json");
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).expect("parse manifest.json");

    assert_eq!(
        manifest.get("tag").and_then(|v| v.as_str()),
        Some("v2.1.0"),
        "manifest should have tag='v2.1.0'"
    );
    assert!(
        manifest.get("sims").is_some(),
        "manifest should have 'sims' key"
    );
    assert_eq!(
        manifest.get("v").and_then(|v| v.as_u64()),
        Some(1),
        "manifest should have v=1"
    );
}

// ─── Step 5: deal check --simulations ────────────────────────────────────────

/// deal check --simulations validates MATLAB bindings structurally (no execution).
#[test]
fn test_phase5_check_simulations_exits_zero() {
    let tmp = copy_showcase_to_temp();
    let root = tmp.path();

    let (ok, _stdout, stderr) = run_deal(&["check", "--simulations"], root);
    assert!(
        ok,
        "deal check --simulations should exit 0 (structural validation)\nstderr: {}",
        stderr
    );
}

// ─── Step 6: deal check --verify (per-REQ report) ────────────────────────────

/// deal check --verify produces a per-REQ report with REQ_BAT_001 + REQ_BAT_002.
///
/// D-87: verify report keyed by REQ_* id. SC#5 PM-query goal: a PM can ask
/// "is REQ_BAT_001 verified?" and get a traceable answer.
#[test]
fn test_phase5_check_verify_produces_req_report() {
    let tmp = copy_showcase_to_temp();
    let root = tmp.path();

    // Run verify — no sim runs needed: REQ_BAT_001 is design-evidence-backed
    // (maps EnergyStorage.battery.usableCapacity -> actualCapacity), so its
    // criterion resolves to a REAL verdict from model values (05-08).
    let (ok, stdout, stderr) =
        run_deal(&["check", "--verify", "model/traceability.dealx"], root);

    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.contains("REQ_BAT_001"),
        "verify report should contain REQ_BAT_001\ncombined: {}",
        combined
    );
    assert!(
        combined.contains("REQ_BAT_002"),
        "verify report should contain REQ_BAT_002\ncombined: {}",
        combined
    );

    // 05-08 GAP CLOSURE: REQ_BAT_001 MUST be PASS, not PARTIAL.
    // actualCapacity = usableCapacity = kWh(85); threshold minCapacity = kWh(85);
    // 85 >= 85 → PASS. This is the real PM-query verdict the synthetic unit
    // fixtures masked (the 05-07 checkpoint reported all-PARTIAL).
    // `verify` prints a verdict TABLE ("PASS  REQ_BAT_001  …"), not "REQ: PASS".
    let req_bat_001_pass = combined
        .lines()
        .any(|l| l.contains("REQ_BAT_001") && (l.contains("PASS") || l.contains("Pass")));
    assert!(
        req_bat_001_pass,
        "REQ_BAT_001 MUST be PASS (85 kWh >= 85 kWh), not PARTIAL\ncombined: {}",
        combined
    );

    // REQ_BAT_002 must be PARTIAL (status="partial" + gap{} block in traceability.dealx)
    let req_bat_002_partial = combined
        .lines()
        .any(|l| l.contains("REQ_BAT_002") && (l.contains("PARTIAL") || l.contains("Partial")));
    assert!(
        req_bat_002_partial,
        "REQ_BAT_002 should appear as PARTIAL in verify report\ncombined: {}",
        combined
    );

    // Summary must no longer be "0 PASS" — at least REQ_BAT_001 + REQ_MOT_001 pass.
    assert!(
        !combined.contains("0 PASS"),
        "verify summary must no longer be 0 PASS (gap closure)\ncombined: {}",
        combined
    );

    // The report exit code with no evidence: either 0 (lenient) or 1 (stale detected)
    // Both are acceptable — what matters is the report content (D-87).
    let _ = ok;
}

// ─── Full chain smoke test ────────────────────────────────────────────────────

/// Full Phase 5 chain: simulate → capture → baseline → check --simulations → check --verify.
///
/// This is the primary E2E test for REQ-phase-5-gate. It exercises the complete
/// showcase verification chain in sequence, confirming all components are wired.
#[test]
fn test_phase5_full_chain() {
    let tmp = copy_showcase_to_temp();
    let root = tmp.path();

    // Step 1: simulate battery_thermal (Python oracle)
    seed_battery_thermal_inputs(root);
    let (sim_ok, _stdout, sim_stderr) = run_deal(&["simulate", "battery_thermal"], root);
    if !sim_ok {
        if sim_stderr.contains("python") && sim_stderr.contains("not found") {
            eprintln!("test_phase5_full_chain: Python not available — skipping full chain");
            return;
        }
        panic!("step 1 simulate battery_thermal failed: {}", sim_stderr);
    }

    // Step 2: evidence capture
    let (capture_ok, _stdout, capture_stderr) = run_deal(&["evidence", "capture"], root);
    assert!(capture_ok, "step 2 evidence capture failed: {}", capture_stderr);

    // Step 3: evidence baseline v2.1.0
    let (baseline_ok, _stdout, baseline_stderr) =
        run_deal(&["evidence", "baseline", "v2.1.0"], root);
    assert!(
        baseline_ok,
        "step 3 evidence baseline v2.1.0 failed: {}",
        baseline_stderr
    );
    assert!(
        root.join("evidence")
            .join("baselines")
            .join("v2.1.0")
            .join("manifest.json")
            .exists(),
        "step 3 baseline manifest.json not found"
    );

    // Step 4: check --simulations (structural MATLAB binding validation)
    let (sims_ok, _stdout, sims_stderr) = run_deal(&["check", "--simulations"], root);
    assert!(
        sims_ok,
        "step 4 check --simulations failed: {}",
        sims_stderr
    );

    // Step 5: check --verify (per-REQ report)
    let (_verify_ok, verify_stdout, verify_stderr) =
        run_deal(&["check", "--verify", "model/traceability.dealx"], root);
    let combined = format!("{}{}", verify_stdout, verify_stderr);

    assert!(
        combined.contains("REQ_BAT_001"),
        "step 5 verify report missing REQ_BAT_001\ncombined: {}",
        combined
    );
    assert!(
        combined.contains("REQ_BAT_002"),
        "step 5 verify report missing REQ_BAT_002\ncombined: {}",
        combined
    );

    // 05-08 GAP CLOSURE: after the full chain the report must carry REAL verdicts.
    //   - REQ_BAT_001 = PASS (design-backed, 85 kWh >= 85 kWh)
    //   - REQ_BAT_002 = PARTIAL (status=partial + gap{})
    //   - the summary is no longer "0 PASS"
    assert!(
        combined
            .lines()
            .any(|l| l.contains("REQ_BAT_001") && (l.contains("PASS") || l.contains("Pass"))),
        "step 5 REQ_BAT_001 MUST be PASS (real verdict)\ncombined: {}",
        combined
    );
    assert!(
        combined
            .lines()
            .any(|l| l.contains("REQ_BAT_002") && (l.contains("PARTIAL") || l.contains("Partial"))),
        "step 5 REQ_BAT_002 MUST be PARTIAL (gap)\ncombined: {}",
        combined
    );
    assert!(
        !combined.contains("0 PASS"),
        "step 5 verify summary must no longer be 0 PASS\ncombined: {}",
        combined
    );
}
