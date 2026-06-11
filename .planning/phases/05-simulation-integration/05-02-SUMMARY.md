---
phase: 05-simulation-integration
plan: "02"
subsystem: deal-sim-python-sdk
tags: [python, sdk, tdd, deal-sim, simulation, stdlib-only, d-78, d-18, d-79]
dependency_graph:
  requires:
    - 05-01 (spec/sims/v0/ schemas, Wave 0 test stubs)
  provides:
    - deal_sim Python SDK — installable via pip install -e . (D-77)
    - DealSimulation base class with inputs/outputs/run/cli()
    - stdlib-only input/output validation (D-78)
    - metadata.py emitting spec/sims/v0/ metadata envelope
    - cli.py wrapping run() output in spec/sims/v0/ protocol envelope
    - MATLAB subprocess adapter with graceful-skip (D-72)
    - Generic subprocess adapter (D-79)
    - pyproject.toml PEP 517 wheel build
    - 31 green unit tests (test_simulation.py + test_validation.py)
  affects:
    - deal-sim sibling repo (initialized as git repo, not tracked in deal/)
tech_stack:
  added:
    - Python 3.10+ package (deal_sim v0.1.0)
    - setuptools.build_meta PEP 517 build backend
    - python -m build wheel toolchain
  patterns:
    - TDD RED/GREEN cycle — test stubs first, then implementation
    - spec/sims/v0/ protocol envelope in output.json (deal_sim_protocol/exit_code/outputs/v)
    - sort_keys=True on all JSON artifacts (D-18 byte-stable evidence)
    - Graceful-skip adapter pattern (D-72, mirrors D-58 DOORS precedent)
key_files:
  created:
    - deal-sim/src/deal_sim/__init__.py (re-exports DealSimulation)
    - deal-sim/src/deal_sim/simulation.py (DealSimulation base class)
    - deal-sim/src/deal_sim/validation.py (stdlib-only type checker D-78)
    - deal-sim/src/deal_sim/metadata.py (metadata envelope producer)
    - deal-sim/src/deal_sim/cli.py (CLI runner with protocol envelope)
    - deal-sim/src/deal_sim/adapters/__init__.py
    - deal-sim/src/deal_sim/adapters/matlab.py (D-72 subprocess adapter)
    - deal-sim/src/deal_sim/adapters/generic.py (D-79 generic subprocess)
    - deal-sim/pyproject.toml (PEP 517 build, stdlib-only deps)
    - deal-sim/.gitignore
  modified:
    - deal-sim/README.md (architecture docs, deferred adapters section, exit codes)
    - deal-sim/tests/test_simulation.py (RED stubs → GREEN with schema envelope assertions)
    - deal-sim/tests/test_validation.py (RED stubs → GREEN)
decisions:
  - "deal-sim initialized as its own git repo — files outside deal/ git tree cannot be staged in deal/; this matches Plan 01 SUMMARY note that test stubs are not tracked in deal/"
  - "setuptools.build_meta used as PEP 517 backend — setuptools.backends.legacy not available in setuptools 82.0.1"
  - "output.json wraps run() scalars in {value, unit} envelope per spec/sims/v0/output.schema.json — raw scalar dict would fail schema validation"
  - "Test stubs updated from @unittest.skip to real assertions — original stubs tested raw scalar keys; updated tests assert schema envelope shape"
  - "D-18 sort_keys=True applies at 6 sites in cli.py (output.json envelope + metadata.json)"
metrics:
  duration: "~17 minutes"
  completed: "2026-06-08"
  tasks: 2
  files_created: 10
  files_modified: 3
---

# Phase 05 Plan 02: deal-sim Python SDK Summary

**One-liner:** `deal_sim` Python SDK with `DealSimulation` base class, stdlib-only type validation (D-78), `cli()` JSON-I/O runner emitting spec/sims/v0 protocol envelopes, MATLAB+generic subprocess adapters (D-79), PEP 517 wheel, and 31 green TDD tests against the battery_thermal.py oracle.

## Tasks Completed

| Task | Name | Commit (deal-sim) | Key Files |
|------|------|-------------------|-----------|
| RED | Test stubs — DealSimulation + validation | a912e23 | tests/test_simulation.py, tests/test_validation.py |
| 1 GREEN | DealSimulation base class + validation + metadata | 4178ce1 | src/deal_sim/{__init__,simulation,validation,metadata,cli}.py + adapters/ |
| 2 GREEN | cli() protocol envelope + pyproject + oracle | 970c4ce | src/deal_sim/cli.py, pyproject.toml, README.md |

## Acceptance Criteria Verified

