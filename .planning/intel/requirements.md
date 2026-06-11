# DEAL Requirements Intel

Source PRD: `/Users/dunnock/projects/deal-lang/DEAL-LANG-ROADMAP.html`
Precedence: 2 (default PRD)
Status: Phase 0 complete; Phases 1–6 ahead.

Each roadmap phase and milestone is captured as one or more `REQ-` entries. The phase exit criterion serves as the acceptance criterion for the phase-level requirement; per-milestone deliverables serve as acceptance criteria for milestone-level requirements.

---

## Phase 0 — Foundation (COMPLETE)

### REQ-phase-0-foundation

- **Source:** `DEAL-LANG-ROADMAP.html` §1 Where You Are
- **Status:** COMPLETE
- **Description:** Spec-grade design foundation: 65 locked design decisions, complete lexical/definition/composition grammars, 19-file showcase model, verification report, parser implementation guide, acquired reference material (SysML v2/KerML JSON schemas, OMG spec PDFs, textual BNFs).
- **Acceptance:** All listed artifacts present and verified (LOCKED + COMPLETE + ACQUIRED status table in §1).

---

## Phase 1 — Foundation (Compiler Core)

### REQ-phase-1-foundation

- **Source:** `DEAL-LANG-ROADMAP.html` §4
- **Description:** Build the Zig compiler core: lexer, parser for `.deal` and `.dealx`, error recovery + diagnostics, C ABI boundary. ZIG track.
- **Acceptance (exit criterion):** `deal parse showcase/*.deal showcase/*.dealx` produces AST JSON for all 19 files, with zero panics and meaningful diagnostics on malformed input.
- **Estimated duration:** 6–8 weeks.

### REQ-phase-1-1-lexer

- **Source:** `DEAL-LANG-ROADMAP.html` §4 Milestone 1.1
- **Description:** Tokenizer for both file modes implementing the full `lexical.ebnf` token set. Lexer selects mode based on file extension; `[< >]` composition tags only activate in `.dealx` mode; `<<operator>>` delimiters need custom handling.
- **Acceptance:** libdeal lexer module produced; token snapshot tests for all 19 showcase files pass with zero UNKNOWN tokens; error tokens carry span + message.

### REQ-phase-1-2-parser-deal

- **Source:** `DEAL-LANG-ROADMAP.html` §4 Milestone 1.2
- **Description:** Recursive-descent parser implementing the 87 `deal.ebnf` productions, with Pratt parsing for expressions. Follow the phased build order from the implementation guide (file structure → element definitions → element bodies → annotations → expressions). AST uses arena allocation.
- **Acceptance:** All 15 `.deal` showcase files parse; AST JSON snapshots stable; Pratt expression parser working.

### REQ-phase-1-3-parser-dealx

- **Source:** `DEAL-LANG-ROADMAP.html` §4 Milestone 1.3
- **Description:** Composition parser for the 43 `dealx.ebnf` productions, with stack-based tag balancing for `[<system>]...[</system>]` etc. Self-closing tags are the common case. The `via={...}` and `carrying={...}` inline blocks on `[<connect>]` are the hardest parse points.
- **Acceptance:** All 4 `.dealx` showcase files parse; tag balance enforcement works; inline block parsing for connect/expose works.

### REQ-phase-1-4-error-recovery

- **Source:** `DEAL-LANG-ROADMAP.html` §4 Milestone 1.4
- **Description:** Survive malformed input gracefully. Synchronize to next statement/definition boundary. Structured diagnostic types with code, message, span, and optional fix suggestion.
- **Acceptance:** 50+ error recovery test cases; structured diagnostic types; span-accurate error reporting.

### REQ-phase-1-5-c-abi

- **Source:** `DEAL-LANG-ROADMAP.html` §4 Milestone 1.5
- **Description:** Expose the compiler as a static library `libdeal.a` with C ABI functions `deal_parse()`, `deal_free()`, `deal_diagnostics()`. Minimal Rust FFI test harness validates the integration architecture.
- **Acceptance:** `libdeal.a` static library; `deal.h` C header; Rust FFI test harness parses a showcase file and reads back the AST.

### REQ-phase-1-gate

- **Source:** `DEAL-LANG-ROADMAP.html` §4 Phase 1 Gate
- **Description:** Phase 1 exit gate.
- **Acceptance:** All 19 showcase files tokenize and parse through the Zig core; AST JSON snapshots stable; error recovery handles at least 50 malformed-input cases without panic; C ABI boundary proven with Rust harness. No external-facing release.

---

## Phase 2 — Prove the Pipeline

