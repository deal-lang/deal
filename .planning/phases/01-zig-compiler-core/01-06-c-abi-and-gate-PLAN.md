---
phase: 01-zig-compiler-core
plan: 06
type: execute
wave: 6
depends_on:
  - 01-05-error-recovery
files_modified:
  - deal/src/lib.zig
  - deal/include/deal.h
  - deal/src/json.zig
  - deal/tests/unit/c_abi_invalid_utf8.zig
  - deal/tests/unit/c_abi_no_leaks.zig
  - deal/tests/unit/c_abi_source_too_large.zig
  - deal/tests/ffi/tests/parse.rs
  - deal/tests/ffi/tests/gate.rs
  - deal/build.zig
autonomous: true
requirements:
  - REQ-phase-1-5-c-abi
  - REQ-phase-1-gate

must_haves:
  decisions_implemented:
    - "D-02: Arena freed by `deal_free` — final wiring confirmed by `c_abi.no_leaks` test running `std.testing.allocator` across ~70 files (19 showcase + ≥50 malformed) with zero leaks reported"
    - "D-04: AST JSON emitted via `deal_ast_json` — D-04 envelope verified by `gate_all_19` Rust test asserting every showcase file's JSON starts with `{\"v\":1,\"mode\":\"`"
    - "D-10: `deal_parse` non-null even on fatal error — UTF-8 invalid input + oversize source_len both return non-null handle carrying the diagnostic; verified by `c_abi.invalid_utf8` (5 cases) + `c_abi.source_too_large` + `ffi_invalid_utf8`"
    - "D-11: Length-prefixed UTF-8 accessors — `deal_ast_json(handle, out_ptr, out_len)` and `deal_diagnostics_json(...)` return arena-owned buffers; lazy + cached on first call (calling twice returns identical (ptr, len)); verified by `ffi_ast_json_cache_stable`"
    - "D-12: Filename hint selects mode (no Zig file I/O) — `handle.mode = endsWith(filename, \".dealx\") ? .dealx : .deal`; verified by `ffi_parse_dealx_real` reading back `\"mode\":\"dealx\"` for `vehicle.dealx`"
    - "D-13: Per-handle thread affinity (no global state) — no statics, no locks in lib.zig; documented in deal.h doxygen `@thread_safety`; symbol table check (`nm -gU`) confirms exactly 6 `deal_*` exports and zero `deal_*_internal` test helpers leak into the ABI"
    - "D-14: `deal_diagnostics_json` exposes full Diagnostic struct — every field (code, severity, message, span, secondary_spans, fix_it, notes) survives the C ABI; verified by `ffi_diagnostics_on_malformed` parsing the JSON via `serde_json`"
    - "D-15: Span round-trips through JSON — `[start, end]` u32 byte-offset array preserved through deal_ast_json and deal_diagnostics_json"
    - "D-18: All emitter outputs preserve alphabetical field order — `json.emitAst` (Plan 03/04) and `json.emitDiagnostics` (Plan 05) both hand-rolled with alphabetical payload-field convention; verified by snapshot byte-stability across all 19 showcase files"
  truths:
    - "deal_parse on invalid UTF-8 returns a non-null handle carrying exactly one diagnostic with code E0001; does not panic"
    - "deal_parse on source > 4 GiB (or equivalently — source_len that overflows u32 spans) returns a handle with diagnostic E0004 and no parsing attempted"
    - "Every allocation made during a parse is freed by deal_free; std.testing.allocator reports zero leaks across an end-to-end parse + free of every showcase + malformed file"
    - "Per-handle thread affinity (D-13): two threads may each parse different sources concurrently without contention or shared state; a single handle is NOT shared across threads (documented in deal.h doxygen)"
    - "deal_ast_json and deal_diagnostics_json buffers are owned by the handle's arena; calling deal_free invalidates the buffer pointers; calling either accessor twice on the same handle returns identical bytes (caching per D-11)"
    - "The Rust FFI gate test (cargo test gate_all_19) loads libdeal.a, parses every one of the 19 showcase files via deal_parse, reads back AST JSON, verifies `{\"v\":1,\"mode\":\"...\"}` envelope, and calls deal_free — zero failures, zero leaks reported by the Rust test runner"
    - "All six C ABI exports use callconv(.c) lowercase (Zig 0.16.0); deal.h has matching prototypes; struct layouts on the boundary are extern struct where they cross"
    - "Test-only helpers `deal_parse_internal` and `deal_free_internal` are pub-but-NOT-@exported (no `callconv(.c)` annotation, no `export` keyword); they accept a caller-provided `std.mem.Allocator` so std.testing.allocator can back the arena for leak detection; they MUST NOT appear in the libdeal.a symbol table"
  artifacts:
    - path: "deal/src/lib.zig"
      provides: "Production C ABI implementation: utf8 validation at top of deal_parse; source_len bound check at u32 max; real parser invocation; lazy + cached JSON emission; arena-owned everything. Also defines test-only helpers `deal_parse_internal(allocator, source, filename)` and `deal_free_internal(handle)` — pub-but-NOT-@exported (no callconv(.c) annotation, no `export` keyword); allocator-injectable for std.testing.allocator leak detection in c_abi.no_leaks; MUST NOT appear in `nm` symbol table (enforced by acceptance criterion in Task 6.1 — `nm | grep -E '_deal_(parse|free)_internal' | wc -l` produces 0)"
      contains: "std.unicode.utf8ValidateSlice"
    - path: "deal/include/deal.h"
      provides: "Final hand-written C header: six prototypes; doxygen documenting ownership, thread model, error model; E-code reference table"
      contains: "deal_parse"
    - path: "deal/tests/unit/c_abi_invalid_utf8.zig"
      provides: "Tests deal_parse on invalid UTF-8 byte sequences"
    - path: "deal/tests/unit/c_abi_no_leaks.zig"
      provides: "Tests that std.testing.allocator reports zero leaks after parsing every showcase + every malformed file with deal_free called on each handle. Drives the test-only `deal_parse_internal` / `deal_free_internal` helpers (declared in deal/src/lib.zig — pub-but-NOT-@exported) so the arena is backed by std.testing.allocator instead of std.heap.page_allocator."
    - path: "deal/tests/unit/c_abi_source_too_large.zig"
      provides: "Tests that source_len overflow produces E0004 without panic"
    - path: "deal/tests/ffi/tests/gate.rs"
      provides: "Phase 1 gate Rust test: load libdeal.a, parse all 19 showcase files, verify AST envelopes, free all handles"
    - path: "deal/tests/ffi/tests/parse.rs"
      provides: "Extended FFI tests: parse + diagnostics readback + double-call cache stability; supersedes Plan 01's stub-level smoke"
  key_links:
    - from: "deal/src/lib.zig deal_parse"
      to: "std.unicode.utf8ValidateSlice"
      via: "First validation step before any lexer / parser invocation; on failure emit E0001 and return handle with no ast_root"
      pattern: "utf8ValidateSlice"
    - from: "deal/include/deal.h"
      to: "deal/src/lib.zig export fn"
      via: "Six C prototypes match six Zig export fn signatures byte-for-byte (return type, param count, param types)"
      pattern: "deal_(parse|free|has_errors|diagnostics_count|diagnostics_json|ast_json)"
    - from: "deal/tests/ffi/tests/gate.rs"
      to: "deal/zig-out/lib/libdeal.a"
      via: "build.rs cargo::rustc-link-lib=static=deal (Plan 01 wiring intact)"
      pattern: "deal_parse|deal_ast_json|deal_free"
    - from: "deal/tests/unit/c_abi_no_leaks.zig"
      to: "std.testing.allocator"
      via: "Default Zig test runner leak-detection: every allocation must be freed before test exit (RESEARCH line 1025 / threat T-resource line 1009)"
      pattern: "std\\.testing\\.allocator"
    - from: "deal/tests/unit/c_abi_no_leaks.zig"
      to: "deal/src/lib.zig deal_parse_internal"
      via: "Test-only entry point taking caller-provided allocator; pub-but-NOT-@exported so it stays out of the C ABI symbol table while remaining callable from in-tree tests"
      pattern: "deal_parse_internal"
