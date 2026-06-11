# DEAL — Digital Engineering Authoring Language

## What This Is

DEAL is a text-first systems engineering compiler infrastructure with its own intermediate representation (IR) that transpiles to multiple target formats: SysML v2 JSON, ReqIF XML, FMI/FMU, ICD packages, and generated documents. The DEAL source file is the canonical artifact and single source of truth; the IR is the kernel; SysML v2 is one export target among several. DEAL serves defense/aerospace systems engineers and software engineers building integrations equally.

## Core Value

A defense/aerospace systems engineer can author a real subsystem in DEAL end-to-end, run it through codegen, and import the result into Cameo / DOORS / Modelica without manual fixup — with every construct readable cold by a human or AI agent without external documentation.

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

- ✓ **Phase 0 — Foundation**: 65 locked design decisions (64 LOCKED + 1 CAPTURED), three complete W3C EBNF grammars (`lexical.ebnf` 758 lines / `deal.ebnf` 1,679 lines / `dealx.ebnf` 897 lines), 19-file showcase model, verification report, parser implementation guide, acquired SysML v2/KerML JSON schemas and OMG spec PDFs — Phase 0 (2026-05-18)
- ✓ **Phase 1 — Zig compiler core**: Stateless four-mode lexer, recursive-descent parser for 87 `.deal` + 43 `.dealx` productions, three-tier error recovery (D-17), 19 byte-stable AST snapshots, 50-file malformed corpus surviving without panic, six-symbol production C ABI (`deal_parse`/`deal_free`/`deal_has_errors`/`deal_diagnostics_count`/`deal_diagnostics_json`/`deal_ast_json`), zero-leak verification via `std.testing.allocator` across 69 files, Rust FFI `gate_all_19` passing — Phase 1 (2026-05-20)
- ✓ **Phase 4 — Ecosystem**: `deal-stdlib` sibling repo (7-exponent SI dimensional metadata per ADR Option A, SI + imperial units, electrical/mechanical interface packages), git-based package resolution (`deal init`/`deal install`, git2 + SHA-pinned `deal.lock`, E2402 guard), dimensional algebra Check #7 in Zig sema (E2500–E2503), ReqIF 1.2 codegen backend (`deal build --target reqif`, XSD-validated `.reqifz`), `deal-lang.org` docs site (Astro/Starlight, DEAL/DEALX Shiki highlighting, 53-snippet parse gate), cumulative `phase-4-gate` + `phase-4-gate-fresh` green — Phase 4 (2026-06-06). Open: DOORS import UAT, CLI dimensional scope decision (04-HUMAN-UAT.md); 3 critical review findings tracked in 04-REVIEW.md.

### Active

<!-- Current scope. Building toward v1.0 (Phases 1–6). -->

See `.planning/REQUIREMENTS.md` for the full requirement list. Active phases:

- [x] **Phase 2**: Prove the pipeline — semantic analysis, DEAL IR v0, SysML v2 JSON codegen, formatter, CLI shell
- [x] **Phase 3**: Editor intelligence — TextMate grammars, tree-sitter, LSP, VS Code extension
- [x] **Phase 4**: Ecosystem — `deal-stdlib`, ReqIF codegen, `deal-lang.org` docs site, package resolution — complete 2026-06-06
- [ ] **Phase 5**: Simulation integration — `deal-sim` Python SDK, orchestration, evidence/verification
- [ ] **Phase 6**: Application — Tauri desktop editor, import pipelines, document generation, stdlib expansion

### Out of Scope (v1)

<!-- Explicit boundaries. Each is in PRD §11 deferred-decisions or explicitly out of scope. -->

- **Centralized package registry** — Phase 4 ships local-path and git-based resolution only; registry deferred to gate evaluation.
- **WASM packaging of the Zig core** — Phase 2 gate decision; evaluated after C ABI boundary proves stable.
- **Expression syntax details (arithmetic / comparison / logical)** — Deferred per ADR; only what the showcase model requires gets implemented in Phase 2.
- **Path expression syntax, MQL query language, full relationship category enumeration** — Deferred per ADR.
- **Default visibility when no `public|protected|private` wrapper** — Deferred per ADR (SD-9 leaves this open).
- **`@document:` category and `[<document>]` composition block** — Deferred per ADR; Phase 6 doc generation works without them.
- **Garbage collection in the compiler** — Native build, no GC (Zig core + Rust frontend).

