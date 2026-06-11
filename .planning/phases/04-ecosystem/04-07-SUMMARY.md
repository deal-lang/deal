---
phase: 04-ecosystem
plan: "07"
subsystem: docs-site
tags: [astro, starlight, shiki, github-pages, documentation]
dependency_graph:
  requires: [04-03]
  provides: [deal-lang.org-scaffold, shiki-grammar-wiring, landing-pages, deploy-workflow]
  affects: [04-08]
tech_stack:
  added:
    - astro@6.4.4
    - "@astrojs/starlight@0.39.3"
  patterns:
    - Shiki custom grammar via parsed JSON objects (aliases for fence ID matching)
    - Astro 6 content collection config (src/content.config.ts)
    - GitHub Pages deploy via withastro/action@v6 + deploy-pages@v4
    - OIDC id-token for Pages deploy (no stored PAT)
key_files:
  created:
    - ../deal-lang.org/astro.config.mjs
    - ../deal-lang.org/package.json
    - ../deal-lang.org/package-lock.json
    - ../deal-lang.org/public/CNAME
    - ../deal-lang.org/src/content.config.ts
    - ../deal-lang.org/src/styles/custom.css
    - ../deal-lang.org/src/grammars/deal.tmLanguage.json
    - ../deal-lang.org/src/grammars/dealx.tmLanguage.json
    - ../deal-lang.org/src/content/docs/index.mdx
    - ../deal-lang.org/src/content/docs/getting-started/installation.mdx
    - ../deal-lang.org/src/content/docs/getting-started/first-project.mdx
    - ../deal-lang.org/src/content/docs/getting-started/concepts.mdx
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
    - ../deal-lang.org/.gitignore
    - .planning/decisions/ADR-phase-4-nc1-domain-amendment.md
  modified: []
decisions:
  - "Shiki grammar aliases: grammar name field is DEAL/DEAL Composition (title-case); aliases: ['deal'] and ['dealx'] added in astro.config.mjs so fences resolve correctly"
  - "NC-1 amendment accepted: deal-lang.org is the deployed domain; deal-lang.org/ directory name retained"
  - "Astro 6 requires src/content.config.ts with docsLoader() + docsSchema() — not present in Starlight scaffold template; added as Rule 3 fix"
metrics:
  duration: "~1h"
  completed: "2026-06-06T16:38:00Z"
  tasks_completed: 3
  files_created: 25
  files_modified: 0
---

# Phase 4 Plan 07: Docs Site Scaffold Summary

Astro 6 + Starlight docs site scaffolded in deal-lang.org with real DEAL/DEALX Shiki highlighting via TextMate grammars copied from vscode-deal, landing + getting-started pages authored from showcase examples, GitHub Pages deploy workflow wired, and NC-1 domain amendment ADR recorded.

## What Was Built

### Task 1: Astro + Starlight scaffold with Shiki grammar wiring

- `astro@6.4.4` + `@astrojs/starlight@0.39.3` installed and configured
- `astro.config.mjs`: `site: 'https://deal-lang.org'`, no `base` (Pitfall 7 avoided), full sidebar per UI-SPEC §Sidebar Structure
- DEAL/DEALX TextMate grammars copied from `vscode-deal/syntaxes/` into `src/grammars/`
- Grammars loaded via `expressiveCode.shiki.langs` as parsed JSON objects (never `{ path: ... }`)
- `src/styles/custom.css`: accent override `--sl-color-accent: #0E7490` per UI-SPEC §Color
- `public/CNAME`: `deal-lang.org` (D-64 domain pin)
- deal-lang.org commits: `63f2cb7`

### Task 2: Landing + getting-started pages + NC-1 ADR

- `index.mdx`: Starlight splash/hero template — h1 `DEAL`, tagline `A text-first language for systems engineering`, primary CTA `Get started` → `/getting-started/installation/`, secondary `View on GitHub`; deal fenced block lifted from `packages/requirements/system.deal` (REQ_SYS_001)
- `getting-started/installation.mdx`: CLI install tabs (macOS ARM64/x86_64, Linux, Windows) + VS Code forward-link
- `getting-started/first-project.mdx`: `deal init` → `deal install` → `deal check` → `deal build` flow; `deal.toml` block with `deal-std` git dependency (D-67/D-69)
- `getting-started/concepts.mdx`: definitions/requirements/compositions/traceability/units — each with showcase-derived deal or dealx fenced example
- `.planning/decisions/ADR-phase-4-nc1-domain-amendment.md`: NC-1 amendment (D-64)
- deal-lang.org commits: `83215a2`; deal repo commit: `c173be8`

### Task 3: GitHub Pages deploy workflow

- `.github/workflows/deploy.yml`: triggers on push to main + workflow_dispatch
- Least-privilege permissions: `pages: write` + `id-token: write` only (T-4-17 OIDC, no PAT)
- `concurrency: group "pages", cancel-in-progress: false`
- Build job: `actions/checkout@v4` then `withastro/action@v6` (handles npm ci + astro build)
- Deploy job: `needs: build`, environment `github-pages`, `actions/deploy-pages@v4`
- TODO comment marks where Plan 08 appends snippet-check + Shiki scope gate
- deal-lang.org commit: `fc8f1fe`