---

<objective>
Complete the C ABI surface: harden `deal_parse` with UTF-8 validation and source-length bounds (the V5/V12 ASVS controls from RESEARCH §Security Domain); verify zero-leak behavior across the full showcase + malformed corpus; finalize `include/deal.h` with production-grade doxygen documenting ownership / thread model / error reporting; and ship the Rust FFI gate test that consumes all 19 showcase files end-to-end through the C ABI as the Phase 1 exit criterion.

Purpose: Plans 02–05 built the lexer, parsers, and recovery against the Zig-internal API; this plan proves that everything is consumable from outside the language boundary. The Rust FFI gate (REQ-phase-1-gate) is the single integration test that PROVES Phase 1 is shippable to Phase 2 (which builds DEAL IR v0 from the AST emitted by `deal_parse`). The leak-check (D-12 / D-13 / threat T-resource) is the security perimeter — Rust will own all integration surfaces going forward (CLI, LSP, Tauri) and cannot tolerate Zig-side leaks.

Output: `libdeal.a` is production-grade for Phase 2 consumption; `deal.h` is hand-written with doxygen documenting every contract; all 19 showcase files parse through the Rust harness producing the D-04 envelope; no `std.testing.allocator` leaks reported; `deal_parse` is robust against invalid UTF-8 and oversize inputs.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md
@deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md
@deal/.planning/phases/01-zig-compiler-core/01-VALIDATION.md
@deal/.planning/phases/01-zig-compiler-core/01-01-SUMMARY.md
@deal/.planning/phases/01-zig-compiler-core/01-05-SUMMARY.md
@deal/src/lib.zig
@deal/src/json.zig
@deal/src/parser.zig
@deal/include/deal.h
@deal/tests/ffi/Cargo.toml
@deal/tests/ffi/build.rs
@deal/tests/ffi/tests/parse.rs
@deal/build.zig
@spec/references/zig-lang/0.16.0/langref.html

<interfaces>
From deal/src/lib.zig (Plan 01 + Plan 03 + Plan 05 — final tightening here):
- `pub const DealHandle = struct { arena: std.heap.ArenaAllocator, ast_root: ?*ast.Node = null, mode: ast.Mode = .deal, source: []const u8 = "", filename: []const u8 = "", diagnostics: std.ArrayList(diagnostics.Diagnostic) = .empty, cached_ast_json: ?[]const u8 = null, cached_diag_json: ?[]const u8 = null };`
- `pub export fn deal_parse(source_ptr, source_len, filename_ptr, filename_len) callconv(.c) ?*anyopaque` — Plan 03 wired the real parser; this plan adds UTF-8 validation + length-bounds at the top.
- `pub export fn deal_free`, `deal_has_errors`, `deal_diagnostics_count`, `deal_ast_json`, `deal_diagnostics_json` — all callconv(.c) lowercase. This plan adds explicit `?*anyopaque` null-check at every entry and a one-line doc comment summarizing the C-side contract.

From deal/src/json.zig (Plan 05):
- `pub fn emitAst(allocator, root, mode, filename) ![]const u8;`
- `pub fn emitDiagnostics(allocator, diags) ![]const u8;`

From deal/include/deal.h (Plan 01):
- Six prototypes + opaque `DealHandle` typedef. This plan tightens the documentation and confirms the prototypes match the final Zig exports.

