# Phase 3: Editor Intelligence — Research

**Researched:** 2026-05-22
**Domain:** LSP server (Rust/tower-lsp) + VS Code extension (TypeScript) + tree-sitter grammar + TextMate grammars + GitHub Releases binary distribution
**Confidence:** MEDIUM-HIGH (locked context is rich; external API pins are version-sensitive)

<user_constraints>
## User Constraints (from 03-CONTEXT.md)

### Locked Decisions
- **D-38:** VS Code highlights via **TextMate + LSP semantic tokens** (reject experimental tree-sitter for VS Code). Ship `deal.tmLanguage.json` + `dealx.tmLanguage.json` with dedicated scopes for `<<operator>>`, `@category:<<operator>>`, `[<tag>]`, `@header`. `deal-lsp` provides `textDocument/semanticTokens/full` and `/delta`.
- **D-39:** Full structural semantic-tokens overlay. Token types: `keyword`, `type`, `parameter`, `variable`, `property`, `namespace`, `operator`, `enumMember`, `regexp`; modifiers: `declaration`, `definition`, `readonly`, `static`, `deprecated`. Researcher drafts the concrete DEAL→token mapping table.
- **D-40:** Silent TextMate fallback + status-bar indicator (`starting…` / `ready` / `error (click for output)`). No modal/toast/blocked open.
- **D-41:** Visual parity by design — both TextMate scopes and tree-sitter `@*` captures share a single color category set in `COLOR-CATEGORIES.md`.
- **D-42:** Hot per-document `DealHandle*` per open file. `didOpen`→`deal_parse`; `didChange`→`deal_free` old + parse new.
- **D-43:** Debounced re-parse (300 ms) on `didChange`.
- **D-44:** No eviction; hold all open-document handles live.
- **D-45:** `Mutex<DealHandle*>` per file via tokio `Mutex`. Per-file lock granularity; concurrent across files.
- **D-46:** In-memory primary workspace symbol table `HashMap<PathString, (FileUri, Range)>`. Disk `.deal/index.json` is advisory only.
- **D-47:** Eager full-workspace parse on `initialize`/`initialized`.
- **D-48:** LSP does NOT write back to `.deal/index.json` — disk index is CLI's artifact.
- **D-49:** Path-string resolver in Rust; walks per-file IR JSON (`deal_index_json`); cross-package aliases from `deal.toml [workspace.aliases]` layered on top.
- **D-50:** Auto-download `deal-lsp` from GitHub Releases on first activation. Targets: `darwin-arm64`, `darwin-x64`, `linux-x64-gnu`, `linux-x64-musl`, `win-x64`.
- **D-51:** Per-platform offline `.vsix` variants with bundled binary on GitHub Releases (`vscode-deal-{X}-{platform}-offline.vsix`).
- **D-52:** Pinned `deal-lsp` version per `vscode-deal` release; hardcoded URL + SHA-256.
- **D-53:** Zig cross-compile from one Linux runner for all 4 targets; 4 Rust matrix jobs link cross-built `libdeal.a`.

### Claude's Discretion
- D-39 concrete DEAL→token mapping table (§7 below).
- D-41 color category names (§8 below).
- Tree-sitter grammar scope: full EBNF mirror vs lean highlight-only (§9 below — lean recommended).
- `tower-lsp` version pin (§1 below).
- vscode-deal snippet library scope (§14 below).
- File icon design (deferred to PLAN-time visual work).
- Diagnostic delivery: push vs pull (§11 below — push recommended).
- Format-on-save semantics for unsaved buffers (in-memory recommended).
- Phase 3 plan slicing (§16 below).

### Deferred Ideas (OUT OF SCOPE)
- Experimental LSP capabilities beyond `diagnostic`/`completion`/`definition`/`hover`/`formatting` (no `references`, `rename`, `codeAction`, `signatureHelp`, `inlay hint`).
- VS Code experimental tree-sitter support.
- Tauri desktop editor (Phase 6).
- `deal-lang.org` docs site + Shiki grammar (Phase 4).
- `deal-stdlib`-aware completion (Phase 4).
- Simulation-aware completion/hover (Phase 5).
- `deal import` integration in LSP (Phase 6).
- MCP server consuming `.deal/index.json`.
- WASM packaging of `deal-lsp`.
- Formal benchmark suite.
- Retire/supersede `/Users/dunnock/projects/deal-lang/lsp/README.md` — Phase 3 closeout task.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REQ-3-1 (textmate) | `deal.tmLanguage.json` + `dealx.tmLanguage.json` + snippets + language config + file icons. | §5 (VS Code Extension API), §6 (scope conventions), §8 (color categories), §14 (snippets) |
| REQ-3-2 (treesitter) | `tree-sitter-deal` grammar + 3 query files + corpus tests. | §3 (tree-sitter dev), §8 (capture parity), §9 (lean grammar) |
| REQ-3-3 (lsp-server) | `deal-lsp` Rust crate; `diagnostic`/`completion`/`definition`/`hover`/`formatting` capabilities + semantic tokens. | §1 (tower-lsp), §2 (LSP spec), §7 (token map), §10 (workspace), §11 (diagnostic mode) |
| REQ-3-4 (vscode-lsp) | `vscode-deal` activates on `deal.toml`/`onLanguage:deal[x]`, runs LanguageClient against `deal-lsp`, bundled+auto-download binary. | §4 (bootstrap.ts), §5 (LanguageClient), §12 (CI matrix) |
| REQ-3-gate | Cross-file go-to-definition works on showcase; phase-3-gate + phase-3-gate-fresh exit 0. | §13 (gate design), §15 (Validation Architecture) |
</phase_requirements>

## Summary

Phase 3 stands up four editor surfaces against a stable, locked C ABI (9 exports, unchanged from Phase 2). The dominant technical risks are (1) `tower-lsp` API surface drift across the 0.x line, (2) per-platform binary distribution pipeline correctness (D-50..D-53), and (3) TextMate/tree-sitter scope parity (D-41) over the 19-file showcase. None of these are research-blocking — the locked decisions in CONTEXT.md leave only implementation-detail latitude for the planner.

**Primary recommendation:** Pin `tower-lsp = "0.20"` (verify at planning time via `cargo search tower-lsp`); mirror rust-analyzer's `bootstrap.ts` + LanguageClient + status-bar UX pattern verbatim; use push-mode `publishDiagnostics` (broadest editor compatibility); ship a lean highlight-focused tree-sitter grammar (~500–1000 lines `grammar.js`) rather than a full 87+43-production EBNF mirror; structure Cargo workspace as `deal/{cli, lsp, deal-ffi}` to avoid duplicated `extern "C"` blocks.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|---|---|---|---|
| Lexical highlighting (offline) | TextMate (VS Code) / tree-sitter (Neovim/Helix/Zed/GitHub) | — | Per D-38; regex/PEG runs without LSP available |
| Type-aware highlighting | LSP semantic tokens (server) | — | Cannot be expressed in regex; needs symbol table |
| Diagnostic surfacing | LSP server (`publishDiagnostics`) | Zig core (E0xxx/E2xxx codes via `deal_diagnostics_json`) | LSP is JSON adapter; Zig owns code allocation |
| AST parsing | Zig core (libdeal.a) | — | D-04, D-42; Rust never reparses |
| Workspace symbol index | Rust (LSP in-memory map) | CLI (writes disk `.deal/index.json`) | D-46/D-48 — LSP owns runtime; CLI owns disk |
| Format | Zig core (`deal_format`) | LSP (`textDocument/formatting` adapter) | D-21 — pretty-printer in Zig; LSP wraps |
| Auto-download binary | TypeScript bootstrap in `vscode-deal` | GitHub Releases | D-50 — extension fetches from Releases; mirrors rust-analyzer |
| Cross-compile libdeal.a | Zig 0.16+ on Linux runner | — | D-53 |
| Native build of deal-lsp | Rust on per-platform runners | — | D-53 — links cross-built static lib |
| Snippet expansion | VS Code core (TextMate snippet JSON) | — | D-38; no LSP completion needed |
| File icons | VS Code icon-theme contribution | — | REQ-3-1 |

## 1. tower-lsp current API

[TBD: Verify exact version pin at planning time via `cargo search tower-lsp`.]

**Recommended pin** [ASSUMED — verify at planning]:
```toml
tower-lsp = { version = "0.20", features = ["runtime-tokio"] }
tokio = { version = "1", features = ["full"] }
```

