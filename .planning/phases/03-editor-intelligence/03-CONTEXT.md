# Phase 3: Editor Intelligence - Context

**Gathered:** 2026-05-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 3 delivers TypeScript-comparable editor intelligence for DEAL across the four supported editor surfaces:

1. **`vscode-deal`** — VS Code / Cursor extension (TypeScript) that auto-activates on `deal.toml`, ships TextMate grammars for baseline highlighting + bracket matching + snippets + file icons, and wires an embedded `deal-lsp` for semantic tokens, diagnostics, completion, hover, definition, and format-on-save.
2. **`deal-lsp`** — Rust LSP server built on `tower-lsp` that calls the Zig core via the 9-export C ABI established in Phase 2 (`deal_parse`, `deal_free`, `deal_has_errors`, `deal_diagnostics_count`, `deal_diagnostics_json`, `deal_ast_json`, `deal_ir_json`, `deal_format`, `deal_index_json`).
3. **`tree-sitter-deal`** — Tree-sitter grammar (`grammar.js` + `queries/highlights.scm` + `queries/injections.scm` + `queries/indents.scm`) targeting Neovim, Helix, Zed, and GitHub web rendering, with a test corpus derived from the 19-file showcase.
4. **TextMate grammars** — `deal.tmLanguage.json` and `dealx.tmLanguage.json` with dedicated scopes for `<<operator>>`, `@category:<<operator>>`, `[<tag>]`, and `@header { ... }`.

The phase ships: vscode-deal extension at publishable state on the VS Code Marketplace; auto-downloaded `deal-lsp` binary with airgap fallback `.vsix` variants on GitHub Releases; tree-sitter-deal npm package with passing test corpus; visual-parity color category set shared across TextMate + tree-sitter highlight queries; phase-3 exit gate inside a freshly-cloned worktree (per ADR-phase-1.5-fresh-worktree-verification).

The phase does NOT ship: package resolution (`deal init`, `deal install`, `deal.lock`) — Phase 4; standard library (`deal-stdlib`) — Phase 4; ReqIF codegen / `deal import` — Phase 4 / Phase 6; documentation site (`deal-lang.org`) — Phase 4; simulation integration (`deal-sim`, `deal simulate`, `deal evidence`) — Phase 5; Tauri desktop editor — Phase 6; experimental LSP capabilities beyond the required five (`textDocument/{references, rename, codeAction, signatureHelp}`) — deferred to a Phase 3.x or later phase.

</domain>

<decisions>
## Implementation Decisions

> Decisions are numbered D-38 onward to continue the project-wide D-01..D-37 sequence from Phases 1, 1.5, and 2.

### VS Code highlighting strategy (REQ-3-1 + REQ-3-4 + DEFER-textmate-vs-treesitter-vscode resolution)

