---
phase: 05-simulation-integration
plan: "03"
subsystem: simulate-orchestrator
tags:
  - rust
  - orchestration
  - simulation
  - subprocess
  - sha256
  - topological-sort
  - python-sdk
dependency_graph:
  requires:
    - 05-01  # stubs, clap surface, spec/sims/v0/ protocol
    - 05-02  # deal-sim Python SDK (run_cli, DealSimulation base class)
  provides:
    - simulate-registry-parse     # parse_registry()
    - simulate-dep-graph          # topological_order() Kahn's
    - simulate-staleness-key      # compute_staleness_key() D-83
    - simulate-dispatch           # dispatch_sim() D-71
    - simulate-graceful-skip      # D-72 skip.json on NotFound
    - simulate-output-validation  # sims_protocol::validate_output()
    - simulate-integration-tests  # GREEN end-to-end Python sim test
  affects:
    - 05-04  # evidence capture plan reads evidence/ artifacts
    - 05-05  # verify plan calls sims_protocol::validate_output
tech_stack:
  added:
    - sha2 crate (SHA-256 staleness key, D-83)
    - hex crate (hex::encode for SHA-256 digest)
    - toml crate (deal.sims.toml deserialization)
    - serde_json (input.json / output.json envelope I/O)
    - jsonschema (OnceLock<Validator> against spec/sims/v0/output.schema.json)
  patterns:
    - Kahn's topological sort with BTreeMap-sorted zero-in-degree queue (D-18 determinism)
    - SHA-256 over (BTreeMap-ordered inputs JSON, sim source bytes, registry TOML section) — D-83
    - OnceLock<Validator> for schema validation (matches schema_registry.rs pattern)
    - std::process::Command::new (never shell interpolation) — T-05-07
    - ErrorKind::NotFound → write_skip_record() + SimResult::Skipped — D-72
    - Dual-target CliError: defined in lib.rs (integration tests) AND main.rs (binary root)
    - Shape A/B input.json unwrapping in Python SDK (_extract_inputs)
key_files:
  created: []
  modified:
    - cli/src/simulate.rs      # orchestration engine (1066 lines, 12 unit tests)
    - cli/src/sims_protocol.rs # output schema validation (316 lines, 5 unit tests)
    - cli/src/lib.rs           # expose simulate/sims_protocol + shared CliError
    - cli/src/main.rs          # comment clarifying dual-target CliError
    - cli/tests/simulate_integration.rs  # GREEN end-to-end tests (3 tests)
    - ../deal-sim/src/deal_sim/cli.py    # Rule 1 fix: envelope extraction + scalar unwrap
decisions:
  - "Dual-target CliError: defined separately in lib.rs (for integration tests) and main.rs (binary root) because Rust has separate crate:: namespaces for each compilation root"
  - "dispatch_python tries python3 first then python as fallback — maximizes CI compatibility"
  - "model_path resolution emits stderr diagnostic for unresolved paths and uses null value rather than failing hard — mirrors D-72 graceful-skip philosophy"
  - "deal-sim cli.py Shape A/B input detection: if top-level 'inputs' key is a dict, treat as spec/sims/v0/ envelope; otherwise bare flat dict (backwards-compatible)"
metrics:
  duration: "~2 hours (split across two sessions due to context compaction)"
  completed: "2026-06-08T19:59:03Z"
  tasks: 2
  files_changed: 6
---

# Phase 5 Plan 03: Simulate Orchestration Engine Summary

Rust orchestration engine for `deal simulate` — registry parse, dependency graph, staleness detection, tool dispatch, and output validation. Both integration tests GREEN.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Registry parse + dep-graph + staleness key | ccd4c36 | cli/src/simulate.rs, cli/src/sims_protocol.rs |
| 2 | Dispatch + IR resolution + protocol validation + GREEN tests | 6bf6343 | cli/src/lib.rs, cli/src/main.rs, cli/tests/simulate_integration.rs |
| 2 (auto-fix) | Python SDK envelope extraction bug | dbf6810 (deal-sim) | deal-sim/src/deal_sim/cli.py |

## What Was Built

### cli/src/simulate.rs — Orchestration Engine

Full implementation of the `deal simulate` engine:

- `parse_registry(path)` — TOML parse of `deal.sims.toml` into `SimsRegistry` with `BTreeMap`-keyed `simulations` map (D-18 alphabetical key order)
- `topological_order(&registry)` — Kahn's algorithm: builds output_path→producer index, adds edge producer→consumer when `inputs[].model_path` matches another sim's `outputs[].model_path`, honors explicit `depends_on`, sorts zero-in-degree queue (D-18 determinism), detects cycles via `order.len() < registry.len()` → `CliError::User("circular dependency...")`
- `compute_staleness_key(resolved_inputs, sim_source_path, registry_entry_toml)` — SHA-256 over BTreeMap-ordered inputs JSON + sim source file bytes + registry TOML section string (D-83)
- `validate_model_path(mp)` — validates against `^[a-zA-Z0-9._]+$` (T-05-06 path-traversal guard)
- `dispatch_sim(sim_name, entry, input_path, output_path, metadata_path, workdir, ev_dir, stderr)` — routes by `entry.tool`:
  - `python`: `Command::new("python3")` → fallback to `python`; passes `--input/--output/--metadata` args
  - `matlab`/`generic`: splits `runner` string into command+args, spawns via `Command::new` (T-05-07 no shell interpolation)
  - `zig`: stub — writes skip.json, returns `Skipped`
  - Unknown tool: writes skip.json, returns `Skipped`
  - `ErrorKind::NotFound` → `write_skip_record(ev_dir, sim_name, reason)` + `SimResult::Skipped` (D-72)
- `build_input_json(entry, ir_elements, stderr)` — resolves `inputs[].model_path` against IR elements map; emits stderr diagnostic for unresolved paths; wraps values into `{"value": v, "unit": u}` spec/sims/v0/ envelope
- `run_simulate_in(project_root, names, all, stale)` — loads IR from project root, resolves dep order, dispatches each sim; staleness check against `metadata.json` staleness_key (D-83)
- `run_simulate(names, all, stale)` — wraps `run_simulate_in` from `std::env::current_dir()`

12 unit tests: `parse_registry_showcase`, `dep_order_producer_before_consumer`, `dep_order_cycle_detection`, `dep_order_deterministic`, `dep_order_showcase`, `staleness_key_deterministic`, `staleness_key_changes_on_source_change`, `staleness_key_changes_on_input_change`, `staleness_key_changes_on_registry_change`, `model_path_valid_paths`, `model_path_invalid_paths`, `graceful_skip_writes_skip_json` — all GREEN.

### cli/src/sims_protocol.rs — Output Schema Validation

- `validate_output(output: &serde_json::Value) -> Result<(), Vec<String>>` — OnceLock<Validator> pattern, loads spec/sims/v0/output.schema.json, validates against jsonschema; falls back to `validate_output_structural()` if schema file unavailable
- `validate_output_structural()` — manual check: `deal_sim_protocol = "v0"`, `exit_code` is integer, `outputs` is object, `v` = 1
- SHA-256 pins on schema files (T-02-21 adapted): `EXPECTED_INPUT_SHA256`, `EXPECTED_OUTPUT_SHA256`, `EXPECTED_METADATA_SHA256`

5 unit tests: `protocol_version_is_v0`, `check_protocol_field_accepts_v0`, `check_protocol_field_rejects_missing`, `validate_output_accepts_valid_envelope`, `validate_output_rejects_missing_protocol_field` — all GREEN.

### deal-sim/src/deal_sim/cli.py — Shape A/B Input Extraction (Rule 1 Auto-Fix)

- `_extract_inputs(raw: dict) -> dict` — detects Shape B envelope (`"inputs"` key is a dict) and extracts the inner inputs dict, unwrapping each `{"value": v, "unit": u}` → `v` scalar. Shape A (bare flat dict) returned as-is.
- `_wrap_outputs(result: dict, outputs_schema: dict) -> dict` — wraps run() scalar values back into `{"value": v, "unit": u}` format using units from `cls.outputs`, producing a valid output.json envelope.
- Output envelope now fully conforms to spec/sims/v0/output.schema.json.

### cli/tests/simulate_integration.rs — GREEN End-to-End Tests

Three tests all GREEN:

1. `test_simulate_battery_thermal_produces_output_json` — creates temp project, pre-populates input.json with valid values (packResistance=0.05 ohm, totalCurrent=250 A, coolantFlowRate=30 L/min), dispatches `battery_thermal.py` via `dispatch_sim()`, asserts output.json exists with `heatGenerated`/`coolantOutTemp` keys, validates against spec/sims/v0/output.schema.json, checks physics Q=I²R = 250²×0.05 = 3125 W (within 1 W tolerance).

