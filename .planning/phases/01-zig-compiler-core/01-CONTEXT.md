# Phase 1: Zig Compiler Core - Context

**Gathered:** 2026-05-19
**Status:** Ready for planning

<domain>
## Phase Boundary

Build the Zig static library `libdeal.a` that lexes and parses both file modes (`.deal` definitions / `.dealx` compositions), produces a JSON-serializable AST for all 19 showcase files, survives 50+ malformed-input cases without panic, and exposes the result through a narrow C ABI consumable from Rust.

**In scope (Phase 1):**
- Lexer covering the full `lexical.ebnf` token set, with parser-driven mode selection
- Recursive-descent parser for `deal.ebnf` (87 productions) plus Pratt expression parser
- Composition parser for `dealx.ebnf` (43 productions) with stack-based tag balancing
- Structured diagnostics with code, severity, primary span, optional secondary spans, optional fix suggestion
- Error recovery to statement / definition / composition-tag boundaries
- `libdeal.a` + `include/deal.h` C ABI: `deal_parse()`, `deal_free()`, `deal_diagnostics_json()`, `deal_ast_json()`, plus thin accessors
- Snapshot tests against `spec/examples/showcase/` (token snapshots, AST JSON snapshots, malformed-input snapshots)
- Rust FFI test harness that links `libdeal.a` and round-trips a showcase file

**Out of scope (Phase 1 — punted to Phase 2+):**
- Semantic analyzer, name resolution, type checking
- DEAL IR v0 (Phase 2)
- Formatter (`deal fmt`), CLI shell (Phase 2 — `deal parse` in Phase 1 is a thin Rust harness only)
- SysML v2 / ReqIF codegen
- LSP server, VS Code extension
- Constraint / path / MQL / `@document:` syntax — already deferred per ADR

</domain>

<decisions>
## Implementation Decisions

### AST Representation & Arena Lifetime
- **D-01:** **Tagged-union AST node.** Single `Node` type with `kind: NodeKind` enum + `union(NodeKind)` payload. Idiomatic Zig per the 0.16.0 langref. Larger nodes than data-oriented but Phase 1 prioritizes correctness over throughput per `deal/README.md` §Key Constraints.
- **D-02:** **Per-handle arena lifetime.** The parse handle returned over the C ABI owns an `ArenaAllocator` that holds the AST, interned strings, source map, and diagnostics. `deal_free(handle)` deallocates the whole arena in one shot. Matches the locked opaque-handle ownership pattern.
- **D-03:** **Unified `NodeKind` enum across `.deal` and `.dealx`.** A single Node hierarchy with kinds for both modes (`def_part`, `def_port`, `comp_tag_open`, `comp_tag_close`, `comp_connect`, `comp_attr`, etc.). Root carries `mode: enum { deal, dealx }`. Shared subtrees: identifiers, literals, unit-literal calls, expressions, doc-comments, annotations. JSON snapshot format is uniform across both file types.
- **D-04:** **AST JSON output shape: compact, schema-versioned, kind-tagged.** Top-level `{ "v": 1, "mode": "deal"|"dealx", "filename": "...", "root": { ... } }`. Each node: `{ "k": "<kind>", "span": [start, end], ...kind-specific fields }`. Short keys keep snapshots reviewable; explicit `"v"` lets Phase 2 detect drift. Spans always present.
- **D-05:** **Adhere to Zig 0.16.0 standards.** All AST, arena, allocator, error-handling, and iterator patterns follow `spec/references/zig-lang/0.16.0/langref.html` — including std.ArrayList allocator-explicit APIs, the post-0.13 `std.heap.ArenaAllocator` shape, std.mem.Allocator interface, and current capture-syntax rules. Downstream researcher should cite the langref section for any non-obvious idiom.

### `.dealx` Mode Switching at `{...}` Boundaries
- **D-06:** **Lexer-on-demand.** Lexer is stateless w.r.t. mode; parser passes the expected mode on every `nextToken(mode)` call. Modes: `.deal_def`, `.dealx_outer`, `.dealx_tag`, `.dealx_expr_brace`. All context lives in the parser, which makes mode transitions visible alongside the grammar production they belong to.
- **D-07:** **Parser-owned open-tag stack with recover-on-mismatch.** Composition parser holds `ArrayList(OpenTag)` of `{ name, span }`. On `[</name>]`, pop and verify. On mismatch, emit a structured diagnostic carrying *both* spans ("opened `[<system>]` here, but `[</subsystem>]` here") and pop anyway to continue parsing — supports the 50+-malformed-cases gate.
- **D-08:** **Inside `{...}` attribute bodies, accept the full `deal.ebnf` expression production.** Unit-literal calls (`V(800)`), member access, identifiers, simple arithmetic, string/number literals. Closing `}` is the natural terminator. Defers nothing that the 19-file showcase doesn't already need; honors the ADR deferrals (constraint/path/MQL syntax) by *not* extending the expression grammar beyond `deal.ebnf`.
- **D-09:** **First-class `comp_connect` node.** `[<connect>] via={...} carrying={...}` gets a dedicated AST kind with typed slots `via_expr: ?NodeIndex` and `carrying_expr: ?NodeIndex` (plus other attributes). The parser has a specialized handler that recognizes `[<connect>]` and treats `via=` / `carrying=` as expression slots, not generic attributes. Encodes CS-4 (physical vs logical separation) at the AST level so Phase 2 IR lowering is trivial.

