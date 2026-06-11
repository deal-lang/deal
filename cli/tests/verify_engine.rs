//! Integration tests for the verify engine (Phase 5 Plan 06, TDD target).
//!
//! These tests exercise:
//!   - Expression evaluator: >= <= == AND + - * / field-path max()
//!   - Three-level verdict: PASS / FAIL / PARTIAL + orthogonal STALE
//!   - Per-REQ report shape (D-87)
//!
//! Oracle cases from traceability.dealx:
//!   REQ_BAT_001 — PASS (design evidence, actualCapacity >= minCapacity)
//!   REQ_BAT_002 — PARTIAL (status="partial" + gap block, max() in compute)
//!   REQ_BAT_003 — AND criteria (actualMaxTemp <= maxTemp AND actualMinTemp >= minTemp)

use deal::verify;
use deal::verify::{eval_expr, resolve_field_path, EvalContext, EvalValue};

// ─── Task 1 Tests: Expression evaluator + field-path resolution ────────────────

/// Helper to build a simple EvalContext from key-value pairs.
fn ctx_from_pairs(pairs: &[(&str, f64)]) -> EvalContext {
    let mut ctx = EvalContext::new();
    for &(k, v) in pairs {
        ctx.set(k, EvalValue::Number(v));
    }
    ctx
}

/// actualRange >= REQ_SYS_001.minRange — evaluates true from resolved values.
#[test]
fn test_gte_comparison_true() {
    let ctx = ctx_from_pairs(&[("actualRange", 520.0), ("REQ_SYS_001.minRange", 500.0)]);
    let result = eval_expr("actualRange >= REQ_SYS_001.minRange", &ctx)
        .expect("eval should succeed");
    assert_eq!(result, EvalValue::Bool(true));
}

/// actualRange >= REQ_SYS_001.minRange — evaluates false when range is too low.
#[test]
fn test_gte_comparison_false() {
    let ctx = ctx_from_pairs(&[("actualRange", 490.0), ("REQ_SYS_001.minRange", 500.0)]);
    let result = eval_expr("actualRange >= REQ_SYS_001.minRange", &ctx)
        .expect("eval should succeed");
    assert_eq!(result, EvalValue::Bool(false));
}

/// AND criterion: actualMaxTemp <= REQ_BAT_003.maxTemp AND actualMinTemp >= REQ_BAT_003.minTemp
/// Both sides true → true (REQ_BAT_003).
#[test]
fn test_and_criteria() {
    let ctx = ctx_from_pairs(&[
        ("actualMaxTemp", 45.0),
        ("REQ_BAT_003.maxTemp", 50.0),
        ("actualMinTemp", -25.0),
        ("REQ_BAT_003.minTemp", -30.0),
    ]);
    let result = eval_expr(
        "actualMaxTemp <= REQ_BAT_003.maxTemp AND actualMinTemp >= REQ_BAT_003.minTemp",
        &ctx,
    )
    .expect("eval should succeed");
    assert_eq!(result, EvalValue::Bool(true));
}

/// AND criterion: second side fails → false.
#[test]
fn test_and_criteria_fails_when_one_side_false() {
    let ctx = ctx_from_pairs(&[
        ("actualMaxTemp", 45.0),
        ("REQ_BAT_003.maxTemp", 50.0),
        ("actualMinTemp", -35.0),    // below minTemp → fails
        ("REQ_BAT_003.minTemp", -30.0),
    ]);
    let result = eval_expr(
        "actualMaxTemp <= REQ_BAT_003.maxTemp AND actualMinTemp >= REQ_BAT_003.minTemp",
        &ctx,
    )
    .expect("eval should succeed");
    assert_eq!(result, EvalValue::Bool(false));
}

/// Compute: margin = actualRange - REQ_SYS_001.minRange; yields correct number.
#[test]
fn test_margin_subtraction() {
    let ctx = ctx_from_pairs(&[("actualRange", 520.0), ("REQ_SYS_001.minRange", 500.0)]);
    let result = eval_expr("actualRange - REQ_SYS_001.minRange", &ctx)
        .expect("eval should succeed");
    assert_eq!(result, EvalValue::Number(20.0));
}

/// Compute: marginPercent = margin / REQ_SYS_001.minRange * 100; yields correct %.
#[test]
fn test_margin_percent() {
    let ctx = ctx_from_pairs(&[("margin", 20.0), ("REQ_SYS_001.minRange", 500.0)]);
    let result = eval_expr("margin / REQ_SYS_001.minRange * 100", &ctx)
        .expect("eval should succeed");
    // 20 / 500 * 100 = 4.0
    match result {
        EvalValue::Number(v) => assert!((v - 4.0).abs() < 1e-9, "expected 4.0, got {}", v),
        other => panic!("expected Number, got {:?}", other),
    }
}

