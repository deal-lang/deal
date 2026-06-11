---
phase: 03-editor-intelligence
plan: 05
plan_id: 03-05
subsystem: editor-intelligence
tags: [vscode-extension, language-client, lsp-wiring, status-bar, d-40, issue-5]
dependency_graph:
  requires:
    - REQ-phase-3-1-textmate          # Plan 03-01 vscode-deal scaffold (package.json, language config, snippets, icons)
    - REQ-phase-3-3-lsp-server        # Plan 03-03 + 03-04 deal-lsp 5-capability server
    - vscode-languageclient@9         # Microsoft official LanguageClient
  provides:
    - extension activation pipeline    # activate() → resolve binary → create client → install status bar → register commands → client.start()
    - LanguageClient factory           # client.ts: ServerOptions(stdio) + LanguageClientOptions(deal/dealx + deal.toml watcher + DEAL Language Server output channel)
    - 3-tier binary resolution         # binary.ts: configured (deal.lsp.path) → bundled (server/) → Plan 06 auto-download stub
    - status bar pure-logic seam       # status.ts: updateStatusBar(state) -> { text, tooltip, command? } (Issue 5 fast-path)
    - status bar vscode wrapper        # status_bar.ts: subscribes client.onDidChangeState, applies updateStatusBar result
    - commands                         # deal.restartServer + deal.showOutput
    - unit test fast path              # .mocharc.unit.json + npm run test:unit, sub-10ms, no electron (Issue 5)
    - electron suite scaffold          # src/test/runTest.ts + suite/index.ts + suite/activation.test.ts (2 graceful-skip tests)
  affects:
    - REQ-phase-3-4-vscode-lsp        # Completed by this plan
    - REQ-phase-3-gate                # Activation + lifecycle behaviors now have runnable tests for the phase gate
tech_stack:
  added:
    - vscode-languageclient:9.0.1     # dependency, pinned exact (T-3-03 supply chain)
    - "@types/vscode:1.95.0"          # devDep, matches package.json engines.vscode
    - "@vscode/test-electron:2.5.2"   # electron test driver
    - "@vscode/test-cli:0.0.12"
    - sinon:22.0.0 + @types/sinon:21.0.1
    - eslint:9.0.0 + @typescript-eslint:8.60.0
  patterns:
    - "D-40 silent-fallback: extension.ts wraps client.start() in try/catch; failures log to output channel and surface via status bar — never a popup"
    - "Issue 5 logic-seam split: status.ts (pure data, vscode-free) + status_bar.ts (vscode wrapper). Unit tests cover the mapper; electron tests cover the wrapper."
    - "Three-tier binary resolution with CWE-22 taint-free design: configured value is escape-hatch (accept); bundled path uses compile-time literals joined via vscode.Uri.joinPath"
    - "Activation returns {client, output, statusBar} so electron tests can introspect lifecycle without exporting internal state"
    - "tsconfig split: main src-only (vsce package bundle); test/tsconfig.json for legacy Plan 01 snapshot harness; src/test/** picked up by main include for electron build"
key_files:
  created:
    - /Users/dunnock/projects/deal-lang/vscode-deal/src/extension.ts                # 76 lines — activation + deactivation
    - /Users/dunnock/projects/deal-lang/vscode-deal/src/client.ts                   # 60 lines — LanguageClient factory
    - /Users/dunnock/projects/deal-lang/vscode-deal/src/binary.ts                   # 106 lines — 3-tier resolver
    - /Users/dunnock/projects/deal-lang/vscode-deal/src/status.ts                   # 82 lines — pure-logic mapper (Issue 5)
    - /Users/dunnock/projects/deal-lang/vscode-deal/src/status_bar.ts               # 50 lines — vscode wrapper
    - /Users/dunnock/projects/deal-lang/vscode-deal/src/commands.ts                 # 59 lines — restartServer + showOutput
    - /Users/dunnock/projects/deal-lang/vscode-deal/src/test/runTest.ts             # electron test driver
    - /Users/dunnock/projects/deal-lang/vscode-deal/src/test/suite/index.ts         # 37 lines — mocha suite loader
    - /Users/dunnock/projects/deal-lang/vscode-deal/src/test/suite/activation.test.ts   # 114 lines — 2 graceful-skip electron tests
    - /Users/dunnock/projects/deal-lang/vscode-deal/src/test/unit/status.test.ts    # 76 lines — 9 unit tests
    - /Users/dunnock/projects/deal-lang/vscode-deal/.mocharc.unit.json              # unit-only mocha config
    - /Users/dunnock/projects/deal-lang/vscode-deal/test/tsconfig.json              # legacy test/ IDE type-check config
  modified:
    - /Users/dunnock/projects/deal-lang/vscode-deal/package.json                    # deps + scripts (test/test:unit/test:grammars/compile/lint)
    - /Users/dunnock/projects/deal-lang/vscode-deal/tsconfig.json                   # reverted to src-only (rootDir + exclude test) per user; legacy test/ uses test/tsconfig.json
