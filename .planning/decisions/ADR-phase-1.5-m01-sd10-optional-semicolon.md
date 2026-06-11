---
phase: 01.5
adr_id: ADR-phase-1.5-m01-sd10-optional-semicolon
status: accepted
date: 2026-05-20
relates_to:
  - REQ-phase-1.5-2-malformed-pinning
  - REQ-phase-1.5-4-parser-strictness
  - SD-10
---

# ADR-phase-1.5-m01: `m01_missing_semicolon.deal` is accepted under SD-10's optional-semicolon rule

## What the malformed file demonstrates

`tests/malformed/m01_missing_semicolon.deal` contains:

```deal
package foo;
part def P { attribute a : T attribute b : T; }
```

The file's filename and presence in `tests/malformed/` reflect the
fixture-author's INITIAL hypothesis that the missing `;` between the two
`attribute` declarations would be reported as a syntax error. Empirical
discovery during Phase 1.5 Plan 03 Task 1 (running `recovery.corpus` with
a temporary PIN-DISCOVERY print) confirms that the parser actually
accepts this file cleanly: zero diagnostics emitted, the AST contains
both attribute declarations.

## Why the grammar permits it

Decision **SD-10** ("optional-semicolon" rule, locked in Phase 0 and
documented in `spec/DESIGN-DECISIONS.md`) defines positions where a `;`
is grammatically OPTIONAL inside a definition body. The transition from
one `attribute` declaration to the next inside a `part def` body is one
of those positions: the `attribute` keyword starts a new declaration
unambiguously regardless of whether the prior `attribute` is terminated
with `;`.

## Why no diagnostic is emitted

A clean parse is the CORRECT behaviour under SD-10. Emitting a
diagnostic would either:

  1. Violate SD-10 directly (the grammar says this construct is legal),
     turning a valid program into an error; or
  2. Require adding a "stylistic warning" lane (W05** range) that
     Phase 1.5 does NOT introduce. Phase 1.5's strictness work covers
     hard syntax errors only; stylistic warnings are deferred to a
     future iteration (Phase 4 ecosystem timing or earlier).

The fixture's name `m01_missing_semicolon.deal` is therefore a
historical artefact, not a description of a syntax error. The
hand-curated corpus author placed it among the malformed files before
SD-10's optional-semicolon rule was finalised; SD-10 made the file
syntactically valid retroactively.

## What Phase 2's semantic analyser should catch

Nothing additional. The file as written is syntactically valid AND
semantically well-formed (both attributes have valid types). If a
future revision of the fixture changes its content to exercise a
specific SD-10 ambiguity (e.g. two consecutive `attribute`s whose
preceding token could ambiguously end either declaration), Phase 2's
analyser should report the ambiguity — but the CURRENT fixture content
has no such ambiguity.

## Disposition

`tests/unit/recovery_corpus.zig` pins this file as
`expected_code = null` with `adr_basename =
"ADR-phase-1.5-m01-sd10-optional-semicolon.md"`. The pin enforcement
loop asserts:

  1. Zero diagnostics are emitted on parse.
  2. This ADR file exists (so the acceptance is documented, not
     accidental).

## Future work

If a future Phase 1.5+ iteration adds a "stylistic warning" lane (e.g.
`W0501 — missing optional semicolon hints at human-readability lint`),
this pin should be updated to `expected_code = Codes.w_missing_optional_semicolon`
and this ADR superseded with a follow-up ADR explaining the policy
change. Until then, the file's acceptance is the documented contract.

## References

- `spec/DESIGN-DECISIONS.md` SD-10 — optional-semicolon rule.
- `.planning/phases/01-zig-compiler-core/01-VALIDATION.md` Validation
  Audit 2026-05-20 — identifies m01 as one of 13 zero-diagnostic
  files; this ADR closes the m01-specific item.
- `.planning/phases/01.5-parser-strictness-test-gap-resolution-inserted/01.5-CONTEXT.md`
  §decisions — locks per-file pinning for m01..m20 and identifies m01 as
  the canonical SD-10 acceptance case.