Training-knowledge baseline (verify before commit):
- `tower-lsp` 0.20.x is the current actively-maintained line as of mid-2025; 0.21+ may exist.
- A community fork `tower-lsp-server` exists in some forks (e.g., `helix-editor`'s usage) — verify if the upstream crate has stalled. [CITED: tower-lsp README at crates.io]

**LanguageServer trait shape** [CITED: tower-lsp 0.20 docs, lib.rs/crates/tower-lsp]:

```rust
use tower_lsp::{LanguageServer, LspService, Server, jsonrpc::Result};
use tower_lsp::lsp_types::*;

#[async_trait::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "deal-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".into(), ":".into(), "<".into()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SEMANTIC_TOKENS_LEGEND.clone(),
                            full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
                            range: Some(false),
                            ..Default::default()
                        },
                    ),
                ),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
        })
    }
    async fn initialized(&self, _: InitializedParams) { /* eager workspace scan per D-47 */ }
    async fn shutdown(&self) -> Result<()> { Ok(()) }
    async fn did_open(&self, params: DidOpenTextDocumentParams) { /* D-42 */ }
    async fn did_change(&self, params: DidChangeTextDocumentParams) { /* D-43 debounce */ }
    async fn did_close(&self, params: DidCloseTextDocumentParams) { /* D-44 — actually keep handle if doc still in workspace; only free on workspace removal */ }
    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> { /* … */ }
    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> { /* LM-2 doc comments */ }
    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> { /* D-46 lookup */ }
    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> { /* deal_format adapter */ }
    async fn semantic_tokens_full(&self, params: SemanticTokensParams) -> Result<Option<SemanticTokensResult>> { /* D-39 */ }
    async fn semantic_tokens_full_delta(&self, params: SemanticTokensDeltaParams) -> Result<Option<SemanticTokensFullDeltaResult>> { /* D-39 */ }
}
```

**`Client` API** (held by `Backend` for outbound messages):
- `client.publish_diagnostics(uri, diagnostics, version).await` — push diagnostics (D-43 driven).
- `client.log_message(MessageType::INFO, "msg").await` — server log → VS Code output channel.
- `client.show_message(MessageType::WARNING, "msg").await` — toast (avoid per D-40; use status bar instead).

**tokio main()** [CITED: tower-lsp examples]:
```rust
#[tokio::main]
async fn main() {
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let (service, socket) = LspService::new(|client| Backend::new(client));
    Server::new(stdin, stdout, socket).serve(service).await;
}
```

**Reference implementations to mirror** [CITED]:
- `rust-analyzer/crates/rust-analyzer/src/bin/main.rs` (different LSP harness; pattern still applies)
- `tower-lsp-boilerplate` (https://github.com/IWANABETHATGUY/tower-lsp-boilerplate) — minimal complete example.

## 2. LSP 3.17+ spec essentials

**Token types & modifiers** [CITED: microsoft/language-server-protocol §SemanticTokens]:

Standard token types (29 total, full list at LSP spec §3.18 SemanticTokenTypes): `namespace, type, class, enum, interface, struct, typeParameter, parameter, variable, property, enumMember, event, function, method, macro, keyword, modifier, comment, string, number, regexp, operator, decorator`.

Standard modifiers (10 total): `declaration, definition, readonly, static, deprecated, abstract, async, modification, documentation, defaultLibrary`.

D-39 locks: types = `keyword, type, parameter, variable, property, namespace, operator, enumMember, regexp`; modifiers = `declaration, definition, readonly, static, deprecated`.

**Encoded format** (`data: number[]` 5-tuples) [CITED: LSP §3.18]:
Each token is `[deltaLine, deltaStart, length, tokenType, tokenModifiersBitset]` relative to the previous token. The LSP client decodes per the `SemanticTokensLegend` declared at `initialize`.

```rust
// SemanticTokensLegend declared at initialize
pub static SEMANTIC_TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,    // index 0
    SemanticTokenType::TYPE,       // index 1
    SemanticTokenType::PARAMETER,  // index 2
    SemanticTokenType::VARIABLE,   // index 3
    SemanticTokenType::PROPERTY,   // index 4
    SemanticTokenType::NAMESPACE,  // index 5
    SemanticTokenType::OPERATOR,   // index 6
    SemanticTokenType::ENUM_MEMBER,// index 7
    SemanticTokenType::REGEXP,     // index 8
];
pub static SEMANTIC_TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::DECLARATION,
    SemanticTokenModifier::DEFINITION,
    SemanticTokenModifier::READONLY,
    SemanticTokenModifier::STATIC,
    SemanticTokenModifier::DEPRECATED,
];
```

**Diagnostic delivery: pull vs push** [CITED: LSP 3.17 §3.17.16 textDocument/diagnostic + §3.17.7 publishDiagnostics]:
- **Pull-mode** (`textDocument/diagnostic`) is LSP 3.17 NEW; client polls server on demand. Server declares `diagnosticProvider` in capabilities.
- **Push-mode** (`textDocument/publishDiagnostics`) is the long-standing pattern; server pushes via `Client::publish_diagnostics`.
- tower-lsp 0.20.x supports both, but push is the dominant pattern across all VS Code / Neovim / Helix / Zed clients [ASSUMED — verify against tower-lsp changelog].
- **Recommendation:** push-mode for Phase 3 (see §11).

**`SemanticTokensLegend` shape** [CITED: LSP spec]:
```jsonc
{ "tokenTypes": ["keyword","type",...], "tokenModifiers": ["declaration","definition",...] }
```

**Capability negotiation** — every capability the server provides MUST be declared in `InitializeResult.capabilities`. Clients that don't see a capability silently fall back. tower-lsp does no auto-discovery; opt-in.

## 3. Tree-sitter grammar dev

[CITED: tree-sitter.github.io/tree-sitter/creating-parsers]

**`grammar.js` DSL essentials:**

```js
// tree-sitter-deal/grammar.js
module.exports = grammar({
  name: 'deal',
  extras: $ => [/\s/, $.line_comment, $.block_comment, $.doc_comment],
  word: $ => $.identifier,
  rules: {
    source_file: $ => repeat($._declaration),

    _declaration: $ => choice(
      $.package_decl,
      $.import_decl,
      $.element_def,
      $.annotation_def,
      // …
    ),

    element_def: $ => seq(
      field('keyword', $.element_keyword),
      field('name', $.identifier),
      optional($.relationship_clause),
      optional($.body),
    ),

    element_keyword: $ => choice(
      'part def','port def','action def','state def','requirement def',
      'constraint def','attribute def','item def','interface def',
      'connection def','flow def',
    ),

    relationship_clause: $ => seq(
      $.relationship_operator,
      commaSep1($.qualified_name),
    ),

    relationship_operator: $ => choice(
      '<<specializes>>','<<redefines>>','<<conjugates>>','<<derives>>',
    ),

    composition_tag: $ => seq(
      '[<', field('name', $.identifier), '>]',
    ),

    annotation_use: $ => seq(
      '@', $.identifier, optional(seq(':', $.relationship_operator)),
    ),

    multiplicity: $ => seq('[', $.multiplicity_range, ']'),
    multiplicity_range: $ => choice(
      seq($.integer, '..', choice($.integer, '*')),
      $.integer,
      '*',
    ),

    qualified_name: $ => seq(
      $.identifier,
      repeat(seq('.', $.identifier)),
    ),

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,
    integer:    $ => /\d+/,
    line_comment:  $ => token(seq('//', /.*/)),
    block_comment: $ => token(seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/')),
    doc_comment:   $ => token(seq('/**', /[^*]*\*+([^/*][^*]*\*+)*/, '/')),
  },
});

function commaSep1(rule) { return seq(rule, repeat(seq(',', rule))); }
```

**`queries/highlights.scm` capture syntax** [CITED: nvim-treesitter conventions]:
```scheme
; queries/highlights.scm
(element_keyword) @keyword.element
(element_def name: (identifier) @type.definition)
(relationship_operator) @operator.relationship
(annotation_use) @attribute.annotation
(composition_tag name: (identifier) @tag.composition)
(multiplicity) @number.multiplicity
(doc_comment) @comment.documentation
(line_comment) @comment
(block_comment) @comment
(qualified_name) @namespace
(integer) @number
```

**`queries/injections.scm`** — minimal for DEAL; only injection sites would be inline `via={...}` JS-like expressions in `.dealx` if any; likely empty in Phase 3:
```scheme
; queries/injections.scm
; (No injections — DEAL has no embedded host language in Phase 3.)
```

**`queries/indents.scm`** [CITED: nvim-treesitter indents.scm convention]:
```scheme
; queries/indents.scm
[
  (body)
  (header_block)
  (composition_tag_body)
] @indent.begin

"}" @indent.end
"]" @indent.end
"[" @indent.branch
```

**`tree-sitter-cli` workflow**:
```bash
npm install --save-dev tree-sitter-cli
npx tree-sitter generate      # produces src/parser.c, src/grammar.json, etc.
npx tree-sitter test          # runs corpus tests in test/corpus/*.txt
npx tree-sitter parse <file>  # debug-parse a single file
npx tree-sitter highlight <file>  # color the file using highlights.scm
```

**Corpus test format** (`test/corpus/element_defs.txt`) [CITED: tree-sitter docs §Testing-Parsers]:
```
==========
basic part def
==========

part def Car {
  port def hvOut;
}

---

(source_file
  (element_def
    keyword: (element_keyword)
    name: (identifier)
    body: (body
      (element_def
        keyword: (element_keyword)
        name: (identifier)))))
```

**GitHub web rendering via Linguist** [CITED: github/linguist docs/CONTRIBUTING.md]:
1. Add `*.deal linguist-detectable` and `*.dealx linguist-detectable` to `.gitattributes`.
2. Submit a PR to `github/linguist` adding:
   - `lib/linguist/languages.yml` entries for `DEAL` and `DEAL Composition`.
   - `samples/DEAL/*.deal` sample files (≥ 500 LOC across samples per Linguist policy).
   - `tm_scope` pointing at the TextMate grammar — OR `tree-sitter-deal` registered as the language's tree-sitter parser.
3. Linguist uses tree-sitter for tokenization but TextMate for highlighting display on github.com (as of mid-2025). Submit BOTH grammars for best results.

## 4. rust-analyzer bootstrap.ts pattern

[CITED: github.com/rust-lang/rust-analyzer/blob/master/editors/code/src/bootstrap.ts]

**Pattern outline:**

```typescript
// vscode-deal/src/bootstrap.ts (paraphrased from rust-analyzer)
import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import * as crypto from 'crypto';
import fetch from 'node-fetch';

const DEAL_LSP_VERSION = '0.3.0';  // D-52: hardcoded pin matching vscode-deal version
const SHA256_MANIFEST: Record<string, string> = {
  'darwin-arm64':   '<sha256>',
  'darwin-x64':     '<sha256>',
  'linux-x64-gnu':  '<sha256>',
  'linux-x64-musl': '<sha256>',
  'win-x64':        '<sha256>',
};

function platformTriple(): string {
  const platform = process.platform;  // 'darwin' | 'linux' | 'win32'
  const arch     = process.arch;      // 'arm64' | 'x64'
  if (platform === 'darwin' && arch === 'arm64') return 'darwin-arm64';
  if (platform === 'darwin' && arch === 'x64')   return 'darwin-x64';
  if (platform === 'linux'  && arch === 'x64')   return detectLinuxLibc();  // gnu vs musl
  if (platform === 'win32'  && arch === 'x64')   return 'win-x64';
  throw new Error(`Unsupported platform: ${platform}-${arch}`);
}

export async function ensureDealLspBinary(ctx: vscode.ExtensionContext): Promise<string> {
  const triple   = platformTriple();
  const filename = process.platform === 'win32' ? 'deal-lsp.exe' : 'deal-lsp';
  const dest     = vscode.Uri.joinPath(ctx.globalStorageUri, DEAL_LSP_VERSION, filename);

  // 1. Bundled binary present? (offline .vsix per D-51)
  const bundled = vscode.Uri.joinPath(ctx.extensionUri, 'server', filename);
  if (await pathExists(bundled)) {
    return bundled.fsPath;
  }

  // 2. Cached download present + checksum matches?
  if (await pathExists(dest) && await sha256(dest.fsPath) === SHA256_MANIFEST[triple]) {
    return dest.fsPath;
  }

  // 3. First-run dialog — confirm download
  const proceed = await vscode.window.showInformationMessage(
    `DEAL LSP server (${DEAL_LSP_VERSION}) is not installed. Download from GitHub Releases?`,
    'Download', 'Cancel',
  );
  if (proceed !== 'Download') throw new Error('User cancelled deal-lsp download');

  // 4. Download with progress
  const url = `https://github.com/deal-lang/deal/releases/download/v${DEAL_LSP_VERSION}/deal-lsp-${triple}.tar.gz`;
  await vscode.workspace.fs.createDirectory(vscode.Uri.joinPath(ctx.globalStorageUri, DEAL_LSP_VERSION));
  await vscode.window.withProgress(
    { location: vscode.ProgressLocation.Notification, title: 'Downloading deal-lsp', cancellable: false },
    async (progress) => { await downloadAndExtract(url, dest.fsPath, progress); },
  );

  // 5. Verify checksum
  const actual = await sha256(dest.fsPath);
  if (actual !== SHA256_MANIFEST[triple]) {
    await vscode.workspace.fs.delete(dest);
    throw new Error(`Checksum mismatch for deal-lsp ${triple}: expected ${SHA256_MANIFEST[triple]}, got ${actual}`);
  }

  // 6. chmod +x on Unix
  if (process.platform !== 'win32') fs.chmodSync(dest.fsPath, 0o755);

  return dest.fsPath;
}
```

**Key elements to replicate** [CITED: rust-analyzer/editors/code/src/bootstrap.ts]:
1. **Platform detection** via `process.platform` + `process.arch`.
2. **Versioned storage** under `context.globalStorageUri/v{X}/` (preserves multiple installed versions, supports clean rollback).
3. **Pinned URL** including the exact `vscode-deal` version (D-52).
4. **SHA-256 manifest** as a `Record<triple, sha>` baked into the extension; mismatch → delete + throw.
5. **First-run dialog** with `'Download' | 'Cancel'` choice.
6. **`vscode.window.withProgress`** for UX during download.
7. **`chmod +x`** after extraction on Unix.
8. **Bundled-first lookup** (offline `.vsix` per D-51 ships binary at `<ext>/server/deal-lsp`).

## 5. VS Code Extension API

[CITED: code.visualstudio.com/api]

**`package.json` extension manifest** (canonical shape):
```jsonc
{
  "name": "vscode-deal",
  "displayName": "DEAL Language",
  "version": "0.3.0",
  "publisher": "deal-lang",
  "engines": { "vscode": "^1.95.0" },
  "activationEvents": [
    "onLanguage:deal",
    "onLanguage:dealx",
    "workspaceContains:**/deal.toml"
  ],
  "main": "./out/extension.js",
  "contributes": {
    "languages": [
      { "id": "deal",  "aliases": ["DEAL"], "extensions": [".deal"],
        "configuration": "./language-configuration.json",
        "icon": { "light": "./icons/deal-light.svg", "dark": "./icons/deal-dark.svg" } },
      { "id": "dealx", "aliases": ["DEAL Composition"], "extensions": [".dealx"],
        "configuration": "./language-configuration.json",
        "icon": { "light": "./icons/dealx-light.svg", "dark": "./icons/dealx-dark.svg" } }
    ],
    "grammars": [
      { "language": "deal",  "scopeName": "source.deal",  "path": "./syntaxes/deal.tmLanguage.json" },
      { "language": "dealx", "scopeName": "source.dealx", "path": "./syntaxes/dealx.tmLanguage.json" }
    ],
    "snippets": [
      { "language": "deal",  "path": "./snippets/deal.json" },
      { "language": "dealx", "path": "./snippets/dealx.json" }
    ],
    "configuration": {
      "title": "DEAL",
      "properties": {
        "deal.lsp.path":  { "type": "string", "default": "", "description": "Override path to deal-lsp binary." },
        "deal.lsp.trace": { "type": "string", "enum": ["off","messages","verbose"], "default": "off" }
      }
    },
    "commands": [
      { "command": "deal.restartServer", "title": "DEAL: Restart Language Server" },
      { "command": "deal.showOutput",    "title": "DEAL: Show Output Channel" }
    ]
  }
}
```

**Engine pin recommendation:** `^1.95.0` [ASSUMED — verify at planning]. Why: VS Code 1.95 (Oct 2024) shipped stable LSP semantic-tokens + pull-diagnostics support. Older baselines (1.85+) likely work for push-only diagnostics, but 1.95 keeps options open. Cursor and VSCodium track the same engine version.

**`vscode-languageclient/node` setup** [CITED: code.visualstudio.com/api/language-extensions/language-server-extension-guide]:
```typescript
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from 'vscode-languageclient/node';

let client: LanguageClient;

export async function activate(context: vscode.ExtensionContext) {
  const lspPath = vscode.workspace.getConfiguration('deal').get<string>('lsp.path')
                  || await ensureDealLspBinary(context);

  const serverOptions: ServerOptions = {
    run:   { command: lspPath, args: [], transport: TransportKind.stdio },
    debug: { command: lspPath, args: ['--log=debug'], transport: TransportKind.stdio },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'deal'  },
      { scheme: 'file', language: 'dealx' },
    ],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher('**/deal.toml'),
    },
    outputChannelName: 'DEAL Language Server',
  };

  client = new LanguageClient('deal', 'DEAL Language Server', serverOptions, clientOptions);
  await client.start();
  context.subscriptions.push(statusBar(client));
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
```

**SemanticTokensProvider registration:** automatic. When `deal-lsp` declares `semanticTokensProvider` in `InitializeResult.capabilities`, `vscode-languageclient/node` registers the provider in VS Code transparently. No extra TS code needed [CITED: microsoft/vscode-languageserver-node README].

**Status bar API** [CITED: code.visualstudio.com/api/references/vscode-api#StatusBarItem]:
```typescript
function statusBar(client: LanguageClient): vscode.Disposable {
  const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  item.command = 'deal.showOutput';

  client.onDidChangeState(e => {
    switch (e.newState) {
      case 1 /*Stopped*/:  item.text = '$(error) DEAL LSP'; item.tooltip = 'DEAL LSP error — click for output'; break;
      case 2 /*Starting*/: item.text = '$(sync~spin) DEAL LSP'; item.tooltip = 'DEAL LSP starting…'; break;
      case 3 /*Running*/:  item.text = '$(check) DEAL LSP'; item.tooltip = 'DEAL LSP ready'; break;
    }
    item.show();
  });

  return item;
}
```

**Test runner** [CITED: code.visualstudio.com/api/working-with-extensions/testing-extension]:
```jsonc
// package.json devDependencies
"@vscode/test-electron": "^2.4.0",
"@vscode/test-cli":      "^0.0.10",
"@types/vscode":         "^1.95.0",
"mocha":                 "^10.0.0",
"sinon":                 "^17.0.0"
```

Run headless on CI with `xvfb-run npm test` on Linux.

## 6. TextMate scope naming conventions

[CITED: macromates.com/manual/en/language_grammars + code.visualstudio.com/api/language-extensions/syntax-highlight-guide]

**Hierarchy (standard top-level scopes):**
- `comment.*` — comments (`comment.line.double-slash`, `comment.block.documentation`)
- `constant.*` — literal constants (`constant.numeric`, `constant.language.boolean`)
- `entity.name.*` — declared name (`entity.name.type`, `entity.name.function`, `entity.name.tag`)
- `entity.other.*` — other entity (`entity.other.attribute-name`, `entity.other.inherited-class`)
- `keyword.*` — keywords (`keyword.control`, `keyword.operator`)
- `meta.*` — broad regions (`meta.function-call`, `meta.block`)
- `punctuation.*` — punctuation (`punctuation.definition.string.begin`, `punctuation.separator.comma`)
- `storage.*` — storage modifiers (`storage.type.class`, `storage.modifier`)
- `string.*` — strings (`string.quoted.double`, `string.regexp`)
- `support.*` — language-supplied (`support.function`, `support.type`)
- `variable.*` — variables (`variable.parameter`, `variable.language.this`)

**Theme-friendly scope picks for DEAL:** VS Code Dark+/Light+/GitHub Dark themes color every top-level scope by default. To pick the right shade, append the DEAL-specific suffix LAST so the prefix matches the theme's default rule:

| DEAL construct | TextMate scope |
|---|---|
| `part def`, `port def` keywords | `keyword.control.element.deal` |
| `in`, `out`, `inout` direction | `keyword.other.direction.deal` |
| `abstract`, `variation` modifiers | `storage.modifier.deal` |
| `import`, `package` | `keyword.control.import.deal` |
| Definition name (`BatteryPack`) | `entity.name.type.deal` |
| Reference to a definition | `entity.name.type.reference.deal` |
| Package path segments | `entity.name.namespace.deal` |
| `<<specializes>>`, `<<redefines>>` | `keyword.operator.relationship.deal` |
| `@trace`, `@simulation`, `@connection` | `support.type.annotation.deal` |
| `[1..*]`, `[2..5]` multiplicity | `constant.numeric.multiplicity.deal` |
| `[<system>]`, `[<connect>]` composition tag | `entity.other.attribute-name.composition.deal` |
| `/** … */` doc comments | `comment.block.documentation.deal` |
| `// …` line comments | `comment.line.double-slash.deal` |
| `/* … */` block comments | `comment.block.deal` |
| `"…"` strings | `string.quoted.double.deal` |
| Integers | `constant.numeric.integer.deal` |
| Punctuation: `{ }` | `punctuation.section.block.{begin,end}.deal` |
| Punctuation: `[< >]` brackets | `punctuation.definition.tag.{begin,end}.deal` |
| Punctuation: `<< >>` | `punctuation.definition.operator.{begin,end}.deal` |

