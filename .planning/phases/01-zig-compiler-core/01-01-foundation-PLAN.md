---
phase: 01-zig-compiler-core
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - deal/build.zig
  - deal/build.zig.zon
  - deal/src/lib.zig
  - deal/src/lexer.zig
  - deal/src/parser.zig
  - deal/src/parser_deal.zig
  - deal/src/parser_dealx.zig
  - deal/src/expr.zig
  - deal/src/ast.zig
  - deal/src/diagnostics.zig
  - deal/src/source_map.zig
  - deal/src/json.zig
  - deal/src/keywords.zig
  - deal/include/deal.h
  - deal/tests/snapshots/.gitkeep
  - deal/tests/malformed/.gitkeep
  - deal/tests/unit/.gitkeep
  - deal/tests/ffi/Cargo.toml
  - deal/tests/ffi/build.rs
  - deal/tests/ffi/tests/parse.rs
autonomous: true
requirements:
  - REQ-phase-1-foundation
user_setup: []

# Plan file convention for Phase 1:
# `01-PLAN-NN-<slug>.md` where NN is the sequential plan number within the phase.
# Execution order follows NN ascending; depends_on captures inter-plan dependencies.

must_haves:
  decisions_implemented:
    - "D-01: Tagged-union AST scaffolded — `Node` struct + `NodeKind` enum (file kinds in Wave 0; Plan 03 extends) per `src/ast.zig` stub"
    - "D-02: Per-handle arena lifetime established — `DealHandle.arena = std.heap.ArenaAllocator.init(page_allocator)`; `deal_free` calls `arena.deinit()` (single-shot free)"
    - "D-03: Unified `NodeKind` across `.deal` + `.dealx` declared in `src/ast.zig` (single enum; Plans 03/04 extend)"
    - "D-04: AST JSON shape `{\"v\":1,\"mode\":\"<m>\",\"filename\":\"<n>\",\"root\":null}` emitted by `src/json.zig` stub `emitAst` (envelope verified by `ffi_dealx_mode_detection`)"
    - "D-05: Zig 0.16.0 langref compliance — `callconv(.c)` lowercase, `b.addLibrary(.linkage=.static, .root_module=createModule(...))`, `installFile`, `addRunArtifact`; banned `callconv(.C)`/`addStaticLibrary` enforced by acceptance grep"
    - "D-06: Four-mode lexer dispatch — `Mode = enum { deal_def, dealx_outer, dealx_tag, dealx_expr_brace }` declared in `src/lexer.zig` stub"
    - "D-07: Parser-owned open-tag stack — Parser struct slot declared (Plan 03 lands the stub field, Plan 04 fills bodies)"
    - "D-08: Inside `{...}` accept full `deal.ebnf` expression production — `src/expr.zig` stub exports `parseExpression(parser, min_bp)` signature"
    - "D-09: First-class `comp_connect` — `NodeKind` enum reserved (declared in Plan 01 stub as part of the unified enum; payload populated in Plan 04)"
    - "D-10: `deal_parse` always returns non-null handle (except OOM) — verified by `ffi_smoke` which calls deal_parse on empty source and asserts handle != null"
    - "D-11: Length-prefixed UTF-8 buffer accessors — `deal_ast_json(handle, out_ptr, out_len)` and `deal_diagnostics_json(...)` signatures use `[*]const u8` + `usize`, owned by arena"
    - "D-12: Input is bytes + filename hint, no file I/O in Zig — `deal_parse(source_ptr, source_len, filename_ptr, filename_len)`; mode selected by `std.mem.endsWith(filename, \".dealx\")`; verified by `ffi_dealx_mode_detection`"
    - "D-13: Per-handle thread affinity, no global state — DealHandle owns its own arena, no statics in lib.zig; documented in deal.h doxygen"
    - "D-14: Rich `Diagnostic` data model — `src/diagnostics.zig` declares `Diagnostic { code, severity, message, span, secondary_spans, fix_it, notes }` with `Severity` / `SpanLabel` / `FixIt` types"
    - "D-15: `Span = extern struct { start: u32, end: u32 }` declared in `src/ast.zig` (extern struct required for C ABI crossing)"
    - "D-16: Letter-prefixed numeric error codes — namespace ranges (E0001-E0099 lexer, E0100-E0299 parser-deal, E0300-E0399 parser-dealx, E0400-E0499 recovery, W0500-W0599 warn, H0600-H0699 hint) documented in `src/diagnostics.zig` comment"
    - "D-17: Three-tier error recovery — interface declared (Parser stub; Plans 03/04 ship sync method stubs, Plan 05 fills bodies)"
    - "D-18: Alphabetical JSON field order invariant — top-level keys `v`, `mode`, `filename`, `root` emit in fixed order from `src/json.zig` stub; payload alphabetical invariant declared for downstream plans"
  truths:
    - "Running `cd deal && zig build` produces `zig-out/lib/libdeal.a` and installs `zig-out/include/deal.h`"
    - "Running `cd deal && zig build test` exits 0 (executes 0 or more tests; test runner wired)"
    - "Running `cargo test --manifest-path deal/tests/ffi/Cargo.toml` builds the FFI harness and the stub smoke test passes against the stub library"
    - "`-Dtest-filter=<area>` build option is wired and accepted by the test runner"
  artifacts:
    - path: "deal/build.zig"
      provides: "Zig build entry: addLibrary static, addTest, install header, -Dtest-filter option, gen-malformed step placeholder"
      contains: "addLibrary"
    - path: "deal/build.zig.zon"
      provides: "Empty package manifest (.name='deal', .version='0.1.0-draft', no dependencies)"
    - path: "deal/src/lib.zig"
      provides: "C ABI stub exports: deal_parse, deal_free, deal_has_errors, deal_diagnostics_count, deal_diagnostics_json, deal_ast_json — all `callconv(.c)` lowercase"
      contains: "callconv(.c)"
    - path: "deal/include/deal.h"
      provides: "Hand-written C header mirroring the six stub exports + DealHandle opaque type"
      contains: "deal_parse"
    - path: "deal/src/lexer.zig"
      provides: "Lexer struct, Mode enum (deal_def, dealx_outer, dealx_tag, dealx_expr_brace), Token struct, Tag enum stub"
    - path: "deal/src/ast.zig"
      provides: "Node struct, NodeKind enum stub (file kinds only), Span struct (u32 start, u32 end)"
    - path: "deal/src/diagnostics.zig"
      provides: "Diagnostic struct, Severity enum, SpanLabel, FixIt"
    - path: "deal/src/source_map.zig"
      provides: "SourceMap struct stub (byte→line/col table, lazy)"
    - path: "deal/src/json.zig"
      provides: "Stub emitAst / emitDiagnostics returning '{\"v\":1}'"
    - path: "deal/src/keywords.zig"
      provides: "Stub StaticStringMap placeholder (empty .initComptime(.{}))"
    - path: "deal/src/parser.zig"
      provides: "Parser driver stub: mode dispatch entry, diagnostic collector slot"
    - path: "deal/src/parser_deal.zig"
      provides: "Module stub with `pub fn parseFile` signature returning ?*Node = null"
    - path: "deal/src/parser_dealx.zig"
      provides: "Module stub with `pub fn parseFile` signature returning ?*Node = null"
    - path: "deal/src/expr.zig"
      provides: "Module stub with `pub fn parseExpression(self: *anyopaque, min_bp: u8)` signature"
    - path: "deal/tests/ffi/Cargo.toml"
      provides: "Cargo package 'deal-ffi-tests' with [lib] crate-type=staticlib N/A — package is a test harness consuming staticlib via build.rs"
    - path: "deal/tests/ffi/build.rs"
      provides: "Invokes `zig build` in parent dir, emits cargo::rustc-link-search and cargo::rustc-link-lib=static=deal directives"
    - path: "deal/tests/ffi/tests/parse.rs"
      provides: "Single #[test] fn ffi_smoke that calls deal_parse on b\"\" + filename + verifies handle != null + calls deal_free"
  key_links:
    - from: "deal/build.zig"
      to: "deal/src/lib.zig"
      via: "b.createModule(.{ .root_source_file = b.path(\"src/lib.zig\") })"
      pattern: "src/lib.zig"
    - from: "deal/build.zig"
      to: "deal/include/deal.h"
      via: "b.installFile(\"include/deal.h\", ...)"
      pattern: "include/deal.h"
    - from: "deal/tests/ffi/build.rs"
      to: "deal/zig-out/lib/libdeal.a"
      via: "cargo::rustc-link-lib=static=deal"
      pattern: "rustc-link-lib=static=deal"
    - from: "deal/src/lib.zig"
      to: "deal/include/deal.h"
      via: "six `extern fn` exports must match six C declarations in deal.h field-for-field"
      pattern: "deal_parse|deal_free|deal_has_errors|deal_diagnostics_count|deal_diagnostics_json|deal_ast_json"
