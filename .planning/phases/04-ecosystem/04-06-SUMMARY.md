---
phase: 04-ecosystem
plan: 06
subsystem: codegen
tags: [reqif, rust, xml, xsd, zip, quick-xml, golden-tests, tdd]

# Dependency graph
requires:
  - phase: 04-01
    provides: OMG ReqIF 1.2 XSD bundle acquired in spec/references/omg-reqif/ + golden fixture skeletons
  - phase: 04-04
    provides: IR JSON transport (deal_ir_json) used by codegen backends

provides:
  - cli/src/reqif_schema.rs — offline ReqIF XSD bundle load + SHA256 verify + structural validation
  - cli/src/reqif.rs — IR v0 JSON → ReqIF 1.2 XML → .reqifz emitter (mirrors sysml_v2.rs)
  - cli/src/main.rs BuildTarget::Reqif + run_build_reqif wired into CLI dispatch
  - cli/tests/golden_reqif.rs — golden-fixture diff + valid-zip tests
  - tests/golden/reqif/*.expected.reqif — real expected ReqIF XML (not placeholders)
  - .planning/phases/04-ecosystem/04-VERIFICATION.md — D-58 import smoke record (human-approved)

affects: [04-08-reqif-docs, 04-09-gate, future-doors-integration]

# Tech tracking
tech-stack:
  added:
    - quick-xml 0.40 (Rust XML reader/writer for ReqIF emission + structural validation)
    - zip 2 (Rust zip writer for .reqifz archive production)
  patterns:
    - reqif_schema.rs mirrors schema_registry.rs (SHA256-verify-at-load pattern, structural validator)
    - reqif.rs mirrors sysml_v2.rs (IR-node walk, D-18 deterministic sort-by-id, emit_from_bytes)
    - Fixed CREATION-TIME constant for byte-deterministic archive output (D-18)
    - D-59 filter: only requirement_def/need_def exported; part_def/port_def silently skipped
    - TDD RED→GREEN: failing golden test committed before implementation

key-files:
  created:
    - cli/src/reqif_schema.rs
    - cli/src/reqif.rs
    - cli/tests/golden_reqif.rs
    - tests/golden/reqif/01-requirement-def.expected.reqif
    - tests/golden/reqif/02-trace-relation.expected.reqif
    - spec/references/omg-reqif/SHA256SUMS
    - .planning/phases/04-ecosystem/04-VERIFICATION.md
  modified:
    - cli/Cargo.toml (quick-xml 0.40, zip 2)
    - cli/src/main.rs (BuildTarget::Reqif, run_build_reqif, mod reqif/reqif_schema)
    - cli/src/lib.rs (expose reqif + reqif_schema modules for integration tests)

key-decisions:
  - "ReqIF ID derivation: DEAL_<qualified_path> with dots replaced by underscores (dots disallowed in XML IDs); e.g. DEAL_requirements_needs_NEED_BAT_001"
  - "Structural validation (well-formed XML + required elements + OMG namespace) is the HARD gate (no mature offline Rust XSD 1.0 validator); Python reqif library is the soft import smoke per RESEARCH Pitfall 3"
  - "D-58 fallback clause accepted: Python reqif v0.0.48 round-trip passes as the import smoke; DOORS trial unavailable on build machine"
  - "0 SPEC-RELATIONS in showcase export is correct D-59 behavior: top-level @trace annotations on part def are structural, not requirement-body trace edges; correct fixture for future plans requires @trace inside requirement bodies"
  - "CREATION-TIME in REQ-IF-HEADER uses a fixed constant (not runtime now()) to preserve D-18 byte-determinism of golden fixtures across CI runs"

patterns-established:
  - "reqif_schema.rs: mirrors schema_registry.rs — verify_sha256() private helper, assert_within path guard, locate_reqif_schemas_dir() with env override + walk-up search"
  - "reqif.rs: mirrors sysml_v2.rs — IrNode/IrEdge structs, sort_by_key(|n| n.id) for D-18, emit_from_bytes() validates XML before zipping"
  - "TDD golden flow: commit failing tests (RED), then fill fixtures + implement (GREEN); fixture holds unwrapped XML for readable diffs"

requirements-completed:
  - REQ-phase-4-2-reqif-codegen

# Metrics
duration: ~2h
completed: 2026-06-06
---

# Phase 04 Plan 06: ReqIF Codegen Backend Summary

**IR → ReqIF 1.2 XML emitter with SHA256-verified XSD structural gate, .reqifz packaging, byte-deterministic golden fixtures, and Python reqif v0.0.48 import smoke (D-58 human-approved fallback)**

## Performance

- **Duration:** ~2h
- **Started:** 2026-06-06
- **Completed:** 2026-06-06
- **Tasks:** 3 (Tasks 1, 2 TDD RED+GREEN, 3 with human-verify checkpoint)
- **Files modified:** 14 (7 created, 7 modified)

## Accomplishments

- Implemented `reqif_schema.rs` mirroring `schema_registry.rs`: SHA256-verified XSD bundle load at startup (T-4-12 tamper detection), structural validation via quick-xml (well-formedness, OMG namespace, required elements, non-empty IDENTIFIER attributes)
- Implemented `reqif.rs` emitter via TDD RED→GREEN cycle: IR v0 JSON → ReqIF 1.2 XML → `.reqifz` archive; requirement_def/need_def → SPEC-OBJECT, @trace edges → SPEC-RELATION, D-59 filter excludes part_def/port_def; byte-deterministic (fixed timestamp, sort-by-id)
- Wired `deal build --target reqif --validate` into the CLI; showcase export produces `build/reqif/model.reqifz` (13 requirements, 0 relations); D-58 import smoke passed via Python reqif v0.0.48 (human-approved)

## Task Commits

Each task was committed atomically:

1. **Task 1: reqif_schema.rs — offline XSD bundle load + SHA256 verify + structural validation** - `2478975` (feat)
2. **Task 2 RED: golden_reqif.rs — failing TDD tests** - `f92f489` (test)
3. **Task 2 GREEN: reqif.rs emitter + main.rs/lib.rs wiring + golden fixtures** - `675f32d` (feat)
4. **Task 3: 04-VERIFICATION.md — D-58 import smoke recorded** - `c9f06d3` (docs)

_TDD task 2 has RED + GREEN commits per TDD gate protocol._

## Files Created/Modified

- `cli/src/reqif_schema.rs` — SHA256-verified XSD bundle load + structural validator (mirrors schema_registry.rs)
- `cli/src/reqif.rs` — IR JSON → ReqIF 1.2 XML → .reqifz emitter (mirrors sysml_v2.rs)
- `cli/tests/golden_reqif.rs` — golden-fixture byte-exact diff + valid-zip test
- `tests/golden/reqif/01-requirement-def.expected.reqif` — real expected ReqIF XML (was Plan-01 placeholder)
- `tests/golden/reqif/02-trace-relation.expected.reqif` — real expected ReqIF XML (was Plan-01 placeholder)
- `spec/references/omg-reqif/SHA256SUMS` — SHA256 pins for reqif.xsd + driver.xsd
- `cli/Cargo.toml` — added quick-xml 0.40, zip 2
- `cli/src/main.rs` — BuildTarget::Reqif variant, run_build_reqif function, mod reqif/reqif_schema
- `cli/src/lib.rs` — pub mod reqif, pub mod reqif_schema (for integration tests)
- `.planning/phases/04-ecosystem/04-VERIFICATION.md` — D-58 import smoke record

## Decisions Made

- **ReqIF ID derivation locked:** `DEAL_<qualified_path_dots_to_underscores>` (e.g. `DEAL_requirements_needs_NEED_BAT_001`). XML IDs cannot contain dots; this derivation is deterministic and reversible.
- **Structural validation as hard gate:** No mature offline Rust XSD 1.0 validator exists (RESEARCH Pitfall 3). The Rust structural validator (well-formed XML + OMG namespace + required elements + non-empty IDENTIFIERs) is the automated hard gate per D-58/D-60. Python `reqif` library is the soft smoke.
- **D-58 fallback accepted (human-approved):** IBM DOORS/DOORS Next trial unavailable on build machine. Python reqif v0.0.48 round-trip passes without exception; stdlib `xml.etree.ElementTree` cross-validates 13 SPEC-OBJECT elements. Human confirmed this is acceptable per the D-58 fallback clause.
- **0 SPEC-RELATIONS is correct (human-confirmed):** The showcase uses `@trace` annotations on `part def` top-level bodies; D-59 and D-25 correctly exclude these (structural annotations, not requirement-body trace edges). Adding `@trace` inside a `requirement_def` body would produce SPEC-RELATION output. No emitter bug.
- **Fixed CREATION-TIME for D-18:** The REQ-IF-HEADER's `CREATION-TIME` field uses a compile-time constant (`2024-01-01T00:00:00Z`) rather than `chrono::Utc::now()`, ensuring two emitter runs on identical input produce byte-identical `.reqif` XML.

## Deviations from Plan

None - plan executed exactly as written. The TDD RED→GREEN sequence was followed per plan task 2 instructions.

The D-58 checkpoint paused for human verification as specified (`type="checkpoint:human-verify" gate="blocking"`). Human response: "Accept fallback" — approved.

## Issues Encountered

- `reqif v0.0.48 spec_objects_lookup` shows 0 in the library's internal index: this is a known quirk of that library version's internal indexing; it does not affect parse success. Stdlib ET cross-validation confirmed all 13 SPEC-OBJECT elements are present and well-formed.
- The `req_if_tool_id` field set by the emitter (`deal`) is not surfaced by the v0.0.48 API — library API detail, not an emitter defect.

## TDD Gate Compliance

- RED gate: `f92f489` — `test(04-06): add failing golden_reqif tests — TDD RED gate`
- GREEN gate: `675f32d` — `feat(04-06): implement reqif.rs emitter (IR JSON -> ReqIF 1.2 XML -> .reqifz)`
- REFACTOR gate: not needed (code clean from initial implementation)

Both mandatory gates (RED then GREEN) are present in correct order.

## User Setup Required

None - no external service configuration required. The D-58 import smoke uses the Python `reqif` library (already installed as `v0.0.48`); no user-side configuration needed for `deal build --target reqif`.

## Next Phase Readiness

- `deal build --target reqif` is fully functional; docs site (Plan 07) and gate (Plan 09) can reference this capability
- Golden fixtures are pinned with real expected XML; any emitter regression will be caught by `cargo test -p deal --test golden_reqif`
- 0-relations scenario is understood; a future fixture with `@trace` inside a `requirement_def` body can exercise SPEC-RELATION output when needed
- Threat T-4-12 (XSD tamper), T-4-13 (XML injection via quick-xml escaping), T-4-14 (D-59 over-export) all mitigated and verified via tests

---
*Phase: 04-ecosystem*
*Completed: 2026-06-06*