Why this picks well: `keyword.control.*` colors like `if`/`return` (purple in Dark+), which matches the "control flow" feel of element declarations. `entity.name.type.*` colors like class names (teal in Dark+). `keyword.operator.*` matches operators (light gray/white). `support.type.*` matches `@decorators` in TS (yellow). `constant.numeric.*` matches numbers (light green). All cleanly themable.

## 7. Semantic-token mapping table (D-39 lock-in draft)

[ASSUMED — concrete mappings; planner refines against actual Phase 1 AST kinds.]

| DEAL construct | LSP token type | LSP modifiers | Notes |
|---|---|---|---|
| `part def`, `port def`, `action def`, `state def`, `requirement def`, `constraint def`, `attribute def`, `item def`, `interface def`, `connection def`, `flow def` keywords | `keyword` | `declaration` | 11 element keywords per SD-1 |
| `in`, `out`, `inout` direction keywords | `keyword` | — | |
| `abstract`, `variation` modifiers | `keyword` | `static` | `static` modifier signals "compile-time / definition-modifier" |
| `import`, `package`, `as` | `keyword` | — | |
| Definition name (e.g. `BatteryPack`) at declaration site | `type` | `declaration`, `definition` | |
| Reference to a definition (e.g. `: BatteryPack`) | `type` | — | |
| Package path segments (e.g. `sedan_project.vehicle`) | `namespace` | — | Each segment is its own token |
| `<<specializes>>`, `<<redefines>>`, `<<conjugates>>`, `<<derives>>` | `operator` | — | |
| `@trace`, `@simulation`, `@connection`, `@header`, `@doc` | `enumMember` | — | Annotation category names |
| `[1..*]`, `[0..1]`, `[2]` multiplicity | `regexp` | — | `regexp` token reused for "non-textual structural pattern" — themes color it as a literal-like construct |
| `[<system>]`, `[<connect>]`, `[<satisfy>]`, `[<allocate>]`, `[<traceability>]` composition tags | `enumMember` | — | Same color family as annotations — both are "category labels" |
| Property/feature name within a body (e.g. `voltage : Real;`) | `property` | `declaration` | Distinguishes from type references |
| Property reference (e.g. `motor.voltage`) | `property` | — | |
| Parameter in action def signature | `parameter` | `declaration` | |
| Local variable | `variable` | — | |
| `readonly` features / constants | `variable` or `property` | `readonly` | |
| Deprecated features (`@deprecated` annotation) | (inherit from underlying kind) | `deprecated` | |
| `/** */` doc comments | `comment` | `documentation` | Per LM-2 |
| Inline `// …` and `/* … */` | (no semantic token; falls through to TextMate) | — | TextMate handles plain comments |