### REQ-phase-2-prove-pipeline

- **Source:** `DEAL-LANG-ROADMAP.html` §5
- **Description:** Semantic analysis + first codegen backend. ZIG + RUST.
- **Acceptance (exit criterion):** `deal build --target sysml-v2 showcase/` produces valid SysML v2 JSON importable into a SysML v2 viewer; `deal fmt` round-trips all showcase files; `deal check` catches type errors and unresolved imports.
- **Estimated duration:** 6–8 weeks (split into Phase 2A/2B if scope compresses).

### REQ-phase-2-1-semantic-analyzer

- **Source:** `DEAL-LANG-ROADMAP.html` §5 Milestone 2.1
- **Description:** Name resolution, type checking, import validation. Resolves package-qualified imports, element references, port type annotations. Enforces multiplicity constraints. Validates `<<specializes>>` targets exist and are type-compatible. Checks `@trace` references point to real requirements. Populates `.deal/index.json`.
- **Acceptance:** Name resolver; type checker; model index generation; import validation.

### REQ-phase-2-2a-ir-v0

- **Source:** `DEAL-LANG-ROADMAP.html` §5 Milestone 2.2a
- **Description:** Define the DEAL IR v0 contract: node kinds, stable element IDs, source spans, package/module identity, resolved-reference representation, relationship graph shape, metadata envelope, diagnostic attachment points, traversal/query API. Scoped to the 19-file showcase + 5–8 golden SysML fixtures.
- **Acceptance:** IR v0 schema/model document; stable ID + span strategy; relationship graph contract; backend traversal API contract.

### REQ-phase-2-2b-ir-lowering

- **Source:** `DEAL-LANG-ROADMAP.html` §5 Milestone 2.2b
- **Description:** AST → DEAL IR lowering pass in Zig. IR strips syntactic sugar, resolves references, represents the system model as a graph of typed elements. Carries everything Phase 2 backend needs: structural relationships, source spans, agent metadata (`@confidence`, `@rationale`), simulation bindings, traceability links. Realizes FA-1.
- **Acceptance:** AST → IR lowering pass; IR v0 conformance tests; IR query/traversal API.

### REQ-phase-2-3-sysml-v2-codegen

- **Source:** `DEAL-LANG-ROADMAP.html` §5 Milestone 2.3
- **Description:** SysML v2 JSON codegen from IR. Map DEAL IR elements to SysML v2 JSON schema types (`part def` → `PartDefinition`, `port` → `PortUsage`, `<<specializes>>` → `Specialization` in `ownedRelationship`, etc.). References: `spec/references/omg-sysml-v2/SysML.json`, `spec/references/omg-kerml-v1/KerML.json`. Schemas use absolute OMG `$id`/`$ref` URLs requiring local schema registry (see 2.3a).
- **Acceptance:** SysML v2 JSON emitter; schema validation of output; showcase model exported + schema-valid; showcase viewer/import smoke result recorded.

### REQ-phase-2-3a-offline-validator

- **Source:** `DEAL-LANG-ROADMAP.html` §5 Milestone 2.3a
- **Description:** Local SysML/KerML schema bundle + validation harness. Standard JSON Schema `$ref` resolver backed by the downloaded schema bundle. Optional `--validate` flag on `deal build`; CI integration.
- **Acceptance:** Local schema registry/resolver; `deal build --validate` flag; CI validation step.

### REQ-phase-2-3b-golden-fixtures

- **Source:** `DEAL-LANG-ROADMAP.html` §5 Milestone 2.3b
- **Description:** Hand-written expected SysML v2 JSON corpus. 5–8 minimal `.deal` source files paired with hand-written expected SysML v2 JSON output and schema validation results. Covers part definitions, port usages, specialization relationships, attribute usages, requirement definitions, traceability links, and one multi-file `.dealx` composition.
- **Acceptance:** 5–8 minimal DEAL → JSON fixture pairs; schema validation pass for each; viewer/import smoke test for at least one fixture; recorded viewer/import smoke result for showcase export.

### REQ-phase-2-4-formatter

- **Source:** `DEAL-LANG-ROADMAP.html` §5 Milestone 2.4
- **Description:** `deal fmt` — canonical formatter. AST → source pretty-printer enforcing canonical style. Round-trip test: parse → format → parse, compare ASTs.
- **Acceptance:** Pretty-printer module; round-trip tests pass for all 19 files; C ABI `deal_format()` exposed.

### REQ-phase-2-5-cli-shell