## Context

**Project structure.** Repo lives at `/Users/dunnock/projects/deal-lang/`. The compiler implementation tree lives at `/Users/dunnock/projects/deal-lang/deal/` (this `.planning/` directory). Grammar specifications and the 19-file showcase model live at `/Users/dunnock/projects/deal-lang/spec/`. The authoritative roadmap document is `/Users/dunnock/projects/deal-lang/DEAL-LANG-ROADMAP.html`.

**Two-language compiler stack.** Phase 1 produces a Zig static library (`libdeal.a`) exposing a C ABI (`deal_parse`, `deal_free`, `deal_diagnostics`). Phase 2 introduces a Rust frontend (CLI, formatter integration, SysML v2 codegen) that calls into the Zig core via FFI. Phase 3 adds Rust LSP server (`tower-lsp`) and TypeScript VS Code extension. Phase 5 adds Python (`deal-sim` SDK on PyPI). Phase 6 adds Tauri + Next.js desktop editor.

**Grammar dependency layering is unidirectional.** `lexical.ebnf` (L1, shared tokens) → `deal.ebnf` (L2/L3, definitions) → `dealx.ebnf` (L4, compositions). `.deal` files never reference `.dealx` constructs. Parser selects grammar mode by file extension. Stack-based tag balancing is the `.dealx` parser's responsibility.

**Reference material acquired (Phase 0).** SysML v2 JSON schema (`spec/references/omg-sysml-v2/SysML.json`), KerML JSON schema (`spec/references/omg-kerml-v1/KerML.json`), OMG textual BNFs and spec PDFs. ReqIF 1.2 XSD must be acquired before Phase 4 Milestone 4.2.

**Verification status carried from Phase 0.** All 65 locked decisions trace to grammar productions. All 19 showcase files (`spec/grammar/examples/showcase/`) parse with disjoint FIRST sets, no orphan productions, no left recursion. Grammar is LL(1) at 8 decision points, LL(2) at 4 justified points.

**Ingest provenance.** All four `.planning/PROJECT.md` / `REQUIREMENTS.md` / `ROADMAP.md` / `STATE.md` documents derive from the ingest synthesis at `.planning/intel/` (entry point: `intel/SYNTHESIS.md`). Source documents: 1 consolidated ADR (65 decisions), 3 W3C EBNF grammars, 1 PRD (`DEAL-LANG-ROADMAP.html`), 1 grammar README. Zero blockers, zero warnings, 8 INFO entries (informational) — see `.planning/INGEST-CONFLICTS.md`.

**Phase-ordering authority.** `DESIGN-DECISIONS.md` §Implementation Staging explicitly cedes ordering authority to `DEAL-LANG-ROADMAP.html`. The ROADMAP.md phase shape mirrors the PRD's Phase 1..6 ordering, not any alternate ordering in the ADR.

## Constraints

- **Tech stack — compiler core**: Zig (lexer, recursive-descent parser, error recovery, arena AST allocation, C ABI). Native, no GC.
- **Tech stack — frontend tooling**: Rust (CLI via `clap`, formatter integration, SysML v2 / ReqIF codegen backends, LSP server via `tower-lsp`). Native, no GC.
- **Tech stack — editor**: TypeScript (VS Code extension), tree-sitter grammar (TS bindings), TextMate grammars (JSON).
- **Tech stack — simulation SDK**: Python (PyPI package `deal-sim`).
- **Tech stack — desktop application**: Tauri + Next.js (Phase 6).
- **Tech stack — documentation site**: Astro + Starlight, Shiki for code highlighting (Phase 4).
- **Grammar contract**: All three W3C EBNF grammars at `0.1.0-draft` are LOCKED specifications. Parsers must implement them faithfully; deviations require ADR.
- **Schema validation**: SysML v2 JSON output must validate against `spec/references/omg-sysml-v2/SysML.json` offline (no network at build time). ReqIF output must validate against OMG ReqIF 1.2 XSD.
- **Round-trip invariant**: `deal fmt` must satisfy parse → format → parse → identical AST for all 19 showcase files.
- **Identity**: `deal fmt` refuses to run without configured `deal config user.name`/`user.email` (per FS-3).
- **Compatibility — SysML v2 import**: SysML v2 symbolic operators (`:>`, `:>>`, etc.) are read by `deal import`; `deal fmt` normalizes to DEAL canonical form (`<<specializes>>`, `<<redefines>>`).

