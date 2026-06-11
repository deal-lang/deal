---
phase: 01-zig-compiler-core
plan: 05
type: execute
wave: 5
depends_on:
  - 01-04-parser-dealx
files_modified:
  - deal/src/diagnostics.zig
  - deal/src/parser_deal.zig
  - deal/src/parser_dealx.zig
  - deal/src/parser.zig
  - deal/src/expr.zig
  - deal/src/json.zig
  - deal/tools/gen-malformed.zig
  - deal/tests/malformed/.gitkeep
  - deal/tests/unit/recovery_statement.zig
  - deal/tests/unit/recovery_definition.zig
  - deal/tests/unit/recovery_dealx_tag.zig
  - deal/tests/unit/recovery_corpus.zig
  - deal/tests/unit/diag_json_roundtrip.zig
  - deal/build.zig
autonomous: true
requirements:
  - REQ-phase-1-4-error-recovery

must_haves:
  decisions_implemented:
    - "D-14: Rich `Diagnostic` struct — `DiagnosticCollector` wraps the arena-owned diagnostic list; `emit`/`emitWithSecondary`/`emitFmt` (comptime fmt only — T-DiagInjection mitigation); every field (code, severity, message, span, secondary_spans, fix_it, notes) round-trips through `deal_diagnostics_json` per `diag.json_roundtrip` test"
    - "D-15: `Span` used in every diagnostic — primary `span: ast.Span` + `secondary_spans: []SpanLabel` + optional `fix_it.replace_span: ast.Span`; byte offsets only"
    - "D-16: Letter-prefixed codes emitted — `diagnostics.Codes` namespace declares every code as `pub const` string (E0001..E0005 lexer, E0100..E0120 parser-deal, E0301..E0304 parser-dealx, E0400..E0403 recovery, W0500 warn, H0600 hint); ranges respect D-16 boundaries"
    - "D-17: Three-tier sync bodies — `syncToStatement` (FOLLOW: `;`, `,`, `}`, `)`, `]`), `syncToDefinition` (FOLLOW: 14 definition keywords + `}` + `eof`), `syncToTag` (FOLLOW: `[<`, `[</`, `kw_package`, `kw_import`, `kw_export`, `eof`); each emits one E0400 covering the dropped span; verified by `recovery.statement`/`recovery.definition`/`recovery.dealx_tag` tests"
  truths:
    - "The parser survives 50+ malformed inputs in deal/tests/malformed/ without panicking; each input produces ≥1 structured Diagnostic with code, severity, message, span"
    - "Statement-level recovery: a missing semicolon resumes at the next statement boundary (next `;`, `,`, or matching close brace) — at most one extra cascading diagnostic"
    - "Definition-level recovery: a malformed definition body skips to the next definition keyword (`part`, `port`, `requirement`, etc.) — subsequent definitions still parse correctly"
    - "Composition-level recovery (.dealx): a malformed tag skips to the next `[<` or `[</` — subsequent tags still parse"
    - "Every Diagnostic field (code, severity, message, span [start, end], secondary_spans, fix_it, notes) round-trips through deal_diagnostics_json with no field loss"
    - "Diagnostics use the D-16 error-code namespace: E0001-E0099 lexer, E0100-E0299 parser-deal, E0300-E0399 parser-dealx, E0400-E0499 recovery/structural, W0500-W0599 warnings, H0600-H0699 hints"
    - "Recursion depth in parseExpression / parseDefinition is bounded at 256; exceeding the bound emits E0403 (nesting too deep) without stack overflow"
  artifacts:
    - path: "deal/src/diagnostics.zig"
      provides: "Full Diagnostic struct + DiagnosticCollector helpers; emit(code, severity, message, span, secondary_spans, fix_it, notes) appends to arena-owned list; error-code documentation"
      contains: "DiagnosticCollector"
    - path: "deal/src/parser_deal.zig"
      provides: "syncToStatement, syncToDefinition helpers; every grammar production calls them on error; expect() now emits + syncs instead of returning the wrong token"
      contains: "syncToStatement"
    - path: "deal/src/parser_dealx.zig"
      provides: "syncToTag helper; the parser_dealx productions call it on error; tag-balance recovery (E0302) already in place from Plan 04 stays"
      contains: "syncToTag"
    - path: "deal/src/expr.zig"
      provides: "Recursion-depth counter on Parser; parseExpression and parseDefinition bump it on entry and decrement on exit; E0403 fires at depth 256"
      contains: "MAX_PARSE_DEPTH"
    - path: "deal/src/json.zig"
      provides: "emitDiagnostics writes a JSON array of Diagnostic objects; every field including secondary_spans and fix_it is emitted in alphabetical order"
      contains: "emitDiagnostics"
    - path: "deal/tools/gen-malformed.zig"
      provides: "Corpus generator: walks tests/showcase/*.deal and *.dealx, applies 5+ mutation strategies (drop-token, swap-token, truncate, inject-garbage, unmatched-bracket), writes 30+ files to tests/malformed/"
    - path: "deal/tests/malformed/"
      provides: "20 hand-curated malformed files + 30+ generator-produced files = ≥50 total"
    - path: "deal/tests/unit/recovery_corpus.zig"
      provides: "Test that walks tests/malformed/ and asserts every file produces ≥1 diagnostic without panic"
  key_links:
    - from: "deal/src/parser_deal.zig syncToStatement / syncToDefinition"
      to: "deal/src/lexer.zig token tag set"
      via: "Skip tokens while peek().tag is not in the FOLLOW set defined by D-17 (statement: ;,},),]; definition: kw_part/kw_port/.../kw_use, kw_export, } at definition depth, eof)"
      pattern: "syncTo(Statement|Definition|Tag)"
    - from: "deal/src/diagnostics.zig"
      to: "deal/src/json.zig emitDiagnostics"
      via: "Diagnostic struct layout — every field name in the struct corresponds to a JSON key"
      pattern: "code|severity|message|span|secondary_spans|fix_it|notes"
    - from: "deal/build.zig gen-malformed step"
      to: "deal/tools/gen-malformed.zig"
      via: "b.addRunArtifact + b.step wiring (per CONTEXT discretionary structure; placeholder added in Plan 01)"
      pattern: "gen-malformed"
---