/// max(a, b, c) returns the maximum (REQ_BAT_002 worstCase).
#[test]
fn test_max_call() {
    let ctx = ctx_from_pairs(&[
        ("chargeTime_25C", 25.0),
        ("chargeTime_neg10C", 31.0),
        ("chargeTime_neg30C", 28.0),
    ]);
    let result = eval_expr("max(chargeTime_25C, chargeTime_neg10C, chargeTime_neg30C)", &ctx)
        .expect("eval should succeed");
    assert_eq!(result, EvalValue::Number(31.0));
}

/// An unresolved field-path yields UnmappedField (feeds PARTIAL, not a panic).
#[test]
fn test_unresolved_field_path_yields_unmapped() {
    let ctx = EvalContext::new(); // empty — no values
    let result = eval_expr("chargeTime_neg10C <= REQ_BAT_002.chargeTime", &ctx);
    // Should return Ok(UnmappedField) or Err with unmapped signal, not panic.
    match result {
        Ok(EvalValue::Unmapped) => {} // expected
        Ok(other) => panic!("expected Unmapped, got {:?}", other),
        Err(e) => panic!("expected Unmapped value, got Err: {}", e),
    }
}

/// An unsupported operator (OR) is rejected with a scope-guard error (D-55).
#[test]
fn test_unsupported_operator_rejected() {
    let ctx = ctx_from_pairs(&[("a", 1.0), ("b", 2.0)]);
    let result = eval_expr("a >= 1 OR b <= 2", &ctx);
    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("unsupported") || msg.contains("D-55") || msg.contains("showcase"),
                "error should mention unsupported grammar, got: {}",
                msg
            );
        }
        Ok(_) => panic!("OR should be rejected as unsupported in showcase grammar"),
    }
}

/// Field-path resolution: dot-separated path resolves against context map.
#[test]
fn test_field_path_resolution() {
    let ctx = ctx_from_pairs(&[("EnergyStorage.battery.usableCapacity", 85.0)]);
    let value = resolve_field_path("EnergyStorage.battery.usableCapacity", &ctx);
    assert_eq!(value, Some(EvalValue::Number(85.0)));
}

/// Verdict enum serializes as SCREAMING_SNAKE_CASE.
#[test]
fn test_verdict_serializes_screaming_snake_case() {
    let v = verify::Verdict::Pass;
    let s = serde_json::to_string(&v).expect("serialize failed");
    assert_eq!(s, "\"PASS\"");

    let v = verify::Verdict::Partial;
    let s = serde_json::to_string(&v).expect("serialize failed");
    assert_eq!(s, "\"PARTIAL\"");

    let v = verify::Verdict::Fail;
    let s = serde_json::to_string(&v).expect("serialize failed");
    assert_eq!(s, "\"FAIL\"");
}

// ─── Task 2 Tests: Three-level verdict + STALE + per-REQ report ───────────────

/// REQ_BAT_001: PASS — actualCapacity >= minCapacity (design evidence).
#[test]
fn test_pass_verdict() {
    // Build a minimal "satisfy block" context:
    // actualCapacity = 85 kWh; REQ_BAT_001.minCapacity = 75 kWh → PASS
    let report = verify::evaluate_showcase_case(
        "REQ_BAT_001",
        "actualCapacity >= REQ_BAT_001.minCapacity",
        /*status=*/ None,
        /*has_gap=*/ false,
        &ctx_from_pairs(&[
            ("actualCapacity", 85.0),
            ("REQ_BAT_001.minCapacity", 75.0),
        ]),
        /*stale=*/ false,
    )
    .expect("evaluate_showcase_case should succeed");

    assert_eq!(report.verdict, verify::Verdict::Pass);
    assert!(!report.stale);
}

/// REQ_BAT_002: PARTIAL — status="partial" AND gap block present.
#[test]
fn test_partial_verdict() {
    let report = verify::evaluate_showcase_case(
        "REQ_BAT_002",
        "worstCase <= REQ_BAT_002.chargeTime",
        /*status=*/ Some("partial"),
        /*has_gap=*/ true,
        &ctx_from_pairs(&[
            ("worstCase", 28.0),
            ("REQ_BAT_002.chargeTime", 30.0),
        ]),
        /*stale=*/ false,
    )
    .expect("evaluate_showcase_case should succeed");

    assert_eq!(report.verdict, verify::Verdict::Partial);
    assert!(!report.stale);
}

/// STALE flag: orthogonal to verdict — stale=true on hash mismatch, not PARTIAL.
#[test]
fn test_stale_detection() {
    let report = verify::evaluate_showcase_case(
        "REQ_BAT_001",
        "actualCapacity >= REQ_BAT_001.minCapacity",
        /*status=*/ None,
        /*has_gap=*/ false,
        &ctx_from_pairs(&[
            ("actualCapacity", 85.0),
            ("REQ_BAT_001.minCapacity", 75.0),
        ]),
        /*stale=*/ true, // hash mismatch
    )
    .expect("evaluate_showcase_case should succeed");

    // PASS + STALE: verdict is PASS but stale flag is true
    assert_eq!(report.verdict, verify::Verdict::Pass);
    assert!(report.stale, "stale flag should be set");
}

