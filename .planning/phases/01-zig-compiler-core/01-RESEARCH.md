# Phase 1: Zig Compiler Core - Research

**Researched:** 2026-05-19
**Domain:** Zig 0.16.0 compiler frontend (lexer + recursive-descent parser + Pratt expressions + composition tag parser + diagnostics + C ABI) for the DEAL language
**Confidence:** HIGH on locked decisions and Zig 0.16.0 idioms (langref + Zig 0.16.0 confirmed installed); MEDIUM on snapshot-test tooling choices (no established Zig snapshot library — recommendation is to hand-roll); MEDIUM on Rust FFI link recipe for macOS arm64 (verified via Cargo docs + Zig langref but not exercised on this exact stack).

## Summary

Phase 1 is greenfield Zig against a fully locked spec. The grammar (130 productions across `lexical.ebnf` / `deal.ebnf` / `dealx.ebnf`), the AST shape (tagged-union with arena lifetime), the C ABI shape (opaque handle + length-prefixed buffers), and the error-recovery strategy (three-tier synchronization) are all decided. Research surfaces the **Zig 0.16.0-specific** idioms — these changed materially in 0.13→0.16 and are NOT optional: the `ArrayList(T)` API is now allocator-explicit (allocator passed at each call, not stored), `callconv(.c)` is lowercase, `b.addLibrary(.{ .linkage = .static, .root_module = b.createModule(...) })` replaces the old `addStaticLibrary`, and `std.StaticStringMap` uses `initComptime`. Test infrastructure decisions and snapshot mechanics are at the planner's discretion — recommend hand-rolled snapshot tests in pure Zig (no external library is standard in the Zig 0.16.0 ecosystem).

**Primary recommendation:** Stand up the build scaffolding and a token-snapshot harness in Wave 0 (week 1), then implement the lexer + .deal parser + .dealx parser + diagnostics + C ABI in five sequential waves, each closing with all-19-showcase snapshot stability for the scope covered. Use the parser implementation guide's wave/milestone structure (already aligned to phase requirements REQ-phase-1-1 through REQ-phase-1-5) as the wave skeleton.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**AST Representation & Arena Lifetime**
- **D-01:** Tagged-union AST node. Single `Node` type with `kind: NodeKind` enum + `union(NodeKind)` payload. Idiomatic Zig per the 0.16.0 langref. Larger nodes than data-oriented but Phase 1 prioritizes correctness over throughput per `deal/README.md` §Key Constraints.
- **D-02:** Per-handle arena lifetime. The parse handle returned over the C ABI owns an `ArenaAllocator` that holds the AST, interned strings, source map, and diagnostics. `deal_free(handle)` deallocates the whole arena in one shot. Matches the locked opaque-handle ownership pattern.
- **D-03:** Unified `NodeKind` enum across `.deal` and `.dealx`. A single Node hierarchy with kinds for both modes (`def_part`, `def_port`, `comp_tag_open`, `comp_tag_close`, `comp_connect`, `comp_attr`, etc.). Root carries `mode: enum { deal, dealx }`. Shared subtrees: identifiers, literals, unit-literal calls, expressions, doc-comments, annotations. JSON snapshot format is uniform across both file types.
- **D-04:** AST JSON output shape: compact, schema-versioned, kind-tagged. Top-level `{ "v": 1, "mode": "deal"|"dealx", "filename": "...", "root": { ... } }`. Each node: `{ "k": "<kind>", "span": [start, end], ...kind-specific fields }`. Short keys keep snapshots reviewable; explicit `"v"` lets Phase 2 detect drift. Spans always present.
- **D-05:** Adhere to Zig 0.16.0 standards per `spec/references/zig-lang/0.16.0/langref.html`.

**`.dealx` Mode Switching at `{...}` Boundaries**
- **D-06:** Lexer-on-demand. Lexer is stateless w.r.t. mode; parser passes the expected mode on every `nextToken(mode)` call. Modes: `.deal_def`, `.dealx_outer`, `.dealx_tag`, `.dealx_expr_brace`.
- **D-07:** Parser-owned open-tag stack with recover-on-mismatch. On `[</name>]`, pop and verify. On mismatch, emit a structured diagnostic carrying both spans and pop anyway to continue parsing.
- **D-08:** Inside `{...}` attribute bodies, accept the full `deal.ebnf` expression production. Closing `}` is the natural terminator.
- **D-09:** First-class `comp_connect` node. Typed slots `via_expr: ?NodeIndex` and `carrying_expr: ?NodeIndex` (plus other attributes).

**C ABI Memory Ownership & Error Path**
- **D-10:** `deal_parse()` always returns a non-null handle. Errors observable through `deal_has_errors(handle)` / `deal_diagnostics_count(handle)` / `deal_diagnostics_json(handle, ...)`.
- **D-11:** Length-prefixed UTF-8 buffer accessors. `deal_ast_json(handle, out_ptr, out_len)` and `deal_diagnostics_json(handle, out_ptr, out_len)` write pointer + length into caller slots. Buffers owned by the handle (freed on `deal_free`). JSON generated lazily on first access and cached on the handle.
- **D-12:** Input is source bytes + filename hint. `deal_parse(source_ptr, source_len, filename_ptr, filename_len) -> *DealHandle`. Filename selects `.deal` vs `.dealx` mode per SD-17.
- **D-13:** Per-handle affinity, no global state. Each `deal_parse()` allocates a fresh arena + handle. Different threads may parse concurrently; a single handle is not shared between threads. No mutexes inside `libdeal`.

**Diagnostic Richness, Spans, and Codes**
- **D-14:** Rich internal data model, simple Phase-1 text renderer. Internal `Diagnostic` struct carries: `code`, `severity`, `message`, primary span, `secondary_spans: []SpanLabel`, optional `fix_it: ?FixIt`, `notes: []const u8`. JSON output exposes full structure. Phase 1 CLI rendering is single-line `error[E0101]: <msg> at file:line:col`.
- **D-15:** Span = byte offsets only. `Span = { start: u32, end: u32 }`. A precomputed line-start table (`SourceMap` on the handle) maps byte → 1-based line/column on demand.
- **D-16:** Letter-prefixed numeric codes, stable across versions. `E0001..E0099` lexer, `E0100..E0299` parser-deal, `E0300..E0399` parser-dealx, `E0400..E0499` recovery / structural, `W0500..W0599` warnings, `H0600..H0699` hints. Once assigned, code-to-meaning never changes.
- **D-17:** Three-tier error-recovery synchronization. Statement-level (`;` or `,`), definition-level (closing `}` of current definition), composition-level (`.dealx` only — next `[<` or `[</`).

### Claude's Discretion

- Snapshot test infrastructure (snapshot format conventions, update flag, layout under `tests/snapshots/` and `tests/malformed/`).
- `build.zig` structure (library target, test wiring, Rust FFI harness integration).
- String interning strategy (hash map vs raw source slices).
- Token snapshot format (likely mirrors AST schema with `"v": 1` envelope).
- Filename → mode resolution (extension check vs explicit override parameter).
- Lexer-on-demand API exact shape (value return vs out-param; lookahead via peek vs shared position).
- Phase 1 stays at parsing; no symbol table or identifier resolution.

### Deferred Ideas (OUT OF SCOPE)