From deal/tests/ffi/tests/parse.rs (Plan 01):
- `ffi_smoke` + `ffi_dealx_mode_detection` tests that ran against the stub. This plan extends with real-parsing tests and replaces the smoke tests with substantively stronger assertions.
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 6.1: Harden deal_parse — UTF-8 validation, source-length bounds, null-handle guards, finalize deal.h</name>
  <read_first>
    - deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md (D-10 always-non-null handle except on alloc failure; D-11 length-prefixed UTF-8 buffer accessors; D-12 input is bytes + filename hint, no file I/O; D-13 per-handle thread affinity)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Pattern 6: C ABI with opaque handle" (lines 447-515), §"Security Domain" §V5 (line 991 — UTF-8 validation), §"Threat Patterns" lines 1004-1013 (esp. T-source_len overflow line 1011, T-msg_injection line 1012)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Don't Hand-Roll" line 532 (`std.unicode.utf8ValidateSlice`)
    - deal/src/lib.zig (Plan 01 + Plan 03 deal_parse body; Plan 05 deal_diagnostics_json body)
    - deal/include/deal.h (Plan 01 — extend doxygen)
    - deal/.planning/phases/01-zig-compiler-core/01-05-SUMMARY.md (for the Codes.e_invalid_utf8 = "E0001" and Codes.e_source_too_large = "E0004" constants)
  </read_first>
  <files>
    deal/src/lib.zig,
    deal/include/deal.h
  </files>
  <action>
1. In `deal/src/lib.zig`, refactor `deal_parse` to add the V5/V12 ASVS controls at the very top, BEFORE any lexer/parser invocation:

   ```
   pub export fn deal_parse(
       source_ptr: [*]const u8, source_len: usize,
       filename_ptr: [*]const u8, filename_len: usize,
   ) callconv(.c) ?*anyopaque {
       // Step 1: allocate arena + handle (D-10 — null only on alloc failure)
       var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
       const allocator = arena.allocator();
       const handle = allocator.create(DealHandle) catch {
           arena.deinit();
           return null;
       };
       handle.* = .{ .arena = arena, .source = "", .filename = "" };

       // Step 2: dupe filename + decide mode (D-12 — extension-based)
       const filename_slice = filename_ptr[0..filename_len];
       handle.filename = allocator.dupe(u8, filename_slice) catch {
           handle.arena.deinit();
           return null;
       };
       handle.mode = if (std.mem.endsWith(u8, handle.filename, ".dealx")) .dealx else .deal;

       // Step 3: V12 bound check — source_len must fit in u32 (D-15 span is u32)
       if (source_len > std.math.maxInt(u32)) {
           const diag = diagnostics.Diagnostic{
               .code = diagnostics.Codes.e_source_too_large,
               .severity = .err,
               .message = "source larger than 4 GiB; spans cannot address it",
               .span = .{ .start = 0, .end = 0 },
           };
           handle.diagnostics.append(allocator, diag) catch {};
           return @ptrCast(handle);
       }

       // Step 4: dupe source bytes (D-13 — independent of caller buffer)
       const source_slice = source_ptr[0..source_len];
       handle.source = allocator.dupe(u8, source_slice) catch {
           handle.arena.deinit();
           return null;
       };

       // Step 5: V5 UTF-8 validation (RESEARCH line 991, line 519, line 1010)
       if (!std.unicode.utf8ValidateSlice(handle.source)) {
           const diag = diagnostics.Diagnostic{
               .code = diagnostics.Codes.e_invalid_utf8,
               .severity = .err,
               .message = "source bytes are not valid UTF-8",
               .span = .{ .start = 0, .end = @intCast(@min(handle.source.len, std.math.maxInt(u32))) },
           };
           handle.diagnostics.append(allocator, diag) catch {};
           // D-10 — return handle with no ast_root; do NOT attempt parsing on invalid UTF-8
           return @ptrCast(handle);
       }

       // Step 6: parse (Plan 03/04 work)
       handle.ast_root = parser.parseFile(handle) catch null;
       return @ptrCast(handle);
   }
   ```

   - Confirm `std.unicode.utf8ValidateSlice` exists in 0.16.0 std-lib (resolves RESEARCH assumption A1). If the function name differs (e.g. `std.unicode.utf8Validate` or path under `std.unicode.utf8`), use whatever Zig 0.16.0 langref documents; record actual name in SUMMARY.
   - Allocations after the arena is created go through `allocator.dupe`. If those fail (e.g. OOM mid-parse), per D-10 we still want to return the handle so the caller can read partial diagnostics. The `catch {}` on diagnostic append is intentional — if the diagnostic-list-append itself fails, we can't do anything but proceed.
   - Step 5 emits the diagnostic and returns the handle WITHOUT calling parseFile. This is critical: invalid UTF-8 can crash the lexer's byte walker if it tries to interpret malformed sequences. The validation gate runs ONCE at the top.

2. Add null-handle guards to all other exports. Pattern for each:

   ```
   pub export fn deal_has_errors(handle_ptr: ?*anyopaque) callconv(.c) bool {
       const p = handle_ptr orelse return false;
       const h: *DealHandle = @ptrCast(@alignCast(p));
       return h.diagnostics.items.len > 0;
   }
   ```

   - `deal_free(null)` is a no-op (matches `free()` C semantics).
   - `deal_diagnostics_count(null) -> 0`.
   - `deal_ast_json(null, ...)` returns `false`; does NOT write to out_ptr/out_len.
   - `deal_diagnostics_json(null, ...)` returns `false`.
   - `deal_has_errors(null) -> false`.