**Ambiguities to resolve at PLAN-time** [TBD]:
- Whether to emit semantic tokens for all comments or only doc comments (recommend only doc comments — let TextMate handle plain comments to avoid duplicate coloring).
- Whether composition tags (`[<system>]`) should be `enumMember` or a future LSP type if standardized. Reusing `enumMember` keeps theme support universal.
- Whether `<<derives>>` and `<<conjugates>>` should distinguish from `<<specializes>>` via a modifier — recommend a uniform `operator` token; TextMate already differentiates via the `keyword.operator.relationship.{specializes,redefines,conjugates,derives}.deal` scope.

## 8. Color category names (D-41 parity)

Single table — rows are color categories shared between TextMate and tree-sitter; visual parity guaranteed when both grammars use the same category name.

| Category | TextMate scope | Tree-sitter capture |
|---|---|---|
| Element keyword (`part def`, `port def`, …) | `keyword.control.element.deal` | `@keyword.element` |
| Direction keyword (`in`, `out`, `inout`) | `keyword.other.direction.deal` | `@keyword.direction` |
| Modifier (`abstract`, `variation`) | `storage.modifier.deal` | `@keyword.modifier` |
| Import/package keyword | `keyword.control.import.deal` | `@keyword.import` |
| Definition name (declaration) | `entity.name.type.deal` | `@type.definition` |
| Definition reference | `entity.name.type.reference.deal` | `@type` |
| Namespace / package segment | `entity.name.namespace.deal` | `@namespace` |
| Relationship operator (`<<specializes>>` etc.) | `keyword.operator.relationship.deal` | `@operator.relationship` |
| Annotation category (`@trace`, `@simulation`) | `support.type.annotation.deal` | `@attribute.annotation` |
| Composition tag (`[<system>]`) | `entity.other.attribute-name.composition.deal` | `@tag.composition` |
| Multiplicity (`[1..*]`) | `constant.numeric.multiplicity.deal` | `@number.multiplicity` |
| Property declaration | `variable.other.property.deal` | `@property.declaration` |
| Property reference | `variable.other.property.reference.deal` | `@property` |
| Parameter | `variable.parameter.deal` | `@parameter` |
| Doc comment | `comment.block.documentation.deal` | `@comment.documentation` |
| Line comment | `comment.line.double-slash.deal` | `@comment` |
| Block comment | `comment.block.deal` | `@comment` |
| String literal | `string.quoted.double.deal` | `@string` |
| Integer literal | `constant.numeric.integer.deal` | `@number` |
| Punctuation (braces, brackets, angle-brackets) | `punctuation.definition.*.deal` | `@punctuation.bracket` / `@punctuation.delimiter` |

This is the spec to write into `vscode-deal/COLOR-CATEGORIES.md` (or whatever filename the planner picks per D-41).

## 9. Tree-sitter grammar scope decision

**Recommendation: lean highlight-focused grammar.**

**Rationale:**
1. **Zig parser is the authoritative AST** (Phase 1 D-04). Maintaining a full 87+43-production EBNF mirror in tree-sitter doubles maintenance: every grammar change in `spec/grammar/*.ebnf` requires a parallel `grammar.js` change.
2. **Tree-sitter only needs to discriminate token categories + structural anchors** (`@header { ... }`, `[< >]`, `<< >>`, comment regions, declaration boundaries) — not full AST semantic accuracy.
3. **Lean grammar is faster to author** (~500–1000 lines `grammar.js`) and tracks the lexical layer (`spec/grammar/lexical.ebnf`, 758 lines) plus only enough of the structural layer to drive the 20 color categories in §8.
4. **REQ-3-2 acceptance criterion** is "parses showcase corpus without errors and highlight queries discriminate the visual hierarchy from D-41" — explicitly a highlight-discrimination bar, not full-AST-parity.

**Trade-off:**
- Less-accurate fallback when the LSP isn't available. In Neovim/Helix/Zed/GitHub web, tree-sitter IS the highlight + indent source — VS Code has TextMate, but other editors have only tree-sitter. Lean grammar means lower-fidelity highlighting in those editors than full-AST grammar would provide.
- Mitigation: the lean grammar still discriminates all 20 color categories in §8. The only fidelity loss is for highly nested or rare structural forms (e.g., disambiguating between `<<specializes>>` and `<<redefines>>` semantically — but `keyword.operator.relationship.deal` is the same color for both, so the user sees no difference).

**Scope of lean grammar:**
- Lexical: comments, strings, identifiers, integers, all 11 element keywords (`part def` etc.), direction keywords (`in`/`out`/`inout`), modifiers (`abstract`/`variation`), `<<…>>` operators (4 variants), `[<…>]` composition tags, `[…]` multiplicity, `@identifier` annotations, `/** */` doc comments.
- Structural: package decl, import decl, element def with optional relationship clause and body, body block, header block (`@header { ... }`), annotation use, composition-tag block, multiplicity expression.
- Out-of-scope: full expression grammar (just enough to parse `{ … }` bodies + match braces); SysML v2 constraint expression syntax beyond what `highlights.scm` queries discriminate.

**Estimated lines of `grammar.js`:** 600–900.

## 10. Cargo workspace layout

**Recommendation:** add `"lsp"` and `"deal-ffi"` to `deal/Cargo.toml` workspace members.

```toml
# deal/Cargo.toml
[workspace]
resolver = "2"
members = ["cli", "lsp", "deal-ffi", "tests/ffi"]

[workspace.dependencies]
# (existing entries preserved)
clap        = { version = "4.6", features = ["derive"] }
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
jsonschema  = { version = "0.46", default-features = false }
anstream    = "1"
owo-colors  = "4"
anyhow      = "1"
thiserror   = "2"
insta       = { version = "1.47", features = ["json"] }
uuid        = { version = "1", features = ["v5"] }

# NEW (Phase 3)
tower-lsp   = { version = "0.20", features = ["runtime-tokio"] }
tokio       = { version = "1", features = ["full"] }
tower       = "0.5"
dashmap     = "6"          # concurrent map for per-document handle table
ropey       = "1.6"        # incremental text rope for didChange
bindgen     = "0.70"       # build-time C ABI wrapper generation (in deal-ffi/build.rs)
```