---

<objective>
Wave 0 scaffolding for the Zig compiler core. Establish the build system, the empty-but-valid stubs for every source file Phase 1 will fill, the C ABI surface as a no-op stub, the hand-written header, and the Rust FFI test harness — all wired together so that subsequent waves (lexer, parsers, recovery, real ABI) only need to fill in bodies rather than create new files.

Purpose: prevent every downstream wave from re-litigating the build graph. After this plan, every other plan in Phase 1 only edits existing files. The C ABI is consumable from Rust on day 1 (even though `deal_parse` returns a stub handle that does no parsing), which makes the FFI link recipe verifiable immediately rather than at the end of the phase.

Output: a `cd deal && zig build` that produces `libdeal.a` + `include/deal.h`; a `zig build test` that runs (0 tests is fine for Wave 0); a `cargo test --manifest-path tests/ffi/Cargo.toml` that builds and runs a single smoke test against the stub.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@deal/.planning/PROJECT.md
@deal/.planning/ROADMAP.md
@deal/.planning/STATE.md
@deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md
@deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md
@deal/.planning/phases/01-zig-compiler-core/01-VALIDATION.md
@deal/README.md
@spec/references/zig-lang/0.16.0/langref.html
@spec/grammar/tmp-references/deal-parser-implementation-guide.html
</context>

