# Requirements: DEAL — Digital Engineering Authoring Language

**Defined:** 2026-05-19
**Core Value:** A defense/aerospace systems engineer can author a real subsystem in DEAL end-to-end, run it through codegen, and import the result into Cameo / DOORS / Modelica without manual fixup.

> Requirement IDs preserve the structure from `.planning/intel/requirements.md`. Each phase-level requirement (`REQ-phase-N-<topic>`) carries the PRD exit criterion as its acceptance condition. Each milestone-level requirement (`REQ-phase-N-M-<topic>`) carries the per-milestone deliverable as its acceptance condition. Source: `/Users/dunnock/projects/deal-lang/DEAL-LANG-ROADMAP.html` (PRD, precedence 2). For the underlying design decisions these requirements implement, see the `<decisions>` block in `PROJECT.md`.

## v1 Requirements

### Phase 0 — Foundation (COMPLETE)

- [x] **REQ-phase-0-foundation** — Spec-grade design foundation: 65 locked design decisions (64 LOCKED + 1 CAPTURED), complete lexical/definition/composition grammars, 19-file showcase model, verification report, parser implementation guide, acquired reference material (SysML v2/KerML JSON schemas, OMG spec PDFs, textual BNFs). *Acceptance: All listed artifacts present and verified (LOCKED + COMPLETE + ACQUIRED status table in PRD §1).*

### Phase 1 — Zig Compiler Core

- [x] **REQ-phase-1-foundation** — Build the Zig compiler core: lexer, parser for `.deal` and `.dealx`, error recovery + diagnostics, C ABI boundary. *Exit: `deal parse showcase/*.deal showcase/*.dealx` produces AST JSON for all 19 files, with zero panics and meaningful diagnostics on malformed input.* — **Complete 2026-05-20** (all six sub-requirements [x]; verified by `gate_all_19`)
- [x] **REQ-phase-1-1-lexer** — Tokenizer for both file modes implementing the full `lexical.ebnf` token set. Lexer selects mode based on file extension; `[< >]` composition tags activate only in `.dealx` mode; `<<operator>>` delimiters need custom handling. *Acceptance: `libdeal` lexer module produced; token snapshot tests for all 19 showcase files pass with zero UNKNOWN tokens; error tokens carry span + message.* (Plan 01-02 — error-token spans will land in Plan 05 via D-16 lexer range)
- [x] **REQ-phase-1-2-parser-deal** — Recursive-descent parser implementing the 87 `deal.ebnf` productions, with Pratt parsing for expressions. Phased build: file structure → element definitions → element bodies → annotations → expressions. AST uses arena allocation. *Acceptance: All 15 `.deal` showcase files parse; AST JSON snapshots stable; Pratt expression parser working.*
- [x] **REQ-phase-1-3-parser-dealx** — Composition parser for the 43 `dealx.ebnf` productions, with stack-based tag balancing for `[<system>]...[</system>]`. Self-closing tags are the common case. `via={...}` and `carrying={...}` inline blocks on `[<connect>]` are the hardest parse points. *Acceptance: All 4 `.dealx` showcase files parse; tag balance enforced; inline block parsing for connect/expose works.* (Plan 01-04)
- [x] **REQ-phase-1-4-error-recovery** — Survive malformed input gracefully. Synchronize to next statement/definition boundary. Structured diagnostic types with code, message, span, optional fix suggestion. *Acceptance: 50+ error recovery test cases; structured diagnostic types; span-accurate error reporting.* (Plan 01-05 — 50-file malformed corpus, D-17 three-tier sync, diag.json_roundtrip)
- [x] **REQ-phase-1-5-c-abi** — Expose the compiler as a static library `libdeal.a` with C ABI functions `deal_parse()`, `deal_free()`, `deal_diagnostics()`. Minimal Rust FFI test harness validates the integration architecture. *Acceptance: `libdeal.a`; `deal.h` C header; Rust FFI test harness parses a showcase file and reads back the AST.* (Plan 01-06 — 6 exports, UTF-8 + bounds hardening, 9 Rust FFI tests)
- [x] **REQ-phase-1-gate** — Phase 1 exit gate. *Acceptance: All 19 showcase files tokenize and parse through the Zig core; AST JSON snapshots stable; error recovery handles at least 50 malformed-input cases without panic; C ABI boundary proven with Rust harness. No external-facing release.* (Plan 01-06 — gate_all_19 passing; zig build phase-1-gate exits 0)