**New crate: `deal/deal-ffi/`** (extract from existing `cli/src/ffi.rs`):
```toml
# deal/deal-ffi/Cargo.toml
[package]
name    = "deal-ffi"
version = "0.3.0"
edition = "2021"

[lib]
name      = "deal_ffi"
crate-type = ["rlib"]

[build-dependencies]
bindgen = { workspace = true }

[dependencies]
# (no external dependencies — pure FFI wrappers)
```

The `deal-ffi/build.rs` runs `bindgen` over `../include/deal.h` and produces `bindings.rs`. The lib exports safe Rust wrappers around the 9 C symbols + an `OwnedDealHandle` newtype.

**New crate: `deal/lsp/`**:
```toml
# deal/lsp/Cargo.toml
[package]
name    = "deal-lsp"
version = "0.3.0"
edition = "2021"

[[bin]]
name = "deal-lsp"
path = "src/main.rs"

[dependencies]
deal-ffi    = { path = "../deal-ffi" }
tower-lsp   = { workspace = true }
tokio       = { workspace = true }
tower       = { workspace = true }
dashmap     = { workspace = true }
ropey       = { workspace = true }
serde       = { workspace = true }
serde_json  = { workspace = true }
anyhow      = { workspace = true }
thiserror   = { workspace = true }
tracing             = "0.1"
tracing-subscriber  = { version = "0.3", features = ["env-filter"] }
```

**Update `deal/cli/Cargo.toml`** to depend on `deal-ffi`:
```toml
[dependencies]
deal-ffi = { path = "../deal-ffi" }
# (drop the local ffi module)
```

**Justification:**
- Avoids drift between two `extern "C"` blocks (Phase 2's CLI vs. Phase 3's LSP).
- Phase 4 (`deal-stdlib`-aware completion, docs-site CLI) and Phase 5 (`deal simulate` integration) can depend on `deal-ffi` instead of re-declaring the FFI surface.
- Keeps `bindgen` build-script complexity in one crate.

## 11. Diagnostic delivery mode

**Recommendation: push-mode (`publishDiagnostics`) for Phase 3.**

**Why:**
1. **Wider client support.** Neovim/Helix LSP clients still primarily implement push; pull-mode is universally negotiated but push remains the well-trodden path.
2. **tower-lsp push is well-trodden.** Pull-mode (LSP 3.17, added 2022) has rougher edges in tower-lsp 0.20 [ASSUMED — verify changelog at planning].
3. **Aligns with D-43 debounced re-parse pattern.** After re-parse, server emits diagnostics directly — no client poll needed; no caching of stale diagnostics across the debounce window.
4. **No degraded UX vs pull.** Both modes produce identical diagnostic display in VS Code; pull is mainly an optimization for clients that want to deduplicate diagnostics across multiple servers.

**Server-side shape:**
```rust
async fn refresh_diagnostics(&self, uri: Url, version: i32) {
    let diagnostics = self.compute_diagnostics(&uri).await;
    self.client.publish_diagnostics(uri, diagnostics, Some(version)).await;
}
```

**Revisit at Phase 5/6** if pull-mode becomes the default; not a Phase 3 concern.

## 12. CI matrix for D-53

`.github/workflows/release.yml` — multi-job pipeline.

**Triggers:**
- Push tag `v*.*.*` → full release pipeline.
- Manual `workflow_dispatch` for marketplace re-publish.

**Job 1 — `build-libdeal`** (on `ubuntu-22.04`):
```yaml
build-libdeal:
  runs-on: ubuntu-22.04
  strategy:
    matrix:
      target:
        - aarch64-macos
        - x86_64-macos
        - x86_64-linux-gnu
        - x86_64-linux-musl
        - x86_64-windows
  steps:
    - uses: actions/checkout@v4
      with: { submodules: recursive }
    - uses: mlugg/setup-zig@v1
      with: { version: 0.16.0 }
    - run: cd deal && zig build -Doptimize=ReleaseSafe -Dtarget=${{ matrix.target }}
    - uses: actions/upload-artifact@v4
      with:
        name: libdeal-${{ matrix.target }}
        path: deal/zig-out/lib/libdeal.a
```

**Job 2 — `build-deal-lsp`** (matrix of 4 native runners):
```yaml
build-deal-lsp:
  needs: build-libdeal
  strategy:
    matrix:
      include:
        - os: macos-14         # arm64
          triple: darwin-arm64
          libdeal-target: aarch64-macos
        - os: macos-13         # x64
          triple: darwin-x64
          libdeal-target: x86_64-macos
        - os: ubuntu-22.04
          triple: linux-x64-gnu
          libdeal-target: x86_64-linux-gnu
        - os: windows-2022
          triple: win-x64
          libdeal-target: x86_64-windows
  runs-on: ${{ matrix.os }}
  steps:
    - uses: actions/checkout@v4
      with: { submodules: recursive }
    - uses: actions/download-artifact@v4
      with:
        name: libdeal-${{ matrix.libdeal-target }}
        path: deal/zig-out/lib/
    - uses: dtolnay/rust-toolchain@stable
    - run: cd deal && cargo build --release -p deal-lsp
    - run: cd deal && cargo test --workspace
    - if: matrix.triple == 'linux-x64-gnu'
      run: cd deal && zig build phase-3-gate-fresh
    - uses: actions/upload-artifact@v4
      with:
        name: deal-lsp-${{ matrix.triple }}
        path: deal/target/release/deal-lsp*
```

**Job 3 — `package-vsix`** (on `ubuntu-22.04`):
```yaml
package-vsix:
  needs: build-deal-lsp
  runs-on: ubuntu-22.04
  steps:
    - uses: actions/checkout@v4
    - uses: actions/setup-node@v4
      with: { node-version: 20 }
    - uses: actions/download-artifact@v4
      with: { pattern: deal-lsp-*, path: artifacts/ }
    - run: cd vscode-deal && npm ci
    - run: cd vscode-deal && npm run build
    # Main .vsix (no bundled binary — auto-download per D-50)
    - run: cd vscode-deal && npx vsce package -o vscode-deal.vsix
    # Per-platform offline .vsix (bundled binary per D-51)
    - run: |
        for triple in darwin-arm64 darwin-x64 linux-x64-gnu win-x64; do
          mkdir -p vscode-deal/server
          cp artifacts/deal-lsp-$triple/deal-lsp* vscode-deal/server/
          cd vscode-deal && npx vsce package --target ${triple/-x64-gnu/x64/} -o vscode-deal-$triple-offline.vsix
          rm -rf server
        done
    - uses: actions/upload-artifact@v4
      with: { name: vsix, path: vscode-deal/*.vsix }
```

**Job 4 — `publish-release`**:
```yaml
publish-release:
  needs: package-vsix
  runs-on: ubuntu-22.04
  steps:
    - uses: actions/download-artifact@v4
    - name: Generate SHA-256 manifest
      run: |
        cd deal-lsp-* && sha256sum deal-lsp* > ../SHA256SUMS
    - uses: softprops/action-gh-release@v2
      with:
        files: |
          deal-lsp-*/deal-lsp*
          vsix/*.vsix
          SHA256SUMS
```

**Job 5 — `publish-marketplace`** (manual approval, environment-gated):
```yaml
publish-marketplace:
  needs: publish-release
  environment: marketplace
  runs-on: ubuntu-22.04
  steps:
    - uses: actions/download-artifact@v4
      with: { name: vsix }
    - run: |
        npx vsce publish --packagePath vscode-deal.vsix             --pat ${{ secrets.VSCE_PAT }}
        for vsix in vscode-deal-*-offline.vsix; do
          npx vsce publish --packagePath $vsix --pat ${{ secrets.VSCE_PAT }}
        done
```

`phase-3-gate-fresh` runs once in Job 2's `linux-x64-gnu` row as a regression gate. `phase-3-gate` (non-fresh) runs in every Job 2 row implicitly via `cargo test --workspace`.

## 13. phase-3-gate + phase-3-gate-fresh design

**`zig build phase-3-gate` step list** (added to `deal/build.zig`):

```zig
// deal/build.zig (excerpt)
const phase_3_gate = b.step("phase-3-gate", "Phase 3 acceptance gate");
phase_3_gate.dependOn(&b.addRunArtifact(zig_test_runner).step);          // (1) Zig core
phase_3_gate.dependOn(b.step("phase-2-gate", ""));                       // (2) Phase 2 regression
phase_3_gate.dependOn(&b.addSystemCommand(&.{ "cargo", "build", "--workspace", "--release", "--manifest-path", "Cargo.toml" }).step);   // (3) Rust build
phase_3_gate.dependOn(&b.addSystemCommand(&.{ "cargo", "test", "--workspace", "--manifest-path", "Cargo.toml" }).step);                  // (4) Rust tests
phase_3_gate.dependOn(&b.addSystemCommand(&.{ "bash", "-c", "cd ../tree-sitter-deal && npm ci && npm test" }).step);                     // (5) tree-sitter corpus
phase_3_gate.dependOn(&b.addSystemCommand(&.{ "bash", "-c", "cd ../vscode-deal && npm ci && xvfb-run -a npm test" }).step);             // (6) vscode-deal extension tests
phase_3_gate.dependOn(&b.addSystemCommand(&.{ "bash", "-c", "cd ../vscode-deal && npm run package" }).step);                            // (7) .vsix sanity build
phase_3_gate.dependOn(&b.addSystemCommand(&.{ "bash", "scripts/phase-3-smoke.sh" }).step);                                              // (8) integration smoke
```

