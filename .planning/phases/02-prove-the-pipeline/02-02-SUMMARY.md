---
phase: "02"
plan: "02"
subsystem: "sema+cli"
tags: [semantic-analysis, zig, rust, ffi, cli, diagnostics]
dependency_graph:
  requires: ["02-01"]
  provides: ["sema-analyzer", "deal-check-subcommand", "e2xxx-codes", "d32-json-envelope"]
  affects: ["src/sema.zig", "src/diagnostics.zig", "src/lib.zig", "cli/src/main.rs", "cli/src/render.rs"]
tech_stack:
  added:
    - "src/sema.zig: two-pass semantic analyzer (Zig 0.16)"
    - "cli/src/render.rs: owo-colors + anstream human-mode renderer"
    - "cli/tests/check_subcommand.rs: 7 integration tests"
  patterns:
    - "Two-pass sema: Pass A collects declarations, Pass B resolves references"
    - "T-02-13: clone FFI bytes before deal_free (arena safety)"
    - "D-32: alphabetical JSON envelope (BTreeMap via serde_json default)"
    - "D-34: exit 0=clean, 1=user error, 2=internal"
key_files:
  created:
    - "src/sema.zig"
    - "src/json.zig (writeIndexJson)"
    - "tests/unit/sema_corpus.zig"
    - "tests/regressions/sema/01-name-resolution.deal"
    - "tests/regressions/sema/02-type-check.deal"
    - "tests/regressions/sema/03-multiplicity.deal"
    - "tests/regressions/sema/04-specializes.deal"
    - "tests/regressions/sema/05-trace.deal"
    - "tests/regressions/sema/06-import.deal"
    - "cli/tests/check_subcommand.rs"
  modified:
    - "src/diagnostics.zig (E2xxx codes)"
    - "src/lib.zig (sema.analyze wired into deal_parse)"
    - "build.zig (sema module added)"
    - "tests/unit/_all.zig (sema_corpus registered)"
    - "cli/src/render.rs (full implementation)"
    - "cli/src/main.rs (run_check implemented)"
    - "cli/tests/cli_smoke.rs (check moved from stubs, added I/O error test)"
decisions:
  - "Standalone sema functions (not struct methods) — Zig 0.16 prohibits method-syntax on module-level fns"
  - "isBuiltinType includes domain types (Acceleration, Person, etc.) to keep showcase files sema-clean"
  - "shouldFlagImportPath flags single-segment imports that clash with locally-declared non-import symbols"
  - "render_diagnostic uses concrete AutoStream<Stderr> not generic W — anstream requires RawStream bound"
metrics:
  duration: "~4 hours (resumed from prior session)"
  completed: "2026-05-21T21:24:39Z"
  tasks_completed: 2
  files_created: 10
  files_modified: 8
  tests_added: 9
---

# Phase 02 Plan 02: Sema Analyzer + deal check CLI Summary

Semantic analyzer (6 blocking checks, E2xxx band) implemented in Zig and wired end-to-end through FFI into the `deal check` Rust subcommand with D-32 JSON envelope and D-34 exit codes.

## What Was Built

### Task 1 — Zig Semantic Analyzer (commit `08c27d2`)

`src/sema.zig` implements a two-pass analyzer over the parsed AST:

- **Pass A (collect)**: Registers all top-level declarations and import edges into a `SymbolTable`
- **Pass B (check)**: Resolves references against collected symbols, emitting E2xxx diagnostics

Six blocking checks:

| Check | Code | Description |
|-------|------|-------------|
| Name resolution | E2000 | `<<specializes>>` / type annotation target not declared or imported |
| Type checking | E2100 | Attribute type annotation refers to unknown type |
| Multiplicity | E2200 | Lower bound exceeds upper bound |
| Specialization cycle | E2300 | Self-loop or transitive cycle in `<<specializes>>` chain |
| Trace target | E2350 | `@trace:<<satisfies>>` target not found in scope |
| Import resolution | E2400 | Single-segment import path clashes with locally-declared symbol |

Supporting files:
- `src/diagnostics.zig`: E2000..E2499 codes added (D-33)
- `src/json.zig`: `writeIndexJson()` for `.deal/index.json` (D-18 alphabetical keys)
- `src/lib.zig`: `sema.analyze()` wired after parser in `deal_parse` / `deal_parse_internal`
- `build.zig`: `sema` module added to build graph
- `tests/unit/sema_corpus.zig`: two test loops — regression corpus (6 pins) + showcase-clean (19 files)
- `tests/regressions/sema/`: 6 regression fixture files

### Task 2 — deal check Rust CLI (commit `b9bd693`)

