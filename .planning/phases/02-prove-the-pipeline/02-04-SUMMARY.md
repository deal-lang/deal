---
phase: "02"
plan: "04"
subsystem: cli
tags: [sysml-v2, codegen, emitter, schema-validation, golden-tests]
dependency_graph:
  requires: [02-03]
  provides: [sysml-v2-emitter, schema-registry, deal-build-subcommand, golden-fixtures]
  affects: [cli/src/main.rs, cli/src/sysml_v2.rs, cli/src/schema_registry.rs]
tech_stack:
  added:
    - jsonschema 0.46 (offline JSON Schema validator with Retrieve trait)
    - uuid 1.x with v5 feature (deterministic UUID synthesis)
    - sha2 0.10 + hex 0.4 (SHA256 tamper-detection on bundled schemas)
  patterns:
    - OnceLock-cached validator (process-global schema compilation)
    - UUID v5 keyed on qualified path (deterministic, stable across runs)
    - BTreeMap-backed serde_json::Map (alphabetical key order D-18)
    - Dual-gate golden testing (byte-exact + independent schema validity)
    - Per-test temp dir isolation for parallel subprocess tests
key_files:
  created:
    - cli/src/schema_registry.rs
    - cli/src/sysml_v2.rs
    - cli/tests/golden_sysml_v2.rs
    - cli/tests/golden_fixture_schema_validity.rs
    - cli/tests/sysml_validate.rs
    - tests/golden/sysml-v2/01-part-def.deal
    - tests/golden/sysml-v2/01-part-def.expected.json
    - tests/golden/sysml-v2/02-port-usage.deal
    - tests/golden/sysml-v2/02-port-usage.expected.json
    - tests/golden/sysml-v2/03-specialization.deal
    - tests/golden/sysml-v2/03-specialization.expected.json
    - tests/golden/sysml-v2/04-attribute-usage.deal
    - tests/golden/sysml-v2/04-attribute-usage.expected.json
    - tests/golden/sysml-v2/05-requirement-def.deal
    - tests/golden/sysml-v2/05-requirement-def.expected.json
    - tests/golden/sysml-v2/06-trace-link.deal
    - tests/golden/sysml-v2/06-trace-link.expected.json
    - tests/golden/sysml-v2/07-package.deal
    - tests/golden/sysml-v2/07-package.expected.json
    - tests/golden/sysml-v2/08-dealx-composition.dealx
    - tests/golden/sysml-v2/08-dealx-composition.expected.json
    - tests/golden/sysml-v2/README.md
  modified:
    - cli/src/main.rs
    - cli/src/ffi.rs
decisions:
  - UUID v5 namespace constant SYSML_NAMESPACE = 0x0e72_4d54_7e1a_4b32_9e8c_8e62_2d8f_1234 locked permanently
  - Dual-gate architecture — Gate 1 byte-exact match, Gate 2 WARNING-03 independent schema validity
  - Per-test unique temp dirs via --output flag to prevent parallel test races
  - build/sysml-v2/ fallback when tests/showcase symlink is broken (spec submodule not initialized)
metrics:
  duration: "~3 hours (including context compaction)"
  completed: "2026-05-21"
  tasks_completed: 2
  files_changed: 26
---

# Phase 02 Plan 04: SysML v2 Codegen Pipeline Summary

SysML v2 emitter with offline JSON Schema validator, UUID v5 synthesis, 8 dual-gate golden fixtures, and `deal build --target sysml-v2` subcommand wired end-to-end.

## What Was Built

### Task 1 — Core pipeline (`874d19b`)

**`cli/src/schema_registry.rs`** (355 LOC): Offline JSON Schema validator using `jsonschema` 0.46's `Retrieve` trait. `LocalBundleRetriever` holds a `HashMap<String, Value>` keyed on `$id` URIs. A recursive `index_schema_ids()` walk registers all `$defs` entries in `SysML.json` and `KerML.json`. SHA256 tamper-detection guards against schema corruption (T-02-21). An `OnceLock<Arc<Validator>>` caches the compiled validator process-wide (Pitfall 4). CWE-22 path-traversal guard via `canonicalize + assert_within`.

**`cli/src/sysml_v2.rs`** (718 LOC): IR JSON to SysML v2 JSON emitter. UUID v5 synthesis via `SYSML_NAMESPACE` constant (locked, must never change). `emit()` iterates IR nodes building a flat `ownedRelationship` list for a workspace Package. Per-kind emitters: `emit_package`, `emit_part_def`, `emit_port_def`, `emit_port_usage`, `emit_part_usage`, `emit_attribute_usage`, `emit_requirement_def`, `emit_connection_usage`, `emit_satisfy_dependency`, `emit_interface_def`, `emit_connection_def`, `emit_item_def`, and a generic fallback. Alphabetical key order guaranteed by `serde_json::Map` (`BTreeMap`-backed, no `preserve_order` feature).

