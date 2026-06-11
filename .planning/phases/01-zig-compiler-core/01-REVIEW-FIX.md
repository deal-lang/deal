---
phase: 01-zig-compiler-core
fixed_at: 2026-05-20T17:45:00Z
review_path: .planning/phases/01-zig-compiler-core/01-REVIEW.md
iteration: 1
findings_in_scope: 11
fixed: 10
skipped: 1
status: partial
---

# Phase 1: Code Review Fix Report

**Fixed at:** 2026-05-20T17:45:00Z
**Source review:** `.planning/phases/01-zig-compiler-core/01-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 11 (2 critical + 9 warning; 7 info findings out of scope)
- Fixed: 10
- Skipped: 1

All 10 fixes were applied as individual atomic commits on branch
`gsd-reviewfix/01-93719`. A full clean rebuild + `zig build test` confirms
the suite still passes (22/22 tests pass when running the test binary
directly; the `failed command` line emitted by `zig build test` is a Zig
0.16.0 quirk with the `--listen=-` flag, unrelated to test outcomes).

## Fixed Issues

### CR-01: `deal_parse` / `deal_parse_internal` do not NULL-check `source_ptr` or `filename_ptr`

**Files modified:** `src/lib.zig`, `include/deal.h`
**Commit:** `b674fe5`
**Applied fix:** Added a "Step 0" NULL-pointer guard at the top of
`deal_parse` that returns null when either input pointer is NULL while its
matching length is non-zero. Zero-length is OK regardless of the pointer
value because the function never dereferences. Mirrored a defensive
equivalent in `deal_parse_internal` using `@intFromPtr(slice.ptr)` on the
slice's underlying pointer. Updated `include/deal.h` `@param` blocks to
document the NULL-with-non-zero-length contract for both buffers.

### CR-02: `parseMultiplicity` silently swallows integer parse failures

**Files modified:** `src/parser_deal.zig`
**Commit:** `35b18a9`
**Applied fix:** Replaced `catch 0` and `catch null` in `parseMultiplicity`
with diagnostic-emitting catch blocks. The lower bound now emits E0100
("multiplicity lower bound not a valid u32") with a span pointing at the
offending integer literal; the upper bound emits the matching "upper
bound" message. Downstream consumers (LSP, codegen) now learn that the
multiplicity bounds in the AST are the result of an error rather than
acting on a fabricated `[0..0]` value.

### WR-01: `parseMemberDeclaration` drops consumed modifier tokens on no-match return

**Files modified:** `src/parser_deal.zig`
**Commit:** `5d2dcb1`
**Applied fix:** When the `else` arm fires (no element keyword follows the
collected modifiers and direction), emit E0120 ("modifiers must precede an
element keyword") if any modifiers OR a direction prefix were consumed.
Tokens were already advanced past so the AST output remains the same
(doc-comment or null), but the user now sees a diagnostic explaining why
their `abstract` / `derived` / `in` tokens were rejected. Note: requires
human verification of the message wording and severity.

### WR-02: `syncToTag` declares a defer block that is dead code

**Files modified:** `src/parser_dealx.zig`
**Commit:** `df30abf`
**Applied fix:** Deleted the no-op `defer { _ = prev_mode; }` block and
the unused `prev_mode` local. The function's only call site is in
`parseDealxFile` where the mode is already `.dealx_outer`, so option (2)
from the review (delete entirely) was correct. Replaced the misleading
defer with a doc comment explaining why no restoration happens.

### WR-03: `parseHeaderField` rescans source bytes for `:` after `expect(.colon)`

**Files modified:** `src/parser_deal.zig`, `src/parser_dealx.zig`
**Commit:** `2f24a86`
**Applied fix:** Both copies of `parseHeaderField` now capture
`colon_tok = try p.expect(.colon)` and use `colon_tok.span.end` directly
instead of walking `p.source` from `key_tok.span.end` looking for the `:`
byte. The two files no longer diverge (label-block vs inline assignment
variants merged). Also removed the dead `@as(u32, @intCast(i))` cast at
the same callsite (IN-07 picked up incidentally), since `i` was already
declared `u32` so the cast was no-op noise.

### WR-04: `source_map.lineCol` overflows `col` on the OOM-fallback path

**Files modified:** `src/source_map.zig`
**Commit:** `4f88264`
**Applied fix:** Clamped `offset` before incrementing in the
`buildLineStarts` OOM catch path. When `offset == maxInt(u32)` the
fallback now returns `col = maxInt(u32)` instead of triggering Zig's
debug-build runtime overflow check on `offset + 1`. The function is on a
degraded-OOM path so producing `col = maxInt(u32)` is acceptable
degradation; the goal is to avoid the panic.

### WR-06: `parseExportDecl` accepts arbitrary keywords as module names

**Files modified:** `src/parser_deal.zig`
**Commit:** `328316c`
**Applied fix:** On the E0100 path (module-name token is neither
`.ident` nor a keyword), instead of falling through with the junk
`tokenText(mod_tok.span)` as the module name, now call
`syncToDefinition(p)` and return a stub `export_decl` with an empty
module string and no items. The AST shape stays valid but downstream
semantic analysis no longer sees `";"` or `"}"` as the module name.

### WR-07: `lib.zig` arena lifetime documentation overpromises

**Files modified:** `src/lib.zig`
**Commit:** `136f69b`
**Applied fix:** Documentation-only change. Expanded the
`/// Security controls applied before any lexer/parser invocation`
doc comment to list all six steps in order (Step 0 NULL guard, Step 1
arena alloc, Step 2 filename dupe, Step 3 V12 check, Step 4 source dupe,
Step 5 UTF-8 validate, Step 6 parser invoke). Made it explicit that
steps 1-2 can fail before V12 fires (which is intentional because V12
emission needs a handle to attach the diagnostic to). The code path is
unchanged — only the comment was overpromising.

