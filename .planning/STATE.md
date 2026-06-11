---
gsd_state_version: 1.0
milestone: v2.1.0
milestone_name: milestone
status: ready_to_plan
stopped_at: Phase 05.2 complete (6/6) — ready to discuss Phase 06
last_updated: 2026-06-10T22:49:15.880Z
last_activity: 2026-06-10
progress:
  total_phases: 10
  completed_phases: 8
  total_plans: 51
  completed_plans: 51
  percent: 80
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-05-19)

**Core value:** A defense/aerospace systems engineer can author a real subsystem in DEAL end-to-end, run it through codegen, and import the result into Cameo / DOORS / Modelica without manual fixup.
**Current focus:** Phase 06 — editor first platform

## Current Position

Phase: 06
Plan: Not started
Status: Ready to plan
Last activity: 2026-06-10

Progress: [██████████] 100%
Phase 1 plan progress: 6/6 ✓
Phase 1.5 plan progress: 4/4 ✓ (01.5-01-test-assertions-and-property ✓; 01.5-02-parser-strictness ✓; 01.5-03-malformed-pinning ✓; 01.5-04-determinism-and-gate ✓)
Phase 2 plan progress: 0/TBD
Phase 05.1 plan progress: 3/4 ✓ (05.1-01-step0-gate-wave-a-spec ✓; 05.1-02-e2501-e2502-e2503-cli-tests ✓; 05.1-03-parser-strictness-d01-d02-d03 ✓)

## Performance Metrics

**Velocity:**

- Total plans completed: 35 (01-01-foundation, 01-02-lexer, 01-03-parser-deal, 01-04-parser-dealx, 01-05-error-recovery)
- Average duration: ~1 session
- Total execution time: ~5 sessions

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 0. Foundation | n/a (spec) | - | - |
| 1. Zig Compiler Core | 5/6 | 5 sessions | 1 session |
| 01 | 6 | - | - |
| 01.5 | 5 | - | - |
| 04 | 9 | - | - |
| 05.1 | 4 | - | - |
| 05.2 | 6 | - | - |

**Recent Trend:**

- Last 5 plans: 01-02-lexer, 01-03-parser-deal (87 productions + 15 .deal snapshots), 01-04-parser-dealx (43 productions + 4 .dealx snapshots, D-07 tag stack, D-08 inline {...}, D-09 first-class comp_connect), 01-05-error-recovery (D-17 three-tier sync, 50-file malformed corpus, diag.json_roundtrip)
- Trend: green; Phase 1 success criteria #1-#4 all met (zero `.unknown` tokens, 19 stable AST snapshots, ≥50 malformed survives)

*Updated after each plan completion*
| Phase 01-zig-compiler-core P03 | 4h | 3 tasks | 22 files |
| Phase 01-zig-compiler-core P04 | 4h | 4 tasks | 13 files |
| Phase 01-zig-compiler-core P05 | 2h | 3 tasks | 56 created + 6 modified |
| Phase 01.5 P01 (test-assertions-and-property) | 1h | 2 tasks | 1 created + 4 modified |
| Phase 01.5 P02 (parser-strictness m08/m18/m19) | 25m | 3 tasks | 1 created + 6 modified |
| Phase 01.5 P03 (malformed-pinning m01..m20) | 12m | 2 tasks | 1 created + 1 modified |
| Phase 01.5 P04 (determinism-and-gate) | 30m | 2 tasks | 1 created + 3 modified |
| Phase 03 P01 | 19m | 2 tasks | 28 files |
| Phase 03 P02 | 45min | 2 tasks | 22 files |
| Phase 03 P03 | 19 | 3 tasks | 18 files |
| Phase 03 P04 | 1h 50m | 4 tasks | 8 created + 6 modified |
| Phase 03 P05 | 1h | 2 tasks | 12 created + 2 modified files |
| Phase 03 P06 | 1h 15m | 2 tasks | 6 created + 3 modified files (multi-repo: deal/ + vscode-deal/) |
| Phase 04-ecosystem P01 | 23m | 3 tasks | 15 files |
| Phase 04-ecosystem P02 | 47m | 2 tasks | 8 files |
| Phase 04-ecosystem P04 | 3 sessions | 2 tasks | 9 files |
| Phase 04-ecosystem P07 | 1h | 3 tasks | 25 files |
| Phase 04-ecosystem P06 | ~2h | 3 tasks | 14 files |
| Phase 04-ecosystem P08 | 80 | 3 tasks | 13 files |
| Phase 05-simulation-integration P01 | ~1.5h | 3 tasks | 23 files |
| Phase 05-simulation-integration P02 | 17m | 2 tasks | 13 files |
| Phase 05-simulation-integration P03 | 120 | 2 tasks | 6 files |
| Phase 05-simulation-integration P04 | 32 | 2 tasks | 7 files |
| Phase 05.1 P01 | 1.5h | 3 tasks | 5 files |
| Phase 05.1 P02 | 7m | 1 tasks | 1 files |
| Phase 05.1 P03 | 12m | 2 tasks | 3 created + 4 modified |
| Phase 05.2 P01 | 45min | 2 tasks | 3 files |
| Phase 05.2 P03 | 240min | 3 tasks | 16 files |
| Phase 05.2-implement-calc-constraint-grammar-sd-21-22-23-in-the-zig-com P04 | 90 | 2 tasks | 9 files |
| Phase 05.2 P06 | 120 | 3 tasks | 6 files |
| Phase 05.2 P05 | 90m | 3 tasks | 14 files |

