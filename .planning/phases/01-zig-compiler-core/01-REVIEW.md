---
phase: 01-zig-compiler-core
reviewed: 2026-05-20T17:15:27Z
depth: standard
files_reviewed: 38
files_reviewed_list:
  - src/ast.zig
  - src/diagnostics.zig
  - src/expr.zig
  - src/json.zig
  - src/keywords.zig
  - src/lexer.zig
  - src/lib.zig
  - src/parser.zig
  - src/parser_deal.zig
  - src/parser_dealx.zig
  - src/source_map.zig
  - include/deal.h
  - tools/gen-malformed.zig
  - build.zig
  - tests/unit/_all.zig
  - tests/unit/c_abi_invalid_utf8.zig
  - tests/unit/c_abi_no_leaks.zig
  - tests/unit/c_abi_source_too_large.zig
  - tests/unit/diag_json_roundtrip.zig
  - tests/unit/expr_precedence.zig
  - tests/unit/lexer_comments.zig
  - tests/unit/lexer_keywords.zig
  - tests/unit/lexer_mode_flip.zig
  - tests/unit/lexer_snapshot.zig
  - tests/unit/lexer_templates.zig
  - tests/unit/parser_deal_coverage.zig
  - tests/unit/parser_deal_snapshot.zig
  - tests/unit/parser_dealx_connect_via.zig
  - tests/unit/parser_dealx_coverage.zig
  - tests/unit/parser_dealx_smoke.zig
  - tests/unit/parser_dealx_snapshot.zig
  - tests/unit/parser_dealx_tag_balance.zig
  - tests/unit/recovery_corpus.zig
  - tests/unit/recovery_definition.zig
  - tests/unit/recovery_dealx_tag.zig
  - tests/unit/recovery_statement.zig
  - tests/ffi/Cargo.toml
  - tests/ffi/tests/gate.rs
  - tests/ffi/tests/parse.rs
findings:
  critical: 2
  warning: 9
  info: 7
  total: 18
status: issues_found
---

# Phase 1: Code Review Report

**Reviewed:** 2026-05-20T17:15:27Z
**Depth:** standard
**Files Reviewed:** 38
**Status:** issues_found

## Summary

Phase 1 ships a five-pass Zig compiler core (lexer, deal parser, dealx parser,
error recovery, C ABI) with extensive test coverage (~70 files exercised by the
no-leaks gate, 19 showcase snapshots, ≥50-file malformed corpus, Rust FFI
gate). The implementation generally adheres to the locked decisions (D-01
through D-18), uses Zig 0.16.0 idioms correctly (`ArrayList = .empty`,
allocator-explicit calls, named module imports), and applies the documented
security controls (UTF-8 validation, u32 source-length bound, comptime format
strings, depth limits on template / tag / expression recursion).

That said, the adversarial review surfaced 18 findings: 2 critical, 9 warning,
7 info. The two critical findings are robustness gaps at the C ABI boundary
(missing NULL-pointer guards on `source_ptr` / `filename_ptr` despite the
length parameter being trusted; and a silent diagnostic loss when integer
literals in multiplicity productions overflow u32). The warnings cover lost
parser state on incomplete member declarations, dead code in `syncToTag`'s
no-op defer block, and several fragile assumptions about cursor/source
alignment that could surface as recovery bugs once the malformed corpus
mutations land in untested combinations.

## Critical Issues

### CR-01: `deal_parse` / `deal_parse_internal` do not NULL-check `source_ptr` or `filename_ptr` before constructing slices

**File:** `src/lib.zig:83, 105`

**Issue:** The C ABI entry point accepts `source_ptr: [*]const u8`,
`source_len: usize`, `filename_ptr: [*]const u8`, `filename_len: usize`. With
`source_len > 0` (or `filename_len > 0`) and `*_ptr == NULL`, the slice
constructor `source_ptr[0..source_len]` produces an invalid slice; `alloc.dupe`
then reads from a NULL base pointer and segfaults. The Rust bindings at
`tests/ffi/src/lib.rs` already declare these arguments as `*const u8` (which
can be NULL from Rust). The hand-written `include/deal.h` API documentation
makes no NULL-safety promise either way, but a real C caller can easily forget
to validate input before calling. The 36 already-documented threats do not
cover "caller passes NULL with non-zero length" — this is a missing input
validation, ASVS V5.