### Phase 1.5 — Parser Strictness & Test-Gap Resolution (INSERTED)

> Closes test-quality gaps surfaced by `/gsd:validate-phase 1` audit (2026-05-20). Phase 1's automated gates are green; this phase strengthens the contracts those gates assert so Phase 2's semantic analyzer builds on a parser whose behavior is *verified*, not merely *intended*. Source: `.planning/phases/01-zig-compiler-core/01-LEARNINGS.md` Surprise #8.

- [x] **REQ-phase-1.5-strictness-and-tests** — Close the test-quality and parser-strictness gaps from the Phase 1 validation audit so the parser contract is asserted, not implicit. *Exit: zero `>= 0` vacuous assertions remain, all 20 hand-curated malformed files are pinned to expected diagnostic codes, determinism is automated, recovery tests assert recovered AST structure, m08/m18/m19 strictness gaps are closed or formally deferred via ADR.* — **Complete 2026-05-21** (all six sub-requirements [x]; Phase 1.5 exit gate green)
- [x] **REQ-phase-1.5-1-test-assertions** — Replace every vacuous assertion (`diags.items.len >= 0` and equivalents) with a named-property check. Strengthen `recovery_statement`, `recovery_definition`, and `recovery_dealx_tag` so each case asserts (a) the exact diagnostic code emitted, (b) the recovered AST contains the expected definition at the expected position (e.g. `df.definitions[1].payload.part_def.name == "Good"`). *Acceptance: `grep -nE 'len >= 0' tests/unit/*.zig` returns 0 matches; each recovery test case has at least one structural-shape assertion; all tests still pass.* — **Complete 2026-05-20** (Plan 01.5-01 — 9 recovery cases strengthened to two-axis BDD via commit ab8f807; grep gate clean)
- [x] **REQ-phase-1.5-2-malformed-pinning** — Pin per-file expected primary diagnostic code for each of the 20 hand-curated `m01..m20` malformed files. Extend `recovery.corpus` to enforce the pin (first diagnostic emitted for `m08_unterminated_block_comment.deal` MUST be the documented code). 30 generator-produced files continue to use the soft 60 % gate. *Acceptance: `tests/malformed/_pins.zig` (or equivalent) declares the 20 pins; `recovery.corpus` fails if any hand-curated file emits a different primary code than its pin.* — COMPLETE 2026-05-20 (Plan 01.5-03): `m_pins` table with 20 entries added to `tests/unit/recovery_corpus.zig` (19 impl pins against Codes.e_*; m01 as accepted-no-diag with ADR-phase-1.5-m01-sd10-optional-semicolon.md). Enforcement loop asserts first emitted diagnostic matches pin; soft 60% gate preserved unchanged for the 30 generator-produced files.
- [x] **REQ-phase-1.5-3-determinism** — Add a `determinism.parse_twice` test that, for each of the 19 showcase files, calls `deal_parse` twice in the same process and asserts byte-identical AST JSON. Locks the D-18 alphabetical-key invariant against future regressions (e.g. accidental HashMap iteration order or non-deterministic emission). *Acceptance: New test in `tests/unit/determinism_parse_twice.zig`; runs on all 19 showcase files; passes; fails if any byte differs across the two parses.* — **Complete 2026-05-21** (Plan 01.5-04 Task 1 — commit 5effb61; uses PUBLIC C ABI deal_parse / deal_ast_json / deal_free per CONTEXT.md lock; gpa.dupes parse-A JSON before deal_free invalidates the arena pointer; also asserts byte-identical diagnostics JSON as a no-cost addon).
- [x] **REQ-phase-1.5-4-parser-strictness** — Investigate m08 (unterminated block comment), m18 (dangling dot operator), m19 (empty arg with comma). For each: either close the gap with a defined diagnostic emission (re-use an existing code where one already fits — `E0003 e_unterminated_comment` already exists in `diagnostics.zig` and is the natural choice for m08; allocate fresh codes in D-16's lexer range `E0001..E0099` or parser-deal range `E0100..E0299` where no fit exists), or document as accepted (the input IS grammatically valid) via an ADR. *Acceptance: All three files now produce a defined diagnostic OR an ADR exists at `.planning/decisions/ADR-phase-1.5-<topic>.md` justifying acceptance.* — COMPLETE 2026-05-20 (Plan 01.5-02): m08 wires EXISTING E0003 e_unterminated_comment; m18 emits new E0121 e_dangling_dot; m19 emits new E0122 e_empty_arg_comma. All three pinned in tests/unit/strictness_m08_m18_m19.zig.
- [x] **REQ-phase-1.5-5-property-tests** — Add a property-style test asserting every AST node's span is contained within its parent's span (`child.span.start >= parent.span.start && child.span.end <= parent.span.end`). Walks the AST of every showcase file (19 total). Complements the snapshot tests by catching span-corruption regressions that byte-equality would still pass through. *Acceptance: New test in `tests/unit/property_span_containment.zig`; runs on all 19 showcase files; passes for the current locked snapshots.* — **Complete 2026-05-20** (Plan 01.5-01 Task 2 — commit 2ed9f07; walker is exhaustive over `union(NodeKind)` so a missing arm is a compile error; runs against 19 showcase + 8 synthetic fixtures with no snapshot regeneration).
- [x] **REQ-phase-1.5-gate** — Phase 1.5 exit gate. *Acceptance: `zig build phase-1-gate` exits 0; new tests `determinism.parse_twice` and `property.span_containment` pass; per-file pins in `recovery.corpus` enforced; no `>= 0` vacuous assertions remain; 01-LEARNINGS.md Surprise #8 backlog items are all addressed or formally deferred via ADR.* — **Complete 2026-05-21** (Plan 01.5-04 Task 2 — commit 92066c9; phase-1.5-gate build step bundles phase-1-gate + full test suite with NOTE comment documenting the CI-authoritative three-step command; ## Phase 1.5 Resolution section appended to 01-LEARNINGS.md closing all 9 Surprise #8 sub-items as Resolved + 1 Deferred via ADR-phase-1.5-m01-sd10-optional-semicolon.md). PHASE 1.5 EXIT GATE: GREEN.

### Phase 2 — Prove the Pipeline

- [x] **REQ-phase-2-prove-pipeline** — Semantic analysis + first codegen backend. *Exit: `deal build --target sysml-v2 showcase/` produces valid SysML v2 JSON importable into a SysML v2 viewer; `deal fmt` round-trips all showcase files; `deal check` catches type errors and unresolved imports.*
- [x] **REQ-phase-2-1-semantic-analyzer** — Name resolution, type checking, import validation. Resolves package-qualified imports, element references, port type annotations. Enforces multiplicity. Validates `<<specializes>>` targets exist and are type-compatible. Checks `@trace` references point to real requirements. Populates `.deal/index.json`. *Acceptance: Name resolver; type checker; model index generation; import validation.*
- [x] **REQ-phase-2-2a-ir-v0** — Define the DEAL IR v0 contract: node kinds, stable element IDs, source spans, package/module identity, resolved-reference representation, relationship graph shape, metadata envelope, diagnostic attachment points, traversal/query API. Scoped to the 19-file showcase + 5–8 golden SysML fixtures. *Acceptance: IR v0 schema/model document; stable ID + span strategy; relationship graph contract; backend traversal API contract.*
- [x] **REQ-phase-2-2b-ir-lowering** — AST → DEAL IR lowering pass in Zig. IR strips syntactic sugar, resolves references, represents the system model as a graph of typed elements. Carries everything Phase 2 backend needs: structural relationships, source spans, agent metadata (`@confidence`, `@rationale`), simulation bindings, traceability links. Realizes FA-1. *Acceptance: AST → IR lowering pass; IR v0 conformance tests; IR query/traversal API.*
- [x] **REQ-phase-2-3-sysml-v2-codegen** — SysML v2 JSON codegen from IR. Map DEAL IR elements to SysML v2 JSON schema types (`part def` → `PartDefinition`, `port` → `PortUsage`, `<<specializes>>` → `Specialization` in `ownedRelationship`, etc.). References: `spec/references/omg-sysml-v2/SysML.json`, `spec/references/omg-kerml-v1/KerML.json`. Schemas use absolute OMG `$id`/`$ref` URLs requiring local schema registry. *Acceptance: SysML v2 JSON emitter; schema validation of output; showcase model exported + schema-valid; showcase viewer/import smoke result recorded.*
- [x] **REQ-phase-2-3a-offline-validator** — Local SysML/KerML schema bundle + validation harness. Standard JSON Schema `$ref` resolver backed by the downloaded schema bundle. Optional `--validate` flag on `deal build`; CI integration. *Acceptance: Local schema registry/resolver; `deal build --validate` flag; CI validation step.*
- [x] **REQ-phase-2-3b-golden-fixtures** — Hand-written expected SysML v2 JSON corpus. 5–8 minimal `.deal` source files paired with hand-written expected SysML v2 JSON output and schema validation results. Covers part definitions, port usages, specialization, attribute usages, requirement definitions, traceability links, and one multi-file `.dealx` composition. *Acceptance: 5–8 minimal DEAL → JSON fixture pairs; schema validation pass for each; viewer/import smoke test for at least one fixture; recorded viewer/import smoke result for showcase export.*
- [x] **REQ-phase-2-4-formatter** — `deal fmt` — canonical formatter. AST → source pretty-printer enforcing canonical style. Round-trip test: parse → format → parse, compare ASTs. *Acceptance: Pretty-printer module; round-trip tests pass for all 19 files; C ABI `deal_format()` exposed.*
- [x] **REQ-phase-2-5-cli-shell** — Rust CLI shell wrapping the Zig core via the C ABI. Commands: `deal parse`, `deal check`, `deal fmt`, `deal build --target sysml-v2`. Built with `clap`. Error output `rustc`/`cargo`-quality. *Acceptance: `deal` CLI binary; parse/check/fmt/build commands; colored diagnostic output.*
- [x] **REQ-phase-2-gate** — Phase 2 exit gate. *Acceptance: `deal check` validates the showcase model; `deal build --target sysml-v2` emits JSON passing offline schema validation; `deal fmt` round-trips all 19 files; DEAL IR v0 documented + implemented; 5–8 golden output fixtures match hand-written expected JSON and validate; showcase export has recorded SysML v2 viewer/import smoke result (or documented blocker); systems engineer can author → validate → export to SysML v2 JSON. Internal demo-ready.*

### Phase 3 — Editor Intelligence

- [x] **REQ-phase-3-editor-intelligence** — VS Code extension + LSP + tree-sitter. *Exit: VS Code user opens the showcase project and gets syntax highlighting, real-time diagnostics, autocomplete (element names + imports), cross-file go-to-definition, format-on-save.* — **Complete 2026-05-27** (umbrella satisfied via Plans 03-01..03-07; all 4 ROADMAP success criteria verified by phase-3-gate + phase-3-gate-fresh both exiting 0).
- [x] **REQ-phase-3-1-textmate** — Syntax highlighting for `.deal` and `.dealx`. Write `deal.tmLanguage.json` and `dealx.tmLanguage.json`. Special scopes for `<<operator>>`, `@category:<<operator>>`, `[<tag>]`, `@header { ... }`. *Acceptance: vscode-deal extension (TextMate only); bracket matching + auto-close for `[< >]`; snippet library; file icons for `.deal` / `.dealx`.*
- [x] **REQ-phase-3-2-treesitter** — Tree-sitter grammar for Neovim, Helix, Zed, GitHub. Derived from EBNF spec, adapted to tree-sitter PEG-like model. Includes `highlights.scm`, injection queries, indent queries. *Acceptance: tree-sitter-deal package; highlight + indent queries; test corpus from showcase.*
- [x] **REQ-phase-3-3-lsp-server** — Rust LSP server using `tower-lsp`, calling the Zig core via C ABI. Capabilities: `textDocument/diagnostic`, `textDocument/completion`, `textDocument/definition`, `textDocument/hover`, `textDocument/formatting`. Incremental re-parse on file change. Uses model index for workspace-wide symbol resolution. *Acceptance: `deal-lsp` binary; diagnostics, completion, definition, hover, formatting working; incremental re-parse; workspace symbol index.*
- [x] **REQ-phase-3-4-vscode-lsp** — Wire the vscode-deal extension to the LSP server. Extension detects `deal.toml` to activate. Replace TextMate-only highlighting with tree-sitter-based highlighting where supported (semantic tokens provider). *Acceptance: vscode-deal extension with LSP; bundled `deal-lsp` binary; semantic tokens provider.*
- [x] **REQ-phase-3-gate** — Phase 3 exit gate. *Acceptance: Opening the showcase project in VS Code delivers syntax highlighting, real-time error checking, autocomplete, cross-file go-to-definition, format-on-save — experience comparable to TypeScript in VS Code.* — **Complete 2026-05-27** (Plan 03-07 Task 1 added phase-3-gate (8 steps: phase-2-gate regression + cargo build/test --workspace --release + tree-sitter test + vscode-deal mocha + .vsix sanity + phase-3-smoke.sh) and phase-3-gate-fresh (ephemeral worktree per ADR-phase-1.5-fresh-worktree-verification with sibling-repo symlink materialization); both exit 0 on the dev's main checkout. phase-3-smoke.sh exercises all 5 LSP capabilities + semantic tokens against the canonical battery.deal showcase via lsp-smoke harness.).

### Phase 4 — Ecosystem

- [x] **REQ-phase-4-ecosystem** — Standard library + second backend (ReqIF) + documentation site. *Exit: A real project can be authored using `deal-stdlib` units and interfaces, exported to both SysML v2 JSON and ReqIF XML, and its documentation is readable on `deal-lang.org`.*
- [x] **REQ-phase-4-1-stdlib** — Author the first standard library packages in DEAL itself. SI base + derived units from BIPM. Electrical interfaces (RJ45, USB-C, CAN, RS-422). Mechanical interfaces (common bolt patterns). *Acceptance: `deal-stdlib/units` (SI + imperial); `deal-stdlib/interfaces/electrical`; `deal-stdlib/interfaces/mechanical`; package resolution for `deal.toml` dependencies.*
- [x] **REQ-phase-4-2-reqif-codegen** — Second codegen backend: IR → ReqIF XML for DOORS / Jama. Map DEAL requirements (`requirement def`, `need def`), traceability links (`@trace:<<satisfies>>`), verification blocks to ReqIF XML elements. Prerequisite: Acquire ReqIF normative references (ReqIF 1.2 XSD from OMG, DOORS-compatible sample export, optional Jama sample). Place in `spec/references/omg-reqif/`. Without the XSD, the emitter has no validation target. *Acceptance: ReqIF XML emitter; showcase requirements exported; DOORS import validation.*
- [x] **REQ-phase-4-3-docs-site** — `deal-lang.org` on Astro + Starlight. Landing, getting-started, language reference (definitions, compositions, requirements, traceability, imports, annotations), CLI reference, VS Code setup. Code examples use custom Shiki grammar built from TextMate grammar. **Critical:** every code block must render with actual syntax highlighting — no placeholders, no broken includes, no unstyled fences. CI step detects unresolved snippet references and missing Shiki grammar scopes; build fails if a `.deal`/`.dealx` block renders without highlighting. *Acceptance: deal-lang.org site; language reference; getting-started tutorial; Shiki grammar for code highlighting; CI snippet render validation.*
- [x] **REQ-phase-4-4-package-resolution** — `deal.toml` dependency resolution. Local-path and git-based resolution (no registry yet). Standard library consumed as a dependency. Lockfile generation for reproducible builds. *Acceptance: `deal.lock` generation; local + git dependency resolution; `deal install` command.*
- [x] **REQ-phase-4-gate** — Phase 4 exit gate — minimum viable language. *Acceptance: New project can scaffold with `deal init`, depend on `deal-std` for units/interfaces, author definitions and compositions, validate with `deal check`, export to SysML v2 JSON or ReqIF XML, read documentation on `deal-lang.org`. ReqIF output validates against OMG XSD. Documentation site CI passes. First external release candidate.*

### Phase 5 — Simulation Integration

- [x] **REQ-phase-5-simulation** — `deal-sim` SDK + simulation pipeline. *Exit: `deal simulate --all` runs the showcase model's Python and MATLAB simulations, captures results as evidence, and `deal check --verify` evaluates verification criteria against simulation output.*
- [x] **REQ-phase-5-1-deal-sim-sdk** — `deal-sim` Python SDK. `DealSimulation` base class declares typed inputs/outputs. CLI runner reads `input.json`, invokes simulation, validates output against declared schema, writes `output.json` with metadata. *Acceptance: `deal-sim` PyPI package; `DealSimulation` base class; input/output validation; CLI runner.*
- [x] **REQ-phase-5-2-orchestration** — `deal.sims.toml` parser + orchestration engine. Resolves dependency graph between simulations, determines staleness, executes in dependency order. `deal simulate --stale` only re-runs simulations whose inputs have changed. MATLAB simulations via MATLAB Engine API for Python. *Acceptance: Simulation registry parser; dependency graph resolution; staleness detection; `deal simulate` command.*
- [x] **REQ-phase-5-3-evidence-verification** — `deal evidence capture` snapshots simulation results + test data into evidence cache. `deal check --verify` evaluates verification criteria. `deal evidence baseline v2.1.0` tags a verified state. Traceability model shows full coverage status. *Acceptance: `deal evidence` command; `deal check --verify`; verification status reporting; baseline tagging.*
- [x] **REQ-phase-5-gate** — Phase 5 exit gate. *Acceptance: Showcase model simulations run, produce evidence, verification criteria evaluate against that evidence. A program manager can ask "is REQ_BAT_001 verified?" and get a traceable answer from the model.*

### Phase 6 — Application

- [ ] **REQ-phase-6-application** — Desktop editor + import pipelines + advanced backends. *Exit: Desktop application where systems engineers author DEAL models with text-first editing, view auto-generated diagrams, and see traceability status — replacing Cameo/Sparx for the authoring workflow.*
- [ ] **REQ-phase-6-1-desktop-editor** — Desktop editor (Tauri + Next.js). Text-first MBSE application. CodeMirror 6 with DEAL language mode (tree-sitter highlighting + LSP intelligence). Model browser tree. Read-only diagram views generated from the IR (block definition, internal block, sequence diagrams). Traceability matrix view. Verification dashboard. Tauri backend manages file watching, model state, IPC with Zig core via Rust FFI. *Acceptance: Desktop application binary; text editor with LSP; model browser; auto-generated diagrams; traceability matrix.*
- [ ] **REQ-phase-6-2-import-pipelines** — Reverse codegen direction. `deal import --from sysml-v2 model.json` converts SysML v2 JSON into `.deal`/`.dealx` files. `deal import --from reqif requirements.xml` converts ReqIF into requirement definitions. Output must be idiomatic DEAL, not a syntactic transliteration. *Acceptance: SysML v2 → DEAL importer; ReqIF → DEAL importer; `deal import` command.*
- [ ] **REQ-phase-6-3-doc-generation** — `deal build --target docs` generates documentation from the model: system description documents, interface control documents, verification status reports. Template-driven (`deal.toml` specifies template name, e.g., `"program-review"`). *Acceptance: Document generation backend; HTML + PDF output; configurable templates.*
- [ ] **REQ-phase-6-4-stdlib-expansion** — Expand `deal-stdlib` with protocols (MIL-STD-1553, ARINC 429, SpaceWire), standards (DO-178C, DO-254 assurance levels; MIL-STD-810H environmental test method references), patterns (TMR, dual-redundant, watchdog). *Acceptance: `deal-stdlib/protocols`; `deal-stdlib/standards`; `deal-stdlib/patterns`.*
- [ ] **REQ-phase-6-gate** — Phase 6 exit gate — full vision delivered. *Acceptance: Engineer can open the desktop editor, author a system model in DEAL using stdlib components, see auto-generated diagrams, run simulations, check verification status, export to SysML v2 JSON / ReqIF XML / generated docs. Import from existing SysML v2 and ReqIF sources works.*

## v2 Requirements (Deferred Decision Points per PRD §11)

These are PRD-documented decision points to be evaluated at named gates. They are not in the current roadmap; they are evaluated at the listed gate and may become v1 scope at that time.

### Distribution

- **DEFER-wasm-packaging** *(evaluated at Phase 2 gate)* — Whether to ship npm-installable WASM compilation of the Zig core for in-browser playground. Evaluate after the C ABI boundary proves stable.
- **DEFER-first-public-release** *(evaluated at Phase 4 gate)* — Phase 4 vs Phase 5 timing for the initial public release.

### Ecosystem

- **DEFER-package-registry** *(evaluated at Phase 4 gate)* — Whether a centralized package registry is needed, or whether git-based dependencies suffice.

### Tooling

- **DEFER-textmate-vs-treesitter-vscode** — RESOLVED 2026-05-27 by D-38 (Phase 3 ships TextMate grammar + LSP semantic tokens; VS Code's experimental tree-sitter support rejected because it would create a dual parser source of truth — tree-sitter-deal already exists as a separately-versioned package for Neovim/Helix/Zed/GitHub and shares no scope/token namespace with the TextMate grammar that vscode-deal uses for offline highlighting + the LSP semantic tokens that augment it for in-language semantic categories per D-40/D-41).
- **DEFER-desktop-editor-priority** *(evaluated at Phase 5 gate)* — Whether the pilot audience needs the Tauri desktop editor, or whether CLI + VS Code is sufficient.

### Language Surface (Deferred per ADR)

These are syntax/language design decisions explicitly deferred in `DESIGN-DECISIONS.md`. They are NOT scheduled to a phase; they will be locked when needed (driven by showcase model coverage):

- **DEFER-expression-syntax** — Arithmetic, comparison, logical expression syntax details (only what showcase requires is implemented in Phase 2).
- **DEFER-path-expression-syntax** — Full path expression syntax.
- **DEFER-relationship-categories** — Full enumeration (SD-6 is CAPTURED, not LOCKED).
- **DEFER-mql** — Model Query Language design.
- **DEFER-codegen-backend-api** — Public codegen backend API for third-party backends.
- **DEFER-default-visibility** — Default visibility when no `public|protected|private` wrapper (SD-9).
- **DEFER-constraint-expression-syntax** — Constraint expression syntax.
- **DEFER-document-category** — `@document:` category design.
- **DEFER-document-block** — `[<document>]` composition block for report generation.

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Garbage collection in the compiler | Native build mandate (Zig core + Rust frontend), per tech-stack constraint. |
| Mobile applications | Desktop-first per FA-3 dual audience; mobile is post-v1. |
| Real-time multi-user collaborative editing | Text-first + git-based collaboration is the model (FA-3, LM-1 TypeScript feel). |
| Cloud IDE / SaaS hosting | DEAL is a local-first toolchain — no remote compilation requirement. |
| Custom rendering engine for diagrams (Phase 6) | Auto-generated, read-only diagram views from the IR; visual editing is out of scope. |
| Proprietary licensing / closed-source distribution | Anti-pattern for the dual-audience (FA-3) ecosystem goals; open licensing assumed. |
| Bidirectional live sync with Cameo/DOORS/Modelica | `deal import` and `deal build` are batch operations; live round-trip is not promised. |

## Traceability

Each v1 requirement maps to exactly one phase. Phases 0–5 (incl. inserted Phase 1.5) are COMPLETE; Phase 6 (Application) is the remaining build phase. *(Status last reconciled 2026-06-09 by `/gsd:audit-milestone` — see `v2.1.0-MILESTONE-AUDIT.md`.)*

| Requirement | Phase | Status |
|-------------|-------|--------|
| REQ-phase-0-foundation | Phase 0 | Complete |
| REQ-phase-1-foundation | Phase 1 | Complete |
| REQ-phase-1-1-lexer | Phase 1 | Complete (Plan 01-02) |
| REQ-phase-1-2-parser-deal | Phase 1 | Complete |
| REQ-phase-1-3-parser-dealx | Phase 1 | Complete (Plan 01-04) |
| REQ-phase-1-4-error-recovery | Phase 1 | Complete (Plan 01-05) |
| REQ-phase-1-5-c-abi | Phase 1 | Complete (Plan 01-06) |
| REQ-phase-1-gate | Phase 1 | Complete (Plan 01-06) |
| REQ-phase-1.5-strictness-and-tests | Phase 1.5 | Complete |
| REQ-phase-1.5-1-test-assertions | Phase 1.5 | Complete (Plan 01.5-01) |
| REQ-phase-1.5-2-malformed-pinning | Phase 1.5 | Complete (Plan 01.5-03) |
| REQ-phase-1.5-3-determinism | Phase 1.5 | Complete (Plan 01.5-04) |
| REQ-phase-1.5-4-parser-strictness | Phase 1.5 | Complete (Plan 01.5-02) |
| REQ-phase-1.5-5-property-tests | Phase 1.5 | Complete (Plan 01.5-01) |
| REQ-phase-1.5-gate | Phase 1.5 | Complete (Plan 01.5-04/05) |
| REQ-phase-2-prove-pipeline | Phase 2 | Complete |
| REQ-phase-2-1-semantic-analyzer | Phase 2 | Complete |
| REQ-phase-2-2a-ir-v0 | Phase 2 | Complete |
| REQ-phase-2-2b-ir-lowering | Phase 2 | Complete |
| REQ-phase-2-3-sysml-v2-codegen | Phase 2 | Complete |
| REQ-phase-2-3a-offline-validator | Phase 2 | Complete |
| REQ-phase-2-3b-golden-fixtures | Phase 2 | Complete |
| REQ-phase-2-4-formatter | Phase 2 | Complete |
| REQ-phase-2-5-cli-shell | Phase 2 | Complete |
| REQ-phase-2-gate | Phase 2 | Complete |
| REQ-phase-3-editor-intelligence | Phase 3 | Completed (Plan 03-07) |
| REQ-phase-3-1-textmate | Phase 3 | Completed (Plan 03-01) |
| REQ-phase-3-2-treesitter | Phase 3 | Completed (Plan 03-02) |
| REQ-phase-3-3-lsp-server | Phase 3 | Completed (Plan 03-03) |
| REQ-phase-3-4-vscode-lsp | Phase 3 | Completed (Plan 03-05) |
| REQ-phase-3-gate | Phase 3 | Completed (Plan 03-07) |
| REQ-phase-4-ecosystem | Phase 4 | Complete |
| REQ-phase-4-1-stdlib | Phase 4 | Complete |
| REQ-phase-4-2-reqif-codegen | Phase 4 | Complete |
| REQ-phase-4-3-docs-site | Phase 4 | Complete |
| REQ-phase-4-4-package-resolution | Phase 4 | Complete |
| REQ-phase-4-gate | Phase 4 | Complete |
| REQ-phase-5-simulation | Phase 5 | Complete |
| REQ-phase-5-1-deal-sim-sdk | Phase 5 | Complete |
| REQ-phase-5-2-orchestration | Phase 5 | Complete |
| REQ-phase-5-3-evidence-verification | Phase 5 | Complete |
| REQ-phase-5-gate | Phase 5 | Complete |
| REQ-phase-6-application | Phase 6 | Pending |
| REQ-phase-6-1-desktop-editor | Phase 6 | Pending |
| REQ-phase-6-2-import-pipelines | Phase 6 | Pending |
| REQ-phase-6-3-doc-generation | Phase 6 | Pending |
| REQ-phase-6-4-stdlib-expansion | Phase 6 | Pending |
| REQ-phase-6-gate | Phase 6 | Pending |

**Coverage:**

- v1 requirements: 48 total (1 Phase 0 + 7 Phase 1 + 7 Phase 1.5 + 10 Phase 2 + 6 Phase 3 + 6 Phase 4 + 5 Phase 5 + 6 Phase 6)
- Mapped to phases: 48
- Unmapped: 0 — coverage 100%
- Satisfied: 42/48 (Phases 0–5 complete); remaining 6 are Phase 6 (not started)

> **Note:** the inserted Phase 1.5 (7 requirements) was originally omitted from this table; added 2026-06-09. The milestone audit's historical 41-count denominator predates this reconciliation.

---

*Requirements defined: 2026-05-19*
*Last updated: 2026-06-09 — traceability reconciled by `/gsd:audit-milestone` (Phase 2 checkboxes, Phase 1.5 rows added, coverage recount). Initial definition 2026-05-19 from ingest synthesis.*
