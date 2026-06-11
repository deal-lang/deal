---
phase: 3
slug: editor-intelligence
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-05-22
backfilled: 2026-05-22
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
>
> Source: §15 of `.planning/phases/03-editor-intelligence/03-RESEARCH.md` (Validation Architecture).
> Phase 3 deliverables span 4 packages (vscode-deal, tree-sitter-deal, deal-lsp Rust crate, TextMate grammars). Validation surface is correspondingly multi-stack.
>
> **Backfill 2026-05-22:** Per-Task Verification Map populated from `<automated>` blocks across all 7 PLAN.md files (Issue 3 resolution); `nyquist_compliant` and `wave_0_complete` flipped to true; Validation Sign-Off ticked.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework — Rust (deal-lsp + cli)** | `cargo test` (workspace) + `insta` for snapshots + `tokio::test` for LSP server tests |
| **Framework — tree-sitter-deal** | `tree-sitter test` (corpus tests) + `tree-sitter highlight` snapshot tests |
| **Framework — vscode-deal (electron)** | `@vscode/test-electron` (headless VS Code instance; xvfb on Linux CI) + `mocha` + `chai` |
| **Framework — vscode-deal (unit)** | Pure `mocha` via `.mocharc.unit.json` — no electron, no xvfb (Issue 5 split; ~1s feedback for status-bar logic) |
| **Framework — Zig core** | Existing `zig build test` (re-asserted via phase-2-gate) |
| **Config files** | `deal/Cargo.toml` (workspace), `tree-sitter-deal/package.json`, `vscode-deal/package.json`, `vscode-deal/.mocharc.unit.json`, `vscode-deal/.vscode-test.mjs`, `deal/build.zig` (phase-3-gate steps) |
| **Quick run command** | `cd deal && cargo test --workspace --lib` (Rust unit tests only; ~2–5s) |
| **Quick unit (vscode-deal)** | `cd vscode-deal && npm run test:unit` (~1s) |
| **Full suite command** | `cd deal && zig build phase-3-gate` (Zig + Rust + tree-sitter + vscode-deal) |
| **Fresh-worktree command** | `cd deal && zig build phase-3-gate-fresh` (binding ADR-phase-1.5-fresh-worktree-verification) |
| **Estimated quick runtime** | ~5 seconds (Rust unit tests); ~1 second (vscode-deal unit) |
| **Estimated full runtime** | ~3–8 minutes (depends on Marketplace dry-run + xvfb) |

---

## Sampling Rate

- **After every task commit:** Run quick command appropriate for the modified package:
  - Rust: `cd deal && cargo test --workspace --lib`
  - tree-sitter-deal: `cd tree-sitter-deal && npm test`
  - vscode-deal status logic: `cd vscode-deal && npm run test:unit` (~1s — Issue 5 fast path)
  - vscode-deal full integration: `cd vscode-deal && npm test` (~30s — electron)
  - TextMate grammars: visual smoke in VS Code dev host (F5)
- **After every plan wave:** Run `cd deal && zig build phase-3-gate`
- **Before `/gsd:verify-work`:** Both `phase-3-gate` and `phase-3-gate-fresh` must be green
- **Max feedback latency:** 30 seconds for quick command; 8 minutes for full suite

---

## Per-Task Verification Map

> Backfilled 2026-05-22 from each PLAN.md's `<automated>` blocks (Issue 3 resolution).
> Test Type classification: **build-invariant** = manifest/structural check; **unit** = single-module logic test; **integration** = multi-module + real subprocess; **snapshot** = byte-for-byte golden compare; **e2e** = full stack via deal-lsp subprocess; **smoke** = end-to-end gate-level run; **regression** = phase-2-gate inheritance.

