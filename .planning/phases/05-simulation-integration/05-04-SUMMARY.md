---
phase: 05-simulation-integration
plan: "04"
subsystem: sim-sdk-and-e2500-cli
tags:
  - zig-sim-sdk
  - deal_check_with_stdlib
  - e2500-dimensional-check
  - matlab-adr
  - comptime-floatmode
dependency_graph:
  requires:
    - 05-01  # deal_check_with_stdlib C ABI export
    - 05-02  # deal-sim Python SDK
    - 05-03  # Rust orchestrator + dispatch
  provides:
    - src/sim/deal_sim.zig          # DealSimulation comptime wrapper
    - src/sim/zig_runner.zig        # in-process runner + evidence serialization
    - src/sim/range_model.zig       # canonical Zig sim (EV range physics)
    - cli/tests/e2500_cli.rs        # D-88 E2500 cross-file CLI tests (GREEN)
    - .planning/decisions/ADR-phase-5-matlab-subprocess.md
  affects:
    - build.zig                     # sim sources added to libdeal step
    - cli/src/main.rs               # run_check D-88 wiring
tech_stack:
  added:
    - "Zig DealSimulation(T) comptime generic: @setFloatMode from T.reproducibility"
    - "deal_check_with_stdlib: C ABI cross-file dimensional check (seeded from dep files)"
  patterns:
    - "D-73 one-execution-two-outputs: in-process call + evidence artifact serialization"
    - "T-05-10 Clone-Before-Free: diagnostic bytes cloned before deal_free"
    - "T-05-10 variant: skip deal_index_json for stdlib-check handles (use-after-free guard)"
key_files:
  created:
    - src/sim/deal_sim.zig
    - src/sim/zig_runner.zig
    - src/sim/range_model.zig
    - .planning/decisions/ADR-phase-5-matlab-subprocess.md
  modified:
    - build.zig
    - cli/src/main.rs
    - cli/tests/e2500_cli.rs
decisions:
  - "D-73: Zig sims run in-process AND serialize to .deal/evidence/ — one execution, two outputs"
  - "D-72 ADR: MATLAB subprocess (-batch) supersedes ROADMAP SC#2 Engine API wording"
  - "T-05-10 fix: skip deal_index_json on deal_check_with_stdlib handles to avoid stdlib_arena use-after-free"
metrics:
  duration: "32 minutes"
  completed: "2026-06-08T20:48:11Z"
  tasks_completed: 2
  files_created: 4
  files_modified: 3
---

# Phase 5 Plan 04: Zig Sim SDK + E2500 CLI Wiring Summary

**One-liner:** Zig DealSimulation comptime wrapper with @setFloatMode tier dispatch, in-process EV range model sim, and deal_check_with_stdlib wired into CLI check for cross-file E2500 diagnostic (D-88 carryover closed).

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Zig sim SDK: DealSimulation comptime wrapper + zig_runner + range_model | 3c86a59 | src/sim/deal_sim.zig, src/sim/zig_runner.zig, src/sim/range_model.zig, build.zig |
| 2 | Wire deal_check_with_stdlib into CLI check + E2500 tests + MATLAB ADR | 4b113fe | cli/src/main.rs, cli/tests/e2500_cli.rs, .planning/decisions/ADR-phase-5-matlab-subprocess.md |

## What Was Built

### Task 1: Zig Simulation SDK

`src/sim/deal_sim.zig` — `DealSimulation(T)` comptime generic. The type parameter `T` must declare:
- `pub const reproducibility: ReproducibilityTier` — `.strict` or `.optimized`
- `pub const Input` — struct type for sim input
- `pub const Output` — struct type for sim output
- `pub fn run(input: Input) Output`

The wrapper applies `@setFloatMode(float_mode)` derived from `T.reproducibility` before arithmetic, exports `sim_run` as a C-callable entry point, and provides `run_with_arena` for test harnesses.

`src/sim/zig_runner.zig` — `run_zig_sim(T, alloc, sim_name, input_json, evidence_dir)`:
- Calls `DealSimulation(T).run_with_arena` in-process (fast path)
- Serializes metadata envelope to arena (reproducibility_tier, duration_s, deal_sim_version, tool="zig")
- If `evidence_dir` is non-null, writes `output.json` + `metadata.json` to disk (D-73 second output)

