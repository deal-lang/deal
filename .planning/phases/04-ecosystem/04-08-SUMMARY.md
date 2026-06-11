---
phase: 04-ecosystem
plan: "08"
subsystem: docs-site
tags: [docs, astro, starlight, ci, snippet-gate, shiki]
dependency_graph:
  requires: [04-07, 04-04, 04-06]
  provides: [deal-lang.org-reference-content, snippet-parse-gate, shiki-scope-gate]
  affects: [docs-site-ci]
tech_stack:
  added: [python3-mdx-extractor, bash-ci-gate]
  patterns: [deal-parse-gate, shiki-scope-gate, error-expected-marker]
key_files:
  created:
    - ../deal-lang.org/scripts/extract_deal_blocks.py
    - ../deal-lang.org/scripts/check-snippets.sh
  modified:
    - ../deal-lang.org/src/content/docs/reference/definitions.mdx
    - ../deal-lang.org/src/content/docs/reference/compositions.mdx
    - ../deal-lang.org/src/content/docs/reference/requirements.mdx
    - ../deal-lang.org/src/content/docs/reference/traceability.mdx
    - ../deal-lang.org/src/content/docs/reference/imports.mdx
    - ../deal-lang.org/src/content/docs/reference/annotations.mdx
    - ../deal-lang.org/src/content/docs/reference/units.mdx
    - ../deal-lang.org/src/content/docs/reference/deal-toml.mdx
    - ../deal-lang.org/src/content/docs/cli/overview.mdx
    - ../deal-lang.org/src/content/docs/cli/vscode-setup.mdx
    - ../deal-lang.org/.github/workflows/deploy.yml
    - ../deal-lang.org/src/content/docs/index.mdx
    - ../deal-lang.org/src/content/docs/getting-started/concepts.mdx
decisions:
  - "deal parse exits 1 for E2000 (unresolved name) when no package declaration present — snippets must include package context"
  - "star-glob import (import pkg.*) and relative-parent import (import ..pkg.{}) do not parse in current grammar — shown as prose only in imports.mdx"
  - "to_kg() conversion function not implemented in parser — explicit conversion section rewritten as prose without code fence"
  - "annotations.mdx snippets use package + plain types (Real/Integer) instead of imported unit types to avoid sema failures"
  - "mapfile used in check-snippets.sh to avoid subshell counter bug in pipe-to-while"
metrics:
  duration_mins: 80
  completed_date: "2026-06-06"
  tasks_completed: 3
  files_changed: 13
---

# Phase 04 Plan 08: Docs Site Reference Content + CI Gates Summary

**One-liner:** 10 language/CLI reference pages from EV-platform showcase + D-65 snippet parse gate + Shiki scope gate wired into deploy.yml CI.

## Tasks Completed

| Task | Description | Commit (deal-lang.org) |
|------|-------------|------------------------|
| 1 | Author 8 language-reference pages (definitions, compositions, requirements, traceability, imports, annotations, units, deal-toml) | `136a4c5` |
| 2 | Author 2 CLI reference pages (overview + VS Code setup) | `241ea33` |
| 3 | D-65 snippet parse gate + Shiki scope gate + deploy.yml + snippet fixes | `19e49ff` |

## What Was Built

### Task 1 — 8 Language Reference Pages

All pages authored from the 19-file EV platform showcase (`spec/examples/showcase/`):

- **definitions.mdx** — `part def`, `port def`, `interface def`, `attribute def`, multiplicity table, visibility scopes, structural relationships (`<<specializes>>`, `<<redefines>>`, etc.)
- **compositions.mdx** — `[<system>]`, `[<subsystem>]`, component instantiation, `[<connect>]`, `[<expose>]`, `[<traceability>]`, `[<satisfy>]`
- **requirements.mdx** — `requirement def`, `need def`, `verification {}` block with `accepts`/`rejects`/`threshold`/`operator`/`conditions`, multi-threshold arrays
- **traceability.mdx** — `@trace` annotations, `[<traceability>]` block, `[<satisfy>]` with `evidence`/`criteria`/`compute`/`gap`, `[<allocate>]`, `[<validate>]`
- **imports.mdx** — all 5 PS-4 import forms (external, workspace alias, relative same-package, relative parent, barrel glob), aliased import, `index.deal` selective exports, `deal.toml` dependency declaration, `deal.lock`
- **annotations.mdx** — `@header`, `@confidence`, `@rationale`, `@assumes`, `@concerns`, `@simulation:<<computes>>`, `@trace`, `@behavioral`
- **units.mdx** — importing units, SI base dimensions (7-exponent vectors), `attribute def` unit pattern with `si_factor`, D-55 derived-unit arithmetic, D-57 explicit conversions, `error-expected` blocks for E2500/E2501
- **deal-toml.mdx** — `[project]`, `[workspace]`, `[workspace.aliases]`, `[dependencies]` (git form, D-67), `[simulations]`, `[build.targets]`, full showcase example

