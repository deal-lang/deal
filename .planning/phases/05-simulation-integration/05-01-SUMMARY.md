---
phase: 05-simulation-integration
plan: "01"
subsystem: simulation-integration
tags: [c-abi, zig, rust, sim-protocol, clap, tdd-red, build-gate]
dependency_graph:
  requires: []
  provides:
    - deal_check_with_stdlib C ABI export (wraps analyzeWithExternalTable)
    - spec/sims/v0/ normative JSON simulation protocol (D-70)
    - Rust CLI scaffolding: simulate, evidence, verify, sims_protocol modules
    - Wave 0 red test stubs for Plans 02-04 targets
    - phase-5-gate + phase-5-gate-fresh build steps
    - deal-sim materialization in verify-fresh-worktree.sh
  affects:
    - src/lib.zig (new C ABI export #10)
    - src/sema.zig (doc comment updated)
    - include/deal.h (new prototype)
    - deal-ffi/src/lib.rs (new extern "C" binding)
    - cli/src/main.rs (new modules, commands, flags)
    - build.zig (phase-5-gate/phase-5-gate-fresh steps)
    - scripts/verify-fresh-worktree.sh (deal-sim symlink + pip install)
    - Cargo.toml (sha2 workspace dep added)
tech_stack:
  added:
    - sha2 = "0.11" (workspace dep, D-83 staleness foundation)
  patterns:
    - C ABI export with ArenaAllocator + DealHandle lifecycle (NULL+non-zero guards)
    - JSON Schema draft-2020-12 (spec/sims/v0/)
    - BTreeMap for alphabetical key order (D-18)
    - #[ignore = "RED stub..."] + @unittest.skip pattern for Wave 0 test stubs
key_files:
  created:
    - src/lib.zig (export fn deal_check_with_stdlib added)
    - spec/sims/v0/input.schema.json
    - spec/sims/v0/output.schema.json
    - spec/sims/v0/metadata.schema.json
    - spec/sims/v0/README.md
    - cli/src/simulate.rs
    - cli/src/evidence.rs
    - cli/src/verify.rs
    - cli/src/sims_protocol.rs
    - cli/tests/simulate_integration.rs
    - cli/tests/e2500_cli.rs
    - deal-sim/tests/test_simulation.py (sibling repo, not tracked in deal/)
    - deal-sim/tests/test_validation.py (sibling repo, not tracked in deal/)
    - scripts/phase-5-smoke.sh
  modified:
    - src/sema.zig (doc comment: "Reachable from C ABI via deal_check_with_stdlib")
    - include/deal.h (DealHandle* deal_check_with_stdlib prototype)
    - deal-ffi/src/lib.rs (extern "C" binding for deal_check_with_stdlib)
    - cli/src/main.rs (module decls, Simulate/Evidence commands, Check flags)
    - build.zig (phase-5-gate, phase-5-gate-fresh steps)
    - scripts/verify-fresh-worktree.sh (deal-sim sibling + pip install -e step)
    - Cargo.toml (sha2 = "0.11" workspace dep)
    - cli/Cargo.toml (sha2 workspace ref, walkdir workspace ref)
decisions:
  - "deal_check_with_stdlib returns ?*anyopaque (DealHandle*) not bool — caller must call deal_free() to avoid memory leak"
  - "stdlib_ir_ptr/len accepts DEAL source bytes (not pre-parsed JSON) — parsed into external SymbolTable inside the function"
  - "stdlib_arena uses defer deinit() safely because analyzeWithExternalTable copies keys into handle arena before returning"
  - "spec/sims/v0/ schemas committed inside spec submodule first, then parent repo stages updated gitlink"
  - "SimEntry includes reproducibility and depends_on fields (D-75 / explicit dependency override)"
  - "sha2 upgraded from 0.10 to 0.11 in workspace to unify version across crates"
  - "Wave 0 Python test stubs placed in deal-sim sibling repo (not tracked in deal/ git)"
metrics:
  duration: "~1.5h (3 sessions due to context compaction)"
  completed: "2026-06-08"
  tasks: 3
  files_created: 15
  files_modified: 8
---

# Phase 05 Plan 01: Wave 0 Scaffolding Summary

**One-liner:** C ABI export `deal_check_with_stdlib` wrapping `analyzeWithExternalTable` with JWT-handle return type, JSON simulation protocol v0 schemas, four Rust CLI module stubs with clap surface, and Wave 0 red-test infrastructure targeting Plans 02-04.

## Tasks Completed

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | deal_check_with_stdlib C ABI export | 1c4c6f3 | src/lib.zig, include/deal.h, deal-ffi/src/lib.rs, src/sema.zig |
| 2 | spec/sims/v0/ normative protocol + Rust stubs + clap surface | 9f44610 | spec/sims/v0/*.json, spec/sims/v0/README.md, cli/src/{simulate,evidence,verify,sims_protocol}.rs, cli/src/main.rs |
| 3 | Wave 0 red test stubs + phase-5-gate + fresh-worktree fixes | 467eb1d | cli/tests/{simulate_integration,e2500_cli}.rs, scripts/phase-5-smoke.sh, build.zig, scripts/verify-fresh-worktree.sh |

## Acceptance Criteria Verified

- `zig build` exits 0; `deal_check_with_stdlib` symbol present in libdeal.a
- `cargo build -p deal` exits 0
- `deal --help` lists `simulate` and `evidence` subcommands
- `deal check --help` lists `--verify`, `--simulations`, `--run-sims` flags
- All 3 spec/sims/v0/*.schema.json parse as valid JSON with `deal_sim_protocol` field
- spec/sims/v0/README.md has exit-code table, D-18 sort requirement, 50+ lines
- `SimEntry` has `reproducibility` and `depends_on` fields (D-75)
- `cargo test --workspace` exits 0 (excluding pre-existing failures)
- build.zig contains `phase-5-gate` and `phase-5-gate-fresh` steps
- verify-fresh-worktree.sh has >3 references to `deal-sim`
- `bash -n verify-fresh-worktree.sh` parses clean
- Wave 0 Python test stubs skip correctly (14 tests, all skipped in deal-sim sibling repo)
- Wave 0 Rust test stubs are ignored (5 tests with #[ignore])

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] deal_check_with_stdlib return type changed from bool to handle pointer**
- **Found during:** Task 1 implementation
- **Issue:** Initial stub returned `bool`, but the implementation creates an arena-backed DealHandle. Returning bool caused a memory leak — the handle was never exposed to the caller and could never be freed.
- **Fix:** Changed return type to `?*anyopaque` (Zig) / `*mut DealHandle` (Rust FFI) / `DealHandle*` (C header). Caller must call `deal_free(handle)` to release.
- **Files modified:** src/lib.zig, include/deal.h, deal-ffi/src/lib.rs
- **Commit:** 1c4c6f3

**2. [Rule 2 - Missing critical functionality] ASVS V5 NULL+non-zero guards added to deal_check_with_stdlib**
- **Found during:** Task 1 — threat model T-05-01/T-05-02 requires these guards at all C ABI entry points
- **Fix:** Added NULL pointer + non-zero length consistency checks for all three ptr/len pairs (source, filename, stdlib_ir) before any dereference. Returns null on invalid inputs.
- **Files modified:** src/lib.zig
- **Commit:** 1c4c6f3

**3. [Rule 2 - Missing critical functionality] UTF-8 validation and source > 4GiB guard**
- **Found during:** Task 1 — matching pattern in existing C ABI exports (deal_parse, deal_check)
- **Fix:** UTF-8 validation on source + filename slices; `source_len > std.math.maxInt(u32)` guard with error diagnostic
- **Files modified:** src/lib.zig
- **Commit:** 1c4c6f3

**4. [Rule 2 - Missing critical functionality] sha2 version unified from 0.10 to 0.11 in workspace**
- **Found during:** Task 2 — cli/Cargo.toml referenced `sha2 = "0.10"` while adding workspace dep
- **Fix:** Upgraded workspace dep to sha2 = "0.11" and made cli reference `{ workspace = true }`; eliminates duplicate sha2 build via Cargo's dependency deduplication
- **Files modified:** Cargo.toml, cli/Cargo.toml
- **Commit:** 9f44610

## Known Stubs

The following stubs are intentional Wave 0 scaffolding — their implementation targets are tracked in future plans:

| Stub | File | Target Plan |
|------|------|-------------|
| run_simulate() / validate_bindings() / evaluate() / validate_output() | cli/src/simulate.rs | Plan 03 |
| run_evidence() / capture() / baseline() | cli/src/evidence.rs | Plan 05 |
| evaluate() / evaluate_from_bytes() / run_verify() | cli/src/verify.rs | Plan 05 |
| validate_input/output/metadata() | cli/src/sims_protocol.rs | Plan 02 |
| phase-5-smoke.sh body | scripts/phase-5-smoke.sh | Plan 07 |
| test_simulate_battery_thermal_produces_output_json | cli/tests/simulate_integration.rs | Plan 03 |
| test_simulate_writes_metadata_json | cli/tests/simulate_integration.rs | Plan 03 |
| test_simulate_stale_flag_skips_fresh_sims | cli/tests/simulate_integration.rs | Plan 03 |
| test_e2500_voltage_assigned_to_mass_attribute | cli/tests/e2500_cli.rs | Plan 04 |
| test_e2500_correct_mass_unit_no_error | cli/tests/e2500_cli.rs | Plan 04 |
| TestDealSimulation (6 tests) | deal-sim/tests/test_simulation.py | Plan 02 |
| TestInputValidation / TestOutputValidation (8 tests) | deal-sim/tests/test_validation.py | Plan 02 |

## Deferred Issues

### Pre-existing test failures (not caused by this plan)

These were confirmed pre-existing via stash test before any Plan 01 changes:

1. **sema_dimensional.regression_pins — 07-conversion-mismatch.deal**: Expected E25xx code not found in 0 diagnostics. Out of scope for Wave 0 scaffolding; logged in deferred-items.md.

2. **deal-lsp doctest — hover::node_label**: Backtick in doc comment causes Rust doctest parse error. Out of scope; logged in deferred-items.md.

### deal-sim Python tests not tracked in deal/ git

The Wave 0 Python test stubs (`deal-sim/tests/test_simulation.py`, `deal-sim/tests/test_validation.py`) are created in the `deal-sim` sibling repository (`/Users/dunnock/projects/deal-lang/deal-sim/`). They are NOT tracked in the `deal/` git repo. Plan 02 will manage these files within the deal-sim repo directly.

## Threat Surface

No new network endpoints or trust-boundary schema changes introduced. The `deal_check_with_stdlib` C ABI export is guarded by NULL+non-zero checks (T-05-01/T-05-02) per ASVS V5. All existing security mitigations from prior C ABI exports (UTF-8 validation, source size guard) are applied consistently.

## Self-Check: PASSED

- src/lib.zig contains `deal_check_with_stdlib`: FOUND
- spec/sims/v0/input.schema.json: FOUND
- spec/sims/v0/output.schema.json: FOUND
- spec/sims/v0/metadata.schema.json: FOUND
- spec/sims/v0/README.md: FOUND
- cli/src/simulate.rs: FOUND
- cli/src/evidence.rs: FOUND
- cli/src/verify.rs: FOUND
- cli/src/sims_protocol.rs: FOUND
- cli/tests/simulate_integration.rs: FOUND
- cli/tests/e2500_cli.rs: FOUND
- scripts/phase-5-smoke.sh: FOUND
- Commit 1c4c6f3: FOUND
- Commit 9f44610: FOUND
- Commit 467eb1d: FOUND