<tasks>

<task type="auto">
  <name>Task 1.1: Create build.zig + build.zig.zon + tests/ scaffolding</name>
  <read_first>
    - deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md (D-05, D-10, D-11, D-13)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Example A: build.zig static library + executable + tests (Zig 0.16.0 API)" (lines 644-683) and §"Wave 0 Gaps" (lines 970-979)
    - deal/README.md (planned layout)
    - spec/references/zig-lang/0.16.0/langref.html §"Mixing Languages" (L14614-14641: addLibrary, createModule, installFile, addTest, addRunArtifact, step)
  </read_first>
  <files>
    deal/build.zig,
    deal/build.zig.zon,
    deal/tests/snapshots/.gitkeep,
    deal/tests/malformed/.gitkeep,
    deal/tests/unit/.gitkeep
  </files>
  <action>
Create `deal/build.zig` using the Zig 0.16.0 builder API exclusively. Required structure:

1. `pub fn build(b: *std.Build) void` (NO `!void` — Zig 0.16.0 build signature is `void`; see langref §14614).
2. `target = b.standardTargetOptions(.{})` and `optimize = b.standardOptimizeOption(.{})`.
3. Define a `test_filter` build option via `b.option([]const u8, "test-filter", "Filter test names by substring")` so `zig build test -Dtest-filter=<area>` works (per RESEARCH §Validation Architecture lines 942-962). Pass the filter into the test step using the 0.16.0 `b.addTest(.{ .filters = ... })` field if available; if the langref shows the option only as `test_runner.filter`, fall back to setting it via `tests.filters = if (test_filter) |f| &.{f} else &.{}` — verify exact field name in the langref before writing.
4. Create `lib` via `b.addLibrary(.{ .linkage = .static, .name = "deal", .root_module = b.createModule(.{ .root_source_file = b.path("src/lib.zig"), .target = target, .optimize = optimize }) })`. Call `b.installArtifact(lib)`.
5. Install the hand-written header: `b.installFile("include/deal.h", "include/deal.h")`. (Assumption A4 in RESEARCH — if `installFile` is `addInstallFile` in 0.16.0, use whichever the langref shows. Both produce the same install result.)
6. Create `tests` via `b.addTest(.{ .root_module = b.createModule(.{ .root_source_file = b.path("src/lib.zig"), .target = target, .optimize = optimize }) })` and apply the test_filter. Wire `const run_tests = b.addRunArtifact(tests); const test_step = b.step("test", "Run Zig unit + snapshot tests"); test_step.dependOn(&run_tests.step);`.
7. Add a placeholder `gen-malformed` step: `const gen_malformed = b.step("gen-malformed", "Regenerate the malformed corpus from showcase mutations"); _ = gen_malformed;` — body is filled in by Plan 05. Keep the step present so dependents can `dependOn` it now.

Create `deal/build.zig.zon` with the minimum 0.16.0 manifest: `.{ .name = .deal, .version = "0.1.0-draft", .minimum_zig_version = "0.16.0", .dependencies = .{}, .paths = .{ "build.zig", "build.zig.zon", "src", "include", "tests" } }`. Note: 0.16.0 requires `.name` as an enum literal `.deal`, NOT a string — verify against `spec/references/zig-lang/0.16.0/langref.html` (search "build.zig.zon"). If the langref shows the string form, use a string.

Create empty `.gitkeep` files in `deal/tests/snapshots/`, `deal/tests/malformed/`, `deal/tests/unit/` so the directories are committed for later waves. The existing `deal/tests/showcase` symlink is left intact.