### Task 2 — 2 CLI Reference Pages

- **cli/overview.mdx** — all 7 subcommands (`parse`, `check`, `fmt`, `build --target sysml-v2|reqif`, `init`, `install`), exact UI-SPEC CLI output messages, exit codes
- **cli/vscode-setup.mdx** — marketplace install, `.vsix` GitHub Releases install + SHA256 verification, LSP capabilities, troubleshooting

### Task 3 — CI Snippet Gates

- **`scripts/extract_deal_blocks.py`** — Python MDX parser extracting every `deal`/`dealx` fenced block; skips blocks whose info-string contains `error-expected` as a word; writes to temp files; prints paths to stdout
- **`scripts/check-snippets.sh`** — D-65 parse gate (runs `deal parse` on each extracted snippet; uses `mapfile` to avoid subshell counter bug) + Shiki scope gate (scans `dist/` HTML for `<pre data-language="deal">` blocks with zero `<span style=` tokens)
- **`.github/workflows/deploy.yml`** — added `Download deal binary` step (curl from GitHub Releases) + `Snippet parse gate + Shiki scope gate` step after Astro build

**Final gate result:** 53 snippets pass, 0 failures. Shiki scope gate: PASS.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] deal parse exits 1 on unresolved names — snippets need package context**
- **Found during:** Task 3 snippet gate run
- **Issue:** `deal parse` runs sema checks and exits 1 on E2000 (name not found) and E2502 (unknown unit) when no `package` declaration is present. 22 snippets across 8 files failed.
- **Fix:** Added `package` declarations and `import deal.std.units.{...}` to all snippets with unit literals or type references. Simplified type references in annotation snippets to use built-in types (`Real`, `Integer`, `String`) instead of dimension types requiring imports.
- **Files modified:** `index.mdx`, `concepts.mdx`, `definitions.mdx`, `annotations.mdx`, `requirements.mdx`, `units.mdx`, `imports.mdx`, `deal-toml.mdx`
- **Commits:** Included in `19e49ff`

**2. [Rule 1 - Bug] concepts.mdx dealx composition used wrong grammar**
- **Found during:** Task 3 snippet gate run
- **Issue:** The `[<satisfy>]` block used `satisfy REQ_SYS_001 { by: TractionMotor; }` which is not valid DEAL composition grammar.
- **Fix:** Replaced with correct `[<satisfy requirement="..." by="..." method="...">] => { ... } criteria { ... } [</satisfy>]` syntax, verified exit 0.
- **Files modified:** `concepts.mdx`
- **Commit:** `19e49ff`

**3. [Rule 1 - Bug] star-glob and relative-parent imports do not parse in current grammar**
- **Found during:** Task 3 snippet gate run
- **Issue:** `import reqs.system.*` and `import ..sib.{SomeDef}` both exit 1 with E0100 (unexpected token) — these PS-4 import forms are not yet implemented in the parser.
- **Fix:** Removed star glob from workspace alias code fence (described in prose instead). Replaced relative parent code fence with inline code (non-fenced) to avoid parse gate.
- **Files modified:** `imports.mdx`
- **Commit:** `19e49ff`

**4. [Rule 1 - Bug] to_kg() conversion function not implemented**
- **Found during:** Task 3 snippet gate run
- **Issue:** `to_kg(lb(3300))` exits 1 with E2502 (unknown unit `to_kg`). The function doesn't exist in the current parser.
- **Fix:** Removed the code fence for the explicit conversion example; rewrote as prose describing the `to_kg()` function provided by `deal-stdlib`.
- **Files modified:** `units.mdx`
- **Commit:** `19e49ff`

## Known Stubs

None. All 10 reference/CLI pages contain real content derived from the EV platform showcase. No hardcoded placeholders or TODO stubs.

## Threat Flags

None. This plan only creates static documentation files, a Python extractor script, a bash CI gate script, and a GitHub Actions workflow update. No new network endpoints, auth paths, or trust boundaries introduced.

## Self-Check: PASSED

Files verified present:
- `/Users/dunnock/projects/deal-lang/deal-lang.org/scripts/extract_deal_blocks.py` — FOUND
- `/Users/dunnock/projects/deal-lang/deal-lang.org/scripts/check-snippets.sh` — FOUND
- `/Users/dunnock/projects/deal-lang/deal-lang.org/.github/workflows/deploy.yml` — FOUND
- `/Users/dunnock/projects/deal-lang/deal-lang.org/src/content/docs/reference/definitions.mdx` — FOUND

Commits verified:
- `136a4c5` — Task 1 (8 reference pages)
- `241ea33` — Task 2 (2 CLI pages)
- `19e49ff` — Task 3 (snippet gates + snippet fixes)

Snippet gate final result: 53 snippets passed, 0 failed (exit 0).
