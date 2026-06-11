---
phase: 01-zig-compiler-core
plan: 04
type: execute
wave: 4
depends_on:
  - 01-03-parser-deal
files_modified:
  - deal/src/ast.zig
  - deal/src/parser_dealx.zig
  - deal/src/parser.zig
  - deal/src/parser_deal.zig
  - deal/src/json.zig
  - deal/tests/unit/parser_dealx_smoke.zig
  - deal/tests/unit/parser_dealx_tag_balance.zig
  - deal/tests/unit/parser_dealx_connect_via.zig
  - deal/tests/unit/parser_dealx_snapshot.zig
  - deal/tests/unit/parser_dealx_coverage.zig
  - deal/tests/snapshots/ast/showcase__model__vehicle.dealx.json
  - deal/tests/snapshots/ast/showcase__model__traceability.dealx.json
  - deal/tests/snapshots/ast/showcase__model__variants__sedan.dealx.json
  - deal/tests/snapshots/ast/showcase__model__variants__performance.dealx.json
  - deal/build.zig
autonomous: true
requirements:
  - REQ-phase-1-3-parser-dealx

must_haves:
  decisions_implemented:
    - "D-03: Unified NodeKind (`.dealx` half) — `comp_*` payload bodies fully populated (`SystemBlock`, `SubsystemBlock`, `ComponentInstance`, `CompConnect`, `ExposeTag`, `AllocateTag`, `TraceabilityBlock`, `SatisfyBlock`, `ValidateTag`, `ObjectLiteral`); `NodeKind` enum itself UNCHANGED from Plan 03"
    - "D-06: Mode flips driven by parser — `parser.enterMode(.dealx_outer)` at file scope, `.dealx_tag` inside tag bodies, `.dealx_expr_brace` inside inline `{...}` per `parser.current_mode`; lookahead cache invalidated on every mode transition"
    - "D-07: Parser-owned open-tag stack — `pub const OpenTag = struct { name, span }`, `MAX_TAG_DEPTH=1024`, `pushTag`/`popTag` bodies emit E0301 (unmatched close), E0302 (mismatched — pop anyway + both spans), E0303 (depth bound), E0304 (unclosed at EOF) per D-17 recovery; verified by `parser_dealx.tag_balance` cases A-E"
    - "D-08: Inline `{...}` accepts full deal expression grammar — `parseInlineObjectOrExpr` re-enters via `expr.parseExpression`; ObjectLiteral and raw expression both supported; verified by `parser_dealx.connect_via` case 3 (nested `via={Outer { inner: {nested:1} }}`)"
    - "D-09: First-class `comp_connect` with typed slots — `CompConnect = struct { from_expr, to_expr, via_expr, carrying_expr: ?*Node, other_props: []*Node }`; parseConnect routes `from`/`to`/`via`/`carrying` keys to typed slots; verified by `parser_dealx.connect_via` case 1"
    - "D-17: Composition-level sync interface — `.dealx` tag-resync hook lands via `pushTag`/`popTag` body fills (Plan 05 adds full `syncToTag` helper for malformed inner-tag content)"
    - "D-18: Snapshots from Plan 03 remain byte-identical — alphabetical field-order invariant preserved when adding `comp_*` arms to `json.zig` switch; verified in Task 4.1c via `git diff` against HEAD for every Plan 03 `.deal` snapshot (must produce 0 lines)"
  truths:
    - "All 4 .dealx showcase files parse through parser_dealx.parseFile producing a non-null ast.Node tree"
    - "All 4 .dealx AST JSON snapshots are byte-stable across consecutive `zig build test -Dtest-filter=parser_dealx.snapshot` runs"
    - "Open-tag stack pushes on `[<name>]` and pops on `[</name>]`; mismatched close emits a diagnostic carrying BOTH spans (E0302) and parsing continues (D-07)"
    - "`[<connect ... via={...} carrying={...} />]` produces a comp_connect Node with the via_expr and carrying_expr slots populated by ObjectLiteral payloads (D-09)"
    - "Inside `via={...}` and `carrying={...}` the lexer is in .dealx_expr_brace mode and the full deal.ebnf expression production is accepted (D-08); recursion back into .dealx_tag is implicit via the parser call stack (RESEARCH Pitfall 7)"
    - "Each of the 43 dealx.ebnf productions has ≥1 fixture exercising it (either in the 4 showcase files or in synthetic coverage tests)"
  artifacts:
    - path: "deal/src/parser_dealx.zig"
      provides: "Composition parser implementing all 43 dealx.ebnf productions with stack-based tag balancing and inline {...} attribute parsing"
      contains: "parseFile"
    - path: "deal/src/ast.zig"
      provides: "comp_* payload bodies are now real (system_block, subsystem_block, component_instance, comp_connect, expose_tag, traceability_block, allocate_tag, satisfy_block, validate_tag) — the NodeKind enum is unchanged from Plan 03"
      contains: "CompConnect"
    - path: "deal/src/json.zig"
      provides: "emitAst handles all comp_* payload variants in the exhaustive switch (extending Plan 03's writer)"
      contains: "comp_connect"
    - path: "deal/src/parser_deal.zig"
      provides: "Parser struct (already shipped in Plan 03 with current_mode field + enterMode/restoreMode/pushTag/popTag stubs per Warning #5 fix); Plan 04 fills the pushTag/popTag bodies with actual stack push/pop + E0301/E0302/E0303 emission. The enterMode/restoreMode bodies (already shipped in Plan 03) are unchanged here — they correctly invalidate the lookahead cache for the new mode transitions."
    - path: "deal/tests/unit/parser_dealx_tag_balance.zig"
      provides: "Test cases proving E0302 fires with both spans on mismatched close and parsing continues"
    - path: "deal/tests/unit/parser_dealx_connect_via.zig"
      provides: "Test case proving comp_connect.via_expr is populated for `[<connect via={Foo{a:1}} />]`"
    - path: "deal/tests/unit/parser_dealx_snapshot.zig"
      provides: "Snapshot test for all 4 .dealx showcase files; UPDATE_SNAPSHOTS=1 regenerates"
    - path: "deal/tests/unit/parser_dealx_coverage.zig"
      provides: "43-production coverage tally for dealx.ebnf"
  key_links:
    - from: "deal/src/parser_dealx.zig"
      to: "deal/src/lexer.zig"
      via: "Lexer.next(.dealx_outer) at file top-level; .dealx_tag inside tag bodies; .dealx_expr_brace inside {...}"
      pattern: "lexer\\.next\\(.*\\.dealx_(outer|tag|expr_brace)\\)"
    - from: "deal/src/parser_dealx.zig"
      to: "deal/src/expr.zig"
      via: "Inside {...} (e.g. via={...}), parseExpression is called via parser_deal.expr to evaluate the inline object/expression"
      pattern: "expr\\.parseExpression"
    - from: "deal/src/parser_dealx.zig"
      to: "deal/src/diagnostics.zig"
      via: "E0301 (unmatched close), E0302 (mismatched close with both spans), E0303 (nesting too deep) per D-16 ranges"
      pattern: "E030[123]"
    - from: "deal/src/parser_deal.zig"
      to: "deal/src/parser_dealx.zig"
      via: "Shared OpenTag type lives in parser_dealx; parser_deal imports it so the Parser struct can hold the unused stack field — keeps both modes using a single Parser type"
      pattern: "parser_dealx\\.OpenTag"
