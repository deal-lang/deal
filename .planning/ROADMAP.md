# Roadmap: DEAL — Digital Engineering Authoring Language

## Overview

DEAL is a text-first systems engineering compiler infrastructure with its own intermediate representation (IR) that transpiles to multiple target formats (SysML v2 JSON, ReqIF XML, FMI/FMU, ICD packages, generated documents). The journey runs from a completed spec-grade foundation (Phase 0) through a Zig compiler core (Phase 1), a first end-to-end pipeline to SysML v2 (Phase 2), editor intelligence (Phase 3), the minimum viable language with stdlib + ReqIF + docs site (Phase 4), simulation integration (Phase 5), and finally a desktop application with import pipelines that lets a defense/aerospace systems engineer author a real subsystem end-to-end and import the result into Cameo / DOORS / Modelica without manual fixup (Phase 6).

Phase ordering follows `DEAL-LANG-ROADMAP.html` (the PRD), which holds ordering authority per `DESIGN-DECISIONS.md` §Implementation Staging.

## Phases

**Phase Numbering:**

- Integer phases (0, 1, 2, ...): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Phase 0 is the completed spec-grade foundation; it remains in the roadmap as a historical anchor and is collapsed below.

- [x] **Phase 0: Foundation** — Spec-grade design (65 decisions, 3 EBNF grammars, 19-file showcase) — COMPLETE 2026-05-18
- [x] **Phase 1: Zig Compiler Core** — Lexer + parser (`.deal`/`.dealx`) + error recovery + C ABI boundary (completed 2026-05-20)
- [x] **Phase 1.5: Parser Strictness & Test-Gap Resolution** — INSERTED — Close test-quality gaps surfaced by `/gsd:validate-phase 1` audit: vacuous assertions, soft-gate parser strictness (m08/m18/m19), determinism contract, recovery-boundary structural assertions, malformed-corpus per-file error pinning. INITIALLY DECLARED COMPLETE 2026-05-21 (false-GREEN — dev-local untracked symlink masked 9 failures); RE-VERIFIED COMPLETE 2026-05-21 via Plan 05 (spec/ committed as submodule + tests/showcase symlink + `phase-1.5-gate-fresh` ephemeral-worktree gate + ADR-phase-1.5-fresh-worktree-verification.md locks the invariant for Phases 2..6).
- [x] **Phase 2: Prove the Pipeline** — Semantic analysis + DEAL IR v0 + SysML v2 JSON codegen + `deal fmt` + CLI (completed 2026-05-22)
- [x] **Phase 3: Editor Intelligence** — TextMate + tree-sitter + LSP + VS Code extension (completed 2026-05-27)
- [x] **Phase 4: Ecosystem** — `deal-stdlib` + ReqIF codegen + `deal-lang.org` docs site + package resolution (completed 2026-06-06)
- [x] **Phase 5: Simulation Integration** — `deal-sim` Python SDK + orchestration + evidence/verification (completed 2026-06-08)
- [ ] **Phase 6: Application** — Tauri desktop editor + import pipelines + document generation + stdlib expansion

## Phase Details

### Phase 0: Foundation (COMPLETE)

**Goal**: Establish a spec-grade design foundation — locked syntax decisions, complete grammars, parseable showcase corpus, acquired reference material — before any compiler implementation begins.
**Depends on**: Nothing (first phase)
**Requirements**: REQ-phase-0-foundation
**Success Criteria** (what must be TRUE):

  1. 65 design decisions are recorded with LOCKED status in a consolidated ADR (64 LOCKED + 1 CAPTURED).
  2. Three W3C EBNF grammars (`lexical.ebnf`, `deal.ebnf`, `dealx.ebnf`) exist at `0.1.0-draft` and cover all 65 decisions.
  3. All 19 showcase files in `examples/showcase/` are parseable per the verification report.
  4. SysML v2 / KerML JSON schemas and OMG textual BNFs are acquired and stored under `spec/references/`.
  5. Parser implementation guide and integration verification report are written.