## Deal Fenced Snippets (for Plan 08 CI gate coverage)

Plan 08's CI snippet-parse gate should cover these files and snippets:

| File | Language | Snippet Source |
|------|----------|----------------|
| `src/content/docs/index.mdx` | `deal` | REQ_SYS_001 from `packages/requirements/system.deal` |
| `src/content/docs/getting-started/concepts.mdx` | `deal` | TractionMotor part def (motor.deal) |
| `src/content/docs/getting-started/concepts.mdx` | `deal` | REQ_SYS_001 requirement def |
| `src/content/docs/getting-started/concepts.mdx` | `deal` | NEED_RANGE need def |
| `src/content/docs/getting-started/concepts.mdx` | `deal` | REQ_BAT_003 (temperature range) |
| `src/content/docs/getting-started/concepts.mdx` | `dealx` | satisfy REQ_SYS_001 traceability block |
| `src/content/docs/getting-started/first-project.mdx` | `toml` | deal.toml scaffold (not a deal parse target) |
| `src/content/docs/getting-started/installation.mdx` | `bash` | CLI install commands (not a deal parse target) |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Astro 6 content collection not recognized without content.config.ts**

- **Found during:** Task 1 build verification
- **Issue:** First build attempt produced "The collection 'docs' does not exist or is empty" because Astro 6 requires an explicit `src/content.config.ts` with `docsLoader()` + `docsSchema()`. The Starlight template scaffold includes this, but the manual scaffold approach in the plan did not list it.
- **Fix:** Created `src/content.config.ts` with Astro 6 content layer configuration.
- **Files modified:** `../deal-lang.org/src/content.config.ts`
- **Commit:** `63f2cb7`

**2. [Rule 3 - Blocking] Shiki grammar name mismatch — fence `\`\`\`deal` not resolved**

- **Found during:** Task 1 build verification (second build attempt)
- **Issue:** Build succeeded but emitted warnings "language 'deal' not found". The TextMate grammar `name` field is `"DEAL"` (title-case); Shiki uses this as the primary language ID, so the lowercase fence `\`\`\`deal` did not resolve.
- **Fix:** Added `aliases: ['deal']` / `aliases: ['dealx']` when spreading grammar objects in `astro.config.mjs`. This registers both `DEAL` and `deal` as valid identifiers. No change to the grammar source files (canonical vscode-deal grammars untouched).
- **Files modified:** `../deal-lang.org/astro.config.mjs`
- **Commit:** `63f2cb7`

## Known Stubs

The following reference and CLI pages are stubs (one-line placeholder bodies). They were created to satisfy Starlight sidebar validation (all sidebar slugs must have matching content files). Full content is authored in Plan 08:

| File | Status | Resolved By |
|------|--------|-------------|
| `src/content/docs/reference/definitions.mdx` | stub | Plan 08 |
| `src/content/docs/reference/compositions.mdx` | stub | Plan 08 |
| `src/content/docs/reference/requirements.mdx` | stub | Plan 08 |
| `src/content/docs/reference/traceability.mdx` | stub | Plan 08 |
| `src/content/docs/reference/imports.mdx` | stub | Plan 08 |
| `src/content/docs/reference/annotations.mdx` | stub | Plan 08 |
| `src/content/docs/reference/units.mdx` | stub | Plan 08 |
| `src/content/docs/reference/deal-toml.mdx` | stub | Plan 08 |
| `src/content/docs/cli/overview.mdx` | stub | Plan 08 |
| `src/content/docs/cli/vscode-setup.mdx` | stub | Plan 08 |

These stubs do NOT prevent Plan 07's goal (site builds, Shiki wiring works, landing/getting-started pages exist) — they are intentional placeholders for Plan 08.

## User Setup Required

Before the site goes live, the domain owner must:

1. Set a DNS CNAME record: `@ → deal-lang.github.io` (or `www → deal-lang.github.io`)
2. In GitHub repo settings for `deal-lang/deal-lang.org` → Pages → Source: GitHub Actions; Custom domain: `deal-lang.org`; Enable HTTPS

See `ADR-phase-4-nc1-domain-amendment.md` §Consequences for full details.

## Self-Check: PASSED

- `../deal-lang.org/astro.config.mjs` exists and contains `deal-lang.org`: FOUND
- `../deal-lang.org/public/CNAME` contains `deal-lang.org`: FOUND
- `../deal-lang.org/src/grammars/deal.tmLanguage.json`: FOUND
- `../deal-lang.org/src/grammars/dealx.tmLanguage.json`: FOUND
- `../deal-lang.org/src/content/docs/index.mdx`: FOUND
- `../deal-lang.org/src/content/docs/getting-started/installation.mdx`: FOUND
- `../deal-lang.org/src/content/docs/getting-started/first-project.mdx`: FOUND
- `../deal-lang.org/src/content/docs/getting-started/concepts.mdx`: FOUND
- `../deal-lang.org/.github/workflows/deploy.yml`: FOUND
- `.planning/decisions/ADR-phase-4-nc1-domain-amendment.md`: FOUND
- deal-lang.org commits 63f2cb7 / 83215a2 / fc8f1fe: FOUND
- deal repo commit c173be8: FOUND
- `npm run build` exits 0: PASSED (15 pages, no unknown-language warnings)