---

<objective>
Implement the composition parser for the 43 `dealx.ebnf` productions with parser-owned open-tag stack, stack-based tag balancing, and inline `{...}` attribute bodies that re-enter the deal expression production (D-08). Wire it through the C ABI so `deal_parse` on any `.dealx` source produces a real AST and `deal_ast_json` emits the D-04 schema-versioned JSON. Achieve byte-stable AST snapshots for all 4 `.dealx` showcase files.

Purpose: `.dealx` mode is the half of the compiler that the parser implementation guide identifies as the hardest parse point (CONTEXT §domain line 15; RESEARCH §"Open Questions" — the inline `{...}` mode-switch is RESEARCH Pitfall 7). Plan 03's `.deal` parser handles 87 productions of definition syntax; this plan handles the 43 productions of composition syntax that USE those definitions to assemble systems. After Plan 04, all 19 showcase files parse end-to-end through the C ABI.

Output: 4 committed .dealx AST snapshots; tag-balance and connect-via unit tests green; production coverage for all 43 dealx.ebnf productions; the C ABI's `deal_ast_json` returns a non-null JSON tree for any of the 4 .dealx files via the Rust FFI harness.
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
@deal/.planning/phases/01-zig-compiler-core/01-02-SUMMARY.md
@deal/.planning/phases/01-zig-compiler-core/01-03-SUMMARY.md
@spec/grammar/dealx.ebnf
@spec/grammar/deal.ebnf
@spec/grammar/lexical.ebnf
@spec/grammar/tmp-references/deal-parser-implementation-guide.html
@deal/src/ast.zig
@deal/src/lexer.zig
@deal/src/parser_deal.zig
@deal/src/parser_dealx.zig
@deal/src/parser.zig
@deal/src/expr.zig
@deal/src/json.zig

<interfaces>
From deal/src/parser_deal.zig (Plan 03 — Plan 03 already ships these as stubs; Plan 04 fills the bodies):
- `pub const Parser = struct { arena: std.mem.Allocator, lexer: lexer.Lexer, peeked: ?lexer.Token = null, diagnostics: *std.ArrayList(diagnostics.Diagnostic), open_tags: std.ArrayList(parser_dealx.OpenTag) = .empty, source: []const u8, current_mode: lexer.Mode = .deal_def, pub fn peek(self) Token, pub fn advance(self) Token, pub fn expect(self, tag) !Token, fn makeNode(...) !*ast.Node, pub fn enterMode(self, m) lexer.Mode, pub fn restoreMode(self, prev) void, pub fn pushTag(self, name, span) void, pub fn popTag(self, name, span) void };`
- Plan 03 ships `current_mode` field plus no-op `pushTag`/`popTag` stubs and a working `enterMode`/`restoreMode` that already invalidates the lookahead cache. Plan 04 ONLY needs to (a) fill `pushTag`/`popTag` bodies with actual stack push/pop and E0301/E0302/E0303/E0304 emission, and (b) drive the mode transitions from parser_dealx callsites. The Parser struct's surface does NOT grow in Plan 04 — Plan 03 already exposes everything parser_dealx needs.

From deal/src/ast.zig (Plan 03 — extend payload bodies; NodeKind enum unchanged):
- `pub const CompConnect = struct { from_expr: ?*Node = null, to_expr: ?*Node = null, via_expr: ?*Node = null, carrying_expr: ?*Node = null, other_props: []*Node = &.{} };` (D-09 — typed slots)
- `pub const ObjectLiteral = struct { fields: []ObjectField };` (Plan 03 placeholder — fully populated here for via={...} payloads)
- `pub const SystemBlock = struct { name: []const u8, props: []*Node, children: []*Node, doc: ?*Node = null };` — and analogs for subsystem_block, traceability_block, satisfy_block, validate_tag.
- Placeholder: pub const ExposeTag = struct { ... };  pub const AllocateTag = struct { ... };  pub const ComponentInstance = struct { ... };

From deal/src/expr.zig (Plan 03 — unchanged):
- `pub fn parseExpression(parser: *Parser, min_bp: u8) !*ast.Node;`

From deal/src/parser.zig (Plan 03):
- Dispatcher: `pub fn parseFile(handle) !?*ast.Node { return switch (handle.mode) { .deal => parser_deal.parseFile(handle), .dealx => parser_dealx.parseFile(handle) }; }`