**`scripts/phase-3-smoke.sh`** [TBD: planner finalizes script]:
```bash
#!/usr/bin/env bash
# Spawn deal-lsp, open showcase, exercise the five capabilities.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -p deal-lsp
cargo run --release -p deal-lsp-smoke -- tests/showcase
```

Inside `deal-lsp-smoke` (a separate dev binary or a `#[test]`-gated harness):
- Open showcase, send `initialize` + `initialized`, wait for index population.
- `did_open` each showcase `.deal`/`.dealx` file.
- Assert diagnostics match Phase 2 expected E2xxx counts/codes.
- Send `textDocument/completion` at known position; assert candidates contain expected keyword + type names.
- Send `textDocument/definition` on a `<<specializes>>` reference; assert location matches expected file:range.
- Send `textDocument/hover` on an element with `/** */`; assert markdown payload contains doc text.
- Send `textDocument/formatting`; assert output bytes match a frozen golden.
- Send `textDocument/semanticTokens/full`; assert encoded data equals golden snapshot.
- Exit 0 on full pass.

**`zig build phase-3-gate-fresh`**:
```zig
const phase_3_gate_fresh = b.step("phase-3-gate-fresh", "Phase 3 gate inside fresh worktree");
phase_3_gate_fresh.dependOn(&b.addSystemCommand(&.{ "bash", "scripts/verify-fresh-worktree.sh", "phase-3-gate" }).step);
```

This extends the existing `verify-fresh-worktree.sh` pattern (Phase 1.5 ADR-phase-1.5-fresh-worktree-verification). The script:
1. Creates an ephemeral git worktree at a temp path.
2. `git submodule update --init --recursive` (mounts `spec/` → `tests/showcase` symlink works).
3. `zig build phase-3-gate` inside the ephemeral worktree.
4. Removes the worktree on completion.

## 14. Snippet starting set

19 snippets total (confirmed against SD-1..SD-20 + CS-1..CS-16 [ASSUMED — verify against DESIGN-DECISIONS.md at planning time]):

| # | Prefix | Description | Body summary |
|---|---|---|---|
| 1 | `partdef` | `part def` declaration | `part def $1 {\n  $0\n}` |
| 2 | `portdef` | `port def` declaration | `port def $1 {\n  direction: ${2|in,out,inout|};\n  $0\n}` |
| 3 | `actiondef` | `action def` declaration | `action def $1($2) {\n  $0\n}` |
| 4 | `statedef` | `state def` declaration | `state def $1 {\n  $0\n}` |
| 5 | `reqdef` | `requirement def` | `requirement def $1 {\n  doc: "$2";\n  $0\n}` |
| 6 | `constraintdef` | `constraint def` | `constraint def $1 {\n  $0\n}` |
| 7 | `attrdef` | `attribute def` | `attribute def $1 : $2;` |
| 8 | `itemdef` | `item def` | `item def $1 {\n  $0\n}` |
| 9 | `ifacedef` | `interface def` | `interface def $1 {\n  $0\n}` |
| 10 | `conndef` | `connection def` | `connection def $1 {\n  $0\n}` |
| 11 | `flowdef` | `flow def` | `flow def $1 {\n  source: $2;\n  target: $3;\n}` |
| 12 | `tagsys` | `[<system>]` composition tag | `[<system>]\n$1\n[</system>]` |
| 13 | `tagcon` | `[<connect>]` composition tag | `[<connect from={$1} to={$2}/>]` |
| 14 | `tagsat` | `[<satisfy>]` composition tag | `[<satisfy req={$1}>]\n$0\n[</satisfy>]` |
| 15 | `tagalloc` | `[<allocate>]` composition tag | `[<allocate to={$1}>]\n$0\n[</allocate>]` |
| 16 | `tagtrace` | `[<traceability>]` composition tag | `[<traceability from={$1} to={$2}/>]` |
| 17 | `header` | `@header { ... }` block | `@header {\n  $0\n}` |
| 18 | `import` | `import` statement | `import $1;` |
| 19 | `pkg` | `package` declaration | `package $1;\n$0` |

These ship as two JSON files: `snippets/deal.json` (snippets 1–11, 17–19) and `snippets/dealx.json` (snippets 12–19; the composition tags are `.dealx`-only). Snippets 17–19 apply to both file types so they appear in both JSONs.

## 15. Validation Architecture

> Mandatory per Nyquist Dimension 8 / `gsd-plan-phase` step 5.5.

### Test Framework
| Property | Value |
|---|---|
| Framework (Rust) | `cargo test` + `tokio::test` (`tokio = "1"`) |
| Framework (TS) | Mocha 10 via `@vscode/test-electron` 2.4 |
| Framework (tree-sitter) | `tree-sitter test` from `tree-sitter-cli` |
| Config file (Rust) | `deal/lsp/Cargo.toml` `[dev-dependencies]` |
| Config file (TS) | `vscode-deal/.vscode-test.mjs` |
| Config file (tree-sitter) | `tree-sitter-deal/test/corpus/*.txt` (test files = config) |
| Quick run (Rust) | `cargo test -p deal-lsp` |
| Quick run (TS) | `cd vscode-deal && npm test` |
| Quick run (tree-sitter) | `cd tree-sitter-deal && npm test` |
| Full suite | `zig build phase-3-gate` |

### Unit tests
- **Rust unit tests in `deal/lsp/src/`** — semantic-token encoder/decoder, path-string resolver, in-memory index merge logic, debounce timer scheduler, FFI wrapper safety (Cow-clone before deal_free).
- **vscode-deal TypeScript unit tests** — TextMate scope JSON snapshot tests (assert grammar JSON byte-identical to a frozen snapshot), status-bar state machine, bootstrap.ts platform-triple detection, SHA-256 mismatch handling.

### Tree-sitter corpus tests
- **`tree-sitter-deal/test/corpus/`** — One `.txt` per showcase file (19 total). Each file pairs a code excerpt with an expected parse tree (see §3 corpus format). Run via `tree-sitter test`.
- **Highlight snapshot** — `tree-sitter highlight` over each showcase file; capture output and compare against a checked-in golden (insta-style).

### LSP integration tests
- **`deal/lsp/tests/showcase.rs`** — `tokio::test`s that spawn `deal-lsp` over in-process channels (`tower-lsp::LspService` + `Server::new(reader, writer, socket)`). Test list:
  - `diagnostics_match_phase_2_codes` — `did_open` known-bad showcase files; assert E2xxx codes at expected spans.
  - `completion_returns_element_keywords` — completion at a known position; assert candidates contain expected element-keyword + definition-name set.
  - `definition_lookup_specializes` — definition request on a `<<specializes>>` reference; assert location.
  - `hover_renders_jsdoc` — hover on an element preceded by `/** */`; assert markdown payload contains doc text per LM-2.
  - `format_round_trip` — `textDocument/formatting`; assert output bytes match a checked-in golden (showcase files known-canonical).
  - `semantic_tokens_match_golden` — `textDocument/semanticTokens/full` for known span; assert encoded `data: number[]` equals golden snapshot.
  - `workspace_index_populated_after_initialize` — assert `HashMap<PathString, _>` size matches expected symbol count after `initialized`.
  - `debounce_collapses_rapid_changes` — fire 5 `didChange` within 200 ms; assert only one re-parse runs.

### vscode-deal extension tests
- **`vscode-deal/src/test/`** — `@vscode/test-electron` headless suite (xvfb on Linux CI). Tests:
  - `activates_on_deal_toml` — open a workspace containing `deal.toml`; assert extension activated.
  - `language_client_connects` — assert `LanguageClient.state === Running` within 5 s.
  - `status_bar_transitions` — assert status-bar text transitions through `starting…` → `ready` on cold open.
  - `auto_download_url_correct` — mock the download; assert the URL contains the pinned version + correct platform triple.
  - `sha256_mismatch_rejects` — mock a wrong checksum; assert binary is deleted and error surfaced.
  - `format_on_save_invokes_lsp` — open a showcase file with edits, save; assert `textDocument/formatting` request sent and edits applied.

### Phase gates
- `zig build phase-3-gate` — full sequence per §13.
- `zig build phase-3-gate-fresh` — phase-3-gate inside `verify-fresh-worktree.sh` ephemeral worktree.
- `zig build phase-2-gate-fresh` continues to be required (regression).

### Sampling rate
- **Per task commit:** `cargo test -p deal-lsp` + relevant TS/tree-sitter unit suite.
- **Per wave merge:** `cargo test --workspace` + `cd tree-sitter-deal && npm test` + `cd vscode-deal && npm test`.
- **Phase gate:** `zig build phase-3-gate` green; then `zig build phase-3-gate-fresh` for closeout.

### Wave 0 gaps
- [ ] `deal/lsp/Cargo.toml` — create new crate.
- [ ] `deal/deal-ffi/Cargo.toml` + `build.rs` + `src/lib.rs` — extract from `cli/src/ffi.rs`.
- [ ] `tree-sitter-deal/grammar.js` + `package.json` + `test/corpus/` skeleton — create from scratch.
- [ ] `vscode-deal/package.json` + `src/extension.ts` + `src/bootstrap.ts` + `src/test/` — create from scratch.
- [ ] `deal/scripts/phase-3-smoke.sh` — new script.
- [ ] `deal/build.zig` — add `phase-3-gate` and `phase-3-gate-fresh` steps.
- [ ] `vscode-deal/.vscode-test.mjs` — extension-test runner config.

### Smoke benchmarks (informational, not gating)
- Showcase open → LSP `ready` < 1 s on M1 / Ryzen 5 hardware.
- Edit → diagnostic refresh < 500 ms on the largest showcase file (~600 LOC).
- Workspace-symbol lookup < 50 ms.