| Task ID | Plan | Wave | Requirement | Threat Ref | Test Type | Automated Command |
|---------|------|------|-------------|------------|-----------|-------------------|
| 03-01-T1 | 03-01 | 1 | REQ-phase-3-1-textmate | T-3-03, T-3-06 | build-invariant + snapshot | `cd vscode-deal && node -e "manifest invariants" && node -e "snippet keys" && grep -c 'keyword.control.element.deal' COLOR-CATEGORIES.md && xmllint --noout icons/*.svg` |
| 03-01-T2 | 03-01 | 1 | REQ-phase-3-1-textmate | T-3-03, T-3-06 | snapshot | `cd vscode-deal && node -e "tmLanguage scope invariants" && npm install && npx mocha --require ts-node/register test/tmLanguage.snapshot.test.ts` |
| 03-02-T1 | 03-02 | 1 | REQ-phase-3-2-treesitter | T-3-03, T-3-06 | build-invariant | `cd tree-sitter-deal && npm install && npx tree-sitter generate && test -f src/parser.c && node -e "grammar.js OK" && wc -l grammar.js` |
| 03-02-T2 | 03-02 | 1 | REQ-phase-3-2-treesitter | T-3-03, T-3-06 | integration + snapshot | `cd tree-sitter-deal && npx tree-sitter generate && npx tree-sitter test && bash scripts/showcase-parse.sh && grep -cE '@keyword.element\|@type.definition\|...' queries/highlights.scm` |
| 03-03-T1 | 03-03 | 1 | REQ-phase-3-3-lsp-server | T-3-03, T-3-05 | build-invariant + unit | `cd deal && cargo build --workspace --release && cargo test --workspace --lib && grep -q 'links = "deal"' deal-ffi/Cargo.toml && ! grep -q 'links = "deal"' cli/Cargo.toml && grep -q 'tower-lsp = { version = "0.20.0"' Cargo.toml` |
| 03-03-T2 | 03-03 | 1 | REQ-phase-3-3-lsp-server | T-3-04, T-3-05 | unit | `cd deal && cargo build -p deal-lsp --release && cargo test -p deal-lsp --lib && grep -q 'DEBOUNCE_MS: u64 = 300' lsp/src/backend.rs && grep -q 'impl LanguageServer for Backend' lsp/src/backend.rs` |
| 03-03-T3 | 03-03 | 1 | REQ-phase-3-3-lsp-server | T-3-04, T-3-05 | integration + snapshot | `cd deal && test -f lsp/tests/golden/formatted/BatteryPack.deal.expected && cargo test -p deal-lsp --test showcase -- --test-threads=1 && grep -q 'include_str!.*BatteryPack.deal.expected' lsp/tests/showcase.rs && ! grep -q 'UPDATE_GOLDEN' lsp/tests/showcase.rs` |
| 03-04-T1 | 03-04 | 2 | REQ-phase-3-3-lsp-server | T-3-04 | unit | `cd deal && cargo build -p deal-lsp --release && cargo test -p deal-lsp --lib && grep -q 'DashMap<String' lsp/src/index.rs && grep -q 'eager_parse' lsp/src/workspace.rs && grep -q 'workspace_symbol_provider' lsp/src/backend.rs` |
| 03-04-T2 | 03-04 | 2 | REQ-phase-3-3-lsp-server | T-3-04 | unit | `cd deal && cargo build -p deal-lsp --release && cargo test -p deal-lsp --lib && grep -q 'CompletionItemKind' lsp/src/completion.rs && grep -q 'HoverContents' lsp/src/hover.rs && grep -q 'GotoDefinitionResponse' lsp/src/definition.rs` |
| 03-04-T3 | 03-04 | 2 | REQ-phase-3-3-lsp-server | T-3-04, T-3-05 | unit + snapshot | `cd deal && cargo build -p deal-lsp --release && cargo build --release -p deal-lsp --bin deal-lsp-gen-golden && grep -q 'encode_for_source' lsp/src/semantic_tokens.rs && grep -q 'DEAL_LSP_DETERMINISTIC_RESULT_ID' lsp/src/semantic_tokens.rs && cargo run --release -p deal-lsp --bin deal-lsp-gen-golden -- ... && test -f lsp/tests/golden/semantic_tokens/BatteryPack.deal.expected.json` |
| 03-04-T4 | 03-04 | 2 | REQ-phase-3-3-lsp-server | T-3-04, T-3-05 | integration + snapshot | `cd deal && cargo test -p deal-lsp --test showcase -- --test-threads=1 && grep -c 'tokio::test' lsp/tests/showcase.rs >= 8 && grep -q 'include_str!.*golden/semantic_tokens' lsp/tests/showcase.rs && ! grep -q 'UPDATE_GOLDEN' lsp/tests/showcase.rs` |
| 03-05-T1 | 03-05 | 2 | REQ-phase-3-4-vscode-lsp | T-3-03, T-3-04 | build-invariant | `cd vscode-deal && npm install && npx tsc -p ./ --noEmit && grep -q "vscode-languageclient/node" src/client.ts && grep -q "TransportKind.stdio" src/client.ts && node -e "package.json deps + scripts OK"` |
| 03-05-T2a | 03-05 | 2 | REQ-phase-3-4-vscode-lsp | T-3-03 | unit | `cd vscode-deal && npx tsc -p ./ && test -f out/status.js && test -f out/test/unit/status.test.js && ! grep -q "from 'vscode'" src/status.ts && grep -q "updateStatusBar" src/status.ts && npm run test:unit` |
| 03-05-T2b | 03-05 | 2 | REQ-phase-3-4-vscode-lsp | T-3-03, T-3-04, T-3-01 | e2e (electron) | `cd vscode-deal && (cd ../deal && cargo build -p deal-lsp --release) && (xvfb-run -a npm test 2>/dev/null \|\| npm test)` |
| 03-06-T1 | 03-06 | 3 | REQ-phase-3-4-vscode-lsp | T-3-01, T-3-03 | unit + e2e (electron) | `cd vscode-deal && npm install && npx tsc -p ./ --noEmit && grep -q "DEAL_LSP_VERSION" src/bootstrap.ts && grep -q "createHash" src/sha256.ts && grep -q "ensureDealLspBinary" src/binary.ts && ! grep -q "TODO .Plan 06." src/binary.ts && (cd ../deal && cargo build -p deal-lsp --release) && (xvfb-run -a npm test \|\| npm test)` |
| 03-06-T2 | 03-06 | 3 | REQ-phase-3-4-vscode-lsp | T-3-01, T-3-02, T-3-03, T-3-07 | build-invariant | `cd deal && test -f .github/workflows/release.yml && test -f SHA256SUMS.template && python3 -c "import yaml; ... 5 jobs present" && grep -q "aarch64-macos\|x86_64-linux-musl\|win-x64" .github/workflows/release.yml && grep -q "environment: marketplace" .github/workflows/release.yml && grep -q "VSCE_PAT" .github/workflows/release.yml && grep -q "phase-3-gate-fresh" .github/workflows/release.yml` |
| 03-07-T1 | 03-07 | 4 | REQ-phase-3-gate | T-3-SC | smoke + build-invariant | `cd deal && grep -q 'phase-3-gate' build.zig && grep -q 'phase-3-gate-fresh' build.zig && test -x scripts/phase-3-smoke.sh && cargo build --release -p lsp-smoke && cargo build --release -p deal-lsp && bash scripts/phase-3-smoke.sh && ! grep -A 2 'phase-3-gate-fresh' .github/workflows/release.yml \| grep -q 'continue-on-error: true'` |
| 03-07-T2 | 03-07 | 4 | REQ-phase-3-gate, REQ-phase-3-editor-intelligence | T-3-07 | regression + smoke (gate) | `cd deal && zig build phase-3-gate && zig build phase-3-gate-fresh && grep -q 'DEFER-textmate-vs-treesitter-vscode.*RESOLVED' .planning/REQUIREMENTS.md && grep -c 'REQ-phase-3.*Completed' .planning/REQUIREMENTS.md >= 6 && grep -q 'Phase 3 COMPLETE 2026-05-22' .planning/STATE.md && grep -q '- \[x\] **Phase 3' .planning/ROADMAP.md` |

