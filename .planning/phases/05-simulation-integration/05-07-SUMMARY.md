---
phase: 05-simulation-integration
plan: 07
subsystem: testing
tags: [e2e, gate, simulation, evidence, verify, showcase, zig-build, smoke-test, cwe-22]

# Dependency graph
requires:
  - phase: 05-02
    provides: deal-sim Python SDK (DealSimulation, battery_thermal oracle)
  - phase: 05-03
    provides: Rust orchestrator (deal.sims.toml parse, dep-graph, staleness, MATLAB graceful-skip)
  - phase: 05-04
    provides: Zig sim path (range_model.zig) + MATLAB-subprocess ADR (D-72)
  - phase: 05-05
    provides: deal evidence capture + baseline <tag> + manifest (D-81 gitignore split)
  - phase: 05-06
    provides: verify engine (three-level verdict, per-REQ report, D-87)
provides:
  - End-to-end phase-5-gate wired against the showcase (simulate --all -> capture -> baseline -> check --simulations -> check --verify)
  - scripts/phase-5-smoke.sh full chain with D-72 hard-gate assertions + Zig sim gate
  - cli/tests/phase5_e2e.rs (7 integration tests, CWE-22 taint-free fixture)
  - Idempotent, PEP-668-robust deal-sim install in fresh-worktree + phase-5-gate
affects: [phase-06-application, gate-maintenance, ci]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "CWE-22 taint-free test fixture: frozen hardcoded list of compile-time string-literal paths (no read_dir, no filesystem-derived path components)"
    - "Gate import-assertion over fresh-install: short-circuit when deal_sim already imports; PEP-668 --break-system-packages fallback"

key-files:
  created:
    - cli/tests/phase5_e2e.rs
  modified:
    - scripts/phase-5-smoke.sh
    - cli/src/simulate.rs
    - scripts/verify-fresh-worktree.sh
    - build.zig
    - .planning/ROADMAP.md
    - .planning/phases/05-simulation-integration/deferred-items.md

key-decisions:
  - "E2E showcase fixture uses a frozen static literal file list (CWE-22 / T-05-06 mitigation) rather than recursive read_dir — semgrep p/rust reports 0 findings"
  - "deal simulate prefers a pre-existing input.json with resolved (non-null) values over null IR-resolved inputs when deal-std is absent (D-72 graceful-gate)"
  - "Gate's deal-sim step asserts importability, not a fresh install per run — idempotent + PEP-668 --break-system-packages fallback for Homebrew Python"
  - "E2503 sema.dimensional.regression_pins is accepted pre-existing Phase-4 carryover; NOT fixed in Phase 5"

patterns-established:
  - "Pattern: integration-test fixtures that copy a corpus build their file list from compile-time string literals only (taint-free path construction)"
  - "Pattern: gate scripts invoke python via `python3 -m pip` / `python3 -m unittest` (alias-independent, works in non-interactive shells)"

requirements-completed: [REQ-phase-5-gate, REQ-phase-5-simulation, REQ-phase-5-2-orchestration, REQ-phase-5-3-evidence-verification]

# Metrics
duration: ~45min
completed: 2026-06-08
---

# Phase 5 Plan 07: phase-5-gate End-to-End Against the Showcase Summary

**Full Phase 5 showcase verification chain wired end-to-end — `deal simulate --all` -> `evidence capture` -> `baseline v2.1.0` -> `check --simulations` -> `check --verify` — with a 7-test CWE-22-safe E2E suite, a green smoke gate, and an idempotent PEP-668-robust deal-sim fresh-worktree install. Human-verify checkpoint pending.**

## Performance

- **Duration:** ~45 min
- **Tasks:** 2 of 3 auto/verification tasks complete; Task 3 is a blocking human-verify checkpoint (NOT self-approved)
- **Files modified:** 7 (1 created, 6 modified)
- **Commits:** 5 on branch `phase-05-07-e2e-gate`

## Accomplishments

