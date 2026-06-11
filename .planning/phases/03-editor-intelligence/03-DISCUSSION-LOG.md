# Phase 3: Editor Intelligence - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in `03-CONTEXT.md` — this log preserves the alternatives considered.

**Date:** 2026-05-22
**Phase:** 3-editor-intelligence
**Areas discussed:** VS Code highlighting strategy, LSP process & state model, Workspace symbol index strategy, deal-lsp binary distribution

---

## VS Code highlighting strategy

### Q1: What's the primary highlighting source for the vscode-deal extension?

| Option | Description | Selected |
|--------|-------------|----------|
| TextMate + LSP semantic tokens | TextMate provides baseline; LSP enriches with type-aware highlights. Modern VS Code pattern (rust-analyzer / ts-server / gopls). | ✓ |
| TextMate-only (Stage 1) | Ship just TextMate + snippets + brackets + icons. No LSP highlighting. | |
| VS Code experimental tree-sitter | Use VS Code's flag-gated tree-sitter support; sharper but flag-gated UX and two parser sources of truth. | |

**Captured as:** D-38

---

### Q2: How aggressive should the LSP's semantic tokens overlay be?

| Option | Description | Selected |
|--------|-------------|----------|
| Full structural overlay | Cover keyword/type/parameter/variable/property/namespace/operator/enumMember/regexp + modifiers. | ✓ |
| Type-aware identifiers only | Color identifier references by resolved type; TextMate carries rest. | |
| Errors-only overlay | Semantic tokens only mark unresolved/erroneous identifiers. | |

**Captured as:** D-39

---

### Q3: Fallback behavior when deal-lsp isn't running?

| Option | Description | Selected |
|--------|-------------|----------|
| Silent TextMate fallback + status bar | File renders immediately; status bar shows LSP status. No modal/toast/block. | ✓ |
| Notification toast on failure | TextMate fallback + intrusive toast on LSP failure. | |
| Block file open on LSP failure | Webview with install instructions; blocks file open. | |

**Captured as:** D-40

---

### Q4: Should tree-sitter highlights.scm and VS Code TextMate scopes target visual parity across editors?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — parity-by-design | Shared color category set used by both grammars. Cross-editor consistency. | ✓ |
| Per-editor optimization | Optimize each grammar for its host editor's conventions. | |
| Minimum-viable highlight, defer parity | Ship both with conventional defaults; defer parity. | |

**Captured as:** D-41

---

## LSP process & state model

### Q1: How should deal-lsp manage Zig parse state for open documents?

| Option | Description | Selected |
|--------|-------------|----------|
| Hot per-document handles | Live `DealHandle*` per open doc; drop+recreate on edit. Matches rust-analyzer / ts-server. | ✓ |
| Single workspace handle per request | Fresh handle per request; simpler lifecycle but per-request parse cost. | |
| Subprocess shell-out to deal CLI | LSP shells out to `deal check --json`; simplest but slow process spawn. | |

**Captured as:** D-42

---

### Q2: When should re-parse fire?

| Option | Description | Selected |
|--------|-------------|----------|
| Debounced on change (300ms) | Re-parse 300ms after last keystroke; matches rust-analyzer / ts-server cadence. | ✓ |
| Synchronous on every change | Re-parse every keystroke; flashes diagnostics during typing. | |
| On-save only | Refresh diagnostics only on save; breaks "comparable to TypeScript" bar. | |

**Captured as:** D-43

---

### Q3: Memory policy for open documents?

| Option | Description | Selected |
|--------|-------------|----------|
| Hold all open documents | AST memory is small; 200 files ≈ few MB. No eviction. | ✓ |
| LRU eviction (top N kept) | Keep N most-recently-accessed; rebuild on revisit. Premature at showcase scale. | |
| Off-screen eviction | Free handle when document not visible; couples to editor visibility events. | |

**Captured as:** D-44

---

### Q4: Concurrency model for Zig C ABI access?

| Option | Description | Selected |
|--------|-------------|----------|
| Per-handle Mutex<DealHandle> | Per-file tokio Mutex; different docs serve concurrently. Matches rust-analyzer. | ✓ |
| Single global mutex | One mutex guards all handles; serializes all LSP work. | |
| Worker thread + channel | Dedicated OS thread owns handles; LSP requests via channel. | |

**Captured as:** D-45

---

## Workspace symbol index strategy

### Q1: Where does the LSP get the workspace symbol table from?

| Option | Description | Selected |
|--------|-------------|----------|
| In-memory primary, disk index advisory | LSP owns in-memory index; disk index = startup warm-cache hint. Accurate during unsaved edits. | ✓ |
| Disk index + watch | Read `.deal/index.json` on startup, watch for changes. Stale during unsaved edits. | |
| Trigger deal check on save, then read disk | Shell out to `deal check` on save to regenerate disk index. Adds latency. | |

**Captured as:** D-46

---

### Q2: Eager full-workspace parse on LSP startup?

| Option | Description | Selected |
|--------|-------------|----------|
| Eager full-workspace parse on startup | Parse all workspace files on `initialize`; cross-file goto-def works immediately. | ✓ |
| Lazy on first cross-file query | Index only opened files; first cross-file action triggers background scan. | |
| Index-from-disk-only | Read `.deal/index.json` if present; prompt user to run `deal check` otherwise. | |

**Captured as:** D-47

---

### Q3: Should LSP sync in-memory index back to disk?

