---
phase: 05-simulation-integration
plan: "08"
subsystem: verify-engine + simulate-orchestrator
gap_closure: true
tags:
  - rust
  - gap-closure
  - model-ir-resolution
  - ast-value-index
  - verify-verdict
  - simulate-inputs
  - spec-submodule
  - d85
  - d86
  - d87
dependency_graph:
  requires:
    - 05-06  # verify engine (evaluator, three-level verdict, per-REQ report)
    - 05-07  # phase-5 e2e gate + smoke (the checkpoint that surfaced the gap)
  provides:
    - model-value-index            # cli/src/model_values.rs — AST path→value resolver
    - verify-model-backed-evidence # maps {} + REQ.attr resolution → real PASS/FAIL
    - simulate-input-resolution    # registry model_path → AST value (real sim runs)
    - showcase-sim-input-values    # spec submodule literal values for sim inputs
    - phase5-verdict-gate          # e2e + smoke gate REQ_BAT_001=PASS, not just presence
  affects:
    - phase-06-application
    - gate-maintenance
tech_stack:
  added: []
  patterns:
    - "AST value index: IR payload carries no literals (only name/type_ref); the AST default_value / component_instance attrs do — build the value map from deal_ast_json, not deal_ir_json"
    - "Path resolution by exact → unique-suffix → progressive-tail match (instance-hierarchy model_path resolves to part-def attribute key)"
    - "Unit-call canonicalization to bare magnitude (kWh(85)→85), applied symmetrically on both comparison operands via one shared resolver (D-08-A)"
key_files:
  created:
    - cli/src/model_values.rs       # 408 lines — shared ModelValueIndex (6 unit tests)
  modified:
    - cli/src/verify.rs             # parse evidence maps {}; resolve design/analysis srcs + REQ.attr refs
    - cli/src/simulate.rs           # resolve_model_path/build_input_json fall back to ModelValueIndex
    - cli/src/lib.rs                # pub mod model_values (lib target)
    - cli/src/main.rs               # pub mod model_values (bin target)
    - cli/tests/phase5_e2e.rs       # assert REQ_BAT_001=PASS, REQ_BAT_002=PARTIAL, not 0 PASS
    - scripts/phase-5-smoke.sh      # hard-gate verdict correctness (removed || true swallow)
    - lsp/tests/golden/formatted/battery.deal.expected             # regen (spec value additions)
    - lsp/tests/golden/semantic_tokens/BatteryPack.deal.expected.json  # regen
    - spec/examples/showcase/packages/vehicle/battery.deal         # (submodule) packResistance/totalCurrent values
    - spec/examples/showcase/packages/vehicle/motor.deal           # (submodule) windingResistance/magnetTemp values
    - spec/examples/showcase/packages/vehicle/components.deal      # (submodule) CoolantPump.flowRate value
decisions:
  - "D-08-A: unit-call literals canonicalize to their bare numeric magnitude (kWh(85)→85.0), NOT SI base units. Both operands of a showcase comparison are authored in the same unit and flow through the same resolver, so comparing magnitudes (85>=85) is correct and symmetric."
  - "Value index is built from the AST (deal_ast_json), not the IR (deal_ir_json): the IR attribute_usage payload has only name + type_ref + span — no literal. The literal lives in AST default_value (defs) and component_instance attrs (model instances)."
  - "resolve() matches exact → unique-suffix → progressive-tail. The registry model_path EnergyStorage.battery.packResistance (instance-hierarchy) is LONGER than the indexed part-def key BatteryPack.packResistance; tail-stripping finds it."
  - "Sim-input values added as part-def defaults in the spec submodule (battery.deal/motor.deal/components.deal), not instance overrides — matches existing showcase authoring pattern and resolves via the part-def key."
  - "Same resolver used in BOTH verify.rs and simulate.rs (shared crate::model_values) so model_path↔value resolution is identical on both engines."
metrics:
  duration: "~2 hours"
  completed: "2026-06-08T23:09:00Z"
  tasks_completed: 3
  files_created: 1
  files_modified: 11
---

# Phase 5 Plan 08: Gap Closure — Model-IR Evidence + Simulation Input Resolution Summary

**One-liner:** Closed the 05-07 checkpoint gap — a shared AST-backed `ModelValueIndex` now resolves model paths and `REQ_*.attr` refs to numeric values, so the real showcase `deal check --verify` yields REQ_BAT_001 = **PASS** (85 kWh ≥ 85 kWh) instead of all-PARTIAL, and `deal simulate --all` runs battery_thermal for real (Q = I²R = 3125 W) instead of failing with "got NoneType".

## The literal acceptance line

```
Summary: 2 PASS, 0 FAIL, 6 PARTIAL, 0 STALE / 8 total
```

