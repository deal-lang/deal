# DEAL Synthesis Summary

Entry point for `gsd-roadmapper`. Per-type intel files live alongside this document in `/Users/dunnock/projects/deal-lang/deal/.planning/intel/`. Conflicts (auto-resolved + open) are recorded in `/Users/dunnock/projects/deal-lang/deal/.planning/INGEST-CONFLICTS.md`.

---

## Doc counts by type

| Type    | Count | Sources |
|---------|-------|---------|
| ADR     | 1     | `spec/grammar/DESIGN-DECISIONS.md` (consolidated, 65 decisions) |
| SPEC    | 3     | `spec/grammar/lexical.ebnf`, `spec/grammar/deal.ebnf`, `spec/grammar/dealx.ebnf` |
| PRD     | 1     | `DEAL-LANG-ROADMAP.html` (6 phases) |
| DOC     | 1     | `spec/grammar/README.md` |
| **Total** | **6** | All classifications consumed; all `manifest_override: true`, all `confidence: high` |

---

## Decisions locked

- 64 LOCKED + 1 CAPTURED (SD-6 captured, not locked) across 8 categories
- Source: `DESIGN-DECISIONS.md` (single consolidated ADR, precedence 0)
- Categories: Foundational Architecture (FA-1..FA-5), Naming (NC-1), Language Model Alignment (LM-1..LM-3), File Structure (FS-1..FS-4), Project Structure (PS-1..PS-10), Syntax–Definitions (SD-1..SD-20), Syntax–Compositions (CS-1..CS-16), Simulation Integration (SIM-1..SIM-5)
- Detail: `decisions.md`

## Requirements extracted

- ~30 entries across 7 phases (Phase 0 through Phase 6) plus deferred decision-points
- Source: `DEAL-LANG-ROADMAP.html` (PRD, precedence 2)
- Phase-level requirements: `REQ-phase-0-foundation` (COMPLETE), `REQ-phase-1-foundation`, `REQ-phase-2-prove-pipeline`, `REQ-phase-3-editor-intelligence`, `REQ-phase-4-ecosystem`, `REQ-phase-5-simulation`, `REQ-phase-6-application`
- Milestone-level: 1.1–1.5, 2.1–2.5 (+2.2a/b, 2.3a/b), 3.1–3.4, 4.1–4.4, 5.1–5.3, 6.1–6.4
- Phase-gate requirements for each phase
- Detail: `requirements.md`

## Constraints

- 3 `protocol`-type constraints (one per W3C EBNF grammar file)
  - `CONSTRAINT-lexical-grammar` — `lexical.ebnf` (L1, 758 lines, ~125 token types) — implements LM-1, LM-2, LM-3, SD-3, SD-5, SD-10, SD-11, SD-12, CS-2, NC-1
  - `CONSTRAINT-definition-grammar` — `deal.ebnf` (L2/L3, 1679 lines, 87 productions) — implements FS-1, FS-2, FS-3, PS-2, PS-3, PS-4, PS-6, SD-1..SD-20, CS-5, CS-15
  - `CONSTRAINT-composition-grammar` — `dealx.ebnf` (L4, 897 lines, 43 productions) — implements CS-1..CS-16, SD-17, SD-19, FS-1, FS-2, FS-3, PS-2, PS-4
- All three grammars are at version `0.1.0-draft` and use W3C EBNF (XML 1.0 Fifth Edition §6)
- Layering: `lexical.ebnf` → `deal.ebnf` → `dealx.ebnf` (unidirectional; `.deal` never references `.dealx`)
- Detail: `constraints.md`

## Context topics

- 2 topics from `grammar/README.md` (DOC, precedence 3): Grammar directory overview; Document-set cross-references and provenance
- Detail: `context.md`

---

## Conflicts

- **Blockers:** 0
- **Warnings (competing variants):** 0
- **Info (auto-resolved + transparency notes):** 8

Open items the roadmapper should be aware of (all logged in `INGEST-CONFLICTS.md`):

1. `DESIGN-DECISIONS.md` §Implementation Staging explicitly cedes ordering authority to `DEAL-LANG-ROADMAP.html` — roadmapper should treat the PRD's phase shape as authoritative even though ADR precedence would normally win.
2. README's "Stage 1" plan is high-level; PRD's Phase 1–3 milestones are the authoritative breakdown.
3. README states `lexical.ebnf` is 370 lines; current file is 758 lines (re-ingest after user completed the grammar). Other README statistics still consistent.
4. SD-6 is CAPTURED, not LOCKED — relationship-category enumeration is partial; full enumeration is in the deferred list.
5. `DESIGN-DECISIONS.md` lives in a `tmp-references` directory — synthesis treats it as authoritative per manifest override.

Full detail (8 INFO entries, 0 blockers, 0 warnings): `/Users/dunnock/projects/deal-lang/deal/.planning/INGEST-CONFLICTS.md`

---

## Pointers (absolute paths)

- Decisions: `/Users/dunnock/projects/deal-lang/deal/.planning/intel/decisions.md`
- Requirements: `/Users/dunnock/projects/deal-lang/deal/.planning/intel/requirements.md`
- Constraints: `/Users/dunnock/projects/deal-lang/deal/.planning/intel/constraints.md`
- Context: `/Users/dunnock/projects/deal-lang/deal/.planning/intel/context.md`
- Conflicts report: `/Users/dunnock/projects/deal-lang/deal/.planning/INGEST-CONFLICTS.md`
- Classifications: `/Users/dunnock/projects/deal-lang/deal/.planning/intel/classifications/`

**STATUS: READY — safe to route to `gsd-roadmapper`.**