Note that `deal_free` (line 230-235) DOES NULL-check via `orelse return`. The
asymmetry is the bug: input validation must be consistent across all six
exports.

**Fix:**
```zig
pub export fn deal_parse(
    source_ptr: [*]const u8,
    source_len: usize,
    filename_ptr: [*]const u8,
    filename_len: usize,
) callconv(.c) ?*anyopaque {
    // Guard against NULL ptr with non-zero length (caller misuse).
    // Zero-length is OK regardless of ptr (we never dereference).
    if (source_len > 0 and @intFromPtr(source_ptr) == 0) return null;
    if (filename_len > 0 and @intFromPtr(filename_ptr) == 0) return null;
    // ... rest unchanged
}
```

Mirror the same guard in `deal_parse_internal` (line 151+) for consistency.
Document the NULL contract in `include/deal.h` `@param` blocks.

### CR-02: `parseMultiplicity` silently swallows integer parse failures, producing wrong AST

**File:** `src/parser_deal.zig:1429, 1440`

**Issue:** When parsing a multiplicity like `[5000000000]` (where the literal
exceeds `maxInt(u32) = 4_294_967_295`), the parser executes:

```zig
lower = std.fmt.parseInt(u32, n_str, 10) catch 0;
// ...
upper = std.fmt.parseInt(u32, u_str, 10) catch null;
```

The `catch 0` / `catch null` silently coerce ANY parse failure (overflow,
non-digit byte injected mid-token, allocator error) into a default value with
NO diagnostic emitted. Downstream consumers (LSP, code-gen) see a
multiplicity of `[0..0]` for `[5000000000]` — wildly different semantics — and
have no way to recognize that an error occurred. This is a data-loss / silent
corruption bug, not just a quality issue:

1. The malformed corpus (`recovery.corpus`) will not flag this because the
   parser returns a valid-looking AST with zero diagnostics.
2. A user writes `[100..10]` (typo: meant `[1..10]`) — the parser produces an
   AST with `lower=100, upper=10` and no warning.
3. The C ABI `deal_has_errors` returns false, so the Rust FFI cannot detect
   the corruption.

E0100 (or a new E0121 "invalid multiplicity bound") should fire on parse
failure, mirroring how `expect` already emits diagnostics on token mismatch.

**Fix:**
```zig
lower = std.fmt.parseInt(u32, n_str, 10) catch blk: {
    try p.diags.append(p.arena, .{
        .code = "E0100",
        .severity = .err,
        .message = "multiplicity lower bound not a valid u32",
        .span = n_tok.span,
    });
    break :blk 0;
};
// ... same pattern for the upper bound at line 1440
```

## Warnings

### WR-01: `parseMemberDeclaration` drops consumed modifier tokens on no-match return

**File:** `src/parser_deal.zig:1059-1117`

**Issue:** The function consumes any leading doc-comment, then collects
modifier keywords into `modifiers` (each via `_ = p.advance()` at line 1077),
then collects direction (lines 1085-1091), then dispatches on the element
keyword. If the next token is NOT a known usage keyword, the `else` arm at
line 1114-1117 returns `doc_node` (if any) or `null`. The collected
`mods_slice` is silently discarded:

```zig
const mods_slice = try modifiers.toOwnedSlice(p.arena);
// ...
else => {
    if (doc_node != null) return doc_node; // mods_slice lost!
    return null;                            // mods_slice lost!
}
```

Because tokens were ADVANCED past (not just peeked), the modifier keywords are
gone from the token stream. The caller (`parseDefinitionBody`) re-enters its
loop and re-peeks at the next token, but the modifiers are now "vanished"
from the parse. Resulting AST silently omits the modifiers a user wrote.

