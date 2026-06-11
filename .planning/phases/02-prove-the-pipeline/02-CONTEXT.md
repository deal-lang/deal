# Phase 2: Prove the Pipeline - Context

**Gathered:** 2026-05-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 2 delivers the first end-to-end DEAL pipeline (`.deal` source → semantic check → DEAL IR v0 → SysML v2 JSON → real viewer) and locks three downstream public-contract surfaces that Phases 3–6 will depend on:

1. **DEAL IR v0 shape** (consumed by Phase 3 LSP, Phase 4 ReqIF emitter, Phase 5 sim bindings, Phase 6 importers)
2. **`deal check --json` diagnostic schema** (consumed by Phase 3 LSP, CI, Phase 4 docs site)
3. **E2xxx semantic error code allocation** (end-user-visible; breaking changes need an ADR)

The phase ships: a Zig semantic analyzer with all 6 blocking checks, a Zig AST→IR lowering pass, a Rust SysML v2 emitter that validates offline against the bundled OMG schemas, 5–8 golden fixtures, a Zig pretty-printer with comment preservation, and a Rust `deal` CLI binary with four subcommands and three global flags. Internal-demo readiness gate, fresh-worktree gate, mid-phase IR-lock checkpoint, and a real-viewer import smoke complete the phase.

The phase does NOT ship ReqIF codegen, `deal import` (reverse direction), LSP scaffolding, `deal init`/package resolution, simulation/evidence semantics, or file-watch loops — those are deferred to Phases 3–6 per SPEC.md.

</domain>

<spec_lock>
## Requirements (locked via SPEC.md)

**9 requirements are locked.** See `02-SPEC.md` for full requirements, boundaries, and acceptance criteria (ambiguity score 0.08; gate ≤ 0.20; 14 acceptance criteria).

Downstream agents MUST read `02-SPEC.md` before planning or implementing. Requirements are not duplicated here.

**In scope (from SPEC.md):**
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

**Out of scope (from SPEC.md):**
- ReqIF codegen + `deal import` (ReqIF→.deal, SysML→.deal) — Phase 4 / Phase 6
- LSP / VS Code / TextMate / tree-sitter — Phase 3
- `deal init` / `deal install` / package resolution — Phase 4
- Simulation, evidence cache, `deal-sim`, verification dashboard — Phase 5
- `deal init --watch` / `deal index --watch` — deferred to Phase 3/4
- Multiple viewer success required — only ONE successful import needed

</spec_lock>

<decisions>
## Implementation Decisions

> Decisions are numbered D-19 onward to continue the project-wide D-01..D-18 sequence from Phase 1.

### Tech-stack split for Phase 2 (Codegen language area)