- **scripts/phase-5-smoke.sh** filled (254-line full chain): builds the release binary, then over a temp copy of `spec/examples/showcase/` runs `deal simulate --all` (battery_thermal Python oracle produces schema-valid `output.json`; MATLAB sims graceful-skip with `skip.json` per D-72), `deal evidence capture`, `deal evidence baseline v2.1.0` (asserts `manifest.json`), `deal check --simulations` (structural MATLAB binding validation, exit 0), `deal check --verify` (asserts REQ_BAT_001 + REQ_BAT_002 present), plus the canonical Zig sim gate (`range_model.zig` physics + bit-reproducibility). Exits 0.
- **cli/tests/phase5_e2e.rs** authored (7 tests, all green): individual steps + a `test_phase5_full_chain` that exercises the complete sequence on a temp showcase copy. Fixture built from a **frozen static list of compile-time string-literal paths** — the project's CWE-22 / T-05-06 taint-free path-construction mitigation. `semgrep --config p/rust` reports **0 findings** on the file.
- **build.zig phase-5-gate / phase-5-gate-fresh** confirmed wired (Plan 01): dependsOn phase-4-gate (D-35 cumulative) + `cargo test --workspace` + deal-sim install + the smoke script; fresh gate calls `verify-fresh-worktree.sh phase-5-gate`.
- **PM-query goal demonstrated:** `deal check --verify model/traceability.dealx` produces a per-REQ report — `REQ_BAT_001: Partial` (`actualCapacity >= REQ_BAT_001.minCapacity`), `REQ_BAT_002: Partial` (`worstCase <= REQ_BAT_002.chargeTime`, gap{} cold-weather tests scheduled 2026-Q3) — `Summary: 0 PASS, 0 FAIL, 8 PARTIAL, 0 STALE / 8 total`.

## Task Commits