Arena allocation makes this non-fatal (memory is reclaimed at `deal_free`),
but the semantic loss matters: `abstract derived foo` parsed as a member
declaration with non-member token after `derived` would lose BOTH modifiers.

**Fix:** Restructure to use lookahead-only (peek2/peek3) until we're committed
to producing a member, OR emit a diagnostic when modifiers were consumed but
no member follows. Recommend:
```zig
else => {
    if (mods_slice.len > 0) {
        try p.diags.append(p.arena, .{
            .code = "E0120",
            .severity = .err,
            .message = "modifiers must precede an element keyword",
            .span = p.peek().span,
        });
    }
    if (doc_node != null) return doc_node;
    return null;
},
```

### WR-02: `syncToTag` declares a defer block that is dead code

**File:** `src/parser_dealx.zig:87-94`

**Issue:** The function captures `prev_mode` then sets up a `defer` block:

```zig
const prev_mode = p.current_mode;
p.current_mode = .dealx_outer;
// ...
defer {
    // [long comment explaining we DON'T restore]
    _ = prev_mode;
}
```

The body of `defer` is `_ = prev_mode;` which discards the captured value at
defer-fire time, which is a no-op. The `prev_mode` local could be dropped
entirely. As written, the code SUGGESTS some restoration behavior to readers
but performs none. This is misleading; either:

1. Actually restore `prev_mode` on exit (if that's the intended semantics), or
2. Delete `prev_mode` and the `defer` block entirely.

The function's only call site is in `parseDealxFile` where the mode is
already `.dealx_outer`, so option (2) is correct. Keeping dead code that
references a "would-be-restored" variable is a maintenance hazard.

**Fix:**
```zig
pub fn syncToTag(p: *Parser) void {
    p.current_mode = .dealx_outer;
    // Invalidate the lookahead cache so the next peek() re-lexes
    // under .dealx_outer.
    if (p.peeked) |tok| {
        p.lex.pos = tok.span.start;
    }
    p.peeked = null;
    // ... rest unchanged (no defer block)
}
```

### WR-03: `parseHeaderField` rescans source bytes for `:` after `expect(.colon)` already consumed it

**File:** `src/parser_deal.zig:463-474`, `src/parser_dealx.zig:283-285`

**Issue:** After `_ = try p.expect(.colon);` consumes the colon token,
`parseHeaderField` rescans `p.source` from `key_tok.span.end` forward looking
for the colon byte:

```zig
const colon_end: u32 = blk: {
    var i = key_tok.span.end;
    while (i < p.source.len and p.source[i] != ':') i += 1;
    if (i >= p.source.len) break :blk @intCast(p.source.len);
    break :blk i + 1;
};
```

This is fragile because:
1. It duplicates work the lexer already did — the colon position is already
   known via the consumed colon token's span, but the code throws that away.
2. If any header field's key text or surrounding whitespace ever contains a
   colon-like byte (unlikely for FS-2 keys, but the grammar may expand), the
   scan could misalign.
3. The two implementations (`parser_deal.zig` and `parser_dealx.zig`) duplicate
   the logic, so divergence is possible — and indeed they DO diverge slightly
   (the `.zig` variant uses a label-block; the `_dealx.zig` variant uses an
   inline assignment).

**Fix:** Capture the colon span from the `expect` return value:
```zig
const colon_tok = try p.expect(.colon);
// Use colon_tok.span.end directly; no rescan needed.
var i = colon_tok.span.end;
// ... rest of the value-text scan from `i`.
```

This is both safer and faster.

### WR-04: `source_map.lineCol` overflows `col` on the OOM-fallback path

**File:** `src/source_map.zig:44`

**Issue:** When `buildLineStarts` returns an allocator error, the fallback is:

```zig
return .{ .line = 1, .col = offset + 1 };
```

If `offset == std.math.maxInt(u32)`, `offset + 1` overflows. The pre-check in
`lib.zig` bounds `source.len ≤ maxInt(u32)`, but `offset` here is a u32
argument supplied by the caller — there's no per-call check. A diagnostic
with `span.end = source.len = maxInt(u32)` (which the E0001 invalid-UTF-8
emission at `lib.zig:120` can theoretically produce if `source.len ==
maxInt(u32)`) would feed `offset = maxInt(u32)` into `lineCol` on the
fallback path and trigger Zig's runtime overflow check in debug builds.

**Fix:** Clamp before incrementing:
```zig
const safe_col: u32 = if (offset == std.math.maxInt(u32))
    std.math.maxInt(u32)
else
    offset + 1;
return .{ .line = 1, .col = safe_col };
```

The function is on a degraded-OOM path so producing `col = maxInt(u32)` is
acceptable degradation; the goal is to avoid the panic.

### WR-05: `peek` in lexer copies the entire 256-byte template_stack on every call

**File:** `src/lexer.zig:386-395`

**Issue:** `peek` saves and restores `template_stack: [64]u32 = 256 bytes` on
every invocation:

```zig
const saved_stack = self.template_stack;  // copy 256 bytes
const tok = self.next(mode);
self.pos = saved_pos;
self.template_depth = saved_depth;
self.template_stack = saved_stack;  // copy 256 bytes back
```

The `Parser` already wraps `peek` with its own one-token cache (line 60-65 of
parser_deal.zig), so the lexer-level `peek` is called only when the parser
cache is empty. Still, every `peek2` call goes through this path TWICE (once
for the cached first token, once for the second). On showcase files of
~10k tokens, this is bounded but on adversarial input it amplifies cost.

This is performance-adjacent but borderline correctness: copying a 256-byte
array on every parser lookahead is wasteful and the `template_stack` is only
mutated by the template-literal scanner. A simpler approach: only save/restore
the stack entries that the inner `next` call could have touched (template
operations push/pop at most one frame).

**Fix:** Save only `template_depth` and the active stack entry:
```zig
const saved_pos = self.pos;
const saved_depth = self.template_depth;
const saved_top = if (self.template_depth > 0)
    self.template_stack[self.template_depth - 1]
else
    @as(u32, 0);
const tok = self.next(mode);
self.pos = saved_pos;
// Only restore if a template frame may have changed.
if (saved_depth != self.template_depth or
    (saved_depth > 0 and self.template_stack[saved_depth - 1] != saved_top))
{
    self.template_depth = saved_depth;
    if (saved_depth > 0) self.template_stack[saved_depth - 1] = saved_top;
}
return tok;
```

This is the kind of restoration that v1 explicitly puts out-of-scope (perf),
but flagging because it touches correctness reasoning around state restoration.

### WR-06: `parseExportDecl` accepts arbitrary keywords as module names without diagnostic when not E0100-mismatch

**File:** `src/parser_deal.zig:614-625`

**Issue:** The module-name read is:
```zig
const mod_tok = p.advance();
if (mod_tok.tag != .ident and !isKeyword(mod_tok.tag)) {
    try p.diags.append(p.arena, .{
        .code = "E0100",
        .severity = .err,
        ...
    });
}
const mod_name = p.tokenText(mod_tok.span);
```

If `mod_tok.tag` is something like `.semicolon`, `.r_brace`, `.eof`, we emit
E0100 BUT STILL use `p.tokenText(mod_tok.span)` — assigning e.g. ";" or "}"
as the module name. This leaks an invalid name into the AST, which downstream
semantic analysis will trip over.

For the showcase corpus this is dormant (well-formed input never triggers
it), but the malformed corpus mutations CAN produce `export ;` patterns and
the parser will produce a stale module name.

**Fix:** On diagnostic, sync and return early or use a sentinel:
```zig
if (mod_tok.tag != .ident and !isKeyword(mod_tok.tag)) {
    try p.diags.append(p.arena, .{ ... });
    syncToDefinition(p);
    // Return a stub export_decl rather than continuing with junk name.
    return p.makeNode(.export_decl, mod_tok.span, .{
        .export_decl = .{ .module = "", .items = &.{} },
    });
}
```

### WR-07: `lib.zig` arena lifetime documentation does not survive allocator failure on filename dupe vs source dupe

**File:** `src/lib.zig:84-87, 105-109`

**Issue:** On OOM during `alloc.dupe(u8, filename_slice)` (line 84) we run
`handle.arena.deinit()` and return null. But the handle itself was allocated
INSIDE the arena. After `deinit`, `handle` is freed. Returning null is
correct.

On OOM during the SOURCE dupe (line 106) — the same flow runs:
```zig
handle.source = alloc.dupe(u8, source_slice) catch {
    handle.arena.deinit();
    return null;
};
```

But at this point we've ALREADY written `handle.filename` (line 84
succeeded). The arena holds the duped filename. `handle.arena.deinit()` frees
it along with the handle. No leak.