2. `test_simulate_writes_metadata_json` — same dispatch, asserts metadata.json has `deal_sim_protocol = "v0"`, `duration_s`, `tool = "python"`.

3. `test_simulate_stale_flag_skips_fresh_sims` — runs `run_simulate_in()` twice; verifies `--stale` flag doesn't crash and evidence dir has `metadata.json` or `skip.json`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Python SDK cli.py passes full input.json envelope to _validate_inputs()**
- **Found during:** Task 2 integration test execution
- **Issue:** `deal_sim/cli.py` read the entire `input.json` (spec/sims/v0/ envelope) and passed it directly to `_validate_inputs()`. The envelope contains top-level keys `deal_sim_protocol`, `inputs`, `v` which are not declared in `cls.inputs`. This caused `ValueError: Input key 'deal_sim_protocol' is not declared in cls.inputs` (exit 2) on every Python sim dispatch.
- **Fix:** Added `_extract_inputs()` helper that detects Shape B envelope (top-level `"inputs"` key is a dict) and extracts + unwraps inner scalars. Added `_wrap_outputs()` to produce conforming output envelope. Updated `run_cli()` to use both helpers.
- **Files modified:** `deal-sim/src/deal_sim/cli.py`
- **Commit:** `dbf6810` (deal-sim repo)

**2. [Rule 3 - Blocking] CliError dual-target compilation issue**
- **Found during:** Integration test compilation
- **Issue:** Integration tests use `deal::simulate::*` which requires `deal::CliError` from the library target. But `CliError` was only defined in `main.rs` (binary root). The library target (`lib.rs`) and binary target (`main.rs`) have different `crate::` namespaces — `lib.rs` modules reference `crate::CliError` in the lib context, which must be defined there.
- **Fix:** Added `CliError` definition to `lib.rs` (canonical for lib target + integration tests). Added comment to `main.rs` explaining the dual-target situation. Both definitions are identical by convention.
- **Files modified:** `cli/src/lib.rs`, `cli/src/main.rs`
- **Commit:** `6bf6343`

## Test Results

```
running 45 tests (lib)
... all ok
running 58 tests (main)
... all ok
running 3 tests (simulate_integration)
test test_simulate_writes_metadata_json ... ok
test test_simulate_battery_thermal_produces_output_json ... ok
test test_simulate_stale_flag_skips_fresh_sims ... ok
test result: ok. 3 passed; 0 failed
```

Total: 103 tests passing across all test suites.

## Security Mitigations Applied

| Threat ID | Mitigation | Verified |
|-----------|-----------|---------|
| T-05-06 | `validate_model_path()` against `^[a-zA-Z0-9._]+$` before filesystem ops | unit test `model_path_invalid_paths` |
| T-05-07 | `Command::new(cmd).args(...)` — runner split by whitespace, never shell-interpolated | `grep -n "sh -c" cli/src/simulate.rs` returns empty |
| T-05-08 | `ErrorKind::NotFound` → `write_skip_record()` + `SimResult::Skipped` (never Err) | unit test `graceful_skip_writes_skip_json` |
| T-05-09 | SHA-256 over BTreeMap-ordered inputs + source bytes + TOML section | unit tests `staleness_key_*` (4 variants) |

## Known Stubs

- `tool = "zig"` dispatch deferred to Plan 04: writes skip.json and returns `SimResult::Skipped("zig tool not yet implemented")`
- IR model_path resolution (`build_input_json`) relies on `deal_ir_json` FFI which requires a compiled DEAL project IR. Integration tests bypass IR resolution by pre-populating `input.json` directly.

## Self-Check: PASSED

- cli/src/simulate.rs: FOUND (1066 lines)
- cli/src/sims_protocol.rs: FOUND (316 lines)
- cli/src/lib.rs: FOUND (44 lines)
- cli/tests/simulate_integration.rs: FOUND (330 lines)
- deal-sim/src/deal_sim/cli.py: FOUND (205 lines)
- Commit ccd4c36: FOUND (Task 1)
- Commit 6bf6343: FOUND (Task 2)
- Commit dbf6810 (deal-sim): FOUND (Python SDK fix)
- cargo test -p deal --test simulate_integration: 3/3 PASSED
