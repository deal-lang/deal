---
phase: 01.5
adr_id: ADR-phase-1.5-fresh-worktree-verification
status: accepted
date: 2026-05-21
relates_to:
  - REQ-phase-1.5-gate
  - REQ-phase-1.5-strictness-and-tests
supersedes: none
---

# ADR-phase-1.5-fresh-worktree-verification: Phase exit verification must run from a freshly-created worktree

## Status

accepted (2026-05-21)

## Context

Phase 1.5's original GREEN claim — recorded in
`.planning/phases/01.5-parser-strictness-test-gap-resolution-inserted/01.5-VERIFICATION.md`
at `verified: 2026-05-20T22:22:38Z` and asserted explicitly in line 73 of that
file ("Phase 1.5 umbrella gate exits 0 — PASS") — was based on a `zig build
phase-1.5-gate` run in the developer's main checkout. That checkout contained
an UNTRACKED, never-committed symbolic link `tests/showcase ->
../../spec/examples/showcase` that resolved into a SIBLING git repository at
`/Users/dunnock/projects/deal-lang/spec/`. Neither the symlink nor the spec
repo's content was part of the `deal/` git tree (`git ls-tree HEAD tests/`
returned only `ffi`, `malformed`, `snapshots`, `unit`).

On 2026-05-21 a fresh `git worktree add` of `deal/` HEAD exposed the gap.
9 of the 33 unit tests aborted with `error.FileNotFound` from
`std.Io.Dir.openFile → dirOpenFilePosix → .NOENT => error.FileNotFound`
because every test in the failing set calls
`cwd.readFileAlloc(io, "tests/showcase/...", ...)` and the path did not
resolve in the fresh worktree. Build summary:
`24/33 tests passed (9 failed)`.

The 9 failing tests (each identified by full test name):

  1. `lexer_snapshot.test.lexer.snapshot`
  2. `lexer_snapshot.test.lexer.spans_monotonic`
  3. `parser_deal_snapshot.test.parser_deal.snapshot`
  4. `parser_deal_coverage.test.parser_deal.coverage`
  5. `parser_dealx_snapshot.test.parser_dealx.snapshot`
  6. `parser_dealx_coverage.test.parser_dealx.coverage`
  7. `property_span_containment.test.property.span_containment`
  8. `determinism_parse_twice.test.determinism.parse_twice`
  9. `c_abi_no_leaks.test.c_abi.no_leaks`

All 9 share identical `FileNotFound` stacks at the first `tests/showcase/...`
read. See:

- `.planning/phases/01.5-parser-strictness-test-gap-resolution-inserted/01.5-UAT.md`
  (status: diagnosed, dated 2026-05-21) — the user-acceptance-test record that
  flipped the gate from PASS to issue.
- `.planning/debug/phase-1-5-gate-failure.md` (status: diagnosed) — the
  full diagnostic walkthrough that ruled out the four spurious hypotheses
  (m01 ADR statFile cwd, synthetic-fixture span violation, recovery soft-gate
  hard-flip, hidden regression between 007311c and 87daddd) and identified the
  missing `tests/showcase` as the actual root cause.

The verification process itself was the bug. The exit gate had never been
exercised on a fresh checkout. A developer running `zig build phase-1.5-gate`
from their main repo saw GREEN; any clone or CI runner saw 9 failures.

## Decision

**Phase N exit verification MUST be performed from a freshly-created git
worktree (or ephemeral CI runner) that materializes all inputs only via
committed git state plus `git submodule update --init --recursive`. The main
developer checkout is NEVER an acceptable verification site for a phase-exit
gate. The mechanical implementation for Phase 1.5 is `zig build
phase-1.5-gate-fresh`; future phase gates (phase-2-gate, phase-3-gate, ...)
MUST add an analogous `-fresh` sibling step.**

The invariant is binding on every future phase from Phase 2 onward through
Phase 6 and any successor milestone.

## Consequences

### Structural guard rails added (committed in this plan)

- `scripts/verify-fresh-worktree.sh` — a `set -euo pipefail` bash script that:
  refuses to run on a dirty main tree; creates an ephemeral worktree under
  `$TMPDIR`; runs `git submodule update --init --recursive` inside it; checks
  that `tests/showcase/packages/vehicle/battery.deal` is materialized; runs
  `zig build phase-1.5-gate` from the ephemeral tree; trap-cleans the
  worktree on EXIT regardless of success or failure.
- `build.zig phase-1.5-gate-fresh` step — `b.addSystemCommand` wrapper that
  shells out to the script. Does NOT depend on `test_step` (that would also
  build the suite in the main tree, defeating the isolation).

