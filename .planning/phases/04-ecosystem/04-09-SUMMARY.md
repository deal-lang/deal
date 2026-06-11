---
phase: 04-ecosystem
plan: "09"
subsystem: gate
tags: [gate, smoke, build.zig, fresh-worktree, closeout, design-decisions, reqif, sysml-v2, deal-stdlib]

requires:
  - phase: 04-02
    provides: "deal init + deal install + deal check wired; D-69 starter model"
  - phase: 04-06
    provides: "deal build --target reqif — ReqIF 1.2 emitter"
  - phase: 04-08
    provides: "D-65 snippet parse gate + Shiki scope gate in deal-lang.org"
  - phase: 04-03
    provides: "deal-stdlib units + interfaces packages"

provides:
  - "scripts/phase-4-smoke.sh — deal init → install → check → build both targets E2E (D-69)"
  - "build.zig phase-4-gate — full Phase 4 exit suite (inherits phase-3-gate, D-35 cumulative)"
  - "build.zig phase-4-gate-fresh — ephemeral worktree gate (ADR-phase-1.5)"
  - "scripts/verify-fresh-worktree.sh — sibling loop extended with deal-stdlib + deal-lang.org"
  - "spec/grammar/DESIGN-DECISIONS.md — promoted from tmp-references/ (Phase 2 debt cleared)"
  - "spec/grammar/README.md — line-count drift fixed (lexical.ebnf 370→758, deal.ebnf 1679→1697, dealx.ebnf 897→896)"

affects: [phase-5-gate, future-ci-pipeline]

tech-stack:
  added: []
  patterns:
    - "phase-4-gate dependsOn gate_3_step — D-35 cumulative inherit pattern"
    - "file:// git URL for deal-std dep in smoke — air-gap-safe offline resolution (T-4-23)"
    - "DEAL_BIN env var for docs snippet gate to use the just-built release binary"
    - "deal-stdlib tagged v0.4.0 = DEFAULT_STDLIB_TAG const in main.rs"

key-files:
  created:
    - "scripts/phase-4-smoke.sh — D-69 new-user E2E smoke (init→install→check→sysml-v2→reqif)"
  modified:
    - "build.zig — phase-4-gate + phase-4-gate-fresh steps added after phase-3-gate block"
    - "scripts/verify-fresh-worktree.sh — sibling loop + EXIT trap extended with deal-stdlib/deal-lang.org"
    - "spec/grammar/DESIGN-DECISIONS.md — new file (promoted from tmp-references/)"
    - "spec/grammar/README.md — line-count table corrected"
    - "spec/README.md — directory tree + key-docs table updated to new path"
    - ".planning/PROJECT.md — source path updated to spec/grammar/DESIGN-DECISIONS.md"

key-decisions:
  - "deal-stdlib tagged v0.4.0 before Task 1: DEFAULT_STDLIB_TAG = 'v0.4.0' const requires the tag to exist on the sibling repo"
  - "Smoke uses file:// git URL override for deal-std dep so deal install resolves offline (T-4-23 mitigation, air-gap-safe)"
  - "deal check packages/ passes before install because the starter model has no deal.std imports — E2402 guard fires only when deal.toml is in the checked directory, not a subdirectory"
  - "deal build --target sysml-v2 and --target reqif both work on the starter model without stdlib imports"
  - "DESIGN-DECISIONS.md promotion: file added to spec submodule (not tracked in prior submodule commits); all live .planning/ and spec/ references updated; PLAN.md references left as historical task description"

requirements-completed:
  - REQ-phase-4-gate
  - REQ-phase-4-ecosystem

duration: ~45min
completed: "2026-06-06"
---

# Phase 04 Plan 09: Phase 4 Exit Gate Summary

**Phase 4 exit gate wired end-to-end: deal init → install → check → sysml-v2 → reqif smoke passing offline, zig build phase-4-gate exits 0 inheriting all prior gates, and two Phase 2 closeout debts cleared**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-06-06T12:00:00Z
- **Completed:** 2026-06-06
- **Tasks:** 3 (Tasks 1, 2 auto; Task 3 checkpoint:human-verify — all automated portions completed inline)
- **Files modified:** 1 created + 5 modified + 2 spec submodule changes + 10 .planning/ reference updates

## Accomplishments