## Accumulated Context

### Roadmap Evolution

- Phase 5.1 inserted after Phase 5: Resolve pending todos and integrate deal-stdlib functionality (URGENT)
- Phase 05.2 inserted after Phase 5: Implement calc/constraint grammar (SD-21/22/23) in the Zig compiler — implements pending todo 2026-06-08-implement-calc-constraint-grammar-in-zig.md (URGENT)

### Decisions

Decisions are logged in PROJECT.md `<decisions>` block (64 LOCKED + 1 CAPTURED) and Key Decisions table.

Recent decisions affecting current work:

- **[Phase 05.2 P03]: ReturnStatement reuses require_statement NodeKind**: same payload shape (condition: *Node, precision: ?*Node) — avoids a 10th new kind; kept consistent with plan D-11 and constraint body semantics.
- **[Phase 05.2 P03]: D-07 honored (contextual sig)**: `sig` NOT added to keywords.zig; recognized only inside parseReturnContract via std.mem.eql. Global keyword table stays stable; no parser state machine changes needed.
- **[Phase 05.2 P03]: D-06 generalized precision**: single `precision: ?*Node = null` slot on both ElementUsage and RequireStatement; emitted in JSON only when non-null (preserves snapshot byte-stability across the 15-file corpus).
- **[Phase 05.2 P03]: Import path segments accept keywords**: parseImportDecl uses `ident or isKeyword(tag)` pattern matching parseQualifiedNameSegments — keyword-named packages (analysis, action, etc.) parse correctly.
- **[Phase 05.1 P03]: D-01 enforced**: `;` consumed explicitly in annotation-body loop; `,` separator emits E0123; trailing comma-tolerate consumer removed (~:1923); m21 fixture uses `@trace:<<satisfies>>` category annotation form (bare `@config {` does not route through parseAnnotationBody).
- **[Phase 05.1 P03]: D-02 enforced**: verification-key switch `else` arm replaced with E0124 emit-then-advance; m22 corpus pin green.
- **[Phase 05.1 P03]: D-03 tested**: isKeyword accept at :1908 was already correct; parser_deal_strictness.zig test (d) proves `from:` keyword key parses with zero diagnostics and `"from"` field present in AST JSON.
- **[Phase 05.1 P01]: D-01 confirmed standing**: `,` is never used as an annotation-body field separator in any showcase file; zero category-(c) occurrences; Waves B/C proceed without re-planning.
- **[Phase 05.1 P01]: SD-15a locked**: `;` is the canonical annotation field terminator; `,` in annotation context will be rejected with E0123 (Wave B / Plan 05.1-03).
- **[Phase 05.1 P01]: SD-15b locked**: Keyword tokens are valid annotation field keys; `AnnotationKey ::= IDENT | _Keyword` now in spec.
- **[Phase 05.1 P01]: D-05 applied**: GapBlock in `dealx.ebnf` replaced with honest-opaque `_OpaqueBody`; structured gap parsing deferred to Wave E.
- **[Phase 05.1 P01]: D-07 honored**: spec Wave-A branch from `dd3bb78` only; calc/constraint recall `7ce59bc` excluded and deferred to Phase 6.
- **[Phase 05.1 P01]: Inherited E2503 carryover**: `sema.dimensional.regression_pins` fails in fresh-worktree context (stdlib sibling absent); pre-existing Phase-4 issue; NOT caused by Wave-A spec edits; deferred to Phase 6.
- **[Phase 04 P07]: Shiki grammar aliases**: grammar `name` field is `DEAL`/`DEAL Composition` (title-case); `aliases: ['deal']`/`['dealx']` added in `astro.config.mjs` so fences resolve correctly with lowercase identifiers.
- **[Phase 04 P07]: NC-1 amendment**: `deal-lang.org` is the deployed domain and sibling directory; DNS CNAME is user-side manual step per ADR-phase-4-nc1-domain-amendment.md.
- **[Phase 04 P07]: Astro 6 content config**: requires `src/content.config.ts` with `docsLoader()` + `docsSchema()` — not optional; Starlight scaffold template includes it but manual scaffold does not list it.
- **Phase ordering authority**: PRD (`DEAL-LANG-ROADMAP.html`) wins over ADR §Implementation Staging — the ADR explicitly cedes ordering authority (INGEST-CONFLICTS INFO #4).
- **Tech stack split**: Zig for compiler core (lexer, parser, error recovery, C ABI); Rust for frontend tooling (CLI, formatter integration, SysML v2 / ReqIF codegen, LSP server).
- **SD-6 status**: Treated as CAPTURED (not LOCKED) — relationship-category enumeration is intentionally partial.
- **Phase 0 complete**: All foundation artifacts verified (65 decisions, 3 EBNF grammars at 758/1679/897 lines, 19-file showcase parseable, reference material acquired) per PRD §1.
- **Plan 01.5-02 m08 disposition**: Wires the EXISTING `Codes.e_unterminated_comment` (E0003 declared in Phase 1 at src/diagnostics.zig:64, never emitted) rather than allocating a new lexer code. No E0006 introduced.
- **Plan 01.5-02 m18 disposition**: Allocates NEW `Codes.e_dangling_dot` (E0121) emitted in src/expr.zig parseExpression dot branch on non-IDENT-after-dot.
- **Plan 01.5-02 m19 disposition**: Allocates NEW `Codes.e_empty_arg_comma` (E0122) emitted in src/expr.zig parseArgList on leading, double, AND trailing commas (grammar §_ArgumentList disallows all three).
- **Plan 01.5-02 lexer→parser drain mechanism**: One-shot `Lexer.unterminated_block_comment: ?ast.Span` field drained by `parser_deal.drainLexerErrors` at parseFile-end. Both parser_deal and parser_dealx call the same helper (no duplication).
- **Plan 01.5-02 trailing-comma policy**: `f(a,)` is an error (E0122). Grammar §_ArgumentList (deal.ebnf line 1680) does not permit trailing commas. If future grammar revision adds trailing-comma permission, remove that branch.
- **Plan 01.5-03 m01 disposition**: ADR-phase-1.5-m01-sd10-optional-semicolon.md — `m01_missing_semicolon.deal` parses cleanly under SD-10's optional-semicolon rule. The hand-curated fixture's name is a historical artefact (the fixture predates SD-10 finalisation). Pinned in `recovery_corpus.zig` as `expected_code = null` with `adr_basename` pointing at the ADR.
- **Plan 01.5-03 empirical-truth pinning**: 8 of 19 impl pins (m02/m03/m04/m06/m07/m14/m17 → E0100; m05/m09/m15 → E0101) diverge from the filename-suggested recovery-tier or lexer-tier code. The pins record the OBSERVED first-code from the parser; the recovery-tier codes typically fire later as secondary. Each divergence is documented inline next to its pin.
- **Plan 01.5-03 hand-curated zero-diag set is now exactly {m01}**: Plan 02 closed m08/m18/m19; m01 is the only hand-curated zero-diag file and is documented via ADR. The remaining 9 zero-diag files in the soft-60% pool are all generator-produced (5 gen_drop_byte_* + 4 gen_swap_pair_* + 1 gen_inject_garbage_*).
- **Plan 01.5-04 determinism test PUBLIC C ABI lock**: The test uses `lib.deal_parse` / `lib.deal_ast_json` / `lib.deal_free` (@exported symbols visible in `nm -gU libdeal.a`), NOT the test-only internal helper. Internal-helper byte-equality could pass while the public ABI's JSON cache produces different bytes — that would be a determinism bug the test must catch. Grep gate `grep -c 'deal_parse_internal' tests/unit/determinism_parse_twice.zig` returns 0.
- **Plan 01.5-04 gpa.dupe-before-free lifetime pattern**: The first parse's AST JSON bytes are copied into a gpa-owned buffer BEFORE `deal_free(handle_a)` invalidates the arena pointer, so both byte strings live simultaneously for the comparison. Same for diagnostics JSON.
- **Plan 01.5-04 two-tier exit gate**: Convenience single-command `zig build phase-1.5-gate` runs the same suite once; CI-authoritative three-step locked command is `zig build phase-1-gate && zig build test -Dtest-filter=determinism && zig build test -Dtest-filter=property`. Both quoted verbatim in build.zig NOTE comment so future maintainers cannot accidentally weaken the CI gate.
- **Phase 1.5 EXIT GATE GREEN**: All 7 criteria pass — phase-1-gate, determinism test, property test, no vacuous len>=0 assertions, no snapshot drift, C ABI symbol count = 6, Surprise #8 closed via Phase 1.5 Resolution section in 01-LEARNINGS.md (9 Resolved + 1 Deferred via ADR).
- [Phase 03]: Phase 3 Plan 01: TextMate scopes must match D-41 §8 table verbatim; COLOR-CATEGORIES.md is the canonical source-of-truth shared with Plan 02 tree-sitter highlights.scm
- [Phase 03]: Phase 3 Plan 01: bumped mocha 10.4.0 → 11.7.6 because Node 26 ESM enforcement breaks the yargs CJS shim in mocha 10
- [Phase 03]: Phase 3 Plan 01: snapshot test harness uses precomputed frozen Record<fixture, absPath> to avoid CWE-22 taint-sink lint failures while keeping parameterized test bodies
- [Phase ?]: Tree-sitter grammar pinned to lean 678-LOC scope per RESEARCH §9; opaque-block tolerance for non-highlighted forms; Zig parser owns AST
- [Phase ?]: tree-sitter-cli pinned exact 0.25.10 (T-3-03 supply-chain mitigation)
- [Phase ?]: Linguist submission DEFERRED to Phase 4 per RESEARCH Open Q3 — package structurally ready but PR out of scope
- [Phase ?]: Honoured D-49 architectural split: deal-ffi + deal-lsp live in the deal/ Rust workspace; Zig core remains single-file-scoped.
- [Phase ?]: tower-lsp pinned to exact 0.20.0 (Plan 03-03 Issue 1) — verified available via cargo search before commit; escalation criterion: yanked → plan revision.
- [Phase ?]: Phase 2 D-32 envelope is flat JSON array with span:[start,end] byte offsets; deal-lsp's diagnostics.rs translates to LSP line/character via ropey (UTF-16 multi-byte safe).
- [Phase 03]: Phase 3 Plan 04: in-memory workspace index is authoritative (D-46); disk .deal/index.json is read-only from the LSP (D-48 — never written back); eager_parse spawns on `initialized` as a background tokio task so the handler returns immediately per LSP spec (D-47).
- [Phase 03]: Phase 3 Plan 04: PS-5 alias resolution lives entirely in Rust (Index::resolve_with_alias); Zig core untouched per D-49 architectural split.
- [Phase 03]: Phase 3 Plan 04: deal_index_json envelope is `{v, deal_version, elements: {fqpn -> {id, kind, source_file, span: [start_byte, end_byte]}}, imports_graph}` — schema differs from PLAN.md's hypothetical `{symbols: [...]}` shape; live-probed via /tmp/probe.
- [Phase 03]: Phase 3 Plan 04: semantic-tokens golden is byte-stable and committed; the dedicated `deal-lsp-gen-golden` cargo binary is the ONLY writer (Issue 6 resolution — no inline UPDATE_GOLDEN escape hatch in the test source).
- [Phase 03]: Phase 3 Plan 04: AST root envelope is `{v, mode, filename, root:{span:...}}` — hover + definition auto-descend through `root` when the outer node lacks a `span` field.
- [Phase 03]: Phase 3 Plan 04: `<<specializes>>` referents in the Phase 1 AST appear as `structural_relationship` nodes with `target_segments: ["..."]` — no inner identifier — and require a dedicated extractor in definition.rs.
- [Phase 03]: Phase 3 Plan 06: MANUAL-PUBLISH VARIANT — Marketplace publish job OMITTED from active release.yml because required-reviewer Environments are unavailable on FREE+private repos. Lives as commented-out stub at bottom of workflow; first v0.3.0 publish done manually via `gh release download + shasum -a 256 -c SHA256SUMS + npx vsce publish --pat $VSCE_PAT`. T-3-02 disposition documented as `mitigate-via-manual-publish-runbook` instead of `mitigate-via-CI-gate`.
- [Phase 03]: Phase 3 Plan 06: bootstrap.ts uses Injectables seam (HttpClient + tripleOverride + manifestOverride + confirmInstallOverride + extractTarball) for tests — cleaner than sinon stubs because the seam is part of the production API surface (documented + type-checked). Tests inject all 5 hooks to exercise URL construction + SHA-256 mismatch path without network or real tar.
- [Phase 03]: Phase 3 Plan 06: CWE-22 taint-free path construction extended from binary.ts to bootstrap.ts — all production filesystem paths built via vscode.Uri.joinPath from VS Code-supplied URIs + compile-time string literals (BINARY_FILENAME frozen Record keyed by hardcoded triple table); semgrep CWE-22 detection defended on first pass.
- [Phase 03]: Phase 3 Plan 06: patch-bootstrap-sha.js is dependency-free Node stdlib so it can run before npm ci would install bootstrap.ts's runtime deps; post-replacement validation tolerates exactly 1 placeholder occurrence (the doc-comment that describes the substitution pattern) — anything else means a SHA256_MANIFEST entry was missed.
- [Phase 04]: Phase 4 Plan 02: git2 vendored-libgit2 feature for deal install git dep resolution — avoids system-libgit2 CI fragility (RESEARCH A4 recommendation).
- [Phase 04]: Phase 4 Plan 02: lib.rs [lib] target alongside [[bin]] in cli/Cargo.toml so integration tests call resolver::resolve_all directly via deal::resolver.
- [Phase 04]: Phase 4 Plan 02: deal init D-69 starter model places satisfy block in model/starter.dealx inside [<traceability>] — grammar requires satisfy inside traceability (not inside .deal definitions).
- [Phase 04]: Phase 4 Plan 02: DEFAULT_STDLIB_TAG = "v0.1.0" in main.rs as single source of truth for deal-std git dep tag; Plan 04-03 updates to real release tag.
- [Phase 04]: Phase 4 Plan 02: E2402 guard in run_check silently skips if deal.toml cannot be parsed — avoids breaking check for projects without manifests or with legacy deal.toml formats.
- [Phase 04]: Phase 4 Plan 03: attr-def-body (Option A) locked as grammar-legal encoding for dimensional metadata — 7 si_M..si_J Integer member defaults per dimension def, si_factor Real per unit def with <<specializes>> structural link; array-literal annotation (Option B) disqualified (E0100).
- [Phase 04]: Phase 4 Plan 03: DEFAULT_STDLIB_TAG = "v0.4.0" — locksteps with Phase 4 toolchain (D-52 pairing). Plan 04-02 placeholder "v0.1.0" should be updated to "v0.4.0" when deal-stdlib is tagged.
- [Phase 04]: Phase 4 Plan 03: percent declared under Mass (si_factor=0.01) pending a future Dimensionless dimension; in_unit used for inch to avoid DEAL "in" keyword collision.
- [Phase ?]: XML IDs cannot contain dots; deterministic and reversible
- [Phase ?]: No mature offline Rust XSD 1.0 validator; structural check (well-formed + OMG namespace + required elements + non-empty IDENTIFIERs) is the automated hard gate per D-58/D-60
- [Phase ?]: Python reqif v0.0.48 round-trip passes as import smoke; DOORS trial unavailable; accepted per D-58 fallback clause
- [Phase ?]: Top-level @trace on part def excluded per D-59/D-25; @trace inside requirement_def body required for SPEC-RELATION output; no emitter bug
- [Phase ?]: REQ-IF-HEADER CREATION-TIME uses compile-time constant (2024-01-01T00:00:00Z) not runtime now(); ensures byte-identical .reqif output across CI runs
- [Phase ?]: deal parse exits 1 for E2000 without package declaration — doc snippets need package + import context
- [Phase ?]: star-glob and relative-parent PS-4 import forms not yet implemented in parser — documented as prose without fenced blocks
- [Phase ?]: mapfile used in check-snippets.sh to capture snippet paths without pipe subshell — avoids FAIL_COUNT reset bug
- [Phase 05-01]: deal_check_with_stdlib returns DealHandle* not bool — caller must call deal_free() to avoid arena memory leak (matches existing handle pattern)
- [Phase 05-01]: stdlib_ir_ptr/len accepts DEAL source bytes parsed into external SymbolTable inside deal_check_with_stdlib — cleaner than pre-parsed JSON
- [Phase 05-01]: spec/sims/v0/ schemas use JSON Schema draft-2020-12 with alphabetical key order (D-18); deal_sim_protocol const field required in all three artifact types
- [Phase 05-01]: Wave 0 test stubs use #[ignore] in Rust and @unittest.skip in Python — compile and collect but never run until target plan implements
- [Phase ?]: deal-sim initialized as own git repo
- [Phase ?]: setuptools.build_meta is correct PEP 517 backend for setuptools 82.0.1
- [Phase ?]: output.json wraps run() scalars in {value,unit} per spec/sims/v0/output.schema.json
- [Phase 05-03]: Dual-target CliError defined separately in lib.rs (integration test access via deal::CliError) and main.rs (binary root crate:: namespace) — identical by convention
- [Phase 05-03]: dispatch_python tries python3 first then python as fallback — maximizes macOS/Linux CI compatibility without requiring specific symlink
- [Phase 05-03]: deal-sim Shape A/B input detection: if top-level 'inputs' key is a dict treat as spec/sims/v0/ envelope and unwrap {value,unit} scalars; otherwise bare flat dict (backwards-compatible for testing)
- [Phase ?]: D-73: Zig sims run in-process AND serialize evidence (DealSimulation comptime wrapper with @setFloatMode tier dispatch)
- [Phase ?]: D-72 ADR: MATLAB subprocess (-batch) supersedes ROADMAP SC#2 Engine API wording for Phase 5
- [Phase ?]: T-05-10 fix: skip deal_index_json on deal_check_with_stdlib handles (stdlib_arena use-after-free guard in Rust caller)
- [Phase ?]: Spec merge fce94c7 on origin/main; deal gitlink pinned; oracle files present
- [Phase ?]: Wave-0 gate uses targeted snapshot regression instead of dependOn(gate_5_step) per plan OR-clause; phase-5-gate sibling-repo limitation documented
- [Phase ?]: ! target/release/deal parse calcs.deal confirms oracle fails pre-Wave-1; absent binary in fresh worktree also satisfies intent
- [Phase ?]: ConstraintRefs resolved via import accepted without E2611 check — single-file sema boundary
- [Phase ?]: sema-only checks (E2600/E2610) have no parser diagnostic; fixtures include E2601; pins document divergence inline
- [Phase ?]: D-06/D-08 precision is parse-and-carry only; enforcement deferred to Lane A numeric model
- [Phase ?]: D-07 TextMate sig: match via lookahead to avoid coloring bare sig identifier
- [Phase ?]: Phase 6 LSP seam: Wave 5 adds calc/constraint arms only; Phase 6 extends same arms
- [Phase ?]: go-to-def for calc/constraint: existing Index covers it; no new code in definition.rs

### Pending Todos

[From .planning/todos/pending/ — ideas captured during sessions]

None yet.

### Blockers/Concerns

[Issues that affect future work]

- **Phase 4 prerequisite**: ReqIF 1.2 XSD (and DOORS-compatible sample export, optional Jama sample) must be acquired and placed under `spec/references/omg-reqif/` before REQ-phase-4-2-reqif-codegen can be implemented. Without the XSD, the ReqIF emitter has no validation target.
- **Documentation hygiene**: `DESIGN-DECISIONS.md` currently lives at `spec/grammar/tmp-references/` — name suggests transient. Promote to a permanent location before public release (Phase 4 gate).
- **README stat drift**: `grammar/README.md` cites `lexical.ebnf` as 370 lines; current file is 758 lines (user completed grammar after README written). Update README during Phase 4 docs work.

## Deferred Items

Items acknowledged and carried forward. Detailed list in REQUIREMENTS.md "v2 Requirements" (PRD §11 decision points + ADR-deferred language surface decisions):

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| Distribution | WASM packaging of Zig core | Evaluate at Phase 2 gate | PRD §11 |
| Distribution | First public release timing (Phase 4 vs 5) | Evaluate at Phase 4 gate | PRD §11 |
| Ecosystem | Centralized package registry | Evaluate at Phase 4 gate | PRD §11 |
| Tooling | TextMate vs tree-sitter in VS Code | Evaluate at Phase 3 gate | PRD §11 |
| Tooling | Desktop editor priority (Tauri) | Evaluate at Phase 5 gate | PRD §11 |
| Language surface | Expression / path / constraint syntax details | Locked when showcase demands | ADR |
| Language surface | Full relationship-category enumeration (SD-6) | Locked when showcase demands | ADR |
| Language surface | MQL query language design | Future milestone | ADR |
| Language surface | Codegen backend public API | Future milestone | ADR |
| Language surface | Default visibility when no wrapper (SD-9) | Locked when showcase demands | ADR |
| Language surface | `@document:` category + `[<document>]` block | Phase 6 timing | ADR |
| Tooling | Textual SysML v2 emitter (`deal build --target sysml-v2-text` → `.sysml` files) | Schedule for Phase 3 (pair with LSP — shares grammar surface) | Phase 2 viewer-smoke checkpoint (02-VIEWER-SMOKE.md, 2026-05-22) — Eclipse SysON consumes textual SysML v2, not OMG JSON interchange |
| Language surface | calc/constraint defs (SD-21/22/23) in Zig compiler + **re-pin spec submodule** to merge of `7ce59bc` (calc/constraint) and `dd3bb78` (sims/v0 + 05-08 showcase input values) | **Phase 6** — see todo `2026-06-08-implement-calc-constraint-grammar-in-zig.md` §Spec submodule recall; user bump `d554d74` superseded for Phase 5, re-apply in Phase 6 | Phase 5 exec (2026-06-08) — current Zig compiler can't parse calc/constraint showcase; gitlink at `dd3bb78` (05-08: `1fb43ca` sims/v0 + literal sim-input values) so the sim gate completes |

## Session Continuity

Last session: 2026-06-10T22:22:02.587Z
Stopped at: Completed 05.2-06-PLAN.md
Resume file: None

### Plan 05-07 decisions (pending human-verify close-out)

- E2E showcase fixture uses a frozen static list of compile-time string-literal paths (CWE-22 / T-05-06 taint-free path construction) instead of recursive read_dir — semgrep p/rust = 0 findings; CWE-22 satisfied without --no-verify bypass.
- phase-5-gate deal-sim step asserts importability (short-circuit when deal_sim imports) with PEP-668 --break-system-packages fallback, not a forced fresh install per run (T-05-19).
- E2503 sema.dimensional.regression_pins = accepted pre-existing Phase-4 carryover (fails on bare `zig build test -Dtest-filter=regression_pins` with no worktree; independent of Plan 07) — NOT fixed in Phase 5; documented in deferred-items.md.