- **Source:** `DEAL-LANG-ROADMAP.html` §5 Milestone 2.5
- **Description:** Rust CLI shell wrapping the Zig core via the C ABI. Commands: `deal parse` (dump AST), `deal check` (validate), `deal fmt` (format), `deal build --target sysml-v2` (codegen). Built with `clap`. Error output should be `rustc`/`cargo`-quality.
- **Acceptance:** `deal` CLI binary; parse/check/fmt/build commands; colored diagnostic output.

### REQ-phase-2-gate

- **Source:** `DEAL-LANG-ROADMAP.html` §5 Phase 2 Gate
- **Description:** Phase 2 exit gate.
- **Acceptance:** `deal check` validates the showcase model; `deal build --target sysml-v2` emits JSON passing offline schema validation; `deal fmt` round-trips all 19 files; DEAL IR v0 documented + implemented; 5–8 golden output fixtures match hand-written expected JSON and validate; showcase export has recorded SysML v2 viewer/import smoke result (or documented blocker); systems engineer can author → validate → export to SysML v2 JSON. Internal demo-ready.

---

## Phase 3 — Editor Intelligence

### REQ-phase-3-editor-intelligence

- **Source:** `DEAL-LANG-ROADMAP.html` §6
- **Description:** VS Code extension + LSP + tree-sitter. RUST + TS.
- **Acceptance (exit criterion):** VS Code user opens the showcase project and gets: syntax highlighting, real-time diagnostics, autocomplete (element names + imports), cross-file go-to-definition, format-on-save.
- **Estimated duration:** 5–7 weeks.

### REQ-phase-3-1-textmate

- **Source:** `DEAL-LANG-ROADMAP.html` §6 Milestone 3.1
- **Description:** Syntax highlighting for `.deal` and `.dealx`. Write `deal.tmLanguage.json` and `dealx.tmLanguage.json`. Special scopes for `<<operator>>`, `@category:<<operator>>`, `[<tag>]`, and `@header { ... }`.
- **Acceptance:** vscode-deal extension (TextMate only); bracket matching + auto-close for `[< >]`; snippet library; file icons for `.deal` / `.dealx`.

### REQ-phase-3-2-treesitter

- **Source:** `DEAL-LANG-ROADMAP.html` §6 Milestone 3.2
- **Description:** Tree-sitter grammar for Neovim, Helix, Zed, GitHub. Derived from the EBNF spec but adapted to tree-sitter's PEG-like model. Includes `highlights.scm`, injection queries, indent queries.
- **Acceptance:** tree-sitter-deal package; highlight + indent queries; test corpus from showcase.

### REQ-phase-3-3-lsp-server

- **Source:** `DEAL-LANG-ROADMAP.html` §6 Milestone 3.3
- **Description:** Rust LSP server using `tower-lsp`, calling the Zig core via the C ABI. Capabilities: `textDocument/diagnostic`, `textDocument/completion`, `textDocument/definition`, `textDocument/hover`, `textDocument/formatting`. Incremental re-parse on file change. Uses model index for workspace-wide symbol resolution.
- **Acceptance:** deal-lsp binary; diagnostics, completion, definition, hover, formatting working; incremental re-parse; workspace symbol index.

### REQ-phase-3-4-vscode-lsp

- **Source:** `DEAL-LANG-ROADMAP.html` §6 Milestone 3.4
- **Description:** Wire the vscode-deal extension to the LSP server. Extension detects `deal.toml` to activate. Replace TextMate-only highlighting with tree-sitter-based highlighting where supported (semantic tokens provider).
- **Acceptance:** vscode-deal extension with LSP; bundled deal-lsp binary; semantic tokens provider.

### REQ-phase-3-gate

- **Source:** `DEAL-LANG-ROADMAP.html` §6 Phase 3 Gate
- **Description:** Phase 3 exit gate.
- **Acceptance:** Opening the showcase project in VS Code delivers syntax highlighting, real-time error checking, autocomplete, cross-file go-to-definition, and format-on-save — experience comparable to TypeScript in VS Code.

---

## Phase 4 — Ecosystem

### REQ-phase-4-ecosystem

- **Source:** `DEAL-LANG-ROADMAP.html` §7
- **Description:** Standard library + second backend (ReqIF) + documentation site.
- **Acceptance (exit criterion):** A real project can be authored using `deal-stdlib` units and interfaces, exported to both SysML v2 JSON and ReqIF XML, and its documentation is readable on `deal-lang.org`.
- **Estimated duration:** 6–8 weeks.

### REQ-phase-4-1-stdlib