### Committed-state changes (committed in this plan)

- `tests/showcase` — was an untracked symlink to `../../spec/examples/showcase`
  (a sibling repo path). Now a committed `120000`-mode symlink to
  `../spec/examples/showcase` (inside the deal repo, via the submodule mount).
- `spec` — new git submodule of `deal/` at mount path `spec`, pinned to
  spec/'s HEAD at plan-time (`fe98f893cefed72a56498f3e0271d459c13a1210`).
  Recorded URL: `git@github.com:deal-lang/spec.git`.
- `.gitmodules` — newly tracked, declares the spec submodule.

### Documentation-surface changes (committed in this plan)

- `README.md` — newly tracked. Documents the mandatory
  `git submodule update --init --recursive` step after clone/worktree-add,
  and points at `zig build phase-1.5-gate-fresh` as the CI-authoritative gate.
- `.planning/decisions/ADR-phase-1.5-fresh-worktree-verification.md` — this
  ADR.
- `.planning/phases/01.5-.../01.5-VERIFICATION.md` — amended in-place with a
  False-GREEN Amendment section + frontmatter `false_green_amended: true` +
  re-verification timestamp.
- `.planning/phases/01.5-.../01.5-CONTEXT.md` — appended
  `<invariants_added_post_completion>` block referencing this ADR.

### Runtime cost

None. This invariant is a CI/verification protocol change. No source under
`src/` is touched; no test source is modified; the C ABI surface (`deal_parse`,
`deal_free`, `deal_diagnostics_count`, `deal_diagnostics_json`, `deal_ast_json`,
`deal_version`) is preserved at 6 symbols. The performance characteristics of
`zig build phase-1.5-gate` itself are unchanged; `phase-1.5-gate-fresh` adds
the one-time cost of cloning the spec submodule into the ephemeral worktree
(small — a few hundred KiB of `.deal[x]` files plus grammar).

### Future-phase obligations

Every phase plan from Phase 2 onward MUST add a `phase-N-gate-fresh` build
step whose definition mirrors `phase-1.5-gate-fresh`: shell out to a script
that creates an ephemeral worktree, runs `git submodule update --init
--recursive`, and runs `phase-N-gate` from the ephemeral tree. The script
itself can be a thin wrapper around `verify-fresh-worktree.sh` parameterised
on the gate name, or a per-phase copy — the planner's choice.

Auditors evaluating any future phase-exit verification should reject any
verification record that was produced from a developer's main checkout
without a corresponding `phase-N-gate-fresh` log line.

## Alternatives considered

The phase-1.5-UAT.md gap.missing block listed three options. Each is recorded
here with a one-sentence rationale for rejection (the user picked OPTION B —
submodule — per the locked planning context):

- **OPTION A (smallest diff): `git add tests/showcase` + commit the existing
  symlink; spell out the spec-as-sibling requirement in README + CI.**
  Rejected because it bakes in a fragile multi-repo-coupling assumption — the
  symlink resolves only on machines where `spec/` happens to be a sibling
  directory of `deal/`. A fresh clone of `deal/` alone would still fail with
  `FileNotFound`. Doesn't fix the root cause.
- **OPTION B (most explicit): make spec/ a git submodule of deal/; change the
  symlink to point into the submodule mount; CI runs `git submodule update
  --init --recursive`.** ACCEPTED. Pins the spec revision per deal/ commit,
  survives any clone / worktree / CI runner, and adds an explicit checkout
  step that CI mechanically performs.
- **OPTION C (heaviest, self-contained): vendor the showcase corpus by
  copying the spec/examples/showcase tree into deal/tests/showcase as real
  files.** Rejected because it duplicates the showcase corpus, decoupling the
  parser test fixtures from the spec repo's authoritative source. A change to
  the spec showcase would require a manual sync step in deal/, easy to forget
  and easy to drift. Multi-repo coupling is eliminated at the cost of
  introducing an out-of-band sync burden.

## References

- `.planning/phases/01.5-parser-strictness-test-gap-resolution-inserted/01.5-UAT.md`
- `.planning/debug/phase-1-5-gate-failure.md`
- `.planning/phases/01.5-parser-strictness-test-gap-resolution-inserted/01.5-VERIFICATION.md`
  (amended in this plan)
- `.planning/phases/01.5-parser-strictness-test-gap-resolution-inserted/01.5-CONTEXT.md`
  (appended in this plan)
- `.planning/phases/01.5-parser-strictness-test-gap-resolution-inserted/01.5-05-tests-showcase-submodule-and-verify-harden-PLAN.md`
- `scripts/verify-fresh-worktree.sh`
- `build.zig` — `phase-1.5-gate-fresh` step
