---
created: 2026-06-08T14:37:43.536Z
title: Wire cross-file dimensional E2500 into deal check CLI
area: planning
phase: 5
source: 04-HUMAN-UAT.md (SC-1 scope decision)
files:
  - cli/src/ (deal_check / deal_parse C ABI entry points)
  - src/sema/ (checkCallDimension, per-file symbol table)
  - sema_dimensional.zig (existing harness — reference, already green)
---

## Problem

Phase 4 SC-1 was human-accepted with a deferred gap: `deal check` does NOT emit
`error[E2500]` for cross-file dimension mismatches end-to-end via the CLI.

Root cause: the CLI `deal check` path processes each file independently through a
single-file C ABI (`deal_parse`) without cross-file symbol seeding. When a model
does `import deal.std.units.{kg, V}` and then `attribute mass : Mass = V(800);`,
the imported units register as `.imported` (not `unit_def`) in the per-file symbol
table, so `checkCallDimension` gracefully skips dimensional verification rather
than emitting E2500.

The full dimensional algebra is already proven: the Zig harness
`sema_dimensional.zig` exercises it with stdlib seeding and all 4 E25xx pins pass.
This is purely the **CLI cross-file wiring** — not new semantic logic.

Deferred from Phase 4 per human ruling in `04-HUMAN-UAT.md` (Test 2 / SC-1):
graceful-skip accepted as Phase 4 scope, E2E CLI wiring → Phase 5.

## Solution

Wire the multi-file `analyzeWithExternalTable` C ABI entry point into the CLI
`deal check` path so imported units are seeded across files (registered as
`unit_def`, not `.imported`) before `checkCallDimension` runs. Then a real
multi-file project with `import deal.std.units.{kg, V}` + a dimension mismatch
should make `deal check` exit non-zero with `error[E2500] dimension mismatch`.

Acceptance: the exact scenario from 04-HUMAN-UAT Test 2 —
`import deal.std.units.{kg, V}` then `attribute mass : Mass = V(800);` —
emits E2500 via the CLI, not just the Zig harness. Add a CLI-level integration
test covering the cross-file path (the Zig harness already covers the algebra).

Surface this when planning Phase 5 (Simulation Integration) — it relates to
`deal check --verify` evaluation correctness.