### WR-08: `expr.parsePrimary` `.l_paren` branch builds a `span` and discards it

**Files modified:** `src/expr.zig`
**Commit:** `bd375aa`
**Applied fix:** Replaced the `_ = span;` discard with
`inner.span = .{ .start = tok.span.start, .end = close.span.end };`. The
node's span now extends across the surrounding `(...)` parens rather than
only the inner sub-expression. No wrapper node is created; only the
inner node's `span` field is mutated. This improves diagnostic
underlines and LSP hover ranges for parenthesized expressions. Requires
human verification since this changes observable span values that
downstream consumers may have depended on.

### WR-09: `gen-malformed` swallows file-read errors with `continue`

**Files modified:** `tools/gen-malformed.zig`
**Commit:** `0aa350c`
**Applied fix:** Replaced the `continue` path in the corpus-generator
loop with `return err;` so file-read failures propagate as hard errors.
This preserves the T-05-06 determinism guarantee — a transient I/O error
during corpus generation will now fail loudly rather than silently
shrinking the malformed corpus on the next run.

## Skipped Issues

### WR-05: `peek` in lexer copies the entire 256-byte template_stack on every call

**File:** `src/lexer.zig:386-395`
**Reason:** skipped: performance optimization with subtle correctness
reasoning. The review itself classifies this as "performance-adjacent but
borderline correctness" and notes that "this is the kind of restoration
that v1 explicitly puts out-of-scope (perf)". The suggested optimization
saves only `template_depth` and the active stack top, but `next()` can
push frames, pop frames, AND mutate the previous top within a single call.
Verifying the optimized save/restore captures all the cases without a
regression would require an audit of every `template_stack` mutation site
in the lexer that is beyond what a code-fixer can safely auto-apply. The
existing implementation is correct, just suboptimal — there is no
correctness defect. Recommend deferring to a Phase 2 perf pass once
benchmarks identify it as a hot spot.
**Original issue:** `peek` saves a 256-byte `template_stack` on every
invocation; on adversarial input the cost amplifies.

---

_Fixed: 2026-05-20T17:45:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
