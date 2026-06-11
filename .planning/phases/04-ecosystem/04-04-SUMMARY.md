---
phase: 04-ecosystem
plan: 04
subsystem: sema
tags: [dimensional-algebra, si-units, check-7, e2500, e2501, e2502, e2503, stdlib-seeding, regression-harness]
depends_on:
  requires: [04-01, 04-02, 04-03]
  provides: [Check #7 dimensional algebra, live regression harness, vendored stdlib wiring]
  affects: [sema.zig, lib.zig, sema_dimensional.zig, main.rs]
tech_stack:
  added: [analyzeWithExternalTable, deal_parse_internal_with_stdlib, collect_deal_files_recursive]
  patterns: [symbol-table-seeding, import-collision-guard, recursive-dir-glob]
key_files:
  created:
    - tests/regressions/sema/07-conversion-mismatch.deal
  modified:
    - src/sema.zig
    - src/lib.zig
    - src/json.zig
    - tests/unit/sema_dimensional.zig
    - cli/src/main.rs
    - spec (submodule)
    - tests/snapshots/ast/showcase__packages__requirements__system.deal.json
    - tests/snapshots/tokens/showcase__packages__requirements__system.deal.json
decisions:
  - "DimVector = [7]i8 with index map 0=M,1=L,2=T,3=I,4=TH,5=N,6=J (BIPM SI Brochure 9th edition)"
  - "builtinDimVector() hardcodes dimension TYPE names (Mass, Voltage, etc.) — acceptable since they're already in isBuiltinType; no unit names hardcoded"
  - "E2502 fires when callee identifier not in scope at all; showcase files use explicit imports so their units are in scope"
  - "analyzeWithExternalTable seeds dimension_def/unit_def only — avoids shadowing local declarations with stdlib imports"
  - "Named import collision guard: if a name is already a unit_def/dimension_def (seeded from stdlib), the import statement does NOT overwrite it with a bare .imported entry"
  - "DEFAULT_STDLIB_TAG updated to v0.4.0 (from v0.1.0 placeholder set by Plan 02)"
metrics:
  duration: "~3 sessions (continued from prior context)"
  completed: "2026-06-06"
  tasks: 2
  files: 9
---

# Phase 04 Plan 04: Dimensional Algebra (Check #7) Summary

**One-liner:** Generic 7-exponent SI dimensional algebra in sema.zig (E2500–E2503), data-driven from stdlib metadata with no hardcoded unit names; full regression harness live with 4 E25xx pins and 19-file showcase-clean assertion.

## What Was Built

### Task 1 — Check #7 dimensional algebra in src/sema.zig (commit 8b085a9)

Added the complete dimensional algebra subsystem to `src/sema.zig`:

**Types and constants** (`DimVector = [7]i8`, `DIM_MASS`, `DIM_VOLTAGE`, etc.): index map follows BIPM SI Brochure 9th edition (0=M, 1=L, 2=T, 3=I, 4=TH, 5=N, 6=J). 16 named DimVector constants covering all SI base and common derived dimensions.

**`DimMeta` union**: distinguishes `dimension` (carries the DimVector directly) from `unit` (carries `dim_name` + `is_conversion` flag for D-57 explicit-conversion-only enforcement).

**Pass A extension in `collectDefinition`**: detects dimension_def and unit_def patterns by reading ADR-specified field names (`si_M..si_J` for dimension, `si_factor` + `<<specializes>>` for unit). No unit names hardcoded — the sema reads only what the ADR metadata encoding says.

**Check #7 functions** (all with explicit `error{OutOfMemory}!void` to break Zig's circular error set inference):
- `checkDimensionalExpr`: entry point from Pass B attribute_usage traversal
- `checkExprDimension`: recursive expression dispatcher
- `checkCallDimension`: handles E2502 (unknown unit), E2500 (dimension mismatch), E2503 (conversion with wrong source dimension)
- `checkBinaryDimension`: handles E2501 (mixed-unit same-dimension without conversion)

**`src/json.zig` fix**: added `dimension_def` and `unit_def` cases to `symbolKindName()` exhaustive switch.

**Regression fixture** `tests/regressions/sema/07-conversion-mismatch.deal`: `to_kg(V(800))` where `to_kg` expects Mass but `V(800)` is Voltage — triggers E2503.

**Bug fix (Rule 1)**: `spec/examples/showcase/packages/requirements/system.deal` was missing `kW` from its import statement, causing E2502 on `kW(250)` in REQ_MOT_001. Fixed in the spec submodule (commit `52a94c6`) and snapshots regenerated.

### Task 2 — Activate dimensional harness + wire vendored stdlib (commit 554e659)

**`src/sema.zig` — `analyzeWithExternalTable`**: new function that accepts an optional `external_table: ?*const SymbolTable`. Before running Pass A on the target file, it seeds the new table with all `dimension_def` and `unit_def` entries from the external table. The key is used by the test harness to give the sema full stdlib unit/dimension knowledge.

**Import collision guard in `collectImport`**: when a `named` import (e.g. `import deal.std.units.{V, to_kg}`) registers names into the symbol table, it checks whether the name is already a `unit_def` or `dimension_def` (seeded from stdlib). If so, it does NOT overwrite with a bare `.imported` entry — preserving the dimensional metadata needed for Check #7.

**`src/lib.zig` — `deal_parse_internal_with_stdlib`**: test-only function that:
1. Parses each stdlib source in a separate `stdlib_arena`
2. Runs `sema.analyze` on each to collect `dimension_def`/`unit_def` entries
3. Merges them into a `merged` symbol table
4. Calls `analyzeWithExternalTable` on the target source with the merged table as the seed
5. The stdlib_arena is freed after the handle's analysis completes (safe because diagnostics are already emitted)

**`tests/unit/sema_dimensional.zig`**: 
- Removed all `// SCAFFOLD` no-op guards (4 occurrences removed)
- Added E2503 regression pin for `07-conversion-mismatch.deal`
- Both test loops now use `lib.deal_parse_internal_with_stdlib` with stdlib sources loaded from `../deal-stdlib/packages/units/`
- `regression_pins` loop asserts the pinned E25xx code is found (returns `error.TestUnexpectedResult` on failure)
- `showcase_clean` loop asserts zero E25xx across all 19 files (returns `error.TestUnexpectedResult` on failure)

**`cli/src/main.rs`**:
- `DEFAULT_STDLIB_TAG` updated from `"v0.1.0"` to `"v0.4.0"` (single source of truth for `deal init` scaffolding, D-67)
- `run_check` now globs `.deal/deps/<dep>/packages/**/*.deal` after the E2402 guard and appends those sources to `resolved_paths` (D-49 vendored stdlib wiring)
- New `collect_deal_files_recursive` helper performs the recursive glob
- If a dep directory is present but has no `.deal` sources, a warning is emitted rather than a fatal error

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] json.zig exhaustive switch on SymbolKind missing new variants**
- **Found during:** Task 1 first build
- **Issue:** After adding `dimension_def` and `unit_def` to `SymbolKind`, `symbolKindName()` in `json.zig` failed to compile (non-exhaustive switch)
- **Fix:** Added the two cases to the switch
- **Files modified:** `src/json.zig`
- **Commit:** 8b085a9