decisions:
  - "D-40 silent-fallback policy implemented in extension.ts — no popups on LSP start failure; all error surface is the output channel + status bar"
  - "Issue 5 resolved: status logic in pure module status.ts (no vscode import); vscode wrapper in status_bar.ts. Unit-test fast path runs in <10ms; electron suite exercises the wrapper end-to-end"
  - "binary.ts auto-download is a Plan 06 TODO; current resolution is configured → bundled (./server/) → null (which triggers status-bar error per D-40)"
  - "Activation returns {client, output, statusBar} so electron tests can introspect lifecycle — exposes test seam without breaking the extension's public API surface"
  - "tsconfig split — main include = src/**/*.ts (covers src/test/* electron suite); test/ legacy snapshot harness uses its own test/tsconfig.json with node+mocha types. Keeps vsce package bundle clean while letting IDE type-check everything"
metrics:
  duration: "interrupted, ~1h cumulative across two executor attempts"
  completed: "2026-05-25"
  tasks: 2
  files_created: 12
  files_modified: 2
  commits: 2
---

# Phase 3 Plan 05: VS Code Extension LSP Wiring Summary

**One-liner:** Closes REQ-phase-3-4-vscode-lsp by wiring `vscode-deal` to consume the `deal-lsp` server built in Plans 03-03/04 — extension.ts orchestrates activation (resolve binary → create LanguageClient with stdio transport → install status bar driven by a pure-logic mapper → register restartServer/showOutput commands → start client with D-40 silent-fallback), and adds a fast unit-test path (`npm run test:unit`, 9 passing in <10 ms) plus an electron suite scaffold (`npm test`, graceful-skip when the LSP binary isn't on PATH).

## What was built

### Task 1 (commit `1c82173`) — LSP wiring scaffold (extension + client + binary + Task-1 stubs)

- **`src/extension.ts`** (~76 LOC) — `activate(context)`:
  - resolves the LSP binary via `resolveBinary(context)`
  - if null, surfaces "DEAL LSP not found" via status bar and returns gracefully (D-40)
  - otherwise calls `createClient(binaryPath, context)`, installs `StatusBarItem` (subscribes to `client.onDidChangeState`), registers commands, then `await client.start()` inside a try/catch. On start failure: log to the output channel; status bar stays in `stopped` state; NO popup.
  - returns `{ client, output, statusBar }` so electron tests can introspect lifecycle state without breaking the extension's public API surface.
  - `deactivate()` is a no-op — lifetime is owned by the status-bar disposable in `context.subscriptions`.
- **`src/client.ts`** (~60 LOC) — pure factory returning an UNSTARTED `LanguageClient`:
  - `ServerOptions { run, debug }` both use `TransportKind.stdio` per RESEARCH §5.
  - `LanguageClientOptions { documentSelector: [{scheme:"file",language:"deal"}, {scheme:"file",language:"dealx"}], synchronize: { fileEvents: workspace.createFileSystemWatcher("**/deal.toml") }, outputChannelName: "DEAL Language Server" }`.
  - Returns the client without starting it so the caller can wrap `start()` in try/catch (D-40).
- **`src/binary.ts`** (~106 LOC) — three-tier resolution:
  1. `vscode.workspace.getConfiguration('deal.lsp').get<string>('path')` — escape hatch (T-3-04 disposition: accept; user-controlled path used verbatim).
  2. Bundled `<extensionPath>/server/{deal-lsp.exe | deal-lsp}` — constructed via two compile-time literals joined with `vscode.Uri.joinPath` (CWE-22 taint-free, mirrors Plan 01 frozen-Record pattern).
  3. Plan 06 TODO: auto-download from GitHub Releases with SHA-256 verification. Sketch + contract documented inline.
- **`src/status_bar.ts` + `src/commands.ts`** — Task 1 STUBS so `extension.ts` compiles cleanly under the per-task gate. Task 2 overwrites both with real implementations.
- **`package.json`** — adds `dependencies.vscode-languageclient@9.0.1`; devDeps `@types/vscode@1.95.0`, `@vscode/test-cli@0.0.12`, `@vscode/test-electron@2.5.2`, `sinon@22.0.0`, `@types/sinon@21.0.1`, `eslint@9.0.0`, `@typescript-eslint/*@8.60.0`. All pinned exact for T-3-03 supply-chain reproducibility. Script split: `test` (electron via `vscode-test`), `test:unit` (mocha — Issue 5 fast path), `test:grammars` (legacy Plan 01 TextMate snapshot harness).

### Task 2 (commit `8467c22`) — status bar + activation suite + unit-test fast path

- **`src/status.ts`** (82 LOC) — pure-logic mapper, vscode-free:
  - `LspState = "starting" | "running" | "stopped"` union.
  - `updateStatusBar(state) -> { text, tooltip, command? }`: 3-arm match → status-bar data.
  - `fromLspState(numeric) -> LspState`: normalizes `vscode-languageclient`'s numeric `State` enum (`Stopped=1 / Running=2 / Starting=3`) to the stable union; fail-safe to `"stopped"` for unknown variants.
  - Exhaustiveness guard (`const _exhaustive: never = state`) keeps the compiler honest when new variants are added.
- **`src/status_bar.ts`** (50 LOC, real impl) — `StatusBarItem` wrapper:
  - Wraps `vscode.window.createStatusBarItem(StatusBarAlignment.Right, 100)`.
  - Subscribes to `client.onDidChangeState`; on each transition calls `fromLspState` → `updateStatusBar` → applies `{ text, tooltip, command }` to the item.
  - D-40 surface: errors here, never popups.
- **`src/commands.ts`** (59 LOC, real impl):
  - `deal.restartServer`: `await client.stop(); await client.start();` — does NOT re-resolve binary (uses existing).
  - `deal.showOutput`: focuses the LanguageClient output channel.
- **`src/test/unit/status.test.ts`** (76 LOC) — 9 mocha unit tests covering both `updateStatusBar` (3 state branches + plain-data invariant) and `fromLspState` (5 mappings + type-narrowing). Runs in <10 ms via `.mocharc.unit.json` (no electron, no `vscode` runtime).
- **`src/test/suite/activation.test.ts`** (114 LOC) — 2 electron tests using `vscode-test`:
  - `extension_activates_on_deal_file` — opens a `.deal` file from the showcase, asserts `extension.isActive === true`.
  - `language_client_connects` — gracefully skips when `deal-lsp` binary isn't resolvable (`exp.client === null` → `this.skip()`); otherwise asserts `client.state === State.Running` within 10s.
  - Uses a regular `function` expression (not arrow) so `this.skip()` binds to mocha's `Context`.
- **`src/test/suite/index.ts`** (37 LOC) — Mocha suite loader: discovers `*.test.js` under `out/test/suite/`, runs with 60s timeout.
- **`src/test/runTest.ts`** — `@vscode/test-electron` driver: downloads VS Code stable, opens `spec/examples/showcase/` as workspace folder, loads compiled extension, runs the suite.
- **`.mocharc.unit.json`** — unit-test config (`require: ts-node/register`, `spec: src/test/unit/**/*.test.ts`, `timeout: 2000`).
- **`tsconfig.json`** — reverted to the original `include: ["src/**/*.ts"]` + `exclude: [..., "test"]` + `rootDir: "./src"` per user's intentional edit. `src/test/**` is still covered by the main include (electron tests compile to `out/test/`). Legacy `test/tmLanguage.snapshot.test.ts` (Plan 01) is covered by a new `test/tsconfig.json` so IDE diagnostics resolve. Both `tsc -p ./` and `tsc -p ./test` exit clean.

## Verification

- `(cd vscode-deal && npx tsc -p ./ --noEmit)` — exits 0
- `(cd vscode-deal && npx tsc -p ./test --noEmit)` — exits 0
- `(cd vscode-deal && npm run test:unit)` — **9/9 unit tests pass** in <10 ms (status_bar_transitions + fromLspState suites)
- `(cd vscode-deal && npx mocha --require ts-node/register test/tmLanguage.snapshot.test.ts)` — **6/6 Plan 01 snapshot tests still pass** (regression-free under the tsconfig split)
- Gate per 03-VALIDATION.md row 03-05-T1: `grep -q "vscode-languageclient/node" src/client.ts && grep -q "TransportKind.stdio" src/client.ts` — both match
- Gate per 03-VALIDATION.md row 03-05-T2a: `! grep -q "from 'vscode'" src/status.ts && grep -q "updateStatusBar" src/status.ts` — both pass (pure module)

## Deviations

1. **(Rule-4, surfaced to user, resolved)** — Plan 04 added `.semgrepignore` exemption for `lsp/src/workspace.rs`; **no new exemptions needed for Plan 05** (Plan 05 is TypeScript; the path-handling in `binary.ts` is taint-free by design — compile-time literals + `vscode.Uri.joinPath`).
2. **(Rule-3, user-driven)** — User reverted `vscode-deal/tsconfig.json` to its src-only shape after the Plan 01 followup had broadened it to include `test/**`. To keep IDE type-checking for the legacy `test/tmLanguage.snapshot.test.ts`, added `vscode-deal/test/tsconfig.json` as a nested project config (with `types: ["node", "mocha"]`). The main `tsconfig.json` still covers `src/test/**` (Plan 05 electron + unit tests) via its `src/**/*.ts` include.
3. **(Rule-1, fix-during-execution)** — Plan 05 Task 2's `activation.test.ts` was written with an arrow function (`it('...', async () => {...})`) and then referenced `this.skip()` — `this` binds to the enclosing scope, not mocha's `Context`. Fixed by switching to a regular function expression. Then dropped the redundant `return` after `this.skip()` (mocha's `skip()` is typed as `() => never`).
4. **(Logistical)** — Two executor attempts:
   - First attempt was interrupted by `API Error: socket connection closed unexpectedly` after writing all Task 2 files but before committing or running the verification suite.
   - Continued inline by the orchestrator (this run): ran `npm install`, added the test/tsconfig.json split, fixed the `this.skip()` bug, verified all gates, committed Task 2 + tsconfig split as a single coherent commit (`8467c22`), wrote this SUMMARY.

## Plan-06 readiness

- `binary.ts` Tier 3 (auto-download) is a stubbed TODO with the bootstrap.ts contract sketch.
- `package.json` already declares `dependencies.vscode-languageclient@9.0.1` so the LanguageClient runtime is in `node_modules` for `vsce package` to bundle in Plan 06.
- The electron suite (`src/test/suite/activation.test.ts`) gracefully skips when `deal-lsp` isn't on PATH — Plan 06's CI workflow can build deal-lsp, set `deal.lsp.path`, and the tests will fully execute.
- `.vscodeignore` (from Plan 01) already excludes `test/`, `src/test/`, `*.test.ts`, `tsconfig.json` — keeps test code out of the published `.vsix`.

## State after completion

- Phase 3 plan position: 5 → 6 (of 7) — **Wave 2 complete**
- REQ-phase-3-4-vscode-lsp: ☐ → ✔
- Decisions logged: D-40 silent-fallback wire pattern; Issue 5 logic-seam split; tsconfig split for vsce-clean bundle
- Resume signal: "Wave 2 complete; pause for Plan 06 manual setup (VSCE_PAT + Marketplace environment)"