Do NOT use `callconv(.C)` (uppercase) anywhere — Zig 0.16.0 uses `callconv(.c)` (RESEARCH §Pitfall 2). Do NOT use `addStaticLibrary` — it was replaced by `addLibrary` with `.linkage = .static` (RESEARCH §State of the Art).
  </action>
  <acceptance_criteria>
    - deal/build.zig contains the literal text `b.addLibrary(` and `.linkage = .static` and `b.installFile(` and `b.step("test"`
    - deal/build.zig contains the literal text `test-filter` (the option name)
    - deal/build.zig does NOT contain the strings `callconv(.C)` (uppercase) or `addStaticLibrary`
    - deal/build.zig.zon contains the string `0.1.0-draft` and `0.16.0`
    - deal/tests/snapshots/.gitkeep, deal/tests/malformed/.gitkeep, deal/tests/unit/.gitkeep all exist
    - **Structural check (Task 1.1 alone — no Zig invocation):** <automated>cd /Users/dunnock/projects/deal-lang/deal && grep -c '"test-filter"' build.zig — produces ≥ 1; confirms `b.option(..., "test-filter", ...)` is declared.</automated> The runtime invocation `zig build --help | grep test-filter` REQUIRES `src/lib.zig` to compile (it's the build-graph root), so it CANNOT run until Task 1.2 lands the stub. The end-of-plan verification block (under `<verification>`) runs `zig build --help | grep -c test-filter` after Task 1.2 commits. NOTE #9 fix: Task 1.1's acceptance asserts only the structural property (build.zig declares the option); Task 1.2's `<verify>` block runs `zig build` which incidentally exercises the option's runtime visibility.
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && grep -v '^[[:space:]]*//' build.zig | grep -c "addLibrary\|installFile\|addTest\|test-filter" 2>&1 — expect ≥ 4 distinct hits across these 4 tokens</automated>
  </verify>
  <done>
    build.zig and build.zig.zon exist, use only Zig 0.16.0 APIs (addLibrary, createModule, installFile, addTest, addRunArtifact, b.step), define a `test-filter` build option, install include/deal.h alongside libdeal.a, and the three test directories are present with .gitkeep markers. **Task 1.1 verifies only structurally** — that `build.zig` declares the option (via grep on the source). The runtime check `zig build --help | grep -c test-filter` runs at end of plan AFTER Task 1.2 ships `src/lib.zig` so the build graph compiles. (NOTE #9 fix: no cross-task verify dependency in Task 1.1.)
  </done>
</task>

<task type="auto">
  <name>Task 1.2: Create src/*.zig stubs and include/deal.h</name>
  <read_first>
    - deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md (D-01..D-17 inclusive)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Recommended Project Structure" (lines 227-265), §"Pattern 6: C ABI with opaque handle" (lines 447-515), §"Example C: Tagged union AST node" (lines 701-745), §"Example E: Diagnostic emission" (lines 824-852)
    - spec/references/zig-lang/0.16.0/langref.html §"extern struct", §"callconv", §"std.heap.ArenaAllocator" (L13575-13591)
    - deal/README.md (Planned Layout)
  </read_first>
  <files>
    deal/src/lib.zig,
    deal/src/lexer.zig,
    deal/src/parser.zig,
    deal/src/parser_deal.zig,
    deal/src/parser_dealx.zig,
    deal/src/expr.zig,
    deal/src/ast.zig,
    deal/src/diagnostics.zig,
    deal/src/source_map.zig,
    deal/src/json.zig,
    deal/src/keywords.zig,
    deal/include/deal.h
  </files>
  <action>
Create each of the 11 `src/*.zig` files as a compilable stub with the public types and function signatures the rest of Phase 1 will fill in. Use `callconv(.c)` lowercase, Zig 0.16.0 `ArrayList(T) = .empty` allocator-explicit shape, and `std.StaticStringMap(V).initComptime(.{})` (never runtime `init()`).

src/ast.zig: declare `pub const Span = extern struct { start: u32, end: u32 };` (per D-15 — must be `extern struct` so it crosses the C ABI cleanly), `pub const NodeKind = enum { deal_file, dealx_file };` (file kinds only in Wave 0; later waves extend), `pub const Mode = enum { deal, dealx };` (for the AST root mode field per D-03), and `pub const Node = struct { kind: NodeKind, span: Span };` — payload union deferred to Plan 03.

src/diagnostics.zig: declare `pub const Severity = enum(u8) { err = 0, warn = 1, info = 2, hint = 3 };` (RESEARCH §Example E line 827), `pub const SpanLabel = struct { span: ast.Span, label: []const u8 };`, `pub const FixIt = struct { replacement: []const u8, replace_span: ast.Span };`, and `pub const Diagnostic = struct { code: []const u8, severity: Severity, message: []const u8, span: ast.Span, secondary_spans: []const SpanLabel = &.{}, fix_it: ?FixIt = null, notes: []const u8 = "" };` — exact field order matches RESEARCH §Example E. The error-code namespace from D-16 is documented in a `/// E0001..E0099 = lexer; E0100..E0299 = parser-deal; E0300..E0399 = parser-dealx; E0400..E0499 = recovery; W0500..W0599 = warning; H0600..H0699 = hint` comment on the Diagnostic struct.

src/source_map.zig: declare `pub const SourceMap = struct { source: []const u8, line_starts: ?[]const u32 = null, pub fn lineCol(self: *SourceMap, allocator: std.mem.Allocator, offset: u32) struct { line: u32, col: u32 } { _ = self; _ = allocator; _ = offset; return .{ .line = 1, .col = 1 }; } };` — stub body returns 1:1; Plan 02 fills in the lazy line-start table.

src/lexer.zig: declare `pub const Mode = enum { deal_def, dealx_outer, dealx_tag, dealx_expr_brace };` (D-06), `pub const Tag = enum { eof, unknown };` (Wave 0 minimal), `pub const Token = struct { tag: Tag, span: ast.Span };`, `pub const Lexer = struct { source: []const u8, pos: u32 = 0, pub fn init(source: []const u8) Lexer { return .{ .source = source }; } pub fn next(self: *Lexer, mode: Mode) Token { _ = self; _ = mode; return .{ .tag = .eof, .span = .{ .start = 0, .end = 0 } }; } pub fn peek(self: *Lexer, mode: Mode) Token { _ = self; _ = mode; return .{ .tag = .eof, .span = .{ .start = 0, .end = 0 } }; } };`

src/keywords.zig: declare `pub const global_keywords = std.StaticStringMap(lexer.Tag).initComptime(.{});` — empty Wave 0 map; Plan 02 populates with 37 keywords.

src/expr.zig: declare `pub fn parseExpression(parser: anytype, min_bp: u8) ?*ast.Node { _ = parser; _ = min_bp; return null; }` — signature only; Plan 03 implements Pratt.

src/parser_deal.zig: declare `pub fn parseFile(handle: *Handle) ?*ast.Node { _ = handle; return null; }` where `Handle` is forward-declared as `pub const Handle = opaque {};` for now — the real handle type is defined in lib.zig and imported in Plan 02+. Use `extern struct` only for ABI-crossing types.

src/parser_dealx.zig: declare `pub fn parseFile(handle: *parser_deal.Handle) ?*ast.Node { _ = handle; return null; }`.

src/parser.zig: declare `pub fn parseFile(handle: *parser_deal.Handle, mode: ast.Mode) ?*ast.Node { return switch (mode) { .deal => parser_deal.parseFile(handle), .dealx => parser_dealx.parseFile(handle) }; }` — dispatcher.

src/json.zig: declare `pub fn emitAst(allocator: std.mem.Allocator, root: ?*ast.Node, mode: ast.Mode, filename: []const u8) ![]const u8 { _ = root; const tag = if (mode == .deal) "deal" else "dealx"; return std.fmt.allocPrint(allocator, "{{\"v\":1,\"mode\":\"{s}\",\"filename\":\"{s}\",\"root\":null}}", .{ tag, filename }); } pub fn emitDiagnostics(allocator: std.mem.Allocator, diags: []const diagnostics.Diagnostic) ![]const u8 { _ = diags; return allocator.dupe(u8, "[]"); }` — produces minimum valid JSON shaped per D-04.

src/lib.zig: define the C ABI per D-10/D-11/D-12/D-13:

  - `pub const DealHandle = struct { arena: std.heap.ArenaAllocator, ast_root: ?*ast.Node = null, mode: ast.Mode = .deal, source: []const u8 = "", filename: []const u8 = "", diagnostics: std.ArrayList(diagnostics.Diagnostic) = .empty, cached_ast_json: ?[]const u8 = null, cached_diag_json: ?[]const u8 = null };`
  - `pub export fn deal_parse(source_ptr: [*]const u8, source_len: usize, filename_ptr: [*]const u8, filename_len: usize) callconv(.c) ?*anyopaque { ... }` — allocates an arena via `std.heap.ArenaAllocator.init(std.heap.page_allocator)`, dupes source + filename into the arena, sets `mode = if (std.mem.endsWith(u8, filename, ".dealx")) .dealx else .deal` (D-12 extension-based mode selection), sets `ast_root = null` (stub), returns the handle ptr cast to `*anyopaque`. On alloc fail return null. Use `arena.allocator().create(DealHandle)` to allocate the handle inside the arena so a single `arena.deinit()` frees everything.
  - `pub export fn deal_free(handle_ptr: ?*anyopaque) callconv(.c) void { if (handle_ptr) |p| { const h: *DealHandle = @ptrCast(@alignCast(p)); var arena = h.arena; arena.deinit(); } }` — copy arena out before deinit per RESEARCH §Pattern 6 lines 492-496.
  - `pub export fn deal_has_errors(handle_ptr: ?*anyopaque) callconv(.c) bool { if (handle_ptr) |p| { const h: *DealHandle = @ptrCast(@alignCast(p)); return h.diagnostics.items.len > 0; } return false; }`
  - `pub export fn deal_diagnostics_count(handle_ptr: ?*anyopaque) callconv(.c) u32 { if (handle_ptr) |p| { const h: *DealHandle = @ptrCast(@alignCast(p)); return @intCast(h.diagnostics.items.len); } return 0; }`
  - `pub export fn deal_ast_json(handle_ptr: ?*anyopaque, out_ptr: *[*]const u8, out_len: *usize) callconv(.c) bool { ... }` — cache JSON on first call (D-11 lazy emission); call `json.emitAst(handle.arena.allocator(), handle.ast_root, handle.mode, handle.filename)` and store in `handle.cached_ast_json`. Write `out_ptr.* = handle.cached_ast_json.?.ptr; out_len.* = handle.cached_ast_json.?.len; return true;`. On allocator failure return false without writing.
  - `pub export fn deal_diagnostics_json(handle_ptr: ?*anyopaque, out_ptr: *[*]const u8, out_len: *usize) callconv(.c) bool { ... }` — mirror of the above using `json.emitDiagnostics`.

  At top of file: `const std = @import("std"); const ast = @import("ast.zig"); const lexer = @import("lexer.zig"); const parser = @import("parser.zig"); const diagnostics = @import("diagnostics.zig"); const json = @import("json.zig");`. Also add `test "stub compiles" { ... }` block that allocates + frees a handle via the public ABI to give `zig build test` something to run and to give Plan 02 a foothold.

Create deal/include/deal.h as a hand-written C header (per D-11, RESEARCH §Pitfall 6 lines 610-619 — do NOT rely on `-femit-h`). Required content:

  - `#ifndef DEAL_H` / `#define DEAL_H` / `#endif` include guard
  - `#include <stddef.h>` and `#include <stdint.h>` and `#include <stdbool.h>`
  - Forward-declare `typedef struct DealHandle DealHandle;` (opaque)
  - Six function prototypes matching the six Zig exports exactly:
    - `DealHandle* deal_parse(const uint8_t* source_ptr, size_t source_len, const uint8_t* filename_ptr, size_t filename_len);`
    - `void deal_free(DealHandle* handle);`
    - `bool deal_has_errors(DealHandle* handle);`
    - `uint32_t deal_diagnostics_count(DealHandle* handle);`
    - `bool deal_ast_json(DealHandle* handle, const uint8_t** out_ptr, size_t* out_len);`
    - `bool deal_diagnostics_json(DealHandle* handle, const uint8_t** out_ptr, size_t* out_len);`
  - Doxygen-style block comment above each prototype documenting: ownership rules (handle owned until deal_free; out_ptr buffers owned by handle and freed by deal_free per D-11); thread model (per-handle affinity, no shared handles across threads per D-13); error-reporting model (handle is non-null unless allocator failed; check deal_has_errors / deal_diagnostics_json — see D-10).
  </action>
  <acceptance_criteria>
    - All 11 files in deal/src/*.zig exist with at least one `pub` declaration each
    - deal/src/lib.zig contains all six literal strings `deal_parse`, `deal_free`, `deal_has_errors`, `deal_diagnostics_count`, `deal_ast_json`, `deal_diagnostics_json`
    - deal/src/lib.zig contains `callconv(.c)` exactly (lowercase) and NOT `callconv(.C)` (uppercase)
    - deal/src/lib.zig contains `std.heap.ArenaAllocator.init` and `arena.deinit()`
    - deal/src/ast.zig declares `Span` as an `extern struct` with `start: u32, end: u32`
    - deal/src/diagnostics.zig contains all of: `Severity`, `SpanLabel`, `FixIt`, `Diagnostic`, `secondary_spans`, `fix_it`, `notes`
    - deal/src/lexer.zig declares the four-variant Mode enum `deal_def`, `dealx_outer`, `dealx_tag`, `dealx_expr_brace`
    - deal/src/keywords.zig uses `std.StaticStringMap` and `initComptime` (not runtime `init`)
    - deal/include/deal.h contains all six function prototypes and `typedef struct DealHandle DealHandle;`
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build 2>&1 — exits 0, produces zig-out/lib/libdeal.a and zig-out/include/deal.h</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test 2>&1 — exits 0 (the stub-compiles test runs and passes)</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && grep -v '^[[:space:]]*//' src/lib.zig | grep -c 'callconv(\.C)' produces 0; grep -c 'callconv(\.c)' produces 6 (one per export)</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build && zig build test && test -f zig-out/lib/libdeal.a && test -f zig-out/include/deal.h && echo OK</automated>
  </verify>
  <done>
    Eleven Zig stubs + hand-written deal.h compile via `zig build` into `zig-out/lib/libdeal.a` and `zig-out/include/deal.h`. `zig build test` runs the stub-compile test successfully. All exports use `callconv(.c)` lowercase. The DealHandle owns an arena that frees the entire handle on `deal_free`. AST JSON stub emits `{"v":1,"mode":"<mode>","filename":"<name>","root":null}` per the D-04 schema. Mode selection by `.deal` vs `.dealx` filename suffix works.
  </done>
</task>

<task type="auto">
  <name>Task 1.3: Rust FFI harness scaffold</name>
  <read_first>
    - deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md (D-10, D-11, D-12, D-13)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Example D: Rust FFI build.rs linking libdeal.a (macOS arm64)" (lines 747-822)
    - deal/src/lib.zig (just-written exports)
    - deal/include/deal.h (just-written header)
  </read_first>
  <files>
    deal/tests/ffi/Cargo.toml,
    deal/tests/ffi/build.rs,
    deal/tests/ffi/tests/parse.rs
  </files>
  <action>
Create `deal/tests/ffi/Cargo.toml` as a standalone Cargo package named `deal-ffi-tests`, edition 2021, version 0.1.0, with `[lib]` declaring it as a rust library (or no [lib] section — the integration test in `tests/parse.rs` is sufficient on its own). Add `[build-dependencies]` empty and no runtime dependencies. Set `links = "deal"` in the `[package]` table to declare the native library dependency for cargo's link tracking.

Create `deal/tests/ffi/build.rs` per RESEARCH §Example D lines 747-776:

  1. Read `CARGO_MANIFEST_DIR` env var; compute `deal_dir = PathBuf::from(manifest_dir).join("../..")` (resolves to `deal/` from `deal/tests/ffi/`).
  2. Run `Command::new("zig").arg("build").current_dir(&deal_dir).status()`. Assert success with a clear panic message if it fails.
  3. Print `cargo::rustc-link-search=native={lib_dir}` where `lib_dir = deal_dir.join("zig-out/lib")`.
  4. Print `cargo::rustc-link-lib=static=deal`.
  5. Print `cargo::rerun-if-changed={deal_dir}/src` and `cargo::rerun-if-changed={deal_dir}/build.zig` and `cargo::rerun-if-changed={deal_dir}/build.zig.zon` and `cargo::rerun-if-changed={deal_dir}/include/deal.h`.

  Use the double-colon `cargo::` namespace (Cargo 1.77+) per RESEARCH §State of the Art line 863. Use forward slashes in path-display; `PathBuf::display` handles platform separators.

Create `deal/tests/ffi/tests/parse.rs` per RESEARCH §Example D lines 778-820 BUT scoped to a stub-level smoke test (the real gate test lives in Plan 06):

  1. Declare the same `#[repr(C)] struct DealHandle { _opaque: [u8; 0] }` opaque type.
  2. `extern "C"` block declaring all six functions matching `deal.h` exactly: `deal_parse`, `deal_free`, `deal_has_errors`, `deal_diagnostics_count`, `deal_ast_json`, `deal_diagnostics_json`.
  3. `#[test] fn ffi_smoke()` — empty source `b""`, filename `b"test.deal"`, call `deal_parse(source.as_ptr(), 0, filename.as_ptr(), filename.len())`, assert handle != null, call `deal_has_errors(handle)` (any return is fine — diagnostics may be empty), call `deal_ast_json(handle, &mut ptr, &mut len)`, assert it returned true and the JSON contains `"v":1` and `"mode":"deal"` and `"filename":"test.deal"`, call `deal_free(handle)`. The entire test body lives inside one `unsafe { ... }` block.
  4. Add a second `#[test] fn ffi_dealx_mode_detection()` — same as above but with filename `b"test.dealx"`, asserting the AST JSON contains `"mode":"dealx"` (D-12 extension-based mode selection from Task 1.2).

Do NOT add the `ffi_smoke_battery` test that loads a real showcase file — that goes in Plan 06's gate (after the parser is real). Wave 0 stays minimal.
  </action>
  <acceptance_criteria>
    - deal/tests/ffi/Cargo.toml contains `name = "deal-ffi-tests"` and `links = "deal"`
    - deal/tests/ffi/build.rs contains literal `cargo::rustc-link-lib=static=deal` and `cargo::rustc-link-search=native=`
    - deal/tests/ffi/build.rs invokes `Command::new("zig")` with arg `"build"`
    - deal/tests/ffi/tests/parse.rs contains `extern "C"` block and the six function declarations, plus `#[test] fn ffi_smoke` and `#[test] fn ffi_dealx_mode_detection`
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml 2>&1 — exits 0, both ffi_smoke and ffi_dealx_mode_detection pass (verifying that the stub deal_parse returns non-null, deal_ast_json emits the D-04 envelope with correct mode, and deal_free does not crash)</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml -- --nocapture 2>&1 | grep -c "test result: ok" — expect ≥ 1</automated>
  </verify>
  <done>
    Rust harness builds and links libdeal.a via build.rs; two smoke tests pass against the stub C ABI verifying handle non-null, mode-by-extension dispatch, and clean free. The harness pattern (build.rs invokes `zig build`, cargo links `static=deal`) is now proven on this exact macOS arm64 + Zig 0.16.0 + Rust 1.93 stack — closing assumption A9 in RESEARCH.md.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Rust caller → C ABI | The FFI test harness (and later, all Rust frontends) cross into Zig via `extern "C"` calls. Caller-provided byte slices and length pairs cross the boundary. |
| Source bytes → arena | `deal_parse` dupes caller bytes into the per-handle arena. Once duped, the bytes are owned by the handle until `deal_free`. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-01-01 | Tampering | deal_parse source_ptr / source_len pair | mitigate | source_len is `usize` from the caller; in Plan 01 we accept it as-is (stub does no parsing). Plan 02+ adds the actual length-bound check + UTF-8 validation. |
| T-01-02 | DoS / Resource Exhaustion | deal_parse arena allocation | mitigate | `std.heap.ArenaAllocator.init(page_allocator)` is single-OS-alloc + sub-allocations from one block; failure returns null (D-10). No leak path on alloc failure: `arena.deinit()` is called before returning null. |
| T-01-03 | Use-after-free | DealHandle pointer post deal_free | accept | C ABI cannot prevent caller misuse. Documented in deal.h doxygen and in RESEARCH §Threats line 1008. Rust wrapper enforces via `Drop` in Plan 06. |
| T-01-04 | Information Disclosure | Source bytes leak via cached_ast_json | accept | The whole point of the AST JSON is to expose the parsed structure. The arena-owned cache is freed by deal_free; no persistence beyond handle lifetime. |
| T-01-05 | Tampering | Filename-based mode selection | mitigate | Mode is decided by `std.mem.endsWith(u8, filename, ".dealx")` (D-12). Caller-provided filename bytes are duped before the check. Default is `.deal` (safer — the `.dealx`-only `[<` token is OFF unless explicitly opted in). |
| T-01-SC | Tampering | Zig / Rust toolchain installation | accept | No external packages are installed in Plan 01 — build.zig.zon has empty `.dependencies`. RESEARCH §Package Legitimacy Audit confirms Zig and Rust are the only external toolchain components, both pre-installed and verified at research time. No `[ASSUMED]` or `[SUS]` packages exist. |
</threat_model>

<verification>
After all three tasks complete:

1. `cd /Users/dunnock/projects/deal-lang/deal && zig build` exits 0 and produces `zig-out/lib/libdeal.a` plus `zig-out/include/deal.h`.
2. `cd /Users/dunnock/projects/deal-lang/deal && zig build test` exits 0 (at minimum the stub-compiles test passes).
3. `cd /Users/dunnock/projects/deal-lang/deal && zig build --help` lists `-Dtest-filter=[string]` as an option.
4. `cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml` exits 0 with `ffi_smoke` and `ffi_dealx_mode_detection` passing.
5. `grep -c 'callconv(\.C)' deal/src/*.zig` returns 0 (no uppercase `.C` anywhere).
</verification>

<success_criteria>
- All Wave 0 Gaps from RESEARCH.md lines 970-979 are filled (every file in the list exists)
- Six C ABI symbols are exported from libdeal.a and consumable from Rust
- include/deal.h is hand-written and committed (not -femit-h generated)
- The Rust FFI link recipe (build.rs invokes zig build; cargo::rustc-link-lib=static=deal) is proven to work on this host
- `-Dtest-filter=<area>` is wired so subsequent plans can write targeted tests like `zig build test -Dtest-filter=lexer.snapshot`
- Filename-based mode selection (.deal vs .dealx) is observable through the stub AST JSON, validating D-12 end-to-end before any real parsing is added
- All Zig 0.16.0 breaking changes are honored: `callconv(.c)` lowercase, `ArrayList(T) = .empty` with explicit allocator, `std.StaticStringMap(V).initComptime(.{})`, `b.addLibrary(.{ .linkage = .static, .root_module = b.createModule(...) })`
</success_criteria>

<output>
Create `.planning/phases/01-zig-compiler-core/01-01-SUMMARY.md` when done, recording:
- Confirmed Zig 0.16.0 API names actually used (resolves RESEARCH assumptions A1, A4 about `installFile`/`addInstallFile` and `utf8ValidateSlice` exact spellings)
- Confirmed test-filter mechanism actually used (`tests.filters` vs build-option threading)
- The exact `cargo::rustc-link-search=native=...` path printed by build.rs (for Plan 06 documentation)
- Any deviations from the RESEARCH §Example A build.zig template forced by 0.16.0 reality
</output>