3. Finalize the lazy + cached JSON emission in `deal_ast_json` and `deal_diagnostics_json`. The pattern (already drafted in Plan 01 / Plan 05 — confirm here):

   ```
   pub export fn deal_ast_json(
       handle_ptr: ?*anyopaque,
       out_ptr: *[*]const u8,
       out_len: *usize,
   ) callconv(.c) bool {
       const p = handle_ptr orelse return false;
       const h: *DealHandle = @ptrCast(@alignCast(p));
       if (h.cached_ast_json == null) {
           const buf = json.emitAst(h.arena.allocator(), h.ast_root, h.mode, h.filename) catch return false;
           h.cached_ast_json = buf;
       }
       out_ptr.* = h.cached_ast_json.?.ptr;
       out_len.* = h.cached_ast_json.?.len;
       return true;
   }
   ```

   - Calling `deal_ast_json(h, ...)` twice returns identical (ptr, len) — the cached buffer is the same arena-owned slice. Tested by Task 6.2's `parse.rs::ast_json_cache_stable`.
   - Same shape for `deal_diagnostics_json` against `cached_diag_json` and `json.emitDiagnostics`.

4. Rewrite `deal/include/deal.h` with production-grade doxygen. Required content:

   - Standard include guards (`#ifndef DEAL_H` ... `#endif`) and `#include <stddef.h>`, `<stdint.h>`, `<stdbool.h>`.
   - File-level doxygen header documenting: phase 1 surface; reference to DEAL spec; ownership invariant ("Handles are owned by the caller and freed via deal_free; buffer pointers returned by deal_ast_json/deal_diagnostics_json are owned by the handle's arena and become invalid after deal_free."); thread model ("Different handles may be used concurrently from different threads. A single handle is NOT thread-safe.").
   - Opaque type: `typedef struct DealHandle DealHandle;` with comment "Opaque parse handle. Allocate via deal_parse, free via deal_free. Do not access fields directly."
   - For each of the six function prototypes, doxygen with:
     - `@brief` one-line summary.
     - `@param` documentation for every parameter (including length pairs).
     - `@returns` documentation including the null/false cases.
     - `@note` for ownership rules.
     - `@thread_safety` for per-handle affinity rules (D-13).
     - For `deal_parse`: list the documented error codes that may appear in the diagnostics on success-with-errors (E0001 invalid UTF-8, E0004 source too large, E0100-E0299 parser-deal, E0300-E0399 parser-dealx, E0400-E0499 recovery).
     - For `deal_ast_json` / `deal_diagnostics_json`: document that buffers are valid until deal_free; UTF-8 length-prefixed (no NUL terminator); calling twice returns identical bytes.
   - Reference table at the bottom: a comment block listing the D-16 error-code namespace ranges. This makes deal.h self-documenting for Rust consumers who haven't read CONTEXT.md.
   - C++ guard: `#ifdef __cplusplus extern "C" { #endif` around the prototypes; matching close.

