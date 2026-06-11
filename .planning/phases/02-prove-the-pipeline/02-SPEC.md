# Phase 2: Prove the Pipeline — Specification

**Created:** 2026-05-21
**Ambiguity score:** 0.08 (gate: ≤ 0.20)
**Requirements:** 9 locked

## Goal

A defense/aerospace systems engineer can author the 19-file showcase model, run `deal check` (which validates names, types, multiplicity, `<<specializes>>` compatibility, `@trace` references, and imports — all blocking errors), run `deal fmt` (which round-trips every showcase file to byte-identical AST with comments preserved), and run `deal build --target sysml-v2` to produce SysML v2 JSON that (a) validates against the bundled OMG SysML v2 + KerML schemas, (b) matches 5–8 hand-written golden fixtures byte-for-byte, and (c) successfully imports into at least one real SysML v2 viewer — proving every public contract surface (DEAL IR v0, `--json` diagnostic output, E2xxx semantic error codes) before Phases 3–6 build on top of them. Internal-demo ready.

## Background

Phase 1.5 closed 2026-05-21 with re-verified GREEN gate after Plan 05 fixed the false-GREEN tests/showcase symlink incident:

- **Zig core** (`src/`, 12 files): lexer + parser_deal + parser_dealx + ast + diagnostics + C ABI (6 exported symbols: `deal_parse`, `deal_ast_json`, `deal_free`, etc.).
- **19 showcase files** (15 `.deal` + 4 `.dealx` under `tests/showcase/` → `../spec/examples/showcase` submodule) all parse to stable AST JSON snapshots.
- **Test infra**: `phase-1.5-gate` + `phase-1.5-gate-fresh` (fresh-worktree gate locked by ADR-phase-1.5-fresh-worktree-verification.md, binds Phases 2..6).
- **Parser strictness**: m08/m18/m19 emit defined codes; m01..m20 corpus pinned per-file; determinism.parse_twice asserts byte-identical JSON across two parses via PUBLIC C ABI.

What does NOT exist today (gap to Phase 2 target):
- No semantic analyzer — no name resolution, no type checking, no multiplicity enforcement, no `<<specializes>>` compatibility check, no `@trace` reference validation, no import resolution, no `.deal/index.json` generation.
- No DEAL IR — the AST is the only representation; nothing yet abstracts away syntactic sugar or carries resolved references / relationship graph / agent metadata.
- No SysML v2 emitter — `spec/references/omg-sysml-v2/SysML.json` (126K lines) and `spec/references/omg-kerml-v1/KerML.json` are present but no code reads them.
- No offline JSON-Schema validator harness with absolute `$id`/`$ref` resolver.
- No golden fixtures — no `.deal` → expected `.json` pairs.
- No `deal fmt` — `src/` has no pretty-printer module; the parser currently DROPS all `//` and `/** */` comments (Phase 2 must change this to attach comments to AST nodes for round-trip fidelity).
- No Rust CLI — `cli/` directory has README only, no `src/`, no `Cargo.toml`. C ABI exposes 6 symbols; CLI is the first real consumer outside the FFI test harness.

This phase is the first end-to-end pipeline (`.deal` source → semantic-checked → IR → SysML v2 JSON → real viewer) and locks three downstream contract surfaces that Phases 3–6 will depend on.

## Requirements

1. **Semantic analyzer — 6 blocking checks + index**: `deal check` runs all six semantic checks; any failure emits a structured ERROR diagnostic and produces a non-zero exit code; on success, writes `.deal/index.json`.
   - Current: No semantic analyzer exists; the AST is consumed only by snapshot tests and the C ABI's `deal_ast_json`.
   - Target: Six blocking checks all implemented and wired into `deal check`: (1) name resolution across files in a package, (2) type checking for attributes/ports/parts, (3) multiplicity enforcement on usages, (4) `<<specializes>>` type compatibility (target exists AND is type-compatible with subject), (5) `@trace` reference validation (target ID resolves to a requirement definition), (6) import resolution (package-qualified imports + element references). On success, `.deal/index.json` is written with the resolved symbol table.
   - Acceptance: Running `deal check` on the unmodified showcase exits 0 and writes a valid `.deal/index.json`; introducing each of 6 hand-crafted regression fixtures (one per check) each causes `deal check` to exit non-zero with the corresponding E2xxx diagnostic code.