<objective>
Implement three-tier error recovery per D-17 (statement / definition / composition-tag synchronization) and the full structured-diagnostic data model per D-14/D-15/D-16. Build a malformed-input corpus generator + hand-curated cases to reach ≥50 malformed inputs, every one producing a structured Diagnostic without panic. Wire `deal_diagnostics_json` so every Diagnostic field round-trips losslessly through the C ABI.

Purpose: parsing is only the first half of Phase 1 — the second half is graceful failure. Plan 03/04 emit a few diagnostic codes (E0100..E0304) but their recovery is minimal (`expect` returns the wrong token and lets the caller continue). Real-world inputs (and especially LSP unsaved buffers) WILL be malformed; the parser must survive ALL of them. The ≥50-malformed-input gate (REQ-phase-1-4-error-recovery, ROADMAP success criterion #4) is the hardest correctness criterion in the phase because it requires reasoning about EVERY error path simultaneously.

Output: parser_deal + parser_dealx now have syncToStatement / syncToDefinition / syncToTag helpers; every recoverable error path calls one of them; the malformed corpus is ≥50 files and `recovery.corpus` asserts every file parses without panic; `diag.json_roundtrip` asserts every diagnostic field survives the C ABI.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md
@deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md
@deal/.planning/phases/01-zig-compiler-core/01-VALIDATION.md
@deal/.planning/phases/01-zig-compiler-core/01-03-SUMMARY.md
@deal/.planning/phases/01-zig-compiler-core/01-04-SUMMARY.md
@spec/grammar/deal.ebnf
@spec/grammar/dealx.ebnf
@spec/grammar/tmp-references/deal-parser-implementation-guide.html
@deal/src/diagnostics.zig
@deal/src/parser_deal.zig
@deal/src/parser_dealx.zig
@deal/src/expr.zig
@deal/src/json.zig
@deal/build.zig
@deal/tests/showcase

<interfaces>
From deal/src/diagnostics.zig (Plan 01 — extend):
- `pub const Severity = enum(u8) { err = 0, warn = 1, info = 2, hint = 3 };`
- `pub const SpanLabel = struct { span: ast.Span, label: []const u8 };`
- `pub const FixIt = struct { replacement: []const u8, replace_span: ast.Span };`
- `pub const Diagnostic = struct { code: []const u8, severity: Severity, message: []const u8, span: ast.Span, secondary_spans: []const SpanLabel = &.{}, fix_it: ?FixIt = null, notes: []const u8 = "" };`

From deal/src/parser_deal.zig (Plan 03/04 — extend):
- `pub const Parser = struct { arena, lexer, peeked, diagnostics: *std.ArrayList(Diagnostic), open_tags, source, current_mode, ... };`
- `pub fn expect(self: *Parser, expected: lexer.Tag) !lexer.Token` — currently minimal recovery; this plan replaces with proper sync.

From deal/src/parser_dealx.zig (Plan 04):
- `pushTag/popTag` already emit E0301/E0302/E0303/E0304; this plan adds syncToTag for malformed-tag content.
- `MAX_TAG_DEPTH = 1024` already bounded.

From deal/src/expr.zig (Plan 03):
- `pub fn parseExpression(parser, min_bp) !*ast.Node` — this plan adds recursion-depth bound.
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 5.1: Diagnostics struct expansion + collector helpers + recursion-depth bound</name>
  <read_first>
    - deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md (D-14 rich internal data model; D-15 Span = byte offsets; D-16 letter-prefixed numeric codes stable across versions; D-17 three-tier synchronization)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Pattern 5: Three-tier error recovery" (lines 428-446 — the sync-set table by tier), §"Example E: Diagnostic emission with both spans" (lines 824-852), §"Pitfall 5" (lines 598-608 — JSON determinism applies to diag JSON too), §"Threat Patterns" line 1006 (stack-overflow from deeply-nested input)
    - spec/grammar/tmp-references/deal-parser-implementation-guide.html §11 (error recovery sync sets — read the exact FOLLOW set tables)
    - deal/src/diagnostics.zig (Plan 01 stub, NEVER expanded — this plan is the first to add real bodies)
    - deal/src/parser_deal.zig (Plan 03/04 — Parser struct)
    - deal/src/expr.zig (Plan 03 — parseExpression)
  </read_first>
  <files>
    deal/src/diagnostics.zig,
    deal/src/expr.zig,
    deal/src/parser_deal.zig
  </files>
  <action>
1. In `deal/src/diagnostics.zig`, add a `DiagnosticCollector` helper that wraps the parser's diagnostic list and provides ergonomic emit functions:

   ```
   pub const DiagnosticCollector = struct {
       list: *std.ArrayList(Diagnostic),
       arena: std.mem.Allocator,

       pub fn emit(self: *DiagnosticCollector, code: []const u8, severity: Severity, message: []const u8, span: ast.Span) !void { ... appends with default secondary_spans/fix_it/notes ... }
       pub fn emitWithSecondary(self: *DiagnosticCollector, code: []const u8, severity: Severity, message: []const u8, span: ast.Span, secondary: []const SpanLabel) !void { ... dupes the secondary slice into the arena ... }
       pub fn emitFmt(self: *DiagnosticCollector, code: []const u8, severity: Severity, span: ast.Span, comptime fmt: []const u8, args: anytype) !void { const msg = try std.fmt.allocPrint(self.arena, fmt, args); try self.emit(code, severity, msg, span); }
   };
   ```

   - CRITICAL (RESEARCH §V7 line 993, threat T-DiagInjection line 1012): `emitFmt` uses a COMPTIME `fmt` string, never a runtime source-derived string. This is a build-time guarantee that source bytes never become format specifiers. Document this with a code comment citing the threat.
   - Every Diagnostic field (message, code, fix_it.replacement, secondary_spans labels, notes) MUST be allocated from `self.arena`. The collector dupes any input slice that came from outside the arena (e.g. `try self.arena.dupe(u8, message)`). RESEARCH anti-pattern line 521 — "Allocating diagnostic strings outside the arena" — is enforced here.

2. Add an error-code constants module at the top of `diagnostics.zig`:

   ```
   /// D-16 error-code namespace (LOCKED — codes never change meaning across versions).
   pub const Codes = struct {
       // Lexer (E0001-E0099)
       pub const e_invalid_utf8 = "E0001";
       pub const e_unterminated_string = "E0002";
       pub const e_unterminated_comment = "E0003";
       pub const e_source_too_large = "E0004";
       pub const e_template_too_deep = "E0005";

       // Parser-deal (E0100-E0299)
       pub const e_expected_token = "E0100";
       pub const e_expected_definition = "E0101";
       pub const e_unexpected_eof = "E0102";
       pub const e_expected_attribute_value = "E0103";
       pub const e_expected_expression = "E0110";
       pub const e_expected_identifier = "E0111";
       pub const e_invalid_modifier = "E0120";

       // Parser-dealx (E0300-E0399)
       pub const e_unmatched_close_tag = "E0301";
       pub const e_mismatched_close_tag = "E0302";
       pub const e_nesting_too_deep_tag = "E0303";
       pub const e_unclosed_tag_at_eof = "E0304";

       // Recovery / structural (E0400-E0499)
       pub const e_sync_dropped_tokens = "E0400";
       pub const e_unclosed_brace = "E0401";
       pub const e_unclosed_bracket = "E0402";
       pub const e_nesting_too_deep_expr = "E0403";

       // Warnings (W0500-W0599)
       pub const w_unused_import = "W0500";

       // Hints (H0600-H0699)
       pub const h_did_you_mean = "H0600";
   };
   ```

   - Codes that Plans 02/03/04 already emit (E0301..E0304, E0100..E0103) are listed here as `pub const` so callsites can use named constants going forward. Plan 02/03/04 may have inlined the string literals — that's fine; this plan introduces named constants and only converts the most error-prone callsites (sync helpers, expect()).

3. In `deal/src/expr.zig` (and `deal/src/parser_deal.zig`'s Parser struct), add the recursion-depth bound:

   - Add `depth: u16 = 0` and `pub const MAX_PARSE_DEPTH: u16 = 256;` to the `Parser` struct.
   - At the top of `parseExpression`, `parseDefinition`, `parseSatisfyBlock`, `parseInlineObjectOrExpr`, and any other recursive-grammar function: insert `parser.depth += 1; defer parser.depth -= 1; if (parser.depth > MAX_PARSE_DEPTH) { try parser.diagnostics.append(parser.arena, Diagnostic{ .code = Codes.e_nesting_too_deep_expr, .severity = .err, .message = "expression / definition nesting too deep", .span = parser.peek().span }); return parser.makeNode(.identifier, parser.peek().span, .{ .identifier = .{ .name = "" } }); }` — emit E0403 and return a sentinel empty Identifier node. The caller's loop then treats the sentinel as an end-of-input and synchronizes.
   - Bound applies to BOTH .deal and .dealx parser frames (they share the Parser struct).

4. **Task 5.1 does NOT change the `expect()` signature.** The Plan-03-shipped `expect` (which returns the wrong token on mismatch + appends a diagnostic) stays unchanged here so that the diagnostics-module expansion can land independently and `zig build` exits 0 mid-plan without breaking every existing callsite. The atomic conversion of `expect()` to an error-returning form + sync at every callsite is bundled into Task 5.2 (Warning #7 fix). Plan 03/04 tests continue to pass after Task 5.1 because no parser callsites are touched here — Task 5.1 only expands `diagnostics.zig` and adds the depth counter.
  </action>
  <acceptance_criteria>
    - deal/src/diagnostics.zig declares `pub const DiagnosticCollector` and `pub const Codes`
    - deal/src/diagnostics.zig declares all error-code constants listed above (grep for E0001, E0002, ..., E0403, W0500, H0600)
    - deal/src/expr.zig contains `MAX_PARSE_DEPTH` and `parser.depth += 1` (or equivalent depth-bump idiom) at the entry of parseExpression
    - deal/src/parser_deal.zig's `expect` function signature is UNCHANGED in this task (it still returns the wrong token + appends a diagnostic per Plan 03's shape); Task 5.2 performs the atomic conversion to error-returning + sync. This deliberate split (Warning #7 fix) keeps `zig build` green at the end of Task 5.1.
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build 2>&1 — exits 0 (the diagnostics expansion + Parser struct extension compiles; expect() signature unchanged so all existing tests stay green — per Warning #7 fix the signature change is bundled into Task 5.2)</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test 2>&1 — exits 0 (Plan 03/04 tests still pass — Task 5.1 has zero parser-callsite churn)</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build 2>&1 | tail -5</automated>
  </verify>
  <done>
    Diagnostic data model is now Phase-2-ready: Diagnostic struct with full secondary_spans/fix_it/notes; DiagnosticCollector helper enforces arena-owned strings; D-16 error codes are named constants. Parser struct has recursion-depth field bounded at 256 with E0403 emission. `expect()` signature unchanged (deliberate, per Warning #7) — `zig build` exits 0 with all Plan 03/04 tests still green. Task 5.2 performs the atomic signature change + sync wiring as one cohesive step.
  </done>
</task>

<task type="auto">
  <name>Task 5.2: syncToStatement / syncToDefinition / syncToTag in parser_deal + parser_dealx + unit tests for each tier</name>
  <read_first>
    - deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md (D-17 three-tier sync sets)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Pattern 5: Three-tier error recovery" (lines 428-446 — exact FOLLOW set table)
    - spec/grammar/tmp-references/deal-parser-implementation-guide.html §11 (sync sets — confirm RESEARCH's table matches)
    - deal/src/parser_deal.zig (Plan 03/04, Task 5.1 — Parser struct, ParseError)
    - deal/src/parser_dealx.zig (Plan 04)
    - deal/src/diagnostics.zig (Task 5.1 — Codes constants)
  </read_first>
  <files>
    deal/src/parser_deal.zig,
    deal/src/parser_dealx.zig,
    deal/src/parser.zig,
    deal/src/expr.zig,
    deal/tests/unit/recovery_statement.zig,
    deal/tests/unit/recovery_definition.zig,
    deal/tests/unit/recovery_dealx_tag.zig
  </files>
  <action>
0. **Atomic `expect()` signature change (Warning #7 — moved here from Task 5.1).** In a single commit, convert `expect()` from "returns wrong token + appends diagnostic" to "returns error union" AND convert EVERY callsite in `parser_deal.zig` AND `parser_dealx.zig` to the new sync pattern (item 2 below). The conversion is atomic — between commit-start and commit-end the codebase is never in a half-converted state. Required steps:

   - Add `pub const ParseError = error{ UnexpectedToken, NestingTooDeep, OutOfMemory };` to `parser_deal.zig` at module scope.
   - Rewrite `expect`:
     ```
     pub fn expect(self: *Parser, expected: lexer.Tag) ParseError!lexer.Token {
         const tok = self.peek();
         if (tok.tag == expected) {
             _ = self.advance();
             return tok;
         }
         try self.diagnostics.append(self.arena, .{
             .code = Codes.e_expected_token,
             .severity = .err,
             .message = try std.fmt.allocPrint(self.arena, "expected {s}, found {s}", .{ @tagName(expected), @tagName(tok.tag) }),
             .span = tok.span,
         });
         return error.UnexpectedToken;  // DO NOT advance — caller's sync helper decides
     }
     ```
   - In the same commit, walk every callsite of `expect(...)` in the following file scopes and convert each to `expect(...) catch { syncTo*(self); return null; }` (or the equivalent error-propagation pattern; item 2 below details the per-scope sync target):
     - `deal/src/parser_deal.zig` — all 14 definitional parsers (`parsePartDef`, `parsePortDef`, `parseActionDef`, `parseStateDef`, `parseAttributeDef`, `parseItemDef`, `parseInterfaceDef`, `parseConnectionDef`, `parseFlowDef`, `parseAllocationDef`, `parseRequirementDef`, `parseConstraintDef`, `parseNeedDef`, `parseUseCaseDef`), all member-list / annotation / type-annotation / multiplicity / modifier helpers, the `parseDealFile` main loop, `parseHeaderBlock`, `parsePackageDecl`, `parseImportDecl`, `parseExportDecl`.
     - `deal/src/parser_dealx.zig` — `parseFile`, `parseDealxFile`, `parseSystemContent`, `parsePairTag`, `parseConnect`, `parseExposeTag`, `parseAllocateTag`, `parseComponentInstance`, `parseAttributes`, `parseInlineObjectOrExpr`, `parseSatisfyBlock`, `parseTraceabilityBlock`, `parseValidateTag`.
   - Inside `expr.parseExpression` / `parsePrefix` / `parsePrimary`, on error: call `syncToStatement` and return a sentinel Identifier node (empty name) instead of propagating the error — see item 5 below.
   - The atomic commit means `zig build` exits 0 BOTH before the commit (Task 5.1 end-state) AND after the commit (Task 5.2 end-state). No intermediate red state. Plan 03/04 snapshots remain byte-identical (well-formed inputs never trigger the error path).

1. Implement the three sync helpers per RESEARCH §Pattern 5 lines 432-443 in `deal/src/parser_deal.zig`:

   ```
   const SyncSet = std.EnumSet(lexer.Tag);

   const STATEMENT_SYNC: SyncSet = SyncSet.init(.{
       .semicolon = true, .comma = true, .r_brace = true, .r_paren = true, .r_bracket = true,
   });

   const DEFINITION_SYNC: SyncSet = SyncSet.init(.{
       .kw_part = true, .kw_port = true, .kw_action = true, .kw_state = true,
       .kw_attribute = true, .kw_item = true, .kw_interface = true, .kw_connection = true,
       .kw_flow = true, .kw_allocation = true, .kw_requirement = true, .kw_constraint = true,
       .kw_need = true, .kw_use = true, .kw_export = true, .r_brace = true, .eof = true,
   });

   pub fn syncToStatement(self: *Parser) void {
       const start = self.peek().span.start;
       var dropped: u32 = 0;
       while (!STATEMENT_SYNC.contains(self.peek().tag) and self.peek().tag != .eof) {
           _ = self.advance();
           dropped += 1;
       }
       if (dropped > 0) {
           const end = self.peek().span.start;  // do NOT consume the sync token
           self.diagnostics.append(self.arena, .{
               .code = Codes.e_sync_dropped_tokens, .severity = .info,
               .message = "skipped tokens during error recovery",
               .span = .{ .start = start, .end = end },
           }) catch {};
       }
   }

   pub fn syncToDefinition(self: *Parser) void {
       /* same shape, uses DEFINITION_SYNC */
   }
   ```

   Notes:
   - `EnumSet` is a Zig 0.16.0 std-lib type. Verify the exact constructor (`std.EnumSet(E).init(.{ .a = true })` vs `initFull` / `initEmpty`); if `init(.{...})` doesn't compile, use `var s = SyncSet.initEmpty(); s.insert(.semicolon); ...` form.
   - The sync helpers do NOT consume the sync token — they leave it at peek so the next production can decide what to do with it.
   - `dropped > 0` triggers an E0400 info-severity diagnostic so the user can see the recovery happened (per D-14's "renderer agnostic" model — info severity won't block CI but will surface in LSP).
   - Catch on `.append` is safe-ignored (`catch {}`) because the diagnostic list itself failing to append is an OOM that's already terminal; the parser's main path uses `try`.

2. Convert every `expect(...)` callsite in `parser_deal.zig` and the function-level error paths to use sync:

   ```
   // Old pattern (Plan 03):
   //   const tok = try self.expect(.l_brace);
   //   if (tok.tag != .l_brace) return error.ParseFailed;
   //
   // New pattern (this plan):
   //   _ = self.expect(.l_brace) catch {
   //       self.syncToStatement();  // or syncToDefinition, depending on context
   //       return null;
   //   };
   ```

   Apply this transformation across all definitional parsers (`parsePartDef`, `parsePortDef`, ..., `parseRequirementDef`), all member parsers, all type/multiplicity/modifier parsers, and the statement loop inside member bodies.

   - INSIDE expressions (parseExpression, parsePrimary, parseArgList): on error, syncToStatement and return a sentinel Identifier node (empty name) — DO NOT propagate the error out of parseExpression because the caller's grammar production may successfully sync at the next semicolon.
   - INSIDE definition bodies (the `{ member* }` loop): on a member-level error, syncToStatement first; if peek is then `r_brace`, exit the loop normally; if it's a definition keyword, exit the loop AND let the outer parser continue from there.
   - AT TOP LEVEL (parseDealFile's main loop): on a `parseDefinition` error, syncToDefinition; if peek is `eof`, stop; else continue the loop.

3. Implement `syncToTag` in `deal/src/parser_dealx.zig`:

   ```
   const TAG_SYNC: std.EnumSet(lexer.Tag) = std.EnumSet(lexer.Tag).init(.{
       .tag_open = true, .tag_close_open = true, .kw_package = true,
       .kw_import = true, .kw_export = true, .eof = true,
   });

   pub fn syncToTag(self: *parser_deal.Parser) void {
       // Mode-aware: we MUST be in .dealx_outer when syncing because tag_open/tag_close_open
       // are only recognized in that mode. If current_mode != .dealx_outer, restore first.
       const prev_mode = self.current_mode;
       self.current_mode = .dealx_outer;
       defer self.current_mode = prev_mode;
       self.peeked = null;  // re-lex under .dealx_outer
       const start = self.peek().span.start;
       var dropped: u32 = 0;
       while (!TAG_SYNC.contains(self.peek().tag) and self.peek().tag != .eof) {
           _ = self.advance();
           dropped += 1;
       }
       if (dropped > 0) { /* emit E0400 with span [start, self.peek().span.start] */ }
   }
   ```

   - Convert every `expect(...)` callsite in parser_dealx (parseSystemBlock, parseConnect, parseAttributes, parseInlineObjectOrExpr, ...) to use `expect() catch { syncToTag(self); return null; }` pattern. Inside an attribute parse error, sync to the next attribute (peek `ident eq` or `tag_close`); inside a tag-content error, sync to the next `[<` / `[</`.

4. Convert parser_deal's parseDealFile main loop to use syncToDefinition on error:

   ```
   while (parser.peek().tag != .eof) {
       const def = parser_deal.parseDefinition(parser) catch null;
       if (def) |d| { try definitions.append(parser.arena, d); }
       else { parser_deal.syncToDefinition(parser); }
   }
   ```

5. Update parseExpression in `deal/src/expr.zig` so that any error inside parsePrefix / parsePrimary returns a sentinel Identifier node AFTER calling syncToStatement (so the calling production continues at the next statement boundary). This is the "expression-bounded sync" arm of D-17.

6. Create `deal/tests/unit/recovery_statement.zig` with `test "recovery.statement"`:
   - Case A — missing semicolon: `"package foo; part def P { attr battery : Battery attr motor : Motor; }"`. The first `attr battery : Battery` is missing its terminating `;`. Expected diagnostics: ≥1 with code E0100 ("expected ;"); span covers the `attr motor` token. After sync, the SECOND attribute parses correctly. Assert: parsed root has 1 definition (P) with 2 attribute members.
   - Case B — missing comma in arg list: `"part def P { x = f(a b); }"`. Sync to next `,` or `)`. Assert: ≥1 diagnostic at `b` token; parsing continues; statement ends at `;`.
   - Case C — sync recovers within {...}: `"part def P { @@@ badtoken; attr ok : T; }"`. Garbage tokens at the start of the body are skipped to `;`; the second attribute parses correctly. Assert: ≥1 E0400 ("skipped tokens"); root has 1 definition with ≥1 attribute member named "ok".

7. Create `deal/tests/unit/recovery_definition.zig` with `test "recovery.definition"`:
   - Case A — malformed first definition, valid second: `"part def Broken { GARBAGE GARBAGE part def Good { attr x : T; } }"`. Wait — `part def` inside an existing body would be a member; instead use: `"part def Broken GARBAGE GARBAGE\npart def Good { attr x : T; }"` (no closing brace for Broken). Assert: ≥1 diagnostic from Broken; ≥1 definition produced (Good) with the correct attribute.
   - Case B — definition keyword in mid-body recovery: `"part def P { @@@ part def Q { attr y : T; } }"`. Inside P's body, garbage tokens; sync sees `part` keyword → DEFINITION_SYNC matches → exits P's body. Then Q parses cleanly. Result: root has 2 definitions (P with no members, Q with 1 attribute).
   - Case C — bad header: `"@header { invalid syntax } part def P {}"`. Header parser fails; syncs to `part` keyword; P parses cleanly.

8. Create `deal/tests/unit/recovery_dealx_tag.zig` with `test "recovery.dealx_tag"`:
   - Case A — malformed tag content: `"[<system Foo>] @@@bad@@@ [<subsystem Bar>][</subsystem>][</system>]"`. The `@@@bad@@@` inside system Foo's content is garbage; syncToTag finds the next `[<` (subsystem). After recovery: system Foo has 1 child (subsystem Bar).
   - Case B — orphan content: `"[<system Foo>][</system>] @@@ garbage @@@ [<system Bar>][</system>]"`. Between the two systems, sync to next `[<`; both systems parse cleanly.
   - Case C — recovery within attributes: `"[<connect from='a' WRONG TOKENS to='b' />]"`. Attribute parser fails on WRONG; syncs forward to next `ident eq` or `tag_self_close`. Result: connect node with from='a', to='b' (if recovery was clean) OR with from='a' only (acceptable per D-17 — diagnostic emitted, parsing continues).

9. Wire all three recovery test files into `deal/build.zig` test step.
  </action>
  <acceptance_criteria>
    - deal/src/parser_deal.zig contains literal strings `STATEMENT_SYNC`, `DEFINITION_SYNC`, `syncToStatement`, `syncToDefinition`
    - deal/src/parser_dealx.zig contains `syncToTag` and `TAG_SYNC`
    - deal/src/parser_deal.zig converts the top-level parseDealFile loop to use `parseDefinition catch null; syncToDefinition(parser);` pattern (grep)
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=recovery.statement 2>&1 — exits 0; all 3 cases pass</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=recovery.definition 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=recovery.dealx_tag 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_deal.snapshot 2>&1 — exits 0 (Plan 03 snapshots still pass; well-formed inputs produce identical output even with the new sync paths)</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_dealx.snapshot 2>&1 — exits 0 (Plan 04 snapshots still pass)</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test 2>&1 | tail -20</automated>
  </verify>
  <done>
    Three-tier recovery (D-17) implemented: syncToStatement, syncToDefinition, syncToTag with FOLLOW sets matching RESEARCH §Pattern 5. The `expect()` signature change is ATOMIC with the per-callsite sync conversion (Warning #7 fix: not split across tasks, no intermediate red state). Every grammar production that calls `expect()` now catches the error and syncs at the right tier. Plan 03/04 snapshots remain byte-stable (the new recovery paths only activate on malformed input, not on the well-formed showcase corpus). Three recovery unit tests (statement, definition, dealx_tag) each have ≥3 cases green.
  </done>
</task>

<task type="auto">
  <name>Task 5.3: Malformed corpus (gen-malformed + hand-curated) + recovery.corpus test + diag.json_roundtrip</name>
  <read_first>
    - deal/.planning/phases/01-zig-compiler-core/01-VALIDATION.md (Wave 0 requirements: "tests/malformed/ — directory + initial hand-curated 20 malformed files + gen-malformed.zig corpus generator (zig build gen-malformed) → ≥50 total")
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Phase Requirements → Test Map" lines 954-958 (recovery.corpus, recovery.statement, recovery.definition, recovery.dealx_tag, diag.json_roundtrip)
    - spec/examples/showcase/ — the source for mutation-based generation
    - deal/src/json.zig (Plan 03's emitAst — extend with emitDiagnostics in this task)
    - deal/src/diagnostics.zig (Task 5.1 — Diagnostic struct)
    - deal/build.zig (Plan 01 placeholder gen-malformed step)
  </read_first>
  <files>
    deal/tools/gen-malformed.zig,
    deal/src/json.zig,
    deal/build.zig,
    deal/tests/malformed/m01_missing_semicolon.deal,
    deal/tests/malformed/m02_unclosed_brace.deal,
    deal/tests/malformed/m03_unclosed_paren.deal,
    deal/tests/malformed/m04_garbage_after_keyword.deal,
    deal/tests/malformed/m05_invalid_utf8.deal,
    deal/tests/malformed/m06_unterminated_string.deal,
    deal/tests/malformed/m07_unterminated_template.deal,
    deal/tests/malformed/m08_unterminated_block_comment.deal,
    deal/tests/malformed/m09_invalid_modifier.deal,
    deal/tests/malformed/m10_orphan_close_tag.dealx,
    deal/tests/malformed/m11_mismatched_close_tag.dealx,
    deal/tests/malformed/m12_unclosed_tag_at_eof.dealx,
    deal/tests/malformed/m13_malformed_attr_value.dealx,
    deal/tests/malformed/m14_brace_inside_attr.dealx,
    deal/tests/malformed/m15_double_at_in_annotation.deal,
    deal/tests/malformed/m16_bare_expression_at_top.deal,
    deal/tests/malformed/m17_keyword_as_ident.deal,
    deal/tests/malformed/m18_dangling_dot.deal,
    deal/tests/malformed/m19_empty_arg_list_comma.deal,
    deal/tests/malformed/m20_deeply_nested_braces.deal,
    deal/tests/unit/recovery_corpus.zig,
    deal/tests/unit/diag_json_roundtrip.zig
  </files>
  <action>
1. Hand-curate 20 malformed input files under `deal/tests/malformed/` (filenames listed in <files>). Each file is intentionally invalid and exercises one of the diagnostic codes from `diagnostics.Codes`:

   - m01_missing_semicolon.deal: `package foo;\npart def P { attr a : T attr b : T; }` (missing `;` after first attr)
   - m02_unclosed_brace.deal: `package foo;\npart def P { attr a : T;` (no closing brace)
   - m03_unclosed_paren.deal: `package foo;\npart def P { x = f(a, b ; }`
   - m04_garbage_after_keyword.deal: `package foo;\npart def @@@bad { }`
   - m05_invalid_utf8.deal: a COMMITTED binary file whose exact byte sequence is:
     - bytes 0x00-0x0C (13 bytes): the ASCII text `package foo;\n` (i.e. the 13 bytes `0x70 0x61 0x63 0x6b 0x61 0x67 0x65 0x20 0x66 0x6f 0x6f 0x3b 0x0a`).
     - byte 0x0D (1 byte): `0xC3` — a leading byte that announces a 2-byte UTF-8 sequence with NO valid continuation byte following it (EOF before the continuation). Per RFC 3629 this is malformed UTF-8.
     - Total file size: 14 bytes.
     This must trigger E0001 at the `deal_parse` UTF-8 validation gate added in Plan 06 (Task 6.1). The file is committed as-is to the repo — its hash is stable across CI machines. The `printf '%s\xc3' "package foo;\n" > m05.deal` shell command shown in earlier drafts was one-time-illustrative for HOW to produce the bytes; the actual artifact is the committed binary, not a regeneration step. NOTE #10 fix: documenting the byte sequence directly removes the regeneration ambiguity.
   - m06_unterminated_string.deal: `package foo;\npart def P { attr msg = "hello world; }`
   - m07_unterminated_template.deal: `package foo;\npart def P { attr msg = \`hello ${world; }`
   - m08_unterminated_block_comment.deal: `package foo;\n/* never closed\npart def P {}`
   - m09_invalid_modifier.deal: `package foo;\n<<not_a_valid_modifier>> part def P {}`
   - m10_orphan_close_tag.dealx: `package model;\n[</orphan>]` (E0301)
   - m11_mismatched_close_tag.dealx: `package model;\n[<system Foo>][</subsystem>]` (E0302 with both spans)
   - m12_unclosed_tag_at_eof.dealx: `package model;\n[<system Foo>]` (E0304)
   - m13_malformed_attr_value.dealx: `package model;\n[<connect from= to='b' />]` (missing value for from)
   - m14_brace_inside_attr.dealx: `package model;\n[<connect via={ unclosed />]` (unclosed inner brace)
   - m15_double_at_in_annotation.deal: `package foo;\n@@trace:satisfies REQ_01\npart def P {}`
   - m16_bare_expression_at_top.deal: `package foo;\n2 + 2;` (expression at file scope)
   - m17_keyword_as_ident.deal: `package foo;\npart def part { attr port : T; }` (using `part` as a type name)
   - m18_dangling_dot.deal: `package foo;\npart def P { x = a.; }`
   - m19_empty_arg_list_comma.deal: `package foo;\npart def P { x = f(,); }`
   - m20_deeply_nested_braces.deal: a programmatically generated file with 300 levels of `{ ` nested (one file in the malformed corpus exercises the E0403 depth bound at 256). Use a Zig comptime string-builder OR generate via gen-malformed.zig at first invocation.

2. Create `deal/tools/gen-malformed.zig` — a standalone Zig executable (run via `zig build gen-malformed`) that produces ≥30 additional malformed files from mutation of the 19 showcase files. Required mutations:

   - **drop-token**: parse the showcase file's lexer output; pick a random token; remove its bytes from the source. Write to `tests/malformed/gen_drop_<showcase>_<index>.deal[x]`.
   - **swap-token**: swap two adjacent tokens' positions in the source.
   - **truncate**: cut the source at a random byte offset (often mid-token).
   - **inject-garbage**: insert `@@@` or `&&&` or a random 5-byte sequence at a random position.
   - **unmatched-bracket**: remove one `}` or `]` or `)` from the source.

   For each showcase file × each mutation type = 19 × 5 = 95 potential files; cap at 30+ by sampling. Use a fixed PRNG seed (e.g. `std.Random.DefaultPrng.init(0xDEAL2026)`) so generation is deterministic and CI-stable.

   The executable:
   - Reads the showcase corpus from `tests/showcase/`.
   - For each file, applies each mutation strategy (at least one of each), writes the output to `tests/malformed/gen_<strategy>_<base>_<seq>.deal[x]`.
   - Stops after writing ≥30 files (so total = 20 hand-curated + 30 generated = 50+).
   - Writes a manifest file `tests/malformed/_manifest.txt` listing every malformed file with the originating showcase + mutation type, so the test report is interpretable.

3. Wire `gen-malformed` step in `deal/build.zig`:

   ```
   const gen_malformed_exe = b.addExecutable(.{
       .name = "gen-malformed",
       .root_module = b.createModule(.{
           .root_source_file = b.path("tools/gen-malformed.zig"),
           .target = target,
           .optimize = optimize,
       }),
   });
   const run_gen_malformed = b.addRunArtifact(gen_malformed_exe);
   const gen_malformed_step = b.step("gen-malformed", "Regenerate the malformed corpus from showcase mutations");
   gen_malformed_step.dependOn(&run_gen_malformed.step);
   ```

   Replace the placeholder gen_malformed step from Plan 01.

4. Add `pub fn emitDiagnostics(allocator, diags) ![]const u8` to `deal/src/json.zig` (replacing Plan 01's stub `return "[]"`). The function writes a JSON array of objects:

   ```
   [
     {"code":"E0302","severity":"err","message":"...","span":[10,18],"secondary_spans":[{"label":"opened here","span":[2,9]}],"fix_it":null,"notes":""},
     ...
   ]
   ```

   - Hand-rolled writer (same convention as emitAst: alphabetical field order per object).
   - `severity` emits as the lowercase enum tag name (`err` / `warn` / `info` / `hint`).
   - `span` emits as a two-element array `[start, end]` (D-15 byte offsets).
   - `secondary_spans` emits as an array of `{"label":"...","span":[s,e]}` objects (alphabetical: label before span).
   - `fix_it` emits as `null` or `{"replace_span":[s,e],"replacement":"..."}` (alphabetical).
   - `notes` always emitted as a string (possibly empty).
   - String escaping per Plan 02's helper.

5. Update `deal/src/lib.zig`'s `deal_diagnostics_json` to call `json.emitDiagnostics(handle.arena.allocator(), handle.diagnostics.items)` (was stubbed in Plan 01 to return `[]`).

6. Create `deal/tests/unit/recovery_corpus.zig` with `test "recovery.corpus"`:
   - Open `tests/malformed/` directory; iterate every file.
   - For each: read the file bytes; call `deal_parse(bytes, filename)` via internal API; assert:
     - `handle != null` (D-10 guarantee)
     - `deal_has_errors(handle) == true` (every malformed file produces ≥1 diagnostic)
     - The parse did NOT panic (Zig test harness's panic-handler catches; pass-condition is "test runner completes the whole loop without abort")
     - `deal_diagnostics_count(handle) >= 1`
   - At end: assert the corpus has ≥50 files. Read the directory listing; count `.deal` + `.dealx` files; fail if < 50.
   - Write a summary line to stderr: "recovery.corpus: 53 malformed files, 87 total diagnostics emitted, 0 panics".

7. Create `deal/tests/unit/diag_json_roundtrip.zig` with `test "diag.json_roundtrip"`:
   - Construct a Diagnostic struct in-memory with ALL fields populated (code="E0302", severity=.err, message="Test message with \"escapes\" and \\backslashes", span=[10,20], secondary_spans=[{label="here", span=[5,8]}, {label="and here", span=[30,35]}], fix_it={replacement="fixed text", replace_span=[10,20]}, notes="Multi-line notes\nwith\ttabs").
   - Call `json.emitDiagnostics(allocator, &.{ diag })`.
   - Parse the resulting JSON via `std.json.parseFromSlice(std.json.Value, ...)`.
   - Assert every field is present in the parsed JSON with the expected value (code, severity="err", message text exactly, span array, secondary_spans length 2 and each with label + span, fix_it.replacement, fix_it.replace_span, notes exact).
   - Add a second case where all optional fields are at their default (fix_it=null, notes="", secondary_spans=&.{}) and assert the JSON contains `"fix_it":null` and `"notes":""` and `"secondary_spans":[]` literally.
  </action>
  <acceptance_criteria>
    - All 20 hand-curated malformed files exist under deal/tests/malformed/
    - deal/tools/gen-malformed.zig exists and compiles
    - deal/build.zig wires `b.step("gen-malformed")`
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build gen-malformed 2>&1 — exits 0; afterwards `ls tests/malformed/*.deal tests/malformed/*.dealx | wc -l` produces ≥ 50</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && ls tests/malformed/_manifest.txt 2>&1 — file exists</automated>
    - deal/src/json.zig contains `pub fn emitDiagnostics`
    - deal/src/lib.zig's `deal_diagnostics_json` calls `json.emitDiagnostics(...)` (not the Plan 01 stub returning `[]`)
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=recovery.corpus 2>&1 — exits 0; the test confirms ≥50 files, every file produces ≥1 diagnostic, no panics</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=diag.json_roundtrip 2>&1 — exits 0; every diagnostic field survives the JSON round-trip</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test 2>&1 — full suite green</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build gen-malformed && zig build test 2>&1 | tail -15</automated>
  </verify>
  <done>
    Malformed corpus has ≥50 files (20 hand-curated covering specific diagnostic codes + 30+ generator-produced via 5 mutation strategies applied to the 19 showcase files). `recovery.corpus` test asserts every file produces ≥1 diagnostic without panic — Phase 1 ROADMAP success criterion #4 met. `diag.json_roundtrip` proves every Diagnostic field (including secondary_spans, fix_it, notes) survives the C ABI without loss. `deal_diagnostics_json` is now the real implementation; the stub from Plan 01 is gone.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Malformed input → parser | Every byte sequence is a potential adversarial input. The recovery machinery is the security perimeter for parser-DoS attacks. |
| Recursion depth → OS stack | parseExpression / parseDefinition recursive descent can consume stack frames proportional to input nesting depth. |
| Diagnostic strings → JSON output | Diagnostic messages embed source-derived text (e.g. tag names). Format-string injection (RESEARCH line 1012) is the primary risk. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-05-01 | DoS | Stack overflow from deeply-nested expressions | mitigate | MAX_PARSE_DEPTH = 256 bound on Parser.depth. Past 256, E0403 fires and parsing returns a sentinel; no stack growth. Exercised by m20_deeply_nested_braces.deal. |
| T-05-02 | DoS | Sync loop infinite-skip on EOF | mitigate | syncTo* explicitly check `peek().tag != .eof` in the loop condition. EOF is in every sync set, so the loop terminates. Tested by m12_unclosed_tag_at_eof.dealx. |
| T-05-03 | Tampering | Diagnostic format-string injection | mitigate | DiagnosticCollector.emitFmt requires a comptime fmt string (`comptime fmt: []const u8`). Zig 0.16.0 enforces this — runtime fmt arguments cannot be `comptime`, so source bytes cannot reach the format machinery. RESEARCH line 1012. |
| T-05-04 | Information Disclosure | Source bytes in diagnostic messages | accept | Diagnostic messages may include grammar terms (token tag names via `@tagName(tag)`, identifier slices) but NOT raw source bytes outside identified spans. Source slices used in messages are bounded to the token span and escaped through the JSON writer's escape helper (Plan 02 / Plan 03). |
| T-05-05 | Resource Exhaustion | Sync helpers allocating diagnostics per dropped token | mitigate | Sync helpers emit at most ONE E0400 diagnostic per sync call (covering the entire skipped range as a single span). NOT one diagnostic per dropped token — that would scale linearly with input size and exhaust the arena. |
| T-05-06 | Tampering | Mutation generator non-determinism | mitigate | gen-malformed.zig uses a fixed PRNG seed (0xDEAL2026). Identical mutations produced on every invocation → CI stability. |
| T-05-SC | Tampering | No new external dependencies | accept | This plan adds Zig std-lib types (`std.EnumSet`, `std.Random.DefaultPrng`, `std.json.parseFromSlice`) only. build.zig.zon `.dependencies` remains empty. |
</threat_model>

<verification>
1. `cd /Users/dunnock/projects/deal-lang/deal && zig build` exits 0.
2. `cd /Users/dunnock/projects/deal-lang/deal && zig build gen-malformed` exits 0; `ls tests/malformed/ | wc -l` ≥ 51 (50 files + _manifest.txt).
3. `cd /Users/dunnock/projects/deal-lang/deal && zig build test` exits 0 — full suite green: all tests from Plans 02/03/04 still pass; new recovery + diag.json_roundtrip tests pass.
4. `cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=recovery.corpus` exits 0; the test reports ≥50 files, ≥1 diagnostic per file, 0 panics.
5. `cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=diag.json_roundtrip` exits 0.
6. `cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_deal.snapshot && zig build test -Dtest-filter=parser_dealx.snapshot` both exit 0 — snapshots from Plans 03/04 are byte-identical (recovery paths only fire on malformed input; well-formed showcase corpus is unaffected).
7. `cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml` exits 0 — Rust harness still consumes the C ABI correctly; `deal_diagnostics_json` returns a non-trivial JSON array (no longer the Plan 01 stub `[]`).
</verification>

<success_criteria>
- Phase 1 ROADMAP success criterion #4 met: "The parser survives 50+ malformed-input test cases without panic, emitting span-accurate structured diagnostics that synchronize at statement/definition boundaries."
- REQ-phase-1-4-error-recovery acceptance met: 50+ error recovery test cases; structured diagnostic types; span-accurate error reporting.
- D-14 (rich internal Diagnostic model) honored: code, severity, message, span, secondary_spans, fix_it, notes all present and JSON-round-trippable.
- D-15 (Span = byte offsets only, u32 start/end) honored throughout sync helpers and diagnostic emission.
- D-16 (letter-prefixed numeric codes, stable across versions) honored: `Codes` namespace declares every code as a `pub const` string constant; ranges respect E0001..E0099 lexer, E0100..E0299 parser-deal, E0300..E0399 parser-dealx, E0400..E0499 recovery, W0500..W0599 warn, H0600..H0699 hint.
- D-17 (three-tier synchronization) honored: syncToStatement / syncToDefinition / syncToTag with FOLLOW sets matching RESEARCH §Pattern 5.
- All Validation Architecture rows for recovery (RESEARCH lines 954-958) green: recovery.corpus, recovery.statement, recovery.definition, recovery.dealx_tag, diag.json_roundtrip.
</success_criteria>

<output>
Create `.planning/phases/01-zig-compiler-core/01-05-SUMMARY.md` when done, recording:
- Final malformed-corpus file count (target: ≥50; record actual).
- The exact mutations applied per showcase file (the manifest content).
- Which expect() callsites in parser_deal/parser_dealx required the largest behavioral changes (e.g. did parseInlineObjectOrExpr need special-case mode restoration on error?).
- Whether the E0403 depth bound at 256 was reached by any of the 19 showcase files (it should NOT be — if showcase nests > 256 levels, the bound is too tight and must be raised).
- Total diagnostic count emitted across the 50+ malformed corpus (for Plan 06's gate test to baseline against).
- Whether `std.EnumSet` API in 0.16.0 matched the literal `SyncSet.init(.{...})` pattern or required the `initEmpty + .insert` form.
</output>