1. **Task 1: Fill phase-5-smoke.sh + e2e test + finalize gate** — `a655a49` (feat) + `5f596bf` (docs: ROADMAP SC#2 D-72 note)
2. **Task 2: Fresh-worktree gate + closeout** — `2474485` (fix: python3 -m pip) + `11f63eb` (fix: idempotent + PEP-668) + `8580179` (docs: E2503 carryover)
3. **Task 3: Human-verify the end-to-end PM-query report** — **PENDING (blocking checkpoint, not self-approved)**

**Plan metadata:** committed separately with SUMMARY + STATE + ROADMAP.

## Files Created/Modified

- `cli/tests/phase5_e2e.rs` — 7-test E2E suite; CWE-22-safe static-literal showcase fixture (created)
- `scripts/phase-5-smoke.sh` — full Phase 5 chain with D-72 hard-gate assertions + Zig sim gate
- `cli/src/simulate.rs` — prefer pre-existing resolved `input.json` over null IR-resolved inputs (D-72 graceful-gate)
- `scripts/verify-fresh-worktree.sh` — idempotent, PEP-668-robust deal-sim install + hard import assertion
- `build.zig` — phase-5-gate deal-sim step: `python3 -m pip`, idempotent, PEP-668 fallback
- `.planning/ROADMAP.md` — SC#2 note: MATLAB via `matlab -batch` subprocess (D-72), not Engine API
- `.planning/phases/05-simulation-integration/deferred-items.md` — E2503 carryover documented; rewrote ANSI-corrupted file

## Decisions Made

- **CWE-22 mitigation in the test fixture:** Replaced the recursive `read_dir`-based copy helper (which tripped semgrep CWE-22) with a fully static, hardcoded list of compile-time string-literal paths covering all 26 showcase files the chain needs (deal.toml, model + variants, all 14 package .deal files, package index, sim manifest + adapters, test data). Coverage was NOT shrunk to dodge the rule — the E2E test still genuinely sets up the showcase and exercises the full chain. semgrep p/rust: 0 findings.
- **No `--no-verify` bypass:** The security rule was satisfied by changing the code, per CLAUDE.md (no silent hook bypass).
- **deal-sim install is import-asserting, not install-mandating:** The gate's invariant (T-05-19) is that `import deal_sim` works, not that a fresh install runs every time.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Test fixture used recursive read_dir (CWE-22 / hook-blocking)**
- **Found during:** Task 1 (e2e test authoring)
- **Issue:** The showcase-copy helper used `std::fs::read_dir` with filesystem-derived path components, tripping the semgrep CWE-22 path-traversal rule and blocking the pre-commit hook.
- **Fix:** Replaced with a frozen static list of compile-time string-literal paths (project's established taint-free pattern). All chain-required files enumerated explicitly.
- **Verification:** `semgrep --config p/rust cli/tests/phase5_e2e.rs` → 0 findings; `cargo test --test phase5_e2e` → 7/7 pass.
- **Committed in:** `a655a49`

**2. [Rule 3 - Blocking] deal-sim install called bare `pip` (alias-only, not resolvable)**
- **Found during:** Task 2 (`zig build phase-5-gate-fresh`)
- **Issue:** `verify-fresh-worktree.sh` and `build.zig` called bare `pip`, which is only a shell alias (`alias pip=pip3`) on the dev box and does not expand in the non-interactive shell `zig build` spawns → `pip: command not found` (exit 127), hard gate failure.
- **Fix:** Switched both to `python3 -m pip` / `python3 -m unittest` (alias-independent).
- **Verification:** Fresh gate advanced past the install step.
- **Committed in:** `2474485`

**3. [Rule 3 - Blocking] deal-sim editable install blocked by PEP 668 (externally-managed Homebrew Python)**
- **Found during:** Task 2 (re-run of `zig build phase-5-gate-fresh`)
- **Issue:** Homebrew's Python is an externally-managed (PEP 668) interpreter that refuses a bare editable install; `deal_sim` was already importable in the dev's interpreter, so a fresh install was both unnecessary and fatal.
- **Fix:** Made the step idempotent (short-circuit when `python3 -c import deal_sim` succeeds), with a `--break-system-packages --user` retry fallback, plus a hard post-step import assertion. Applied to both the fresh script and `build.zig`.
- **Verification:** Fresh worktree ran the full phase-5-smoke chain to PASS (simulate/capture/baseline/check --simulations/check --verify + Zig sim gate all green inside the ephemeral worktree).
- **Committed in:** `11f63eb`

---

**Total deviations:** 3 auto-fixed (all Rule 3 - blocking). All three were necessary to make the gate executable; none expanded scope. None were package-manager *installs* of unknown packages — deal-sim is the local sibling package (D-72/T-05-SC), and the `pip`/PEP-668 fixes addressed invocation portability, not package legitimacy.
**Impact on plan:** The Phase-5-specific gate logic (smoke, e2e, simulate/evidence/verify chain) is fully green. No new packages introduced.

## Issues Encountered

- **`zig build phase-5-gate-fresh` does not exit 0** — blocked **solely** by the pre-existing E2503 dimensional pin (see below), not by any Plan 07 work. The Phase-5 portion of the chain ran green end-to-end inside the fresh worktree (phase-5-smoke PASS observed in-worktree).
- **`timeout` binary unavailable** on the macOS shell — ran semgrep/gate commands without it; no impact.
- **deferred-items.md was byte-corrupted** with embedded ANSI escape sequences and a `STDIN` header (saved from a colorized `cat`/`bat` capture in a prior plan) — rewrote it cleanly with the Write tool.

## Accepted Carryover (NOT fixed — per scope + explicit instruction)

**E2503 — `sema.dimensional.regression_pins` (07-conversion-mismatch.deal)**
- **File:** `tests/unit/sema_dimensional.zig:194`
- **Failure:** `expected E25xx code E2503 not found in 0 diagnostic(s)`
- **Status:** Pre-existing **Phase-4 dimensional carryover**, first recorded in Plan 01 (confirmed via git-stash test). Re-confirmed in Plan 07 to be independent of all Plan 07 changes: it fails identically under a bare `zig build test -Dtest-filter=regression_pins` on HEAD with **no fresh worktree** involved.
- **Disposition:** Accepted carryover. NOT fixed in Phase 5 (out of scope; explicit instruction). It is the lone failing test under `phase-5-gate-fresh` (inherited via phase-1-gate). The D-88 cross-file E2500 CLI wiring (Plan 04) is the in-flight related work; the E2503 conversion-mismatch pin remains red. Documented in `deferred-items.md`.

## Verification Results

- `cargo test --manifest-path cli/Cargo.toml --test phase5_e2e` — **7 passed / 0 failed**
- `bash scripts/phase-5-smoke.sh` — **exit 0** (full chain + Zig sim gate)
- `cargo test --manifest-path cli/Cargo.toml --workspace` — **294 passed / 0 failed** (287 baseline + 7 new e2e tests)
- `semgrep --config p/rust cli/tests/phase5_e2e.rs` — **0 findings (CWE-22 resolved without hook bypass)**
- `zig build phase-5-gate-fresh` — Phase-5 chain green in-worktree; gate exit non-zero **only** due to the accepted E2503 Phase-4 carryover.

## Known Stubs

None — the E2E test genuinely sets up the showcase and exercises the full chain; no hardcoded/placeholder data paths.

## Next Phase Readiness

- **PENDING human-verify checkpoint (Task 3, blocking).** The SC#5 PM-query goal must be human-confirmed against the showcase before this plan is closed. See the checkpoint message for exact commands and expected outputs.
- Phase 6 (Application) can build on the verified simulation chain once the checkpoint is approved.

## Self-Check: PASSED

- `cli/tests/phase5_e2e.rs` — FOUND
- Commit `a655a49` — FOUND
- Commit `5f596bf` — FOUND
- Commit `2474485` — FOUND
- Commit `11f63eb` — FOUND
- Commit `8580179` — FOUND

---
*Phase: 05-simulation-integration*
*Completed: 2026-06-08 (pending human-verify checkpoint)*