(REQ_BAT_001: Pass, REQ_BAT_002: Partial, REQ_MOT_001: Pass — the prior checkpoint reported `0 PASS, 0 FAIL, 8 PARTIAL`.)

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Verify engine: resolve model-backed evidence to IR values | `1e2dca7` | cli/src/model_values.rs, cli/src/verify.rs, cli/src/lib.rs, cli/src/main.rs |
| 2 | Simulate: resolve sim inputs + add showcase input values (spec submodule) | `4e96bca` (parent) + `dd3bb78` (spec submodule) | cli/src/simulate.rs, cli/src/model_values.rs, spec/* (3 files), STATE.md, todo recall |
| 3 | Real e2e acceptance + tighten the gate (verdict correctness) | `a0c5a5f` | cli/tests/phase5_e2e.rs, scripts/phase-5-smoke.sh, lsp goldens (2) |

## Root Cause (confirmed by dumping the real IR + AST)

Before writing any resolution code, the merged IR `elements` map for the showcase was dumped. Two facts drove the whole design:

1. **The IR carries NO literal values.** An `attribute_usage` IR element's `payload` is `{ name, type_ref }` plus a source `span` — the value `kWh(85)` is not in the IR. It lives only in the **AST**: `attribute_usage.default_value` for defs, and `component_instance.attrs[].fields[].value` for model instances (e.g. vehicle.dealx `[<BatteryPack as="battery" usableCapacity={kWh(85)} ... />]`).
2. **The IR element key form differs from the model_path form.** IR key: `vehicle.battery.BatteryPack.usableCapacity` (package + part-def qualified). Registry/maps model_path: `EnergyStorage.battery.usableCapacity` (system-instance hierarchy). Requirement ref: `REQ_BAT_001.minCapacity` vs IR key `requirements.system.REQ_BAT_001.minCapacity`.

So the verify engine's old `binding:`-only path and simulate's direct IR-key lookup both produced Unmapped/null. The fix is one shared AST value index used by both engines.

## What Was Built

### cli/src/model_values.rs — shared `ModelValueIndex` (new, 408 lines)

- `ModelValueIndex::build(project_root)` — walks every `.deal`/`.dealx` AST (deal_parse → deal_ast_json, clone-before-free) and indexes attribute values under several path forms: `Owner.attr` (req-def/part-def), `Subsystem.alias.attr` + `alias.attr` + `PartDef.attr` (instance overrides), and bare `attr`.
- `resolve(query)` — exact match → unique-suffix match (query is a dot-suffix of one key) → progressive-tail match (a shorter indexed key is a dot-suffix of a longer query). Ambiguity (>1 distinct value) → `None`.
- `value_to_f64` — `int_literal`/`float_literal`/`real_literal` → parsed magnitude; unit `call` (`kWh(85)`) → its single numeric arg (unit discarded, D-08-A).
- 6 unit tests (REQ attr, instance override, PASS-equality, long-query part-def tail match).

### cli/src/verify.rs — model-backed evidence resolution (Task 1)

- New `parse_maps_block()` parses `evidence design/analysis/test/simulation { maps { <src> -> <field> } }` from annotation body text; `EvidenceMap { src, field, kind }` captured per satisfy block.
- `evaluate()` builds the EvalContext from: design/analysis srcs → `model_index.resolve()`; simulation srcs → sim `output.json` value; `REQ_*.attr` refs in criteria/compute → `resolve_req_attr_refs()` against requirement defs.
- Criteria like `actualCapacity >= REQ_BAT_001.minCapacity` now evaluate to a real `Bool` → PASS/FAIL. Genuinely missing data still → Unmapped → PARTIAL.

### cli/src/simulate.rs — sim-input resolution (Task 2)

- `resolve_model_path` / `build_input_json` take the shared `ModelValueIndex` and fall back to it after the legacy IR-elements lookup. The T-05-06 path-alphabet guard is unchanged (not weakened).
- `run_simulate_in` builds the index once and threads it through all `build_input_json` call sites.

### spec submodule (Task 2, commit `dd3bb78`)

Added literal values to value-less sim-input attributes:
- `battery.deal`: `packResistance = ohm(0.05)`, `totalCurrent = A(250)`
- `motor.deal`: `windingResistance = ohm(0.012)`, `magnetTemp = degC(80)`
- `components.deal` (CoolantPump): `flowRate = LPM(30)`

Parent gitlink bumped `1fb43ca → dd3bb78`. Phase-6 recall updated in both the todo (`2026-06-08-implement-calc-constraint-grammar-in-zig.md` §Spec submodule recall) and STATE.md Deferred Items.

### Gate tightening (Task 3)

- `phase5_e2e.rs`: both verify tests now assert REQ_BAT_001 = PASS, REQ_BAT_002 = PARTIAL, and `!contains("0 PASS")` — not mere presence.
- `phase-5-smoke.sh`: the verify step hard-fails on wrong verdicts (no more `|| true` swallow of correctness); exit-code leniency retained only for the D-84 stale path.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `value_to_f64` did not handle `real_literal` AST nodes**
- **Found during:** Task 2 (battery_thermal still resolved to null after wiring).
- **Issue:** The float value `ohm(0.05)` is encoded as a `real_literal` AST node (text "0.05"), but the resolver only matched `int_literal`/`float_literal`. All decimal sim inputs stayed null → "expected Real, got NoneType".
- **Fix:** Added `real_literal` to the accepted numeric-literal kinds in `value_to_f64`.
- **Verification:** battery_thermal runs for real (packResistance=0.05).
- **Commit:** `4e96bca`

**2. [Rule 3 - Blocking] `resolve()` only matched short-query-suffix, not long-query-tail**
- **Found during:** Task 2 (`EnergyStorage.battery.packResistance` → None while `BatteryPack.packResistance` → 0.05).
- **Issue:** The registry model_path (instance-hierarchy, 3 segments) is LONGER than the indexed part-def key (2 segments). The original suffix logic only handled the query being a suffix of a key, not a key being a suffix of the query.
- **Fix:** Added progressive-tail matching (strip leading query segments, longest tail first).
- **Verification:** All three battery_thermal inputs + motor inputs resolve; `model_values` unit test `resolves_part_def_attribute_via_long_query` added.
- **Commit:** `4e96bca`

**3. [Rule 3 - Blocking] LSP golden fixtures diverged after spec input-value additions**
- **Found during:** Task 3 (`cargo test --workspace`).
- **Issue:** `lsp/tests/showcase.rs` formats and tokenizes `battery.deal` against committed goldens. The Task 2 value additions changed two lines, breaking `format_round_trip` and `semantic_tokens_match_golden`.
- **Fix:** Regenerated both goldens via their committed generators (`deal fmt --stdout` and `deal-lsp-gen-golden`). Diff is exactly the two value-bearing lines — no semantic drift.
- **Verification:** `cargo test -p deal-lsp --test showcase` → 8/8 pass.
- **Commit:** `a0c5a5f`

**Total deviations:** 3 auto-fixed (all Rule 3 - blocking). None expanded scope; all were necessary to make the real chain work and keep the workspace green. No package-manager installs.

## Constraints Honored

- **IR key dump FIRST:** dumped the real merged IR/AST before writing resolution code; the design follows the REAL key naming (AST default_value / instance attrs), not an assumption.
- **Symmetric canonicalization:** `kWh(85) >= kWh(85)` compares true because both operands flow through the one resolver and reduce to magnitude 85 (D-08-A, documented in module).
- **Same matching strategy in verify.rs and simulate.rs** (shared `crate::model_values`).
- **T-05-06 path guard not weakened.**
- **Did NOT touch** src/sema.zig or the E2503 `sema.dimensional.regression_pins` pin (separate accepted Phase-4 carryover).
- **Kept all 15 verify_engine unit tests green** (no EvalContext API break — model resolution is additive; tests use the unchanged `evaluate_showcase_case`).

## Verification Results

- Real chain (no pre-seed, temp showcase copy): `deal simulate --all` exit 0, **battery_thermal completed for real** (heatGenerated 3125 W, coolantOutTemp 21.68 °C); MATLAB sims graceful-skip; `deal check --verify` → REQ_BAT_001 = **Pass**, REQ_BAT_002 = Partial, `Summary: 2 PASS, 0 FAIL, 6 PARTIAL, 0 STALE / 8 total`.
- `cargo test --manifest-path cli/Cargo.toml --workspace` — **green, exit 0** (31 test-result blocks ok, 0 failed).
- `bash scripts/phase-5-smoke.sh` — **exit 0**, now gates verdict correctness (REQ_BAT_001=PASS, no "0 PASS", REQ_BAT_002=PARTIAL).
- `cargo test -p deal --test verify_engine` — 15/15 pass (unchanged).
- New spec submodule commit: `dd3bb78238e44704d1df16d20174d8800873118f`.

## Known Stubs

None. The 6 remaining PARTIAL verdicts are correct: REQ_BAT_002 is `status="partial"` + `gap{}` (cold-weather tests scheduled 2026-Q3); REQ_BAT_003 / REQ_THM_001 / REQ_SYS_001/002/003 are test- or MATLAB-sim-backed with no captured evidence (range_model.py and motor_thermal.py are pre-existing 0-byte showcase stubs; motor_efficiency/vehicle_dynamics are MATLAB → graceful-skip). These stay PARTIAL until that evidence is supplied — they are NOT masked.

## Self-Check: PASSED

- cli/src/model_values.rs — FOUND (408 lines)
- cli/src/verify.rs — FOUND (maps {} + REQ.attr resolution wired)
- cli/src/simulate.rs — FOUND (ModelValueIndex fallback wired)
- scripts/phase-5-smoke.sh — FOUND (verdict-gated)
- cli/tests/phase5_e2e.rs — FOUND (REQ_BAT_001=PASS asserted)
- Commit 1e2dca7 (Task 1) — FOUND
- Commit 4e96bca (Task 2 parent) — FOUND
- Commit a0c5a5f (Task 3) — FOUND
- spec submodule commit dd3bb78 — FOUND; parent gitlink pins `160000 commit dd3bb78 spec`