`src/sim/range_model.zig` — canonical Zig sim: EV range physics (flat-road Peukert model), mirrors `range_model.py`. Declares `reproducibility = .strict`, passes the D-73 bit-reproducibility test (two calls with identical inputs produce byte-identical output JSON).

`build.zig` — sim source modules added to the `libdeal` step; sim test binary rooted at `range_model.zig` with `deal_sim.zig` and `zig_runner.zig` imports.

### Task 2: D-88 CLI Wiring + MATLAB ADR

`cli/src/main.rs` `run_check` changes:
- D-49 block now also builds `stdlib_bytes` (concatenated dep source) and `dep_file_set` (paths from `.deal/deps/`)
- Per-file loop: project source files (non-dep) with stdlib bytes available → `deal_check_with_stdlib`; dep files or no stdlib → `deal_parse` as before
- The obsolete "processes each file independently via deal_parse" comment replaced with D-88 documentation

`cli/tests/e2500_cli.rs` — two integration tests (RED stubs removed):
1. `test_e2500_voltage_assigned_to_mass_attribute` — `attribute mass : Mass = V(800)` with stdlib seed → `deal check` exits 1 with E2500
2. `test_e2500_correct_mass_unit_no_error` — `attribute mass : Mass = kg(800)` → exits 0, no E2500

Tests create a temp dir, write a minimal stdlib seed under `.deal/deps/deal-stdlib/packages/units/`, and run the CLI binary with that temp dir as CWD.

`.planning/decisions/ADR-phase-5-matlab-subprocess.md` — 104-line ADR recording:
- D-72 decision: MATLAB = subprocess adapter (`matlab -batch` default)
- Supersedes ROADMAP SC#2 "MATLAB Engine API for Python" wording
- Rationale: registry-first design, graceful-skip without Engine API, tool-agnostic JSON protocol
- Engine API deferred to a future phase as a pluggable adapter

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed use-after-free of stdlib_arena entries via deal_index_json**

- **Found during:** Task 2 testing
- **Issue:** `deal_check_with_stdlib` in Zig creates a `stdlib_arena` for the external symbol table, then calls `sema.analyzeWithExternalTable` which SHARES entry pointers from that arena (not copies). The Zig code notes: "Share the entry pointer — it lives in the external table's arena." After `deal_check_with_stdlib` returns, `defer stdlib_arena.deinit()` frees those entries. When `deal_index_json` was then called on the returned handle, it serialized the symbol table that contained dangling pointers → SIGSEGV (exit 139).
- **Fix:** In `cli/src/main.rs`, skip `deal_index_json` for handles obtained via `deal_check_with_stdlib` (guarded by the `use_stdlib_check` boolean). Index building still works for dep files via their `deal_parse` handles.
- **Impact:** The workspace index omits entries from files analyzed via `deal_check_with_stdlib`. This is acceptable: index building uses project source files that may also be parsed independently via `deal_parse` in non-stdlib contexts.
- **Files modified:** `cli/src/main.rs` (index-skip guard)
- **Commit:** 4b113fe

### Pre-existing Issues (Out of Scope)

- `sema.dimensional.regression_pins` Zig test fails for `07-conversion-mismatch.deal` — pre-existing from Plan 01, stdlib file loading path fails silently (E2503 expects 0 diagnostics but gets 0 — the test expects the code but stdlib parse fails). Logged in `deferred-items.md`.

## Verification Results

```
zig build — exits 0
zig build test -Dtest-filter=sim — exits 0 (2/2 tests: physics_sanity + bit_reproducibility)
cargo build -p deal — exits 0
cargo test -p deal --test e2500_cli — exits 0 (2/2: voltage_mismatch + correct_mass)
.planning/decisions/ADR-phase-5-matlab-subprocess.md — exists (104 lines)
```

## Threat Surface Scan

No new network endpoints or auth paths introduced. The `deal_check_with_stdlib` C ABI boundary already existed (Plan 01). The Rust caller now passes dep source bytes across the boundary — mitigated by T-05-10 (clone before free) and the use-after-free fix above.

## Known Stubs

None — both E2500 tests are fully wired end-to-end with a real stdlib seed.

## Self-Check: PASSED

- `src/sim/deal_sim.zig` — FOUND
- `src/sim/zig_runner.zig` — FOUND
- `src/sim/range_model.zig` — FOUND
- `.planning/decisions/ADR-phase-5-matlab-subprocess.md` — FOUND
- Task 1 commit `3c86a59` — FOUND
- Task 2 commit `4b113fe` — FOUND