/// PARTIAL via unmapped field: when a return field has no evidence, verdict is PARTIAL.
#[test]
fn test_partial_via_unmapped_field() {
    // chargeTime_neg10C is not in context (unmapped)
    let report = verify::evaluate_showcase_case(
        "REQ_BAT_002",
        "worstCase <= REQ_BAT_002.chargeTime",
        /*status=*/ None,     // no explicit status=partial
        /*has_gap=*/ false,   // no gap block
        &ctx_from_pairs(&[
            // worstCase is missing — context only has the threshold
            ("REQ_BAT_002.chargeTime", 30.0),
            // worstCase NOT present → unmapped → PARTIAL
        ]),
        /*stale=*/ false,
    )
    .expect("evaluate_showcase_case should succeed");

    assert_eq!(report.verdict, verify::Verdict::Partial);
}

// ─── Staleness wiring tests (D-83/D-84) ───────────────────────────────────────
//
// Regression coverage for the milestone-audit blocker: `run_verify` previously
// hardcoded `stale_overrides = HashMap::new()`, so `check_staleness` was never
// invoked and the STALE verdict was unreachable in production. These tests pin
// the wiring that resolves staleness from the recorded baseline manifest
// (`evidence/baselines/<tag>/manifest.json`) so drift is actually detected.

/// Write a baseline manifest recording `content_hash` for one sim, plus the
/// current evidence `output.json`. Returns the project root tempdir.
fn setup_baseline_and_evidence(
    sim: &str,
    tag: &str,
    recorded_hash: &str,
    current_output_bytes: &[u8],
) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    // evidence/baselines/<tag>/manifest.json (repo root — NOT under .deal, D-81)
    let manifest_dir = root.join("evidence").join("baselines").join(tag);
    std::fs::create_dir_all(&manifest_dir).unwrap();
    let manifest = serde_json::json!({
        "sims": { sim: { "content_hash": recorded_hash } },
        "tag": tag,
        "v": 1,
    });
    std::fs::write(
        manifest_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    // .deal/evidence/<sim>/output.json (current evidence)
    let ev_dir = root.join(".deal").join("evidence").join(sim);
    std::fs::create_dir_all(&ev_dir).unwrap();
    std::fs::write(ev_dir.join("output.json"), current_output_bytes).unwrap();

    tmp
}

/// Drifted evidence (current hash ≠ recorded baseline hash) must be flagged STALE.
#[test]
fn test_build_stale_overrides_flags_drift() {
    let blessed = br#"{"outputs":{"heatGenerated":{"value":3125.0}}}"#;
    let recorded_hash = verify::compute_content_hash(blessed);
    let drifted = br#"{"outputs":{"heatGenerated":{"value":9999.0}}}"#;

    let tmp = setup_baseline_and_evidence("battery_thermal", "v1", &recorded_hash, drifted);
    let overrides = verify::build_stale_overrides(
        tmp.path(),
        &["battery_thermal".to_string()],
        Some("v1"),
    );

    assert_eq!(
        overrides.get("battery_thermal"),
        Some(&true),
        "evidence that drifted from the baseline content_hash must be STALE"
    );
}

/// Evidence matching the recorded baseline hash must NOT be flagged stale.
#[test]
fn test_build_stale_overrides_fresh_not_stale() {
    let blessed = br#"{"outputs":{"heatGenerated":{"value":3125.0}}}"#;
    let recorded_hash = verify::compute_content_hash(blessed);

    // current evidence == blessed bytes → hashes match → fresh
    let tmp = setup_baseline_and_evidence("battery_thermal", "v1", &recorded_hash, blessed);
    let overrides = verify::build_stale_overrides(
        tmp.path(),
        &["battery_thermal".to_string()],
        Some("v1"),
    );

    assert_eq!(
        overrides.get("battery_thermal"),
        Some(&false),
        "evidence matching the baseline content_hash must be fresh"
    );
}

/// `discover_baseline_tag` resolves the lexicographically-greatest tag dir that
/// carries a manifest.json (deterministic selection; baselines are versioned).
#[test]
fn test_discover_baseline_tag_picks_greatest() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    for tag in ["v1", "v2", "v10"] {
        let dir = root.join("evidence").join("baselines").join(tag);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("manifest.json"), b"{\"sims\":{},\"v\":1}").unwrap();
    }
    // A stray dir without a manifest must be ignored.
    std::fs::create_dir_all(root.join("evidence").join("baselines").join("zzz-nomanifest"))
        .unwrap();

    assert_eq!(
        verify::discover_baseline_tag(root).as_deref(),
        Some("v2"),
        "should pick the lexicographically-greatest tag carrying a manifest (v2 > v10 > v1)"
    );
}

/// No baselines present → no tag → staleness resolution is a no-op (not stale).
#[test]
fn test_discover_baseline_tag_none_when_absent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    assert_eq!(verify::discover_baseline_tag(tmp.path()), None);
}