5. Verify that the six Zig export-fn signatures match the six C prototypes byte-for-byte. Use a simple `zig build && cd zig-out && nm -gU lib/libdeal.a | grep _deal_` (macOS) or `nm -D --defined-only lib/libdeal.a | grep deal_` (Linux) to confirm the symbol table exposes exactly the six expected names. Document the platform command used in SUMMARY.
  </action>
  <acceptance_criteria>
    - deal/src/lib.zig contains the literal string `std.unicode.utf8ValidateSlice` (or the actual 0.16.0 name; verify and document)
    - deal/src/lib.zig contains both `Codes.e_invalid_utf8` and `Codes.e_source_too_large` references
    - deal/src/lib.zig contains `handle_ptr orelse return` in all 6 export-fn bodies (verified by grep — each accepts `?*anyopaque` and handles null)
    - deal/include/deal.h contains doxygen blocks for all 6 functions (verified by grep `@brief` count ≥ 6)
    - deal/include/deal.h contains `extern "C"` guard for C++ inclusion
    - deal/include/deal.h contains the error-code reference table (grep for "E0001", "E0100", "E0300", "E0400")
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build 2>&1 — exits 0; libdeal.a regenerates</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && nm -gU zig-out/lib/libdeal.a 2>/dev/null | grep -o '_deal_[a-z_]*' | sort -u | wc -l — produces EXACTLY 6 (the six C ABI exports: _deal_parse, _deal_free, _deal_has_errors, _deal_diagnostics_count, _deal_ast_json, _deal_diagnostics_json; on Linux substitute `nm -D --defined-only`)</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && nm -gU zig-out/lib/libdeal.a 2>/dev/null | grep -E '_deal_(parse|free)_internal' | wc -l — produces EXACTLY 0 (the test-only `deal_parse_internal` and `deal_free_internal` are pub-but-NOT-@exported and MUST NOT appear in the symbol table; on Linux substitute `nm -D --defined-only`. Warning #3 enforcement.)</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build && grep -c '@brief' include/deal.h</automated>
  </verify>
  <done>
    deal_parse now performs UTF-8 validation (V5 ASVS) and source-length bound checking (V12 ASVS / D-15 u32 spans) BEFORE invoking the lexer. All 6 exports handle null `handle_ptr` gracefully. deal.h is hand-written with full doxygen covering ownership, thread model, error model, and the D-16 error-code reference. The library still builds and the symbol table exposes exactly six `deal_*` functions.
  </done>
</task>

<task type="auto">
  <name>Task 6.2: C ABI unit tests (invalid_utf8 + source_too_large + no_leaks) + Rust FFI gate test (all 19 files)</name>
  <read_first>
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Phase Requirements → Test Map" lines 959-962 (c_abi.invalid_utf8, c_abi.no_leaks, ffi_smoke, gate_all_19)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Example D: Rust FFI build.rs" (lines 747-822 — extern declarations and the parse-battery test as the model for gate.rs)
    - deal/.planning/phases/01-zig-compiler-core/01-VALIDATION.md (Sampling Rate, Phase gate command)
    - deal/src/lib.zig (Task 6.1 — final exports)
    - deal/include/deal.h (Task 6.1 — final header)
    - deal/tests/ffi/tests/parse.rs (Plan 01 — extend with new tests)
    - spec/references/zig-lang/0.16.0/langref.html §"std.testing.allocator" L1810-1849 (leak detection)
  </read_first>
  <files>
    deal/tests/unit/c_abi_invalid_utf8.zig,
    deal/tests/unit/c_abi_source_too_large.zig,
    deal/tests/unit/c_abi_no_leaks.zig,
    deal/tests/ffi/tests/parse.rs,
    deal/tests/ffi/tests/gate.rs,
    deal/build.zig
  </files>
  <action>
1. Create `deal/tests/unit/c_abi_invalid_utf8.zig` with `test "c_abi.invalid_utf8"`:
   - Construct invalid-UTF-8 byte sequences (each at most 8 bytes — small enough to inspect easily):
     - Case A: lone continuation byte: `&[_]u8{ 0x80 }` (0x80 is a continuation byte with no leading byte).
     - Case B: invalid start byte: `&[_]u8{ 0xff }` (0xff is never valid in UTF-8).
     - Case C: truncated 2-byte sequence: `&[_]u8{ 0xc3 }` (high bit indicates a 2-byte sequence but second byte is missing).
     - Case D: overlong encoding: `&[_]u8{ 0xc0, 0x80 }` (encodes U+0000 in 2 bytes; should be rejected per RFC 3629).
     - Case E: valid prefix + invalid mid: `&[_]u8{ 'a', 'b', 0xff, 'c' }` — the parser must reject the entire input even though "ab" is valid prefix.
   - For each case: call `lib.deal_parse(bytes.ptr, bytes.len, "test.deal".ptr, 9)`; assert handle != null (D-10); call `lib.deal_has_errors(handle)` → true; call `lib.deal_diagnostics_count(handle)` → 1; emit diagnostics JSON via `lib.deal_diagnostics_json` and parse it; assert the single diagnostic's code is `E0001`; assert `ast_root == null` (the parser was not invoked); call `lib.deal_free(handle)`.

2. Create `deal/tests/unit/c_abi_source_too_large.zig` with `test "c_abi.source_too_large"`:
   - This case is tricky because we can't actually allocate > 4 GiB in a test. Instead we invoke `deal_parse` with a SMALL byte buffer but pass a deliberately oversized `source_len`. The check happens BEFORE the buffer is read, so an oversized length value triggers the bound check without ever reading invalid memory.
   - Call `lib.deal_parse(some_small_buf.ptr, std.math.maxInt(u32) + 1, "test.deal".ptr, 9)` (a `usize` value > `maxInt(u32)`).
   - Assert handle != null; assert exactly one diagnostic with code `E0004`; assert `ast_root == null`.
   - WARNING: passing a fake source_len risks the parser reading past the buffer if the bound check is missing. The test is the gate that proves the bound check fires FIRST. Add a comment in the test citing this.

3. Create `deal/tests/unit/c_abi_no_leaks.zig` with `test "c_abi.no_leaks"`:
   - Use `std.testing.allocator` as the BACKING allocator for an ArenaAllocator that we substitute INTO the DealHandle (a debug-only path). To avoid invasive changes to deal_parse for testing, the test reaches inside the lib.zig API to use an alternate entry point `deal_parse_with_allocator(source, filename, allocator)` that the test exposes for internal use only — add this as a `pub fn` in lib.zig (NOT exported, no `callconv(.c)`) for test use:
     ```
     pub fn deal_parse_internal(allocator: std.mem.Allocator, source: []const u8, filename: []const u8) !*DealHandle { ... same logic as deal_parse but with caller-provided arena-backing allocator ... }
     pub fn deal_free_internal(handle: *DealHandle) void { ... same as deal_free but does NOT call arena.deinit() — caller controls; instead returns the arena so the caller can deinit ... }
     ```
   - In the test: for each file in `tests/showcase/*.{deal,dealx}` AND each file in `tests/malformed/*.{deal,dealx}` (combined ~70 files):
     - Read the file bytes with `std.testing.allocator`.
     - Call `deal_parse_internal(std.testing.allocator, bytes, filename)` — the arena uses `std.testing.allocator` as the page-allocator backing.
     - Read AST JSON + diagnostics JSON via the public accessors (forcing the cache to populate).
     - Call `deal_free_internal(handle)` and explicitly arena.deinit() it.
     - Free the source bytes with std.testing.allocator.
   - After the loop, the test framework's `std.testing.allocator` leak-detection (langref L1810-1849) automatically reports any leaks at test exit. The test passes if there are zero leaks.
   - DOCUMENT the design choice in SUMMARY: the internal `deal_parse_internal` is NOT part of the C ABI (it's pub-but-not-export). It exists solely for the leak test because std.testing.allocator cannot back std.heap.page_allocator.

4. Extend `deal/tests/ffi/tests/parse.rs` with additional Rust tests beyond Plan 01's stub-level smoke. Replace the `ffi_smoke` test body so it now exercises the REAL parser (not the stub):

   - `#[test] fn ffi_parse_battery_real()`:
     - Load `tests/showcase/packages/vehicle/battery.deal` bytes.
     - Call deal_parse; assert handle non-null.
     - Call deal_has_errors → assert false (battery.deal is well-formed).
     - Call deal_ast_json; assert the returned JSON contains `"v":1`, `"mode":"deal"`, `"filename":"battery.deal"`, and `"k":"deal_file"`.
     - Call deal_free.
   - `#[test] fn ffi_parse_dealx_real()`:
     - Same but for `tests/showcase/model/vehicle.dealx`.
     - Assert AST JSON contains `"mode":"dealx"`, `"k":"dealx_file"`, AND at least one `"k":"comp_connect"` (the D-09 first-class kind — vehicle.dealx contains the canonical connect example).
   - `#[test] fn ffi_diagnostics_on_malformed()`:
     - Load `tests/malformed/m02_unclosed_brace.deal` bytes.
     - Call deal_parse; assert handle non-null.
     - Call deal_has_errors → true.
     - Call deal_diagnostics_count → ≥ 1.
     - Call deal_diagnostics_json; parse the returned JSON via `serde_json::from_slice` (add `serde_json = "1"` to Cargo.toml dev-dependencies); assert the array has ≥ 1 element; assert at least one entry has `"code"` matching `E0100`..`E0499` (a parser error code).
     - Call deal_free.
   - `#[test] fn ffi_ast_json_cache_stable()`:
     - Parse any showcase file; call deal_ast_json twice; assert (ptr, len) are identical the second time (cached) AND the bytes are identical.
     - Call deal_free.
   - `#[test] fn ffi_invalid_utf8()`:
     - Pass invalid bytes `&[0xff, 0xfe, 0xfd]`; assert handle non-null; assert exactly one diagnostic with code `E0001`; assert AST JSON has `"root":null`.

5. Create `deal/tests/ffi/tests/gate.rs` — the Phase 1 gate test:

   - `#[test] fn gate_all_19()` is the SINGLE test that proves Phase 1 exits successfully.
   - Enumerate the 19 showcase paths (15 .deal + 4 .dealx) as a constant array.
   - For each path:
     - Read file bytes with `std::fs::read`.
     - Call `deal_parse(bytes.as_ptr(), bytes.len(), filename.as_ptr(), filename.len())`.
     - Assert `handle != null`.
     - Assert `deal_has_errors(handle) == false` (every showcase file is well-formed per Phase 0).
     - Call `deal_ast_json(handle, &mut ptr, &mut len)`; assert returned true.
     - Convert the JSON slice to `&str` (via `std::str::from_utf8`); assert it starts with `{"v":1,"mode":"`.
     - Parse the JSON via `serde_json::from_str(s)` (verify no parse errors — the JSON is well-formed); extract `["mode"]` and assert it matches the file extension.
     - Call `deal_free(handle)`.
   - At end: print summary "gate_all_19: 19/19 files parsed cleanly".
   - This single test IS the REQ-phase-1-gate acceptance criterion: "All 19 showcase files tokenize and parse through the Zig core; AST JSON snapshots stable; error recovery handles at least 50 malformed-input cases without panic; C ABI boundary proven with Rust harness."

6. Update `deal/tests/ffi/Cargo.toml` to add `[dev-dependencies] serde_json = "1"`. The build.rs is unchanged from Plan 01.

7. Update `deal/build.zig` to add the three new unit-test files to the test step.
  </action>
  <acceptance_criteria>
    - deal/tests/unit/c_abi_invalid_utf8.zig declares `test "c_abi.invalid_utf8"` and tests cases A-E
    - deal/tests/unit/c_abi_source_too_large.zig declares `test "c_abi.source_too_large"`
    - deal/tests/unit/c_abi_no_leaks.zig declares `test "c_abi.no_leaks"`
    - deal/tests/ffi/tests/parse.rs contains all five new `#[test] fn` declarations (ffi_parse_battery_real, ffi_parse_dealx_real, ffi_diagnostics_on_malformed, ffi_ast_json_cache_stable, ffi_invalid_utf8)
    - deal/tests/ffi/tests/gate.rs contains exactly one `#[test] fn gate_all_19`
    - deal/tests/ffi/Cargo.toml contains `serde_json` in `[dev-dependencies]`
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=c_abi.invalid_utf8 2>&1 — exits 0; all 5 cases pass</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=c_abi.source_too_large 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=c_abi.no_leaks 2>&1 — exits 0; std.testing.allocator reports zero leaks across ~70 files</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml ffi_parse_battery_real 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml ffi_parse_dealx_real 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml ffi_diagnostics_on_malformed 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml ffi_ast_json_cache_stable 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml ffi_invalid_utf8 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml gate_all_19 2>&1 — exits 0; the Phase 1 gate test passes</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test && cargo test --manifest-path tests/ffi/Cargo.toml 2>&1 | tail -20</automated>
  </verify>
  <done>
    Three C ABI unit tests (c_abi.invalid_utf8, c_abi.source_too_large, c_abi.no_leaks) each green. Six Rust FFI tests covering parse-deal, parse-dealx, diagnostics-on-malformed, ast-json-cache-stable, invalid-utf8-via-FFI, and the Phase 1 gate test gate_all_19 all green. The single test that defines Phase 1's exit criterion (gate_all_19) parses every one of the 19 showcase files via the Rust → C → Zig boundary and reads back valid AST JSON. Zero leaks reported by std.testing.allocator across ~70 files (19 showcase + ≥50 malformed).
  </done>
</task>

<task type="auto">
  <name>Task 6.3: Full-suite green + phase-gate verification + SUMMARY documenting Phase 1 closure</name>
  <read_first>
    - deal/.planning/phases/01-zig-compiler-core/01-VALIDATION.md (Validation Sign-Off checklist at the bottom; Sampling Rate "Before /gsd:verify-work: Full suite green")
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Validation Architecture" §"Phase gate command" (line 936)
    - deal/.planning/ROADMAP.md §Phase 1 Success Criteria (5 criteria)
    - All prior SUMMARY files: 01-01-SUMMARY.md through 01-05-SUMMARY.md
  </read_first>
  <files>
    deal/build.zig,
    deal/.planning/phases/01-zig-compiler-core/01-VALIDATION.md
  </files>
  <action>
1. Verify the full Phase 1 gate-command pipeline runs cleanly end-to-end:

   - `cd deal && zig build` — produces zig-out/lib/libdeal.a + zig-out/include/deal.h
   - `cd deal && zig build test` — full Zig test suite green: lexer tests (5) + parser_deal tests (3) + parser_dealx tests (5) + recovery tests (5) + c_abi tests (3) + diag.json_roundtrip = 22 tests minimum
   - `cd deal && cargo test --manifest-path tests/ffi/Cargo.toml` — 8 FFI tests pass (2 from Plan 01 + 5 new in Plan 06 + 1 gate test)
   - `cd deal && zig build gen-malformed` (re-runnable, idempotent for the seed) — corpus stays at ≥ 50 files

2. Add a `phase-1-gate` umbrella step to `deal/build.zig` for one-shot verification:

   ```
   const gate_step = b.step("phase-1-gate", "Run the full Phase 1 exit-gate verification");
   gate_step.dependOn(test_step);
   // Note: cargo invocation cannot be expressed inside build.zig directly without losing
   // exit-code semantics. Document in PLAN-06-SUMMARY.md that the full gate is
   //   `zig build phase-1-gate && cargo test --manifest-path tests/ffi/Cargo.toml`
   ```

3. Open `deal/.planning/phases/01-zig-compiler-core/01-VALIDATION.md` and populate the Per-Task Verification Map with one row per task across Plans 01-06. Required columns per row: Task ID (e.g. `1.1`, `1.2`, ..., `6.3`), Plan (e.g. `01-01-foundation-PLAN`), Wave (1-6), Requirement (REQ-phase-1-foundation through REQ-phase-1-gate), Threat Ref (e.g. T-01-01), Secure Behavior (brief), Test Type (unit/snapshot/integration/coverage/scaffold), Automated Command (the `zig build test -Dtest-filter=...` or `cargo test ...` invocation), File Exists (✅ / ❌), Status (⬜ pending until execute completes — set to ⬜ for now; gsd-plan-checker updates to ✅ during /gsd:execute-phase).

   At the bottom of VALIDATION.md, in `## Validation Sign-Off`:
   - Check all five bullets
   - Set `nyquist_compliant: true` in the frontmatter
   - Set `wave_0_complete: true` (since Plan 01 = Wave 1 = the Wave 0 scaffolding tier per CONTEXT terminology)
   - Set `status: ready-for-execute`
   - Add a note: "Approval: ready for /gsd:execute-phase 1"

4. Confirm acceptance of all 5 Phase 1 success criteria from ROADMAP:
   - Criterion 1 (lexer zero-UNKNOWN on 19 files): covered by Plan 02 `lexer.snapshot` test
   - Criterion 2 (15 .deal AST snapshots stable): covered by Plan 03 `parser_deal.snapshot` test
   - Criterion 3 (4 .dealx AST snapshots with tag balancing + connect-via): covered by Plan 04 `parser_dealx.snapshot` + `parser_dealx.tag_balance` + `parser_dealx.connect_via`
   - Criterion 4 (50+ malformed inputs, no panic, structured diagnostics): covered by Plan 05 `recovery.corpus` + `diag.json_roundtrip` + Plan 06 `c_abi.no_leaks`
   - Criterion 5 (Rust FFI loads libdeal.a + parses + reads AST through deal.h): covered by Plan 06 `gate_all_19` Rust test

5. Record gate execution timing as a baseline for Phase 2:
   - Run `time zig build test` and record the seconds
   - Run `time cargo test --manifest-path tests/ffi/Cargo.toml` and record the seconds
   - Both numbers go in SUMMARY for the Phase 2 planner to baseline against
  </action>
  <acceptance_criteria>
    - deal/build.zig contains `b.step("phase-1-gate", ...)` (grep)
    - deal/.planning/phases/01-zig-compiler-core/01-VALIDATION.md has `nyquist_compliant: true` and `status: ready-for-execute` in the frontmatter (grep)
    - deal/.planning/phases/01-zig-compiler-core/01-VALIDATION.md Per-Task Verification Map has ≥ 14 rows populated (one per task across the 6 plans; some plans have 2-3 tasks)
    - deal/.planning/phases/01-zig-compiler-core/01-VALIDATION.md Validation Sign-Off has all five checkboxes checked
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build phase-1-gate 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test 2>&1 — full Zig suite green</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml 2>&1 — full Rust suite green; gate_all_19 in the output</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && ls tests/snapshots/ast/*.json | wc -l — produces 19 (15 .deal + 4 .dealx)</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && ls tests/malformed/*.deal tests/malformed/*.dealx 2>/dev/null | wc -l — produces ≥ 50</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build phase-1-gate && cargo test --manifest-path tests/ffi/Cargo.toml gate_all_19 2>&1 | tail -5</automated>
  </verify>
  <done>
    Phase 1 gate verified end-to-end: `zig build phase-1-gate && cargo test --manifest-path tests/ffi/Cargo.toml` exits 0 with all five ROADMAP success criteria observable. VALIDATION.md is finalized with the Per-Task Verification Map populated and Sign-Off bullets checked. Baseline timing recorded for Phase 2. The Zig compiler core is consumable from Rust; Phase 2 (DEAL IR v0 + SysML v2 codegen) can begin.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Rust caller → C ABI | All six C ABI exports are the perimeter. Null pointers, invalid UTF-8, oversized lengths, and use-after-free are the four attack vectors. |
| Handle arena → process memory | Per-handle ArenaAllocator backed by page_allocator owns ALL allocations. deal_free releases the entire arena. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-06-01 | Tampering | deal_parse with invalid UTF-8 source | mitigate | V5 ASVS — `std.unicode.utf8ValidateSlice` at the top of deal_parse. On failure: emit E0001, return handle with no parse attempt. Tested by c_abi.invalid_utf8 (5 cases) + ffi_invalid_utf8. |
| T-06-02 | DoS / Memory Corruption | source_len > u32 max overflowing Span fields | mitigate | V12 ASVS — explicit `if (source_len > std.math.maxInt(u32))` check BEFORE buffer access. Emit E0004, return handle with no parse. Tested by c_abi.source_too_large. RESEARCH line 1011. |
| T-06-03 | Use-after-free | Rust caller dereferences buffer after deal_free | accept | C ABI cannot prevent caller misuse. Documented in deal.h doxygen with explicit "valid until deal_free" notes on the two buffer accessors. Rust wrapper (Phase 2 CLI) wraps DealHandle in `Drop` to enforce automatically. |
| T-06-04 | Null pointer dereference | Caller passes null to any export | mitigate | Every export uses `handle_ptr orelse return <safe_default>;` at entry. deal_free(null) is a no-op (C convention). |
| T-06-05 | Resource Exhaustion / Memory leak | Allocation made during parse that bypasses arena | mitigate | All allocations route through `handle.arena.allocator()`. c_abi.no_leaks test (~70 files = 19 showcase + 50 malformed) using `std.testing.allocator` as the arena's backing allocator confirms zero leaks. RESEARCH line 1025. |
| T-06-06 | Information Disclosure | JSON output includes internal state beyond the documented contract | accept | The D-04 schema is THE contract. The JSON includes only the fields documented in the per-payload `switch` arms in json.zig. No internal pointers, allocator state, or diagnostic-list capacity leaks. |
| T-06-07 | Tampering | Concurrent access to a single DealHandle from multiple threads | accept | D-13 explicitly defines per-handle thread affinity. deal.h doxygen documents this. Violation is a caller bug, not a library bug. The library has no locks → no contention; different handles in different threads are independent. |
| T-06-SC | Tampering | Final integration uses `serde_json` Rust crate | mitigate | serde_json is in the Rust ecosystem's top-10 most-downloaded crates (1B+ downloads/year per crates.io); upstream maintained by dtolnay. Not [ASSUMED] / [SUS]. Added only to dev-dependencies (not link-affecting). Confirms RESEARCH §Package Legitimacy Audit. |
</threat_model>

<verification>
1. `cd /Users/dunnock/projects/deal-lang/deal && zig build` exits 0 — libdeal.a + include/deal.h produced.
2. `cd /Users/dunnock/projects/deal-lang/deal && zig build test` exits 0 — full Zig suite green (~22 tests).
3. `cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml` exits 0 — full Rust suite green (8 FFI tests).
4. `cd /Users/dunnock/projects/deal-lang/deal && zig build phase-1-gate` exits 0 — umbrella step works.
5. `cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml gate_all_19` exits 0 — the Phase 1 single-name gate test passes.
6. `cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=c_abi.no_leaks` exits 0 — std.testing.allocator reports zero leaks across 19 showcase + ≥50 malformed files.
7. `cd /Users/dunnock/projects/deal-lang/deal && nm -gU zig-out/lib/libdeal.a 2>/dev/null | grep -c '_deal_'` produces ≥ 6 (six C exports).
8. `cd /Users/dunnock/projects/deal-lang/deal && grep -c 'callconv(\.C)' src/*.zig` produces 0 (no uppercase remnant).
</verification>

<success_criteria>
- Phase 1 ROADMAP success criterion #5 met: "A Rust FFI test harness loads `libdeal.a`, parses a showcase file via `deal_parse()`, and reads back the AST through `deal.h`." — gate_all_19 exceeds this by parsing all 19 showcase files.
- REQ-phase-1-5-c-abi acceptance met: `libdeal.a`; `deal.h` C header; Rust FFI test harness parses a showcase file and reads back the AST.
- REQ-phase-1-gate acceptance met: "All 19 showcase files tokenize and parse through the Zig core; AST JSON snapshots stable; error recovery handles at least 50 malformed-input cases without panic; C ABI boundary proven with Rust harness. No external-facing release."
- D-10 (always-non-null handle except alloc fail) honored end-to-end.
- D-11 (length-prefixed UTF-8 buffer accessors, owned by handle, freed by deal_free, lazy + cached) honored and tested by ffi_ast_json_cache_stable.
- D-12 (input is source bytes + filename hint; file I/O in Rust) honored — no file paths cross the C ABI; Rust loads files and passes bytes.
- D-13 (per-handle thread affinity, no global state) honored — no statics, no locks; documented in deal.h.
- D-14, D-15, D-16, D-17 (diagnostic model, span shape, code namespace, three-tier sync) all observable through `deal_diagnostics_json` + the malformed corpus result.
- All five ROADMAP Phase 1 success criteria are now demonstrably true via automated commands.
</success_criteria>

<output>
Create `.planning/phases/01-zig-compiler-core/01-06-SUMMARY.md` when done, recording:
- The exact 0.16.0 UTF-8 validation function name actually used (resolves RESEARCH assumption A1: was it `std.unicode.utf8ValidateSlice` or a different name?)
- Whether the `deal_parse_internal` test-only entry point survived into the final lib.zig or was refactored away.
- Final symbol-table snapshot of libdeal.a (output of `nm -gU` or `nm -D --defined-only`) showing exactly the six expected exports and no accidental leakage.
- Baseline timing for `zig build test` and `cargo test ...` (for Phase 2 planner to compare against when adding semantic analyzer + IR codegen).
- Total final test count across Zig + Rust (the Phase 1 test budget).
- Total final malformed-corpus size and total diagnostic-emission count when the corpus is run through deal_parse.
- Final size of libdeal.a (release mode if applicable) — for the Phase 4 docs-site to surface as a compiler-binary stat.
- Any remaining RESEARCH assumptions (A1-A10) still unresolved after Phase 1 — Phase 2 should pick up on those.
- A one-sentence Phase 1 conclusion suitable to paste into ROADMAP.md / STATE.md when Phase 1 is marked complete.
</output>
</content>
</invoke>