HOWEVER: the V12 "source too large" path at line 93-102 is BEFORE the source
dupe. It correctly returns the handle without setting `handle.source`. But
the test at `c_abi_source_too_large.zig:64` then calls `deal_ast_json` which
calls `json.emitAst(..., h.ast_root, h.mode, h.filename)`. `ast_root` is null
(parser not invoked). `h.filename` is the duped filename — fine. But the
function emits `{"root":null}` correctly.

This is not a bug per se, but the COMMENT on lines 56-59 says "Security
controls applied before any lexer/parser invocation: ... source_len >
maxInt(u32) → emit E0004". The check actually fires AFTER `handle.filename`
allocation — both can fail. Document the ordering explicitly.

**Fix:** Reorder so V12 check happens BEFORE any allocation that can fail:
```zig
// Step 1: V12 bound check FIRST so we cannot OOM on dupe before checking.
if (source_len > std.math.maxInt(u32)) {
    // ... emit E0004 with whatever filename/handle we can construct
}
// Step 2: allocate arena + handle ...
```

Alternatively, document that filename dupe + V12 check are independent and
both report through the handle. The current code is correct but the comment
overpromises.

### WR-08: `expr.parsePrimary` `.l_paren` branch builds a `span` and then discards it via `_ = span`

**File:** `src/expr.zig:242-253`

**Issue:**
```zig
.l_paren => {
    const inner = try parseExpression(parser, 0);
    const close = try parser.expect(.r_paren);
    const span = ast.Span{ .start = tok.span.start, .end = close.span.end };
    _ = span;
    return inner;
},
```

The `span` is computed then immediately discarded. The comment explains the
design choice ("just return inner unchanged to avoid a wrapper node"), but
the side effect is that diagnostic spans for parenthesized expressions WILL
point only at the inner expression — NOT at the surrounding parens. For
example, `(1 + 2)` will report a span of `[1, 4]` (the `1 + 2` substring)
instead of `[0, 6]` (the full parenthesized form). This degrades LSP hover
quality and may surprise downstream consumers.

Either commit to the wrapper-free design (delete the dead `span` calc) or
attach the wider span to `inner` before returning:
```zig
.l_paren => {
    const inner = try parseExpression(parser, 0);
    const close = try parser.expect(.r_paren);
    inner.span = .{ .start = tok.span.start, .end = close.span.end };
    return inner;
},
```

The latter is the more useful behavior; the former is at least honest.

### WR-09: `gen-malformed` swallows file-read errors with `continue`, silently shrinking the corpus

**File:** `tools/gen-malformed.zig:80-84`

**Issue:** In the main loop:
```zig
const src_bytes = cwd.readFileAlloc(io, src_path, alloc, .unlimited) catch |err| {
    std.debug.print("gen-malformed: read {s} failed ({s})\n", .{ src_path, @errorName(err) });
    file_idx += 1;
    continue;
};
```