**`cli/src/main.rs`**: `deal build --target sysml-v2 [--validate] [--output PATH]` subcommand. Reads source, calls `deal_parse` FFI (with Pitfall 3 Cow-clone of IR JSON bytes before `deal_free`), invokes `sysml_v2::emit_from_bytes()`, accumulates into consolidated workspace Package (D-24), optionally schema-validates, writes output.

**`cli/tests/sysml_validate.rs`**: End-to-end subprocess tests — sema error → exit 1, alphabetical order regression, fixture 01 exits 0, fixture 01 --validate exits 0.

### Task 2 — Fixtures, tests, docs (`4ffefcf`)

**8 golden fixture pairs** in `tests/golden/sysml-v2/` covering all major mapping categories (part_def, port_usage, specialization, attribute_usage, requirement_def, trace_link, package hierarchy, dealx composition).

**`cli/tests/golden_sysml_v2.rs`**: Gate 1 — byte-exact match per fixture. Each test uses a unique temp dir with `--output` flag to prevent parallel test race conditions (all 8 tests previously shared `build/sysml-v2/showcase.sysml-v2.json`).

**`cli/tests/golden_fixture_schema_validity.rs`**: Gate 2 — WARNING-03 independent schema validity. Reads `.expected.json` files directly (no emitter invocation) and validates UUID format, `@id == elementId`, known `@type`, and alphabetical key order at every nesting level. 9 tests (8 per-fixture + 1 rolled-up).

**`tests/golden/sysml-v2/README.md`**: Documents the two-gate workflow and snapshot-update procedure.

**`cli/src/ffi.rs`**: Fixed comment `Plan 02-04 will append deal_format` → `Plan 02-05 will append deal_format (D-21)`.

## Test Results

All 21 tests pass:
- `cargo test --test golden_sysml_v2`: 8/8 pass
- `cargo test --test golden_fixture_schema_validity`: 9/9 pass
- `cargo test --test sysml_validate`: 4/4 pass

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Parallel test race condition on shared output file**
- **Found during:** Task 2 test run
- **Issue:** All 8 `golden_sysml_v2` tests ran in parallel and all wrote to the same `build/sysml-v2/showcase.sysml-v2.json` output file. Each test read the file after its build but found another test's output due to interleaving. The `left` side of each assertion contained fragments from multiple fixtures concatenated together.
- **Fix:** Added `--output PATH` flag to `deal build` subcommand. Each test now writes to a unique temp dir (`/tmp/deal-golden-<stem>-<pid>/out.json`) and passes that path via `--output`. The 8 tests run safely in parallel with no shared file access.
- **Files modified:** `cli/src/main.rs` (added `--output` to `Build` command args + `run_build` parameter), `cli/tests/golden_sysml_v2.rs` (use temp dir per test)
- **Commit:** 4ffefcf

**2. [Rule 3 - Blocking] Unused import warning in schema validity test**
- **Found during:** Task 2 first compile
- **Issue:** `golden_fixture_schema_validity.rs` had `use std::process::Command;` inside `validate_expected_json()` left over from a planned subprocess approach that was replaced by inline structural checks.
- **Fix:** Removed the unused import.
- **Files modified:** `cli/tests/golden_fixture_schema_validity.rs`
- **Commit:** 4ffefcf

### Pre-existing Issues (not fixed, out of scope)

- `check_clean_file_exits_zero` test in `check_subcommand.rs` fails because `tests/showcase` is a broken symlink (spec submodule not initialized in worktree). Pre-dates Plan 02-04 changes.

## Known Stubs

None. The emitter produces real SysML v2 JSON for all 8 fixture categories. The `dealx` composition (fixture 08) emits a `Namespace` element with `dealKind: "system"` for .dealx system blocks — this is a documented simplification tracked in D-xx (future plan will add full ConnectionUsage mapping for `.dealx` `connect` directives).

## Threat Flags

None. No new network endpoints, auth paths, or trust boundary crossings introduced. Schema validation runs offline against bundled files only; `LocalBundleRetriever` returns `Err` for any non-local URI.

## Self-Check

### Check created files exist

All key files verified present on disk. Both task commits (874d19b, 4ffefcf) confirmed in git log.

## Self-Check: PASSED
