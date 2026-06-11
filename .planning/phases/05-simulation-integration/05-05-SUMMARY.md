---
phase: 05-simulation-integration
plan: "05"
subsystem: evidence-capture-baseline
tags:
  - rust
  - evidence
  - sha256
  - btreemap
  - d18
  - d81
  - d82
dependency_graph:
  requires:
    - 05-01  # evidence.rs stub, clap surface, lib.rs module tree
    - 05-03  # simulate orchestrator writes .deal/evidence/<sim>/output.json + metadata.json
  provides:
    - evidence-capture          # run_evidence_capture_in() snapshots working cache
    - evidence-baseline         # run_evidence_baseline_in() frozen snapshot + manifest
    - evidence-manifest-schema  # D-82 manifest with content_hash, verdict, tier, tool, tool_version
    - evidence-byte-stable      # D-18 BTreeMap alphabetical keys, byte-identical twice test
  affects:
    - 05-06  # verify engine reads evidence/baselines/<tag>/manifest.json
    - 05-07  # smoke test exercises baseline manifest end-to-end
tech_stack:
  added: []
  patterns:
    - sha2::Sha256 over output.json bytes for per-sim content_hash (D-82/T-05-12)
    - BTreeMap alphabetical key order for byte-stable manifest.json (D-18/T-05-14)
    - write_file_atomic (tmp+rename) for crash-safe output writes
    - Provisional DEFAULT_VERDICT="PARTIAL" from metadata.json until Plan 06 backfills (D-82)
    - D-81 clean split: .deal/evidence/ gitignored, evidence/baselines/ tracked
key_files:
  created:
    - cli/tests/evidence_baseline.rs  # 6 GREEN integration tests
  modified:
    - cli/src/evidence.rs             # full implementation (554 lines)
    - cli/src/lib.rs                  # added pub mod evidence
decisions:
  - "Verdict sourced from metadata.json `verdict` field; defaults to PARTIAL when absent or invalid — baseline never calls verify engine (Plan 06 dependency avoided per plan spec)"
  - "Capture writes to .deal/captured/ (working-cache side of D-81); baseline writes to evidence/baselines/<tag>/ (tracked side) — two separate output dirs"
  - "content_hash is SHA-256 over raw output.json bytes (not pretty-printed re-encoding) to ensure hash stability across serialization variants"
  - "top-level manifest keys ordered alphabetically via BTreeMap: sims, tag, timestamp, v — byte-identical-twice test enforces it"
metrics:
  duration: "~5 minutes"
  completed: "2026-06-08T21:00:59Z"
  tasks: 2
  files_changed: 3
---

# Phase 5 Plan 05: Evidence Capture and Baseline Summary

`deal evidence capture` snapshots `.deal/evidence/` working runs into `.deal/captured/`; `deal evidence baseline <tag>` writes a frozen git-tracked snapshot plus a D-82 BTreeMap-keyed manifest with SHA-256 content hashes and PARTIAL/PASS/FAIL verdicts sourced from metadata.json.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | deal evidence capture — snapshot working runs | 8d94036 | cli/src/evidence.rs, cli/src/lib.rs |
| 2 | deal evidence baseline tag — frozen snapshot + manifest | e754f2d | cli/src/evidence.rs, cli/tests/evidence_baseline.rs |

## What Was Built

### cli/src/evidence.rs — Evidence Capture and Baseline (554 lines)

**`run_evidence_capture_in(project_root)`**
- Scans `.deal/evidence/` via `std::fs::read_dir`, iterates immediate subdirs (one per sim)
- Copies `output.json`, `metadata.json`, `skip.json` (if present) into `.deal/captured/<sim>/`
- Writes BTreeMap-keyed aggregate `index.json` to `.deal/captured/` (D-18)
- Uses `write_file_atomic` (tmp+rename) for all writes
- Succeeds gracefully when no `.deal/evidence/` exists yet