If ANY showcase file fails to read, we print a message and continue. The
TARGET_COUNT is 30 mutated files; if reads fail, the loop iterates more times
to compensate. But `produced` only increments on success, and `file_idx` and
`strategy_idx` may go out of sync with `produced`. The mutation determinism
guarantee (T-05-06 — same seed produces same files) requires that the
showcase corpus is stable; a transient I/O error during corpus generation
would silently produce a different malformed corpus on next run. This breaks
the `recovery.corpus` reproducibility guarantee.

**Fix:** Treat read failures as hard errors:
```zig
const src_bytes = try cwd.readFileAlloc(io, src_path, alloc, .unlimited);
```

Or at minimum, increment `produced` only if subsequent steps succeed and
exit with non-zero status if any reads failed.

## Info

### IN-01: `parser_deal.zig` `parseExportDecl` uses bare `expect(.dot)` but the grammar requires `.l_brace` directly

**File:** `src/parser_deal.zig:627`

**Issue:** After the module name:
```zig
_ = try p.expect(.dot);
_ = try p.expect(.l_brace);
```

The two-token sequence `dot l_brace` matches `export module . { ... }`. This
matches the grammar (`export NAME "." "{" ... "}"`), but the comment doesn't
explain why a dot is required between the module name and the brace. The
showcase files would clarify (e.g. `export vehicle.battery.{ Foo, Bar };`),
but without a unit test exercising export specifically, the grammar is
unverified here.

**Fix:** Add a parser_deal_coverage synthetic fixture for `export_decl` with
the `.{` syntax to lock the grammar.

### IN-02: `lexer.zig` `scanQuotedSlice` does not validate escape body, silently accepts illegal escape sequences

**File:** `src/lexer.zig:524-531`

**Issue:** The escape handling is:
```zig
if (c == '\\' and self.pos + 1 < self.source.len) {
    // We don't validate the escape body in Plan 02
    self.pos += 2;
    continue;
}
```

A string `"foo\zbar"` (illegal escape `\z`) tokenizes as one string_literal
without complaint. Plan comment promises "the parser / semantic pass can
re-validate when it needs the literal value" — but the AST `string_literal`
payload only carries the slice WITH escapes; no consumer (Plan 04+) has been
shown to re-validate. Phase 3 LSP will end up shipping invalid-escape strings
to downstream tools.

**Fix:** Not strictly a Phase 1 bug — the deferred-validation contract is
documented. Add a TODO comment cross-referencing where validation will
happen (Phase 2 semantic analyzer task).

### IN-03: `expr.zig` `parsePrimary` fallback `else` branch wraps any unexpected token as an identifier without emitting a diagnostic

**File:** `src/expr.zig:255-261`

**Issue:**
```zig
else => {
    // Unexpected token — return an identifier node for error recovery.
    const name = parser.source[tok.span.start..tok.span.end];
    return makeNode(allocator, .identifier, tok.span, .{
        .identifier = .{ .name = name },
    });
},
```

No diagnostic is emitted. The caller has no way to learn that an unexpected
token was wrapped. The resulting AST is structurally valid but semantically
nonsense. Consider:
- `attribute x = ;` — the `;` after `=` is treated as an identifier named
  ";", silently.
- The `expect_expression` E0110 code in `Codes.e_expected_expression` exists
  exactly for this case but is unused.

**Fix:** Emit `E0110` for the fallback:
```zig
else => {
    parser.diags.append(parser.arena, .{
        .code = "E0110",
        .severity = .err,
        .message = "expected expression",
        .span = tok.span,
    }) catch {};
    // ... wrap as identifier for recovery
},
```

### IN-04: `parser_dealx.zig` `parseImportDecl` unconditionally advances on the first segment, treating non-IDENT as identifier text

**File:** `src/parser_dealx.zig:342-351`

**Issue:**
```zig
const first_seg = p.advance();  // unconditional advance
if (first_seg.tag != .ident and !isIdentLikeKeyword(first_seg.tag)) {
    try p.diags.append(p.arena, .{ ... });
}
try segments.append(p.arena, p.tokenText(first_seg.span));
```