### C ABI Memory Ownership & Error Path
- **D-10:** **`deal_parse()` always returns a non-null handle.** Even on fatal errors, callers receive a handle whose AST may be partial. Errors are observable through `deal_has_errors(handle)` / `deal_diagnostics_count(handle)` / `deal_diagnostics_json(handle, ...)`. Aligns with the error-recovery goal and the LSP-friendly partial-AST use case downstream.
- **D-11:** **Length-prefixed UTF-8 buffer accessors.** `deal_ast_json(handle, out_ptr, out_len)` and `deal_diagnostics_json(handle, out_ptr, out_len)` write pointer + length into caller slots. Buffers are owned by the handle (freed on `deal_free`). JSON is generated lazily on first access and cached on the handle. No NUL-terminated strings — UTF-8 in source can contain bytes that confuse C-string scanners.
- **D-12:** **Input is source bytes + filename hint.** `deal_parse(source_ptr, source_len, filename_ptr, filename_len) -> *DealHandle`. Filename selects `.deal` vs `.dealx` mode (per SD-17, locked) and populates diagnostic spans. File I/O stays in Rust — supports LSP unsaved buffers and in-memory test inputs.
- **D-13:** **Thread model: per-handle affinity, no global state.** Each `deal_parse()` allocates a fresh arena + handle. Different threads may parse concurrently; a single handle is not shared between threads (LSP serializes per-document access). No mutexes inside `libdeal`. No global allocator state.

### Diagnostic Richness, Spans, and Codes
- **D-14:** **Rich internal data model, simple Phase-1 text renderer.** Internal `Diagnostic` struct carries: `code`, `severity`, `message`, primary span, `secondary_spans: []SpanLabel`, optional `fix_it: ?FixIt`, `notes: []const u8`. JSON output exposes the full structure. Phase 1 CLI rendering is single-line `error[E0101]: <msg> at file:line:col`. Phase 3 LSP gets to use the rich data without re-instrumenting the parser.
- **D-15:** **Span = byte offsets only.** `Span = { start: u32, end: u32 }`. 8 bytes, easy to manipulate, matches Zig std + rustc. A precomputed line-start table (`SourceMap` on the handle) maps byte → 1-based line/column on demand. Renderers convert as needed; AST stays compact.
- **D-16:** **Letter-prefixed numeric codes, stable across versions.** `E0001..E0099` lexer, `E0100..E0299` parser-deal, `E0300..E0399` parser-dealx, `E0400..E0499` recovery / structural, `W0500..W0599` warnings, `H0600..H0699` hints. Once assigned, code-to-meaning never changes. Categories enable triage and grep. Matches rustc / clang convention.
- **D-17:** **Three-tier error-recovery synchronization.** Statement-level: sync to `;` or `,` inside expressions and statements. Definition-level: sync to the closing `}` of the current definition. Composition-level (`.dealx` only): sync to the next `[<` or `[</` token. Maximizes salvageable AST under malformed input and directly serves the "50+ malformed cases survive without panic" gate.

### JSON Emission Conventions
- **D-18:** **AST JSON object fields are emitted in alphabetical order within each payload.** This is a snapshot-stability invariant and applies across all plans (Plans 02 token JSON, 03 .deal AST JSON, 04 .dealx AST JSON, 05 diagnostic JSON, 06 C ABI buffer accessors). Top-level keys are emitted in a fixed canonical order per emitter: token JSON emits `v`, `mode`, `tokens`; AST JSON emits `v`, `mode`, `filename`, `root`; diagnostic JSON is a top-level array (no top-level keys). Within each node's payload AND within each diagnostic object, fields are written in alphabetical order. The Zig 0.16.0 exhaustive `switch (node.payload)` over the tagged union enforces that no NodeKind is silently dropped; adding a NodeKind without an emitter arm is a compile error. Plan 03 establishes the convention; Plans 04/05/06 MUST NOT reorder. Snapshots committed in Plan 03 (15 `.deal` files) MUST remain byte-identical after Plan 04 lands its `comp_*` arms — verified by Plan 04 Task 4.1c acceptance gate.