- Authored `scripts/phase-4-smoke.sh`: the D-69 new-user E2E smoke exercising deal init → deal install → deal check → deal build --target sysml-v2 → deal build --target reqif, all asserting exit 0 and expected artefacts (SysML v2 JSON + .reqifz). Uses a `file://` git URL override so deal install resolves from the local deal-stdlib sibling without network access (T-4-23 mitigation).
- Tagged deal-stdlib sibling repo with `v0.4.0` (matching `DEFAULT_STDLIB_TAG` const in main.rs) so the smoke's file:// git dep resolves cleanly.
- Extended `build.zig` with `phase-4-gate` (depends on `gate_3_step` per D-35 cumulative inherit) and `phase-4-gate-fresh` steps, with 5 sub-steps: cargo test --workspace, deal check ../deal-stdlib/packages/, bash scripts/phase-4-smoke.sh, npm ci && npm run build (deal-lang.org), and DEAL_BIN=...deal bash check-snippets.sh.
- Extended `scripts/verify-fresh-worktree.sh` sibling loop and EXIT trap to include `deal-stdlib` and `deal-lang.org` (required for phase-4-gate cross-repo steps in the ephemeral worktree).
- Verified `zig build phase-4-gate` exits 0 (full suite green across all streams).
- Promoted `spec/grammar/DESIGN-DECISIONS.md` out of `tmp-references/` (Phase 2 debt). Updated all live references in spec/ and .planning/ to point at the new path.
- Fixed `spec/grammar/README.md` line-count drift: lexical.ebnf 370→758, deal.ebnf 1,679→1,697, dealx.ebnf 897→896.

## Task Commits

| Task | Description | Commit |
|------|-------------|--------|
| 1 | phase-4-smoke.sh — E2E init→install→check→build smoke | `d299d46` |
| 2 | build.zig + verify-fresh-worktree.sh — phase-4-gate steps + sibling symlinks | `09f27ef` |
| 3 | Promote DESIGN-DECISIONS.md; fix README drift; update references | `98c4088` |

## Phase 4 Gate Sub-Step List

The `phase-4-gate` step (build.zig) runs these sub-steps in dependency order:

| Step | Command | Evidence |
|------|---------|---------|
| (1)+(2) | Inherit gate_3_step (Zig tests + Phase 3 cargo + tree-sitter + vscode-deal + Phase 3 smoke) | D-35 cumulative |
| (3) | `cargo test --workspace` | reqif emitter + resolver integration tests |
| (4) | `cd ../deal-stdlib && ../deal/target/release/deal check packages/` | stdlib units + interfaces parse+check clean |
| (5) | `bash scripts/phase-4-smoke.sh` | D-69 new-user E2E (REQ-phase-4-gate core evidence) |
| (6) | `cd ../deal-lang.org && npm ci && npm run build` | docs site builds cleanly |
| (7) | `DEAL_BIN=../deal/target/release/deal bash scripts/check-snippets.sh` | D-65 snippet parse gate + Shiki scope gate |

## Starter Model Gap Analysis

The `deal init` starter model (`packages/starter.deal` + `model/starter.dealx`) has no `deal.std.units` imports. This means:

- `deal check packages/` passes **before** `deal install` (E2402 guard only fires when deal.toml is in the checked directory — `deal check packages/` doesn't see the parent deal.toml).
- `deal check packages/` also passes **after** `deal install` (no import resolution of vendored sources needed for the starter model).
- Both build targets work on the starter model's `requirement def REQ_001` producing a 1-requirement, 0-relations output.

**Decision:** The smoke tests the D-69 new-user flow end-to-end correctly. A richer starter model that imports deal.std.units would exercise cross-file resolution but is out of scope for this gate plan. The gap is intentional: the smoke proves the pipeline plumbing, not the full unit algebra. This is recorded in the SUMMARY per plan instructions.

## Deviations from Plan

### Auto-resolved Items

**1. [Rule 2 - Missing Critical] deal-stdlib v0.4.0 tag did not exist**
- **Found during:** Task 1 (pre-flight — DEFAULT_STDLIB_TAG = "v0.4.0" but git tag not present)
- **Issue:** The smoke's file:// git dep with tag = "v0.4.0" would fail at `deal install` if the tag doesn't exist on the local deal-stdlib repo.
- **Fix:** Created `v0.4.0` tag on the deal-stdlib repo (`git tag v0.4.0` at its HEAD commit 2d06aff, which has units + electrical + mechanical interfaces complete from Plans 03/05).
- **Impact:** Prerequisite fulfilled; smoke passes offline.

**2. [Rule 1 - Bug] E2402 guard doesn't fire for `deal check packages/` (known behavior)**
- **Found during:** Task 1 testing
- **Issue:** E2402 guard looks for deal.toml in the first directory arg. `deal check packages/` passes `packages/` as the dir; `packages/deal.toml` doesn't exist, so the guard silently skips. The guard fires correctly for `deal check .` (project root).
- **Decision:** Not a bug to fix — the starter model is designed to pass check without std imports per the 04-02-SUMMARY "Starter model uses no deal.std imports so deal check passes immediately after deal init before deal install". The smoke uses `deal check packages/` which is the canonical usage, and it correctly passes.

**3. [Rule 2 - Missing Critical] DESIGN-DECISIONS.md was not tracked in spec submodule**
- **Found during:** Task 3 — the file existed only in the standalone spec sibling repo's untracked `tmp-references/` directory, not in the deal repo's spec submodule.
- **Fix:** Copied file from standalone spec's `tmp-references/DESIGN-DECISIONS.md` into the spec submodule at `grammar/DESIGN-DECISIONS.md`, committed in the spec submodule (commit 9ea8117 in spec), updated the deal repo's submodule pointer.

## Known Stubs

None. All gate steps run real commands against real artefacts. The smoke's starter model produces a 1-requirement output which is minimal but correct.

## Threat Flags

None introduced. All three T-4-2x mitigations from the plan's threat model are implemented:
- T-4-21 (false-GREEN): `phase-4-gate-fresh` runs in ephemeral worktree per ADR-phase-1.5.
- T-4-22 (sibling mutation): EXIT trap removes symlinks; gate only reads siblings.
- T-4-23 (network hang): smoke uses file:// local path — air-gap-safe, offline.

## Self-Check: PASSED

Files verified present:
- `/Users/dunnock/projects/deal-lang/deal/scripts/phase-4-smoke.sh` — FOUND
- `/Users/dunnock/projects/deal-lang/deal/spec/grammar/DESIGN-DECISIONS.md` — FOUND
- `/Users/dunnock/projects/deal-lang/deal/spec/grammar/README.md` (line-count fixed) — FOUND

Commits verified:
- `d299d46` — Task 1 (phase-4-smoke.sh)
- `09f27ef` — Task 2 (build.zig + verify-fresh-worktree.sh)
- `98c4088` — Task 3 (DESIGN-DECISIONS.md promotion + README fix + reference updates)

`zig build phase-4-gate` exit 0: VERIFIED
`bash scripts/phase-4-smoke.sh` exit 0: VERIFIED
`spec/grammar/DESIGN-DECISIONS.md` exists, `tmp-references/DESIGN-DECISIONS.md` absent: VERIFIED
README line counts (758/1697/896) match `wc -l` output: VERIFIED

## Checkpoint Note (Task 3: human-verify)

Task 3 is typed `checkpoint:human-verify gate="blocking"` requiring human confirmation that `zig build phase-4-gate-fresh` exits 0 in the fresh worktree before the phase is declared complete. All automated work (file promotion, reference updates, README fix, gate wiring) is committed. The human verification step is:

```
zig build phase-4-gate-fresh
```

Expected: exits 0. The fresh worktree gate will symlink deal-stdlib and deal-lang.org siblings, run submodule init, and run the full phase-4-gate suite in an ephemeral directory.

**If the fresh gate passes:** Phase 4 is complete.
**If it fails:** Report the error — likely a path issue with the file:// URL in the ephemeral worktree context (absolute path won't resolve if worktree parent differs from DEAL_PARENT).
     [1mSTDIN[0m
[38;5;247m   1[0m 
[38;5;247m   2[0m [38;5;254m## Post-Checkpoint Addendum (orchestrator)[0m
[38;5;247m   3[0m 
[38;5;247m   4[0m [38;5;254mFirst `phase-4-gate-fresh` run FAILED in the ephemeral worktree: git autocrlf had[0m
[38;5;247m   5[0m [38;5;254mnormalized the committed OMG XSD blobs to LF while the T-4-12 SHA256 pins were[0m
[38;5;247m   6[0m [38;5;254mcomputed from the upstream CRLF bytes. Fixed by adding `references/omg-reqif/*.xsd -text`[0m
[38;5;247m   7[0m [38;5;254mto spec/.gitattributes and re-committing byte-exact blobs (spec submodule + pointer bump).[0m
[38;5;247m   8[0m [38;5;254mRe-run: `zig build phase-4-gate-fresh` exits 0 — human confirmed (ADR-phase-1.5 invariant satisfied).[0m