**Status legend:** ⬜ pending · ✅ green · ❌ red · ⚠️ flaky.
All rows are ⬜ pending pre-execution (planning artifact). Execute-phase flips them per task completion. The contract verified here is that every task has an `<automated>` command and each command is non-trivial — i.e. Nyquist-compliant per the sign-off checklist below.

**Row count:** 18 task rows across 7 plans (Plan 05 Task 2 contributes two rows — 2a unit, 2b electron — per Issue 5 split, distinguishing the ~1s mocha unit suite from the ~30s electron suite).

---

## Wave 0 Requirements

Phase 3 starts from scaffold-only packages. Wave 0 set up the test harnesses themselves. All items below are now SATISFIED by Plan 03 (Cargo workspace + deal-lsp/tests/showcase.rs), Plan 05 (vscode-deal test harness + unit suite per Issue 5), Plan 02 (tree-sitter-deal package.json + corpus), and Plan 07 (phase-3-gate + phase-3-smoke.sh).

- [x] `deal/Cargo.toml` — `"lsp"`, `"deal-ffi"`, `"lsp-smoke"` workspace members added by Plan 03 (lsp + deal-ffi) and Plan 07 (lsp-smoke)
- [x] `deal/lsp/Cargo.toml` — `deal-lsp` crate with `tower-lsp` (pinned 0.20.0 per Issue 1), `tokio`, `serde_json`, `dashmap` deps (Plan 03 Task 2)
- [x] `deal/lsp/tests/showcase.rs` — `tokio::test` harness that spawns deal-lsp via in-process channels (Plan 03 Task 3; extended in Plan 04 Task 4)
- [x] `deal/lsp/tests/golden/formatted/BatteryPack.deal.expected` — committed golden for format_round_trip (Plan 03 Task 3 per Issue 6)
- [x] `deal/lsp/tests/golden/semantic_tokens/BatteryPack.deal.expected.json` — committed golden for semantic_tokens_match_golden (Plan 04 Task 3 per Issue 6, generated by `deal-lsp-gen-golden` binary)
- [x] `tree-sitter-deal/package.json` — npm package with `tree-sitter-cli` devDep, `test/corpus/` scaffold, `queries/*.scm` skeletons (Plan 02 Task 1)
- [x] `tree-sitter-deal/test/corpus/*.txt` — corpus tests covering the 19 showcase files (Plan 02 Task 2)
- [x] `vscode-deal/package.json` — VS Code extension manifest with `@vscode/test-electron`, `@types/vscode`, `mocha`, `chai`, `vscode-languageclient` deps (Plan 05 Task 1)
- [x] `vscode-deal/.mocharc.unit.json` — pure-mocha config for the unit suite added by Plan 05 Task 2 (Issue 5 split)
- [x] `vscode-deal/src/test/runTest.ts` — `@vscode/test-electron` runner entry point (Plan 05 Task 2)
- [x] `vscode-deal/src/test/suite/activation.test.ts` — extension activation + LSP wiring tests, 3 electron tests (Plan 05 Task 2)
- [x] `vscode-deal/src/test/unit/status.test.ts` — pure-mocha unit test for status.ts (Plan 05 Task 2 per Issue 5 — replaces electron-based status_bar_transitions)
- [x] `deal/build.zig` — `phase-3-gate` and `phase-3-gate-fresh` build steps (Plan 07 Task 1)
- [x] `deal/scripts/phase-3-smoke.sh` — end-to-end smoke that spawns deal-lsp, opens showcase project, asserts LSP responses (Plan 07 Task 1)
- [x] `deal/lsp-smoke/src/main.rs` — Cargo workspace member implementing the 5-capability smoke (Plan 07 Task 1)