### Claude's Discretion

The following implementation details were not explicitly discussed and remain at the planner's / researcher's discretion (informed by Zig 0.16.0 langref and the parser implementation guide):

- **Snapshot test infrastructure** — exact JSON snapshot format conventions, snapshot-update flag (`--update-snapshots` vs env var), where snapshots live under `tests/snapshots/`, how the test harness exercises both token-level and AST-level snapshots, malformed-input corpus layout under `tests/malformed/`.
- **`build.zig` structure** — single library target vs split modules, how `tests/` is wired into `zig build test`, whether the Rust FFI harness is invoked from `zig build` or stands alone via cargo.
- **String interning strategy** — whether identifiers are interned into the arena via a hash map or stored as `[]const u8` slices into the original source. Researcher should compare against Zig's own compiler choice in 0.16.0.
- **Token snapshot format** — exact JSON shape (likely `{ "v": 1, "tokens": [{ "k": "...", "span": [..], "text": "..." }] }` mirroring the AST schema).
- **Filename → mode resolution** — pure extension check (`.deal` vs `.dealx`), or also accept an explicit `mode` parameter override on `deal_parse()`.
- **Lexer-on-demand API shape** — whether `nextToken(mode)` returns a `Token` value or fills a caller-provided pointer; whether lookahead requires a separate `peekToken(mode)` or a single shared lexer position.
- **Identifier resolution scope in Phase 1** — Phase 1 ends at parsing; no symbol table. The AST should record raw identifier tokens (not resolved references). Anything resolution-shaped is Phase 2.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Grammar specifications (LOCKED at `0.1.0-draft` per FA / SD-17)
- `spec/grammar/lexical.ebnf` — Shared token set for both `.deal` and `.dealx` modes (758 lines). Lexer must implement this faithfully.
- `spec/grammar/deal.ebnf` — 87 productions for `.deal` definition files (1,679 lines). Pratt expression precedence is documented here.
- `spec/grammar/dealx.ebnf` — 43 productions for `.dealx` composition files (897 lines). Tag balancing rules and inline `{...}` attribute syntax.
- `spec/grammar/README.md` — Grammar README; cross-references the three EBNF files and documents `.deal`/`.dealx` mode selection.

### Locked design decisions
- `spec/grammar/DESIGN-DECISIONS.md` — Single consolidated ADR. 64 LOCKED + 1 CAPTURED (SD-6). Particularly relevant: FA-1..5 (architecture), FS-1..4 (file structure), SD-1..20 (definition syntax), CS-1..16 (composition syntax). Downstream may NOT contradict any LOCKED entry.

### Parser implementation guidance
- `spec/grammar/tmp-references/deal-parser-implementation-guide.html` — Phase 0 deliverable; concrete guidance for the recursive-descent + Pratt parser implementation. Strongly recommended reading before writing parser code.

### Zig language standards
- `spec/references/zig-lang/0.16.0/langref.html` — **MUST read.** Zig 0.16.0 standards and best practices for AST representation, arena allocators (`std.heap.ArenaAllocator`), `std.mem.Allocator` interface, capture syntax, `std.ArrayList` allocator-explicit APIs, error-set handling, packed structs, tagged unions. Any non-obvious idiom in the codebase should cite a section of this file.
- `spec/references/zig-lang/0.16.0/README.md` — Companion notes for the 0.16.0 reference.

### Integration oracle (snapshot truth)
- `spec/examples/showcase/` — 19-file showcase project: 15 `.deal` files in `packages/{vehicle,requirements,use-cases,interfaces}/`, 4 `.dealx` files in `model/`. Phase 1's exit gate is: all 19 tokenize and parse with stable JSON snapshots; lexer reports zero UNKNOWN tokens.
- `spec/examples/showcase/deal.toml` — Project manifest. Phase 1 doesn't parse `deal.toml` directly but the layout assumes its presence.
- `spec/examples/showcase/simulations/deal.sims.toml` — Simulation registry. Out of scope for Phase 1 parsing; informational only.

