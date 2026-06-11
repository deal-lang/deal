# Deferred Items — Phase 05 Execution

## Pre-existing test failures (out of scope for Phase 5)

### sema_dimensional.regression_pins — 07-conversion-mismatch.deal
- **File:** tests/unit/sema_dimensional.zig:194
- **Failure:** `sema.dimensional.regression_pins FAILED for 07-conversion-mismatch.deal: expected E25xx code E2503 not found in 0 diagnostic(s)`
- **Present before:** Phase 05 Plan 01 (confirmed via git stash test)
- **Disposition:** Pre-existing Phase-4 dimensional carryover; accepted, out of scope.
- **Re-confirmed in Plan 07 (2026-06-08):** Surfaced again as the lone failing Zig
  test under `zig build phase-5-gate-fresh` (phase-1-gate inherited step, 61 pass /
  1 fail) AND under a bare `zig build test -Dtest-filter=regression_pins` on current
  HEAD with no fresh worktree — proving it is independent of all Plan 07 changes
  (smoke / e2e / gate wiring). NOT a Plan 07 regression. The D-88 cross-file E2500
  CLI wiring (Plan 04) is the in-flight related work; the E2503 conversion-mismatch
  pin remains red as accepted Phase-4 carryover and is NOT fixed in Phase 5.

### deal-lsp doctest — hover::node_label
- **File:** lsp/src/hover.rs line 145
- **Failure:** Backtick rendering in doc comment causes Rust doctest parse error
- **Disposition:** Pre-existing; out of scope for Phase 05
