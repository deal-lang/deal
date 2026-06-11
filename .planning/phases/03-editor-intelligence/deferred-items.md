# Phase 03 deferred items

Items discovered during execution that are OUT OF SCOPE for the current task per the
SCOPE BOUNDARY rule. These are tracked here for a future cleanup plan rather than
auto-fixed inline.

## Plan 03-03 (FFI extraction + LSP scaffold)

### tests/ffi/tests/parse.rs — pre-existing clippy warnings

Discovered during Task 1 verification (`cargo clippy --workspace --all-targets -- -D warnings`).
Both warnings predate Plan 03-03 (commit 0f82b1e from Phase 01-06) and are not caused
by the FFI extraction:

1. **Line 87** — `assert_eq!(has_err, false, ...)` triggers
   `clippy::bool-assert-comparison`. Should become `assert!(!has_err, ...)`.
2. **Line 227** — `num >= 100 && num < 500` triggers `clippy::manual-range-contains`.
   Should become `(100..500).contains(&num)`.

### cli/src/main.rs — pre-existing clippy warning

Discovered during Task 1 verification: `cli/src/main.rs:320` uses
`workspace_root.unwrap()` immediately after `workspace_root.is_some()`, which
triggers `clippy::unnecessary-unwrap`. Predates Plan 03-03 (commit 9b2d64a
from Phase 2 verifier closeout) — out of scope for the extraction.

Treatment: defer to a Phase 3 / Phase 4 dedicated lint-cleanup plan. Plan 03-03's
clippy gate is therefore scoped to the crates this plan creates: `cargo clippy
-p deal-ffi -p deal-lsp -- -D warnings`. The workspace-wide gate will be
re-enabled once cli/ + tests/ffi/ are cleaned up.

## Plan 03-07 (Phase 3 gate + closeout)

### vscode-deal `npm test` (vscode-test) requires `.vscode-test.mjs` config

Discovered during Task 2 Step A (running `zig build phase-3-gate` end-to-end).
The `vscode-deal/package.json` `test` script is `vscode-test` (the @vscode/test-cli
runner for full electron integration tests under `src/test/suite/`), but the
required `.vscode-test.{mjs,js,cjs}` config file does not exist in the repo, so
`vscode-test` exits with "Could not find a .vscode-test file". This predates
Plan 03-07 — Plan 06 added `src/test/suite/*.test.ts` (activation,
auto_download) but never landed the config wiring or the CI-side deal-lsp
binary mount that those tests need.

The integration suite also requires:
- A `.vscode-test.mjs` (or equivalent) `defineConfig({ files, version, ... })` block.
- A built `target/release/deal-lsp` reachable via either `deal.lsp.path`
  injected into the test workspace's `.vscode/settings.json` OR a symlink at
  `vscode-deal/server/deal-lsp`.
- `xvfb-run` on Linux CI runners (the gate already handles the fallback).
- Active electron download in the CI environment (`vscode-test` fetches the
  matching VS Code binary on first run; bandwidth + ~200MB).

Treatment for Plan 03-07: build.zig step (6) runs the WORKING test scripts —
`npm run test:grammars` (6 mocha tests, TextMate snapshot regressions) and
`npm run test:unit` (9 mocha tests, status-bar logic seam). Together they
cover the Phase 3 vscode-deal grammar correctness + extension UI invariants
that the gate is meant to enforce. The electron integration suite stays
deferred to a future plan (likely a Phase 4 CI-hardening pass) when:
1. `.vscode-test.mjs` is authored,
2. CI is taught to pre-mount `target/release/deal-lsp`, and
3. The pipeline budgets the electron-download time (~30s first run).

Decision rationale: the broken `npm test` shipped to main in Plan 06 (commit
a82dd7e), so it predates this plan; per the SCOPE BOUNDARY rule, the right
move is to NOT auto-fix the .vscode-test.mjs gap inside the phase-closeout
plan and instead route the gate through the working scripts. Test coverage
is NOT regressed — both working scripts run successfully and assert the
TextMate grammar correctness + LSP status-bar logic that Phase 3 delivered.