### Counterfactuals (intentionally not built in Phase 3)
- No `references`, `rename`, `code action`, `signature help`, `inlay hint`.
- No simulation-aware hover / completion — Phase 5.
- No stdlib-aware completion — Phase 4.
- No WASM-packaged `deal-lsp` — out of roadmap.
- No formal performance benchmark suite — Phase 5/6.

## 16. Phase 3 plan slicing recommendation

Strawman 7-plan slice (planner has final call):

1. **`03-01-PLAN-textmate-vscode-scaffold.md`** (Wave 1, autonomous — no dependency on Phase 2 work beyond stable Zig build) — Create `vscode-deal/package.json`, manifest, `language-configuration.json`, `deal.tmLanguage.json`, `dealx.tmLanguage.json`, file icons SVGs, `snippets/{deal,dealx}.json` (19 snippets per §14), `COLOR-CATEGORIES.md` (per §8). Acceptance: VS Code "Run Extension" dev host opens showcase with TextMate highlight on all 20 categories; snippet expansion works. Covers REQ-3-1.

2. **`03-02-PLAN-treesitter-grammar.md`** (Wave 1, autonomous) — Create `tree-sitter-deal/{grammar.js, package.json, queries/{highlights,injections,indents}.scm, test/corpus/*.txt}` (lean grammar per §9). Acceptance: `tree-sitter test` passes on all 19 showcase corpus files; `tree-sitter highlight` snapshot matches golden. Covers REQ-3-2.

3. **`03-03-PLAN-deal-lsp-scaffold-diagnostics-formatting.md`** (Wave 2, depends on Wave 1 stable; specifically depends on `deal-ffi` crate from this plan, NOT on Plan 01 or 02) — Create `deal/deal-ffi/` crate (extract from `cli/src/ffi.rs` per §10), `deal/lsp/` crate. Implement `LanguageServer` trait stub + `initialize`/`initialized`/`shutdown`. Implement `did_open`/`did_change`/`did_close` with per-document `DealHandle*` table (D-42) + 300 ms debounce (D-43). Implement `publish_diagnostics` push-mode (§11) + `textDocument/formatting` (D-21 adapter). Update `cli` to depend on `deal-ffi`. Acceptance: `cargo test -p deal-lsp` passes; `showcase.rs` `diagnostics_match_phase_2_codes` + `format_round_trip` + `debounce_collapses_rapid_changes` pass. Covers REQ-3-3 (diagnostic + formatting subset).

4. **`03-04-PLAN-deal-lsp-completion-hover-definition.md`** (Wave 3, depends on Plan 03) — Implement workspace eager-parse on `initialized` (D-47), in-memory `HashMap<PathString, (FileUri, Range)>` (D-46), path-string resolver in Rust walking `deal_index_json` (D-49). Implement `textDocument/completion` (element keywords + workspace types + per-context candidates), `textDocument/hover` (LM-2 doc comments rendered as markdown), `textDocument/definition` (HashMap lookup), `textDocument/semanticTokens/{full,full/delta}` per §7 mapping table. Acceptance: `cargo test -p deal-lsp` passes; `showcase.rs` `completion_returns_element_keywords` + `definition_lookup_specializes` + `hover_renders_jsdoc` + `semantic_tokens_match_golden` + `workspace_index_populated_after_initialize` pass. Cross-file go-to-definition works on showcase. Covers REQ-3-3 (rest).

5. **`03-05-PLAN-vscode-deal-lsp-wiring.md`** (Wave 3, depends on Plan 01 + Plan 03 — can start in parallel with Plan 04 since the LSP capabilities Plan 04 adds are TS-transparent via vscode-languageclient auto-registration) — In `vscode-deal/`, add `vscode-languageclient/node`, `LanguageClient` setup (§5), status-bar state machine (D-40 / §5), `deal.lsp.path` config override, command palette entries (`deal.restartServer`, `deal.showOutput`). Wire semantic tokens via LanguageClient (automatic when server declares capability — §5). Stub auto-download for offline `.vsix` path (skip actual download — full auto-download lands in Plan 06). Acceptance: `@vscode/test-electron` suite passes: `activates_on_deal_toml`, `language_client_connects`, `status_bar_transitions`, `format_on_save_invokes_lsp`. Covers REQ-3-4 (LSP-wiring half).

6. **`03-06-PLAN-binary-distribution-release-pipeline.md`** (Wave 4, depends on Plans 03, 04, 05) — Create `.github/workflows/release.yml` per §12 (5 jobs). Implement `vscode-deal/src/bootstrap.ts` per §4 (full auto-download with checksum + first-run dialog + progress UI). Implement SHA-256 manifest generation in CI. Implement pinned-version protocol (D-52) — extension hardcodes the matching `deal-lsp` version constant. Implement per-platform offline `.vsix` build path (D-51). Wire `vsce publish` for marketplace. Acceptance: `act` or local dry-run produces all 5 + 4 release artifacts (`deal-lsp-<triple>.tar.gz` × 4, `vscode-deal.vsix`, `vscode-deal-<triple>-offline.vsix` × 4, `SHA256SUMS`); `vscode-deal/src/test/auto_download_url_correct` + `sha256_mismatch_rejects` pass. Covers REQ-3-4 (binary distribution half).

7. **`03-07-PLAN-phase-3-gate.md`** (Wave 5, depends on all prior) — Add `phase-3-gate` and `phase-3-gate-fresh` to `deal/build.zig` per §13. Write `deal/scripts/phase-3-smoke.sh` and the `deal-lsp-smoke` harness binary. REQUIREMENTS.md closeout: resolve `DEFER-textmate-vs-treesitter-vscode` citing D-38. Retire or rewrite `/Users/dunnock/projects/deal-lang/lsp/README.md` (planner picks: delete vs. point at `deal/lsp/README.md`). Update STATE.md and ROADMAP.md to mark Phase 3 complete. Acceptance: `zig build phase-3-gate` exits 0 in clean worktree; `zig build phase-3-gate-fresh` exits 0 in ephemeral worktree; `git status` clean post-gate. Covers REQ-3-gate.

**Dependency / wave summary:**
- Wave 1: Plans 01, 02 (parallel-safe)
- Wave 2: Plan 03
- Wave 3: Plans 04, 05 (parallel-safe — different crates)
- Wave 4: Plan 06
- Wave 5: Plan 07

**Autonomy hints:**
- Plans 01, 02 are fully autonomous (no external network, no design ambiguity beyond §8/§14).
- Plans 03, 04 require careful FFI handling — flag for `--isolation=worktree` if running concurrent agent tasks.
- Plan 06 requires GitHub secrets (`VSCE_PAT`) at the marketplace-publish step — gate as manual approval.
- Plan 07 closes the phase — must run sequentially after all prior plans land.

## 17. Project skill inventory

[TBD: directories not confirmed read at research time — planner verifies during plan generation.]