From deal/src/json.zig (Plan 03):
- Hand-rolled emitAst with exhaustive switch over Payload — Plan 04 adds the comp_* arms (Plan 03 left them as TODO/empty-string placeholders).
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 4.1a: Open-tag stack + Parser struct activation + pair-tag dispatch skeleton</name>
  <read_first>
    - spec/grammar/dealx.ebnf — read all 897 lines once. For Task 4.1a focus only on the file-structure productions (DealxFile, package, import, export) and the pair-tag pattern (`[<system>] ... [</system>]`, `[<subsystem>] ... [</subsystem>]`).
    - deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md (D-06 four modes; D-07 parser-owned open-tag stack with recover-on-mismatch; D-18 alphabetical-field JSON invariant)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Pattern 4: Composition parser with parser-owned open-tag stack" (lines 383-426 — pushTag/popTag with E0301 vs E0302), §"Pitfall 1: Two lexers" (lines 558-566)
    - deal/.planning/phases/01-zig-compiler-core/01-03-SUMMARY.md — Plan 03 SUMMARY for the Parser struct stub layout that Plan 04 is filling in
    - deal/src/lexer.zig (Plan 02 — confirm tag_open, tag_close_open, tag_self_close, tag_close are emitted only in .dealx_outer / .dealx_tag modes)
    - deal/src/parser_deal.zig (Plan 03 — Parser struct with current_mode + enterMode/restoreMode/pushTag/popTag STUBS shipped per Plan 03 Task 3.2 Warning #5 fix; this task fills the pushTag/popTag bodies)
  </read_first>
  <files>
    deal/src/parser_dealx.zig,
    deal/src/parser_deal.zig,
    deal/src/parser.zig
  </files>
  <action>
1. In `deal/src/parser_dealx.zig`, declare the tag-stack type at module scope:

   ```
   pub const OpenTag = struct { name: []const u8, span: ast.Span };
   pub const MAX_TAG_DEPTH: u32 = 1024;  // RESEARCH line 1006 threat T-04-01 DoS bound
   ```

   `OpenTag` MUST be `pub` because Plan 03's Parser struct already forward-imports it via `open_tags: std.ArrayList(parser_dealx.OpenTag) = .empty` (per the Warning #5 stub shipped in Plan 03).

2. In `deal/src/parser_deal.zig`, fill in the bodies of the previously-no-op `pushTag` / `popTag` methods (Plan 03 shipped them as `_ = self; _ = name; _ = span;` no-ops per the Warning #5 fix). The new bodies follow RESEARCH §Pattern 4 lines 398-423:

   - `pushTag(name, span)`: if `self.open_tags.items.len >= parser_dealx.MAX_TAG_DEPTH`, emit E0303 (`diagnostics.Codes.e_nesting_too_deep_tag` once that constant lands in Plan 05; for Plan 04 use the literal string `"E0303"`) at the current position and return WITHOUT pushing. Else `try self.open_tags.append(self.arena, .{ .name = name, .span = span });`.
   - `popTag(expected_name, close_span)`: if stack is empty, emit E0301 "unmatched composition close tag" at close_span and return (nothing to pop). Else `const top = self.open_tags.pop().?;` then if `!std.mem.eql(u8, top.name, expected_name)`, emit E0302 "mismatched composition close tag" with primary span = close_span, secondary span = top.span with label "opened here" (RESEARCH §Example E lines 843-851). The pop happens BEFORE the eql check so we always advance the stack (D-07: "pop anyway to continue").

   `enterMode` / `restoreMode` keep the bodies already shipped in Plan 03 — they correctly invalidate the lookahead cache and that's all Plan 04 needs from them.

3. In `deal/src/parser_dealx.zig`, implement the file-structure entry and the minimal pair-tag skeleton needed to make case A pass:

   - `pub fn parseFile(handle: *lib.DealHandle) !?*ast.Node` — creates a Parser identical to parser_deal's (same arena, same lexer, same diagnostics); sets `parser.current_mode = .deal_def` initially (package/import/export use `.deal_def`-style tokens); calls `parseDealxFile(&parser)`; at end of file, checks `parser.open_tags.items.len > 0` and emits E0304 for each remaining open tag at its open span.
   - `fn parseDealxFile(parser: *Parser) !*ast.Node` — after consuming package/import/export (which can reuse `parser_deal.parsePackageDecl` etc.), switches `parser.current_mode = .dealx_outer` and loops:
     - peek `tag_open` → call `parseSystemContent` (the dispatcher in Task 4.1b, but for Task 4.1a only `system` and `subsystem` need to dispatch).
     - peek `kw_package` / `kw_import` / `kw_export` → reuse parser_deal helpers.
     - peek `eof` → break.
   - `fn parseSystemContent(parser: *Parser) !*ast.Node` — peek `tag_open`, advance, peek the ident; for THIS task implement only the `"system"` and `"subsystem"` branches (call `parsePairTag`); other tag names fall through to a TODO panic (`@panic("Task 4.1b dispatch table not yet implemented")`) — case A's smoke test only triggers `"system"` / `"subsystem"`.
   - `fn parsePairTag(parser: *Parser, name: []const u8, kind: ast.NodeKind) !*ast.Node` — call `parser.pushTag(name, open_span)`; consume the `tag_close` ending the opening tag; loop reading inner content (for Task 4.1a, only nested `tag_open` for system/subsystem children — NO attributes yet, NO components, NO connect/satisfy/etc.); on `tag_close_open`, advance, consume the inner ident, expect `tag_close`, then call `parser.popTag(inner_name, close_span)`; return the assembled Node with `SystemBlock { name, props: &.{}, children: list, doc: null }` (props empty until 4.1b adds parseAttributes).
   - Update `deal/src/parser.zig` so the `.dealx` arm of the dispatcher actually calls `parser_dealx.parseFile` (no longer a stub returning null).

4. Smoke-test case A: source = `"package model;
[<system Vehicle>]
  [<subsystem Body>][</subsystem>]
[</system>]"`. Expected result: non-null root of kind `.dealx_file`; one definition of kind `.system_block` named "Vehicle" with one child of kind `.subsystem_block` named "Body"; zero diagnostics; `parser.open_tags.items.len == 0` at end of file.

   This case has NO attributes and NO inner components — that's deliberate. Task 4.1b adds attribute parsing; Task 4.1c adds composite blocks. Each subtask is independently verifiable.
  </action>
  <acceptance_criteria>
    - deal/src/parser_dealx.zig contains `pub const OpenTag`, `pub const MAX_TAG_DEPTH`, `pub fn parseFile`, `parseDealxFile`, `parseSystemContent`, `parsePairTag` (grep for these literal names)
    - deal/src/parser_dealx.zig contains literal string `MAX_TAG_DEPTH` and value `1024`
    - deal/src/parser_dealx.zig contains references to `"E0301"`, `"E0302"`, `"E0303"`, `"E0304"` (or `diagnostics.Codes.e_*` constants once Plan 05 introduces them — for Plan 04 the literal strings are acceptable)
    - deal/src/parser_deal.zig's `pushTag` and `popTag` bodies are non-empty (no longer `_ = self; _ = name; _ = span;` no-ops); both reference `self.open_tags.append` (push) or `self.open_tags.pop` (pop)
    - deal/src/parser.zig dispatches on `.dealx` mode by calling `parser_dealx.parseFile` (not a stub)
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_dealx.smoke 2>&1 — exits 0; case A passes (one `system Vehicle` containing one `subsystem Body`, no attrs, no inner components)</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_deal 2>&1 — exits 0 (Plan 03 tests still pass after pushTag/popTag bodies land)</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_dealx.smoke 2>&1 | tail -5</automated>
  </verify>
  <done>
    Open-tag stack with MAX_TAG_DEPTH=1024 bound; pushTag/popTag bodies filled with E0301/E0302/E0303 emission; parser_dealx.parseFile + parseDealxFile + minimal parseSystemContent + parsePairTag implement the case-A skeleton: a `system Vehicle` block containing a `subsystem Body` child, no attributes, no inner components. parser.zig dispatcher now calls parser_dealx for .dealx-mode parses. Plan 03 .deal tests still green.
  </done>
</task>

<task type="auto">
  <name>Task 4.1b: Attribute parser + first-class comp_connect + simple self-closing tags</name>
  <read_first>
    - spec/grammar/dealx.ebnf — focus on the attribute syntax (`name=value` inside `[<tag ... />]`) and the `[<connect>]` / `[<expose>]` / `[<allocate>]` / `[<component>]` self-closing tag forms.
    - spec/grammar/deal.ebnf §19 — re-read the expression production; inside `via={...}` and `carrying={...}` the parser must accept the full expression grammar (D-08).
    - deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md (D-08 inline {...} accepts full deal expression; D-09 first-class comp_connect with typed slots)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Pitfall 7: .dealx {...} re-entering" (lines 620-628 — the mode-switch boundary)
    - deal/src/parser_dealx.zig (Task 4.1a — extend; do not rewrite)
    - deal/src/expr.zig (Plan 03 — `expr.parseExpression(parser, 0)` is the entry for `{...}` content)
    - deal/src/ast.zig (Plan 03 — extend ONLY the CompConnect slots in this subtask; SystemBlock/etc. payload expansion is Task 4.1c)
  </read_first>
  <files>
    deal/src/parser_dealx.zig,
    deal/src/ast.zig
  </files>
  <action>
1. In `deal/src/ast.zig`, fill in the `CompConnect` payload struct (Plan 03 shipped it as a placeholder with the slot names). Confirm:

   ```
   pub const CompConnect = struct {
       from_expr: ?*Node = null,
       to_expr: ?*Node = null,
       via_expr: ?*Node = null,
       carrying_expr: ?*Node = null,
       other_props: []*Node = &.{},
   };
   ```

   The slot NAMES are locked by D-09; the json.zig emitter (extended in Task 4.1c) writes them in alphabetical order per D-18: `carrying`, `from`, `other_props`, `to`, `via`. Do not rename or reorder.

2. In `deal/src/parser_dealx.zig`, add the attribute parser and the D-08 mode-switch:

   - `fn parseAttributes(parser: *Parser) !std.ArrayList(ast.ObjectField)`:
     - Loop until `tag_close` or `tag_self_close`:
       - Consume `ident` (attribute name) → `name`.
       - Expect `eq`.
       - Peek the next token:
         - `string_literal` / `int_literal` / `real_literal` / `template_head` / `kw_true` / `kw_false` → consume as a simple PropValue node; pack into an `ObjectField { key=name, value=Node(literal), span=... }`.
         - `l_brace` → this is the D-08 inline `{...}` block. **Use the explicit save-and-restore pattern below — NOT `defer parser.restoreMode(prev);`** (per threat T-04-03 in this plan's threat model, defer is forbidden for mode restoration because the lookahead-cache invalidation must be synchronized with the actual restore call, not delayed until function exit).

           ```
           const prev = parser.enterMode(.dealx_expr_brace);
           errdefer parser.restoreMode(prev);    // safe on error path (see invariant below)
           _ = try parser.expect(.l_brace);
           const result = try parseInlineObjectOrExpr(parser);
           _ = try parser.expect(.r_brace);
           parser.restoreMode(prev);             // explicit restore on the success path
           // result is the ObjectField value
           ```

           **Invariant (document inline in code comment citing T-04-03):** `errdefer parser.restoreMode(prev)` is SAFE on the error path because:
             (a) `restoreMode` invalidates `self.peeked` to null, so any pre-lexed token under `.dealx_expr_brace` is discarded.
             (b) The next call after restore re-lexes under the restored mode.
             (c) Diagnostic emission inside `parseInlineObjectOrExpr` happens BEFORE the errdefer fires; no diagnostic ever runs while the cache is stale.
             (d) Using a plain `defer` (always-runs) is forbidden because it would suppress the success-path explicit restore at the wrong sequence point.
         - Anything else → emit E0103 (`"expected attribute value"`) with primary span at peek; advance one token; continue the loop (Plan 05 expands this with full sync logic).
     - Return the list of ObjectFields.

3. Implement the first-class `comp_connect` parser (D-09):

   - `fn parseConnect(parser: *Parser) !*ast.Node`:
     - Read attributes via `parseAttributes`. (parseConnect is a SELF-CLOSING tag — closes with `tag_self_close` not `tag_close_open`/`tag_close_end`.)
     - Build a `CompConnect` payload. For each ObjectField in the returned list:
       - If `field.key == "from"` → set `cc.from_expr = field.value`
       - If `field.key == "to"` → set `cc.to_expr = field.value`
       - If `field.key == "via"` → set `cc.via_expr = field.value`
       - If `field.key == "carrying"` → set `cc.carrying_expr = field.value`
       - Else → append the value Node to `cc.other_props` (use `std.ArrayList(*ast.Node)` accumulator passed via the arena)
     - Return Node `{ .kind = .comp_connect, .span = [tag_open_span.start, tag_self_close_span.end], .payload = .{ .comp_connect = cc } }`.

4. Implement minimal self-closing parsers for the other simple tag names:

   - `fn parseExposeTag(parser: *Parser) !*ast.Node` — reads attrs; builds an `ExposeTag` payload with `port_path` (the `port` attribute value if present), `as_alias` (the `as` attribute), and any other_props.
   - `fn parseAllocateTag(parser: *Parser) !*ast.Node` — reads attrs; builds an `AllocateTag` payload with `from_path` and `to_path`.
   - `fn parseComponentInstance(parser: *Parser) !*ast.Node` — reads attrs; builds a `ComponentInstance` payload with `type_name` (from the tag name itself, not an attribute) and any prop attributes.

5. Extend `parseInlineObjectOrExpr(parser)`:
   - At entry, `parser.current_mode == .dealx_expr_brace` (caller already called `enterMode`) and the opening `{` has just been consumed by the caller.
   - Peek: if `ident` followed by `colon` → parse an `ObjectLiteral`. Loop: parse `ident : <expr> (,|EOF or })` until `r_brace` (do NOT consume the `r_brace` — caller handles it).
   - Otherwise → raw expression. Call `expr.parseExpression(parser, 0)`.
   - **CRITICAL (RESEARCH Pitfall 7):** inside this function, the expression parser may encounter further `{` tokens (e.g. `via={Outer { inner: {nested:1} }}`). The PARSER stack handles this naturally — when parseInlineObjectOrExpr recurses (via parseExpression → parsePrimary recognizing an ObjectLiteral primary), the current_mode stays `.dealx_expr_brace` because it's never restored mid-recursion. The mode pops back to `.dealx_tag` ONLY when this parseInlineObjectOrExpr's outer caller's `parser.restoreMode(prev)` runs (item 2 above). Document this invariant in a code comment citing Pitfall 7 AND the T-04-03 errdefer pattern from item 2.

6. Extend `parseSystemContent` (the dispatcher from Task 4.1a) to dispatch all self-closing tag names: `"connect" → parseConnect`, `"expose" → parseExposeTag`, `"allocate" → parseAllocateTag`, default (other IDENT) → `parseComponentInstance`. Pair-tag names (`"system"`, `"subsystem"`) continue to dispatch to `parsePairTag` per Task 4.1a. The pair-tags `"satisfy"`, `"traceability"`, `"validate"` remain TODO panics — Task 4.1c fills them in.

7. Verification case (Task 4.1b's specific gate): parse `"[<connect from='a' to='b' via={CANHarness { connectorType: \"Molex MX150\" }} />]"`:
   - Root has one `comp_connect` node.
   - `cc.from_expr` non-null, points at StringLiteral "a".
   - `cc.to_expr` non-null, points at "b".
   - `cc.via_expr` non-null, points at a Call("CANHarness", [ObjectLiteral{connectorType:"Molex MX150"}]) OR an ObjectLiteral-via-Call shape per Plan 03 SUMMARY's resolution of "IDENT { ... }" — both shapes are acceptable; the test asserts cc.via_expr is non-null and has a recursive structure reaching the inner StringLiteral.
   - `cc.carrying_expr == null`; `cc.other_props.len == 0`.
   - Zero diagnostics emitted.
  </action>
  <acceptance_criteria>
    - deal/src/parser_dealx.zig contains `parseAttributes`, `parseConnect`, `parseExposeTag`, `parseAllocateTag`, `parseComponentInstance`, `parseInlineObjectOrExpr` (grep)
    - deal/src/parser_dealx.zig contains the literal pattern `errdefer parser.restoreMode(prev)` AND `parser.restoreMode(prev);` on a separate non-defer line (the explicit success-path restore — verified by grep counting both forms)
    - deal/src/parser_dealx.zig does NOT contain `defer parser.restoreMode` (literal `defer ` followed by the restore call — Warning #8 / T-04-03 forbids it)
    - deal/src/ast.zig's `CompConnect` struct has all four typed slots `from_expr`, `to_expr`, `via_expr`, `carrying_expr` plus `other_props`
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_dealx.connect_via 2>&1 — exits 0; case 1 of `parser_dealx.connect_via` passes (the `via={CANHarness{...}}` case)</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && grep -n 'defer parser.restoreMode' src/parser_dealx.zig 2>&1 | grep -v 'errdefer' | wc -l — produces 0 (no plain `defer parser.restoreMode`)</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && grep -c 'errdefer parser.restoreMode\|parser.restoreMode(prev);' src/parser_dealx.zig — produces ≥ 2 (errdefer on error path AND explicit restore on success path)</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_dealx.connect_via 2>&1 | tail -5</automated>
  </verify>
  <done>
    parseAttributes implements the D-08 mode-switch using the threat-model-correct `enterMode + errdefer restoreMode + explicit success-path restoreMode` pattern (NO plain `defer restoreMode` — T-04-03 invariant documented inline). parseConnect populates typed CompConnect slots per D-09. Self-closing tag dispatch covers connect/expose/allocate/component-instance. The `parser_dealx.connect_via` case 1 gate passes (`[<connect>] from=a to=b via={CANHarness{connectorType:"Molex MX150"}} />]`).
  </done>
</task>

<task type="auto">
  <name>Task 4.1c: Composite blocks (satisfy/traceability/validate) + AST payload expansion + exhaustive json.zig switch + snapshot byte-stability gate</name>
  <read_first>
    - spec/grammar/dealx.ebnf — focus on the composite-block productions: `[<satisfy>]` body (criteria, evidence, compute, gap, maps), `[<traceability>]` body (relationship list: satisfies, traces, refines, derives), `[<validate>]` props.
    - deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md (D-18 alphabetical-field JSON invariant)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Pitfall 5: std.json.Stringify non-deterministic order" (lines 598-608) — Plan 03 introduced the hand-rolled writer; Task 4.1c extends the exhaustive switch to cover comp_* arms WITHOUT changing the alphabetical-field convention.
    - deal/.planning/phases/01-zig-compiler-core/01-03-SUMMARY.md — Plan 03 SUMMARY for the existing alphabetical-field convention applied to .deal payloads
    - deal/src/parser_dealx.zig (Task 4.1a + 4.1b — extend; do not rewrite)
    - deal/src/ast.zig (Plan 03 placeholders for SystemBlock/SubsystemBlock/TraceabilityBlock/SatisfyBlock/ValidateTag/ComponentInstance/ExposeTag/AllocateTag/ObjectLiteral — this task fills the bodies)
    - deal/src/json.zig (Plan 03 — extend the exhaustive `switch (node.payload)` to add comp_* arms)
  </read_first>
  <files>
    deal/src/parser_dealx.zig,
    deal/src/ast.zig,
    deal/src/json.zig
  </files>
  <action>
1. In `deal/src/ast.zig`, expand every comp_* payload struct from Plan 03's `pub const X = struct {};` placeholder to its full shape. Field order is internal-only (Zig field order has no JSON impact when the emitter writes fields alphabetically per D-18); use the canonical-clarity order below:

   - `pub const SystemBlock = struct { name: []const u8, props: []*ast.Node = &.{}, children: []*ast.Node = &.{}, doc: ?*ast.Node = null };`
   - `pub const SubsystemBlock = SystemBlock;` (alias if structurally identical; else duplicate)
   - `pub const ComponentInstance = struct { type_name: []const u8, alias: ?[]const u8 = null, props: []*ast.Node = &.{} };`
   - `pub const ExposeTag = struct { port_path: []const u8, as_alias: ?[]const u8 = null, props: []*ast.Node = &.{} };`
   - `pub const AllocateTag = struct { from_path: []const u8, to_path: []const u8, props: []*ast.Node = &.{} };`
   - `pub const TraceKind = enum { satisfies, traces, refines, derives };`  (extend per dealx.ebnf — if more relationships exist, add them; the locked set is the four listed)
   - `pub const TraceLink = struct { kind: TraceKind, source: *ast.Node, target: *ast.Node, span: ast.Span };`
   - `pub const TraceabilityBlock = struct { relationships: []TraceLink = &.{} };`
   - `pub const SatisfyBlock = struct { name: []const u8, criteria: ?*ast.Node = null, evidence: ?*ast.Node = null, compute: ?*ast.Node = null, gap: ?*ast.Node = null };`
   - `pub const ValidateTag = struct { props: []*ast.Node = &.{} };`
   - `pub const ObjectLiteral = struct { fields: []ObjectField = &.{} };`  (Plan 03 placeholder fully populated here)
   - `pub const ObjectField = struct { key: []const u8, value: *ast.Node, span: ast.Span };` (already declared in Plan 03; confirm present)

2. In `deal/src/parser_dealx.zig`, implement the composite-block parsers:

   - `fn parseSatisfyBlock(parser: *Parser) !*ast.Node`: consume opening `[<satisfy name=...>]`; push tag; loop reading sub-blocks `criteria`, `evidence`, `compute`, `gap` (each is an inner pair-tag — read dealx.ebnf for the exact body grammar; each sub-block contains an expression or an inline object literal that's parsed via the same D-08 mode-switch from Task 4.1b); expect `[</satisfy>]`; pop tag. Return Node with kind `.satisfy_block` and the populated SatisfyBlock payload.
   - `fn parseTraceabilityBlock(parser: *Parser) !*ast.Node`: consume opening `[<traceability>]`; push tag; loop reading TraceLink rows (each row is `IDENT <<relationship>> IDENT` or the equivalent dealx.ebnf syntax — use the kind enum for `satisfies`/`traces`/`refines`/`derives`); expect `[</traceability>]`; pop tag.
   - `fn parseValidateTag(parser: *Parser) !*ast.Node`: handle as a self-closing tag (similar to parseConnect from Task 4.1b) with attribute parsing.
   - Update `parseSystemContent` dispatcher to wire these in: `"satisfy" → parseSatisfyBlock`, `"traceability" → parseTraceabilityBlock`, `"validate" → parseValidateTag`. Remove the TODO panics from Task 4.1a.

3. In `deal/src/json.zig`, extend the exhaustive `switch (node.payload)` block from Plan 03 to add one arm per comp_* variant. Field-emission order per arm is ALPHABETICAL (D-18). Required arms (with the alphabetical key order each MUST emit):

   - `.system_block => |sb| { write keys in order: "children", "doc", "name", "props" }`
   - `.subsystem_block => |sb| { write: "children", "doc", "name", "props" }`
   - `.component_instance => |ci| { write: "alias", "props", "type_name" }`
   - `.expose_tag => |et| { write: "as_alias", "port_path", "props" }`
   - `.allocate_tag => |at| { write: "from_path", "props", "to_path" }`
   - `.comp_connect => |cc| { write: "carrying", "from", "other_props", "to", "via" }` (locked by D-09)
   - `.traceability_block => |tb| { write: "relationships" }` — each TraceLink emits `{ "kind": "satisfies"|..., "source": {...}, "span": [s,e], "target": {...} }` in alphabetical order (`kind`, `source`, `span`, `target`).
   - `.satisfy_block => |sb| { write: "compute", "criteria", "evidence", "gap", "name" }`
   - `.validate_tag => |vt| { write: "props" }`
   - `.object_literal => |ol| { write: "fields" }` — each ObjectField emits `{ "key", "span", "value" }` alphabetical.

   The Zig 0.16.0 compiler will FAIL to compile if any Payload variant is missing — this is the exhaustiveness guard from RESEARCH line 535. Use that as the structural check: if `zig build` exits 0, every comp_* kind is wired.

4. Verification: the SNAPSHOT BYTE-STABILITY gate.

   - All 4 `.dealx` showcase files MUST now parse cleanly through `parseFile` and produce non-null roots with zero diagnostics. Snapshot test (`parser_dealx.snapshot`) in Task 4.2 will assert byte-equality against the committed snapshots.
   - **D-18 invariant — the 15 `.deal` snapshots committed in Plan 03 MUST remain byte-identical after Plan 04 lands.** This is the Warning #6 fix: alphabetical-field emission is a phase-level invariant; adding `comp_*` arms to the json.zig switch does NOT reorder existing `.deal` payload arms. Acceptance gate below verifies this with `git diff` against `HEAD` for every Plan 03 snapshot.

5. Smoke confirmation case: parse `spec/examples/showcase/model/traceability.dealx` end-to-end; assert non-null root; assert ≥1 `.traceability_block` node in the tree; assert each TraceLink has a non-null `source` and `target` and `kind` in {satisfies, traces, refines, derives}.
  </action>
  <acceptance_criteria>
    - deal/src/ast.zig defines full payload bodies for SystemBlock, SubsystemBlock (or alias), ComponentInstance, ExposeTag, AllocateTag, TraceabilityBlock (with TraceLink + TraceKind), SatisfyBlock, ValidateTag, ObjectLiteral (no longer `= struct {};` placeholders — verified by grep for the field names like `relationships`, `criteria`, `evidence`, `from_path`, `to_path`, `port_path`)
    - deal/src/parser_dealx.zig contains `parseSatisfyBlock`, `parseTraceabilityBlock`, `parseValidateTag` (grep)
    - deal/src/parser_dealx.zig's `parseSystemContent` dispatcher contains literal strings `"satisfy"`, `"traceability"`, `"validate"` AND no longer contains `@panic` for these names
    - deal/src/json.zig switch arm exists for every comp_* NodeKind variant — Zig 0.16.0 exhaustive-switch compile would fail otherwise; verified by `zig build` exit 0 AND grep for `\.satisfy_block =>`, `\.traceability_block =>`, `\.comp_connect =>`, `\.system_block =>`, `\.subsystem_block =>`, `\.component_instance =>`, `\.expose_tag =>`, `\.allocate_tag =>`, `\.validate_tag =>`, `\.object_literal =>` (10 arms total)
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_dealx.snapshot 2>&1 — exits 0 (all 4 .dealx snapshots byte-match — the gate criterion)</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && find tests/snapshots/ast -name 'showcase__packages__*.deal.json' | xargs -I{} sh -c 'diff <(cat {}) <(git show HEAD:{}) || true' 2>&1 | grep -c '^[<>]' — produces 0 lines (the 15 .deal snapshots committed in Plan 03 are byte-identical after Plan 04 lands; D-18 invariant verified)</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_deal.snapshot 2>&1 — exits 0 (Plan 03's `.deal` snapshot test still green; emission order unchanged)</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test 2>&1 | tail -10</automated>
  </verify>
  <done>
    All 9 comp_* AST payload structs fully populated. parseSatisfyBlock / parseTraceabilityBlock / parseValidateTag implemented; parseSystemContent dispatch table complete (no TODO panics). json.zig exhaustive switch covers every comp_* variant emitting fields in alphabetical order per D-18. **THE GATE:** all 4 `.dealx` snapshots byte-match committed snapshots AND all 15 `.deal` snapshots remain byte-identical to Plan 03's commit (proving D-18 invariant). Phase 1 ROADMAP success criterion #3 met at the AST-shape level (Task 4.2 closes the loop with tag-balance recovery + connect_via + coverage tests).
  </done>
</task>

<task type="auto">
  <name>Task 4.2: Tag-balance + connect-via unit tests + 4 .dealx showcase snapshots + 43-production coverage</name>
  <read_first>
    - spec/examples/showcase/model/vehicle.dealx — the canonical pair-tag + connect-via case
    - spec/examples/showcase/model/traceability.dealx — the canonical traceability_block / satisfy_block case
    - spec/examples/showcase/model/variants/sedan.dealx, performance.dealx — variant compositions
    - spec/grammar/dealx.ebnf — for the 43-production coverage map
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Phase Requirements → Test Map" lines 950-953 (parser_dealx.snapshot, parser_dealx.tag_balance, parser_dealx.connect_via, coverage)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Pattern 4" line 419-423 (the mismatched-close test pattern with both spans)
    - deal/tests/unit/parser_deal_snapshot.zig (Plan 03 — mirror the UPDATE_SNAPSHOTS=1 pattern)
    - deal/src/parser_dealx.zig (just written)
  </read_first>
  <files>
    deal/tests/unit/parser_dealx_smoke.zig,
    deal/tests/unit/parser_dealx_tag_balance.zig,
    deal/tests/unit/parser_dealx_connect_via.zig,
    deal/tests/unit/parser_dealx_snapshot.zig,
    deal/tests/unit/parser_dealx_coverage.zig,
    deal/tests/snapshots/ast/showcase__model__vehicle.dealx.json,
    deal/tests/snapshots/ast/showcase__model__traceability.dealx.json,
    deal/tests/snapshots/ast/showcase__model__variants__sedan.dealx.json,
    deal/tests/snapshots/ast/showcase__model__variants__performance.dealx.json,
    deal/build.zig
  </files>
  <action>
1. Create `deal/tests/unit/parser_dealx_smoke.zig` with `test "parser_dealx.smoke"`:
   - Parse `"package model;\n[<system Vehicle>]\n[</system>]"` via the internal Zig API (build a DealHandle stub or call parser_dealx.parseFile directly).
   - Assert: non-null root; root.kind == .dealx_file; one definition of kind .system_block named "Vehicle"; no diagnostics.

2. Create `deal/tests/unit/parser_dealx_tag_balance.zig` with `test "parser_dealx.tag_balance"`:
   - Case A — clean match: `"[<system Foo>][</system>]"` → no diagnostics; system_block named "Foo".
   - Case B — mismatched close (E0302): `"[<system Foo>][</subsystem>]"` → exactly one diagnostic with code "E0302"; primary span covers the `[</subsystem>]` close; secondary_spans has one entry whose label contains "opened here" and whose span covers the `[<system Foo>]` open. Per D-07, parsing continues — the AST should still produce a SystemBlock node (possibly with empty children).
   - Case C — unmatched close (E0301): `"[</orphan>]"` → exactly one diagnostic with code "E0301" at the orphan-close span. No system_block produced (or an empty file with the diagnostic).
   - Case D — depth-bound (E0303): build a source with 1025 nested `[<lvl>]` opens (Zig comptime string-builder); assert E0303 fires at depth 1024 and parsing does not panic. (This validates RESEARCH §Threat T-DoS line 1006.)
   - Case E — unclosed at EOF (E0304): `"[<system Foo>]"` (no close) → exactly one E0304 diagnostic referring to the open-tag span.

3. Create `deal/tests/unit/parser_dealx_connect_via.zig` with `test "parser_dealx.connect_via"`:
   - Parse `"[<connect from='a' to='b' via={CANHarness { connectorType: \"Molex MX150\" }} />]"`.
   - Assert: root has one connect node; node.kind == .comp_connect; cc = node.payload.comp_connect; `cc.from_expr` is non-null and points at a StringLiteral "a"; `cc.to_expr` non-null and points at "b"; `cc.via_expr` is non-null and its kind is `.call` or `.object_literal` (depending on how Plan 03 decided to handle `IDENT { ... }` — per Plan 03 SUMMARY); if it's a Call to "CANHarness" then its args[0] is an ObjectLiteral with one field `connectorType` mapped to StringLiteral "Molex MX150"; `cc.carrying_expr` is null; `cc.other_props.len` is 0.
   - Second case — both via and carrying: `"[<connect from='a' to='b' via={X} carrying={Y} />]"` → both slots populated.
   - Third case — nested inline objects (RESEARCH Pitfall 7): `"[<connect via={Outer { inner: {nested: 1} }} />]"` → via_expr is non-null; recursive structure has the expected three-level shape; no panic; no extraneous diagnostics.

4. Create `deal/tests/unit/parser_dealx_snapshot.zig` with `test "parser_dealx.snapshot"`:
   - Enumerate the 4 .dealx showcase paths (`model/vehicle.dealx`, `model/traceability.dealx`, `model/variants/sedan.dealx`, `model/variants/performance.dealx`).
   - For each: read, parse via `parser_dealx.parseFile`, emit JSON via `json.emitAst`, compare to committed snapshot at `tests/snapshots/ast/showcase__<path>.json` (path slashes → double-underscore, same convention as Plans 02/03).
   - UPDATE_SNAPSHOTS=1 regenerates. On mismatch, write `.actual` and fail.
   - Additional assertion: parsing each file produces a non-null root AND zero diagnostics (the 4 .dealx showcase files are well-formed per Phase 0).

5. Create `deal/tests/unit/parser_dealx_coverage.zig` with `test "parser_dealx.coverage"`:
   - Walk the AST of every parsed .dealx showcase file (recursive visit).
   - Required NodeKinds (mapped from the 43 dealx.ebnf productions):
     - File: `dealx_file`
     - Structural: `system_block`, `subsystem_block`, `component_instance`, `expose_tag`, `allocate_tag`
     - Connections: `comp_connect`
     - Traceability: `traceability_block`, `satisfy_block`, `validate_tag`
     - Inline content: `object_literal`, `template_literal` (if .dealx uses interpolation in props)
     - Expressions inside {...}: at least `identifier`, `int_literal`, `string_literal`, `member_access` (covered when via={...} contains an expression)
   - For productions not exercised by the showcase, add synthetic fixtures in this file (e.g. allocate_tag may not appear in the 4 .dealx files — add an inline `"[<system X>][<allocate from='a' to='b'/>][</system>]"` fixture).
   - Assert every required kind has count ≥ 1. Print tally to stderr.

6. Generate the 4 .dealx AST snapshots via `cd deal && UPDATE_SNAPSHOTS=1 zig build test -Dtest-filter=parser_dealx.snapshot`. Visually inspect all 4 (per VALIDATION.md manual-only-verifications row). Confirm:
   - Top-level `{"v":1,"mode":"dealx","filename":"<basename>","root":{"k":"dealx_file",...}}`
   - Every `comp_connect` node has the four typed slots (carrying, from, to, via) emitted in alphabetical order
   - Span coverage: root span starts at 0 and ends at source.len; tag spans correctly bracket their `[<...>]` extent.

7. Commit all 4 snapshots. Wire all 5 new unit-test files into `deal/build.zig` test step.

8. Re-run the C ABI smoke through Rust: `cd deal && cargo test --manifest-path tests/ffi/Cargo.toml` — the existing `ffi_dealx_mode_detection` test (Plan 01) now produces a real AST instead of `"root":null`; the assertion `json.contains("\"mode\":\"dealx\"")` should still pass.
  </action>
  <acceptance_criteria>
    - deal/tests/unit/parser_dealx_smoke.zig contains exactly one `test "parser_dealx.smoke"` block
    - deal/tests/unit/parser_dealx_tag_balance.zig contains exactly one `test "parser_dealx.tag_balance"` block and references E0301, E0302, E0303, E0304
    - deal/tests/unit/parser_dealx_connect_via.zig contains exactly one `test "parser_dealx.connect_via"` block and at least three sub-cases
    - deal/tests/unit/parser_dealx_snapshot.zig contains exactly one `test "parser_dealx.snapshot"` block
    - deal/tests/unit/parser_dealx_coverage.zig contains exactly one `test "parser_dealx.coverage"` block
    - All 4 .dealx AST snapshot files exist under deal/tests/snapshots/ast/
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_dealx.tag_balance 2>&1 — exits 0; all 5 cases (A-E) pass</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_dealx.connect_via 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_dealx.snapshot 2>&1 — exits 0; all 4 snapshots byte-match</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_dealx.coverage 2>&1 — exits 0; every required NodeKind has count ≥ 1</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && ls tests/snapshots/ast/*.dealx.json | wc -l — produces exactly 4</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test 2>&1 — exits 0 (full suite green: Plan 02 lexer + Plan 03 parser_deal + Plan 04 parser_dealx)</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml 2>&1 — exits 0 (FFI tests still pass)</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test 2>&1 | tail -15</automated>
  </verify>
  <done>
    Phase 1 ROADMAP success criterion #3 met: "The 4 .dealx showcase files parse through composition parser covering 43 productions, with stack-based tag balancing and inline via={...}/carrying={...} blocks working." E0301/E0302/E0303/E0304 diagnostic codes are exercised by tag_balance test. D-09 first-class comp_connect typed slots are verified by connect_via test. RESEARCH Pitfall 7 (nested inline {...} mode handling) is covered by case 3 of connect_via.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Tag stack | Caller-provided source can attempt to exhaust the open-tag stack via deeply-nested tags (RESEARCH §Threat T-02). |
| Inline `{...}` mode boundary | The mode-switch at `{` opens a re-entrant deal expression production. Malformed inputs can attempt to escape the brace context. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-04-01 | DoS | parser_dealx open_tags stack depth | mitigate | `MAX_TAG_DEPTH = 1024` (RESEARCH line 1006). Past 1024, E0303 fires and the push is dropped; parsing continues without stack growth. Tested by `parser_dealx.tag_balance` case D. |
| T-04-02 | DoS | Inline {...} brace recursion | mitigate | Brace recursion is bounded by the same OS stack as parseExpression. Plan 05 adds a recursion-depth bound via a counter on the Parser struct. Plan 04 inherits the OS-stack limit. |
| T-04-03 | Tampering | Mode-switch leak (current_mode not restored on early return) | mitigate | `enterMode` returns the previous mode. Every parseInlineObjectOrExpr / parseAttributes path uses the threat-model-correct pattern: `const prev = parser.enterMode(.dealx_expr_brace); errdefer parser.restoreMode(prev); /* parse body */ parser.restoreMode(prev);` — i.e. `errdefer` covers the error path (safe because restoreMode invalidates the stale lookahead cache and any subsequent diagnostic emission runs after the cache is cleared), and an EXPLICIT `restoreMode(prev)` runs on the success path. Plain `defer restoreMode(prev)` is FORBIDDEN (Warning #8) because it conflicts with the success-path explicit restore and creates a double-restore sequence point. Acceptance criteria for Task 4.1b enforce this via grep: `defer parser.restoreMode` (without `err` prefix) must produce 0 matches. |
| T-04-04 | Information Disclosure | Tag-name byte ranges in E0302 diagnostic messages | accept | Diagnostic messages include the literal source slice for the tag name (e.g. "Mismatched tag: opened `[<system>]`, closed `[</subsystem>]`"). This is the documented diagnostic shape. Source bytes are escaped through the JSON emitter's standard escaping (Plan 03). |
| T-04-SC | Tampering | No new external dependencies | accept | This plan uses only Zig 0.16.0 std-lib (no new packages). |
</threat_model>

<verification>
1. `cd /Users/dunnock/projects/deal-lang/deal && zig build` exits 0.
2. `cd /Users/dunnock/projects/deal-lang/deal && zig build test` exits 0 — full Zig suite green (lexer + parser_deal + parser_dealx).
3. `cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_dealx.snapshot` exits 0 — all 4 .dealx snapshots byte-match.
4. `cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_dealx.tag_balance` exits 0 — all 5 cases pass.
5. `cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_dealx.connect_via` exits 0.
6. `cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_dealx.coverage` exits 0.
7. `cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml` exits 0 — Rust harness still works.
8. `ls deal/tests/snapshots/ast/*.json | wc -l` = 19 (15 .deal from Plan 03 + 4 .dealx from this plan).
</verification>

<success_criteria>
- Phase 1 ROADMAP success criterion #3 met: 4 .dealx files parse with stack-based tag balancing and inline via={...}/carrying={...} blocks.
- REQ-phase-1-3-parser-dealx acceptance met: all 4 .dealx showcase files parse; tag balance enforced; inline block parsing for connect/expose works.
- D-06 (four-mode lexer dispatch driven by parser) honored: enterMode/restoreMode + lookahead-cache invalidation per Pitfall 7.
- D-07 (parser-owned open-tag stack with recover-on-mismatch) honored: pop-anyway on E0302 with both spans in the diagnostic.
- D-08 (inside {...} accept full deal expression production) honored: inline object literal AND raw expression both supported.
- D-09 (first-class comp_connect with typed slots) honored: via_expr / carrying_expr are explicit `?*Node` fields, not generic ObjectLiteral attrs.
- All 19 showcase files now produce committed AST JSON snapshots (15 .deal from Plan 03 + 4 .dealx from this plan).
- All Validation Architecture rows for parser_dealx (RESEARCH lines 950-953) green.
</success_criteria>

<output>
Create `.planning/phases/01-zig-compiler-core/01-04-SUMMARY.md` when done, recording:
- Whether `parseInlineObjectOrExpr` distinguishes ObjectLiteral from raw expression by peek-2 (`ident colon`) or by attempting parse-and-recover.
- Whether MAX_TAG_DEPTH = 1024 was kept or revised based on showcase-file observed max depth (the four showcase files probably nest ≤ 4 levels, so 1024 is generous).
- Which dealx.ebnf productions required synthetic fixtures (i.e. were not exercised by the 4 showcase files alone).
- Final layout of Parser.current_mode and how enterMode/restoreMode are paired across the parser_dealx callsites — Plan 05 needs to understand this when adding three-tier recovery (which also manipulates lexer state).
- Whether any of the 4 .dealx files contained syntax that required clarification of dealx.ebnf (RESEARCH assumption A7).
</output>