**`run_evidence_baseline_in(project_root, tag)`**
- Reads output.json bytes from `.deal/evidence/<sim>/` for each sim that has run
- Computes SHA-256 content hash via `sha2::Sha256` (D-82/T-05-12 tamper detection)
- Reads `tool`, `tool_version`, `reproducibility_tier`, `verdict` from `metadata.json`
  - Verdict defaults to `"PARTIAL"` when metadata absent or field invalid (D-82)
  - Does NOT call `crate::verify::evaluate()` — baseline only transcribes recorded verdict
- Copies frozen `output.json` into `evidence/baselines/<tag>/<sim>/` (tracked, repo root)
- Writes `evidence/baselines/<tag>/manifest.json` via BTreeMap (D-18 alphabetical keys):
  - Per-sim: `content_hash`, `reproducibility_tier`, `tool`, `tool_version`, `verdict`
  - Top-level: `sims`, `tag`, `timestamp`, `v:1`

**6 unit tests** (all GREEN, run as lib + bin targets):
`capture_no_evidence_dir_succeeds`, `capture_copies_artifacts`, `capture_index_uses_btreemap`,
`read_metadata_defaults_when_absent`, `read_metadata_parses_fields`,
`read_metadata_invalid_verdict_defaults_to_partial`

### cli/tests/evidence_baseline.rs — Integration Tests (292 lines, 6 tests, all GREEN)

1. `test_baseline_writes_manifest_with_content_hash_and_verdict` — manifest.json exists, SHA-256 hash is 64 hex chars, verdict in {PASS,FAIL,PARTIAL}, all D-82 fields present
2. `test_baseline_writes_frozen_output_snapshot` — output.json copied to baseline dir
3. `test_baseline_byte_identical_on_repeated_run` — two runs over identical evidence produce byte-identical manifest.json (D-18 byte-stability)
4. `test_manifest_keys_are_alphabetical` — `alpha_sim` before `zebra_sim` in JSON text (BTreeMap, D-18)
5. `test_baseline_errors_when_no_evidence_cache` — returns `CliError::User` with descriptive message
6. `test_baseline_verdict_defaults_to_partial_when_metadata_absent` — PARTIAL default confirmed

## Acceptance Criteria Verified

- `cargo test -p deal --test evidence_baseline`: 6/6 PASSED
- `cargo test -p deal evidence`: 6/6 unit tests PASSED (lib + bin targets)
- `git check-ignore .deal/evidence/x` succeeds (ignored); `git check-ignore evidence/baselines/x` fails (tracked) — D-81 split holds
- manifest.json keys alphabetical; byte-identical-twice test passes (D-18)
- verdict field is valid enum member (PASS|FAIL|PARTIAL), sourced from metadata.json (D-82)
- No calls to `crate::verify::evaluate()` in baseline — no Wave 4 dependency

## Security Mitigations Applied

| Threat ID | Mitigation | Verified |
|-----------|------------|---------|
| T-05-12 | SHA-256 content_hash per sim in git-tracked manifest | test_baseline_writes_manifest_with_content_hash_and_verdict |
| T-05-13 | .deal/ gitignored; evidence/baselines/ tracked — git check-ignore confirmed | D-81 split check |
| T-05-14 | BTreeMap alphabetical keys; byte-identical-twice test | test_baseline_byte_identical_on_repeated_run |

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

- `verdict` field in metadata.json is written as `"PARTIAL"` by the simulate runner (Plan 03) until Plan 06's verify engine backfills real PASS/FAIL/PARTIAL values. This is intentional per Plan 05 spec ("provisional default of PARTIAL"). The authoritative end-to-end verdict-correctness assertion lives in Plan 07's smoke test.

## Self-Check: PASSED

- cli/src/evidence.rs: FOUND (554 lines)
- cli/src/lib.rs: FOUND (44 lines, pub mod evidence added)
- cli/tests/evidence_baseline.rs: FOUND (292 lines)
- Commit 8d94036: FOUND (Task 1 — evidence capture)
- Commit e754f2d: FOUND (Task 2 — evidence baseline + tests)
- `cargo test -p deal --test evidence_baseline`: 6/6 PASSED
- `git check-ignore .deal/evidence/x`: DEAL_IGNORED
- `git check-ignore evidence/baselines/x`: BASELINE_TRACKED
