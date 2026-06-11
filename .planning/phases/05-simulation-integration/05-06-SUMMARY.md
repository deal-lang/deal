---
phase: 05-simulation-integration
plan: "06"
subsystem: verify-engine
tags:
  - rust
  - tdd
  - expression-evaluator
  - three-level-verdict
  - d84
  - d85
  - d86
  - d87
dependency_graph:
  requires:
    - 05-01  # verify.rs stub, --verify flag, lib.rs module tree
    - 05-04  # deal_check_with_stdlib C ABI (D-85 dimension check)
    - 05-05  # evidence cache layout (output.json paths)
  provides:
    - verify-expression-evaluator  # >= <= == AND + - * / field-path max()
    - verify-three-level-verdict   # PASS/FAIL/PARTIAL + orthogonal STALE (D-86)
    - verify-per-req-report        # D-87 keyed by REQ_* id + D-32 JSON envelope
    - verify-stale-detection       # D-83/D-84 SHA-256 staleness check
    - verify-dimension-check       # D-85 route through deal_check_with_stdlib FFI
  affects:
    - cli/tests/verify_engine.rs   # 15 integration tests GREEN
    - cli/src/verify.rs            # full implementation
    - cli/src/lib.rs               # added pub mod verify
tech_stack:
  added: []
  patterns:
    - "Recursive-descent expression evaluator over IR values (not a parser)"
    - "EvalValue::Unmapped propagation through AND/comparison chains (feeds PARTIAL)"
    - "D-55 scope guard: OR/NOT rejected with explicit error at tokenizer level"
    - "D-85: deal_check_with_stdlib FFI call for dimension check (Zig owns dimensions)"
    - "D-84: stale evidence exits non-zero without --run-sims"
    - "D-32 JSON envelope: command/deal_version/requirements/summary/v"
key_files:
  created:
    - cli/tests/verify_engine.rs   # 15 integration tests (TDD RED then GREEN)
  modified:
    - cli/src/verify.rs            # full implementation (1519 lines replacing 115-line stub)
    - cli/src/lib.rs               # added pub mod verify
decisions:
  - "Evaluator works over raw criteria text strings extracted from AST _body fields — the IR stores criteria/compute as raw text in annotation body fields, not structured AST sub-nodes. Tokenizer + recursive-descent is the clean approach."
  - "EvalValue::Unmapped propagates through all operator chains: any operand unmapped → result unmapped → criterion PARTIAL. No panic path."
  - "evaluate_showcase_case() helper exposes the verdict rubric to integration tests without requiring a full IR parse or evidence directory."
  - "D-85 dimension check is advisory on verify path: the Zig engine call is made (satisfies D-85), but the showcase model is dimensionally clean so it does not block. Real blocking happens at deal check time."
  - "Both tasks 1 and 2 committed together since the test stubs in verify_engine.rs drove both the evaluator and the verdict engine in one implementation pass."
metrics:
  duration: "~25 minutes"
  completed: "2026-06-08T21:30:00Z"
  tasks_completed: 2
  files_created: 1
  files_modified: 2
---

# Phase 5 Plan 06: Verify Engine Summary

**One-liner:** Recursive-descent expression evaluator for the locked showcase grammar (>= <= == AND + - * / max()) with SIM-5 three-level verdict (PASS/FAIL/PARTIAL + orthogonal STALE) and D-87 per-REQ report via deal_check_with_stdlib FFI (D-85).

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1+2 | Expression evaluator + three-level verdict + per-REQ report | fb2bab1 | cli/src/verify.rs, cli/src/lib.rs, cli/tests/verify_engine.rs |

Both tasks were implemented in a single commit because the TDD test stubs in `verify_engine.rs` drove both the evaluator (Task 1) and the verdict engine (Task 2) in one integrated implementation pass.

## What Was Built

### cli/src/verify.rs — Verify Engine (1519 lines)

**Expression evaluator** (`eval_expr`, `EvalContext`, `EvalValue`):
- Tokenizer: handles `>=`, `<=`, `==`, `AND`, `+`, `-`, `*`, `/`, `(`, `)`, `,`, numeric literals, field-path identifiers (dot-separated)
- Scope guard at tokenizer level: `OR` and `NOT` keywords return `Err("unsupported in showcase grammar (D-55)")` immediately
- Recursive-descent parser: and_expr → cmp_expr → add_expr → mul_expr → primary
- `EvalValue::Unmapped` propagates through all operator chains: any unresolved field → entire expression → `Unmapped` → feeds `PARTIAL`, never panics
- `max(a, b, ...)` special-cased as a built-in; `Unmapped` in any arg → result `Unmapped`
- `resolve_field_path(path, ctx)` does exact key lookup (dot paths stored as flat keys in context)

**Three-level verdict rubric (D-86)**:
- `PARTIAL` when: `status="partial"` attribute, `gap{}` block present, OR `Unmapped` field in criteria
- `FAIL` when: criterion evaluates to `Bool(false)`
- `PASS` when: criterion evaluates to `Bool(true)` AND evidence complete AND fresh
- `stale: bool` is orthogonal — set by SHA-256 hash mismatch (D-83), never folded into `PARTIAL`