- **Source:** `DEAL-LANG-ROADMAP.html` §7 Milestone 4.1
- **Description:** Author the first standard library packages in DEAL itself. SI base + derived units from BIPM. Electrical interfaces (RJ45, USB-C, CAN, RS-422). Mechanical interfaces (common bolt patterns).
- **Acceptance:** deal-stdlib/units (SI + imperial); deal-stdlib/interfaces/electrical; deal-stdlib/interfaces/mechanical; package resolution for deal.toml dependencies.

### REQ-phase-4-2-reqif-codegen

- **Source:** `DEAL-LANG-ROADMAP.html` §7 Milestone 4.2
- **Description:** Second codegen backend: IR → ReqIF XML for DOORS / Jama. Map DEAL requirements (`requirement def`, `need def`), traceability links (`@trace:<<satisfies>>`), and verification blocks to ReqIF XML elements.
- **Prerequisite:** Acquire ReqIF normative references (ReqIF 1.2 XSD from OMG, DOORS-compatible sample export, optional Jama sample). Place in `spec/references/omg-reqif/`. Without the XSD, the emitter has no validation target.
- **Acceptance:** ReqIF XML emitter; showcase requirements exported; DOORS import validation.

### REQ-phase-4-3-docs-site

- **Source:** `DEAL-LANG-ROADMAP.html` §7 Milestone 4.3
- **Description:** `deal-lang.org` on Astro + Starlight. Landing page, getting-started guide, language reference (definitions, compositions, requirements, traceability, imports, annotations), CLI reference, VS Code setup. Code examples use a custom Shiki grammar built from the TextMate grammar.
- **Critical acceptance criterion:** Every code block on the site must render with actual syntax highlighting — no placeholder text, no broken includes, no unstyled fences. CI step detects unresolved snippet references and missing Shiki grammar scopes. If a `.deal` or `.dealx` code block renders without highlighting, the build fails.
- **Acceptance:** deal-lang.org site; language reference; getting-started tutorial; Shiki grammar for code highlighting; CI snippet render validation.

### REQ-phase-4-4-package-resolution

- **Source:** `DEAL-LANG-ROADMAP.html` §7 Milestone 4.4
- **Description:** `deal.toml` dependency resolution. Local-path and git-based resolution (no registry yet). Standard library consumed as a dependency. Lockfile generation for reproducible builds.
- **Acceptance:** deal.lock generation; local + git dependency resolution; `deal install` command.

### REQ-phase-4-gate

- **Source:** `DEAL-LANG-ROADMAP.html` §7 Phase 4 Gate
- **Description:** Phase 4 exit gate — minimum viable language.
- **Acceptance:** New project can scaffold with `deal init`, depend on `deal-std` for units/interfaces, author definitions and compositions, validate with `deal check`, export to SysML v2 JSON or ReqIF XML, read documentation on `deal-lang.org`. ReqIF output validates against OMG XSD. Documentation site CI passes. First external release candidate.

---

## Phase 5 — Simulation Integration

### REQ-phase-5-simulation

- **Source:** `DEAL-LANG-ROADMAP.html` §8
- **Description:** `deal-sim` SDK + simulation pipeline.
- **Acceptance (exit criterion):** `deal simulate --all` runs the showcase model's Python and MATLAB simulations, captures results as evidence, and `deal check --verify` evaluates verification criteria against simulation output.
- **Estimated duration:** 5–7 weeks.

### REQ-phase-5-1-deal-sim-sdk

- **Source:** `DEAL-LANG-ROADMAP.html` §8 Milestone 5.1
- **Description:** `deal-sim` Python SDK. `DealSimulation` base class declares typed inputs/outputs. CLI runner reads `input.json`, invokes simulation, validates output against declared schema, writes `output.json` with metadata.
- **Acceptance:** deal-sim PyPI package; DealSimulation base class; input/output validation; CLI runner.

### REQ-phase-5-2-orchestration

- **Source:** `DEAL-LANG-ROADMAP.html` §8 Milestone 5.2
- **Description:** `deal.sims.toml` parser + orchestration engine. Resolves the dependency graph between simulations, determines staleness, executes in dependency order. `deal simulate --stale` only re-runs simulations whose inputs have changed. MATLAB simulations via MATLAB Engine API for Python.
- **Acceptance:** Simulation registry parser; dependency graph resolution; staleness detection; `deal simulate` command.

### REQ-phase-5-3-evidence-verification