- **D-19:** **Rust owns the SysML v2 emitter and the offline JSON-Schema validator.** Walks the IR via the new C ABI `deal_ir_json()`. Pros: mature `jsonschema` crate ecosystem to validate against the 126K-line OMG schemas offline; aligns with PROJECT.md tech-stack constraint ("Rust owns CLI, formatter integration, SysML v2 / ReqIF codegen backends"); pays one IR-serialize round-trip across the FFI boundary per `deal build` (acceptable). The same module owns the schema-registry / absolute `$id`/`$ref` resolver against `spec/references/omg-sysml-v2/SysML.json` + `omg-kerml-v1/KerML.json`.
- **D-20:** **Zig owns the semantic analyzer.** New module `src/sema.zig` runs all 6 blocking checks (name resolution, type checking, multiplicity enforcement, `<<specializes>>` compatibility, `@trace` reference validation, import resolution) and produces `.deal/index.json`. Walks the AST in the same arena as the parser — no FFI cost. New diagnostic codes flow through the existing `diagnostics.zig` `Codes` namespace (see D-22).
- **D-21:** **Zig owns the pretty-printer (`deal fmt`'s emitter).** New module `src/fmt.zig` walks the AST (with attached comments per D-25) and emits canonical formatted source bytes. Rust CLI calls via new C ABI symbol `deal_format(handle, out_ptr, out_len)` — explicitly named in SPEC.md REQ #7 acceptance criterion.

### IR v0 transport and contract (IR v0 design area)

- **D-22:** **IR crosses the FFI boundary as JSON via a new C ABI symbol `deal_ir_json(handle, out_ptr, out_len)`** — extends the established Phase 1 pattern (D-11). Locked alongside Phase 1's existing 6 exports; bumps the C ABI export count to 8 (adding `deal_ir_json` + `deal_format`). Honors D-04/D-18 conventions: top-level `{"v": 1, "ir_version": "v0", ...}`, alphabetical-key ordering inside each node payload. Rust consumes via serde_json into typed structs.
- **D-23:** **Stable element IDs are fully-qualified path strings.** Example: `sedan_project.vehicle.electrical.BatteryPack.hvOut`. Used as: (a) `@trace` targets, (b) SysML v2 `$id` URL suffixes, (c) `.deal/index.json` keys, (d) IR `find(id)` lookups. Mirrors PS-2 dot-paths. Deterministic by construction; uniqueness guaranteed by name-resolution (one of the 6 blocking checks). Path-component separators are `.` between package segments and `.` between element members — implementations should escape literal `.` in identifiers if the showcase ever surfaces one (currently no showcase identifier contains `.`).
- **D-24:** **The IR is a single workspace-wide graph.** One IR instance spans the entire `deal.toml` workspace; cross-package references are first-class edges, not stringly path references awaiting resolution. `deal build --target sysml-v2 tests/showcase/` produces ONE consolidated `tests/showcase/build/sysml-v2/showcase.sysml-v2.json` file (NOT per-package). The 5–8 golden fixtures (REQ #6) each produce one JSON output. Matches FS-4 "system-wide index" intent. Incremental graph rebuild is explicitly out of scope (Phase 3 LSP concern).
- **D-25:** **The IR is comment-free.** Comments and trivia stay on the AST only. `deal fmt` walks the AST (per D-21); the SysML v2 emitter walks the IR (per D-19). Mirrors typical compiler splits (TypeScript AST has trivia, IR doesn't). Future ADRs may open paths for doc comments to flow into SysML's `documentation` ownedRelationship — explicitly deferred to a Phase 2.x or Phase 4 ADR rather than locked now.
- **D-26:** **IR v0 backend traversal API surface (locked):**
  - `walk(visitor)` — pre/post-order visitor over the entire graph
  - `find(id) -> ?Node` — resolves a fully-qualified path string to a node
  - `references(id) -> []EdgeRef` — all incoming edges (callers of, satisfiers of, traceables to, etc.)
  - `children(id) -> []Node` / `parent(id) -> ?Node` — structural navigation
  - **Not in v0:** query DSL (MQL is deferred per PROJECT.md ADR; do not introduce a partial query language here). Ad-hoc queries are coded as visitors.
- **D-27:** **IR v0 dual-document specification location and shape.** Both artifacts MUST land in this phase (per REQ #2):
  - `.planning/decisions/ADR-deal-ir-v0.md` — rationale (the why), alternatives rejected (including this discussion's options), links to the normative spec.
  - `spec/ir/v0/schema.json` — JSON Schema document for IR node kinds (loadable by any standard JSON-Schema validator).
  - `spec/ir/v0/README.md` — Markdown reference explaining ID strategy (per D-23), span carry-over from AST, relationship graph contract, metadata envelope shape (`@confidence`/`@rationale`/`@assumes`/`@concerns` — schema deferred to research), diagnostic attachment points, and the backend traversal API contract (per D-26). Both files referenced from `02-SUMMARY.md` at phase close.

### Comment attachment (Comment-attachment grammar area)

- **D-28:** **Attachment model: leading + trailing comments on declaration/statement nodes.** Each declaration node gets two new optional fields: `leading_comments: []Comment` (comments above the node before the next blank line) and `trailing_comments: []Comment` (comments on the same physical line after the node's last token). Comments between two declarations attach as `trailing` to the previous OR `leading` to the next using **Go/gofmt-style blank-line rules**: if separated by a blank line from the previous declaration, the comment is leading on the next; otherwise trailing on the previous. (See `references/gofmt` for the established convention; researcher should cite the exact rule shape.)
- **D-29:** **JSDoc `/** */` doc comments are promoted to a typed `doc_comment: ?DocComment` field on each declaration.** Reuses the existing `DocComment` struct already declared at `src/ast.zig:393` (which already parses `@tag` fields per Phase 1's grammar work). The existing `doc_comment` NodeKind union arm (at `src/ast.zig:602`) stays for the rare floating-doc-comment edge case (a `/** */` not immediately followed by a declaration), but the common case is the attached field. `//` and `/* */` go in `leading_comments`/`trailing_comments`; `/** */` goes in `doc_comment`. This separation lets LSP hover and future SysML `documentation` lowering find doc comments structurally.
- **D-30:** **AST JSON stays at `"v": 1` — no schema-version bump for comments.** The 19 existing AST snapshots are regenerated in a single dedicated commit (one-shot refresh); the diff is reviewed for unexpected churn (only the new `leading_comments` / `trailing_comments` / `doc_comment` fields should appear; structural fields must remain byte-identical). Determinism test (`tests/unit/determinism_parse_twice.zig`) MUST continue to pass after refresh. Property-span-containment test MUST continue to pass — comment spans are children of the node they're attached to (their byte ranges fall within or adjacent to the node's range).
- **D-31:** **Comment attachment lands EARLY in plan order — before semantic analyzer or IR lowering work begins.** Plan 02-01 (per the slicing in D-37) does the parser/lexer changes + snapshot refresh, so every downstream consumer (sema, IR lowering, codegen, fmt) sees the final AST shape from day one. Mid-phase AST-shape churn is explicitly avoided.

### Diagnostic shape and error code allocation (--json + E2xxx area)

- **D-32:** **`deal {subcommand} --json` envelope** — extends Phase 1's existing diagnostic JSON (D-14) with a small wrapping object:
  ```json
  {
    "v": 1,
    "command": "check" | "parse" | "fmt" | "build",
    "deal_version": "<from cargo + zig build>",
    "diagnostics": [ /* Phase 1's Diagnostic array, unchanged */ ],
    "summary": { "errors": N, "warnings": N, "hints": N }
  }
  ```
  Honors D-04 (`"v"` first), D-18 (alphabetical keys inside each diagnostic object, alphabetical keys in `summary`). The `deal_diagnostics_json()` C ABI symbol stays unchanged — Rust CLI does the envelope-wrap.
- **D-33:** **E2xxx semantic error code band — extends D-16:**
  - `E2000..E2099` — name resolution (Check #1)
  - `E2100..E2199` — type checking (Check #2)
  - `E2200..E2299` — multiplicity enforcement (Check #3)
  - `E2300..E2349` — `<<specializes>>` compatibility (Check #4)
  - `E2350..E2399` — `@trace` reference validation (Check #5)
  - `E2400..E2499` — import resolution (Check #6)

  Extension committed in `src/diagnostics.zig` alongside the existing `// D-16` documentation comment. Codes are stable across versions per D-16; reuse-after-removal is forbidden. Diagnostic strings declared as `pub const` in the `Codes` namespace; each new code carries a doc comment citing its semantic check and the SPEC.md acceptance fixture that exercises it.
- **D-34:** **Rust CLI exit code semantics (extending SPEC.md constraints):** `0` = success; `1` = user-visible error (one or more diagnostics with `severity = "error"` emitted, or `--validate` failed); `2` = internal error (panic, FFI shape mismatch, schema-registry failed to load). `deal check` returns `1` on any blocking semantic check failure. The CLI maps `deal_has_errors() != 0` → exit 1; an FFI-layer error (e.g., `deal_parse()` couldn't allocate) → exit 2.

### Phase-2 gates and verification (Plan slicing + gate area)

- **D-35:** **Phase-2 exit gate inherits and extends the fresh-worktree invariant.** Two new build steps land:
  - `zig build phase-2-gate` — bundles Phase 1.5's full gate (regression protection) + Phase 2 acceptance criteria (REQ #1–#8 + #9 except viewer).
  - `zig build phase-2-gate-fresh` — runs `phase-2-gate` inside an ephemeral worktree after `git submodule update --init --recursive`, per ADR-phase-1.5-fresh-worktree-verification.md. Reuses (or mirrors) `scripts/verify-fresh-worktree.sh`.

  The phase exit ALSO requires (a) viewer-import smoke recorded in `02-VERIFICATION.md` (or documented blocker if all attempts failed), (b) mid-phase IR-lock checkpoint review timestamp + outcome recorded.
- **D-36:** **SysML v2 viewer selection for REQ #9 acceptance** — researcher tries, in priority order: (1) OpenMBEE SysML v2 Pilot Implementation (Eclipse-based open-source reference), (2) IncQuery's SysML v2 toolset (commercial trial), (3) NoMagic/Cameo SysML v2 plugin if a trial path exists, (4) any conformant generic JSON-Schema viewer as fallback. First tool that produces a successful import wins; that tool's name is recorded in `02-VERIFICATION.md`. Other attempts are also logged. A documented blocker (all attempts failed) is acceptable per SPEC.md REQ #9 only if every listed tool's failure is recorded with a reason.
- **D-37:** **Plan slicing — 6 plans with the IR-lock checkpoint between Plan 03 and Plan 04:**
  - **Plan 02-01: CLI shell + comment attachment.** Stand up the Rust `cli/` crate (Cargo workspace, `clap` skeleton, `--json`/`--color`/`--verbose` global flags, `deal --version`, four subcommand stubs that error-out cleanly). Implement comment attachment per D-28..D-31 (parser/lexer changes, AST snapshot refresh, determinism + property tests pass on refreshed snapshots).
  - **Plan 02-02: Semantic analyzer.** `src/sema.zig` implementing all 6 blocking checks. E2xxx code allocation per D-33 wired into `diagnostics.zig`. `.deal/index.json` writer. `deal check` end-to-end on showcase (exit 0 on clean; 6 regression fixtures each exit 1 with the expected E2xxx code).
  - **Plan 02-03: DEAL IR v0 + lowering.** ADR-deal-ir-v0.md + `spec/ir/v0/` schema + Markdown reference. `src/ir.zig` + `src/lowering.zig`. `deal_ir_json()` C ABI symbol. Backend traversal API per D-26. Determinism extended to IR (parse-twice → byte-identical IR).
  - **🔒 IR-lock checkpoint — recorded review.** Pause before codegen begins. Outcome (approved / changes-requested with itemized changes) recorded in `02-VERIFICATION.md` with timestamp. SPEC.md REQ #9 acceptance criterion.
  - **Plan 02-04: SysML v2 codegen + offline validator + golden fixtures.** Rust SysML v2 emitter walking IR JSON. Offline JSON-Schema validator with absolute `$id`/`$ref` resolver. `--validate` flag. 5–8 hand-written golden fixtures with byte-exact match + schema-valid acceptance gates.
  - **Plan 02-05: `deal fmt` + comment-preserving round-trip.** `src/fmt.zig` pretty-printer. `deal_format()` C ABI symbol. Rust `deal fmt` wiring. All 19 round-trip tests (parse → format → parse → identical AST + every comment preserved at original attachment point).
  - **Plan 02-06: phase-2-gate + phase-2-gate-fresh + viewer-import smoke + verification.** Final gate steps in `build.zig`. Viewer-import attempted per D-36. `02-VERIFICATION.md` written with both the IR-lock checkpoint record AND the viewer outcome.

### Claude's Discretion

- **Schema-registry implementation crate.** Rust crate selection for offline JSON-Schema validation (e.g., `jsonschema` vs `boon` vs `schemars`) is researcher's call; the constraint is offline-only resolution of absolute OMG `$id`/`$ref` URLs against the local bundle.
- **Cargo workspace layout.** Whether `cli/` and `tests/ffi/` share a Cargo workspace, and where `bindgen` of `deal.h` happens (build.rs vs committed wrapper). Phase 1's `tests/ffi/Cargo.toml` is the closest existing precedent.
- **Agent-metadata envelope shape in IR.** `@confidence` / `@rationale` / `@assumes` / `@concerns` map to which IR field names and types — research the showcase's actual usage and pick the shape that round-trips losslessly. Locked: it's a typed envelope, NOT a free-form map.
- **Simulation-binding representation in IR.** How `@simulation:<<computes>>` annotations are represented — defer to research; the IR carries them but the exact shape isn't locked beyond "the SysML v2 emitter doesn't need them in Phase 2; sim integration in Phase 5 will read them."
- **Doc-comment `documentation` lowering to SysML.** Whether `/** */` text flows into SysML v2 `documentation` ownedRelationships in Phase 2's emitter. Researcher should evaluate against the 19 showcase doc-comments and the SysML schema's `documentation` shape; if straightforward, do it; if not, defer to a follow-up ADR and emit without `documentation`.
- **Blank-line rule edge cases for comment attachment.** Exact gofmt rule shape (single blank vs multiple blanks, comments at EOF, comments inside `{...}` attribute bodies that aren't on declarations). Researcher cites Go's `go/ast` and `go/printer` for the canonical pattern.
- **Whether `deal_format()` returns the formatted bytes or writes to a caller-supplied path.** Lean toward bytes (LSP-friendly), but specifics like null-terminator handling parallel D-11's existing accessors.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 2 spec lock (READ FIRST)
- `.planning/phases/02-prove-the-pipeline/02-SPEC.md` — **Locked requirements. MUST read before any planning or implementation.** 9 requirements, 14 acceptance criteria, ambiguity 0.08.

### Project authority
- `.planning/PROJECT.md` — Tech stack (Zig core / Rust frontend), 64 LOCKED + 1 CAPTURED design decisions in `<decisions>` block, ingest provenance.
- `.planning/REQUIREMENTS.md` §Phase 2 — REQ-phase-2-prove-pipeline, REQ-phase-2-1-semantic-analyzer, REQ-phase-2-2a-ir-v0, REQ-phase-2-2b-ir-lowering, REQ-phase-2-3-sysml-v2-codegen, REQ-phase-2-3a-offline-validator, REQ-phase-2-3b-golden-fixtures, REQ-phase-2-4-formatter, REQ-phase-2-5-cli-shell, REQ-phase-2-gate.
- `.planning/ROADMAP.md` §"Phase 2: Prove the Pipeline" — Goal statement and 6 success criteria.
- `.planning/STATE.md` — Phase 1.5 GREEN; Phase 1 plan progress 6/6 ✓; Phase 1.5 plan progress 5/5 ✓.

### Locked design decisions (Phase 0 ADR + Phase 1/1.5 D-codes)
- `spec/grammar/DESIGN-DECISIONS.md` — Consolidated ADR (64 LOCKED + 1 CAPTURED). Particularly: FA-1..5 (architecture; IR is the kernel), FS-1..4 (file structure; `.deal/index.json` per FS-4), PS-1..10 (project structure; `deal.toml`, packages, `.deal/` gitignored), SD-1..20 (definition syntax), CS-1..16 (composition syntax). Phase 2 cannot contradict any LOCKED entry.
- `.planning/phases/01-zig-compiler-core/01-CONTEXT.md` — Phase 1 D-01..D-18 (AST tagged-union, per-handle arena, JSON shape, C ABI, diagnostic model, span = u32 byte offsets, **D-16 error-code ranges**, **D-18 alphabetical-key JSON invariant**). Phase 2 MUST preserve all.
- `.planning/phases/01.5-parser-strictness-test-gap-resolution-inserted/01.5-CONTEXT.md` — Phase 1.5 strictness decisions (m08/m18/m19 dispositions, per-file pinning, determinism test, span-containment property test). The fresh-worktree invariant (`invariants_added_post_completion` block) binds Phase 2..6.
- `.planning/decisions/ADR-phase-1.5-fresh-worktree-verification.md` — **Binding ADR.** Phase 2 MUST add `phase-2-gate-fresh` build step. Authoritative gate command: `zig build phase-2-gate-fresh` must exit 0 inside an ephemeral worktree.
- `.planning/decisions/ADR-phase-1.5-m01-sd10-optional-semicolon.md` — SD-10 semicolon disposition for `m01_missing_semicolon.deal`; informational.

### Grammar specifications (LOCKED at `0.1.0-draft`)
- `spec/grammar/lexical.ebnf` — Shared token set (758 lines). Comment-attachment work (D-28..D-30) must respect §2 Comments token kinds (`//`, `/* */`, `/** */`).
- `spec/grammar/deal.ebnf` — 87 productions for `.deal` definition files (1,679 lines). Semantic analyzer's name-resolution, type-check, multiplicity, and `<<specializes>>` checks operate against the AST shape this grammar locks.
- `spec/grammar/dealx.ebnf` — 43 productions for `.dealx` composition files (897 lines). Sema's `@trace` reference validation operates here.
- `spec/grammar/README.md` — Grammar README; cross-references the three EBNF files.

### Codegen target schemas (LOCKED at `2024-09` per OMG release)
- `spec/references/omg-sysml-v2/SysML.json` — **SysML v2 JSON Schema (126,290 lines).** The SysML v2 emitter (D-19) maps DEAL IR elements to types declared here: `PartDefinition`, `PortUsage`, `Specialization`, `AttributeUsage`, `RequirementDefinition`, traceability `Dependency`, `Package`. Absolute `$id`/`$ref` URLs require local registry resolution (REQ #5).
- `spec/references/omg-kerml-v1/KerML.json` — **KerML JSON Schema (42,284 lines).** Sibling schema; SysML.json `$refs` into it. The offline validator MUST resolve both schemas without network.
- `spec/references/omg-kerml-v1/KerML-Model-Interchange.json` — Companion (small, 110 lines).
- `spec/references/omg-specs/` — OMG textual BNFs and PDFs; cross-reference for emitter mapping decisions.

### Zig language standards (LOCKED for Phase 2 code)
- `spec/references/zig-lang/0.16.0/langref.html` — **MUST read.** All sema, IR, and fmt code uses 0.16.0 idioms: `std.heap.ArenaAllocator` shape, `std.ArrayList` allocator-explicit APIs, post-0.13 capture syntax, error-set patterns, tagged-union exhaustive switches, packed structs.
- `spec/references/zig-lang/0.16.0/README.md` — Companion notes.

### Integration oracle (snapshot truth)
- `spec/examples/showcase/` — **19-file showcase project.** All Phase 2 acceptance gates run against this corpus (15 `.deal` + 4 `.dealx`). `deal check` exits 0, `deal build` exits 0, `deal fmt` round-trips every file. Mounted at `deal/tests/showcase/` via committed symlink (per Phase 1.5 Plan 05).
- `spec/examples/showcase/deal.toml` — Project manifest. Phase 2 parses `deal.toml` for the `[workspace]`, `[workspace.aliases]`, and `[build.targets]` blocks (required for import resolution and `deal build --target sysml-v2`).
- `spec/examples/showcase/simulations/deal.sims.toml` — Simulation registry; informational for Phase 2 (Phase 5 work).

### Implementation files (Phase 1 + 1.5 deliverables that Phase 2 modifies)
- `deal/src/lexer.zig` — Comment-attachment work (D-28..D-30) modifies `skipTrivia` to emit `//` and `/* */` as tokens instead of skipping; `unterminated_block_comment` field already exists (Phase 1.5 Plan 02 wiring for E0003).
- `deal/src/parser_deal.zig` + `deal/src/parser_dealx.zig` — Comment attachment to declaration nodes per D-28..D-29.
- `deal/src/ast.zig` — Extend `Node` payload variants with `leading_comments: []Comment`, `trailing_comments: []Comment`, `doc_comment: ?DocComment` (reuse existing `DocComment` struct at line 393). Add IR types (D-22..D-27).
- `deal/src/diagnostics.zig` — Extend `Codes` namespace with E2xxx allocations per D-33. Update the `// D-16` documentation comment with the new band.
- `deal/src/json.zig` — Extend AST JSON emitter for new comment fields (D-18 alphabetical-key ordering). Add IR JSON emitter (D-22 → `deal_ir_json` body).
- `deal/src/lib.zig` — Export new C ABI symbols: `deal_ir_json` (D-22), `deal_format` (D-21). Total exports rises 6 → 8.
- `deal/include/deal.h` — Regenerate or hand-update with the two new symbol declarations + their out-param shape (mirrors D-11 length-prefixed UTF-8 pattern).
- `deal/build.zig` — Add `phase-2-gate` and `phase-2-gate-fresh` build steps (D-35); the latter shells out to `scripts/verify-fresh-worktree.sh` per ADR-phase-1.5-fresh-worktree-verification.md.

### New files Phase 2 will create
- `deal/src/sema.zig` — Semantic analyzer (D-20). New file; reuses parser arena.
- `deal/src/ir.zig` — DEAL IR v0 types (D-22..D-27). New file.
- `deal/src/lowering.zig` — AST → IR lowering pass (REQ #3). New file.
- `deal/src/fmt.zig` — Pretty-printer (D-21). New file.
- `deal/cli/Cargo.toml` + `deal/cli/src/main.rs` — Rust CLI shell (D-19; REQ #8). New crate.
- `deal/cli/src/sysml_v2.rs` — SysML v2 emitter (D-19). Or split into a sibling crate.
- `deal/cli/src/schema_registry.rs` — Offline JSON-Schema validator (REQ #5).
- `deal/tests/golden/sysml-v2/{01..08}.deal` + `expected.json` — 5–8 golden fixtures (REQ #6). New corpus.
- `deal/tests/showcase/build/sysml-v2/showcase.sysml-v2.json` — Committed expected codegen output, or gitignored (planner decides).
- `.planning/decisions/ADR-deal-ir-v0.md` — IR v0 rationale + alternatives rejected (D-27).
- `spec/ir/v0/schema.json` — Normative IR v0 JSON Schema (D-27). Lives in the `spec/` submodule.
- `spec/ir/v0/README.md` — Markdown reference for IR v0 (D-27). Lives in the `spec/` submodule.

### Existing infrastructure to mirror
- `deal/scripts/verify-fresh-worktree.sh` — Reuse for `phase-2-gate-fresh` (D-35).
- `deal/tests/unit/determinism_parse_twice.zig` — Mirror for `determinism_lower_twice.zig` (Phase 2's IR-determinism test per REQ #3).
- `deal/tests/unit/property_span_containment.zig` — Span-containment property test; ensure refreshed snapshots (D-30) still pass it.
- `deal/tests/ffi/Cargo.toml` — Existing Rust FFI test harness; Phase 2 CLI may share workspace or stand alone.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`src/diagnostics.zig` `Codes` namespace** — Phase 1/1.5 established the `pub const` pattern for codes (E0001..E0122 currently allocated). E2xxx codes (D-33) extend this same pattern with one-line doc comments per entry.
- **`src/json.zig` D-18 alphabetical-key emitter** — Phase 1 locked the canonical AST JSON shape. IR JSON emitter (D-22) and `--json` envelope (D-32) reuse the same conventions and (likely) some of the same emitter helpers.
- **`src/ast.zig` `DocComment` struct (line 393)** — Already parses `@tag` fields per Phase 1. D-29 promotes it from a NodeKind union arm to an attached field; the struct itself is reused.
- **C ABI handle pattern (D-10..D-13)** — Established opaque-handle + per-handle arena. New `deal_ir_json` and `deal_format` symbols (D-22, D-21) extend this pattern unchanged.
- **`tests/showcase/` committed symlink → `../spec/examples/showcase`** — Phase 1.5 Plan 05 fix. All Phase 2 acceptance gates use this path; the fresh-worktree gate (D-35) verifies it materializes correctly via `git submodule update --init --recursive`.
- **`tests/ffi/Cargo.toml` Rust FFI harness** — Phase 1 deliverable; closest existing Rust precedent for the `cli/` crate's `extern "C"` declarations and `bindgen` integration if used.

### Established Patterns
- **Tech-stack split: Zig owns compiler core; Rust owns frontend tooling.** PROJECT.md constraint. D-19..D-21 honor it: emitter + validator + CLI in Rust; sema + IR + fmt-printer in Zig.
- **Schema-versioned JSON top-level.** Every emitted JSON document carries `"v": <integer>` as the first key. D-04 (AST) + D-32 (`--json` envelope) + D-22 (IR via `deal_ir_json`) all conform.
- **Alphabetical key ordering inside payloads (D-18).** Non-negotiable for snapshot stability. The IR JSON emitter and `--json` envelope MUST follow.
- **Per-handle arena allocation (D-02).** Sema, IR lowering, and fmt all allocate into the parser's existing arena — no new arenas, no cross-arena pointers.
- **Fresh-worktree verification (Phase 1.5 ADR).** Every phase-N gate must have a `phase-N-gate-fresh` sibling that runs in an ephemeral worktree. Phase 2 inherits and adds `phase-2-gate-fresh`.
- **Diagnostic emission goes through `diagnostics.zig`.** All E2xxx codes are declared in the `Codes` namespace as `pub const`; emission sites call `Diagnostic.error(.code, message, span, ...)`. No ad-hoc string-error strings.
- **C ABI exports are length-prefixed UTF-8 (D-11), never NUL-terminated.** `deal_ir_json` and `deal_format` follow.

### Integration Points
- **AST is the input to sema, IR lowering, and fmt.** The AST shape locked by Phase 1 + Phase 2's comment-attachment refresh (D-28..D-30) is the contract surface those three modules consume.
- **IR is the input to the SysML v2 emitter (D-19, in Rust).** Transport is JSON via `deal_ir_json` (D-22). Phase 4 ReqIF emitter, Phase 5 sim bindings, and Phase 6 importers will also consume this IR shape.
- **`.deal/index.json` is the input to Phase 3 LSP's workspace-wide symbol resolution.** Phase 2's sema (D-20) is the producer. The shape is part of the locked public contract surface even though SPEC.md REQ #1 doesn't explicitly call it out — researcher should produce a schema sketch alongside the symbol-table implementation.
- **`deal.h` C header.** Updated when D-19 (emitter) and D-21 (`deal_format`) land their C ABI exports. Rust CLI consumes via `bindgen` or hand-written `extern "C"` declarations.

</code_context>

<specifics>
## Specific Ideas

- **Path-string IDs use `.` as separator** (D-23). Example: `sedan_project.vehicle.electrical.BatteryPack.hvOut`. If the showcase ever surfaces an identifier with a literal `.`, escape it; the current 19-file corpus does not. Researcher should grep for any edge cases.
- **The IR-lock checkpoint is mechanically a recorded review step**, not an automated gate. Plan 02-03 ends with a halt; user reviews IR v0 (ADR + schema + Markdown ref + first lowering pass output); records approval / changes-requested in `02-VERIFICATION.md` with timestamp; Plan 02-04 begins only after recorded approval. SPEC.md REQ #9 acceptance criterion #2.
- **6 hand-crafted regression fixtures for sema (one per check)** — Required by SPEC.md REQ #1 acceptance. Should live at `deal/tests/regressions/sema/{01-name-resolution.deal, 02-type-check.deal, 03-multiplicity.deal, 04-specializes.deal, 05-trace.deal, 06-import.deal}` and each MUST cause `deal check` to exit 1 with a known E2xxx code. Build into Plan 02-02's acceptance gate.
- **5–8 golden fixtures cover the 5 SysML mapping categories** — REQ #6 acceptance: `01-part-def.deal`, `02-port-usage.deal`, `03-specialization.deal`, `04-attribute-usage.deal`, `05-requirement-def.deal`, `06-trace-link.deal`, `07-package.deal`, `08-dealx-composition.dealx`. Each pairs with a hand-written `expected.json`. Byte-exact match + schema validation are both required.
- **Determinism for IR extends Phase 1.5's pattern.** A `tests/unit/determinism_lower_twice.zig` mirrors `determinism_parse_twice.zig` (Phase 1.5 Plan 04) — lowers each showcase file twice and asserts byte-identical IR JSON via the public `deal_ir_json` ABI. Required by SPEC.md REQ #3 acceptance ("Two byte-equal lowering passes of the same source produce byte-identical IR").
- **Phase 1's six C ABI exports stay unchanged.** D-19/D-21 add `deal_ir_json` and `deal_format`, bringing the public ABI surface to 8 exports. `nm -gU libdeal.a` grep gate (already established in Phase 1.5) extends from `count == 6` to `count == 8`.
- **gofmt blank-line attachment rule** (D-28). When in doubt about comment placement, mirror Go's `go/ast`/`go/printer` behavior — researcher should cite the exact rule (e.g., "trailing if on the same line OR within `cgo`-style range; leading if separated by a blank line").
- **Viewer-import as binary outcome** (D-36, SPEC.md REQ #9). Either at least one viewer imported successfully (record name + timestamp + screenshot path if useful) OR every attempted viewer failed (record each name + failure mode). No partial credit.

</specifics>

<deferred>
## Deferred Ideas

- **MQL query language for IR** — Considered as a Phase-2 traversal API option (D-26 second choice) but explicitly deferred per PROJECT.md ADR. Phase 6+ concern.
- **IR carries doc comments as `documentation` field for SysML emit** — Came up as a third option in D-25's discussion. Could open with a follow-up ADR if the SysML output is conspicuously missing inline documentation (research the showcase output). Phase 2 ships comment-free IR.
- **AST JSON `"v": 2` schema bump** — Considered (D-30 third option) but rejected to keep one AST contract surface. If a future phase needs an incompatible AST shape, the schema bump becomes available; for now, comment-attachment is additive within v1.
- **Per-package IR + per-file SysML output** — Considered (D-24 second/third options) but rejected. If a future workflow needs per-package output (e.g., a multi-million-element model that crashes single-file viewers), a new `--target sysml-v2-per-package` could be added without changing the IR shape.
- **Free-floating trivia list (token-stream order)** — Considered as D-28 third option. The current attachment design is structural, not stream-based. If the formatter discovers attachment-rule edge cases that don't round-trip cleanly, this is the fallback architecture.
- **FlatBuffers / shared-struct IR transport** — Considered as D-22 second/third options but rejected. JSON keeps everything aligned with D-04/D-18 conventions and avoids a build-time schema codegen toolchain.
- **`deal init` / `deal install` / package resolution** — Phase 4 scope; SPEC.md out-of-scope perimeter.
- **LSP / VS Code / tree-sitter** — Phase 3 scope; SPEC.md out-of-scope perimeter.
- **ReqIF emitter + reverse importers (`deal import`)** — Phase 4 / Phase 6.
- **Simulation / evidence / `deal-sim`** — Phase 5.
- **WASM packaging of Zig core** — Evaluate at Phase 2 gate per PRD §11 / PROJECT.md ✓Out-of-Scope.
- **Promote `DESIGN-DECISIONS.md` out of `tmp-references/`** — STATE.md blocker; do during Phase 4 docs work.
- **Update `grammar/README.md` 370→758 line count drift** — STATE.md blocker; do during Phase 4 docs work.

</deferred>

---

*Phase: 2-prove-the-pipeline*
*Context gathered: 2026-05-21*
*Sources: 02-SPEC.md (9 locked requirements), PROJECT.md, REQUIREMENTS.md, ROADMAP.md, STATE.md, 01-CONTEXT.md, 01.5-CONTEXT.md, ADR-phase-1.5-fresh-worktree-verification.md*