2. **DEAL IR v0 — dual-document specification**: IR v0 is specified BOTH as an architectural decision record (the why) AND as a normative schema (the what).
   - Current: No IR specification exists; no `spec/ir/` directory.
   - Target: `ADR-deal-ir-v0.md` exists in `.planning/decisions/` (or equivalent path consistent with prior ADRs) capturing the design rationale, alternatives rejected, and links to the normative spec. `spec/ir/v0/` exists in the `spec/` submodule with: (a) a JSON Schema document for the IR node kinds, (b) a Markdown reference explaining ID strategy, span carry-over, relationship graph contract, metadata envelope, diagnostic attachment points, and the backend traversal/query API contract.
   - Acceptance: Both files exist and are referenced from `02-SUMMARY.md`; the JSON Schema is loadable by a standard JSON-Schema validator; an automated test asserts the IR-lowering output for every showcase file validates against the schema.

3. **AST → IR lowering pass (Zig)**: A Zig module implements the AST → DEAL IR v0 lowering with conformance tests against the 19-file showcase plus 5–8 golden fixtures.
   - Current: No lowering pass; no IR data structures.
   - Target: A Zig module (e.g., `src/ir.zig` + `src/lowering.zig`) defines the IR types, lowers AST → IR for both `.deal` and `.dealx` inputs, resolves references (via the semantic analyzer's symbol table), strips syntactic sugar, and produces a graph of typed elements carrying resolved references, source spans, agent metadata (`@confidence`, `@rationale`, `@assumes`, `@concerns`), simulation bindings, and traceability links. A backend traversal/query API is exposed.
   - Acceptance: For all 19 showcase files, AST → IR conformance test produces an IR that (a) validates against the spec/ir/v0/ JSON Schema, (b) preserves every span back to a valid source location, (c) resolves every cross-reference to a non-null target. Two byte-equal lowering passes of the same source produce byte-identical IR (determinism).

4. **SysML v2 JSON codegen**: IR → SysML v2 JSON emitter targets the bundled OMG schemas.
   - Current: No emitter; OMG schemas present but unread.
   - Target: A code path (Rust or Zig — discuss-phase decides) walks the IR via the traversal API and emits SysML v2 JSON. `part def` → `PartDefinition`, `port` → `PortUsage`, `<<specializes>>` → `Specialization` in `ownedRelationship`, `attribute usage` → `AttributeUsage`, `requirement def` → `RequirementDefinition`, `@trace` → traceability `Dependency`, `package` → `Package`. Output uses absolute OMG `$id`/`$ref` URLs with the local schema registry resolving them.
   - Acceptance: `deal build --target sysml-v2 tests/showcase/` exits 0 and writes one JSON file per package (or one consolidated file — discuss-phase decides) under `tests/showcase/build/sysml-v2/`. The output passes offline schema validation (REQ #5). The output imports successfully into at least one real SysML v2 viewer (REQ #9 acceptance criterion).

5. **Offline JSON-Schema validator harness**: Local schema bundle + `$ref` resolver + `--validate` flag + CI step.
   - Current: Schemas present at `spec/references/omg-sysml-v2/SysML.json` and `spec/references/omg-kerml-v1/KerML.json`; no validator wired up.
   - Target: A schema-registry module loads both schemas at startup, resolves absolute OMG `$id`/`$ref` URLs against the local bundle (no network access required). `deal build --target sysml-v2 --validate` runs the emitter then validates each output JSON against the loaded schemas, exiting non-zero on any validation error. A CI step runs `deal build --validate` against the showcase as a gating check.
   - Acceptance: `deal build --target sysml-v2 --validate tests/showcase/` exits 0 with all schemas resolved offline (verified with network disabled in test); injecting an invalid field into the emitter output (test harness) causes `--validate` to exit non-zero with a schema-path-and-error diagnostic.

6. **5–8 golden fixtures (byte-exact match)**: Hand-written `.deal` → expected `.json` pairs covering the core IR concepts.
   - Current: No golden fixtures exist.
   - Target: 5–8 minimal `.deal`/`.dealx` files under `tests/golden/sysml-v2/` (e.g., `01-part-def.deal`, `02-port-usage.deal`, `03-specialization.deal`, `04-attribute-usage.deal`, `05-requirement-def.deal`, `06-trace-link.deal`, `07-package.deal`, `08-dealx-composition.dealx`) each paired with a hand-written `expected.json`. A test compares the emitter output byte-for-byte against `expected.json` and asserts schema validity for every output.
   - Acceptance: All 5–8 fixtures pass byte-exact match against the emitter output AND pass schema validation; introducing a regression in the emitter (test harness) causes byte-mismatch failure with a useful diff.

7. **`deal fmt` — round-trip with comment preservation**: Pretty-printer round-trips every showcase file with byte-identical AST AND every `//` line comment and `/** */` doc-comment surviving at its original attachment point.
   - Current: No formatter; parser drops all comments (no AST comment-attachment infrastructure).
   - Target: Two-part work: (1) parser changes to attach `//` and `/** */` comments to the AST nodes they precede/follow (leading + trailing positions), (2) a new pretty-printer module emits canonical formatted source from the AST including attached comments. `deal_format()` exposed via the C ABI for Rust CLI consumption.
   - Acceptance: For all 19 showcase files: parse → format → parse → AST equality holds AND every comment present in the original file is present in the round-tripped output at the same attachment point. An automated test enumerates all comments per file (with line/column) and asserts each survives.

8. **Rust CLI shell (`deal` binary) with global flags**: `parse`, `check`, `fmt`, `build --target sysml-v2` subcommands + `--json`, `--color`, `--verbose` global flags.
   - Current: `cli/` directory has README only — no `Cargo.toml`, no `src/`, no binary.
   - Target: A Rust `deal` binary built with `clap` exposes four subcommands (`parse`, `check`, `fmt`, `build --target sysml-v2`) and three global flags (`--json` for machine-readable diagnostic output, `--color={auto,always,never}` for TTY color control, `--verbose` for diagnostic verbosity). Calls into the Zig core via the C ABI (`libdeal.a` linkage already established by Phase 1's FFI harness). Human-mode diagnostic output is `rustc`/`cargo` quality (colored, span-pointed, with source snippets); `--json` mode emits a stable schema documented in this phase.
   - Acceptance: `cargo build --release` produces a `deal` binary; `deal --version` works; each of the four subcommands runs end-to-end on the showcase; `deal check --json` output validates against the `--json` diagnostic schema (also a Phase 2 deliverable, locked as a public contract); `--color=never` produces no ANSI escapes; `--color=always` produces them in non-TTY output.

9. **Phase 2 exit gate — including fresh-worktree gate**: All Phase 1.5 gates still pass + Phase 2 gates pass + fresh-worktree variant + viewer-import smoke + IR-lock mid-phase checkpoint observed.
   - Current: Only `phase-1.5-gate` and `phase-1.5-gate-fresh` exist.
   - Target: `zig build phase-2-gate` and a `phase-2-gate-fresh` equivalent (which runs the same gate inside a freshly-created git worktree after `git submodule update --init --recursive`, per ADR-phase-1.5-fresh-worktree-verification.md). The gate runs: every Phase 1.5 gate (regression protection) + `deal check` against the showcase exits 0 with `.deal/index.json` written + `deal build --target sysml-v2 --validate` exits 0 + all 5–8 golden fixtures byte-match + all 19 fmt round-trips pass + at least one named SysML v2 viewer (the specific tool recorded in `02-VERIFICATION.md`) imports the showcase JSON successfully (or a documented blocker is recorded showing ALL attempted tools failed with reasons). Mid-phase IR-lock checkpoint: after the IR v0 spec (REQ #2) and lowering pass (REQ #3) land — but before SysML codegen (REQ #4) begins — a recorded review step pauses execution; the recorded outcome (approved / changes-requested with itemized changes) appears in `02-VERIFICATION.md`.
   - Acceptance: `zig build phase-2-gate-fresh` exits 0 on a freshly-cloned worktree of `deal-lang/deal`. `02-VERIFICATION.md` records: (a) the specific SysML viewer used and its import outcome (or the documented blocker), (b) the IR-lock checkpoint review timestamp and outcome.

## Boundaries

**In scope:**
- Semantic analyzer with all 6 checks blocking + `.deal/index.json` generation
- DEAL IR v0: `ADR-deal-ir-v0.md` + `spec/ir/v0/` JSON Schema + Markdown reference
- AST → IR lowering pass (Zig) for all 19 showcase files
- SysML v2 JSON emitter targeting bundled OMG schemas
- Offline JSON-Schema validator with absolute `$id`/`$ref` resolver + `--validate` flag
- 5–8 hand-written golden fixtures (byte-exact match + schema validation)
- `deal fmt` with comment preservation (requires parser changes to attach comments)
- Rust `deal` CLI binary: `parse`, `check`, `fmt`, `build --target sysml-v2` subcommands
- Global CLI flags: `--json`, `--color={auto,always,never}`, `--verbose`
- Stable `--json` diagnostic output schema (public contract)
- Stable E2xxx semantic error code allocation (public contract)
- Mid-phase IR-lock checkpoint between IR landing and codegen starting
- `phase-2-gate` + `phase-2-gate-fresh` build steps
- Real SysML v2 viewer import smoke (success required, documented blocker only if all viewers fail)

**Out of scope:**
- **ReqIF codegen + `deal import` (SysML/ReqIF → .deal)** — ReqIF emitter is Phase 4; reverse importers are Phase 6. Even though IR is freshly in place, both directions are deferred to keep Phase 2 a single forward pipeline.
- **LSP / VS Code / TextMate / tree-sitter** — Phase 3. The Rust `deal` CLI binary lives entirely separately from any future `deal-lsp` binary; no LSP scaffolding lands in Phase 2.
- **`deal init` / `deal install` / package resolution** — Phase 4 (`deal-stdlib`, `deal-lang.org`). The showcase uses `tests/showcase/deal.toml` directly; no project scaffolding or dependency resolution work happens now.
- **Simulation, evidence cache, `deal-sim`, verification dashboard** — Phase 5. The `.deal/simulations/` cache directory referenced in showcase `deal.toml` is ignored by Phase 2 tooling; `@evidence` / verification semantics are not analyzed.
- **`deal init --watch` / `deal index --watch`** — file-watch loops are deferred to Phase 3/4 tooling needs.
- **Multiple viewer success required** — only ONE successful SysML v2 viewer import is required; multi-viewer compatibility is a Phase 6 desktop-tooling concern.

## Constraints

- **Fresh-worktree invariant** (inherited from Phase 1.5 ADR): `phase-2-gate-fresh` MUST exit 0 inside a freshly-created git worktree after `git submodule update --init --recursive`. No dev-local untracked files may mask test passes.
- **Offline-only validation**: The JSON-Schema validator MUST resolve all OMG `$id`/`$ref` URLs against the local `spec/references/` bundle. Tests run with network disabled to enforce this.
- **Parser-comment attachment is a one-way door**: REQ #7 requires parser changes to attach `//` and `/** */` comments to AST nodes. This affects every AST consumer (snapshots, IR lowering, C ABI `deal_ast_json`). Existing snapshots will need refresh; the determinism test must continue to pass after attachment.
- **Public contract surfaces** (3 — all locked in Phase 2, breaking changes after Phase 2 require an ADR):
  1. DEAL IR v0 shape (consumed by Phase 3 LSP, Phase 4 ReqIF emitter, Phase 5 sim bindings, Phase 6 importers)
  2. `deal check --json` diagnostic output schema (consumed by Phase 3 LSP, CI, Phase 4 docs site)
  3. E2xxx semantic error code allocation (visible to end users; changes are breaking)
- **Tech-stack split** (PROJECT.md decision): Zig owns the compiler core (semantic analyzer, IR lowering, fmt's pretty-printer is implementation-flexible). Rust owns CLI shell + diagnostic rendering. Codegen language (Zig or Rust) is a discuss-phase decision.
- **CLI exit codes**: 0 = success, 1 = user-visible error (diagnostic emitted), 2 = internal error (bug/panic). `deal check` returns 1 on any blocking semantic check failure.
- **Determinism extended to IR**: REQ #3 acceptance requires byte-identical IR across two lowering passes of the same source (parallel to Phase 1.5's `determinism.parse_twice` for AST).

## Acceptance Criteria

- [ ] `deal check tests/showcase/` exits 0 and writes a valid `.deal/index.json`
- [ ] 6 hand-crafted regression fixtures (one per semantic check) each cause `deal check` to exit non-zero with the expected E2xxx code
- [ ] `ADR-deal-ir-v0.md` and `spec/ir/v0/` (JSON Schema + Markdown reference) both exist and are referenced from `02-SUMMARY.md`
- [ ] AST → IR lowering for all 19 showcase files produces IR that validates against the spec/ir/v0/ JSON Schema
- [ ] Two lowering passes of the same source produce byte-identical IR (determinism)
- [ ] `deal build --target sysml-v2 --validate tests/showcase/` exits 0 with network disabled (offline validation works)
- [ ] All 5–8 golden fixtures pass byte-exact match against emitter output AND schema validation
- [ ] All 19 showcase files round-trip through `deal fmt` with byte-identical AST AND every comment preserved at its original attachment point
- [ ] `deal --version` works; all four subcommands (`parse`, `check`, `fmt`, `build --target sysml-v2`) run end-to-end on the showcase
- [ ] `deal check --json` output validates against the published `--json` diagnostic schema
- [ ] `--color={auto,always,never}` honored: no ANSI escapes when `never`, present when `always` even in non-TTY output
- [ ] `zig build phase-2-gate-fresh` exits 0 inside a freshly-cloned worktree
- [ ] `02-VERIFICATION.md` records the specific SysML v2 viewer used and a successful import outcome (or a documented blocker showing ALL attempted tools failed)
- [ ] `02-VERIFICATION.md` records the mid-phase IR-lock checkpoint review timestamp and approval outcome

## Ambiguity Report

| Dimension          | Score | Min  | Status | Notes                                                       |
|--------------------|-------|------|--------|-------------------------------------------------------------|
| Goal Clarity       | 0.95  | 0.75 | ✓      | Internal-demo readiness, 9 sub-reqs, viewer-import locked   |
| Boundary Clarity   | 0.92  | 0.70 | ✓      | All 4 adjacent phases explicitly excluded                   |
| Constraint Clarity | 0.90  | 0.65 | ✓      | Fresh-worktree invariant + 3 public contracts locked        |
| Acceptance Criteria| 0.90  | 0.70 | ✓      | 14 pass/fail criteria, viewer-import is binary              |
| **Ambiguity**      | 0.08  | ≤0.20| ✓      | Comfortable margin                                           |

## Interview Log

| Round | Perspective         | Question summary                                                              | Decision locked                                                                                                  |
|-------|---------------------|-------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------|
| 1     | Researcher          | Primary proof-of-life for Phase 2 success?                                    | Internal-demo readiness — all 9 sub-requirements at internal-demo quality (no single headline deliverable)        |
| 1     | Researcher          | Should Phase 2 inherit Phase 1.5 fresh-worktree gate invariant?               | Yes — `phase-2-gate-fresh` required (binds the ADR-phase-1.5-fresh-worktree-verification.md invariant)            |
| 2     | Researcher+Simplifier | Semantic analyzer scope — blocking vs warning vs deferred?                  | All 6 checks BLOCKING + `.deal/index.json` (maximum semantic rigor)                                              |
| 2     | Researcher+Simplifier | `deal fmt` round-trip fidelity — comments?                                  | Identical AST + comments preserved (parser must attach `//` and `/** */` comments to AST nodes)                  |
| 2     | Researcher+Simplifier | CLI surface — which subcommands ship in Phase 2?                            | 4 subcommands (`parse`, `check`, `fmt`, `build`) + global flags `--json`, `--color`, `--verbose`                  |
| 3     | Boundary Keeper     | Viewer-smoke acceptance — what counts as PASSING?                             | Successful import in at least one real viewer; documented blocker only if ALL attempted tools failed              |
| 3     | Boundary Keeper     | DEAL IR v0 specification location?                                            | BOTH `ADR-deal-ir-v0.md` (rationale) AND `spec/ir/v0/` (normative JSON Schema + Markdown reference)              |
| 4     | Boundary Keeper     | Confirm out-of-scope perimeter — all 4 deferred?                              | Yes — all 4 deferred (ReqIF/import → Ph4/6; LSP → Ph3; init/install → Ph4; simulation → Ph5)                     |
| 4     | Failure Analyst     | Highest downstream rework risk?                                               | Equal risk on IR shape, `--json` diagnostic schema, E2xxx codes → all three treated as locked public contracts    |
| 4     | Failure Analyst     | Mid-phase checkpoint between IR landing and codegen starting?                 | Yes — IR-lock checkpoint required (recorded review in `02-VERIFICATION.md`)                                       |

---

*Phase: 02-prove-the-pipeline*
*Spec created: 2026-05-21*
*Next step: /gsd:discuss-phase 2 — implementation decisions (Zig vs Rust for codegen, IR schema details, comment-attachment grammar, `--json` diagnostic schema shape, SysML viewer selection, plan slicing around the IR-lock checkpoint)*