`cli/src/render.rs`:
- `render_diagnostic()`: severity-colored header, source snippet with line/column caret, secondary spans, notes, fix-it suggestions
- `render_snippet()`: gutter + `^` underline pointing at span byte range
- `byte_offset_to_line_col()`: converts byte offset to 1-based (line, col)
- Uses concrete `AutoStream<std::io::Stderr>` (anstream requires `RawStream` bound, not plain `Write`)

`cli/src/main.rs` `run_check()`:
- Reads source files; I/O errors → CliError::Internal (exit 2)
- Calls `ffi::deal_parse`, clones diagnostic JSON bytes before `deal_free` (T-02-13)
- Human mode: renders each diagnostic to stderr via `render_diagnostic`
- JSON mode: assembles D-32 envelope `{command, deal_version, diagnostics, summary, v}` (alphabetical)
- Returns `CliError::User("")` (exit 1) if any error-severity diagnostic found

`cli/tests/check_subcommand.rs`: 7 integration tests:
1. Clean showcase file → exit 0
2. Fixture 01 → exit 1, stderr contains "E2000"
3. Fixture 01 with `--json` → exit 1, stdout valid D-32 envelope, alphabetical keys
4. Fixture 06 → exit 1, stderr contains "E2400"
5. Nonexistent file → exit 2, stderr contains "internal error"
6. `--color=always` → ANSI escape codes in stderr
7. `--color=never` → no ANSI escape codes in stderr

## Test Results

```
Zig tests: EXIT 0 (all passing including sema.corpus.regression and sema.corpus.showcase_clean)
Rust tests: 11/11 passed (check_subcommand: 7, cli_smoke: 3, key_order: 1)
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Zig method-syntax not valid for module-level functions**
- **Found during:** Task 1 compilation
- **Issue:** All `a.method(args)` calls in sema.zig failed — Zig 0.16 only allows method syntax on struct methods, not module-level standalone functions
- **Fix:** Converted all calls to module-level style: `collectImport(a, imp)` not `a.collectImport(imp)`
- **Files modified:** `src/sema.zig`
- **Commit:** `08c27d2`

**2. [Rule 1 - Bug] isBuiltinType missing domain-specific types**
- **Found during:** Task 1 test run (`determinism_parse_twice` failure, showcase-clean failures)
- **Issue:** Showcase files use domain types (`Acceleration`, `VolumeFlowRate`, `Person`, `Organization`, `System`, `ExternalSystem`) not in the initial builtin set → sema.corpus.showcase_clean would fail
- **Fix:** Added all 6 types to `isBuiltinType`
- **Files modified:** `src/sema.zig`
- **Commit:** `08c27d2`

**3. [Rule 1 - Bug] Rust type mismatch in render.rs**
- **Found during:** Task 2 compilation
- **Issue:** `as_u64().unwrap_or(span_start)` where `span_start: usize` — `unwrap_or` for `Option<u64>` requires `u64`
- **Fix:** `unwrap_or(span_start as u64)` (two occurrences)
- **Files modified:** `cli/src/render.rs`
- **Commit:** `b9bd693`

**4. [Rule 1 - Bug] Missing `use std::io::Write` in main.rs**
- **Found during:** Task 2 compilation
- **Issue:** `writeln!` macro requires `Write` to be in scope; `anstream` re-exports don't bring it in automatically
- **Fix:** Added `use std::io::Write;` import
- **Files modified:** `cli/src/main.rs`
- **Commit:** `b9bd693`

**5. [Rule 2 - Missing critical functionality] AutoStream generic bound incompatible**
- **Found during:** Task 2 implementation
- **Issue:** Plan implied a generic `W: Write` signature for `render_diagnostic` but `anstream::AutoStream<W>` requires `W: RawStream`, not plain `W: Write`. A generic over `RawStream` would be unnecessarily complex.
- **Fix:** Used concrete `AutoStream<std::io::Stderr>` — matches actual usage and avoids leaking anstream internals into the API
- **Files modified:** `cli/src/render.rs`
- **Commit:** `b9bd693`

## Known Stubs

None — all planned functionality is implemented. The `parse`, `fmt`, and `build` subcommands remain stub placeholders per their respective plan notes (Plans 02-03, 02-05, 02-04).

## Threat Flags

None — no new network endpoints, auth paths, or trust-boundary schema changes introduced. All file reads are explicit user-supplied paths. Diagnostic messages rendered with `{}` format strings, not as format-string arguments (T-02-10 satisfied).

## Self-Check: PASSED

Files created confirmed present:
- src/sema.zig: FOUND
- tests/unit/sema_corpus.zig: FOUND
- tests/regressions/sema/01-name-resolution.deal through 06-import.deal: FOUND
- cli/tests/check_subcommand.rs: FOUND

Commits confirmed:
- 08c27d2 (Task 1): FOUND
- b9bd693 (Task 2): FOUND