- `pip install -e deal-sim` succeeds offline (no network) — confirmed
- `python -m unittest discover tests` exits 0 — 31 tests OK
- `from deal_sim import DealSimulation` works after install
- Oracle `battery_thermal.py` runs UNMODIFIED: heatGenerated=3125.0, coolantOutTemp=21.68
- `output.json` schema valid: `deal_sim_protocol="v0"`, `exit_code=0`, `outputs={...}`, `v=1`
- `grep -c sort_keys deal-sim/src/deal_sim/cli.py` = 6 (D-18)
- `python -m build --wheel` produces `deal_sim-0.1.0-py3-none-any.whl`
- README.md marks STK + FMI/FMU as deferred to Phase 6 (D-79)
- No numpy/scipy/jsonschema imports in deal_sim core (D-78 stdlib-only)
- metadata.json contains deal_sim_protocol, tool, tool_version, duration_s, reproducibility_tier

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] setuptools.backends.legacy build backend not available**
- **Found during:** Task 2 — pyproject.toml creation
- **Issue:** RESEARCH.md recommended `setuptools.backends.legacy:build` but this module does not exist in setuptools 82.0.1. The correct backend is `setuptools.build_meta`.
- **Fix:** Changed `build-backend = "setuptools.backends.legacy:build"` to `build-backend = "setuptools.build_meta"` in pyproject.toml.
- **Files modified:** deal-sim/pyproject.toml
- **Commit:** 970c4ce

**2. [Rule 1 - Bug] Tests asserted raw scalar keys; schema requires envelope**
- **Found during:** Task 2 — adding protocol envelope to cli.py
- **Issue:** Task 1 tests checked `out["heatGenerated"]` directly. Task 2 wraps output in `{"deal_sim_protocol": "v0", "exit_code": 0, "outputs": {...}, "v": 1}` — raw key access would fail.
- **Fix:** Updated test_simulation.py tests to assert envelope shape: `out["outputs"]["heatGenerated"]["value"]` instead of `out["heatGenerated"]`. Added `test_output_json_schema_valid` and `test_output_validation_failure_raises_before_write` tests.
- **Files modified:** deal-sim/tests/test_simulation.py
- **Commit:** 970c4ce

### Structural Note (not a deviation)

The deal-sim directory at `/Users/dunnock/projects/deal-lang/deal-sim/` has no git tracking. It is outside the `deal/` git tree and cannot be staged there. Plan 01 SUMMARY documented this: "deal-sim Python tests not tracked in deal/ git — Plan 02 will manage these files within the deal-sim repo directly."

Resolution: Initialized deal-sim as its own git repo for this plan. Commits for both tasks went to deal-sim's git. The deal/ main repo records only metadata (this SUMMARY + STATE.md updates).

## Known Stubs

None — all plan-02 targets are fully implemented.

The following remain deferred by design (D-79, documented in README):

| Stub | File | Target Plan |
|------|------|-------------|
| STK API adapter | deal-sim/src/deal_sim/adapters/stk.py | Phase 6 |
| FMI/FMU co-simulation adapter | deal-sim/src/deal_sim/adapters/fmi.py | Phase 6 |

## Threat Surface

No new network endpoints or trust boundaries introduced. The threat mitigations from the plan's threat model were applied:

- **T-05-03 (Tampering — malformed input.json):** `_validate_inputs` type-checks every field before `run()`. ValueError raised on type mismatch, naming the offending field. cli() exits 2 on validation failure.
- **T-05-04 (Non-deterministic key order):** `json.dump(..., sort_keys=True)` applied to both output.json and metadata.json at 6 sites in cli.py (D-18 byte-stable artifacts).
- **T-05-05 (Third-party supply chain):** `grep -rE "import (numpy|scipy|jsonschema)" deal-sim/src/deal_sim/` returns no matches. All validation uses Python stdlib only (D-78).

## Self-Check: PASSED

- deal-sim/src/deal_sim/__init__.py: FOUND
- deal-sim/src/deal_sim/simulation.py: FOUND
- deal-sim/src/deal_sim/validation.py: FOUND
- deal-sim/src/deal_sim/metadata.py: FOUND
- deal-sim/src/deal_sim/cli.py: FOUND
- deal-sim/src/deal_sim/adapters/matlab.py: FOUND
- deal-sim/src/deal_sim/adapters/generic.py: FOUND
- deal-sim/pyproject.toml: FOUND
- deal-sim/tests/test_simulation.py (31 tests green): CONFIRMED
- deal-sim/tests/test_validation.py (part of 31 tests): CONFIRMED
- Oracle battery_thermal.py runs unmodified: CONFIRMED
- Wheel deal_sim-0.1.0-py3-none-any.whl: FOUND at deal-sim/dist/
- Commit a912e23 (RED stubs): FOUND in deal-sim git
- Commit 4178ce1 (GREEN base class): FOUND in deal-sim git
- Commit 970c4ce (GREEN cli+pyproject): FOUND in deal-sim git