**Plans**: Complete

### Phase 1: Zig Compiler Core

**Goal**: `deal parse showcase/*.deal showcase/*.dealx` produces AST JSON for all 19 files, with zero panics and meaningful diagnostics on malformed input. Compiler is consumable from Rust via a C ABI.
**Depends on**: Phase 0
**Requirements**: REQ-phase-1-foundation, REQ-phase-1-1-lexer, REQ-phase-1-2-parser-deal, REQ-phase-1-3-parser-dealx, REQ-phase-1-4-error-recovery, REQ-phase-1-5-c-abi, REQ-phase-1-gate
**Success Criteria** (what must be TRUE):

  1. A Zig lexer tokenizes all 19 showcase files with zero UNKNOWN tokens; lexer mode selection by file extension works (`[< >]` activates only in `.dealx`).
  2. The 15 `.deal` showcase files parse through a recursive-descent + Pratt parser covering the 87 `deal.ebnf` productions, producing stable AST JSON snapshots.
  3. The 4 `.dealx` showcase files parse through a composition parser covering the 43 `dealx.ebnf` productions, with stack-based tag balancing and inline `via={...}`/`carrying={...}` blocks working.
  4. The parser survives 50+ malformed-input test cases without panic, emitting span-accurate structured diagnostics that synchronize at statement/definition boundaries.
  5. A Rust FFI test harness loads `libdeal.a`, parses a showcase file via `deal_parse()`, and reads back the AST through `deal.h`.

**Plans**: 6 plans
Plans:

- [x] 01-01-foundation-PLAN.md — Wave 0 scaffolding: build.zig + build.zig.zon + 11 src/*.zig stubs + include/deal.h + tests/ harness + Rust FFI scaffold (REQ-phase-1-foundation) — COMPLETE 2026-05-19, see 01-01-SUMMARY.md
- [x] 01-02-lexer-PLAN.md — Stateless lexer-on-demand with four-mode dispatch; 19 showcase files lex to zero UNKNOWN tokens (REQ-phase-1-1-lexer) — COMPLETE 2026-05-19, see 01-02-SUMMARY.md
- [x] 01-03-parser-deal-PLAN.md — Recursive-descent + Pratt parser for 87 deal.ebnf productions; 15 .deal AST snapshots stable (REQ-phase-1-2-parser-deal)
- [x] 01-04-parser-dealx-PLAN.md — Composition parser for 43 dealx.ebnf productions with stack-based tag balancing + inline via={...}/carrying={...} (REQ-phase-1-3-parser-dealx)
- [x] 01-05-error-recovery-PLAN.md — Three-tier recovery (D-17) + Diagnostic round-trip + ≥50-file malformed corpus (REQ-phase-1-4-error-recovery) — COMPLETE 2026-05-20, see 01-05-SUMMARY.md
- [x] 01-06-c-abi-and-gate-PLAN.md — Production C ABI hardening + zero-leak verification + Rust FFI gate_all_19 test (REQ-phase-1-5-c-abi + REQ-phase-1-gate)

### Phase 1.5: Parser Strictness & Test-Gap Resolution (INSERTED)

**Goal**: Phase 1's automated gates are green, but `/gsd:validate-phase 1` surfaced concrete test-quality weaknesses (vacuous `>= 0` assertions, soft-gate parser strictness, no determinism check, recovery tests assert presence rather than recovered AST shape, m08/m18/m19 malformed files slip through silently). Close those gaps so Phase 2's semantic analyzer builds on a parser whose contract is *asserted*, not merely *intended*.
**Depends on**: Phase 1
**Requirements**: REQ-phase-1.5-strictness-and-tests, REQ-phase-1.5-1-test-assertions, REQ-phase-1.5-2-malformed-pinning, REQ-phase-1.5-3-determinism, REQ-phase-1.5-4-parser-strictness, REQ-phase-1.5-5-property-tests, REQ-phase-1.5-gate
**Success Criteria** (what must be TRUE):

  1. No `*.zig` test assertion of the form `>= 0` against a `*.len` (unsigned) value; every recovery test asserts (a) exact diagnostic code, (b) recovered AST structure at the expected position, not just `definitions.len >= 1`.
  2. Each of the 20 hand-curated `m01..m20` malformed files declares an expected primary diagnostic code; `recovery.corpus` enforces the pin (per-file expected code matches first diagnostic emitted), and the soft 60 % gate becomes a per-file pin for the hand-curated set while remaining soft for the 30 generated files.
  3. A new `determinism.parse_twice` test parses every showcase file twice in the same process and asserts byte-identical AST JSON; the D-18 alphabetical-key invariant is now automated, not implicit.
  4. m08 (unterminated block comment), m18 (dangling dot operator), and m19 (empty arg with comma) each produce a defined diagnostic code (or are explicitly documented as accepted via an ADR with rationale).
  5. A new property-style test asserts every AST node's span is contained within its parent's span across the 19-file showcase corpus.
  6. Phase 1.5 exit gate: `zig build phase-1-gate && zig build test -Dtest-filter=determinism && zig build test -Dtest-filter=property` all exit 0; LEARNINGS.md Surprise #8 backlog items are all addressed or formally deferred.
  7. **(Added 2026-05-21 by Plan 05 gap closure)** Phase 1.5 exit gate runs GREEN inside a FRESHLY CREATED git worktree of the deal repo after `git submodule update --init --recursive` — i.e., `zig build phase-1.5-gate-fresh` exits 0. The original criterion #6 alone was insufficient because the developer's main checkout had a local-only `tests/showcase` symlink that masked a 9-test failure on any clean environment.

**Plans**: 5 plans
Plans:

- [x] 01.5-01-test-assertions-and-property-PLAN.md — Strengthen 9 recovery test cases (exact code + recovered AST shape) + new property.span_containment test (REQ-phase-1.5-1-test-assertions, REQ-phase-1.5-5-property-tests) — wave 1 — COMPLETE 2026-05-20, see 01.5-01-SUMMARY.md
- [x] 01.5-02-parser-strictness-PLAN.md — m08 wires EXISTING E0003 e_unterminated_comment (lexer→parser drain); m18 emits new E0121 e_dangling_dot; m19 emits new E0122 e_empty_arg_comma (REQ-phase-1.5-4-parser-strictness) — wave 2 — COMPLETE 2026-05-20, see 01.5-02-SUMMARY.md
- [x] 01.5-03-malformed-pinning-PLAN.md — Per-file pin table for m01..m20 in recovery.corpus + ADR for m01 SD-10 accepted-no-diag disposition (REQ-phase-1.5-2-malformed-pinning) — wave 3 — COMPLETE 2026-05-20, see 01.5-03-SUMMARY.md
- [x] 01.5-04-determinism-and-gate-PLAN.md — determinism.parse_twice test + phase-1.5-gate build step + final exit-gate verification (REQ-phase-1.5-3-determinism, REQ-phase-1.5-gate, REQ-phase-1.5-strictness-and-tests) — wave 4 — COMPLETE 2026-05-21 (NOTE: original GREEN claim AMENDED on 2026-05-21 — false-GREEN due to local-only tests/showcase symlink; see 01.5-05 below for actual closure), see 01.5-04-SUMMARY.md
- [x] 01.5-05-tests-showcase-submodule-and-verify-harden-PLAN.md — Gap closure: spec/ committed as submodule (pinned fe98f89, SSH URL); tests/showcase committed as relative symlink → ../spec/examples/showcase; phase-1.5-gate-fresh ephemeral-worktree gate (33/33 tests passing); ADR-phase-1.5-fresh-worktree-verification.md locks the invariant for Phases 2..6 (REQ-phase-1.5-gate, REQ-phase-1.5-strictness-and-tests) — wave 5 — COMPLETE 2026-05-21 (false-GREEN remediation), see 01.5-05-SUMMARY.md

### Phase 2: Prove the Pipeline

**Goal**: `deal build --target sysml-v2 showcase/` produces valid SysML v2 JSON importable into a SysML v2 viewer; `deal fmt` round-trips all showcase files; `deal check` catches type errors and unresolved imports. A systems engineer can author → validate → export to SysML v2 JSON.
**Depends on**: Phase 1.5
**Requirements**: REQ-phase-2-prove-pipeline, REQ-phase-2-1-semantic-analyzer, REQ-phase-2-2a-ir-v0, REQ-phase-2-2b-ir-lowering, REQ-phase-2-3-sysml-v2-codegen, REQ-phase-2-3a-offline-validator, REQ-phase-2-3b-golden-fixtures, REQ-phase-2-4-formatter, REQ-phase-2-5-cli-shell, REQ-phase-2-gate
**Success Criteria** (what must be TRUE):

  1. `deal check` validates the showcase model: name resolution, type checking, multiplicity enforcement, `<<specializes>>` type-compatibility, `@trace` reference validation, and import resolution all work; `.deal/index.json` is generated.
  2. DEAL IR v0 is documented (schema/model document, stable ID + span strategy, relationship graph contract, backend traversal API) and implemented via an AST → IR lowering pass with conformance tests.
  3. `deal build --target sysml-v2` emits JSON that passes offline schema validation against the bundled SysML v2 / KerML schemas, and 5–8 hand-written golden fixtures match expected output byte-for-byte.
  4. `deal fmt` round-trips all 19 showcase files (parse → format → parse → identical AST).
  5. A Rust `deal` CLI binary exposes `parse`, `check`, `fmt`, `build --target sysml-v2` commands with colored, `rustc`-quality diagnostic output.
  6. Showcase export has a recorded SysML v2 viewer/import smoke result (or a documented blocker).

**Plans**: TBD

### Phase 3: Editor Intelligence

**Goal**: Opening the showcase project in VS Code delivers syntax highlighting, real-time error checking, autocomplete, cross-file go-to-definition, and format-on-save — experience comparable to TypeScript in VS Code.
**Depends on**: Phase 2
**Requirements**: REQ-phase-3-editor-intelligence, REQ-phase-3-1-textmate, REQ-phase-3-2-treesitter, REQ-phase-3-3-lsp-server, REQ-phase-3-4-vscode-lsp, REQ-phase-3-gate
**Success Criteria** (what must be TRUE):

  1. A VS Code user opening a `.deal` or `.dealx` file sees syntax highlighting via TextMate grammars (with dedicated scopes for `<<operator>>`, `@category:<<operator>>`, `[<tag>]`, `@header { ... }`) plus bracket matching and snippets.
  2. A tree-sitter-deal package provides highlight and indent queries for Neovim, Helix, Zed, and GitHub, with a test corpus derived from the showcase.
  3. A `deal-lsp` binary (Rust, `tower-lsp`) calls the Zig core via C ABI and delivers `textDocument/diagnostic`, `completion`, `definition`, `hover`, and `formatting` with incremental re-parse and a workspace-wide symbol index.
  4. The vscode-deal extension auto-activates on `deal.toml`, bundles `deal-lsp`, and exposes semantic tokens — replacing TextMate-only highlighting where supported.
  5. Format-on-save invokes `deal fmt` through the LSP without losing identity or content.

**Plans**: TBD
**UI hint**: yes

### Phase 4: Ecosystem

**Goal**: A new project can scaffold with `deal init`, depend on `deal-std` for units/interfaces, author definitions and compositions, validate with `deal check`, export to SysML v2 JSON or ReqIF XML, and read documentation on `deal-lang.org`. First external release candidate.
**Depends on**: Phase 3
**Requirements**: REQ-phase-4-ecosystem, REQ-phase-4-1-stdlib, REQ-phase-4-2-reqif-codegen, REQ-phase-4-3-docs-site, REQ-phase-4-4-package-resolution, REQ-phase-4-gate
**Success Criteria** (what must be TRUE):

  1. A user can write `import deal.std.units.{kg, V}; attribute mass : Mass = kg(1500);` in their own project and `deal check` resolves the dependency through `deal.toml` against `deal-stdlib` (units, electrical interfaces, mechanical interfaces).
  2. `deal build --target reqif` emits XML that validates against the OMG ReqIF 1.2 XSD and imports successfully into DOORS for the showcase requirements.
  3. `deal-lang.org` is live on Astro + Starlight with landing, getting-started, language reference, CLI reference, and VS Code setup pages — and every `.deal`/`.dealx` code block renders with actual Shiki syntax highlighting (CI fails the build on placeholders or missing scopes).
  4. `deal install` resolves local-path and git-based dependencies and generates a `deal.lock` for reproducible builds.
  5. A new project scaffolded with `deal init` can be authored, validated, exported to both SysML v2 JSON and ReqIF XML, and its docs are readable on `deal-lang.org`.

**Plans**: 9 plans

- [x] 04-01-PLAN.md — ReqIF XSD bundle + E-code reservations + Wave-0 test scaffolds
- [x] 04-02-PLAN.md — package resolution: deal.toml [dependencies], deal install, deal.lock, deal init
- [x] 04-03-PLAN.md — deal-stdlib units package + dimension-metadata ADR
- [x] 04-04-PLAN.md — dimensional algebra sema Check #7 (E2500..E2503) + vendored-stdlib resolution
- [x] 04-05-PLAN.md — deal-stdlib electrical + mechanical interface packages
- [x] 04-06-PLAN.md — ReqIF emitter (reqif.rs/reqif_schema.rs) + .reqifz + golden fixtures + DOORS smoke
- [x] 04-07-PLAN.md — deal-lang.org Astro+Starlight scaffold + Shiki grammars + landing/getting-started + deploy
- [x] 04-08-PLAN.md — docs language/CLI reference pages + snippet parse gate + Shiki scope gate
- [x] 04-09-PLAN.md — phase-4-gate + phase-4-gate-fresh + closeout

**UI hint**: yes

### Phase 5: Simulation Integration

**Goal**: `deal simulate --all` runs the showcase model's Python and MATLAB simulations, captures results as evidence, and `deal check --verify` evaluates verification criteria against simulation output. A program manager can ask "is REQ_BAT_001 verified?" and get a traceable answer from the model.
**Depends on**: Phase 4
**Requirements**: REQ-phase-5-simulation, REQ-phase-5-1-deal-sim-sdk, REQ-phase-5-2-orchestration, REQ-phase-5-3-evidence-verification, REQ-phase-5-gate
**Success Criteria** (what must be TRUE):

  1. A Python simulation author can `from deal_sim import DealSimulation`, declare typed `inputs`/`outputs` dicts, implement `run(self, inputs) -> dict`, and `MySim.cli()` produces an `output.json` validated against the declared schema.
  2. `deal simulate --all` discovers simulations via `deal.sims.toml`, resolves the dependency graph, runs them in order, and `deal simulate --stale` re-runs only simulations whose inputs changed (MATLAB simulations execute via the `matlab -batch` subprocess adapter per the D-72 ADR — not the MATLAB Engine API; when MATLAB is absent the sim graceful-skips with a `skip.json` rather than hard-failing, and `deal check --simulations` validates the MATLAB bindings structurally).
  3. `deal evidence capture` snapshots simulation results and test data into the evidence cache; `deal evidence baseline v2.1.0` tags a verified state.
  4. `deal check --verify` performs three-level verification (structural completeness, criteria evaluation with PASS/FAIL/PARTIAL and margins, evidence freshness) against the cached evidence.
  5. The traceability model surfaces full coverage status: a query against a `REQ_*` ID returns its satisfaction status, supporting evidence, and any stale results.

**Plans**: 7 plans
Plans:
**Wave 1**

- [x] 05-01-PLAN.md — Wave 0 scaffolding: deal_check_with_stdlib C ABI export + spec/sims/v0 protocol + Rust module/clap stubs + red test stubs + phase-5-gate(-fresh) + fresh-worktree deal-sim fix

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 05-02-PLAN.md — deal-sim Python SDK: DealSimulation base class + cli() JSON runner + stdlib-only validation + MATLAB/generic adapters + local wheel (battery_thermal.py oracle)
- [x] 05-03-PLAN.md — Rust orchestrator: deal.sims.toml parse + Kahn dep-graph + SHA-256 staleness + tool dispatch + MATLAB graceful-skip + model_path->IR resolution
- [x] 05-04-PLAN.md — Zig sim path: deal_sim.zig comptime @setFloatMode wrapper + in-process runner + canonical range_model.zig + D-88 cross-file E2500 CLI wiring + MATLAB-subprocess ADR

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 05-05-PLAN.md — Evidence: deal evidence capture + baseline <tag> frozen snapshot + manifest (hashes/verdicts), D-81 gitignore split, D-18 byte-stable

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 05-06-PLAN.md — Verify engine: narrow showcase expression evaluator + SIM-5 three-level verdict (PASS/FAIL/PARTIAL + STALE) + Zig dimension check + per-REQ report (D-87)

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 05-07-PLAN.md — phase-5-gate end-to-end against the showcase + e2e test + fresh-worktree gate + human-verify PM-query checkpoint (impl + 7-test e2e + smoke green; CWE-22-safe fixture; deal-sim install idempotent+PEP-668; E2503 accepted carryover — SC#5 human-verify APPROVED 2026-06-08)

**Wave 6** *(gap closure — re-runs the 05-07 SC#5 checkpoint)*

- [x] 05-08-PLAN.md — Gap closure: model-IR evidence + simulation input resolution. Shared AST `ModelValueIndex` resolves model paths + `REQ_*.attr` refs to values. Real showcase `deal check --verify` → REQ_BAT_001=PASS (was all-PARTIAL); `deal simulate --all` runs battery_thermal for real (Q=3125 W). cargo --workspace green; smoke now gates verdict correctness; spec submodule bumped `1fb43ca→dd3bb78` (sim-input values). Impl complete (commits 1e2dca7, 4e96bca, a0c5a5f) — SC#5 re-verify APPROVED 2026-06-08 (REQ_BAT_001=PASS confirmed).

### Phase 05.1: Resolve pending todos and integrate deal-stdlib functionality (INSERTED)

**Goal:** Resolve two carried-over compiler/stdlib todos before Phase 6: (TODO 2) close the deal-stdlib dimensional integration gap by adding CLI integration coverage for all four cross-file E25xx checks (E2500–E2503); and (TODO 1) reconcile the EBNF↔parser drift and convert silent-tolerance regressions into explicit accept-or-diagnostic (`;` canonical / `,` rejected, unknown verification key → hard error, keyword field key explicit accept, real `array_literal` AST node), restoring the Phase 1.5 strictness contract with the showcase corpus as oracle.
**Requirements**: Scope anchored to CONTEXT decisions D-01..D-11 (no formal REQ-* IDs assigned). D-01 `;`/`,` strictness; D-02 unknown-key hard error; D-03 keyword field key; D-04 array unification; D-05 honest-opaque gap EBNF; D-06 spec push + gitlink bump; D-07 calc/constraint `7ce59bc` NOT merged; D-08/D-09 E25xx CLI wiring + breadth; D-10 Step 0 gate; D-11 ideal forms for downstream calc/constraint.
**Depends on:** Phase 5
**Plans:** 4/4 plans complete

Plans:
**Wave 1** *(preconditions + independent TODO 2 — parallel, no file overlap)*

- [x] 05.1-01-PLAN.md — Step 0 empirical gate (D-10) + Wave A spec edits: honest-opaque GapBlock, AnnotationKey/optional-`;`/I6 citations, DESIGN-DECISIONS notes; push spec + bump parent gitlink (D-05/D-06/D-07)
- [x] 05.1-02-PLAN.md — TODO 2: add E2501/E2502/E2503 CLI integration tests to cli/tests/e2500_cli.rs (extend MINIMAL_STDLIB with `g` unit); wiring already complete per D-88 (D-08/D-09)

**Wave 2** *(parser strictness — depends on Step 0 gate)*

- [x] 05.1-03-PLAN.md — Wave B parser strictness: `;` explicit consume + `,` reject (E0123), unknown verification key → E0124, keyword field key tested accept; new malformed fixtures m21/m22 + pins (D-01/D-02/D-03/D-11)

**Wave 3** *(array unification — depends on Wave B; shared parser_deal.zig/fmt.zig)*

- [x] 05.1-04-PLAN.md — Wave C array unification: real `array_literal` AST node through ast/expr/ir/lowering/json/fmt; delete both duplicate parsers; byte-identical snapshot regeneration (D-04/D-11)

### Phase 05.2: Implement calc/constraint grammar (SD-21/22/23) in the Zig compiler (INSERTED)

**Goal:** Build first-class `calc def` (pure typed functions) and `constraint def` + `require` (named Boolean predicates) down the full hand-written Zig pipeline — lexer → parser/AST → fmt/sema → IR/lowering → SysML/KerML codegen — plus a `=>` return/precision contract that seeds the numeric-model precision vocabulary, plus editor surfaces (tree-sitter + vscode-deal + LSP). The two `examples/showcase/packages/analysis/{calcs,constraints}.deal` files are the parse / round-trip oracle.
**Requirements**: No formal REQ-* IDs. Scope anchored to CONTEXT decisions D-01..D-12 (mapped to internal REQ-05.2-D01..D09). D-01/D-02/D-03 spec merge + Wave 0 gate; D-04 Wave 5 editor (rich, Phase 6 seam); D-05 wave spine; D-06 generalized precision attach points; D-07 sig contextual; D-08 parse-and-carry precision slot; D-09 ± canonical + => reuses ARROW; D-10 E26xx sema checks; D-11 out/inout accepted, assignment body = parse error; D-12 IR calc_def shape aligned to deal-sim I/O→KerML.
**Depends on:** Phase 5 (and Phase 05.1)
**Plans:** 6/6 plans complete

Plans:
**Wave 0** *(BLOCKING spec-merge precondition — nothing else starts until green)*

- [x] 05.2-01-PLAN.md — Merge spec `7ce59bc` into spec origin/main (3 hand-conflicts + D-06 SD-23 amendment + D-03 fixtures), push, bump deal gitlink; phase-05.2-wave0-gate (D-01/D-02/D-03)

**Wave 1** *(lexer/keywords — after Wave 0)*

- [x] 05.2-02-PLAN.md — Add kw_calc/kw_return/kw_require + plus_minus (± 2-byte dispatch); sig stays IDENT; token snapshots stable (D-03/D-07/D-09)

**Wave 2** *(AST/parser — the array_literal analog wave; gated by parse oracle)*

- [x] 05.2-03-PLAN.md — 9 new NodeKinds + payloads; replace ConstraintDef=ElementDef alias (5 sites); parse functions incl. contextual sig + => return contract; calcs.deal + constraints.deal parse exit 0 (D-04/D-07/D-09/D-11)

**Wave 3** *(fmt/sema + editor — parallel, no file overlap)*

- [x] 05.2-04-PLAN.md — E2600..E2699 band; purity/dimensional/require-Boolean/ConstraintRef-cycle sema checks (reuse checkDimensionalExpr + checkSpecializationCycle); m25/m26/m27 corpus pins; +/- → ± verified (D-06/D-08/D-10)
- [x] 05.2-06-PLAN.md — Wave 5 editor (rich, D-04, depends Wave 2 only): tree-sitter-deal + vscode-deal + LSP calc_def semantic token/signature hover/go-to-def; Phase 6 LSP seam documented (D-04/D-07)

**Wave 4** *(IR/lowering/codegen + phase gate — after Wave 3 fmt/sema)*

- [x] 05.2-05-PLAN.md — IR calc_def + generic precision slot (D-08) shaped to deal-sim I/O→KerML (D-12); schema.json; emit_calc_def (Function) + emit_constraint_def (Predicate); golden fixtures; phase-05.2-gate(-fresh) (D-06/D-08/D-12)

### Phase 6: Application

**Goal**: A defense/aerospace systems engineer can open the desktop editor, author a system model in DEAL using stdlib components, see auto-generated diagrams, run simulations, check verification status, export to SysML v2 JSON / ReqIF XML / generated docs, and import from existing SysML v2 and ReqIF sources without manual fixup — replacing Cameo/Sparx for the authoring workflow.
**Depends on**: Phase 5
**Requirements**: REQ-phase-6-application, REQ-phase-6-1-desktop-editor, REQ-phase-6-2-import-pipelines, REQ-phase-6-3-doc-generation, REQ-phase-6-4-stdlib-expansion, REQ-phase-6-gate
**Success Criteria** (what must be TRUE):

  1. A Tauri + Next.js desktop binary opens a DEAL project, presents text-first editing in CodeMirror 6 with LSP intelligence, a model browser tree, read-only auto-generated diagrams (block definition, internal block, sequence), a traceability matrix, and a verification dashboard.
  2. `deal import --from sysml-v2 model.json` converts existing SysML v2 JSON into idiomatic `.deal`/`.dealx` files (not a syntactic transliteration), and `deal import --from reqif requirements.xml` produces idiomatic requirement definitions.
  3. `deal build --target docs` generates template-driven HTML and PDF documentation (system descriptions, ICDs, verification status reports) from the model.
  4. `deal-stdlib` expansion ships protocols (MIL-STD-1553, ARINC 429, SpaceWire), standards (DO-178C, DO-254 assurance levels; MIL-STD-810H environmental test method references), and patterns (TMR, dual-redundant, watchdog).
  5. The end-to-end loop holds: an engineer authors → simulates → verifies → exports / imports across SysML v2, ReqIF, and generated docs from the desktop editor.

**Plans**: TBD
**UI hint**: yes

## Progress

**Execution Order:**
Phases execute in numeric order: 0 → 1 → 2 → 3 → 4 → 5 → 6

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 0. Foundation | n/a | Complete | 2026-05-18 |
| 1. Zig Compiler Core | 6/6 | Complete   | 2026-05-20 |
| 1.5. Parser Strictness & Test-Gap Resolution | 5/5 | Complete (re-verified after false-GREEN remediation via Plan 05 + phase-1.5-gate-fresh) | 2026-05-21 |
| 2. Prove the Pipeline | 6/6 | Complete   | 2026-05-22 |
| 3. Editor Intelligence | 5/7 | In Progress|  |
| 4. Ecosystem | 9/9 | Complete   | 2026-06-06 |
| 5. Simulation Integration | 8/8 | Complete   | 2026-06-08 |
| 05.1. Resolve Pending Todos + Integrate deal-stdlib | 4/4 | Complete    | 2026-06-09 |
| 05.2. calc/constraint grammar (SD-21/22/23) | 6/6 | Complete    | 2026-06-10 |
| 6. Application | 0/TBD | Not started | - |