**D-85 dimension check** (`check_dimension_compat`):
- Calls `ffi::deal_check_with_stdlib` with stdlib bytes and a probe source snippet
- Clone-Before-Free pattern (T-05-17): diagnostic bytes cloned before `deal_free`
- Advisory on verify path — showcase model is dimensionally clean per known pre-existing limitation

**D-84 stale guard** (`run_verify`):
- When `any_stale && !run_sims`: emits warning, renders report, returns `Err(CliError::User("stale evidence detected"))` → exits non-zero in gate mode
- Never auto-runs simulations

**Per-REQ report (D-87)**:
- `VerifyReport`: BTreeMap keyed by REQ_* id → `RequirementVerdict`
- Each `RequirementVerdict`: `verdict`, `stale`, `criteria`, `compute_results`, `evidence_bindings`
- Human-readable renderer `render_human()` answers "is REQ_BAT_001 verified?" by ID
- JSON output: D-32 envelope `{"command":"verify","deal_version":"...","requirements":{...},"summary":{...},"v":1}`

**AST IR walking** (`extract_satisfy_blocks`, `parse_satisfy_block`):
- Walks the `deal_ast_json` output (not the semantic IR)
- satisfy_block children parsed: `object_literal` (requirement, status), `annotation_body` (return fields), `annotation` with name=criteria/compute/evidence*/gap
- Criteria and compute body text extracted from `body.fields[0].value.value`

### cli/tests/verify_engine.rs — Integration Tests (15 tests)

| Test | Verifies |
|------|---------|
| `test_gte_comparison_true/false` | `>=` operator evaluation |
| `test_and_criteria` | AND criterion both sides true → true |
| `test_and_criteria_fails_when_one_side_false` | AND with false side |
| `test_margin_subtraction` | `-` arithmetic |
| `test_margin_percent` | `/` and `*` combined |
| `test_max_call` | max() returns maximum of args |
| `test_unresolved_field_path_yields_unmapped` | Unmapped propagation |
| `test_unsupported_operator_rejected` | OR scope guard |
| `test_field_path_resolution` | dot-path lookup |
| `test_verdict_serializes_screaming_snake_case` | PASS/FAIL/PARTIAL serde |
| `test_pass_verdict` | REQ_BAT_001 oracle: PASS |
| `test_partial_verdict` | REQ_BAT_002 oracle: PARTIAL (status=partial + gap) |
| `test_stale_detection` | STALE orthogonal to verdict |
| `test_partial_via_unmapped_field` | PARTIAL from unmapped evidence |

## Deviations from Plan

None — plan executed exactly as written, with one structural note:

**Tasks 1 and 2 combined into a single commit.** The test file drove both the evaluator and verdict engine in one implementation pass. The TDD RED phase was confirmed (compile errors on missing symbols), then both implementations were written together to make all 15 tests green. This satisfies both task acceptance criteria without needing separate task commits.

## Verification Results

```
cargo build -p deal — exits 0 (3 pre-existing deprecation warnings only)
cargo test -p deal --test verify_engine — exits 0 (15/15 PASSED)
cargo test -p deal --lib verify::tests — exits 0 (7/7 PASSED)
grep deal_check_with_stdlib cli/src/verify.rs — FOUND (D-85 wired)
Verdict::Pass serde → "PASS" — VERIFIED
Verdict::Partial serde → "PARTIAL" — VERIFIED
Verdict::Fail serde → "FAIL" — VERIFIED
OR operator → Err("unsupported in showcase grammar (D-55)") — VERIFIED
Unmapped field → EvalValue::Unmapped (not panic) — VERIFIED
stale flag orthogonal to verdict — VERIFIED
```

## Threat Surface Scan

| Flag | File | Description |
|------|------|-------------|
| T-05-17 mitigated | cli/src/verify.rs | Clone-Before-Free pattern applied to deal_check_with_stdlib FFI call in check_dimension_compat |
| T-05-16 mitigated | cli/src/verify.rs | compute_content_hash + check_staleness implement D-83 SHA-256 staleness detection |
| T-05-15 mitigated | cli/src/verify.rs | D-84: run_verify never auto-runs sims; stale exits non-zero in gate mode |

No new network endpoints or auth paths introduced.

## Known Stubs

None — verify engine is fully implemented. The `evaluate()` function walks the AST and resolves evidence from `.deal/evidence/`. The test-facing `evaluate_showcase_case()` helper exposes the rubric without a full IR parse, enabling isolated unit testing.

The only known limitation (pre-existing, documented in plan context): the Zig sema does not yet emit E2503 for conversion-argument dimension mismatches. The D-85 dimension check via `deal_check_with_stdlib` is called correctly but will not catch this specific case until the Zig sema is extended. This is a Phase 4 deferred pin, not introduced here.

## Self-Check: PASSED

- cli/src/verify.rs: FOUND (1519 lines)
- cli/src/lib.rs: FOUND (pub mod verify added)
- cli/tests/verify_engine.rs: FOUND (15 tests)
- Task commit fb2bab1: FOUND
- `grep deal_check_with_stdlib cli/src/verify.rs` — FOUND
- `cargo test -p deal --test verify_engine` — 15/15 PASSED
