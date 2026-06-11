---
phase: 04-ecosystem
plan: "01"
subsystem: diagnostics, test-scaffolds, schema-bundle
tags: [reqif, xsd, e-codes, dimensional-algebra, golden-fixtures, wave-0]
dependency_graph:
  requires: []
  provides:
    - spec/references/omg-reqif/reqif.xsd (ReqIF 1.2 XSD validation target — D-60)
    - src/diagnostics.zig Codes.e_dimension_mismatch (E2500) et al.
    - tests/unit/sema_dimensional.zig (dimensional regression harness)
    - tests/golden/reqif/ (ReqIF golden fixture skeleton)
    - tests/regressions/sema/07-*.deal (dimensional regression fixtures)
  affects:
    - plan 04-02 (imports e_dimension_mismatch from diagnostics)
    - plan 04-04 (imports e_dependency_not_resolved E2402)
    - plan 04-05 (ReqIF emitter validates against reqif.xsd; fills .expected.reqif)
tech_stack:
  added: []
  patterns:
    - SHA256SUMS tamper-detection manifest (mirrors omg-sysml-v2 bundle convention)
    - comptime E-code guard (sema_dimensional.zig fails to compile if codes missing)
    - scaffold-skip pattern (test loop skips rather than fails until Plan 04 emits E25xx)
key_files:
  created:
    - spec/references/omg-reqif/reqif.xsd
    - spec/references/omg-reqif/driver.xsd
    - spec/references/omg-reqif/SHA256SUMS
    - spec/references/omg-reqif/README.md
    - tests/golden/reqif/README.md
    - tests/golden/reqif/01-requirement-def.deal
    - tests/golden/reqif/01-requirement-def.expected.reqif
    - tests/golden/reqif/02-trace-relation.deal
    - tests/golden/reqif/02-trace-relation.expected.reqif
    - tests/unit/sema_dimensional.zig
    - tests/regressions/sema/07-dimensional-mismatch.deal
    - tests/regressions/sema/07-mixed-unit.deal
    - tests/regressions/sema/07-unknown-unit.deal
  modified:
    - src/diagnostics.zig (E2402 + E2500..E2503 reserved; E2500..E2599 band comment added)
    - tests/unit/_all.zig (sema_dimensional.zig registered in umbrella)
decisions:
  - "Wave 0 scaffold-skip pattern: sema_dimensional.zig regression_pins loop skips (not fails) when E25xx code absent — Plan 04 removes the guard once checkDimensionalExpr is implemented"
  - "comptime E-code guard in sema_dimensional.zig enforces D-16 reservation contract at build time — file fails to compile if any E25xx constant is removed"
  - "07-*.deal fixtures use parse-valid but sema-invalid constructs so they can test the sema layer in isolation"
metrics:
  duration: "~23 minutes (Tasks 2+3; Task 1 completed in prior agent session)"
  completed: "2026-06-06"
  tasks_completed: 3
  files_created: 13
  files_modified: 2
---

# Phase 04 Plan 01: Foundation — XSD Bundle, E-codes, Wave 0 Scaffolds Summary

**One-liner:** OMG ReqIF 1.2 XSD acquired and SHA256-pinned (D-60), E2402+E2500..E2503 reserved in diagnostics.zig, and Wave 0 test scaffolds (golden fixture skeleton + dimensional sema harness) created as failing-first targets for Plans 04-02 through 04-05.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Acquire and verify OMG ReqIF 1.2 XSD bundle (D-60) | a356964 (deal), f68f15c (spec submodule) | spec/references/omg-reqif/{reqif.xsd, driver.xsd, SHA256SUMS, README.md} |
| 2 | Reserve E2402 + E2500..E2503 in diagnostics.zig | 952e127 | src/diagnostics.zig |
| 3 | Create Wave 0 test scaffolds — ReqIF golden skeleton + dimensional sema harness | c04f0ce | tests/golden/reqif/, tests/unit/sema_dimensional.zig, tests/regressions/sema/07-*.deal, tests/unit/_all.zig |

## Decisions Made

1. **Scaffold-skip pattern for sema_dimensional.zig**: The `regression_pins` loop prints a SCAFFOLD-SKIP message and continues (rather than returning `error.TestUnexpectedResult`) when the E25xx code is absent. This allows the harness to be build-registered and green before Plan 04 implements `checkDimensionalExpr`. Plan 04 removes the scaffold guard.

2. **comptime E-code guard**: `sema_dimensional.zig` references all five new constants (`e_dimension_mismatch`, `e_mixed_unit_comparison`, `e_unknown_unit`, `e_conversion_type_mismatch`, `e_dependency_not_resolved`) in a `comptime` block. If any code constant is removed from `diagnostics.zig`, the file fails to compile — enforcing the D-16 reservation contract at build time across the full plan wave.

3. **07-*.deal fixtures are parse-valid, sema-dirty**: The regression fixtures use syntactically valid DEAL constructs (`V(800)`, `kg(2000) > lb(4409)`, `furlongs(400)`) that the parser accepts cleanly. This cleanly separates parser and sema layers in test attribution.

4. **Golden .expected.reqif placeholders**: Wave 0 expected files contain a single comment line (`<!-- EXPECTED: hand-written ReqIF XML — to be authored in Plan 04-05 -->`). The byte-exact diff gate in Plan 04-05's test runner will replace these with the actual emitter output once the ReqIF emitter is implemented.

## Verification Results

- `cd spec/references/omg-reqif && shasum -a 256 -c SHA256SUMS` — exits 0 (Task 1 human-verified)
- `zig build` — exits 0 with all new E-codes and sema_dimensional.zig registered
- `zig build test -Dtest-filter=sema` — exits 0; regression_pins scaffold-skips gracefully, showcase_clean passes (zero E25xx in 19 showcase files)
- `cargo run -p deal -- parse tests/golden/reqif/01-requirement-def.deal` — exits 0
- `cargo run -p deal -- parse tests/golden/reqif/02-trace-relation.deal` — exits 0
- All three `07-*.deal` fixtures — `deal parse` exits 0

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

- `tests/golden/reqif/01-requirement-def.expected.reqif` — placeholder comment; real ReqIF XML authored in Plan 04-05
- `tests/golden/reqif/02-trace-relation.expected.reqif` — placeholder comment; real ReqIF XML authored in Plan 04-05
- `tests/unit/sema_dimensional.zig` regression_pins loop — scaffold-skip mode until Plan 04 implements `checkDimensionalExpr`

These stubs are intentional Wave 0 scaffolds. Plan 04-05 fills the expected ReqIF XML; Plan 04-02 (dimensional checker) activates the regression assertions.

## Self-Check: PASSED

Files exist:
- spec/references/omg-reqif/reqif.xsd — FOUND (committed in spec submodule f68f15c)
- src/diagnostics.zig — FOUND, contains e_dimension_mismatch = "E2500"
- tests/unit/sema_dimensional.zig — FOUND
- tests/golden/reqif/01-requirement-def.deal — FOUND
- tests/regressions/sema/07-dimensional-mismatch.deal — FOUND

Commits exist:
- f68f15c — FOUND (spec submodule, Task 1)
- 952e127 — FOUND (diagnostics.zig E-codes, Task 2)
- c04f0ce — FOUND (Wave 0 scaffolds, Task 3)