### Project authority
- `.planning/PROJECT.md` — Tech stack, locked decisions, ingest provenance, ADR precedence.
- `.planning/REQUIREMENTS.md` §Phase 1 — REQ-phase-1-foundation, REQ-phase-1-1-lexer, REQ-phase-1-2-parser-deal, REQ-phase-1-3-parser-dealx, REQ-phase-1-4-error-recovery, REQ-phase-1-5-c-abi, REQ-phase-1-gate.
- `.planning/ROADMAP.md` §"Phase 1: Zig Compiler Core" — Goal statement and success criteria.
- `deal/README.md` — Planned source-tree layout (`src/{lib,lexer,parser,parser_deal,parser_dealx,expr,ast,diagnostics,json}.zig`), Phase 1 milestones, Phase 2 boundary, key constraints (extension-driven mode, opaque-handle ABI, no internal threading).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Directory scaffolding only.** `deal/src/`, `deal/include/`, `deal/tests/showcase/` exist as empty directories. No source files yet. Phase 1 is a greenfield implementation against the planned layout in `deal/README.md`.
- **Showcase corpus exists** at `spec/examples/showcase/` — 19 files ready to drive snapshot tests on day one.
- **Reference material acquired** in Phase 0: SysML v2 JSON schema, KerML JSON schema, OMG textual BNFs, OMG spec PDFs, Zig 0.16.0 langref. No external network fetches needed.

### Established Patterns
- **Layered grammar dependency is unidirectional.** `lexical.ebnf` → `deal.ebnf` → `dealx.ebnf`. `.deal` files never reference `.dealx` constructs. Encoded in the parser as separate parser modules sharing a common AST and a common lexer.
- **Opaque-handle C ABI.** Locked in `deal/README.md` §Key Constraints. Rust never owns Zig memory.
- **No GC.** Both Zig core and Rust frontend are native, manual-memory languages. Arena allocation is the dominant pattern in the Zig side.

### Integration Points
- **Phase 2 boundary is the AST.** Phase 2 builds DEAL IR v0 from the AST emitted by `deal_parse()`. The AST JSON schema (v1) is the contract — bumping it forces a coordinated change. Do not let Phase 2 reach into Zig internals beyond the C ABI.
- **Rust FFI test harness lives in this phase.** It is the minimum proof that the C ABI is consumable from the language ecosystem that will own all integration surfaces (CLI / LSP / Tauri). Keep it tiny — one parse + one diagnostic readback + free.
- **Showcase paths are stable.** `spec/examples/showcase/` will not move during Phase 1. Snapshot tests can refer to it by relative path.

</code_context>

<specifics>
## Specific Ideas

- **Zig 0.16.0 idioms are mandatory, not aspirational.** User explicitly directed: "refer to `~/projects/deal-lang/spec/references/zig-lang/0.16.0/*` for current Zig standards and best practices. Particularly the `*.html`." This is non-negotiable — implementations that use pre-0.16 patterns (older ArrayList APIs, older allocator interfaces, deprecated capture syntax) must be rejected.
- **Snapshot schema versioning matters from day one.** Every JSON snapshot top-level object carries `"v": 1`. Phase 2 IR lowering and Phase 3 LSP both depend on this being explicit and bumpable.
- **AST shape is read by Rust.** Even though Phase 1's harness is minimal, the eventual Rust side will deserialize AST JSON. Keys should be short but unambiguous (`"k"` not `"type"` — `type` is a Rust reserved word in match patterns).

</specifics>

<deferred>
## Deferred Ideas

- **Formatter (`deal fmt`)** — Phase 2.
- **Semantic analysis / name resolution / type checking** — Phase 2.
- **DEAL IR v0 lowering** — Phase 2.
- **SysML v2 / ReqIF codegen** — Phase 2 / Phase 4.
- **CLI shell (`deal parse`, `deal check`, etc. as real CLIs)** — Phase 2 (Phase 1's "deal parse" is just a Rust test harness binary).
- **LSP server, tree-sitter grammar, TextMate grammars, VS Code extension** — Phase 3.
- **Pretty multi-line CLI diagnostic rendering with source-line snippets + carets** — Phase 3 LSP / Phase 6 editor, not Phase 1.
- **Incremental reparse / AST diffing** — Phase 3 LSP.
- **String interning hash map vs raw source slices** — left to research / planning; not blocking.
- **`build.zig` packaging shape** — left to research / planning.
- **`@document:` category and `[<document>]` composition block** — deferred per ADR (Phase 6 doc generation works without them).
- **Constraint / path / MQL expression syntax** — deferred per ADR. Phase 1 expression grammar = `deal.ebnf` expression production, nothing more.
- **Fix `spec/grammar/lexical.ebnf` line 749 summary comment:** change `Keywords:     76` to `Keywords:     85` to match the `_Keyword` production at lines 459-481. This is a comment-only fix — the grammar remains LOCKED at 0.1.0-draft per FA. Deferred to a separate spec PR so the Phase 1 plan-revision (iteration 2) does not touch grammar files. The 85-count is authoritative for Plan 02 (verified: `awk '/^_Keyword[[:space:]]*::=/,/^$/' spec/grammar/lexical.ebnf | grep -oE '"[a-zA-Z@][a-zA-Z_]*"' | sort -u | wc -l` → 85).

</deferred>

---

*Phase: 1-zig-compiler-core*
*Context gathered: 2026-05-19*