## Key Decisions

<!-- Locked design decisions are embedded in the <decisions> block below. -->
<!-- This table is for cross-cutting project decisions made during execution. -->

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Adopt full 7-phase PRD scope (Phase 0–6) | User-supplied roadmap scope; PRD ordering authority confirmed | — Pending |
| Phase ordering follows `DEAL-LANG-ROADMAP.html`, not ADR §Implementation Staging | ADR explicitly cedes ordering authority to PRD (INGEST-CONFLICTS INFO #4) | ✓ Good |
| Zig for core, Rust for frontend tooling | Native build, no GC, C ABI boundary keeps language ecosystems decoupled | — Pending |
| `lexical.ebnf` re-ingested at 758 lines (was 370 in README) | User completed the grammar after README was written; current file authoritative | ✓ Good |
| SD-6 treated as CAPTURED, not LOCKED | Single decision marked CAPTURED in ADR; full relationship-category enumeration deferred | ✓ Good |

---

<decisions>

## Locked Design Decisions

Source: `spec/grammar/DESIGN-DECISIONS.md` (consolidated ADR, precedence 0, `manifest_override: true`; promoted from tmp-references/ in Phase 4 Plan 09). Status as of 2026-05-16: 64 LOCKED + 1 CAPTURED (SD-6). These decisions are LOCKED — downstream phases cannot overwrite them. Modifications require a new ADR.

### Foundational Architecture

- **FA-1 [LOCKED]** — DEAL is a systems engineering compiler infrastructure. Text-first authoring language with its own IR that transpiles to multiple target formats. The DEAL IR is the kernel (not SysML v2 JSON); SysML v2 JSON is one export target among several; the IR carries everything (structure, agent metadata, simulation bindings); import and export are separate paths; the DEAL source file is the canonical artifact and single source of truth. **Pipeline:** DEAL Source → AST → DEAL IR (kernel) → multiple codegen backends (SysML v2 JSON, ReqIF XML, FMI/FMU, ICD packages, Documents).
- **FA-2 [LOCKED]** — Object-oriented library system is first-class. Reusable, composable, versionable libraries as a core language feature.
- **FA-3 [LOCKED]** — Dual-purpose design. DEAL serves defense/aerospace systems engineers and software engineers building integrations equally.
- **FA-4 [LOCKED]** — Self-explanatory syntax principle. Every construct must be understandable by an AI agent or human reading the file cold — without external documentation or language training.
- **FA-5 [LOCKED]** — Simulation integration is first-class. Models serve as simulation inputs and consume simulation outputs. JSON as universal I/O. `deal_sim` Python SDK. `deal simulate` CLI.

### Naming

- **NC-1 [LOCKED]** — Language: DEAL (Digital Engineering Authoring Language). CLI: `deal`. Extensions: `.deal` (definitions) / `.dealx` (compositions). Organization: `deal-lang` (GitHub). Website: `deal-lang.org`.

### Language Model Alignment

- **LM-1 [LOCKED]** — TypeScript as primary language feel. Deviations only for systems engineering constructs with no TypeScript analog.
- **LM-2 [LOCKED]** — Doc comments use JSDoc style: `/** ... */` with `@tag` directives. Doc comments precede declarations.
- **LM-3 [LOCKED]** — String literals — TypeScript model. Double quotes for strings; backticks for multi-line and template strings (`${expr}` interpolation). Single quotes accepted as alias; `deal fmt` normalizes to double quotes.

### File Structure

- **FS-1 [LOCKED]** — File anatomy. Optional `@header { ... }` → `package` declaration → zero or more `import` statements → model content.
- **FS-2 [LOCKED]** — `@header` block with CM fields: `path`, `schema`, `created`, `modified`, `reviewed`, `hash`, `status` (draft|review|baseline|superseded), `baseline`, `marking`. Timestamps include `by "Name <email>"` + optional `via tool-id`.
- **FS-3 [LOCKED]** — Attribution model. `by` identifies a human; `via` optionally identifies the tool. Git-style identity (`deal config --global user.name`/`user.email`). `deal fmt` refuses to run without configured identity. Human accountable; tool informational.
- **FS-4 [LOCKED]** — System-wide index. No per-file index. Project-wide `.deal/index.json` generated by `deal index`, gitignored. Queried by `deal query`, LSP, MCP server.

### Project Structure

- **PS-1 [LOCKED]** — `deal.toml` project manifest. TOML with `[project]`, `[workspace]`, `[workspace.aliases]`, `[dependencies]`, `[simulations]`, `[build.targets]`.
- **PS-2 [LOCKED]** — Package declarations with dot paths: `package sedan_project.vehicle;`.
- **PS-3 [LOCKED]** — Barrel exports via `index.deal`. Explicit selective exports follow TypeScript `index.ts` pattern: `export electrical.{HVDCPort, CANBus};`.
- **PS-4 [LOCKED]** — Five import forms: external (`import deal.std.units.{kg};`), workspace alias (`import tracking.{Radar};`), relative same-package (`import .local.{X};`), relative parent (`import ..sib.{Y};`), barrel glob (`import interfaces.*;`), plus aliased form (`import interfaces as intf;`).
- **PS-5 [LOCKED]** — Workspace aliases — pnpm-style. `[workspace.aliases]` maps alias names to package paths.
- **PS-6 [LOCKED]** — Unit literals — function form from libraries: `import deal.std.units.{kg}; attribute mass : Mass = kg(1500);`.
- **PS-7 [LOCKED]** — Build targets in `deal.toml`. `[build.targets]` maps target names to `{ format, output }` records.
- **PS-8 [LOCKED]** — Standard directory layout. `deal.toml`, `.deal/` (gitignored), `model/` (`.dealx`), `packages/` (`.deal`), `simulations/` (with `deal.sims.toml`), `test/data/`, `docs/`.
- **PS-9 [LOCKED]** — `deal config` for user identity. Git-style: `deal config --global user.name "..."` and `user.email "..."`.
- **PS-10 [LOCKED]** — `.deal/` generated directory. Gitignored. Contains index, parser cache, simulation I/O cache.

### Syntax — Definitions

- **SD-1 [LOCKED]** — Element keywords match SysML v2 vocabulary: `part def`, `port def`, `action def`, `state def`, `requirement def`, `constraint def`, `attribute def`, `item def`, `interface def`, `connection def`, `flow def`, `allocation def`.
- **SD-2 [LOCKED]** — Typing uses colon: `part engine : Engine;` / `attribute mass : Mass;`.
- **SD-3 [LOCKED]** — Double angle bracket delimiters for structural relationships: `<<specializes>>`, `<<redefines>>`, `<<subsets>>`, `<<satisfies>>`, `<<allocated to>>`, `<<derives from>>`.
- **SD-4 [LOCKED]** — Two-tier operator system. Tier 1 = declaration-level (no prefix). Tier 2 = annotation-level (with `@category:` prefix).
- **SD-5 [LOCKED]** — SysML v2 symbolic operators as import aliases: `:>` → `<<specializes>>`, `:>>` → `<<redefines>>`, etc. `deal import` reads; `deal fmt` normalizes.
- **SD-6 [CAPTURED — NOT LOCKED]** — Relationship categories. Defined categories: `@trace:`, `@connection:`, `@behavioral:`, `@flow:`, `@state:`, `@temporal:`, `@requirement:`, `@simulation:`, `@document:`. Full enumeration deferred.
- **SD-7 [LOCKED]** — Modifiers are bare keywords: `abstract`, `derived`, `readonly`, `ordered` prefix declarations.
- **SD-8 [LOCKED]** — Direction keywords are bare: `in`, `out`, `inout` prefix port declarations.
- **SD-9 [LOCKED]** — Visibility as scope wrappers: `public ( ... )`, `protected ( ... )`, `private ( ... )` wrap members. Default visibility when no wrapper is deferred.
- **SD-10 [LOCKED]** — Semicolons. Required on single-line declarations. Optional after block-closing braces.
- **SD-11 [LOCKED]** — Comments: `// single-line`, `/* block */`, `/** JSDoc */`.
- **SD-12 [LOCKED]** — String literals. Double quotes default; backticks for multi-line/template; single quotes accepted (fmt normalizes). Consistent with LM-3.
- **SD-13 [LOCKED]** — Unit literals — function form: `kg(1500)`, `V(800)`, `km(200) / hr(1)`. Consistent with PS-6.
- **SD-14 [LOCKED]** — Multiplicity. Square brackets: `[4]`, `[1..5]`, `[*]`, `[1..*]`, `[0..1]`. After the type annotation.
- **SD-15 [LOCKED]** — Annotation value syntax. Inline single-value `@confidence: 0.85`. Braces for multi-field: `@simulation:<<computes>> thermalProfile { equation: "...", tool: "python", entry: "..." }`.
- **SD-16 [LOCKED]** — Doc comments + structured annotations. `/** */` for narrative; `@` for queryable metadata.
- **SD-17 [LOCKED]** — File extensions. `.deal` for definition files; `.dealx` for composition files. `.dealx` can contain inline definitions for one-off types; `deal fmt` warns if an inline definition is referenced by multiple compositions. `.deal` files never import from `.dealx`. Parser uses extension to select grammar mode.
- **SD-18 [LOCKED]** — Needs, requirements, use cases as definition keywords. `need def NEED_RANGE { }`, `requirement def REQ_SYS_001 { }`, `use case def LongDistanceTrip { }`. Definitions in `.deal`; allocations and traceability in `.dealx`.
- **SD-19 [LOCKED]** — Definitions and allocations are separated. `packages/` (`.deal`) holds WHAT; `model/` (`.dealx`) holds HOW.
- **SD-20 [LOCKED]** — Requirements declare verification contracts. `requirement def` body contains `verification { accepts: [methods], rejects: [methods], threshold: attr, operator: ">="|"<="|"==", conditions: ... }`. `deal check` enforces method type-checking against accepts/rejects.

### Syntax — Compositions

- **CS-1 [LOCKED]** — Dual syntax model. `.deal` uses bare keyword definitions; `.dealx` uses `[<tag>]` syntax.
- **CS-2 [LOCKED]** — `[< >]` composition tags. JSX-like: `[<system EVPlatform>] ... [</system>]`, self-closing instances `[<BatteryPack as="battery" voltage={V(800)} />]`, attributes on `[<connect>]`, `[<expose>]`, etc.
- **CS-3 [LOCKED]** — Component instances use `as`. Instance binding via `as="name"`.
- **CS-4 [LOCKED]** — Physical/logical connection separation. `[<connect>]` separates `via=` (physical: cable, harness — typed against `connection def`) from `carrying=` (logical: power, data — typed against `flow def`).
- **CS-5 [LOCKED]** — `connection def` / `flow def` keywords. `connection def HVDCCable { }` (medium); `flow def PowerDelivery { }` (content).
- **CS-6 [LOCKED]** — `expose` keyword. `[<expose battery.hvOut as="hvPowerOut" />]`.
- **CS-7 [LOCKED]** — Hierarchical nesting. System > subsystem > subsystem. Each level has internal wiring and exposed interfaces.
- **CS-8 [LOCKED]** — Prop validation via multiplicity. `deal check` validates compositions satisfy required props (`[1]`), respect optionals (`[0..1]`), meet collection minimums (`[1..*]`).
- **CS-9 [LOCKED]** — Traceability composition block. `[<traceability EVPlatformTraces>]` encloses `[<allocate>]`, `[<satisfy>] => { ... }`, `[<validate>]`.
- **CS-10 [LOCKED]** — Satisfy block with executable criteria and typed returns. `[<satisfy requirement="..." by="..." method="...">] => { returnFields }` body contains `criteria { boolean exprs }`, `evidence simulation { source, binding, maps { src -> dst } }`, `compute { derived = ... }`, optional `gap { risk, mitigation }`. `method` type-checked against `verification.accepts`. `deal check --verify` evaluates all criteria.
- **CS-11 [LOCKED]** — Validate block. `[<validate requirement="..." by="...">]` body has `scenario:`, `status:`, `evidence:` text fields.
- **CS-12 [LOCKED]** — Allocate block. `[<allocate from="..." to="..." relationship=<<derives>> />]` declares directed allocation.
- **CS-13 [LOCKED]** — Satisfy returns populated via evidence maps. `evidence simulation { source, binding, maps { totalRange -> actualRange } }` connects simulation output fields to return value names.
- **CS-14 [LOCKED]** — Satisfy returns derived via compute blocks. `compute { margin = actualRange - REQ_SYS_001.minRange; ... }` derives additional return values.
- **CS-15 [LOCKED]** — Return values referenceable as `TraceName.ReqID.fieldName` — e.g., `EVPlatformTraces.REQ_SYS_003.actualMass` or `${EVPlatformTraces.REQ_SYS_001.marginPercent}`.
- **CS-16 [LOCKED]** — Verification method type-checking. If a requirement declares `accepts: [test]` and a satisfy block specifies `method="simulation"`, `deal check` errors. Method must match accepted list.

### Simulation Integration

- **SIM-1 [LOCKED]** — JSON as universal simulation I/O. `DEAL model → input.json → Simulation → output.json → DEAL model`.
- **SIM-2 [LOCKED]** — `deal_sim` Python SDK. Subclass `DealSimulation` declares typed `inputs = {...}` / `outputs = {...}` dicts and a `run(self, inputs) -> dict` method. `if __name__ == "__main__": MySim.cli()` activates CLI entry.
- **SIM-3 [LOCKED]** — Simulation registry — `deal.sims.toml`. `[simulations.<name>]` with `tool`, `entry`, `class`, `binds_to`, `annotation`, `inputs`/`outputs` `{ model_path, param, unit }` arrays.
- **SIM-4 [LOCKED]** — Simulation CLI commands. `deal simulate <name>`, `deal simulate --all`, `deal simulate --stale`, `deal check --simulations`, `deal check --verify`, `deal check --verify --run-sims`, `deal evidence capture`, `deal evidence baseline v2.1.0`.
- **SIM-5 [LOCKED]** — Three-level verification. Level 1: structural completeness. Level 2: criteria evaluation (PASS/FAIL/PARTIAL with margins). Level 3: evidence freshness.

### Decisions Explicitly Deferred (per ADR)

These are NOT locked. Downstream planning must not assume them:

- Expression syntax details (arithmetic, comparison, logical)
- Path expression syntax
- Full relationship category enumeration (SD-6 is CAPTURED, not LOCKED)
- MQL query language design
- Codegen backend API
- Default visibility when no wrapper (SD-9)
- Constraint expression syntax
- `@document:` category design
- `[<document>]` composition block for report generation

### Decision Provenance

- Source: `/Users/dunnock/projects/deal-lang/spec/grammar/DESIGN-DECISIONS.md` (single consolidated ADR; promoted from tmp-references/ in Phase 4 Plan 09)
- Session: 2026-05-16 (3 sessions)
- Total: 64 LOCKED + 1 CAPTURED across 8 categories
- Ingest precedence: 0 (manifest override, highest)
- Per `DESIGN-DECISIONS.md` §Implementation Staging: ordering authority for execution is ceded to `DEAL-LANG-ROADMAP.html` (the PRD)

</decisions>

---

*Last updated: 2026-06-06 after Phase 4 (ecosystem) completion.*