- **Source:** `DEAL-LANG-ROADMAP.html` §8 Milestone 5.3
- **Description:** `deal evidence capture` snapshots simulation results + test data into the evidence cache. `deal check --verify` evaluates verification criteria. `deal evidence baseline v2.1.0` tags a verified state. Traceability model shows full coverage status.
- **Acceptance:** `deal evidence` command; `deal check --verify`; verification status reporting; baseline tagging.

### REQ-phase-5-gate

- **Source:** `DEAL-LANG-ROADMAP.html` §8 Phase 5 Gate
- **Description:** Phase 5 exit gate.
- **Acceptance:** Showcase model simulations run, produce evidence, verification criteria evaluate against that evidence. A program manager can ask "is REQ_BAT_001 verified?" and get a traceable answer from the model.

---

## Phase 6 — Application

### REQ-phase-6-application

- **Source:** `DEAL-LANG-ROADMAP.html` §9
- **Description:** Desktop editor + import pipelines + advanced backends.
- **Acceptance (exit criterion):** Desktop application where systems engineers author DEAL models with text-first editing, view auto-generated diagrams, and see traceability status — replacing Cameo/Sparx for the authoring workflow.
- **Estimated duration:** 10–14 weeks (parallelizable internally).

### REQ-phase-6-1-desktop-editor

- **Source:** `DEAL-LANG-ROADMAP.html` §9 Milestone 6.1
- **Description:** Desktop editor (Tauri + Next.js). Text-first MBSE application. CodeMirror 6 with DEAL language mode (tree-sitter highlighting + LSP intelligence). Model browser tree. Read-only diagram views generated from the IR (block definition, internal block, sequence diagrams). Traceability matrix view. Verification dashboard. Tauri backend manages file watching, model state, IPC with Zig core via Rust FFI.
- **Acceptance:** Desktop application binary; text editor with LSP; model browser; auto-generated diagrams; traceability matrix.

### REQ-phase-6-2-import-pipelines

- **Source:** `DEAL-LANG-ROADMAP.html` §9 Milestone 6.2
- **Description:** Reverse codegen direction. `deal import --from sysml-v2 model.json` converts SysML v2 JSON into `.deal`/`.dealx` files. `deal import --from reqif requirements.xml` converts ReqIF into requirement definitions. Output must be idiomatic DEAL, not a syntactic transliteration.
- **Acceptance:** SysML v2 → DEAL importer; ReqIF → DEAL importer; `deal import` command.

### REQ-phase-6-3-doc-generation

- **Source:** `DEAL-LANG-ROADMAP.html` §9 Milestone 6.3
- **Description:** `deal build --target docs` generates documentation from the model: system description documents, interface control documents, verification status reports. Template-driven (`deal.toml` specifies template name, e.g., `"program-review"`).
- **Acceptance:** Document generation backend; HTML + PDF output; configurable templates.

### REQ-phase-6-4-stdlib-expansion

- **Source:** `DEAL-LANG-ROADMAP.html` §9 Milestone 6.4
- **Description:** Expand deal-stdlib with protocols (MIL-STD-1553, ARINC 429, SpaceWire), standards (DO-178C, DO-254 assurance levels; MIL-STD-810H environmental test method references), patterns (TMR, dual-redundant, watchdog).
- **Acceptance:** deal-stdlib/protocols; deal-stdlib/standards; deal-stdlib/patterns.

### REQ-phase-6-gate

- **Source:** `DEAL-LANG-ROADMAP.html` §9 Phase 6 Gate
- **Description:** Phase 6 exit gate — full vision delivered.
- **Acceptance:** Engineer can open the desktop editor, author a system model in DEAL using stdlib components, see auto-generated diagrams, run simulations, check verification status, export to SysML v2 JSON / ReqIF XML / generated docs. Import from existing SysML v2 and ReqIF sources works.

---

## Cross-cutting / Deferred decision points (per PRD §11)

The PRD documents these decision points to be evaluated at named gates (not yet locked):

- **Phase 2 gate → WASM packaging:** Whether to ship npm-installable WASM compilation of the Zig core for in-browser playground. Evaluate after C ABI boundary is proven.
- **Phase 3 gate → TextMate vs tree-sitter in VS Code:** Depends on whether VS Code's experimental tree-sitter support is stable by Phase 3.
- **Phase 4 gate → Package registry:** Is a centralized package registry needed, or do git-based dependencies suffice?
- **Phase 5 gate → Desktop editor priority:** Does pilot audience need Tauri desktop editor, or is CLI + VS Code sufficient?
- **Phase 4 gate → First public release:** Phase 4 vs Phase 5 timing for initial public release.

---

*Total: ~30 requirement entries extracted across 7 phases (0–6).*