**2. [Rule 1 - Bug] Circular error set in dimensional check functions**
- **Found during:** Task 1 compilation
- **Issue:** Zig's inferred error sets cannot handle mutual recursion between `checkExprDimension` and `checkBinaryDimension`
- **Fix:** Added explicit `error{OutOfMemory}!void` return type annotations to all four dimensional check functions
- **Files modified:** `src/sema.zig`
- **Commit:** 8b085a9

**3. [Rule 1 - Bug] Missing kW import in spec showcase system.deal**
- **Found during:** Task 1 determinism/snapshot tests after implementing E2502
- **Issue:** `system.deal` used `kW(250)` in REQ_MOT_001 but only imported `{kg, km, hr, kWh, min, degC, s}` — E2502 fired
- **Fix:** Added `kW` to the import statement; regenerated AST and token snapshots
- **Files modified:** `spec/examples/showcase/packages/requirements/system.deal`, two snapshot files
- **Commit:** 8b085a9 (snapshots) + spec submodule commit `52a94c6`

**4. [Rule 1 - Bug] Named import overwrites stdlib-seeded unit_def entries**
- **Found during:** Task 2 — `07-conversion-mismatch.deal` produced 0 diagnostics instead of E2503
- **Issue:** `import deal.std.units.{V, to_kg}` in the fixture called `StringHashMap.put` which overwrote the stdlib-seeded `unit_def` entries for `V` and `to_kg` with plain `.imported` entries, making `isConversionCall(to_kg)` return false
- **Fix:** Added guard in `collectImport` named-import path: skip `put` if existing entry is already `unit_def` or `dimension_def`
- **Files modified:** `src/sema.zig`
- **Commit:** 554e659

**5. [Rule 1 - Bug] Zig 0.16.0 ArrayList API mismatch in test file**
- **Found during:** Task 2 first test compile
- **Issue:** `std.ArrayList(T).init(gpa)` pattern fails in Zig 0.16.0 (allocator-explicit `.empty` pattern required); also `sources.append(item)` and `sources.toOwnedSlice()` need explicit allocator parameter
- **Fix:** Changed to `= .empty` + `append(gpa, ...)` + `toOwnedSlice(gpa)` + `deinit(gpa)` pattern
- **Files modified:** `tests/unit/sema_dimensional.zig`
- **Commit:** 554e659

## Known Stubs

None. The dimensional check operates on real stdlib metadata. All 4 E25xx codes fire correctly. The showcase-clean assertion passes against real stdlib-seeded analysis.

## Architecture Notes

The dimensional algebra is **data-driven**: the Zig core contains zero hardcoded unit names. All SI exponent knowledge comes from parsing `attribute def Mass { attribute si_M : Integer = 1; ... }` metadata in the stdlib DEAL files. An empty stdlib means no unit metadata → dimensional checks gracefully skip (no spurious E25xx).

The **import collision guard** is the key correctness invariant: when `analyzeWithExternalTable` seeds stdlib metadata before Pass A, the fixture's own import statements must not erase that metadata. The guard ensures the richer `unit_def` entry survives the import registration.

The **CLI wiring** adds vendored stdlib sources to `resolved_paths` in `run_check`. The current C ABI processes each file independently via `deal_parse` (single-file). Full cross-file symbol seeding for the CLI path (equivalent to `deal_parse_internal_with_stdlib`) requires a future multi-file C ABI entry point. The Zig test harness already exercises the correct path.

## Self-Check: PASSED

- `src/sema.zig` exists and contains `pub const DimVector` ✓
- `src/lib.zig` exists and contains `deal_parse_internal_with_stdlib` ✓
- `tests/unit/sema_dimensional.zig` exists with 0 SCAFFOLD guards ✓
- `tests/regressions/sema/07-conversion-mismatch.deal` exists ✓
- `cli/src/main.rs` contains `.deal/deps` (5 occurrences) and `DEFAULT_STDLIB_TAG: &str = "v0.4.0"` ✓
- Commit 8b085a9 exists ✓
- Commit 554e659 exists ✓
- `zig build test -Dtest-filter=sema.dimensional` exits 0 ✓
- `cargo build -p deal` succeeds ✓