| Option | Description | Selected |
|--------|-------------|----------|
| No — disk index owned by `deal check` | Disk index = CLI artifact; LSP in-memory ephemeral. No two-writer conflict. | ✓ |
| Sync on save with debouncing | LSP writes back to disk on save. Conflicts with concurrent `deal check`. | |
| Sync on explicit command | Add 'DEAL: Refresh workspace index' command. Engineers won't discover. | |

**Captured as:** D-48

---

### Q4: Where does the path-string ID resolver live?

| Option | Description | Selected |
|--------|-------------|----------|
| In Rust, walking IR JSON per file | LSP calls `deal_index_json` per file; merges in Rust HashMap. O(1) lookup. | ✓ |
| Push resolver into Zig core | New C ABI `deal_resolve_id`; pushes workspace concept into Zig. Architectural change. | |
| Re-use `deal check` resolver via subprocess | Shell out to `deal query <id>` per resolution. Slow. | |

**Captured as:** D-49

---

## deal-lsp binary distribution

### Q1: How does vscode-deal acquire the deal-lsp binary?

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-download on first activation | Small .vsix; downloads platform binary from GitHub Releases on first activation. | ✓ |
| Bundle platform binaries in .vsix | Per-platform .vsix variants (~10-30MB each); works offline. | |
| PATH discovery only | Look for `deal-lsp` on PATH; show install instructions if missing. | |

**Captured as:** D-50

---

### Q2: Should the extension support an offline install path for airgapped users?

| Option | Description | Selected |
|--------|-------------|----------|
| Bundled fallback .vsix variants | Primary = small auto-download .vsix; offline per-platform .vsix on GitHub Releases. Both paths supported. | ✓ |
| Configurable binary path setting | `deal-lang.lsp.path` setting; engineer acquires binary out-of-band. | |
| Online-only for now, revisit at Phase 6 | Ship auto-download only; defer airgap to Phase 6. | |

**Captured as:** D-51

---

### Q3: Version compatibility model between vscode-deal and deal-lsp?

| Option | Description | Selected |
|--------|-------------|----------|
| Pinned-version per extension release | Each vscode-deal release pins exact deal-lsp version; deterministic. Matches rust-analyzer. | ✓ |
| Semver-range matching | Extension declares `deal-lsp >=X, <Y`; runtime resolves latest matching. | |
| Latest at install time | Always download latest deal-lsp; may break compatibility. | |

**Captured as:** D-52

---

### Q4: Where does the deal-lsp binary get built and published from?

| Option | Description | Selected |
|--------|-------------|----------|
| GitHub Actions matrix → GitHub Releases | CI builds + uploads on `v*` tag; standard for Rust LSP. | ✓ |
| crates.io install (`cargo install deal-lsp`) | Requires Rust toolchain; wrong for defense engineers. | |
| Custom hosting (deal-lang.org) | Build in CI, push to deal-lang.org. Adds infra ownership. | |

**Captured as:** part of D-50 + D-53 release pipeline

---

### Q5: How should the Zig core build for the cross-platform target matrix?

| Option | Description | Selected |
|--------|-------------|----------|
| Zig cross-compile from one host | One linux-x64 runner cross-builds libdeal.a for all 4 targets; Rust links per target. | ✓ |
| Native build per platform | Full GitHub Actions matrix; eliminates cross-compile risk. | |
| Defer Windows for v1 | Ship mac-arm64/mac-x64/linux-x64 in Phase 3; defer Windows to Phase 6. | |

**Captured as:** D-53

---

## Claude's Discretion

Areas left to researcher / planner per `03-CONTEXT.md` `<decisions>` "Claude's Discretion" subsection:

- Exact LSP-standard semantic-token-type mapping table (D-39 details)
- Color category names (D-41 `COLOR-CATEGORIES.md` shape)
- Tree-sitter grammar scope (full EBNF mirror vs. lean highlight-focused)
- `tower-lsp` version pin and wrapper-crate selection
- vscode-deal snippet library scope (Claude proposes 19; researcher may expand)
- File icon visual design (`.deal` blueprint sheet, `.dealx` JSX-style brackets)
- Diagnostic delivery model (pull `textDocument/diagnostic` LSP 3.17 vs. push `publishDiagnostics`)
- Format-on-save semantics with unsaved buffers (in-memory parse vs. save+invoke)
- Phase 3 plan slicing (5–7 plans; planner proposes during `gsd:plan-phase`)

## Deferred Ideas

Captured in `03-CONTEXT.md` `<deferred>` section. Summary:

- Experimental LSP capabilities beyond required five (`references`, `codeAction`, `rename`, `signatureHelp`) — Phase 3.x / Phase 4
- VS Code experimental tree-sitter (re-evaluate at Phase 4/5 gate if default-on)
- Tauri desktop editor (Phase 6)
- `deal-lang.org` docs site + Shiki grammar (Phase 4 — consumes Phase 3 TextMate grammars)
- `deal-stdlib`-aware completion (Phase 4)
- Simulation-aware completion / hover (Phase 5)
- `deal import` integration (Phase 6)
- MCP server consuming `.deal/index.json` (out-of-roadmap; Phase 3 contract preserves MCP-readiness)
- WASM packaging of `deal-lsp` (out of scope)
- Formal performance benchmarks (smoke benchmarks only in Phase 3; full suite Phase 5/6)
- Retire or supersede stale `/Users/dunnock/projects/deal-lang/lsp/README.md` (Phase 3 closeout task)
- REQUIREMENTS.md update: mark `DEFER-textmate-vs-treesitter-vscode` as resolved by D-38 (Phase 3 closeout task)
