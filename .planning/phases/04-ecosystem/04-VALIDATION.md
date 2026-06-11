---
phase: 4
slug: ecosystem
status: validated
nyquist_compliant: true
wave_0_complete: true
created: 2026-06-05
validated: 2026-06-06
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Zig test runner (built-in) + `cargo test --workspace` (Rust) + `npm run build` (docs site) |
| **Config file** | `build.zig` (Zig); `Cargo.toml` workspace (Rust); `../deal-lang.org/package.json` (docs) |
| **Quick run command** | `zig build test -Dtest-filter=sema && cargo test -p deal --lib` |
| **Full suite command** | `zig build test && cargo test --workspace` |
| **Estimated runtime** | ~120 seconds (full suite); ~30 seconds (quick) |

---

## Sampling Rate

- **After every task commit:** Run `zig build test -Dtest-filter=<area> && cargo test -p deal --lib`
- **After every plan wave:** Run `zig build test && cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite green PLUS `zig build phase-4-gate && zig build phase-4-gate-fresh`
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 04-01-01 | 01 | 1 | REQ-phase-4-2-reqif-codegen | XSD-tamper (T-02-21 pattern) | SHA256SUMS verifies XSD bundle | unit | `cd spec/references/omg-reqif && shasum -a 256 -c SHA256SUMS` | ✅ | ✅ green |
| 04-01-02 | 01 | 1 | REQ-phase-4-1-stdlib | — | N/A | unit | `zig build test -Dtest-filter=diagnostics` | ✅ | ✅ green |
| 04-01-03 | 01 | 1 | REQ-phase-4-1-stdlib, REQ-phase-4-2-reqif-codegen | — | N/A | scaffold | `zig build test -Dtest-filter=sema; ls tests/golden/reqif/ tests/regressions/sema/07-*.deal` | ✅ | ✅ green |
| 04-02-01 | 02 | 1 | REQ-phase-4-4-package-resolution | T-4-02, T-4-03, T-4-04 | Path-traversal + URL-scheme rejection asserted; deterministic lock | integration | `cd cli && cargo test --test resolver_test` | ✅ | ✅ green |
| 04-02-02 | 02 | 1 | REQ-phase-4-4-package-resolution | T-4-02 | Overwrite guard on `deal init`; E2402 missing-dep guard | smoke | `cargo run -p deal -- init __tmp` + starter-file asserts (see plan) | ✅ | ✅ green |
| 04-03-01 | 03 | 1 | REQ-phase-4-1-stdlib | — | N/A (ADR decision; parser is the oracle) | probe | `cargo run -q -p deal -- parse <probe.deal>` | ✅ | ✅ green |
| 04-03-02 | 03 | 1 | REQ-phase-4-1-stdlib | — | N/A | integration | `for f in ../deal-stdlib/packages/units/*.deal; do cargo run -q -p deal -- parse "$f"; done` | ✅ | ✅ green |
| 04-04-01 | 04 | 2 | REQ-phase-4-1-stdlib | T-4-08 | Bounded recursion in dim-vector evaluation | unit | `zig build test -Dtest-filter=sema` | ✅ | ✅ green |
| 04-04-02 | 04 | 2 | REQ-phase-4-1-stdlib | T-4-09 | Vendored stdlib only parsed, never executed | unit + build | `zig build test -Dtest-filter=sema.dimensional && cargo build -p deal` | ✅ | ✅ green |
| 04-05-01 | 05 | 2 | REQ-phase-4-1-stdlib | — | N/A | integration | `for f in ../deal-stdlib/packages/interfaces/electrical/*.deal; do cargo run -q -p deal -- parse "$f"; done` | ✅ | ✅ green |
| 04-05-02 | 05 | 2 | REQ-phase-4-1-stdlib | — | N/A | integration | `for f in ../deal-stdlib/packages/interfaces/mechanical/*.deal; do cargo run -q -p deal -- parse "$f"; done` | ✅ | ✅ green |
| 04-06-01 | 06 | 3 | REQ-phase-4-2-reqif-codegen | XSD-tamper | SHA256 verify at load; path-traversal guard in bundle locate | unit | `cd cli && cargo test reqif_schema` | ✅ | ✅ green |
| 04-06-02 | 06 | 3 | REQ-phase-4-2-reqif-codegen | — | Deterministic output (BTreeMap, sorted nodes) | golden | `cd cli && cargo test --test golden_reqif` | ✅ | ✅ green |
| 04-06-03 | 06 | 3 | REQ-phase-4-2-reqif-codegen | — | Structural validation is the hard gate (OQ-1) | e2e | `cargo run -q -p deal -- build --target reqif --validate tests/showcase/` | ✅ | ✅ green |
| 04-07-01 | 07 | 2 | REQ-phase-4-3-docs-site | npm supply chain | `npm ci` with committed lockfile | smoke | `cd ../deal-lang.org && npm ci && npm run build` | ✅ | ✅ green |
| 04-07-02 | 07 | 2 | REQ-phase-4-3-docs-site | — | N/A | build | `cd ../deal-lang.org && npm run build` + page `ls` asserts | ✅ | ✅ green |
| 04-07-03 | 07 | 2 | REQ-phase-4-3-docs-site | CI token scope | `pages: write` permission scoped in deploy.yml | config | `grep` asserts on `.github/workflows/deploy.yml` | ✅ | ✅ green |
| 04-08-01 | 08 | 4 | REQ-phase-4-3-docs-site | — | N/A | build | `ls src/content/docs/reference/*.mdx \| wc -l` + `npm run build` | ✅ | ✅ green |
| 04-08-02 | 08 | 4 | REQ-phase-4-3-docs-site | — | N/A | build | `ls` + `npm run build` + `grep 'deal init'` asserts | ✅ | ✅ green |
| 04-08-03 | 08 | 4 | REQ-phase-4-3-docs-site | snippet gate runs local binary only | `deal parse` local, no network (RESEARCH threat table) | integration | `DEAL_BIN=<repo>/target/debug/deal bash scripts/check-snippets.sh` (snippet parse + Shiki scope gate; OQ-2) | ✅ | ✅ green |
| 04-09-01 | 09 | 5 | REQ-phase-4-gate, REQ-phase-4-ecosystem | — | E2E uses local-path override, no network in gate | e2e | `bash scripts/phase-4-smoke.sh` | ✅ | ✅ green |
| 04-09-02 | 09 | 5 | REQ-phase-4-gate | — | Fresh-worktree gate proves no untracked-file dependence | e2e | `zig build phase-4-gate` | ✅ | ✅ green |
| 04-09-03 | 09 | 5 | REQ-phase-4-gate | — | N/A | closeout | `test -f spec/grammar/DESIGN-DECISIONS.md` + drift greps | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Created by Plan 01 Task 3 (scaffolds) and the first task of each owning plan:

- [x] `tests/unit/sema_dimensional.zig` — dimensional algebra harness stubs (REQ-phase-4-1-stdlib) — Plan 01 Task 3
- [x] `tests/regressions/sema/07-*.deal` — dimensional regression fixtures — Plan 01 Task 3 (4 fixtures)
- [x] `tests/golden/reqif/` + 2 fixture pairs (REQ-phase-4-2-reqif-codegen) — Plan 01 Task 3
- [x] `cli/tests/resolver_test.rs` — `deal install` integration test (REQ-phase-4-4-package-resolution) — Plan 02 Task 1 (5 tests)
- [x] `../deal-lang.org/scripts/check-snippets.sh` — docs snippet CI gate (REQ-phase-4-3-docs-site) — Plan 08 Task 3 (53 snippets + Shiki scope gate)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| DOORS import of showcase `.reqifz` | REQ-phase-4-2-reqif-codegen | Requires a licensed IBM DOORS instance not available in CI | Import `build/reqif/model.reqifz` into DOORS; confirm requirements + trace links appear. Automated fallback: Python `reqif` library parse smoke (Plan 06 Task 3) |
| `deal-lang.org` live on production DNS | REQ-phase-4-3-docs-site | GitHub Pages + DNS propagation is an external deployment event | After deploy workflow runs green, open `https://deal-lang.org` (NC-1 ADR domain); confirm landing, getting-started, reference, CLI, VS Code pages render with highlighted code blocks |
| UI-SPEC visual compliance (typography budget, accent scope, focal point) | REQ-phase-4-3-docs-site | Visual design conformance needs human review | Compare rendered site against `04-UI-SPEC.md` contracts: 4 font sizes / 2 weights, accent #0E7490 only in its 4 scoped elements |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (Plan 01 Task 3 + Plan 02 Task 1 + Plan 08 Task 3)
- [x] No watch-mode flags
- [x] Feedback latency < 120s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** ready for execution

---

## Validation Audit 2026-06-06

| Metric | Count |
|--------|-------|
| Requirements audited | 23 |
| Missing tests (gaps) | 0 |
| Failing on entry | 9 |
| Resolved | 9 |
| Escalated | 0 |

**Findings:**

- Structural test coverage was complete on entry — every task row had an existing automated command and artifact. No test generation was required.
- 9 rows were red due to a single environment regression, not test gaps: superproject commit `8ecd764` moved the `spec` submodule pointer from `fc533e9` (Phase 4 artifacts) back to `214a6c4`. Since `tests/showcase → ../spec/examples/showcase` is a symlink, this broke the OMG ReqIF XSD bundle (04-01-01, 04-06-01/02/03), the showcase kW-import fix (04-04-01/02, Zig snapshots), and `spec/grammar/DESIGN-DECISIONS.md` (04-09-03).
- **Resolution:** spec submodule checked out at local `main` (`fc533e9`, a strict descendant of `214a6c4` — fast-forward, nothing lost). The one dirty worktree file (`system.deal` kW import) was byte-identical to `52a94c6` already on `main` and was absorbed.
- Re-run results: Zig 64/64; `reqif_schema` 10/10; `golden_reqif` 3/3; `resolver_test` 5/5; SHA256SUMS OK; e2e ReqIF build OK (13 requirements); docs `npm ci && npm run build` OK (15 pages); snippet gate 53/53 + Shiki scope PASS; `phase-4-smoke.sh` all 5 steps PASS; `zig build phase-4-gate` exit 0.
- ⚠️ Spec `main` is 5 commits ahead of `origin/main` (unpushed). The superproject pin `fc533e9` is unreachable for fresh clones until `git -C spec push origin main` is run.
- Note: 04-08-03 command requires `DEAL_BIN` pointing at a built `deal` binary (command updated in the map above).

**Result: Phase 4 is Nyquist-compliant — all 23 automated verifications green; 3 documented manual-only behaviors remain (DOORS import, production DNS, UI-SPEC visual review).**