- Formatter (`deal fmt`) — Phase 2.
- Semantic analysis / name resolution / type checking — Phase 2.
- DEAL IR v0 lowering — Phase 2.
- SysML v2 / ReqIF codegen — Phase 2 / Phase 4.
- CLI shell (`deal parse`, `deal check`, etc. as real CLIs) — Phase 2 (Phase 1's "deal parse" is just a Rust test harness binary).
- LSP server, tree-sitter grammar, TextMate grammars, VS Code extension — Phase 3.
- Pretty multi-line CLI diagnostic rendering with source-line snippets + carets — Phase 3 / Phase 6.
- Incremental reparse / AST diffing — Phase 3 LSP.
- String interning hash map vs raw source slices — left to research / planning.
- `build.zig` packaging shape — left to research / planning.
- `@document:` category and `[<document>]` composition block — deferred per ADR.
- Constraint / path / MQL expression syntax — deferred per ADR. Phase 1 expression grammar = `deal.ebnf` expression production, nothing more.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REQ-phase-1-foundation | Build the Zig compiler core: lexer, parser for `.deal` and `.dealx`, error recovery + diagnostics, C ABI boundary. Exit: `deal parse showcase/*.deal showcase/*.dealx` produces AST JSON for all 19 files. | Architectural Responsibility Map + Standard Stack + Architecture Patterns (file-tree layout in `deal/README.md` already mirrored here). |
| REQ-phase-1-1-lexer | Tokenizer implementing full `lexical.ebnf` token set. Mode selection by file extension; `[< >]` activates only in `.dealx`. Token snapshot tests for all 19 showcase files; zero UNKNOWN tokens. | Pattern 1 (Lexer-on-demand) + Pattern 2 (Comptime keyword map via `StaticStringMap.initComptime`) + Snapshot strategy. |
| REQ-phase-1-2-parser-deal | Recursive-descent for 87 `deal.ebnf` productions + Pratt for expressions. AST uses arena allocation. All 15 `.deal` showcase files parse; AST JSON snapshots stable. | Pattern 3 (recursive-descent + Pratt) + Pratt precedence table extracted from `deal.ebnf` Section 19. |
| REQ-phase-1-3-parser-dealx | 43 `dealx.ebnf` productions with stack-based tag balancing. `via={...}` / `carrying={...}` inline blocks. All 4 `.dealx` files parse. | Pattern 4 (composition parser, parser-owned open-tag stack) + D-08 (`{}` re-enters `.deal` expression production) + D-09 (first-class `comp_connect` node). |
| REQ-phase-1-4-error-recovery | Survive malformed input gracefully. Three-tier sync. Structured diagnostics with code + span + optional fix. 50+ recovery test cases. | Pattern 5 (three-tier sync sets, collector pattern) + Code-namespace map from D-16 + Malformed corpus strategy in Snapshot section. |
| REQ-phase-1-5-c-abi | Expose `libdeal.a` + `deal.h`. `deal_parse()`, `deal_free()`, `deal_diagnostics_json()`, `deal_ast_json()`, accessors. Rust FFI harness parses a showcase file and reads back the AST. | Pattern 6 (C ABI, `extern struct` + `callconv(.c)` lowercase + opaque handle) + Rust FFI section (`build.rs` cargo directives for static link). |
| REQ-phase-1-gate | All 19 files tokenize and parse; AST JSON snapshots stable; 50+ malformed cases survive without panic; C ABI proven with Rust harness. | Validation Architecture section maps every gate criterion to an automated command. |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Source-byte tokenization (lexer) | Zig static library (`libdeal.a`) | — | Pure computation; no I/O. Locked tech stack — Zig core. |
| Recursive-descent parsing for `.deal` (87 productions) | Zig static library | — | Stateless w.r.t. mode (D-06); arena-allocated AST (D-02). |
| Composition parsing for `.dealx` (43 productions, tag balancing) | Zig static library | — | Parser-owned open-tag stack (D-07); inline `{...}` re-enters expression grammar (D-08). |
| Diagnostic collection + structured emission | Zig static library | — | Owned by parse handle's arena (D-02, D-14); JSON shape stable (D-04, D-16). |
| AST JSON serialization | Zig static library | — | Lazy generation, cached on handle (D-11). Snapshot oracle. |
| Source-byte read from disk | Rust FFI test harness | — | Filename → mode; Zig receives bytes + filename hint (D-12). |
| Static-library link + binary build | Zig build system (`build.zig`) | Rust `build.rs` (consumer) | `b.addLibrary(.{ .linkage = .static })` in Zig; Rust harness links via `cargo::rustc-link-lib=static=deal`. |
| `deal.h` header generation | Zig build system | — | Hand-written `include/deal.h` is recommended (see Common Pitfalls: `-femit-h` discussion). |
| Snapshot regression detection (token / AST / malformed) | Zig test harness (`zig build test`) | — | Pure-Zig file-diff against committed `tests/snapshots/*.json`. |
| Rust FFI smoke: load `libdeal.a`, call `deal_parse`, read JSON, free | Rust binary (cargo test) | — | `deal-ffi` crate; closes REQ-phase-1-5 gate criterion. |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Zig | 0.16.0 (installed: `/opt/homebrew/bin/zig`, verified via `zig version`) | Compiler implementation language | LOCKED. Tagged-union AST, arena allocator, `extern struct` for C ABI, comptime keyword tables. [VERIFIED: zig version output] |
| `std.heap.ArenaAllocator` | std-lib 0.16.0 | Per-handle arena lifetime (D-02) | Standard CLI/compiler pattern per langref §"Choosing an Allocator" — "free everything at once at the end". Backed by `std.heap.page_allocator`. [CITED: langref.html L13575-13591] |
| `std.heap.page_allocator` | std-lib 0.16.0 | Backing allocator for the parse-handle arena | Recommended in CLI-allocation example. Single OS-level alloc per parse; arena handles all sub-allocations. [CITED: langref.html L13578] |
| `std.testing.allocator` | std-lib 0.16.0 | Leak detection inside unit tests | Default test runner reports leaks. Use in *test* allocations, not in production parse-handle code. [CITED: langref.html L1810-1849] |
| `std.ArrayList(T)` | std-lib 0.16.0 (allocator-explicit shape) | Resizable collections (open-tag stack, diagnostic list, token buffers) | **0.16.0 API:** `var list: std.ArrayList(T) = .empty; try list.append(allocator, item); list.deinit(allocator);` — allocator passed at each call, NOT stored. This is a breaking change from pre-0.13. [CITED: langref.html L1813, L1815] |
| `std.StaticStringMap(V)` | std-lib 0.16.0 | Comptime keyword/contextual-name lookup tables | `initComptime()` is the supported entry point in 0.16.0 (runtime `init()` was removed). Comptime perfect hash, zero runtime init cost. [CITED: ziglang/zig issue #19936, PR #21498] |
| `std.json.Stringify` | std-lib 0.16.0 | AST JSON emission via `WriteStream` | Works for plain types; **known limitations** with intrusive interfaces (issue #25233, #16519). Sufficient for the simple tagged-union → flat JSON shape DEAL needs. [CITED: ziglang/zig issue #25233] |
| `std.unicode.utf8ValidateSlice` | std-lib 0.16.0 | Validate input source is well-formed UTF-8 before lexing | Avoid hand-rolling UTF-8 validation. Validate once at the top of `deal_parse`; emit `E0001` if invalid; do not panic. [ASSUMED: std-lib API present across recent versions; verify name when implementing] |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `std.heap.DebugAllocator` | 0.16.0 | Optional GPA for parsing in debug builds | Only if profiling allocations matters in Phase 1; default to `page_allocator`+arena. [CITED: langref.html L13550] |
| `std.testing.expectEqualStrings` | 0.16.0 | Snapshot comparison in unit tests | Diff token snapshot / AST snapshot against committed file. [ASSUMED: present in 0.16.0 — verify when scaffolding] |
| `std.fs.cwd().readFileAlloc` | 0.16.0 | Read showcase files in test harness | For loading `tests/showcase/*.deal` content during snapshot tests. [ASSUMED — verify exact name in 0.16.0; alternative `std.fs.cwd().openFile(...).readToEndAlloc(...)`] |
| Rust 1.93.0 stable | system | FFI test harness | Verified installed: `rustc 1.93.0 (254b59607 2026-01-19)`. Use std-only — no extra crates needed for the Phase 1 smoke. [VERIFIED: `rustc --version`] |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Tagged-union AST (D-01) | Data-oriented (NodeIndex + side tables, Zig's own compiler style) | Better cache locality, harder to write. **REJECTED by D-01.** |
| `std.heap.ArenaAllocator(page_allocator)` | `std.heap.c_allocator` (requires libc link) | libc link is unnecessary for the static library and complicates Rust FFI builds. Stay pure-Zig. |
| `std.json.Stringify` | Hand-written JSON writer over `std.Io.Writer` | Hand-rolling gives deterministic output ordering for snapshot stability — but adds maintenance cost. Recommendation: try `std.json.Stringify` first; fall back to a thin custom writer if intrusive-interface bugs bite. The flat node shape DEAL uses is unlikely to trigger the known issues. |
| `zig build-lib -femit-h` to generate `deal.h` | Hand-written `include/deal.h` | `-femit-h` historically generated `.h` files for `export fn` declarations; behavior across 0.16.0 is **not verified in the langref** (the langref only documents `b.installHeadersDirectory` and shows hand-written `mathtest.h` consumed by C). Hand-writing `deal.h` is safer and gives explicit control over ABI struct layout (D-14 / D-15 require careful `extern struct` shape). [ASSUMED] |
| Snapshot test library (e.g. an external "insta-for-Zig") | Hand-rolled file-diff in `zig test` | **No mature external snapshot library exists in the Zig 0.16.0 ecosystem.** Hand-roll: read expected JSON, compare with `expectEqualStrings`, emit a `.actual` file on mismatch, document an `--update-snapshots` workflow via env var. |

**Installation:** No external packages. Verify Zig + Rust:
```bash
zig version                           # expect: 0.16.0
rustc --version                       # expect: 1.93.0 (or compatible 1.83+)
```

**Version verification:**
```bash
# Zig is the only external runtime. Verified at research time:
$ zig version
0.16.0
$ rustc --version
rustc 1.93.0 (254b59607 2026-01-19)
$ uname -ms
Darwin arm64
```
[VERIFIED via local shell commands at /Users/dunnock/projects/deal-lang/, 2026-05-19]

## Package Legitimacy Audit

Phase 1 installs **zero external packages**. Both Zig and Rust use only their respective standard libraries. No `build.zig.zon` dependencies in Phase 1. No `Cargo.toml` dependencies for the FFI harness beyond std.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| (none) | — | — | — | — | n/a | n/a |

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

*Phase 2 will introduce Rust frontend dependencies (clap, etc.); audit at that time.*

## Architecture Patterns

### System Architecture Diagram

```
                  ┌─────────────────────────────────────────────────────┐
                  │                  Rust FFI test harness              │
                  │  (cargo test in tests/ffi/ — Phase 1 minimum)       │
                  │                                                     │
                  │  read file → bytes + filename → deal_parse(...)     │
                  │                                  │                  │
                  │              deal_ast_json(...)  ▼                  │
                  │  deal_diagnostics_json(...) read back               │
                  │                                  │                  │
                  │                          deal_free(handle)          │
                  └──────────────────────────────────┼──────────────────┘
                                                     │
                                       extern "C" / callconv(.c)
                                                     │
        ┌────────────────────────────────────────────▼────────────────────────────────────┐
        │                       libdeal.a    (Zig static library)                         │
        │                                                                                  │
        │   src/lib.zig (C ABI exports)                                                    │
        │       │                                                                          │
        │       ▼                                                                          │
        │   ┌──────────────────┐                                                           │
        │   │  Parse handle    │  ← arena owns: AST, source map, interned strings,        │
        │   │  (opaque)        │    diagnostic list, cached AST JSON, cached diag JSON    │
        │   │  ArenaAllocator  │                                                           │
        │   └────────┬─────────┘                                                           │
        │            │                                                                     │
        │            ▼                                                                     │
        │   src/parser.zig  ◄── chooses .deal/.dealx by filename extension (D-12)          │
        │       │       │                                                                  │
        │       │       │ (drives mode on every nextToken call — D-06)                     │
        │       │       ▼                                                                  │
        │       │   src/lexer.zig  ── stateless, accepts a mode enum per call             │
        │       │                                                                          │
        │       ├──▶ src/parser_deal.zig  (87 productions, recursive descent)             │
        │       │       └──▶ src/expr.zig (Pratt; precedence table from deal.ebnf §19)    │
        │       │                                                                          │
        │       ├──▶ src/parser_dealx.zig (43 productions; open-tag stack — D-07)         │
        │       │       └──▶ ConnectTag handler (typed via= / carrying= slots — D-09)     │
        │       │            └──▶ re-enters deal expression production inside { } (D-08)  │
        │       │                                                                          │
        │       └──▶ src/diagnostics.zig                                                   │
        │              ↑ collector pattern: parser appends, never aborts                  │
        │                                                                                  │
        │   ┌──────────────────┐     ┌──────────────────┐                                  │
        │   │  src/ast.zig     │ ──▶ │  src/json.zig    │ ──▶ lazy AST JSON buffer        │
        │   │  tagged union    │     │  std.json.Stringify or hand-rolled writer          │
        │   │  (D-01, D-03)    │     │  (deterministic key order!)                        │
        │   └──────────────────┘     └──────────────────┘                                  │
        └──────────────────────────────────────────────────────────────────────────────────┘
                                                     ▲
                                                     │ reads
                                          ┌──────────┴───────────────┐
                                          │ tests/showcase/  (symlink to                │
                                          │ spec/examples/showcase/  — 15 .deal +       │
                                          │ 4 .dealx)                                   │
                                          │ tests/snapshots/ (committed JSON)           │
                                          │ tests/malformed/ (≥50 corrupted inputs)     │
                                          └─────────────────────────────────────────────┘
```

**Reader walkthrough (primary use case):** Rust harness reads `vehicle.dealx` from disk → passes bytes + filename to `deal_parse` → Zig allocates a fresh arena, initializes a parser, opens the source through `parser.zig` → parser inspects the filename's `.dealx` extension and dispatches to `parser_dealx.zig` → parser_dealx pulls tokens from `lexer.zig` with `mode = .dealx_outer` → at each `[<` opens a tag, pushes the tag name onto its open-tag stack, dispatches based on tag identifier (system, subsystem, connect, expose, etc.) → when it hits `via={` inside `[<connect>]`, it asks the lexer for tokens in `mode = .dealx_expr_brace` which causes it to re-enter the full `deal.ebnf` expression production (D-08) for the InlineObjectLiteral → on `}` it pops back to `.dealx_tag` mode → on `[</subsystem>]` it pops the open-tag stack and checks the name matches → at EOF it returns the parse handle to Zig's exported `deal_parse` → Rust harness calls `deal_ast_json` (Zig lazily serializes the AST into a buffer in the same arena, returns ptr+len), reads back, calls `deal_free` which drops the arena.

### Recommended Project Structure

(Follows `deal/README.md` §"Planned Layout" — already locked.)

```
deal/
├── build.zig                          # entry: lib + tests + Rust harness orchestration
├── build.zig.zon                      # empty for Phase 1 (no external Zig packages)
├── include/
│   └── deal.h                         # hand-written C ABI header
├── src/
│   ├── lib.zig                        # C ABI exports (extern fn ..., extern struct ...)
│   ├── lexer.zig                      # shared tokenization with mode-on-demand
│   ├── parser.zig                     # parser driver — mode dispatch, error collector
│   ├── parser_deal.zig                # .deal definition grammar (87 productions)
│   ├── parser_dealx.zig               # .dealx composition grammar (43 productions)
│   ├── expr.zig                       # Pratt expression parser (shared by both)
│   ├── ast.zig                        # tagged-union Node + payload structs
│   ├── diagnostics.zig                # structured Diagnostic + SpanLabel + FixIt
│   ├── source_map.zig                 # byte → 1-based line/col (lazy line-start table)
│   ├── json.zig                       # AST + diagnostics → JSON (snapshot oracle)
│   └── keywords.zig                   # StaticStringMap-based keyword tables
├── tests/
│   ├── showcase/                      # symlink to spec/examples/showcase (already present)
│   ├── snapshots/                     # committed .json files (token + AST + diag corpora)
│   ├── malformed/                     # ≥50 mutated showcase variants for recovery tests
│   ├── unit/                          # per-module pure-Zig tests
│   └── ffi/                           # Rust harness (cargo workspace, links libdeal.a)
│       ├── Cargo.toml
│       ├── build.rs                   # invokes `zig build` and emits cargo: link directives
│       └── tests/parse.rs             # the Phase 1 gate criterion #5
└── README.md
```

**Justification for additions beyond README's layout:**
- `src/source_map.zig` — `Span = { start: u32, end: u32 }` (D-15) is byte-only; the line-start table is a separate concern and small enough to deserve its own file.
- `src/keywords.zig` — `StaticStringMap.initComptime` tables for global keywords and contextual names (composition tag identifiers, header field keys, satisfy-block sub-keywords). Centralizing avoids duplication across parser_deal / parser_dealx.
- `tests/unit/` vs `tests/snapshots/` — unit tests cover lexer rule edge cases (DOC vs BLOCK comment disambiguation, template `${}`, etc.); snapshot tests cover end-to-end stability. Both are needed.
- `tests/ffi/` — Rust harness must live somewhere; co-locating under `deal/tests/ffi/` keeps it under the Zig project's authority. (The parser implementation guide places it under a separate `crates/deal-ffi/` workspace at the repo root — that's Phase 2 territory; for Phase 1 a self-contained `tests/ffi/` is simpler.)

### Pattern 1: Lexer-on-demand with mode parameter (D-06)

**What:** Lexer is a thin stateless scanner. Parser passes `mode: Mode` on every `next(mode)` or `peek(mode)` call. The mode determines which character sequences the lexer accepts as tokens.

**When to use:** Whenever the same source bytes mean different tokens in different syntactic contexts (e.g. `[<` is `TAG_OPEN` only in `.dealx`; in `.deal` it should be `LBRACKET` + `LT`).

**Modes (the four locked in D-06):**
- `.deal_def` — definition mode (used in `.deal` files and inside inline definitions in `.dealx`)
- `.dealx_outer` — top-level `.dealx` content; recognizes `[<` and `[</`
- `.dealx_tag` — inside an open tag pair `[<name ... >]` or `[<name ... />]`; recognizes `>]`, `/>]`, `=`, prop names
- `.dealx_expr_brace` — inside `{...}` attribute body in a tag (e.g. `via={...}`); accepts the full `deal.ebnf` expression production

**Example:**
```zig
// Source: deal-parser-implementation-guide.html §4 + Zig 0.16.0 idioms
pub const Mode = enum { deal_def, dealx_outer, dealx_tag, dealx_expr_brace };

pub const Token = struct {
    tag: Tag,
    span: Span,           // byte offsets only (D-15)
    // No owned text — slice into source on demand via span
};

pub const Lexer = struct {
    source: []const u8,
    pos: u32,              // current byte offset

    pub fn next(self: *Lexer, mode: Mode) Token { ... }
    pub fn peek(self: *Lexer, mode: Mode) Token { ... }  // pure — no state mutation
};
```

The parser owns mode context. Mode never lives on the lexer.

### Pattern 2: Comptime keyword and contextual-name tables

**What:** Use `std.StaticStringMap` with `initComptime` to map keyword strings to token tags. Multiple tables — one for globally reserved keywords (37 per the grammar README), one per contextual name set (header fields, tag names, satisfy sub-blocks).

**When to use:** Any time the lexer or parser needs "is this IDENT actually a keyword/contextual-name?" lookup. Single comptime hash; zero runtime init.

**Example:**
```zig
// Source: andrewkelley.me string-matching post; ziglang/zig issue #19936
pub const Tag = enum { kw_part, kw_def, kw_port, kw_attribute, /* ... */, ident };

pub const global_keywords = std.StaticStringMap(Tag).initComptime(.{
    .{ "part",      .kw_part },
    .{ "def",       .kw_def },
    .{ "port",      .kw_port },
    .{ "attribute", .kw_attribute },
    // ... 37 globally reserved keywords from lexical.ebnf Section 5
});

// In lexer: after scanning an IDENT, check the keyword table
if (global_keywords.get(slice)) |kw_tag| return Token{ .tag = kw_tag, .span = sp };
return Token{ .tag = .ident, .span = sp };
```

[CITED: andrewkelley.me "String Matching based on Compile Time Perfect Hashing in Zig"; ziglang/zig issues for `initComptime` migration.]

### Pattern 3: Recursive-descent + Pratt expressions (87 productions)

**What:** One Zig function per grammar production (lexical structure of the EBNF). Expressions use Pratt parsing — a single `parseExpression(min_bp: u8)` loop with operator binding-power lookups.

**When to use:** Default for the entire `.deal` parser. The 87 productions and their FIRST sets are already documented in `deal.ebnf` — the implementation is mechanical translation.

**Pratt precedence table (extracted from `spec/grammar/deal.ebnf` Section 19, lines 1549-1606):**

| Level | Operators (left → right tokens) | Left BP | Right BP | Kind |
|-------|---------------------------------|---------|----------|------|
| 1 (lowest) | `OR` | 10 | 11 | Binary left-assoc |
| 2 | `AND` | 20 | 21 | Binary left-assoc |
| 3 | `==`  `!=` | 30 | 31 | Binary left-assoc |
| 4 | `>=`  `<=`  `>`  `<` | 40 | 41 | Binary left-assoc |
| 5 | `+`  `-` | 50 | 51 | Binary left-assoc |
| 6 | `*`  `/` | 60 | 61 | Binary left-assoc |
| 7 | `-`  `NOT`  `!` (prefix) | — | 70 | Unary right |
| 8 (highest) | `.` (member access) and `(` (call) | 80 | — | Postfix left |

`AND` and `OR` are **keywords** in `lexical.ebnf` (capitalized), not symbolic. Verify the lexer emits them as the right token type. (See `dealx.ebnf` CriteriaBlock example: `actualMaxTemp <= REQ_BAT_003.maxTemp AND actualMinTemp >= REQ_BAT_003.minTemp`.)

**Example:**
```zig
// Source: deal-parser-implementation-guide.html §6 + langref idioms
pub fn parseExpression(self: *Parser, min_bp: u8) !*Node {
    var left = try self.parsePrefix();
    while (true) {
        const tok = self.peek();
        const bp = leftBindingPower(tok.tag) orelse break;
        if (bp < min_bp) break;

        if (tok.tag == .dot) {
            _ = self.advance();
            const member = try self.expect(.ident);
            left = try self.makeMemberAccess(left, member);
        } else if (tok.tag == .l_paren) {
            _ = self.advance();
            const args = try self.parseArgList();
            _ = try self.expect(.r_paren);
            left = try self.makeCall(left, args);
        } else {
            _ = self.advance();
            const right = try self.parseExpression(rightBindingPower(tok.tag));
            left = try self.makeBinary(left, tok.tag, right);
        }
    }
    return left;
}
```

**Organization:** Recommend one Zig file per major production cluster, NOT one giant `parser.zig`:
- `parser_deal.zig` — file structure (DealFile, HeaderBlock, ImportDecl, ExportDecl), definitions dispatch (`Definition` and the 14 element types), member declarations, type annotations, multiplicity, modifiers, annotations.
- `expr.zig` — Pratt loop + binding-power table + primary expression + function call.

Zig's own compiler splits `Ast.zig` and per-area parser functions across multiple files for the same reason.

### Pattern 4: Composition parser with parser-owned open-tag stack (D-07)

**What:** The `.dealx` parser holds `std.ArrayList(OpenTag)` where `OpenTag = struct { name: []const u8, span: Span }`. On `[<name>]` push; on `[</name>]` pop and verify name matches; on mismatch emit `E0301` with both spans and pop anyway.

**When to use:** Whenever `parser_dealx.zig` enters or exits a paired tag (`system`, `subsystem`, `traceability`, `satisfy`, `validate`). Self-closing tags (`[<expose ... />]`, `[<connect ... />]`, `[<allocate ... />]`, `[<TypeName ... />]`) do NOT push.

**Example:**
```zig
const OpenTag = struct { name: []const u8, span: Span };
const Parser = struct {
    open_tags: std.ArrayList(OpenTag) = .empty,    // 0.16.0 allocator-explicit
    arena: std.mem.Allocator,
    diagnostics: *std.ArrayList(Diagnostic),
    // ...

    fn pushTag(self: *Parser, name: []const u8, span: Span) !void {
        try self.open_tags.append(self.arena, .{ .name = name, .span = span });
    }

    fn popTag(self: *Parser, expected_name: []const u8, close_span: Span) !void {
        if (self.open_tags.items.len == 0) {
            try self.diagnostics.append(self.arena, .{
                .code = "E0301",
                .severity = .err,
                .message = "Unmatched close tag",
                .span = close_span,
                .secondary_spans = &.{},
            });
            return;
        }
        const top = self.open_tags.pop();
        if (!std.mem.eql(u8, top.name, expected_name)) {
            try self.diagnostics.append(self.arena, .{
                .code = "E0302",
                .severity = .err,
                .message = "Mismatched tag close",
                .span = close_span,
                .secondary_spans = &.{ .{ .span = top.span, .label = "opened here" } },
            });
            // Pop anyway to continue (D-07).
        }
    }
};
```

### Pattern 5: Three-tier error recovery (D-17)

**What:** A `Diagnostic` is appended to the parser's collector list on every recoverable error. The parser then "synchronizes" by skipping tokens until a known boundary token is found, then resumes parsing at that boundary's grammar production.

**Sync sets (concrete tokens to skip-to, by tier):**

| Tier | Used in | Sync to (FOLLOW set) |
|------|---------|----------------------|
| Statement-level | Inside member declarations, statement lists, expressions | `;` or `,` or matching `}` or `)` or `]` (whichever closes the current frame) |
| Definition-level | Top of `_FileContent` loop / element body | Definition-keyword start (`part`, `port`, `action`, `state`, `attribute`, `item`, `interface`, `connection`, `flow`, `allocation`, `requirement`, `constraint`, `need`, `use`) OR `export` OR `}` at definition's matching depth OR EOF |
| File-level / Composition-level (`.dealx` only) | Top of `_DealxContent` loop or inside a composition block | `[<` (next tag open) OR `[</` (close tag) OR `package` / `import` / `export` / EOF |
| Inside visibility wrapper `public ( ... )` | Member parser | `)` |
| Inside satisfy body | SatisfyBlock parser | `criteria`, `evidence`, `compute`, `gap` keywords OR `[</satisfy>]` |
| Inside expression | Pratt loop | `;` `}` `)` `]` |

These sets match the table in §11 of `deal-parser-implementation-guide.html`. Implement as small helper functions: `fn syncToStatement(self: *Parser) void`, `fn syncToDefinition(self: *Parser) void`, `fn syncToTag(self: *Parser) void`.

**Collector pattern:** The parser owns `diagnostics: std.ArrayList(Diagnostic)` and appends on every error. It NEVER returns a fatal `error.ParseFailed` from the C ABI surface (D-10) — instead it returns whatever partial AST it managed to build, with the diagnostic list populated.

### Pattern 6: C ABI with opaque handle (D-10, D-11, D-13)

**What:** Zig exports `deal_parse`, `deal_free`, `deal_has_errors`, `deal_diagnostics_count`, `deal_diagnostics_json`, `deal_ast_json`. The handle is `*anyopaque` (opaque pointer) externally; internally it points to a Zig struct that owns the arena.

**`callconv(.c)` is lowercase in Zig 0.16.0.** [CITED: langref.html L5405, L10808, L14553 — all use `callconv(.c)`.] Older Zig used `callconv(.C)`; this was renamed. Do NOT use uppercase `.C` in 0.16.0 — it will fail to compile.

**Example:**
```zig
// Source: parser implementation guide §3 + Zig 0.16.0 langref §extern struct + §callconv
const DealHandle = struct {
    arena: std.heap.ArenaAllocator,
    ast_root: ?*Node,                 // null if total lex failure
    diagnostics: std.ArrayList(Diagnostic),
    source: []const u8,               // copied into arena on parse
    filename: []const u8,
    cached_ast_json: ?[]const u8 = null,
    cached_diag_json: ?[]const u8 = null,
};

pub export fn deal_parse(
    source_ptr: [*]const u8, source_len: usize,
    filename_ptr: [*]const u8, filename_len: usize,
) callconv(.c) ?*anyopaque {
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    const allocator = arena.allocator();

    const handle = allocator.create(DealHandle) catch {
        // page_allocator alloc-fail is the only path that can produce null (D-10 allows
        // null only on alloc fail; everything else returns a handle).
        arena.deinit();
        return null;
    };
    handle.arena = arena;
    handle.source = allocator.dupe(u8, source_ptr[0..source_len]) catch unreachable;
    handle.filename = allocator.dupe(u8, filename_ptr[0..filename_len]) catch unreachable;
    handle.diagnostics = .empty;
    handle.ast_root = null;
    handle.cached_ast_json = null;
    handle.cached_diag_json = null;

    // Parse — never panics; appends diagnostics on errors.
    handle.ast_root = parser.parseFile(handle) catch null;
    return @ptrCast(handle);
}

pub export fn deal_free(handle_ptr: *anyopaque) callconv(.c) void {
    const handle: *DealHandle = @ptrCast(@alignCast(handle_ptr));
    var arena = handle.arena;            // copy the arena out; freeing it invalidates handle
    arena.deinit();
}

pub export fn deal_ast_json(
    handle_ptr: *anyopaque,
    out_ptr: *[*]const u8,
    out_len: *usize,
) callconv(.c) bool {
    const handle: *DealHandle = @ptrCast(@alignCast(handle_ptr));
    if (handle.cached_ast_json == null) {
        const buf = json.emitAst(handle.arena.allocator(), handle.ast_root) catch return false;
        handle.cached_ast_json = buf;
    }
    out_ptr.* = handle.cached_ast_json.?.ptr;
    out_len.* = handle.cached_ast_json.?.len;
    return true;
}
```

**Extern struct for diagnostics (if exposing via accessor):** Per `deal-parser-implementation-guide.html` §3, a `DealDiagnostic` `extern struct` with fixed-layout fields is a viable alternative to JSON for high-frequency reads. **For Phase 1, JSON-only is sufficient** (D-11 locks length-prefixed UTF-8 buffer accessors) — keep the surface narrow.

### Anti-Patterns to Avoid

- **Lexer that owns mode state.** Violates D-06. Mode lives on the parser; lexer takes it as a parameter each call.
- **Hand-rolled UTF-8 validation.** Use `std.unicode.utf8ValidateSlice` or its equivalent in 0.16.0. Hand-rolling is a known footgun for combining characters and surrogate pairs.
- **Returning Zig error sets across the C ABI.** All `deal_*` functions return `bool` / `?*anyopaque` / primitive types. NEVER `!T`. Errors are reported through the diagnostic list, not the function-return error channel.
- **Allocating diagnostic strings outside the arena.** Every string in a `Diagnostic` (message, code, secondary labels, fix-it text) must be allocated from the handle's arena. Otherwise `deal_free` won't free it.
- **`callconv(.C)` (uppercase).** Use `callconv(.c)` (lowercase) in 0.16.0. [CITED: langref.html L5405, L10808, L14553]
- **`std.ArrayList(T).init(allocator)` stored-allocator style.** Removed in 0.16.0. Use `.empty` initializer and pass allocator at each call site. [CITED: langref.html L1813-1815]
- **Returning NUL-terminated C strings.** D-11 mandates length-prefixed buffers. UTF-8 source bytes can contain NUL inside string literals.
- **Storing tokens in a `[]Token` slab.** Tokens are produced on-demand by the lexer; storing the full list violates the "lexer-on-demand" intent of D-06 and balloons memory. Parser holds at most a one-token lookahead buffer.
- **Emitting non-deterministic JSON key order.** Snapshot tests will become non-reproducible. Either use a custom writer with explicit field order, or verify `std.json.Stringify` emits in declaration order for tagged unions. If unsure, hand-roll the JSON for the top-level `Node` shape.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| UTF-8 validation of source bytes | Custom byte-walker checking continuation bits | `std.unicode.utf8ValidateSlice(source)` | Combining characters, surrogate pairs, overlong encodings are all edge cases the std-lib already covers. |
| Keyword recognition | `if (std.mem.eql(u8, s, "part")) ...` chains | `std.StaticStringMap(Tag).initComptime(.{...})` | 37 globally reserved keywords + multiple contextual sets = comptime perfect hash is the right shape. [CITED: andrewkelley.me] |
| Byte-offset → line/column | Per-token line counter in the lexer | Lazy `SourceMap` line-start table on the handle (D-15) | Phase 1 only needs line/col when emitting diagnostics — computing it lazily keeps the lexer hot path cheap. |
| Generic tagged-union switch dispatch | Long manual `if (node.kind == ...)` chains | Zig's exhaustive `switch (node.*)` on a `union(enum)` | Zig's compiler enforces case exhaustiveness — adding a new node kind without handling it becomes a compile error, not a runtime bug. [CITED: langref.html L5184 "switch on tagged union"] |
| Arena management around the C ABI | Manual `defer alloc.destroy(x)` per allocation | `std.heap.ArenaAllocator` per handle; `arena.deinit()` in `deal_free` | Locked by D-02. The std-lib arena is the standard CLI/compiler pattern. [CITED: langref.html L13575] |
| JSON writing | Building strings with `std.fmt.allocPrint` and concatenation | `std.json.Stringify.WriteStream` (or a thin custom writer if intrusive-interface bugs bite — issue #25233) | Stream-based writer handles escaping and unicode automatically. |
| Snapshot test diff | `if (!std.mem.eql(...)) ...` | `std.testing.expectEqualStrings(expected, actual)` | Built-in test helper emits a readable diff on mismatch. |

**Key insight:** Zig's standard library and Zig 0.16.0 idioms cover ~90% of the boilerplate. The phase plan should treat any "I'll write a quick X" instinct as a check-the-std-lib-first signal.

## Runtime State Inventory

> Phase 1 is **greenfield** — `deal/src/` and `deal/include/` are empty directories. There is no existing runtime state to migrate. This section intentionally records "none found" so the planner doesn't infer migration work that doesn't exist.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — Phase 1 creates no persistent storage (parse handle's arena is in-process only) | None |
| Live service config | None — no external service is involved in Phase 1 | None |
| OS-registered state | None — no OS hooks (no daemons, no schedulers, no installed binaries) | None |
| Secrets/env vars | None — no secrets or env vars consumed | None |
| Build artifacts | None — `deal/src/` is empty; nothing to invalidate | The wave-0 task creates `build.zig` and `build.zig.zon` from scratch |

**Verified by:** `ls deal/src/ deal/include/ deal/tests/` returns directories with only the showcase symlink at `tests/showcase → ../../spec/examples/showcase`. No stale state to clean.

## Common Pitfalls

### Pitfall 1: Treating `.deal` and `.dealx` as separate lexers

**What goes wrong:** Implementer creates two `Lexer` types or two token-tag enums, then has to dual-maintain every change.

**Why it happens:** The grammar files are split (`deal.ebnf` vs `dealx.ebnf`); the natural inclination is to mirror that split in code.

**How to avoid:** D-06 mandates ONE stateless lexer with a `Mode` parameter. The token-tag enum is unified — `TAG_OPEN` and `TAG_CLOSE` are tokens that the lexer emits only when called with `mode = .dealx_outer` or `.dealx_tag`. In `.deal_def` mode, `[` and `<` are independent tokens.

**Warning signs:** Two files named `lexer_deal.zig` and `lexer_dealx.zig`. A `Lexer.mode` field. Token enums diverging.

### Pitfall 2: `callconv(.C)` uppercase in 0.16.0

**What goes wrong:** Code that compiled in Zig 0.13 fails to build in 0.16.0 with "expected enum literal".

**Why it happens:** Zig renamed the calling-convention enum literal from `.C` to `.c` in the 0.14/0.15 reshuffle. Pre-existing tutorials and the parser implementation guide both use the old form in places.

**How to avoid:** Search the codebase for `callconv(.C)` and replace with `callconv(.c)`. The langref examples in 0.16.0 all use lowercase.

**Warning signs:** Build error "no member named C". The langref (L5405, L10808, L14553) consistently uses lowercase.

### Pitfall 3: `ArrayList.init(allocator)` storing the allocator

**What goes wrong:** Code uses `var list = std.ArrayList(T).init(alloc); try list.append(item);` — fails to compile in 0.16.0 because `append` now requires the allocator to be passed in.

**Why it happens:** Pre-0.13 Zig stored the allocator on the list. 0.16.0 removed that — the allocator is passed on each call.

**How to avoid:** Use the documented 0.16.0 pattern: `var list: std.ArrayList(T) = .empty; try list.append(allocator, item); list.deinit(allocator);` [CITED: langref.html L1813-1815]

**Warning signs:** Build error about `append` expecting 2 args. Search for `ArrayList(T).init(`.

### Pitfall 4: Returning Zig error sets across `extern fn` boundary

**What goes wrong:** `pub export fn deal_parse(...) callconv(.c) !*DealHandle` — Zig refuses to compile because error sets are not C-ABI-compatible.

**Why it happens:** Zig's `!T` (`error{} || T`) is a tagged union, not a C return type.

**How to avoid:** All `extern fn` returns are concrete C types: `?*anyopaque`, `bool`, `u32`, etc. Catch Zig errors inside the function body and convert to the diagnostic list / a bool flag / a null pointer. D-10 explicitly states `deal_parse` always returns a non-null handle except on allocator failure.

**Warning signs:** Error "function with calling convention 'c' cannot return error union".

### Pitfall 5: `std.json.Stringify` non-deterministic key order on tagged unions

**What goes wrong:** Snapshot tests are reproducible on one machine but break on another, or break when the Zig version bumps.

**Why it happens:** Issue [#25233](https://github.com/ziglang/zig/issues/25233) — `std.json.Stringify.write` has known interactions with intrusive interfaces and may not preserve declaration order across all Zig versions.

**How to avoid:** For the AST JSON snapshot (D-04), do one of:
1. Use `std.json.Stringify` AND lock the Zig version (already locked to 0.16.0 by D-05) AND verify byte-for-byte stability across two consecutive builds during the lexer phase.
2. Hand-roll a thin writer that emits keys in a fixed order (`"k"` first, then `"span"`, then kind-specific fields alphabetically). ~150 LOC, full control.

**Warning signs:** A token-snapshot test passes locally but fails in CI on a different macOS version; or breaks on a future minor Zig release.

### Pitfall 6: `zig build-lib -femit-h` for header generation

**What goes wrong:** The build emits a `.h` that doesn't match the hand-written ABI struct layout (D-14 needs precise field ordering for diagnostic structs).

**Why it happens:** `-femit-h` historically translated Zig types to C, but the translation rules don't always match what a hand-tuned `deal.h` should look like (e.g. enum sizes, `extern struct` padding). The 0.16.0 langref doesn't show `-femit-h` in its build.zig examples — instead it shows hand-written `mathtest.h` consumed alongside the library (L14605-14613).

**How to avoid:** Hand-write `include/deal.h`. Treat it as a first-class artifact under version control. Use `b.installHeadersDirectory(...)` in `build.zig` to install it alongside `libdeal.a` (verify exact API name in 0.16.0; the precise call may be `b.addInstallDirectory(...)`).

**Warning signs:** `deal.h` regenerates on every build; struct layout in `deal.h` doesn't match the Rust FFI harness's expectations.

### Pitfall 7: `.dealx` `{...}` re-entering the deal expression production but at the wrong mode

**What goes wrong:** Inside `via={CANHarness { connectorType: "Molex MX150" }}`, the parser tokenizes `CANHarness` as a deal identifier, then re-enters `.dealx_tag` instead of staying in `.dealx_expr_brace` for the inner `{...}`.

**Why it happens:** The mode change at `{` is easy; the mode change back at the matching `}` requires a brace-depth counter or recursive descent.

**How to avoid:** Mode is implicit in the parse stack. When `parseInlineObjectLiteral` (the function parsing `IDENT "{" ObjectField+ "}"`) is on the stack, the lexer is called with `mode = .dealx_expr_brace`. When it returns, the caller's frame takes over and the mode reverts naturally. Don't try to track mode in a side-table — let the recursion do it.

**Warning signs:** Tag-balance errors emitted from inside what should be an expression.

### Pitfall 8: Missing `tests/showcase` symlink in CI

**What goes wrong:** Snapshot tests pass locally but fail in CI because the symlink wasn't checked out.

**Why it happens:** The current symlink `deal/tests/showcase → ../../spec/examples/showcase` is a relative symlink that crosses out of the `deal/` directory. Some CI checkouts strip the parent context.

**How to avoid:** Either (a) keep the symlink and run all CI from the repo root; (b) replace the symlink with a `zig build` step that copies the corpus during test setup; (c) commit a flat copy under `deal/tests/showcase-fixtures/` and reconcile occasionally. Recommendation: option (a) is simplest for Phase 1.

**Warning signs:** "file not found" on `tests/showcase/packages/vehicle/battery.deal` only in CI.

## Code Examples

Verified patterns:

### Example A: build.zig static library + executable + tests (Zig 0.16.0 API)

```zig
// Source: spec/references/zig-lang/0.16.0/langref.html §"Mixing Languages" L14614-14641
const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // libdeal.a — the static library that Rust will link against.
    const lib = b.addLibrary(.{
        .linkage = .static,
        .name = "deal",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/lib.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });
    b.installArtifact(lib);

    // Install include/deal.h alongside libdeal.a so cargo can find it.
    b.installFile("include/deal.h", "include/deal.h");

    // Unit + snapshot tests
    const tests = b.addTest(.{
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/lib.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });
    const run_tests = b.addRunArtifact(tests);
    const test_step = b.step("test", "Run Zig unit + snapshot tests");
    test_step.dependOn(&run_tests.step);
}
```

[CITED: langref.html L14614-14641 — `b.addLibrary(.{ .linkage = .dynamic, ... .root_module = b.createModule(.{...}), })`. Static linkage works identically; substitute `.static`.]

### Example B: ArrayList in Zig 0.16.0 (allocator-explicit)

```zig
// Source: langref.html L1813-1815
const std = @import("std");

test "open-tag stack" {
    const gpa = std.testing.allocator;
    var stack: std.ArrayList(struct { name: []const u8, line: u32 }) = .empty;
    defer stack.deinit(gpa);  // explicit allocator!
    try stack.append(gpa, .{ .name = "system", .line = 33 });
    try stack.append(gpa, .{ .name = "subsystem", .line = 44 });
    try std.testing.expectEqual(@as(usize, 2), stack.items.len);
}
```

### Example C: Tagged union AST node (Node + payload structs)

```zig
// Source: parser implementation guide §7 + langref §"Tagged union" L5164
pub const NodeKind = enum {
    deal_file, dealx_file, header_block, package_decl, import_decl, export_decl,
    part_def, port_def, action_def, state_def, attribute_def, item_def,
    interface_def, connection_def, flow_def, allocation_def, requirement_def,
    constraint_def, need_def, use_case_def,
    // members
    part_usage, port_usage, attribute_usage, action_usage, /* ... */
    // expressions
    binary, unary, member_access, call, identifier, integer_literal, real_literal,
    string_literal, template_literal, boolean_literal, unit_call,
    // composition
    system_block, subsystem_block, component_instance, comp_connect, expose_tag,
    traceability_block, allocate_tag, satisfy_block, validate_tag,
    // payload-bearing helpers
    annotation, doc_comment,
};

pub const Node = struct {
    kind: NodeKind,
    span: Span,
    payload: Payload,
};

pub const Payload = union(NodeKind) {
    deal_file: DealFile,
    dealx_file: DealxFile,
    // ... one variant per NodeKind
    binary: BinaryExpr,
    unary: UnaryExpr,
    comp_connect: CompConnect,   // D-09 first-class typed slots
    // ...
};

pub const CompConnect = struct {
    from_expr: ?*Node,        // string-typed PropValue
    to_expr: ?*Node,          // string-typed PropValue
    via_expr: ?*Node,         // InlineObjectLiteral (D-09)
    carrying_expr: ?*Node,    // InlineObjectLiteral (D-09)
    other_props: []*Node,
};
```

### Example D: Rust FFI build.rs linking libdeal.a (macOS arm64)

```rust
// Source: doc.rust-lang.org/cargo/reference/build-scripts.html + kornel.ski/rust-sys-crate
// File: deal/tests/ffi/build.rs
use std::process::Command;
use std::env;
use std::path::PathBuf;

fn main() {
    // 1. Invoke `zig build` to produce libdeal.a in zig-out/lib/
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let deal_dir = PathBuf::from(&manifest_dir).join("../..");  // up to deal/
    let status = Command::new("zig")
        .arg("build")
        .current_dir(&deal_dir)
        .status()
        .expect("failed to invoke zig build");
    assert!(status.success(), "zig build failed");

    // 2. Tell cargo where to find libdeal.a and to link it statically.
    let lib_dir = deal_dir.join("zig-out/lib");
    println!("cargo::rustc-link-search=native={}", lib_dir.display());
    println!("cargo::rustc-link-lib=static=deal");

    // 3. Re-run if any Zig source changes.
    println!("cargo::rerun-if-changed={}", deal_dir.join("src").display());
    println!("cargo::rerun-if-changed={}", deal_dir.join("build.zig").display());
}
```

```rust
// File: deal/tests/ffi/src/lib.rs (or tests/parse.rs)
use std::ffi::c_void;
use std::os::raw::c_char;

#[repr(C)]
struct DealHandle { _opaque: [u8; 0] }

extern "C" {
    fn deal_parse(
        source_ptr: *const u8, source_len: usize,
        filename_ptr: *const u8, filename_len: usize,
    ) -> *mut DealHandle;
    fn deal_free(handle: *mut DealHandle);
    fn deal_has_errors(handle: *mut DealHandle) -> bool;
    fn deal_ast_json(
        handle: *mut DealHandle,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    ) -> bool;
}

#[test]
fn parse_battery_dot_deal() {
    let source = std::fs::read("../showcase/packages/vehicle/battery.deal").unwrap();
    let filename = b"battery.deal";
    let handle = unsafe {
        deal_parse(source.as_ptr(), source.len(), filename.as_ptr(), filename.len())
    };
    assert!(!handle.is_null());
    unsafe {
        assert!(!deal_has_errors(handle), "battery.deal should parse cleanly");
        let mut ptr: *const u8 = std::ptr::null();
        let mut len: usize = 0;
        assert!(deal_ast_json(handle, &mut ptr, &mut len));
        let json_bytes = std::slice::from_raw_parts(ptr, len);
        let json = std::str::from_utf8(json_bytes).unwrap();
        assert!(json.contains("\"v\":1"));
        assert!(json.contains("\"mode\":\"deal\""));
        deal_free(handle);
    }
}
```

[CITED: cargo build-scripts reference; cargo `::` namespace is the 1.77+ form, supported in Rust 1.93.]

### Example E: Diagnostic emission with both spans (D-07 mismatched tag)

```zig
pub const Severity = enum(u8) { err = 0, warn = 1, info = 2, hint = 3 };

pub const SpanLabel = struct { span: Span, label: []const u8 };
pub const FixIt = struct { replacement: []const u8, replace_span: Span };

pub const Diagnostic = struct {
    code: []const u8,           // e.g. "E0302"
    severity: Severity,
    message: []const u8,
    span: Span,                 // primary span
    secondary_spans: []const SpanLabel = &.{},
    fix_it: ?FixIt = null,
    notes: []const u8 = "",
};

// On mismatched composition close tag:
try diagnostics.append(arena_allocator, .{
    .code = "E0302",
    .severity = .err,
    .message = "Mismatched composition close tag",
    .span = close_tag_span,
    .secondary_spans = &.{
        .{ .span = opened_tag_span, .label = "opened here" },
    },
});
```

## State of the Art

| Old Approach | Current Approach (0.16.0) | When Changed | Impact |
|--------------|---------------------------|--------------|--------|
| `callconv(.C)` (uppercase) | `callconv(.c)` (lowercase) | Pre-0.16.0 enum literal rename | Pre-existing tutorials and the parser implementation guide use the old form in places — must be corrected on copy. |
| `std.ArrayList(T).init(allocator)` (stored allocator) | `var list: std.ArrayList(T) = .empty; list.append(allocator, item);` | 0.13+ allocator-explicit redesign | Every `ArrayList` call site needs the allocator threaded explicitly. Cleaner ownership; more typing. [CITED: langref L1813-1815] |
| `b.addStaticLibrary(.{...})` | `b.addLibrary(.{ .linkage = .static, ..., .root_module = b.createModule(.{...}) })` | 0.14/0.15 build API consolidation | Single `addLibrary` for static + dynamic; `root_module` is now an explicit `Module` object created via `b.createModule`. [CITED: langref L14617-14624] |
| `std.StaticStringMap.init(...)` runtime | `std.StaticStringMap.initComptime(.{...})` only | Pre-0.16.0 runtime API removed | Slightly stricter — keys must be known at comptime. Not a regression for keyword tables. [CITED: ziglang/zig issue #19936] |
| `std.json.writeStream(...)` | `std.json.Stringify` with `WriteStream` | Reorganized around 0.14 | Same idea, different module structure. Some bugs around intrusive interfaces (#25233). |
| `cargo:rustc-link-lib=...` (single colon) | `cargo::rustc-link-lib=...` (double colon) | Cargo 1.77+ | Old single-colon form still works for compatibility, but `::` is the new standard. Both are valid in Rust 1.93. |

**Deprecated/outdated:**
- `callconv(.C)` — use `callconv(.c)`
- `addStaticLibrary` — use `addLibrary` with `.linkage = .static`
- Stored-allocator `ArrayList` shape — use `.empty` initializer + per-call allocator

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `std.unicode.utf8ValidateSlice` exists in 0.16.0 std-lib with that exact name | Standard Stack / Don't Hand-Roll | Low — symbol may be at slightly different path; the validation primitive itself definitely exists. Resolve in first lexer task by `zig build` + IDE jump-to-def. |
| A2 | `std.testing.expectEqualStrings` available and stable | Standard Stack | Negligible — standard testing helper; pinned to 0.16.0. |
| A3 | `std.fs.cwd().readFileAlloc` is the right primitive for reading test fixtures in 0.16.0 | Standard Stack | Low — alternative `openFile + readToEndAlloc` is documented and works regardless. |
| A4 | `b.installFile("include/deal.h", "include/deal.h")` is the correct 0.16.0 API to install a hand-written header alongside `libdeal.a` | Code Examples §A | Medium — the precise function name may be `b.installFile` or `b.addInstallFile` in 0.16.0. Resolve in build-scaffolding task; both are well-documented in the std.Build API. |
| A5 | `b.path(...)` is the 0.16.0 way to construct a build-time path | Code Examples §A | Negligible — used in the langref's own examples. [CITED: langref L14621] |
| A6 | `std.heap.smp_allocator` exists as a release-mode general-purpose allocator | Standard Stack (mentioned in alternatives) | Negligible — not on the critical path; Phase 1 uses arena + page allocator. [CITED: langref L13618] |
| A7 | The 19 showcase files are syntactically valid against the locked grammar | Showcase corpus | Mitigated by Phase 0's verification report ("19/19 showcase files parseable" per `spec/grammar/README.md`). If a file is found to violate the grammar during Phase 1, the grammar's parser-test-coverage is the source of truth; file is updated, not the grammar. |
| A8 | `-femit-h` is unreliable for our use case in 0.16.0 | Common Pitfalls / Alternatives | Low — recommendation is to hand-write `deal.h` regardless; this just rules out the alternative path. |
| A9 | `cargo::rustc-link-lib=static=deal` correctly causes Rust to link `libdeal.a` from the search path on macOS arm64 | Code Examples §D | Low — standard Cargo behavior; verified across many sys-crates. |
| A10 | Snapshot tests do not need any external snapshot library — pure-Zig hand-roll is sufficient | Snapshot strategy / Alternatives | Low — the 19-file × 3-snapshot-type (token / AST / malformed) × ~5MB total surface is small enough that hand-rolled diff is fine. If team experience differs, a planning checkpoint can reconsider. |

## Open Questions (RESOLVED)

1. **Should the parser pre-allocate a fixed-size token lookahead buffer, or fetch tokens one at a time?**
   - What we know: D-06 says "lexer-on-demand" — lexer is stateless. Parser holds at most one peek-token. A fixed-size lookahead ring buffer of, say, 2-4 tokens would be cheaper than re-lexing on each peek.
   - What's unclear: Is two-token lookahead ever needed? `FunctionCallExpression` is documented as LL(2) in `spec/grammar/README.md`.
   - Recommendation: Implement single-token peek (one buffered next-token cached on the parser). If LL(2) ambiguities arise during parser_deal implementation, escalate to a two-token buffer in that wave's task. Don't over-engineer up-front.
   - **RESOLVED:** Single-token peek with restore-pos. The `Parser` struct holds `peeked: ?lexer.Token = null` and `peek()` saves the lexer position, calls `next(mode)`, and restores. Adopted in Plan 02 §peek/§next implementation; consumed by Plan 03 SUMMARY's parser_deal smoke test. If LL(2) becomes necessary later, the cache is extended without changing the API.

2. **String interning: hash map vs raw source slices for identifiers?**
   - What we know: CONTEXT.md marks this as Claude's Discretion. Zig's own compiler interns identifiers via a hash map. AST identifier nodes only need byte spans for diagnostics; the text can be retrieved from the source slice.
   - What's unclear: Whether Phase 2 (which adds name resolution) would benefit from pre-interned identifiers — but Phase 1's `D-04` AST JSON includes `"name": "..."` strings literally, so interning saves only on duplicate-string allocation.
   - Recommendation: **Phase 1 stores identifiers as `[]const u8` slices into the source buffer.** No interning. The source buffer lives in the arena (`deal_parse` dupes the caller's bytes into the arena on entry — D-13 reentry rules require independence from the caller's buffer). Phase 2 can add an interner if profiling shows it matters.
   - **RESOLVED:** Raw source slices, no interner. Every `Identifier`, `IntLiteral`, `RealLiteral`, `StringLiteral`, `name`, `key` field in the AST is a `[]const u8` slice into the arena-owned dupe of the caller's source bytes. Adopted in Plan 03 §payload structs (Task 3.1); Phase 2 may add an interner later without breaking the AST shape.

3. **Snapshot update workflow — env var, build flag, or manual?**
   - What we know: This is a discretionary call. Common patterns: `UPDATE_SNAPSHOTS=1 zig build test` or `zig build update-snapshots`.
   - What's unclear: Which fits Zig's build system better.
   - Recommendation: A separate `zig build update-snapshots` step that runs the same test harness but writes the actual output to `tests/snapshots/*.json` instead of comparing. Simple to implement: a build option that the test code reads via `@import("build_options")`.
   - **RESOLVED:** Environment variable `UPDATE_SNAPSHOTS=1`. The test harness reads it via `std.process.getEnvVarOwned`; when set, the harness writes actual JSON to the snapshot path instead of asserting equality. Adopted in Plan 02 Task 2.3 (token snapshots) and mirrored by Plan 03/04 (AST snapshots). One workflow across all snapshot types; no extra build step needed.

4. **`.dealx` mode `dealx_expr_brace` inside nested `{...}` — does the lexer need a brace-depth counter?**
   - What we know: D-08 says the full `deal.ebnf` expression production is accepted; closing `}` is the natural terminator.
   - What's unclear: Within an InlineObjectLiteral, attribute values can be expressions that themselves contain `{...}` (e.g. nested object literals, template strings with `${...}`). Does the lexer need to count braces?
   - Recommendation: No — recursive descent in the parser handles brace pairing. The lexer sees every `{` and `}` as a distinct token; the parser's recursion stack tracks pairing. The only mode change is between the OUTER `{...}` (tag-attribute body, mode `.dealx_expr_brace`) and other contexts. Template `${...}` is handled by lexer-internal template-string state, which is mode-independent.
   - **RESOLVED:** Not needed — parser recursion handles pairing. Adopted in Plan 04 §Pitfall 7 note inside `parseInlineObjectOrExpr`: the function recurses naturally through `parseExpression → parsePrimary → ObjectLiteral`, and the mode pops back to `.dealx_tag` only when the outer-most `parseInlineObjectOrExpr` returns. The lexer remains stateless w.r.t. braces.

5. **Malformed corpus — generated or hand-curated?**
   - What we know: 50+ malformed cases are required by REQ-phase-1-4. No malformed corpus exists yet.
   - What's unclear: Whether to generate them by mutating showcase files or write them by hand.
   - Recommendation: A mix. (a) Hand-curate ~20 cases covering the most common error categories (mismatched tag, missing semicolon, unterminated string, invalid escape, bad annotation, garbage after package decl, unclosed brace, unmatched `[<`, invalid identifier, EOF mid-tag). (b) Generate ~30 mutations by programmatically deleting random tokens / inserting garbage in showcase files. The 50+ floor is then comfortably exceeded. Document the malformed corpus generator as a `zig build gen-malformed` step.
   - **RESOLVED:** 20 hand-curated + 30+ generated via `gen-malformed.zig`. Adopted in Plan 05 Task 5.3: 20 hand-curated files (`m01_missing_semicolon.deal` through `m20_deeply_nested_braces.deal`) each targeting a specific D-16 code; `deal/tools/gen-malformed.zig` mutates the 19 showcase files via 5 strategies (drop-token, swap-token, truncate, inject-garbage, unmatched-bracket) using a fixed PRNG seed (`0xDEAL2026`) for ≥30 additional files, comfortably exceeding the 50+ floor.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Zig | All Zig source files, build.zig | ✓ | 0.16.0 | — (BLOCKING if missing — but it isn't) |
| Rust + Cargo | FFI test harness (REQ-phase-1-5) | ✓ | rustc 1.93.0, cargo 1.93.0 | — |
| macOS arm64 | Development platform | ✓ | Darwin 25.4.0 arm64 | — |
| `tests/showcase` symlink | All snapshot tests | ✓ | resolves to `spec/examples/showcase/` (15 .deal + 4 .dealx confirmed) | — |
| `.planning/config.json` | GSD orchestration | ✗ | — | Defaults apply; `nyquist_validation` treated as enabled (this section is included). |
| `codegraph` index | Code navigation hints | ✓ (`deal/.codegraph/index.bin` present) | — | Not used during Phase 1 implementation since `src/` is empty; will populate as code is written. |

**Missing dependencies with no fallback:** none.

**Missing dependencies with fallback:** `.planning/config.json` absent — treated as defaults-enabled per orchestrator convention. No action required.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Zig built-in test runner (`zig build test`) + standard `test "..."` blocks + Rust cargo test for FFI smoke |
| Config file | `deal/build.zig` (Zig tests) + `deal/tests/ffi/Cargo.toml` (Rust harness) — both must be created in Wave 0 |
| Quick run command | `cd deal && zig build test` (Zig unit + snapshot tests) |
| Full suite command | `cd deal && zig build test && cargo test --manifest-path tests/ffi/Cargo.toml` |
| Phase gate command | All of the above pass + all 19 showcase files have stable snapshots + ≥50 malformed inputs produce diagnostics without panic |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| REQ-phase-1-1-lexer | All 19 showcase files tokenize with zero UNKNOWN tokens | snapshot | `zig build test -Dtest-filter=lexer.snapshot` | ❌ Wave 0 (build.zig + harness + snapshots/) |
| REQ-phase-1-1-lexer | Mode-flip: `[<` in `.deal` mode produces `LBRACKET LT` (not `TAG_OPEN`) | unit | `zig build test -Dtest-filter=lexer.mode_flip` | ❌ Wave 0 |
| REQ-phase-1-1-lexer | Keyword table recognizes 37 reserved keywords; non-keywords pass through as `IDENT` | unit | `zig build test -Dtest-filter=lexer.keywords` | ❌ Wave 0 |
| REQ-phase-1-1-lexer | DOC vs BLOCK comment disambiguation (`/**` vs `/*`) | unit | `zig build test -Dtest-filter=lexer.comments` | ❌ Wave 0 |
| REQ-phase-1-1-lexer | Template `${}` lexer state stack (TEMPLATE_HEAD/MIDDLE/TAIL) | unit | `zig build test -Dtest-filter=lexer.templates` | ❌ Wave 0 |
| REQ-phase-1-2-parser-deal | All 15 `.deal` showcase files parse, AST JSON snapshots stable | snapshot | `zig build test -Dtest-filter=parser_deal.snapshot` | ❌ Wave 0 |
| REQ-phase-1-2-parser-deal | Pratt precedence matches deal.ebnf §19 (8 levels) | unit | `zig build test -Dtest-filter=expr.precedence` | ❌ Wave 0 |
| REQ-phase-1-2-parser-deal | Each of the 87 deal.ebnf productions exercised by ≥1 fixture (snapshot or unit) | coverage | `zig build coverage` (Wave-0-defined) | ❌ Wave 0 |
| REQ-phase-1-3-parser-dealx | All 4 `.dealx` showcase files parse, AST JSON snapshots stable | snapshot | `zig build test -Dtest-filter=parser_dealx.snapshot` | ❌ Wave 0 |
| REQ-phase-1-3-parser-dealx | Tag balance: `[<system>] ... [</subsystem>]` emits E0302 with both spans, parsing continues | unit | `zig build test -Dtest-filter=parser_dealx.tag_balance` | ❌ Wave 0 |
| REQ-phase-1-3-parser-dealx | `[<connect>]` with `via={...}` produces `comp_connect` node with `via_expr` slot populated | unit | `zig build test -Dtest-filter=parser_dealx.connect_via` | ❌ Wave 0 |
| REQ-phase-1-3-parser-dealx | Each of the 43 dealx.ebnf productions exercised by ≥1 fixture | coverage | `zig build coverage` | ❌ Wave 0 |
| REQ-phase-1-4-error-recovery | ≥50 malformed inputs in `tests/malformed/` produce diagnostics without panic | snapshot | `zig build test -Dtest-filter=recovery.corpus` | ❌ Wave 0 (corpus generation) |
| REQ-phase-1-4-error-recovery | Statement-level sync: missing `;` recovers at next statement | unit | `zig build test -Dtest-filter=recovery.statement` | ❌ Wave 0 |
| REQ-phase-1-4-error-recovery | Definition-level sync: malformed body skips to next definition keyword | unit | `zig build test -Dtest-filter=recovery.definition` | ❌ Wave 0 |
| REQ-phase-1-4-error-recovery | `.dealx` tag-level sync: malformed tag skips to next `[<` or `[</` | unit | `zig build test -Dtest-filter=recovery.dealx_tag` | ❌ Wave 0 |
| REQ-phase-1-4-error-recovery | Diagnostic struct round-trip: every field appears in `deal_diagnostics_json` output | unit | `zig build test -Dtest-filter=diag.json_roundtrip` | ❌ Wave 0 |
| REQ-phase-1-5-c-abi | Rust harness loads libdeal.a, calls `deal_parse` on one showcase file, reads AST JSON, calls `deal_free` | integration | `cargo test --manifest-path tests/ffi/Cargo.toml ffi_smoke` | ❌ Wave 0 |
| REQ-phase-1-5-c-abi | `deal_parse` on invalid UTF-8 returns a handle with one diagnostic E0001, no panic | unit | `zig build test -Dtest-filter=c_abi.invalid_utf8` | ❌ Wave 0 |
| REQ-phase-1-5-c-abi | `deal_free` releases all memory (no leaks reported by std.testing.allocator wrapper) | unit | `zig build test -Dtest-filter=c_abi.no_leaks` | ❌ Wave 0 |
| REQ-phase-1-gate | All above tests green; `deal parse showcase/*.deal showcase/*.dealx` (via Rust harness CLI) succeeds for all 19 | integration | `cargo test --manifest-path tests/ffi/Cargo.toml gate_all_19` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cd deal && zig build test -Dtest-filter=<area>` — fast targeted run (<5s typical)
- **Per wave merge:** `cd deal && zig build test` — full Zig test suite (<30s typical)
- **Phase gate:** Full suite green: `cd deal && zig build test && cargo test --manifest-path tests/ffi/Cargo.toml`, then `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `deal/build.zig` — entry point that compiles `libdeal.a`, runs tests, and exposes `-Dtest-filter` build option
- [ ] `deal/build.zig.zon` — empty package manifest (no external deps in Phase 1)
- [ ] `deal/src/lib.zig` — stub with C ABI exports (`deal_parse`, `deal_free`, `deal_has_errors`, `deal_diagnostics_count`, `deal_diagnostics_json`, `deal_ast_json`) returning sensible defaults so the FFI compiles before lexer/parser exist
- [ ] `deal/include/deal.h` — hand-written C header matching the stub exports
- [ ] `deal/src/lexer.zig`, `deal/src/parser.zig`, `deal/src/parser_deal.zig`, `deal/src/parser_dealx.zig`, `deal/src/expr.zig`, `deal/src/ast.zig`, `deal/src/diagnostics.zig`, `deal/src/source_map.zig`, `deal/src/json.zig`, `deal/src/keywords.zig` — file stubs with `pub` types ready to fill
- [ ] `deal/tests/snapshots/` — empty directory (committed via .gitkeep) for token/AST/diagnostic snapshots
- [ ] `deal/tests/malformed/` — directory + initial hand-curated 20 malformed files + a `gen-malformed.zig` corpus generator (run via `zig build gen-malformed`)
- [ ] `deal/tests/ffi/Cargo.toml` + `deal/tests/ffi/build.rs` + `deal/tests/ffi/tests/parse.rs` — Rust FFI harness scaffolding (will start returning success once the C ABI stub is in)
- [ ] `deal/tests/unit/` — directory + initial mode_flip + comments test files referenced by the harness

## Security Domain

> `security_enforcement` is not explicitly configured. Treated as enabled (default-on) per orchestrator convention.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | No identity surface in Phase 1 — `deal_parse` accepts source bytes from any caller. |
| V3 Session Management | no | No sessions. Per-handle, single-threaded (D-13). |
| V4 Access Control | no | No access control surface — caller already has the bytes. |
| V5 Input Validation | **yes** | Validate input bytes are well-formed UTF-8 before lexing (`std.unicode.utf8ValidateSlice`). Bound source_len (reject pathological multi-GB inputs with `E0001`). |
| V6 Cryptography | no | No cryptographic operations. (`@header { hash: sha256:... }` is parsed as opaque text per FS-2 — verification is semantic, deferred to Phase 2.) |
| V7 Error Handling and Logging | **yes** | The diagnostic system IS the error-handling surface. Diagnostic messages must never include uncontrolled source content beyond the span (avoid leaking secrets if a caller passes credentialed text). Spans expose byte offsets, not source text. |
| V8 Data Protection | no | Source bytes are duped into the arena and freed via `deal_free`; no persistence. |
| V9 Communication | no | No network. |
| V10 Malicious Code | n/a | Phase 1 has no plugin / extension surface. |
| V11 Business Logic | no | n/a for a parser. |
| V12 Files and Resources | **yes** | The Zig core does NOT read files — D-12 explicitly keeps file I/O in the Rust harness. This is a security property to preserve: do not accept file paths over the C ABI; only bytes + filename hint (for diagnostics + mode dispatch). |
| V13 API and Web Service | n/a | C ABI is not a web/network API. |
| V14 Configuration | no | No config. |

### Known Threat Patterns for Zig 0.16.0 parser + C ABI

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Stack overflow from deeply-nested input (e.g. 100,000 nested `[<system>][<subsystem>]...` tags) | Denial-of-Service | Bound recursion depth in `parser_dealx.parseSystemContent` and emit `E0303` ("nesting too deep") past the limit (suggested limit: 1024). Catch in Phase 1 to prevent Rust harness from crashing. |
| Quadratic blowup from pathological lexer input (e.g. unterminated template `\`${a + \`${b + ...` repeated) | Denial-of-Service | The lexer's template-string state stack must also be bounded. Same `E0303` family or a dedicated lexer code. |
| Use-after-free across the C ABI | Tampering / Information Disclosure | The Rust harness must call `deal_free` exactly once per handle. Document in `deal.h` comments. Rust wrapper enforces via `Drop`. |
| Memory leak in `deal_parse` on partial failure | Resource Exhaustion | The arena ensures all-or-nothing: on any internal error before returning a handle, `arena.deinit()` is called and null is returned. After a handle is returned, `deal_free` is the only release path. |
| Buffer overrun on length-prefixed accessors | Memory Corruption | `deal_ast_json` writes `*out_ptr` and `*out_len` from buffers OWNED by the handle's arena. The caller does not pass a buffer to write into. No overrun is possible. |
| Integer overflow on `source_len` cast to u32 spans | Memory Corruption | `Span` uses `u32` (D-15). Reject sources > 4 GiB with `E0004` at the top of `deal_parse`. Saturate at 2^32-1 if reasonable. |
| Diagnostic message string injection from source content | Information Disclosure | Diagnostic messages must use static format strings or `std.fmt.allocPrint` with explicit format specifiers. Never `std.fmt.allocPrint(arena, source[span.start..span.end], .{})` — that would treat source bytes as a format string. |
| Locale-dependent identifier comparison | Tampering | All keyword comparisons use `std.mem.eql(u8, ...)` (byte equality). No `std.ascii.eqlIgnoreCase`. The grammar uses ASCII keywords only. |

## Sources

### Primary (HIGH confidence)
- `spec/references/zig-lang/0.16.0/langref.html` — Zig 0.16.0 language reference, vendored 2026-05-18. Specifically verified:
  - `callconv(.c)` lowercase: L5405, L10808, L14553
  - `std.ArrayList(T)` allocator-explicit API: L1813-1815
  - `std.heap.ArenaAllocator.init(std.heap.page_allocator)`: L13575-13591
  - `b.addLibrary(.{ .linkage = ..., .root_module = b.createModule(...) })`: L14614-14641
  - Tagged-union `switch` exhaustiveness: L5164-5235
  - `extern struct`: §"extern struct"
  - `std.testing.allocator` leak detection: L1810-1849
- `spec/grammar/lexical.ebnf` (758 lines) — token set the lexer must produce
- `spec/grammar/deal.ebnf` (1,679 lines, 87 productions) — read sections covering file structure, header block, expression production (§19), and FIRST sets
- `spec/grammar/dealx.ebnf` (897 lines, 43 productions) — read sections covering composition block, connect tag, satisfy block, traceability
- `spec/grammar/README.md` — confirmed 130 productions, LL(1) at 8 / LL(2) at 4, 19-file showcase parseability
- `spec/grammar/tmp-references/deal-parser-implementation-guide.html` — read in full; provides Pratt precedence table, C ABI shape, lexer complexity table, error-recovery sync sets, wave roadmap (already aligned to phase requirements)
- `spec/grammar/DESIGN-DECISIONS.md` references (FS-1, FS-2, SD-1..SD-20, CS-1..CS-16) — confirmed via `PROJECT.md` summary
- `deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md` — locked decisions D-01..D-17 (the source of `<user_constraints>` above)
- `deal/.planning/phases/01-zig-compiler-core/01-DISCUSSION-LOG.md` — audit trail confirms each lock was chosen against documented alternatives
- `deal/.planning/REQUIREMENTS.md` §Phase 1 — REQ-phase-1-foundation through REQ-phase-1-gate
- `deal/.planning/ROADMAP.md` §Phase 1 — Goal and success criteria
- `deal/README.md` — Planned source-tree layout and Phase 1 milestones (already aligned)
- `spec/examples/showcase/` — confirmed 15 `.deal` + 4 `.dealx` (`find` command output)
- Local environment verified: `zig version` → 0.16.0; `rustc --version` → 1.93.0; `uname -ms` → Darwin arm64

### Secondary (MEDIUM confidence)
- Cargo Build Scripts reference (doc.rust-lang.org/cargo/reference/build-scripts.html) — `cargo::rustc-link-search`, `cargo::rustc-link-lib=static=...` semantics. Verified syntax is current via web search.
- Andrew Kelley's "String Matching based on Compile Time Perfect Hashing in Zig" (andrewkelley.me) — `std.StaticStringMap` rationale and history.
- ziglang/zig issues #19936, #21498, #25233, #16519 — `std.StaticStringMap.initComptime` migration; `std.json.Stringify` known limitations.
- Ziggit thread "How to pass write stream from std.json correctly?" — community pattern for `std.json` writers.

### Tertiary (LOW confidence — flagged for validation in Wave 0)
- Exact name of `b.installFile` vs `b.addInstallFile` in 0.16.0 — verify when scaffolding `build.zig`.
- Exact name of `std.fs.cwd().readFileAlloc` vs `openFile + readToEndAlloc` for fixture loading — both work, pick during Wave 0.
- `std.unicode.utf8ValidateSlice` exact name — search std-lib when implementing the lexer entry point.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — Zig 0.16.0 langref vendored and grep-verified for `callconv`, `ArenaAllocator`, `ArrayList`, `addLibrary`; environment confirmed via `zig version`
- Architecture: HIGH — every pattern maps to a locked decision in CONTEXT.md or to a Zig 0.16.0 idiom in the langref or to a documented section of the parser implementation guide
- Pitfalls: HIGH — six of eight pitfalls are version-specific Zig API changes that are documented in the langref or in ziglang/zig issues; the remaining two (mode-switching, CI symlink) are derived from concrete grammar/repo structure
- Validation architecture: HIGH — every phase requirement has a named test target; Wave 0 gaps are enumerated explicitly
- Security: MEDIUM — ASVS categories applied conservatively to a parser-only surface; standard parser-DoS mitigations identified
- Rust FFI link recipe: MEDIUM — verified via Cargo docs but not exercised on this exact macOS arm64 + Zig 0.16.0 stack; first FFI smoke test in Wave 0 will validate

**Research date:** 2026-05-19
**Valid until:** 2026-06-19 (~30 days for the stable parts: locked decisions, grammar files, Zig 0.16.0 — none of which are expected to move in this window). Re-check Zig 0.16.x patch-release notes if implementation slips past June 2026.

## RESEARCH COMPLETE

Phase 1 is unusually well-specified for a greenfield phase: 17 locked decisions cover AST shape, arena lifetime, mode-switching strategy, C ABI memory ownership, diagnostic structure, span representation, error-code namespace, and three-tier recovery; the grammar is 100% complete at `0.1.0-draft`; the 19-file showcase corpus is the integration oracle and is already symlinked into `deal/tests/showcase/`; the Zig 0.16.0 reference is vendored locally; and the dev host has Zig 0.16.0 + Rust 1.93.0 confirmed installed. Research surfaces the Zig 0.16.0-specific idioms that the planner MUST honor (the breaking changes from earlier Zig releases — `callconv(.c)` lowercase, `ArrayList` allocator-explicit API, `b.addLibrary` replacing `addStaticLibrary`, `StaticStringMap.initComptime`-only), maps every phase requirement to an automated test command in the Validation Architecture section, enumerates the Wave 0 scaffolding gaps, and lists ten honest assumptions for the planner to verify in early waves. The planner can produce a clean five-wave plan (Wave 0 scaffolding + Waves 1-5 mirroring REQ-phase-1-1 through REQ-phase-1-5) without further blocking research.