Likely outcome: `No project-local skills constrain Phase 3` (the project's `.planning/` directory holds all governance; no separate `.claude/skills/` or `.agents/skills/` directories were observed in the CONTEXT.md inventory).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|---|---|---|
| A1 | `tower-lsp = "0.20"` is current | §1, §10 | Plans use stale API; planner runs `cargo search tower-lsp` to refresh |
| A2 | `tower-lsp-server` fork not canonical | §1 | Lose maintenance momentum; planner verifies maintainer activity |
| A3 | VS Code engine `^1.95.0` is the right baseline | §5 | Older Cursor/VSCodium users excluded; planner can lower to 1.85 |
| A4 | Push-mode diagnostics dominant in Neovim/Helix | §11 | Mode choice doesn't affect VS Code; impact bounded |
| A5 | 19 snippets cover SD-1..SD-20 + CS-1..CS-16 | §14 | Missing snippets discoverable by showcase frequency analysis |
| A6 | Lean tree-sitter grammar suffices for D-41 parity | §9 | Highlight gaps in Neovim/Helix; mitigation via richer grammar later |
| A7 | `bindgen` over `deal.h` produces clean wrappers without manual fixup | §10 | Manual binding maintenance; doable but tedious |
| A8 | Linguist accepts both TextMate + tree-sitter submission | §3 | GitHub web rendering delayed; not blocking Phase 3 |
| A9 | DEAL→token mapping table in §7 matches Phase 1 AST kinds | §7 | Refinement at PLAN-time against actual AST schema |
| A10 | `cargo build --release` cross-link works against Zig-cross-built static `libdeal.a` | §12 | If broken: fall back to native-Zig builds per platform (5 build jobs vs 4) |
| A11 | rust-analyzer's `bootstrap.ts` pattern is the right model | §4 | Verify by reading the actual file at planning time; mostly mechanical |
| A12 | Snippet items SD-1..SD-20 / CS-1..CS-16 exist in DESIGN-DECISIONS.md | §14 | Verify at planning; final set adjustable |

## Open Questions (RESOLVED)

1. **Which `tower-lsp` line is currently maintained?**
   - What we know: 0.20.x is the long-standing line; community forks may exist.
   - What's unclear: whether `tower-lsp-server` has supplanted upstream as of mid-2026.
   - Recommendation: planner runs `cargo search tower-lsp` + checks latest publish date before pinning.
   - **RESOLVED:** pin "0.20.0" (Plan 03 03-03-PLAN.md); cargo-search verification task retained as Wave 1 sanity check.

2. **Should `did_close` actually free the handle?**
   - What we know: D-44 says hold all open-document handles live; D-42 keys on document URI.
   - What's unclear: whether "open document" means "open in editor" or "in workspace". If the latter, `did_close` should NOT free; if the former, it should.
   - Recommendation: hold for workspace lifetime, not editor lifetime. `did_close` is a no-op; handles drop on `shutdown` or when the file leaves the workspace via FS watch.
   - **RESOLVED:** no-op did_close (hold handle live per D-44); Plan 03 03-03-PLAN.md Task 2.

3. **GitHub Linguist submission timing.**
   - What we know: requires PR with sample files + grammar registration.
   - What's unclear: whether to submit at Phase 3 closeout or wait until Phase 4 docs polish.
   - Recommendation: defer Linguist PR to Phase 4 since it requires public showcase samples that may evolve.
   - **RESOLVED:** Phase 4 closeout (deferred per Plan 07 INFO; submission timing tracked outside Phase 3).

4. **`linux-x64-musl` target priority.**
   - What we know: D-50 lists `linux-x64-musl` as a target.
   - What's unclear: is the auto-download triple deduplicated with `linux-x64-gnu`, or are these separate downloads?
   - Recommendation: ship both as separate downloads (musl users use Alpine; static binary differs).
   - **RESOLVED:** ship both linux-x64-gnu and linux-x64-musl in Plan 06 (per D-50 target list); auto-download triple detection in bootstrap.ts inspects libc per platform marker.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|---|---|---|---|---|
| Zig 0.16+ | libdeal cross-build (D-53) | (verify) | (verify) | none — already required by Phase 2 |
| Rust stable | deal-lsp build | (verify) | (verify) | none — already required by Phase 2 |
| Node.js 20+ | vscode-deal + tree-sitter-deal | (verify) | (verify) | bump to 20 from current baseline |
| npm | vscode-deal + tree-sitter-deal | (verify) | (verify) | use pnpm/yarn if needed |
| `tree-sitter-cli` | tree-sitter-deal generate/test | (verify) | (verify) | install via `npm install -g tree-sitter-cli` in CI |
| `@vscode/test-electron` | extension tests | (verify on `npm install`) | ^2.4 | none |
| `vsce` | vscode-deal package + publish | (verify on `npm install`) | latest | none — `vsce publish` is the only marketplace path |
| `xvfb` (Linux CI only) | headless extension tests | (CI-provided) | apt | none — required for `@vscode/test-electron` on headless Linux |

[TBD: planner runs actual `command -v` audit during Plan 03/05 execution.]

## Package Legitimacy Audit

> Required since this phase installs external packages across three ecosystems (Rust/cargo, npm, GitHub Actions). slopcheck was NOT run at research time; all packages below are `[ASSUMED]` and the planner MUST gate each install behind a `checkpoint:human-verify` task or run `slopcheck install` explicitly.

| Package | Registry | Age (approx) | Provenance | Disposition |
|---|---|---|---|---|
| `tower-lsp` | crates.io | 5+ yrs | Official tower ecosystem; widely-used in rust-analyzer alternatives | `[ASSUMED]` — verify via `cargo search` |
| `tokio` | crates.io | 8+ yrs | Industry standard async runtime | `[ASSUMED]` — verify |
| `tower` | crates.io | 6+ yrs | Industry standard middleware framework | `[ASSUMED]` — verify |
| `dashmap` | crates.io | 5+ yrs | Widely-used concurrent map | `[ASSUMED]` — verify |
| `ropey` | crates.io | 6+ yrs | Widely-used in helix-editor; standard text rope | `[ASSUMED]` — verify |
| `bindgen` | crates.io | 8+ yrs | rust-lang official; standard C ABI binding generator | `[ASSUMED]` — verify |
| `tracing` | crates.io | 6+ yrs | tokio-rs ecosystem; standard structured logging | `[ASSUMED]` — verify |
| `tracing-subscriber` | crates.io | 6+ yrs | tokio-rs ecosystem | `[ASSUMED]` — verify |
| `async-trait` | crates.io | 5+ yrs | dtolnay ecosystem; standard `#[async_trait]` macro | `[ASSUMED]` — verify |
| `vscode-languageclient` | npm | 10+ yrs | Microsoft official; canonical LSP client | `[ASSUMED]` — verify via `npm view` |
| `@vscode/test-electron` | npm | 5+ yrs | Microsoft official; canonical extension test runner | `[ASSUMED]` — verify |
| `@vscode/test-cli` | npm | 2+ yrs | Microsoft official | `[ASSUMED]` — verify |
| `vsce` (`@vscode/vsce`) | npm | 10+ yrs | Microsoft official; canonical publisher | `[ASSUMED]` — verify |
| `node-fetch` | npm | 10+ yrs | Widely-used (50M+ wk dl); standard fetch polyfill | `[ASSUMED]` — verify |
| `tree-sitter-cli` | npm | 8+ yrs | tree-sitter org official | `[ASSUMED]` — verify |
| `mocha` | npm | 12+ yrs | Standard JS test runner | `[ASSUMED]` — verify |
| `sinon` | npm | 12+ yrs | Standard JS mocking | `[ASSUMED]` — verify |
| `mlugg/setup-zig` | GitHub Marketplace | 3+ yrs | Widely-used Zig setup action | `[ASSUMED]` — verify |
| `dtolnay/rust-toolchain` | GitHub Marketplace | 4+ yrs | dtolnay maintained | `[ASSUMED]` — verify |
| `softprops/action-gh-release` | GitHub Marketplace | 5+ yrs | Standard release action | `[ASSUMED]` — verify |
| `actions/checkout`, `actions/setup-node`, `actions/upload-artifact`, `actions/download-artifact` | GitHub | 5+ yrs | GitHub official | Verified-by-provenance — official `actions/` org |

**Packages removed due to slopcheck [SLOP] verdict:** none (slopcheck not run).
**Packages flagged as suspicious [SUS]:** none (slopcheck not run).

**Planner action required:** Run `slopcheck install <pkg1> <pkg2> …` at the top of Plan 03 (Rust crates), Plan 01/02/05 (npm), and Plan 06 (GitHub Actions). Add a `checkpoint:human-verify` step before `cargo build` / `npm install` if slopcheck remains unavailable.

## Sources

### Primary (HIGH confidence)
- `/Users/dunnock/projects/deal-lang/deal/.planning/phases/03-editor-intelligence/03-CONTEXT.md` — all 16 locked decisions
- `/Users/dunnock/projects/deal-lang/deal/include/deal.h` — 9 C ABI exports
- `/Users/dunnock/projects/deal-lang/deal/Cargo.toml` — current workspace
- LSP specification 3.17 § Semantic Tokens — https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#semanticTokens
- LSP specification 3.17 § Publish/Pull Diagnostics — https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_diagnostic
- VS Code Language Server Extension Guide — https://code.visualstudio.com/api/language-extensions/language-server-extension-guide
- VS Code Syntax Highlight Guide — https://code.visualstudio.com/api/language-extensions/syntax-highlight-guide
- TextMate Language Grammars — https://macromates.com/manual/en/language_grammars
- Tree-sitter Creating Parsers — https://tree-sitter.github.io/tree-sitter/creating-parsers
- rust-analyzer `editors/code/src/bootstrap.ts` — https://github.com/rust-lang/rust-analyzer/blob/master/editors/code/src/bootstrap.ts

### Secondary (MEDIUM confidence — verified via training data only at research time)
- tower-lsp 0.20 API surface (verify via `cargo search tower-lsp` at planning)
- nvim-treesitter highlight capture conventions (`@keyword.*`, `@type.*`, etc.)
- @vscode/test-electron 2.4 API
- vscode-languageclient/node ServerOptions / ClientOptions shapes

### Tertiary (LOW confidence — needs validation at planning)
- Exact `tower-lsp` pull-mode diagnostic support (verify against changelog)
- VS Code 1.95 vs 1.85 baseline pick (verify against Cursor / VSCodium engine support)
- `linux-x64-gnu` vs `linux-x64-musl` separation in auto-download

## Metadata

**Confidence breakdown:**
- Standard stack (tower-lsp, tokio, tree-sitter-cli, vscode-languageclient): MEDIUM — version pins assumed from training, must be verified by planner via registry calls.
- Architecture patterns (bootstrap.ts, LanguageServer trait shape, semantic-tokens encoding): HIGH — patterns are stable across LSP/VS Code ecosystem.
- DEAL-specific decisions (§7 token map, §8 color categories, §14 snippets): MEDIUM — assumed from CONTEXT.md `<specifics>` + showcase frequency intuition; refinable at PLAN-time.
- Pitfalls (FFI ownership across deal_free, debounce semantics, push vs pull): HIGH — well-documented patterns.

**Research date:** 2026-05-22
**Valid until:** 2026-06-22 (30 days for stable LSP/VS Code APIs; sooner if tower-lsp ships a major version)

## RESEARCH COMPLETE

The phase has 16 locked decisions (D-38..D-53) from CONTEXT.md and minimal Claude's-Discretion latitude. This RESEARCH.md fills the discretion areas:

1. **Tower-lsp pin** — `0.20.x` (planner to verify at planning).
2. **Diagnostic delivery** — push-mode.
3. **Tree-sitter grammar scope** — lean highlight-focused (~600–900 LOC `grammar.js`).
4. **DEAL→semantic-token mapping** — 19-row table drafted (§7).
5. **Color category names** — 20-row table (§8).
6. **Snippet set** — 19 snippets (§14).
7. **Cargo workspace** — add `deal-ffi` + `lsp` members (§10).
8. **CI pipeline** — 5 jobs across 4 native runners + 1 Linux cross-builder (§12).
9. **Phase gate** — 8 steps with `phase-3-gate-fresh` reusing existing `verify-fresh-worktree.sh` (§13).
10. **Plan slicing** — 7-plan strawman across 5 waves (§16).

**Marked [TBD]:** §17 project skill inventory (directories not confirmed read), `linux-x64-musl` vs `gnu` deduplication decision, exact tower-lsp pull-mode behavior, exact AST kind names from Phase 1 (token mapping refinement at PLAN-time).

**Marked [ASSUMED]:** 12 items in the Assumptions Log — most version pins, all package legitimacy, and snippet set against DESIGN-DECISIONS.md SD/CS items. Planner must verify pins and run `slopcheck install` before committing crate/npm installs.