---

## Manual-Only Verifications

> Phase 3 is heavily automated — most visual checks reduce to snapshot tests. The list below is the irreducible manual surface.

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| TextMate scopes render with sensible colors under Dark+ / Light+ / GitHub Dark themes | REQ-phase-3-1-textmate | Theme-specific color rendering is subjective and cannot be unit-tested; snapshot tests cover scope-tag emission, not visual output | Open `spec/examples/showcase/sedan_project/vehicle/electrical/BatteryPack.deal` in VS Code dev host under each of Dark+, Light+, GitHub Dark, GitHub Light. Confirm element keywords, operator `<<…>>`, composition tags `[<…>]`, multiplicity `[1..*]`, and `@trace`/`@simulation` annotations are visually distinct. |
| File icons render correctly in the explorer | REQ-phase-3-1-textmate | Icon-theme contract is loaded by VS Code shell, not the extension JS | Open showcase project in VS Code dev host, confirm `.deal` shows the blueprint icon and `.dealx` shows the JSX-composition icon. |
| Format-on-save UX in dirty buffer | REQ-phase-3-3-lsp-server | LSP edit-application UX involves VS Code core; integration test asserts response correctness, not editor visual behavior | Open a showcase file, make a whitespace edit, save (Cmd+S / Ctrl+S). Confirm `deal-lsp` reformats without flicker and cursor stays on the intended line. |
| Status-bar transitions (`starting…` → `ready` → `error (click for output)`) | REQ-phase-3-4-vscode-lsp / D-40 | Pure-function state mapping is now unit-tested (Issue 5 split; status.ts unit suite). Visual transition feel best confirmed live. | Cold-open showcase project; confirm status bar shows `DEAL LSP: starting…` then transitions to `DEAL LSP: ready`. Stop the bundled binary; confirm transition to `DEAL LSP: error (click for output)` and clicking opens the output channel. |
| Cross-file go-to-definition feels instant on showcase | REQ-phase-3-gate | "Comparable to TypeScript in VS Code" is a UX claim; LSP integration test asserts correctness but not perceived latency | In showcase, `Cmd-click` / `F12` a definition reference that lives in another package. Confirm definition opens within ~1s. |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies (verified by Per-Task Verification Map — 18 rows, every task `<automated>` block present)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify (every task in every plan carries an `<automated>` block)
- [x] Wave 0 covers all MISSING references (see Wave 0 Requirements above — all 15 boxes checked)
- [x] No watch-mode flags (use `cargo test`, not `cargo watch`)
- [x] Feedback latency < 30s (quick command) / < 8min (full suite) — unit suite for status.ts is ~1s per Issue 5
- [x] `nyquist_compliant: true` set in frontmatter (planner backfilled 2026-05-22 — Issue 3)
- [x] `wave_0_complete: true` set in frontmatter (planner backfilled 2026-05-22 — Issue 3)

**Approval:** approved 2026-05-22 — Per-Task Verification Map backfilled from all 7 PLAN.md `<automated>` blocks; nyquist_compliant + wave_0_complete frontmatter flipped per Issue 3 resolution; phase-3-gate authors finalize sign-off when /gsd:verify-phase 3 runs at Phase 3 closeout.
