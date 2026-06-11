# Phase 2 Viewer-Import Smoke Test Record

**Plan:** 02-06
**Decision:** D-36 — SysML v2 viewer-import smoke required for Phase 2 exit
**Artifact under test:** `spec/examples/showcase/build/sysml-v2/showcase.sysml-v2.json` (produced by `deal build --target sysml-v2 tests/showcase/ --validate`)

## Status

**DOCUMENTED BLOCKER** — D-36 binary-outcome rule (success OR documented blocker) is satisfied; Phase 2 exits GREEN.

## Recorded outcome

**Viewer attempted:** Eclipse SysON 2025.x (D-36 priority 1, preferred) — https://syson.eclipse.dev
**Date:** 2026-05-22
**Approver:** Project owner (you@example.com)
**Channel:** Interactive `/gsd-execute-phase 2` session — viewer screenshot reviewed in chat

### What worked

- `deal build --target sysml-v2 tests/showcase/ --validate` produces `showcase.sysml-v2.json` (151 KB, 301 elements rooted at a synthetic `Workspace` Package containing every showcase declaration).
- Offline schema validation passes (`--validate` exits 0) against the bundled OMG SysML.json + KerML.json at `tests/schemas/`.
- SysON accepts the file at upload time — it is listed in the project Explorer alongside the textual SysML model.
- All 8 hand-authored golden fixtures in `tests/golden/sysml-v2/` pass both byte-exact (`golden_sysml_v2.rs`) and independent schema-validity (`golden_fixture_schema_validity.rs`) gates.

### Blocker

SysON's navigable tree (Package → view) operates on the **textual** SysML v2 language (`.sysml` files created via `Create a new Model → SysMLv2`). The OMG SysML v2 **interchange JSON** that this phase emits — the canonical JSON serialization used by the OMG API and Services Specification — is treated by SysON as an **opaque file attachment**: it appears in the project Explorer but is not lowered into a navigable model. The "Details" panel shows empty Core Properties; the file node cannot be expanded.

This is a tooling-ecosystem impedance mismatch, not a defect in DEAL's emitter:
- The DEAL JSON shape matches what `tests/schemas/SysML.json` (the OMG-supplied JSON Schema) expects.
- The same caveat applies to most current SysML v2 viewers — they are built around textual SysML v2 with the JSON used between API services rather than as a direct human-facing import.

### Mitigation / Phase 3 follow-up

**Carried forward to Phase 3:** add a `deal build --target sysml-v2-text` emitter that produces `.sysml` files (textual SysML v2 language) SysON and other viewers can open as a real, navigable model. The OMG JSON interchange format remains the canonical authoring target — the textual emitter is an additional viewer-compatible output, not a replacement.

Tracking:
- Added to STATE.md `Deferred Items` table → "Textual SysML v2 emitter (`deal build --target sysml-v2-text`)" → deferred at Phase 2 viewer-smoke checkpoint → schedule for Phase 3.
- Phase 3 planning (`/gsd:spec-phase 3` or equivalent) should add a REQ for this emitter alongside REQ-phase-3-* (LSP / VS Code) work — the LSP and textual emitter share the SysML v2 grammar surface and may benefit from a single grammar/parser asset.

## Viewer priority order followed (D-36)

| Priority | Viewer | Attempted? | Outcome |
|----------|--------|------------|---------|
| 1 | Eclipse SysON 2025.x | yes | Accepts JSON; not navigable (textual-only UI) — documented above |
| 2 | OpenMBEE SysML v2 Pilot Implementation | no | Not attempted — same textual/JSON impedance expected per ecosystem survey; revisit after Phase 3 textual emitter lands |
| 3 | IncQuery SysML v2 toolset (commercial trial) | no | Not attempted — commercial trial; deferred to Phase 3 textual-emitter follow-up |
| 4 | NoMagic / Cameo SysML v2 plugin | no | Not attempted — same |
| 5 | Generic JSON-Schema fallback | implicit | `deal build --validate` already runs the bundled SysML.json validator; exit 0 — proves the JSON is OMG-schema-valid even though the human-facing viewer cannot render it |

## Effect on Phase 2 SPEC criteria

- §ROADMAP Phase 2 Success Criterion #6 (recorded viewer/import smoke or documented blocker): **satisfied** — this file is the record.
- §SPEC.md criterion #13 (02-VERIFICATION.md records the specific SysML v2 viewer used and successful import outcome or documented blocker): **satisfied** — see matrix update in `02-VERIFICATION.md`.
