---
phase: 5
slug: simulation-integration
status: approved
nyquist_compliant: true
wave_0_complete: true
created: 2026-06-08
audited: 2026-06-09
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Seeded from `05-RESEARCH.md` § Validation Architecture. The planner completes the
> Per-Task Verification Map once tasks are assigned IDs.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework (Zig)** | Zig built-in test runner (`zig build test`) |
| **Framework (Rust)** | Cargo test (`cargo test --workspace`) |
| **Framework (Python)** | Python `unittest` / stdlib test runner (airgap-safe; no external test deps) |
| **Config file** | `build.zig` (Zig), `Cargo.toml` (Rust), `pyproject.toml` (Python) |
| **Quick run command** | `cargo test -p deal --lib && python -m unittest discover deal-sim/tests` |
| **Full suite command** | `zig build phase-5-gate && cargo test --workspace` |
| **Estimated runtime** | ~60–120 seconds (excludes MATLAB; see Manual-Only) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p deal --lib && python -m unittest discover deal-sim/tests`
- **After every plan wave:** Run `cargo test --workspace && zig build test`
- **Before `/gsd:verify-work`:** `zig build phase-5-gate` must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

> Requirement→test seeds below come from research. The planner MUST map each to concrete Task IDs
> (`{N}-{plan}-{task}`) and Threat Refs once plans exist.

| Req ID | Behavior | Test Type | Automated Command | File Exists | Status |
|--------|----------|-----------|-------------------|-------------|--------|
| REQ-phase-5-1-deal-sim-sdk | `DealSimulation.cli()` reads `input.json`, validates, runs, writes `output.json` | unit (Python) | `cd ../deal-sim && python3 -m unittest discover tests` (`tests/test_simulation.py`) | ✅ `../deal-sim` (D-81 split) | ✅ green |
| REQ-phase-5-1-deal-sim-sdk | Schema validation rejects wrong-type input | unit (Python) | `cd ../deal-sim && python3 -m unittest discover tests` (`tests/test_validation.py`) | ✅ `../deal-sim` (D-81 split) | ✅ green |
| REQ-phase-5-2-orchestration | TOML parser reads showcase sims registry | unit (Rust) | `cargo test -p deal --lib simulate::tests::parse_registry_showcase` | ✅ `cli/src/simulate.rs` | ✅ green |
| REQ-phase-5-2-orchestration | Topological sort produces correct order for showcase sims | unit (Rust) | `cargo test -p deal --lib simulate::tests::dep_order_showcase` | ✅ `cli/src/simulate.rs` | ✅ green |
| REQ-phase-5-2-orchestration | Staleness hash changes when source file changes | unit (Rust) | `cargo test -p deal --lib simulate::tests::staleness_key_changes_on_source_change` | ✅ `cli/src/simulate.rs` | ✅ green |
| REQ-phase-5-2-orchestration | `deal simulate battery_thermal` runs Python sim end-to-end | integration | `cargo test -p deal --test simulate_integration` | ✅ `cli/tests/simulate_integration.rs` | ✅ green |
| REQ-phase-5-3-evidence-verification | STALE flag renders in per-REQ report when set | integration (Rust) | `cargo test -p deal --test verify_engine test_stale_detection` | ✅ `cli/tests/verify_engine.rs` | ✅ green |
| REQ-phase-5-3-evidence-verification | STALE **detected** in `run_verify`: drifted evidence vs baseline manifest → stale (D-83/D-84) | integration (Rust) | `cargo test -p deal --test verify_engine test_build_stale_overrides_flags_drift` (+ `_fresh_not_stale`, `discover_baseline_tag_*`) | ✅ `cli/tests/verify_engine.rs` | ✅ green |
| REQ-phase-5-3-evidence-verification | PARTIAL verdict (gap / unmapped field) | integration (Rust) | `cargo test -p deal --test verify_engine test_partial_verdict` | ✅ `cli/tests/verify_engine.rs` | ✅ green |
| REQ-phase-5-3-evidence-verification | PASS verdict with design evidence | integration (Rust) | `cargo test -p deal --test verify_engine test_pass_verdict` | ✅ `cli/tests/verify_engine.rs` | ✅ green |
| REQ-phase-5-3-evidence-verification | AND operator in criteria | unit + integration (Rust) | `cargo test -p deal --test verify_engine test_and_criteria` (+ `--lib verify::tests::and_criteria`) | ✅ `cli/tests/verify_engine.rs` | ✅ green |
| D-88 (E2500 carryover) | `deal check` exits non-zero with E2500 for cross-file dimension mismatch | integration | `cargo test -p deal --test e2500_cli` | ✅ `cli/tests/e2500_cli.rs` | ✅ green |
| REQ-phase-5-gate | `deal simulate --all` + `deal check --verify` full showcase green | gate | `zig build phase-5-gate` | ✅ `build.zig` | ⚠️ conditional — see note |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky/conditional*

> **⚠️ Gate note (audit 2026-06-09):** `zig build phase-5-gate` is RED in a bare workspace, but **no Phase-5 requirement fails**. The redness is entirely inherited/environmental:
> 1. `sema.dimensional.regression_pins` (E2503) — the **documented Phase-4 carryover pin** (see `05-LEARNINGS.md` / `deferred-items.md`), which fails only because the `../deal-stdlib` sibling is not materialized (`mod.deal` → `FileNotFound`). The transitive `recovery.corpus` failures share this root cause.
> 2. `scripts/check-snippets.sh` — fails because the `../deal-lang.org` docs sibling is absent.
>
> Both resolve when siblings are materialized (`phase-5-gate-fresh` symlinks them), except the lone E2503 pin, which is tracked Phase-4 debt independent of Phase 5. All Phase-5-specific suites (Python SDK 31✓, simulate/verify lib, `simulate_integration`, `verify_engine`, `e2500_cli`) are green.

---

## Wave 0 Requirements

> All Wave 0 dependencies materialized and green (audit 2026-06-09).

- [x] `../deal-sim/tests/test_simulation.py` — REQ-phase-5-1 SDK contract (sibling repo, D-81 gitignore split)
- [x] `../deal-sim/tests/test_validation.py` — REQ-phase-5-1 input validation (sibling repo)
- [x] `cli/tests/simulate_integration.rs` — end-to-end Python sim execution
- [x] `cli/tests/e2500_cli.rs` — D-88 cross-file E2500 CLI integration test
- [x] `cli/src/simulate.rs` — orchestration module (registry parse, Kahn topo sort, SHA-256 staleness)
- [x] `cli/src/evidence.rs` — evidence module
- [x] `cli/src/verify.rs` — verify engine module
- [x] `spec/sims/v0/` — protocol schema directory (input/output/metadata schemas)
- [x] `build.zig` — `phase-5-gate` + `phase-5-gate-fresh` build steps
- [x] C ABI export `deal_check_with_stdlib` for `analyzeWithExternalTable` (D-85/D-88 prerequisite)
- [x] `verify-fresh-worktree.sh` — `deal-sim` sibling symlink + idempotent PEP-668-robust install

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| MATLAB simulation executes via Engine API / `matlab -batch` | REQ-phase-5-2-orchestration | MATLAB absent in CI (research Pitfall 5) — gate must not fail hard when MATLAB unavailable | On a host with MATLAB: `deal simulate <matlab-sim>` produces schema-valid `output.json`; confirm subprocess adapter path |
| `deal check --verify --run-sims` re-runs stale sims to refresh evidence | REQ-phase-5-3-evidence-verification | Re-run path invokes `simulate::run_simulate_in` (Python/MATLAB tool) — non-deterministic in airgap CI; the invoked function is itself covered by `simulate_integration` + `simulate::tests`. The *wiring* (run_verify → run_simulate_in) is verified by code-review; the *detection* half is fully automated (see Per-Task Map). | On a host with Python: drift `.deal/evidence/<sim>/output.json`, run `deal check --verify` → expect STALE + non-zero exit; re-run `deal check --verify --run-sims` → evidence regenerated, verdict refreshed. |
| Full `phase-5-gate` green end-to-end | REQ-phase-5-gate | Requires sibling repos (`../deal-stdlib`, `../deal-lang.org`, `../deal-sim`) materialized; bare workspace yields inherited Phase-4 E2503 pin + snippet failures | In a materialized workspace: `zig build phase-5-gate-fresh` (symlinks siblings). Expect green except the documented E2503 dimensional-pin carryover (Phase-4 debt, tracked in `deferred-items.md`). |

### Known Carryover (not Phase-5 debt)
- **E2503 dimensional pin** — `sema.dimensional.regression_pins` (`tests/unit/sema_dimensional.zig`) expects `E2503` for `07-conversion-mismatch.deal`; produces 0 diagnostics when `../deal-stdlib/packages/units/mod.deal` is unavailable. Pre-existing Phase-4 debt, proven independent of Phase-5 work (see `05-LEARNINGS.md`).

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (incl. C ABI export + fresh-worktree gaps)
- [x] No watch-mode flags
- [x] Feedback latency < 120s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-06-08 (plan-checker verified all 14 implementation tasks carry `<automated>` verify blocks; `wave_0_complete` flips true after Wave 0 executes)

---

## Validation Audit 2026-06-09

Retroactive Nyquist audit (`/gsd:validate-phase 5`) — verified the pre-execution contract against the executed codebase by running every referenced suite.

| Metric | Count |
|--------|-------|
| Requirements audited | 12 |
| COVERED (green automated) | 11 |
| PARTIAL / conditional | 1 (`REQ-phase-5-gate` — inherited Phase-4 debt only) |
| MISSING (no test) | 0 |
| Gaps requiring new tests | 0 |
| Tests generated | 0 |

**Suites run:** `../deal-sim` 31 Python tests ✓ · `simulate` lib 12 ✓ · `verify` lib 7 ✓ · `verify_engine` 15 ✓ · `simulate_integration` 3 ✓ · `e2500_cli` 2 ✓.

**Corrections applied:** Per-Task Map test paths/commands updated to reflect actual locations (Python in `../deal-sim`; verify verdicts in `cli/tests/verify_engine.rs`; simulate test-fn names). `wave_0_complete` flipped `false → true`. Gate row reclassified ⚠️ conditional with carryover note.

**Conclusion:** Phase 5 remains **NYQUIST-COMPLIANT** — all Phase-5 requirements have green automated verification. The lone red (`phase-5-gate`) is attributable solely to environmental sibling absence + the documented Phase-4 E2503 carryover, not to any Phase-5 coverage gap.

---

## Validation Audit 2026-06-09 (#2) — STALE-detection wiring gap

The same-day milestone audit (`v2.1.0-MILESTONE-AUDIT.md`, 07:20) found that audit #1
had **mis-classified** `REQ-phase-5-3` STALE detection as green. Re-audit confirmed a real
gap masked by a misleading test:

- `test_stale_detection` injects `/*stale=*/ true` directly into the evaluator — it proves
  the STALE flag *renders*, not that it is *detected*.
- `verify::check_staleness` (verify.rs:506) was **dead code** — its only repo occurrence was
  its own definition; `run_verify` hardcoded `stale_overrides = HashMap::new()` (verify.rs
  "would be populated by staleness check"), so the STALE verdict was **unreachable in
  production** and drifted evidence yielded wrong PASS/PARTIAL.
- The `--run-sims` flag was dead: `run_verify` received it but never called
  `simulate::run_simulate` — stale evidence could not be refreshed.

| Metric | Count |
|--------|-------|
| Gaps found | 2 (STALE detection unreachable; `--run-sims` re-run dead) |
| Resolved (automated) | 1 — STALE detection now wired + 4 new tests |
| Resolved (wiring + manual-verify) | 1 — `--run-sims` re-run wired to `run_simulate_in` (manual-only test, Python-dependent) |
| Escalated | 0 |
| Tests generated | 4 (`cli/tests/verify_engine.rs`) |

**Fix (TDD red→green):**
- New seam in `cli/src/verify.rs`: `discover_baseline_tag()` resolves the baseline tag from
  `evidence/baselines/<tag>/manifest.json`; `build_stale_overrides()` maps each sim binding →
  stale via `check_staleness`. `run_verify` now collects evidence bindings, resolves staleness,
  and (on `--run-sims`) force-refreshes via `simulate::run_simulate_in` before evaluating.
- New tests: `test_build_stale_overrides_flags_drift`, `_fresh_not_stale`,
  `test_discover_baseline_tag_picks_greatest`, `_none_when_absent`.
- Suites green: `deal` lib 62 ✓ · `verify_engine` 19 ✓ · `simulate_integration` 3 ✓ · `e2500_cli` 2 ✓.

**Design note (baseline-tag selection):** with no "current baseline" pointer convention,
`discover_baseline_tag` picks the lexicographically-greatest tag carrying a manifest
(deterministic, D-18; versioned tags ⇒ ≈ latest). A future closure phase may add an explicit
active-baseline selector.

**Conclusion:** the STALE-detection half of `REQ-phase-5-3` is now genuinely **COVERED**
(not a false marker); the `--run-sims` re-run half is **wired + manual-only** (Python-dependent,
consistent with the MATLAB carve-out). Milestone-audit blockers for the verify subsystem are
closed at the wiring level.