- **D-38:** **Primary highlighting source for `vscode-deal` is TextMate + LSP semantic tokens.** Ship `deal.tmLanguage.json` and `dealx.tmLanguage.json` (REQ-3-1 mandate: scopes for `<<operator>>`, `@category:<<operator>>`, `[<tag>]`, `@header`); `deal-lsp` provides `textDocument/semanticTokens/full` and `/delta`. TextMate gives instant offline color; LSP enriches with type-aware information that regex cannot express. Resolves DEFER-textmate-vs-treesitter-vscode: **VS Code's experimental tree-sitter support is rejected** for Phase 3 — flag-gated UX, duplicative of REQ-3-2's tree-sitter work, and would create two parser sources of truth.
- **D-39:** **Full structural semantic-tokens overlay.** `deal-lsp` declares the full LSP-standard token-type set (`keyword`, `type`, `parameter`, `variable`, `property`, `namespace`, `operator`, `enumMember`, `regexp`, plus modifiers `declaration`, `definition`, `readonly`, `static`, `deprecated`). Token categories map: `keyword` = bare keywords (`part def`, `in`, `out`, `abstract`); `type` = definition references (`Part`, `Port`, `Connection`); `namespace` = package path segments (`sedan_project.vehicle.electrical`); `operator` = `<<specializes>>`, `<<redefines>>`, etc.; `enumMember` = annotation categories (`@trace`, `@simulation`, `@connection`); `regexp` = multiplicity ranges (`[1..*]`). Researcher will draft the exact mapping table against the 19-file showcase and lock it in `03-PLAN-*.md`.
- **D-40:** **Silent TextMate fallback + status-bar status indicator** when `deal-lsp` is not running (binary downloading, crash, missing on PATH for the configurable-path escape hatch). File renders immediately with TextMate-only highlighting; status bar shows `DEAL LSP: starting…` → `DEAL LSP: ready` → `DEAL LSP: error (click for output)`. **No modal, no toast, no blocked file open.** Matches rust-analyzer / gopls UX; preserves read-only workflows (e.g., reviewing a `.deal` file in a PR before installing the LSP).
- **D-41:** **Visual parity-by-design across TextMate and tree-sitter.** Both grammars target a shared color category set documented in `vscode-deal/COLOR-CATEGORIES.md` (or equivalent, name is planner's call) — e.g., `keyword.element.deal.part-def`, `entity.name.operator.specializes`, `string.tag.composition`. Same category names appear as TextMate scopes and as tree-sitter highlight captures. Result: GitHub web, VS Code, Neovim, Helix, and Zed render the same `.deal` file with the same color story. Researcher cites prevailing TextMate scope conventions and tree-sitter `@*` capture conventions to ensure each editor's default theme picks them up cleanly.

### LSP process & state model (REQ-3-3 architecture)

- **D-42:** **Hot per-document `DealHandle*` per open file.** On `textDocument/didOpen`, `deal-lsp` calls `deal_parse(source, len, file_path)` and stores the returned `DealHandle*` keyed by document URI. On `textDocument/didChange`, drop the old handle (`deal_free`) and create a fresh one with the new buffer. Memory bound is the AST arena per file (~5–20 kB for showcase files per Phase 1 measurements) × N open files; well under budget for engineer-scale projects. Faster than per-request parse, simpler than incremental Zig integration. Matches rust-analyzer's salsa-database and ts-server's `Program` pattern.
- **D-43:** **Debounced re-parse on change (300 ms).** `textDocument/didChange` schedules a re-parse 300 ms after the last keystroke; subsequent changes within the window reset the timer. After re-parse, `deal-lsp` recomputes diagnostics, semantic tokens, and (incrementally) the in-memory symbol index for this file. Within typical typing cadence (≤300 ms gaps), no parsing happens; on a pause, diagnostics refresh within a beat. Matches rust-analyzer (250–500 ms) and ts-server (350 ms).
- **D-44:** **Hold all open-document handles live; no eviction.** AST memory is small enough that 200+ open files fit in a few MB — well within developer-machine budget. No LRU, no off-screen eviction. Phase 5 or Phase 6 may revisit if a real project surfaces a memory issue.
- **D-45:** **Concurrency: per-handle `Mutex<DealHandle*>` guarded by a tokio `Mutex`.** Different documents serve LSP requests concurrently; same-document requests queue. The `DealHandle*` is wrapped in a Rust newtype (`struct OwnedDealHandle(*mut DealHandle)`) implementing `Send` (the Zig arena is fully owned and the C ABI is thread-safe for distinct handles per Phase 1 D-02). Lock granularity = per file. Matches how rust-analyzer guards its salsa state.

### Workspace symbol index strategy (REQ-3-3 workspace-symbol + cross-file go-to-definition)

- **D-46:** **In-memory primary, disk `.deal/index.json` advisory.** `deal-lsp` owns its own in-memory workspace symbol table: `HashMap<PathString, (FileUri, Range)>` where `PathString` is the D-23 fully-qualified path ID (e.g., `sedan_project.vehicle.electrical.BatteryPack.hvOut`). The table is populated by per-document `deal_index_json` calls on first parse and updated on every re-parse. The disk `.deal/index.json` (FS-4, Phase 2) is consulted on startup as a warm-cache hint but is NOT authoritative for the LSP — the in-memory table is, because it stays accurate through unsaved edits.
- **D-47:** **Eager full-workspace parse on LSP `initialize`/`initialized`.** On project open, walk `deal.toml` to find `[workspace]` and discover `packages/` (`.deal`) + `model/` (`.dealx`) files. Parse each into a per-document handle and populate the in-memory index. Cost ≈ N × parser-time-per-file (Phase 1 measured sub-ms per showcase file); sub-second for showcase, a few seconds for engineer-scale projects. Cross-file go-to-definition works the instant the project opens — required to meet the REQ-3-gate bar "comparable to TypeScript in VS Code."
- **D-48:** **No LSP write-back to `.deal/index.json`.** The disk index is the CLI's artifact: produced by `deal check` (or whichever `deal` subcommand the user runs), consumed by CI, future MCP servers, and the docs-site CI snippet-render gate (Phase 4). LSP's in-memory index is ephemeral within the LSP session. Engineer refreshes the disk index by running `deal check` (or `deal index` if that subcommand is added in Phase 4). This preserves Phase 2's contract that the disk index is deterministic + gitignored + owned by the CLI side of the FFI boundary.
- **D-49:** **Path-string ID resolver lives in Rust, walking per-file IR JSON.** `deal-lsp` calls `deal_index_json(handle)` for each open document and merges per-file symbol maps into the global Rust `HashMap`. Cross-package alias resolution from `deal.toml [workspace.aliases]` (PS-5) layers on top of the merged map in Rust — does NOT modify the Zig core. Resolution is O(1) HashMap lookup. The Zig core stays single-file-scoped; workspace concerns stay in Rust.

### `deal-lsp` binary distribution (REQ-3-4 binary bundling)

- **D-50:** **Auto-download `deal-lsp` from GitHub Releases on first activation.** Primary distribution is a small `vscode-deal.vsix` on the VS Code Marketplace (no bundled binaries inside). On first activation (`onLanguage:deal`, `onLanguage:dealx`, `workspaceContains:deal.toml`), the extension downloads the platform-correct `deal-lsp` binary from `https://github.com/deal-lang/deal/releases/download/v{X}/deal-lsp-{platform}.tar.gz` into the extension's global storage. Subsequent launches verify checksum (SHA-256 manifest committed alongside each release) and reuse. Targets: `darwin-arm64`, `darwin-x64`, `linux-x64-gnu`, `linux-x64-musl`, `win-x64`.
- **D-51:** **Bundled offline `.vsix` variants for airgapped users.** Per-platform offline `.vsix` files are also published to GitHub Releases (e.g., `vscode-deal-{X}-darwin-arm64-offline.vsix`) containing the bundled `deal-lsp` binary. Airgapped defense/aerospace engineers grab the offline variant manually. Adds release-pipeline complexity (~5 extra build outputs per release) but no extension-code complexity beyond detecting whether a bundled binary is present at activation time. Resolves the PROJECT.md tension: Marketplace install path optimized for connected users, airgap path preserved.
- **D-52:** **Pinned `deal-lsp` version per `vscode-deal` release.** Each `vscode-deal.vsix` hard-codes the exact `deal-lsp` version it expects (e.g., `vscode-deal@0.3.0 ↔ deal-lsp@0.3.0`); download URL hard-coded with embedded SHA-256. Engineer updating `vscode-deal` automatically pulls the matching `deal-lsp` on next activation. No semver-range resolution, no runtime drift. Matches rust-analyzer's `package.json + bootstrap.ts` pattern.
- **D-53:** **Zig cross-compile from one host (`linux-x64` runner) for all four targets.** One GitHub Actions job builds `libdeal.a` for all platforms via `zig build -Dtarget=<aarch64-macos|x86_64-macos|x86_64-linux-gnu|x86_64-windows>`. The Rust release matrix (4 jobs) then links each cross-built `libdeal.a` into `deal-lsp` natively per target. Saves CI minutes vs. a full native-build matrix (1 Zig job + 4 Rust jobs vs. 4 full Zig+Rust jobs). Zig 0.16.0's cross-compile is solid and is how the Zig project itself ships. Phase 3 covers all 4 targets at launch; no platform deferred.

### Claude's Discretion

The following implementation details are intentionally NOT locked here — researcher and planner have latitude:

- **Exact LSP-standard semantic-token-type mapping table** (D-39) — researcher drafts against the showcase, lock in plan/PR.
- **Color category names** (D-41 `COLOR-CATEGORIES.md` shape) — researcher picks names matching prevailing TextMate scope conventions and tree-sitter `@*` conventions; both must use the same names.
- **Tree-sitter grammar scope** — full EBNF mirror (1:1 with the 87+43 productions) vs. lean highlight-focused grammar (only what `highlights.scm`/`indents.scm` queries need to discriminate). Researcher evaluates against the Zig parser AST shape; the constraint is REQ-3-2 acceptance: tree-sitter parses showcase corpus without errors and highlight queries discriminate the visual hierarchy from D-41.
- **`tower-lsp` version pin** and any wrapper-crate selection (e.g., `tower-lsp` vs. `lspower` fork) — researcher picks the most-maintained option as of phase start.
- **vscode-deal snippet library scope** (REQ-3-1) — Claude picks initial snippet set covering the 11 element keywords (per SD-1) plus the four most-common composition tags (`[<system>]`, `[<connect>]`, `[<satisfy>]`, `[<allocate>]`). Researcher may expand based on showcase frequency analysis.
- **File icon design** (REQ-3-1) — Claude designs `.deal` (definition file, blueprint-style) and `.dealx` (composition file, JSX-style) icons; PROJECT.md does not specify visual identity.
- **Diagnostic delivery model** — pull (`textDocument/diagnostic`, LSP 3.17) vs. push (`textDocument/publishDiagnostics`). Researcher picks per `tower-lsp` capability + VS Code client compatibility. Both are LSP-spec-valid.
- **Format-on-save semantics with unsaved buffers** — does LSP `textDocument/formatting` parse the in-memory buffer (which `deal-lsp` already holds per D-42) or save+invoke `deal fmt` CLI? Lean toward in-memory (no save-needed UX) but parallel D-21's existing C ABI behavior.
- **Phase 3 plan slicing** — researcher and planner propose the 5–7 plan slice for `gsd:plan-phase`; the major waves probably are: (1) TextMate + VS Code scaffold; (2) tree-sitter grammar + test corpus; (3) deal-lsp scaffold + diagnostics + formatting; (4) deal-lsp completion + hover + definition with workspace index; (5) vscode-deal LSP wiring + semantic tokens; (6) binary distribution + release pipeline; (7) phase-3-gate + phase-3-gate-fresh + acceptance smoke. Final slicing is the planner's call.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project authority

- `.planning/PROJECT.md` — Tech stack (Rust LSP via `tower-lsp`, TypeScript VS Code extension, tree-sitter grammar TS bindings, Astro+Starlight for Phase 4 docs, **but Phase 3 is pure tooling**), 64 LOCKED + 1 CAPTURED design decisions in `<decisions>` block.
- `.planning/REQUIREMENTS.md` §Phase 3 — REQ-phase-3-editor-intelligence, REQ-phase-3-1-textmate, REQ-phase-3-2-treesitter, REQ-phase-3-3-lsp-server, REQ-phase-3-4-vscode-lsp, REQ-phase-3-gate.
- `.planning/ROADMAP.md` §"Phase 3: Editor Intelligence" — Goal statement and 5 success criteria.
- `.planning/REQUIREMENTS.md` §"v2 Requirements" entry **DEFER-textmate-vs-treesitter-vscode** — resolved by D-38 (reject experimental tree-sitter, ship TextMate + LSP semantic tokens).
- `.planning/STATE.md` — Phase 2 COMPLETE 2026-05-22; 17/17 plans complete; carries forward all Phase 1/1.5/2 D-codes.

### Phase 2 locked public-contract surfaces (Phase 3 consumers)

Phase 2 locked three contract surfaces that Phase 3 consumes verbatim. Modifications require a new ADR.

- `.planning/phases/02-prove-the-pipeline/02-CONTEXT.md` — Phase 2 D-19..D-37, including D-22 (`deal_ir_json` C ABI symbol), D-21 (`deal_format` C ABI symbol), D-23 (fully-qualified path-string IDs), D-26 (IR traversal API), D-32 (`--json` diagnostic envelope), D-33 (E2xxx semantic error code allocation), D-34 (CLI exit code semantics), D-37 (Phase 2 plan slicing — informational).
- `.planning/phases/02-prove-the-pipeline/02-SPEC.md` — Phase 2 SPEC.md (9 locked requirements + 14 acceptance criteria) — informational; Phase 3 inherits the contract surfaces, not the requirements.
- `.planning/decisions/ADR-deal-ir-v0.md` — DEAL IR v0 rationale. The LSP consumes IR v0 JSON via `deal_ir_json` for completion / hover / definition lookups beyond what `deal_index_json` covers.
- `spec/ir/v0/schema.json` + `spec/ir/v0/README.md` — Normative IR v0 schema + Markdown reference. Lives in the `spec/` submodule.
- `deal/include/deal.h` — Updated C ABI header (9 exports as of Phase 2 closeout: `deal_parse`, `deal_free`, `deal_has_errors`, `deal_diagnostics_count`, `deal_diagnostics_json`, `deal_ast_json`, `deal_ir_json`, `deal_format`, `deal_index_json`). Phase 3 does NOT add new C ABI exports — all editor intelligence is layered in Rust on top of the existing 9.

### Phase 1 + 1.5 inherited invariants

- `.planning/phases/01-zig-compiler-core/01-CONTEXT.md` — Phase 1 D-01..D-18: AST tagged-union, per-handle arena (D-02), JSON shape with `"v"` first (D-04), C ABI handle pattern + length-prefixed UTF-8 (D-11), diagnostic model (D-14), span = u32 byte offsets, **D-16 error-code ranges** (E0xxx parser codes preserved; E2xxx added by Phase 2; E3xxx reserved if Phase 3 surfaces LSP-only diagnostics), **D-18 alphabetical-key JSON invariant**.
- `.planning/phases/01.5-parser-strictness-test-gap-resolution-inserted/01.5-CONTEXT.md` — Strictness decisions (m08/m18/m19 dispositions, per-file pinning, determinism test, span-containment property test, fresh-worktree invariant for Phases 2..6).
- `.planning/decisions/ADR-phase-1.5-fresh-worktree-verification.md` — **Binding ADR.** Phase 3 MUST add `phase-3-gate-fresh` build step that runs the full Phase 3 gate inside a freshly-cloned git worktree after `git submodule update --init --recursive`. Authoritative gate command: `zig build phase-3-gate-fresh` must exit 0 inside an ephemeral worktree. Phase 3 also inherits `phase-2-gate-fresh` as a regression check.

### Phase 0 LOCKED design decisions (informational — must not contradict)

- `spec/grammar/DESIGN-DECISIONS.md` — Consolidated ADR (64 LOCKED + 1 CAPTURED). Phase 3 must respect: **FA-4** (self-explanatory syntax — hover/completion should reinforce, not work around, the cold-readability principle), **FS-4** (`.deal/index.json` is the system-wide symbol index — LSP's in-memory index is per-D-46 advised by it, not authoritative over it), **LM-2** (JSDoc `/** */` doc comments precede declarations — hover renders them), **NC-1** (extension is `.deal` / `.dealx`; CLI is `deal`; LSP binary name SHOULD be `deal-lsp`), **PS-1** (`deal.toml` triggers extension activation), **PS-5** (`[workspace.aliases]` — LSP cross-package alias resolution per D-49), **SD-1..SD-20** + **CS-1..CS-16** (snippet library targets these element keywords and composition tags).

### Grammar specifications (LOCKED at `0.1.0-draft`)

- `spec/grammar/lexical.ebnf` — Shared token set (758 lines). TextMate grammars and tree-sitter `grammar.js` token rules MUST match the lexical productions (e.g., string literals, comments per §2, identifier rules).
- `spec/grammar/deal.ebnf` — 87 productions for `.deal` definition files. Tree-sitter grammar (or the lean highlight-only variant per Claude's Discretion) must cover the subset needed for highlight-query discrimination.
- `spec/grammar/dealx.ebnf` — 43 productions for `.dealx` composition files. Tree-sitter must handle `[< >]` composition tags and inline `via={...}`/`carrying={...}` blocks.
- `spec/grammar/README.md` — Grammar README; cross-references the three EBNF files. (Note: STATE.md flagged 370→758 line count drift to be addressed during Phase 4 docs work; not a Phase 3 concern.)

### Phase-3 implementation targets (top-level packages — currently scaffold READMEs only)

- `/Users/dunnock/projects/deal-lang/vscode-deal/README.md` — VS Code extension. Currently scaffold-only. Phase 3 turns it into a publishable extension. The README's Stage 2 section accurately reflects D-38..D-41.
- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/README.md` — Tree-sitter grammar package. Currently scaffold-only. Phase 3 turns it into a publishable npm package.
- `/Users/dunnock/projects/deal-lang/lsp/README.md` — **⚠ STALE — DO NOT USE AS DESIGN INPUT.** Describes a TypeScript LSP using `vscode-languageserver`; this contradicts the LOCKED tech stack (Rust + `tower-lsp` per PROJECT.md + REQ-3-3). Phase 3 either retires this README or supersedes it; the locked design is in this CONTEXT.md and ROADMAP.md.
- `/Users/dunnock/projects/deal-lang/editor/README.md` — Tauri desktop editor. Phase 6 scope; informational only for Phase 3.

### Integration oracle (snapshot truth — Phase 3 acceptance)

- `spec/examples/showcase/` — **19-file showcase project** (15 `.deal` + 4 `.dealx`). All Phase 3 acceptance gates run against this corpus: tree-sitter test corpus derived from it (REQ-3-2), VS Code "open showcase project" exit gate (REQ-3-gate). Mounted at `deal/tests/showcase/` via committed symlink (Phase 1.5 Plan 05).
- `spec/examples/showcase/deal.toml` — Project manifest. Triggers `vscode-deal` activation per D-50.

### External references — researcher to fetch

- **`tower-lsp` documentation** — Rust LSP framework. Researcher uses `context7` MCP or web search to fetch current API for `LanguageServer` trait, `tokio` integration, semantic-tokens-delta support, `textDocument/diagnostic` pull-mode availability.
- **LSP specification 3.17+** — Standard reference for the five required capabilities (`diagnostic`, `completion`, `definition`, `hover`, `formatting`) and `semanticTokens`. Per D-39, also `semanticTokens/full`, `semanticTokens/delta`, and the token-type/modifier table.
- **VS Code Extension API** — `activationEvents`, `LanguageClient` from `vscode-languageclient`, `SemanticTokensProvider`, status bar API. Researcher fetches current API for VS Code 1.95+ (or whichever is the prevailing baseline at phase start).
- **Tree-sitter documentation** — `grammar.js` DSL, `highlights.scm` capture syntax, `injections.scm` for embedded languages, `indents.scm` for indent queries, `tree-sitter-cli` for grammar dev + corpus testing.
- **rust-analyzer reference implementation** — Pattern source for: (a) `bootstrap.ts` auto-download (D-50, D-52), (b) `tower-lsp`-style per-document state (D-42, D-44, D-45), (c) workspace symbol index lifecycle (D-46, D-47), (d) salsa/database concurrency (D-45 informs Phase 3's simpler `Mutex<DealHandle*>` model).
- **TextMate scope naming conventions** — Researcher cites the prevailing convention (e.g., the textmate.org `language grammars` page, the VS Code Wiki's "Syntax Highlight Guide") to ensure D-41 color category names map cleanly to default themes.
- **OMG SysML v2 viewer/import** — Informational; Phase 2 already recorded viewer-import smoke outcome in `02-VERIFICATION.md`.

### Existing infrastructure to mirror

- `deal/scripts/verify-fresh-worktree.sh` — Reuse for `phase-3-gate-fresh` (D-53 release pipeline cross-builds, but the gate itself runs the same ephemeral-worktree pattern).
- `deal/tests/ffi/Cargo.toml` — Existing Rust FFI harness; closest existing precedent for `deal-lsp`'s `extern "C"` declarations and `bindgen` integration over `deal.h`.
- `deal/cli/Cargo.toml` (Phase 2 deliverable) — The Rust CLI crate's `clap` skeleton, `--json` envelope handling, and FFI bindings are direct precedents for `deal-lsp`'s Rust-side surface. Likely both crates live in a shared Cargo workspace.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **C ABI exports (9 symbols)** — `deal_parse`, `deal_free`, `deal_has_errors`, `deal_diagnostics_count`, `deal_diagnostics_json`, `deal_ast_json`, `deal_ir_json`, `deal_ir_json`, `deal_format`, `deal_index_json`. `deal-lsp` calls all 9 from Rust; no new C ABI symbols added by Phase 3. The `nm -gU` export-count gate moves from 9 (Phase 2 closeout) to 9 (unchanged).
- **D-23 fully-qualified path-string IDs** — Already locked by Phase 2 sema's name-resolution pass. `deal-lsp`'s in-memory workspace index keys on these directly (D-46).
- **D-32 `--json` envelope schema** — `deal-lsp` reuses the envelope for `textDocument/publishDiagnostics` payload structure (or its `textDocument/diagnostic` pull-mode equivalent) — engineers see the same diagnostic JSON shape across CLI and LSP.
- **D-33 E2xxx codes** — `deal-lsp` surfaces these as LSP `Diagnostic.code`; engineers click the code link for documentation. Phase 3 may add an E3xxx range if LSP-only diagnostics emerge (e.g., "this completion was suppressed because the workspace index is rebuilding").
- **D-18 alphabetical-key JSON invariant** — `deal-lsp` outputs JSON (LSP responses) following the same conventions for any DEAL-specific extension payloads.
- **`deal/cli/` Rust crate (Phase 2)** — Sibling crate to `deal-lsp` in the same Cargo workspace. Shares `bindgen` over `deal.h`, shares FFI wrapper module.
- **`tests/showcase/` committed symlink → `../spec/examples/showcase`** — Phase 1.5 Plan 05 fix. Tree-sitter test corpus derives from this path; `deal-lsp` integration tests open the showcase project; vscode-deal acceptance gate opens this project in VS Code. The `phase-3-gate-fresh` materializes it via `git submodule update --init --recursive`.

### Established Patterns

- **Tech-stack split: Zig owns compiler core; Rust owns frontend tooling.** PROJECT.md constraint. D-42..D-45, D-49 honor it: LSP state, concurrency, and workspace-index logic in Rust; the Zig core stays single-file-scoped.
- **Per-handle arena allocation (D-02).** LSP's per-document handles (D-42) follow the existing pattern — no shared arenas, no cross-arena pointers.
- **C ABI is length-prefixed UTF-8 (D-11), never NUL-terminated.** `deal-lsp`'s FFI wrappers honor this when calling `deal_ir_json`, `deal_format`, `deal_index_json`.
- **Fresh-worktree verification (Phase 1.5 ADR).** Phase 3 adds `phase-3-gate-fresh`.
- **Diagnostic emission through `diagnostics.zig`.** Any new E3xxx codes (if emerged) follow the `pub const` namespace pattern + D-18 alphabetical-key JSON output.

### Integration Points

- **AST + IR + diagnostics + format + index** flow into Rust over the existing 9-export C ABI surface. `deal-lsp` is mostly a JSON adapter between LSP protocol and these existing buffers.
- **`.deal/index.json`** is the disk handoff between `deal check` (writer) and `deal-lsp` startup hint (reader). LSP does NOT write it back (D-48).
- **`deal.toml`** triggers `vscode-deal` activation. Parsed by the LSP at `initialize` to discover the workspace and seed the in-memory index per D-47.
- **`tree-sitter-deal/` npm package** is a self-contained editor surface — does not link to `libdeal.a`. Pure tree-sitter grammar + queries + test corpus.
- **GitHub Releases pipeline** is the new infra surface this phase introduces. Phase 4 docs site (`deal-lang.org`) will consume the same pipeline indirectly (links to releases for install instructions).

</code_context>

<specifics>
## Specific Ideas

- **TextMate + LSP semantic tokens is the modern VS Code pattern.** Researcher should cite rust-analyzer's `tmLanguage` + `semanticTokensProvider` combination as the closest existing precedent. Same conceptual layering Phase 3 ships.
- **The DEFER-textmate-vs-treesitter-vscode decision is now LOCKED.** REQUIREMENTS.md should be updated at phase closeout: change "DEFER-textmate-vs-treesitter-vscode *(evaluated at Phase 3 gate)*" to a resolved entry citing D-38. Planner includes this REQUIREMENTS.md update as a closeout task.
- **`deal-lsp` binary name** is `deal-lsp` per the existing `lsp/README.md` and PROJECT.md tech stack naming; preserved here.
- **Cargo workspace layout** — `deal/cli/` (Phase 2) + `deal/lsp/` (Phase 3, new) likely share a `deal/Cargo.toml` workspace with shared FFI module. Planner decides exact crate split (e.g., separate `deal-ffi` crate vs. duplicated `extern "C"` blocks).
- **VS Code Marketplace `targets` field** supports per-platform `.vsix` (e.g., `darwin-arm64`, `linux-x64`, `win32-x64`) — used by D-51 for offline variants. Auto-download `.vsix` is platform-agnostic and uses the `darwin`/`linux`/`win32` runtime detection in `extension.ts`.
- **`deal-lsp` GitHub Release tag scheme** — researcher proposes (e.g., `lsp-v0.3.0`) but should keep `deal-lsp` versions in lockstep with `vscode-deal` versions for D-52 simplicity. Final scheme is planner's call.
- **300 ms debounce window** (D-43) is the design starting point; researcher may revise based on `tower-lsp` + VS Code client interaction patterns (e.g., if VS Code's `textDocument/didChange` is already throttled).
- **Snippet library starting set** (Claude's discretion): 11 element-keyword snippets (one per `part def`, `port def`, `action def`, `state def`, `requirement def`, `constraint def`, `attribute def`, `item def`, `interface def`, `connection def`, `flow def`) + 5 composition-tag snippets (`[<system>]`, `[<connect>]`, `[<satisfy>]`, `[<allocate>]`, `[<traceability>]`) + 3 helpers (`@header { }`, `import …`, `package …;`). Total: 19 snippets.
- **File icon iconography** (Claude's discretion): `.deal` = solid blueprint sheet (definition file); `.dealx` = JSX-style angle-brackets with composition badge (composition file). Researcher refines colors against the VS Code icon-theme contract.

</specifics>

<deferred>
## Deferred Ideas

- **Experimental LSP capabilities beyond REQ-3-3 required set.** REQ-3-3 locks five capabilities (`diagnostic`, `completion`, `definition`, `hover`, `formatting`). The `lsp/README.md` scaffold mentions `references`, `codeAction`, `workspace/symbol`, but only `workspace/symbol` is implied by REQ-3-gate's cross-file go-to-definition requirement (and is delivered via D-46/D-47 index). `references`, `codeAction`, `rename`, `signatureHelp` are deferred to a Phase 3.x or Phase 4 capability extension.
- **VS Code experimental tree-sitter support** — Rejected by D-38 for Phase 3. If VS Code's tree-sitter support becomes default-on (no flag) in a future release, re-evaluate at Phase 4 or 5 gate.
- **Tauri desktop editor** — Phase 6 scope. `editor/` top-level package stays scaffold-only through Phase 3..5.
- **`deal-lang.org` documentation site + Shiki grammar derived from TextMate** — Phase 4. Will consume the TextMate grammars Phase 3 ships (REQ-4-3 CI gate fails if `.deal`/`.dealx` blocks render without highlighting). Phase 3 designs TextMate scopes with Phase 4's Shiki adapter in mind.
- **`deal-stdlib`-aware completion** — Phase 4. Once `deal-stdlib` ships with units/interfaces, LSP completion should surface `kg`, `V`, `RJ45`, `CAN`, etc. from the imported stdlib. Phase 3 ships completion for what the workspace defines; stdlib symbols flow in automatically once Phase 4's package resolution lands.
- **Simulation-aware completion / hover** — Phase 5. `@simulation:<<computes>>` and `evidence simulation { source, binding, maps … }` get richer hover content (e.g., last simulation timestamp, evidence freshness) once Phase 5's evidence cache exists.
- **`deal import` integration in LSP** — Phase 6. If `deal import --from sysml-v2` exists, vscode-deal may offer "Import SysML v2…" command. Not a Phase 3 concern.
- **MCP server consuming `.deal/index.json`** — Out-of-roadmap. Mentioned in PROJECT.md FS-4 as a future consumer. Phase 3's disk-index contract (D-48: no LSP write-back) preserves MCP-readiness.
- **WASM packaging of `deal-lsp`** — Out of scope. PRD §11 DEFER-wasm-packaging was evaluated at Phase 2 gate; the Zig C ABI proved stable. WASM repackaging would be a Phase 4+ effort if the docs-site playground requires it.
- **`deal-lsp` performance budgets / benchmarks beyond what Phase 1's parser timings imply** — Deferred. Planner adds smoke benchmarks (e.g., showcase open → ready < 1s, edit → diagnostic refresh < 500ms) but no formal benchmark suite. Phase 5 or 6 may add one.
- **Retire or supersede `/Users/dunnock/projects/deal-lang/lsp/README.md`** — Phase 3 closeout task. The TypeScript-LSP design is stale; either delete the README and rely on the new Rust LSP's README inside `deal/lsp/` (per the Cargo workspace layout), or rewrite the top-level README to point at the new Rust LSP. Planner decides.

</deferred>

---

*Phase: 3-editor-intelligence*
*Context gathered: 2026-05-22*
*Sources: PROJECT.md, REQUIREMENTS.md, ROADMAP.md, STATE.md, 01-CONTEXT.md, 01.5-CONTEXT.md, 02-CONTEXT.md, ADR-phase-1.5-fresh-worktree-verification.md, ADR-deal-ir-v0.md, top-level scaffold READMEs (`vscode-deal/`, `tree-sitter-deal/`, `lsp/`, `editor/`)*