If the user writes `import ;` (typo), `first_seg = .semicolon`. We emit
E0100 and then `segments.append(p.tokenText(.semicolon.span))` — adding ";"
as a path segment. This produces an `import_decl` with `path = [";"]`. The
downstream semantic analyzer or LSP will see a nonsense import path.

**Fix:** Sync and return on the bad path:
```zig
if (first_seg.tag != .ident and !isIdentLikeKeyword(first_seg.tag)) {
    try p.diags.append(p.arena, .{ ... });
    // Synthesize an empty import to keep AST shape valid.
    syncToDefinition(p);
    return p.makeNode(.import_decl, start.span, .{
        .import_decl = .{ .path = &.{}, .kind = .simple, .items = &.{} },
    });
}
```

### IN-05: Inconsistent OOM-handling pattern: some paths use `catch {}` to swallow OOM, others propagate

**File:** Multiple — e.g. `src/parser_deal.zig:171-176`, `src/parser_dealx.zig:215-220`

**Issue:** Within `pushTag` and `popTag` (parser_deal.zig:169-224), OOM on
`self.diags.append(...)` is swallowed via `catch {}`. This makes sense because
the function returns `void` and there's no other error channel. But other
parser entry points (`parseFile`, `parseAttribute`) use `try ... catch
|err|` blocks to convert OOM to a fatal E0004 diagnostic. The mixed strategy
means OOM during a deeply-nested parse could silently lose diagnostics on
the way up the stack — the top-level `parseDealFile` would emit E0004 but
the inner pushTag/popTag E0303/E0301/E0302 diagnostics were already silently
dropped.

This isn't a correctness defect (the program still completes; the user just
sees fewer diagnostics under memory pressure), but it's an inconsistency
worth standardizing.

**Fix:** Document the policy explicitly: "OOM during diagnostic-append in
void functions is swallowed; OOM in error-returning functions propagates."
Add a short comment block in `Parser` near `pushTag`/`popTag` explaining
this is intentional.

### IN-06: `json.zig` `appendU32` uses `catch unreachable` on `bufPrint`, but `unreachable` is debug-only

**File:** `src/json.zig:1135-1137`

**Issue:**
```zig
var tmp: [10]u8 = undefined;
const slice = std.fmt.bufPrint(&tmp, "{d}", .{value}) catch unreachable;
```

`bufPrint` for a u32 into a `[10]u8` cannot overflow (u32 max = 4294967295 =
10 chars). The `catch unreachable` is mathematically correct. But in
release-fast or release-small builds, `unreachable` is UB — if the assertion
EVER becomes false (e.g. format spec changed to `"{d:0>11}"`), the compiler
optimizes around it instead of trapping. Future-proof: use `catch
@panic("appendU32 buffer too small")` so release builds still fault loudly,
OR use `std.fmt.formatIntBuf` which has a documented bound.

**Fix:**
```zig
var tmp: [10]u8 = undefined;
const slice = std.fmt.bufPrint(&tmp, "{d}", .{value}) catch
    @panic("appendU32: buffer too small (u32 max = 10 chars)");
```

Same pattern at line 1161 in `appendJsonStringEscaped` for the `\u00XX`
fallback (6-byte tmp). The 6-byte bound IS correct for `\u00XX`, but the
same release-mode concern applies.

### IN-07: `parser_deal.zig` `parseHeaderField` line 506-510 token-skip loop uses `i` as u32 cast to `@as(u32, @intCast(i))` but `i` is already u32

**File:** `src/parser_deal.zig:508`

**Issue:**
```zig
if (next.span.start >= @as(u32, @intCast(i))) break;
```

`i` is declared as `var i: u32 = colon_end;` at line 478, so `@intCast(i)` is a
no-op. This compiles but is dead casting noise — likely a leftover from a
refactor where `i` was previously `usize`.

**Fix:**
```zig
if (next.span.start >= i) break;
```

Search the rest of the codebase for similar dead `@intCast` patterns —
`@intCast` on a same-typed value is a code smell that suggests a previous
type was changed without cleaning up the casts.

---

_Reviewed: 2026-05-20T17:15:27Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
