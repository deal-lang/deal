---
phase: 01-zig-compiler-core
plan: 03
type: execute
wave: 3
depends_on:
  - 01-02-lexer
files_modified:
  - deal/src/ast.zig
  - deal/src/parser.zig
  - deal/src/parser_deal.zig
  - deal/src/expr.zig
  - deal/src/json.zig
  - deal/src/lib.zig
  - deal/tests/unit/expr_precedence.zig
  - deal/tests/unit/parser_deal_smoke.zig
  - deal/tests/unit/parser_deal_snapshot.zig
  - deal/tests/unit/parser_deal_coverage.zig
  - deal/tests/snapshots/ast/.gitkeep
  - deal/build.zig
autonomous: true
requirements:
  - REQ-phase-1-2-parser-deal

must_haves:
  decisions_implemented:
    - "D-01: Tagged-union AST realized — `Node = struct { kind: NodeKind, span: Span, payload: Payload }` where `Payload = union(NodeKind) { ... }` with one arm per kind; Zig 0.16.0 exhaustive-switch enforcement guards completeness"
    - "D-02: Arena allocator per parse — every `*ast.Node` allocated via `parser.arena.allocator().create(ast.Node)`; nodes never stored outside the arena; `deal_free` releases all in one shot"
    - "D-03: Unified NodeKind (`.deal` half) — `NodeKind` enum extended to ~60 variants covering file/header/imports/exports + 14 element-def kinds + member usages + expression nodes + `.dealx` `comp_*` placeholders (Plan 04 fills payload bodies)"
    - "D-04: AST JSON emission — hand-rolled `json.emitAst` produces `{\"v\":1,\"mode\":\"deal\",\"filename\":\"<name>\",\"root\":{\"k\":\"deal_file\",\"span\":[s,e],...}}` with deterministic top-level key order"
    - "D-05: Zig 0.16.0 idioms — no `callconv(.C)`, no `ArrayList(T).init`, exhaustive switch enforced by compiler"
    - "D-08: Inside `{...}` accept full deal expression production — `expr.parseExpression(parser, 0)` is the entry for every Expression non-terminal (Plan 04 reuses this for `.dealx` inline `via={...}` content)"
    - "D-15: `Span` on every node — `Node` struct carries `span: Span`; verified by `parser_deal.snapshot` JSON containing `\"span\":[s,e]` on every node"
    - "D-17: Statement-level sync interface stub — Parser struct ships `enterMode`/`restoreMode`/`pushTag`/`popTag` method declarations (no-op bodies in Plan 03; Plan 05 ships the full `syncToStatement`/`syncToDefinition` bodies)"
    - "D-18: Alphabetical JSON payload field order established — every `switch (node.payload)` arm in `json.zig` emits fields alphabetically (locked here; Plan 04 must not reorder); verified by `parser_deal.snapshot` byte-stability"
  truths:
    - "All 15 .deal showcase files parse through parser_deal.parseFile producing a non-null ast.Node tree"
    - "All 15 .deal AST JSON snapshots are byte-stable across consecutive `zig build test -Dtest-filter=parser_deal.snapshot` runs"
    - "Pratt expression parser respects the 8-level precedence table from deal.ebnf §19: OR (BP 10) < AND (BP 20) < ==/!= (BP 30) < relational (BP 40) < +/- (BP 50) < */ (BP 60) < unary (BP 70) < .|( postfix (BP 80)"
    - "Each of the 87 deal.ebnf productions has ≥1 fixture exercising it (either in the 15 showcase files or in synthetic coverage tests)"
    - "Per-handle ArenaAllocator owns every Node + every interior []const u8 in the AST; deal_free releases all of it in one shot"
  artifacts:
    - path: "deal/src/ast.zig"
      provides: "Tagged-union Node + Payload covering all .deal NodeKinds (file/header/imports/exports + 14 element-def kinds + member usages + expression nodes per D-01/D-03/D-09)"
      contains: "union(NodeKind)"
    - path: "deal/src/parser_deal.zig"
      provides: "Recursive-descent parser implementing all 87 deal.ebnf productions; uses self.current_mode (always .deal_def in Plan 03) on every lexer call; Parser struct exposes current_mode field + enterMode/restoreMode/pushTag/popTag no-op stubs that Plan 04 fills in"
      contains: "parseFile"
    - path: "deal/src/expr.zig"
      provides: "Pratt parseExpression(min_bp) implementing the 8-level binding-power table"
      contains: "leftBindingPower"
    - path: "deal/src/json.zig"
      provides: "emitAst(allocator, root, mode, filename) producing D-04 hand-rolled deterministic JSON with kind-tagged nodes"
      contains: "emitAst"
    - path: "deal/src/lib.zig"
      provides: "deal_parse now calls parser.parseFile and stores the real ast_root; deal_ast_json emits the real tree via json.emitAst"
    - path: "deal/tests/unit/expr_precedence.zig"
      provides: "Unit tests asserting the 8 precedence levels produce the documented tree shape"
    - path: "deal/tests/unit/parser_deal_snapshot.zig"
      provides: "Snapshot test for all 15 .deal showcase files (UPDATE_SNAPSHOTS=1 generates; subsequent runs assert)"
    - path: "deal/tests/unit/parser_deal_coverage.zig"
      provides: "Production-coverage test exercising the 87 deal.ebnf productions; emits a coverage tally"
  key_links:
    - from: "deal/src/parser_deal.zig"
      to: "deal/src/lexer.zig"
      via: "Lexer.next(.deal_def) / Lexer.peek(.deal_def) on every token consumption"
      pattern: "lexer\\.next\\(.*\\.deal_def\\)"
    - from: "deal/src/parser_deal.zig"
      to: "deal/src/expr.zig"
      via: "calls expr.parseExpression(parser, 0) for every Expression non-terminal"
      pattern: "expr\\.parseExpression"
    - from: "deal/src/lib.zig"
      to: "deal/src/parser.zig"
      via: "deal_parse calls parser.parseFile(handle, handle.mode) to populate ast_root"
      pattern: "parser\\.parseFile"
    - from: "deal/src/expr.zig"
      to: "spec/grammar/deal.ebnf §19"
      via: "Binding-power table mirrors deal.ebnf §19 lines 1549-1606 (Pratt precedence)"
      pattern: "leftBindingPower|rightBindingPower"
---

<objective>
Implement the recursive-descent parser for the 87 `.deal` grammar productions (definitions, members, annotations, imports/exports/header, all expressions via Pratt). Wire it through the C ABI so `deal_parse` on any `.deal` source produces a real AST and `deal_ast_json` emits the D-04 schema-versioned JSON. Achieve byte-stable AST snapshots for all 15 `.deal` showcase files.

Purpose: this is the heart of Phase 1 for the definition mode. The grammar is locked at `0.1.0-draft` (lexical/deal/dealx EBNF). The Pratt precedence table is documented exhaustively in RESEARCH §Pattern 3. The implementation guide drafts a phased build (file structure → element definitions → element bodies → annotations → expressions) which this plan compresses into a single sweep because the grammar is mechanical translation once the Pratt loop is correct.

Output: 15 committed AST JSON snapshots; an expression-precedence unit test green; a production-coverage test green; the C ABI's `deal_ast_json` returns a non-null JSON tree for any of the 15 showcase files when invoked via the Rust FFI harness.
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
@spec/grammar/deal.ebnf
@spec/grammar/lexical.ebnf
@spec/grammar/README.md
@spec/grammar/tmp-references/deal-parser-implementation-guide.html
@deal/src/ast.zig
@deal/src/lexer.zig
@deal/src/parser_deal.zig
@deal/src/parser.zig
@deal/src/expr.zig
@deal/src/json.zig
@deal/src/lib.zig
@deal/build.zig

<interfaces>
From deal/src/lexer.zig (Plan 02 — DO NOT modify; consume as-is):
- `pub const Mode = enum { deal_def, dealx_outer, dealx_tag, dealx_expr_brace };`
- `pub const Tag = enum { ... 80+ variants including kw_part, kw_def, kw_port, ident, int_literal, real_literal, string_literal, template_head, plus, minus, star, slash, eq_eq, bang_eq, lt, gt, lt_eq, gt_eq, eq, dot, comma, semicolon, l_paren, r_paren, l_brace, r_brace, l_bracket, r_bracket, at, kw_and, kw_or, kw_not, eof, ... };`
- `pub const Token = struct { tag: Tag, span: ast.Span };`
- `pub const Lexer = struct { source: []const u8, pos: u32, pub fn init(source: []const u8) Lexer; pub fn next(self: *Lexer, mode: Mode) Token; pub fn peek(self: *Lexer, mode: Mode) Token; };`

From deal/src/ast.zig (Plan 01 stub — this plan extends):
- `pub const Span = extern struct { start: u32, end: u32 };` (KEEP — `extern struct` is required for C ABI)
- `pub const Mode = enum { deal, dealx };` (file-level mode)
- `pub const NodeKind = enum { deal_file, dealx_file };` (this plan extends to ~60 variants)

From deal/src/lib.zig (Plan 01 — this plan modifies):
- `pub const DealHandle = struct { arena: std.heap.ArenaAllocator, ast_root: ?*ast.Node = null, mode: ast.Mode = .deal, source: []const u8, filename: []const u8, diagnostics: std.ArrayList(diagnostics.Diagnostic) = .empty, ... };`
- `pub export fn deal_parse(...) callconv(.c) ?*anyopaque` — currently sets `ast_root = null`; this plan changes it to call `parser.parseFile(handle, handle.mode)` and store the returned root.

From deal/src/diagnostics.zig (Plan 01 — consumed read-only by this plan; Plan 05 expands):
- `pub const Diagnostic = struct { code: []const u8, severity: Severity, message: []const u8, span: ast.Span, ... };`
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 3.1: Extend AST and implement Pratt expression parser</name>
  <read_first>
    - spec/grammar/deal.ebnf — read fully, especially §19 (Expression, lines covering the 8 precedence levels per RESEARCH lines 333-346), §6-§14 (definition productions: PartDef, PortDef, AttributeDef, ActionDef, StateDef, ItemDef, InterfaceDef, ConnectionDef, FlowDef, AllocationDef, RequirementDef, ConstraintDef, NeedDef, UseCaseDef), §15-§18 (members: PartUsage, PortUsage, AttributeUsage, ActionUsage; annotations; multiplicity; modifiers)
    - spec/grammar/tmp-references/deal-parser-implementation-guide.html §6 (Pratt loop) and §7 (Node structure)
    - deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md (D-01 tagged-union AST, D-02 arena, D-03 unified NodeKind, D-04 JSON shape, D-09 first-class comp_connect — comp_* kinds are declared here for Plan 04 to fill bodies)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Pattern 3: Recursive-descent + Pratt" (lines 327-381 — especially the binding-power table), §"Example C: Tagged union AST node" (lines 701-745)
    - deal/src/ast.zig (current minimal stub)
    - deal/src/expr.zig (stub)
    - spec/references/zig-lang/0.16.0/langref.html §"Tagged union" L5164-5235
  </read_first>
  <files>
    deal/src/ast.zig,
    deal/src/expr.zig
  </files>
  <action>
1. In `deal/src/ast.zig`, extend `NodeKind` to enumerate every kind needed by `.deal` grammar productions (Plan 04 will add the `comp_*` variants for `.dealx` — declare those NOW with TODO-bodied payloads so Plan 04 only fills the payload, not the enum). Required NodeKinds:

   File-level: `deal_file`, `dealx_file` (already), `header_block`, `package_decl`, `import_decl`, `export_decl`.

   Definitions (14 per RESEARCH §Example C lines 708-710): `part_def`, `port_def`, `action_def`, `state_def`, `attribute_def`, `item_def`, `interface_def`, `connection_def`, `flow_def`, `allocation_def`, `requirement_def`, `constraint_def`, `need_def`, `use_case_def`.

   Members/usages: `part_usage`, `port_usage`, `attribute_usage`, `action_usage`, `state_usage`, `item_usage`, `interface_usage`, `connection_usage`, `flow_usage`, `allocation_usage`, `requirement_usage`, `constraint_usage`, `need_usage`, `use_case_usage`.

   Type/structural: `type_annotation`, `multiplicity`, `modifier_list`, `visibility_wrapper`, `specialization`, `redefinition`.

   Annotations: `annotation` (`@trace`, `@confidence`, `@rationale`, etc.), `doc_comment`.

   Expressions: `binary`, `unary`, `member_access`, `call`, `index_access` (if grammar uses), `identifier`, `int_literal`, `real_literal`, `string_literal`, `template_literal`, `boolean_literal`, `unit_call`, `template_part` (head/middle/tail substring), `interpolation` (the expression inside `${...}`), `path_expr` (if grammar §path uses), `object_literal` (for `via={...}` / `carrying={...}` content, shared with Plan 04).

   Composition (.dealx — declared NOW, bodies filled in Plan 04): `system_block`, `subsystem_block`, `component_instance`, `comp_connect`, `expose_tag`, `traceability_block`, `allocate_tag`, `satisfy_block`, `validate_tag`.

2. Declare the `Payload` union with one variant per NodeKind. Use small payload structs declared in the same file:

   - `pub const DealFile = struct { header: ?*Node, package_decl: ?*Node, imports: []*Node, exports: []*Node, definitions: []*Node };`
   - `pub const HeaderBlock = struct { fields: []HeaderField };` where `HeaderField = struct { key: []const u8, value: []const u8, span: Span };` (header values are opaque text per FS-2 — already accounted for in RESEARCH §Security V6).
   - `pub const PackageDecl = struct { name_segments: [][]const u8 };` (e.g. `package deal.std.units` → 3 segments).
   - `pub const ImportDecl = struct { path: [][]const u8, kind: enum { simple, named, wildcard }, items: []ImportItem };` where `ImportItem = struct { name: []const u8, alias: ?[]const u8 };`.
   - `pub const PartDef = struct { name: []const u8, specializes: ?*Node, modifiers: []*Node, annotations: []*Node, members: []*Node, doc: ?*Node };` — and 13 mirror structs for the other definition kinds. The shape is mechanical; share fields via small helpers where possible (e.g. `pub const NamedDef = struct { name: []const u8, modifiers: []*Node, annotations: []*Node, members: []*Node, doc: ?*Node };` and have `pub const PortDef = NamedDef;` etc. if Zig allows the aliasing pattern — it does via `pub const PortDef = NamedDef;`).
   - Expression payloads: `pub const Binary = struct { op: BinaryOp, lhs: *Node, rhs: *Node };` where `pub const BinaryOp = enum { add, sub, mul, div, eq, neq, lt, le, gt, ge, log_and, log_or };` — these are the 12 operators in the Pratt table.
   - `pub const Unary = struct { op: UnaryOp, operand: *Node };` where `pub const UnaryOp = enum { neg, not, bang };` — three unary forms per RESEARCH table line 343.
   - `pub const MemberAccess = struct { receiver: *Node, member: []const u8 };` (D-08 / RESEARCH line 344 postfix `.`).
   - `pub const Call = struct { callee: *Node, args: []*Node };` (RESEARCH line 344 postfix `(`).
   - `pub const Identifier = struct { name: []const u8 };`
   - `pub const IntLiteral = struct { text: []const u8 };` — keep as text to avoid parse failures on overflow; Phase 2 semantic analysis parses to integer.
   - `pub const RealLiteral = struct { text: []const u8 };`
   - `pub const StringLiteral = struct { value: []const u8 };` — pre-unescaped slice into the ESCAPED text region of source; Phase 2 unescapes.
   - `pub const TemplateLiteral = struct { parts: []TemplatePart };` where `pub const TemplatePart = union(enum) { text: []const u8, expr: *Node };` — alternating fixed text and interpolated expression.
   - `pub const BoolLiteral = struct { value: bool };`
   - `pub const UnitCall = struct { unit: []const u8, value: *Node };` (e.g. `V(800)` — the `unit` is `"V"`, the `value` is the IntLiteral 800).
   - `pub const ObjectLiteral = struct { fields: []ObjectField };` where `pub const ObjectField = struct { key: []const u8, value: *Node, span: Span };`.
   - `pub const Annotation = struct { name: []const u8, category: ?[]const u8, value: ?*Node };` — covers `@trace:satisfies:REQ_01`, `@confidence(high)`, etc.

   For composition variants (Plan 04): declare empty placeholder structs `pub const CompConnect = struct { from_expr: ?*Node = null, to_expr: ?*Node = null, via_expr: ?*Node = null, carrying_expr: ?*Node = null, other_props: []*Node = &.{} };` (matches RESEARCH §Example C lines 738-744, D-09). Plan 04 implements parseConnect but the type is declared here so json.zig can switch over the full union.

3. Add the `Node` struct (already declared in Plan 01 but only `(kind, span)`): expand to `pub const Node = struct { kind: NodeKind, span: Span, payload: Payload };`. Use `pub const Payload = union(NodeKind) { deal_file: DealFile, dealx_file: DealxFile, header_block: HeaderBlock, ... };` with one arm per NodeKind. Zig 0.16.0 exhaustive-switch enforcement (RESEARCH line 535) will make adding a NodeKind without a payload arm a compile error — that's the desired property.

4. In `deal/src/expr.zig`, implement the Pratt parser per RESEARCH §Pattern 3 lines 348-375. Required functions:

   - `fn leftBindingPower(tag: lexer.Tag) ?u8` — returns the left binding-power per the 8-level table (lines 333-346):
     - `kw_or` → 10
     - `kw_and` → 20
     - `eq_eq, bang_eq` → 30
     - `lt, le, gt, ge` → 40
     - `plus, minus` → 50
     - `star, slash` → 60
     - `dot` → 80 (postfix member access)
     - `l_paren` → 80 (postfix call)
     - everything else → `null` (terminates the Pratt loop)
   - `fn rightBindingPower(tag: lexer.Tag) u8` — right BP for the same operators (left+1 for left-assoc binary, equal to left for right-assoc; since all 6 binary levels are left-assoc per RESEARCH table, `right = left + 1`).
   - `pub fn parseExpression(parser: *Parser, min_bp: u8) !*ast.Node` where `Parser` is the parser_deal.Parser type imported via `const parser_deal = @import("parser_deal.zig");`. Body follows RESEARCH lines 351-374:
     ```
     var left = try parsePrefix(parser);
     while (true) {
         const tok = parser.peek();
         const bp = leftBindingPower(tok.tag) orelse break;
         if (bp < min_bp) break;
         if (tok.tag == .dot) { ... member access ... }
         else if (tok.tag == .l_paren) { ... call w/ parseArgList ... }
         else { _ = parser.advance(); const right = try parseExpression(parser, rightBindingPower(tok.tag)); left = try makeBinary(parser, left, tok.tag, right); }
     }
     return left;
     ```
   - `fn parsePrefix(parser: *Parser) !*ast.Node` — handle unary prefix operators (`minus`, `kw_not`, `bang` per RESEARCH line 343 — BP 70 right-assoc, so `parseExpression(parser, 70)`), then `parsePrimary`.
   - `fn parsePrimary(parser: *Parser) !*ast.Node` — int_literal, real_literal, string_literal, template_literal, boolean_literal, identifier, parenthesized expression `( expr )`, unit-call `IDENT ( expr )` (LL(2) ambiguity with call — peek 2 tokens to disambiguate `Vehicle(...)` (call) from `V(800)` (unit_call); the parser implementation guide handles this by treating `IDENT ( ... )` always as `Call` and letting Phase 2 semantic analysis reclassify unit calls — pick this simpler approach and document in SUMMARY).
   - `fn parseArgList(parser: *Parser) !std.ArrayList(*ast.Node)` — parse `expr (, expr)*` until `r_paren`; return the arg list.
   - `fn parseTemplateLiteral(parser: *Parser) !*ast.Node` — consume `template_head`, loop reading `expr` (recursively via parseExpression with min_bp 0) followed by `template_middle` or `template_tail`, build `TemplatePart` array.

5. Allocation discipline: every `*ast.Node` is allocated from `parser.arena.allocator()` (per D-02). Use `try parser.arena.allocator().create(ast.Node)` followed by initialization. NEVER store nodes outside the arena.

6. Add a `test "expr.precedence"` block at the bottom of `deal/src/expr.zig` (or in `tests/unit/expr_precedence.zig` — Task 3.2):
   - Source `"a + b * c"` parses to `Binary(add, Identifier(a), Binary(mul, Identifier(b), Identifier(c)))` (mul binds tighter).
   - Source `"a OR b AND c"` parses to `Binary(or, Identifier(a), Binary(and, Identifier(b), Identifier(c)))` (and binds tighter than or).
   - Source `"-a.b(c, d)"` parses to `Unary(neg, Call(MemberAccess(Identifier(a), "b"), [c, d]))` — unary BP 70 < postfix BP 80, so postfix binds first.
   - Source `"a == b AND c"` parses to `Binary(and, Binary(eq, a, b), c)` — eq BP 30 > and BP 20.
  </action>
  <acceptance_criteria>
    - deal/src/ast.zig declares all 14 *_def NodeKind variants (search: `part_def, port_def, action_def, state_def, attribute_def, item_def, interface_def, connection_def, flow_def, allocation_def, requirement_def, constraint_def, need_def, use_case_def` — each appears as an enum variant)
    - deal/src/ast.zig declares `pub const Payload = union(NodeKind) { ... }` with one arm per NodeKind
    - deal/src/ast.zig declares `pub const BinaryOp = enum { add, sub, mul, div, eq, neq, lt, le, gt, ge, log_and, log_or };` (12 ops total)
    - deal/src/expr.zig declares both `leftBindingPower` and `rightBindingPower` and `parseExpression`
    - deal/src/expr.zig contains the literal binding-power values `10`, `20`, `30`, `40`, `50`, `60`, `70`, `80` (the 8 precedence levels)
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build 2>&1 — exits 0 (the AST extension + Pratt parser compile; even though parser_deal.zig is still a stub, expr.zig must compile against the updated AST)</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build 2>&1 | tail -5</automated>
  </verify>
  <done>
    AST tagged-union covers all .deal NodeKinds (Plan 04's .dealx kinds are declared as placeholders so json.zig switches stay exhaustive). Pratt expression parser implements the 8-level binding-power table from deal.ebnf §19 / RESEARCH lines 333-346. Library still compiles.
  </done>
</task>

<task type="auto">
  <name>Task 3.2: Implement parser_deal.zig for all 87 productions + AST JSON emitter + wire C ABI</name>
  <read_first>
    - spec/grammar/deal.ebnf — read every production. The 87 productions are enumerated in the file; map each to a Zig function in parser_deal.zig.
    - spec/grammar/tmp-references/deal-parser-implementation-guide.html §3 (C ABI), §6 (Pratt), §7 (Node shape), §11 (error recovery sync sets — Plan 05 implements; this plan just makes sure productions return on error gracefully)
    - deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md (D-02 arena, D-04 JSON shape, D-13 thread model)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Pattern 3" (lines 327-381), §"Pattern 6: C ABI" (lines 447-515 — esp. lines 466-490 deal_parse + 498-512 deal_ast_json), §"Pitfall 5" (lines 598-608 JSON determinism)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Phase Requirements → Test Map" (lines 938-962) for the exact `zig build test -Dtest-filter=parser_deal.*` commands
    - deal/src/ast.zig (just expanded)
    - deal/src/expr.zig (just written)
    - deal/src/lexer.zig (Plan 02)
    - deal/src/lib.zig (Plan 01 — modify deal_parse and deal_ast_json bodies)
  </read_first>
  <files>
    deal/src/parser_deal.zig,
    deal/src/parser.zig,
    deal/src/json.zig,
    deal/src/lib.zig
  </files>
  <action>
1. In `deal/src/parser_deal.zig`, define the Parser struct with `current_mode` field AND no-op stubs for the mode/tag-stack helpers that Plan 04 will fill in. Plan 03 ships them as STUBS so that the `<interfaces>` block exported to Plan 04 (and consumed by `parser_dealx.zig`) is honest about pre-existing surface area:

   ```
   pub const Parser = struct {
       arena: std.mem.Allocator,        // points at handle.arena.allocator()
       lexer: lexer.Lexer,
       peeked: ?lexer.Token = null,     // single-token lookahead cache (RESEARCH Open Question #1, RESOLVED)
       diagnostics: *std.ArrayList(diagnostics.Diagnostic),
       open_tags: std.ArrayList(parser_dealx.OpenTag) = .empty,  // unused in .deal; declared so parser_dealx can share the struct (Plan 04 pushes/pops)
       source: []const u8,
       current_mode: lexer.Mode = .deal_def,   // Plan 03 hardcodes to .deal_def; Plan 04 makes it dynamic via enterMode/restoreMode

       pub fn peek(self: *Parser) lexer.Token { ... uses self.current_mode (always .deal_def in Plan 03) ... }
       pub fn advance(self: *Parser) lexer.Token { ... uses self.current_mode ... }
       pub fn expect(self: *Parser, expected: lexer.Tag) !lexer.Token { ... }
       fn makeNode(self: *Parser, kind: ast.NodeKind, span: ast.Span, payload: ast.Payload) !*ast.Node { ... }

       // Mode-switching stubs — Plan 03 ships no-op bodies; Plan 04 fills them in for .dealx mode transitions.
       // These exist NOW so that parser_dealx.zig (Plan 04) can compile against a stable Parser surface.
       pub fn enterMode(self: *Parser, m: lexer.Mode) lexer.Mode {
           const prev = self.current_mode;
           self.current_mode = m;
           self.peeked = null;  // invalidate lookahead — Plan 04 documents this as Pitfall 7 mitigation
           return prev;
       }
       pub fn restoreMode(self: *Parser, prev: lexer.Mode) void {
           self.current_mode = prev;
           self.peeked = null;
       }
       // Tag-stack stubs — no-op in Plan 03; Plan 04 fills them.
       pub fn pushTag(self: *Parser, name: []const u8, span: ast.Span) void { _ = self; _ = name; _ = span; }
       pub fn popTag(self: *Parser, expected_name: []const u8, close_span: ast.Span) void { _ = self; _ = expected_name; _ = close_span; }
   };
   ```

   - `peek` returns from `peeked` if set, else calls `lexer.peek(self.current_mode)` (NOT hardcoded `.deal_def`). In Plan 03 the current_mode is always `.deal_def`; Plan 04 mutates it. Storing the mode on the struct lets Plan 04 swap behavior without altering Plan 03 callsites.
   - `advance` returns the peeked token if set (clearing `peeked`), else calls `lexer.next(self.current_mode)`.
   - `expect` calls `advance` and asserts `result.tag == expected`; on mismatch, appends a diagnostic (E0100..E0299 namespace per D-16) AND returns the actual token anyway so the caller can continue. Plan 05 will rewrite `expect` to return an error union; for Plan 03 we use minimal recovery (return the mismatched token, let the caller continue).
   - The `enterMode`/`restoreMode`/`pushTag`/`popTag` stubs in Plan 03 are deliberately no-ops EXCEPT for the lookahead invalidation in enterMode/restoreMode (which is correct behavior even for the `.deal_def → .deal_def` no-op case). Plan 04 adds the actual stack push/pop and dispatch to `lexer.Mode` variants beyond `.deal_def`. Shipping the methods now means Plan 04 fills BODIES, not declarations — and the interface contract documented in Plan 04's `<interfaces>` block ("these methods exist on Parser") is truthful from the moment Plan 03 lands.

2. Implement `pub fn parseFile(handle: *Handle) !?*ast.Node` where `Handle` is the type from `lib.zig` (use `const lib = @import("lib.zig");` and reference `lib.DealHandle`). The function:

   - Initializes a Parser with `arena = handle.arena.allocator()`, `lexer = lexer.Lexer.init(handle.source)`, `diagnostics = &handle.diagnostics`.
   - Calls `parseDealFile(&parser)` and returns the result.
   - Catches OOM errors and appends a fatal diagnostic E0004 (resource exhaustion per D-16); returns `null`.

3. Implement one function per production. The 87 productions cluster into ~10 areas; implement them in this order so unit tests can progress incrementally:

   a. **File structure** (4 productions): `parseDealFile`, `parseHeaderBlock` (`@header { key: "value", ... }` per FS-2 — value is opaque text), `parsePackageDecl`, `parseImportDecl` (`import deal.std.units.{kg as kilogram, V}` — handles `.{...}` named-imports and `as` aliasing per the grammar), `parseExportDecl`.

   b. **Definition dispatch** (per the `Definition` non-terminal in deal.ebnf): a single `parseDefinition` that peeks for one of the 14 definition keywords (`part`, `port`, `action`, `state`, `attribute`, `item`, `interface`, `connection`, `flow`, `allocation`, `requirement`, `constraint`, `need`, `use case`) and dispatches to `parsePartDef` / `parsePortDef` / ... — each definition function reads `IDENT` (name), optional `specializes IDENT` (or `<<specializes>>` with full type ref — read deal.ebnf for exact syntax), optional `[<<modifier>>]*`, optional `@annotation*`, then `{` member-list `}`.

   c. **Visibility wrapper** (1 production): `parseVisibilityWrapper` handles `public ( ... )`, `protected ( ... )`, `private ( ... )` per SD-9. Inside the parens is a member-list.

   d. **Members and usages** (14 productions): for each definition kind there's a corresponding `usage` form (e.g. inside `part def Vehicle { battery : Battery; }` the `battery : Battery` is a `part_usage`). Implement `parseMember` that dispatches by keyword OR by absence of a definition keyword (then it's a usage) — read the grammar for the exact rule.

   e. **Type annotations and multiplicity** (~6 productions): `parseTypeAnnotation` (`: Type`), `parseMultiplicity` (`[0..*]`, `[1]`, `[1..3]`), `parseModifierList` (the `<<...>>` brackets), `parseSpecialization` (`specializes Type`), `parseRedefinition` (`redefines name`).

   f. **Annotations** (~5 productions): `parseAnnotation` for `@trace:satisfies REQ_01`, `@confidence(high)`, `@rationale("text")`, `@validate { ... }`. The exact annotation grammar lives in deal.ebnf §17 (verify section number when reading).

   g. **Statements / member bodies / expressions**: all expressions go through `expr.parseExpression(parser, 0)`. Statements may be just `expression ;` or specific forms — read deal.ebnf for the StatementList non-terminal.

   h. **Doc comments**: when the lexer emits a `doc_comment` token, the parser attaches it to the next definition or member as `doc: ?*Node`. Implement this in a `consumeLeadingDocComment` helper that runs at the start of each definition / member parser.

4. Implement `deal/src/parser.zig`:

   ```
   pub fn parseFile(handle: *lib.DealHandle) !?*ast.Node {
       return switch (handle.mode) {
           .deal => parser_deal.parseFile(handle),
           .dealx => parser_dealx.parseFile(handle),   // stub in Plan 03; Plan 04 fills body
       };
   }
   ```

5. In `deal/src/json.zig`, implement the real `pub fn emitAst(allocator, root, mode, filename)`. Hand-roll the writer (RESEARCH §Pitfall 5 — do NOT trust `std.json.Stringify` for the top-level object due to issue #25233). Output shape per D-04:

   ```
   {"v":1,"mode":"deal","filename":"battery.deal","root":{"k":"deal_file","span":[0,N],"header":...,"package":...,"imports":[...],"exports":[...],"definitions":[...]}}
   ```

   - Top-level keys in fixed order: `v`, `mode`, `filename`, `root`.
   - Each node JSON: `k` first, `span` second, then payload-specific fields in alphabetical order (deterministic — pick alphabetical and stick with it; document in SUMMARY).
   - `switch (node.payload) { ... }` is exhaustive over Payload (Zig 0.16.0 enforces — adding a NodeKind without a json branch is a compile error per RESEARCH line 535).
   - For each Payload variant, write the fields in alphabetical order. Lists `[]*Node` emit as JSON arrays of recursive node JSONs. Optional `?*Node` emit as `null` or the recursive form.
   - String escaping: same rules as Plan 02 token emitter (`"`, `\`, `\n`, `\r`, `\t`, control bytes < 0x20). Re-use the helper from Plan 02 if you wrote one; otherwise define it here.

6. Update `deal/src/lib.zig`:

   - In `deal_parse`: after duping source + filename into the arena, call `parser.parseFile(handle) catch null` and store in `handle.ast_root`. If `parser.parseFile` returns an OOM error, append diagnostic E0004 and set ast_root to null — but STILL return the handle (D-10).
   - In `deal_ast_json`: now that ast_root may be non-null, the lazy JSON emission calls `json.emitAst(handle.arena.allocator(), handle.ast_root, handle.mode, handle.filename)` and caches the result on `handle.cached_ast_json`.

7. Smoke-test wiring: add `test "parser_deal.smoke"` at the bottom of `parser_deal.zig` parsing the source `"package foo;\n\npart def Bar {}"` and asserting:
   - `parseFile` returns a non-null Node
   - The root's kind is `.deal_file`
   - root.payload.deal_file.package_decl is not null
   - root.payload.deal_file.definitions.len == 1
   - definitions[0].kind == .part_def
   - definitions[0].payload.part_def.name == "Bar"
  </action>
  <acceptance_criteria>
    - deal/src/parser_deal.zig contains `pub const Parser = struct {`
    - deal/src/parser_deal.zig contains `pub fn parseFile(`
    - deal/src/parser_deal.zig contains literal `current_mode`, `enterMode`, `restoreMode`, `pushTag`, `popTag` (Plan 03 ships no-op stubs; Plan 04 fills bodies — Warning #5 fix)
    - deal/src/parser_deal.zig contains functions for at minimum: `parseDealFile`, `parseHeaderBlock`, `parsePackageDecl`, `parseImportDecl`, `parseDefinition`, `parsePartDef`, `parsePortDef`, `parseAttributeDef`, `parseRequirementDef`, `parseAnnotation`, `parseTypeAnnotation`, `parseMultiplicity`, `parseMember` (grep for these literal names)
    - deal/src/parser.zig dispatches on `handle.mode` and calls `parser_deal.parseFile` for `.deal` mode
    - deal/src/json.zig contains an exhaustive `switch (node.payload)` (verified by grep — at minimum 14 `*_def =>` arms covering the 14 definition kinds)
    - deal/src/lib.zig's `deal_parse` calls `parser.parseFile`
    - deal/src/lib.zig's `deal_ast_json` calls `json.emitAst` with the real ast_root
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_deal.smoke 2>&1 — exits 0; the smoke test passes</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_deal 2>&1 | tail -10</automated>
  </verify>
  <done>
    Recursive-descent parser implemented for all 87 deal.ebnf productions. AST JSON emitter produces deterministic D-04-shaped output. C ABI wired so `deal_parse` populates the real AST and `deal_ast_json` returns the JSON. Smoke test (`package foo; part def Bar {}`) parses successfully through the FFI.
  </done>
</task>

<task type="auto">
  <name>Task 3.3: 15 .deal showcase snapshots + Pratt precedence test + 87-production coverage</name>
  <read_first>
    - deal/.planning/phases/01-zig-compiler-core/01-VALIDATION.md (manual-only snapshot reviewer-acceptance row)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Phase Requirements → Test Map" lines 947-949 (parser_deal.snapshot, expr.precedence, coverage)
    - spec/examples/showcase/packages/ — read each of the 15 .deal files
    - spec/grammar/deal.ebnf — to write the production-coverage map
    - deal/tests/unit/lexer_snapshot.zig (Plan 02) — mirror its UPDATE_SNAPSHOTS=1 pattern
    - deal/src/parser_deal.zig (just written)
    - deal/src/json.zig (just written)
  </read_first>
  <files>
    deal/tests/unit/expr_precedence.zig,
    deal/tests/unit/parser_deal_snapshot.zig,
    deal/tests/unit/parser_deal_coverage.zig,
    deal/tests/snapshots/ast/.gitkeep,
    deal/tests/snapshots/ast/showcase__packages__vehicle__battery.deal.json,
    deal/tests/snapshots/ast/showcase__packages__vehicle__motor.deal.json,
    deal/tests/snapshots/ast/showcase__packages__vehicle__behaviors.deal.json,
    deal/tests/snapshots/ast/showcase__packages__vehicle__components.deal.json,
    deal/tests/snapshots/ast/showcase__packages__vehicle__index.deal.json,
    deal/tests/snapshots/ast/showcase__packages__interfaces__electrical.deal.json,
    deal/tests/snapshots/ast/showcase__packages__interfaces__thermal.deal.json,
    deal/tests/snapshots/ast/showcase__packages__interfaces__connections.deal.json,
    deal/tests/snapshots/ast/showcase__packages__interfaces__index.deal.json,
    deal/tests/snapshots/ast/showcase__packages__requirements__system.deal.json,
    deal/tests/snapshots/ast/showcase__packages__requirements__needs.deal.json,
    deal/tests/snapshots/ast/showcase__packages__requirements__index.deal.json,
    deal/tests/snapshots/ast/showcase__packages__use-cases__driving.deal.json,
    deal/tests/snapshots/ast/showcase__packages__use-cases__charging.deal.json,
    deal/tests/snapshots/ast/showcase__packages__use-cases__index.deal.json,
    deal/build.zig
  </files>
  <action>
1. Create `deal/tests/unit/expr_precedence.zig` with `test "expr.precedence"` (SOLE test name — matches `-Dtest-filter=expr.precedence`):
   - Build a tiny harness: `fn parseExprFromSource(allocator, src) !*ast.Node` that constructs a minimal Parser (no diagnostics list needed for these tests; use a local diagnostics ArrayList), runs `expr.parseExpression(&parser, 0)`, returns the root expression.
   - For each of the 6 binary-operator levels, parse a triple involving the next-higher level and assert the tree structure:
     - `"a OR b AND c"` → `Binary(or, Identifier(a), Binary(and, b, c))`
     - `"a AND b == c"` → `Binary(and, a, Binary(eq, b, c))`
     - `"a == b > c"` → `Binary(eq, a, Binary(gt, b, c))`
     - `"a > b + c"` → `Binary(gt, a, Binary(add, b, c))`
     - `"a + b * c"` → `Binary(add, a, Binary(mul, b, c))`
     - `"a * b.c(d)"` → `Binary(mul, a, Call(MemberAccess(b, "c"), [d]))`
   - For unary BP 70: `"NOT a == b"` → `Binary(eq, Unary(not, a), b)` (unary BP 70 > eq BP 30, but unary is PREFIX, so the NOT applies to `a` and then the `== b` consumes the result — verify against the table semantics; if your implementation makes NOT lower-binding, document in SUMMARY).
   - For postfix BP 80: `"-a.b(c)"` → `Unary(neg, Call(MemberAccess(a, "b"), [c]))` (postfix BP 80 > unary BP 70, so `.b(c)` binds first).
   - Assertions use `try std.testing.expectEqual` on `node.kind`, payload tags, and recursive structure.

2. Create `deal/tests/unit/parser_deal_snapshot.zig` with `test "parser_deal.snapshot"`:
   - Enumerate the 15 .deal showcase paths (same convention as Plan 02 snapshot test — copy and adapt).
   - For each: read the file, invoke `deal_parse(source, filename)` via internal Zig API (not the C ABI — use `lib.DealHandle` and `parser.parseFile` directly; this avoids the FFI overhead in unit tests). Get the JSON via `json.emitAst(...)`.
   - Compare against the committed snapshot at `tests/snapshots/ast/showcase__<path>.json`. UPDATE_SNAPSHOTS=1 to regenerate. On mismatch, write `.actual` and fail.
   - Additional assertion: parsing each file must produce a non-null `ast_root` AND the file's diagnostic count must be 0 (the 15 .deal showcase files are all well-formed per Phase 0's verification report; if parsing introduces a diagnostic it's a regression in the parser).

3. Create `deal/tests/unit/parser_deal_coverage.zig` with `test "parser_deal.coverage"`:
   - Walk the AST of every parsed showcase file (using a recursive `fn visit(node: *ast.Node, seen: *std.AutoArrayHashMap(ast.NodeKind, void))` function).
   - At the end, write the production-coverage tally to stderr (one line per NodeKind: `kind=part_def count=42`).
   - Assertion: every kind in a curated "required for Phase 1" subset MUST appear ≥ 1 time across the corpus. The required subset (mapped from deal.ebnf production list):
     - All 14 `*_def` kinds (part, port, action, state, attribute, item, interface, connection, flow, allocation, requirement, constraint, need, use_case)
     - All 14 `*_usage` kinds
     - File-level: `deal_file`, `header_block`, `package_decl`, `import_decl`, `export_decl`
     - Expressions: `binary`, `unary`, `member_access`, `call`, `identifier`, `int_literal`, `real_literal`, `string_literal`, `template_literal`, `boolean_literal`, `unit_call`
     - Annotations: `annotation`, `doc_comment`
     - Type/structural: `type_annotation`, `multiplicity`, `modifier_list`, `specialization`
   - If a required kind has count 0, fail with a message naming the missing kind. This is the "87 productions each exercised by ≥1 fixture" gate (RESEARCH line 949).
   - For productions that the 15 showcase files don't naturally exercise (e.g. `visibility_wrapper`, `redefinition`), add 1-3 hand-written fixtures as inline string sources within the test file (`const fixtures = [_]struct { name: []const u8, src: []const u8 }{...};`) and include them in the coverage walk.

4. Generate the 15 AST snapshots via `cd deal && UPDATE_SNAPSHOTS=1 zig build test -Dtest-filter=parser_deal.snapshot`. Visually inspect at least 3 (battery.deal as the canonical part_def case; index.deal as the canonical package+import case; requirements/system.deal as the canonical requirement_def + annotation case) per the VALIDATION.md manual-only-verifications row. Confirm:
   - Top-level `{"v":1,"mode":"deal","filename":"<basename>","root":{...}}`
   - Spans are byte-aligned with the source (start of root is 0; end of root is `source.len`)
   - Definition names match (e.g. `battery.deal` produces `"name":"Battery"` somewhere)

5. Commit all 15 snapshots. Wire `expr_precedence.zig`, `parser_deal_snapshot.zig`, `parser_deal_coverage.zig` into `deal/build.zig` test step.

6. Re-run Plan 01's FFI smoke test to confirm the C ABI still works through the now-real parser: `cd deal && cargo test --manifest-path tests/ffi/Cargo.toml` should still pass. The smoke test in Plan 01 used an empty source; with the real parser, an empty source should produce a `deal_file` root with empty imports/exports/definitions and zero diagnostics.
  </action>
  <acceptance_criteria>
    - deal/tests/unit/expr_precedence.zig contains exactly one `test "expr.precedence"` block
    - deal/tests/unit/parser_deal_snapshot.zig contains exactly one `test "parser_deal.snapshot"` block
    - deal/tests/unit/parser_deal_coverage.zig contains exactly one `test "parser_deal.coverage"` block
    - All 15 .deal AST snapshot files exist under deal/tests/snapshots/ast/
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=expr.precedence 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_deal.snapshot 2>&1 — exits 0; the 15-file snapshot test passes</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_deal.coverage 2>&1 — exits 0; every required NodeKind has count ≥ 1</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml 2>&1 — exits 0; Plan 01's smoke tests still green after C ABI now invokes the real parser</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && ls tests/snapshots/ast/*.deal.json | wc -l — produces exactly 15</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test 2>&1 | tail -15</automated>
  </verify>
  <done>
    Phase 1 ROADMAP success criterion #2 met: "The 15 .deal showcase files parse through recursive-descent + Pratt covering 87 productions, producing stable AST JSON snapshots." Pratt precedence test green (8 levels documented). Production coverage test green (every NodeKind required by deal.ebnf has ≥ 1 fixture). C ABI's deal_ast_json now returns the real tree for any of the 15 showcase files when called via the Rust harness.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Lexer tokens → Parser AST | Tokens are trusted (Plan 02 guarantees zero-unknown for valid inputs; malformed inputs from Plan 05 produce `unknown` tokens). Parser must handle `unknown` tokens without panic. |
| AST → JSON | The hand-rolled JSON writer is the source of truth for the snapshot contract. Determinism is a security property of the test suite. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-03-01 | DoS | Recursion depth in parseExpression / parseDefinition | mitigate | Plan 05 implements depth bound + E0303. Plan 03 keeps recursion natural; pathological inputs are not yet a Phase 1 acceptance criterion (the showcase corpus is well-formed). |
| T-03-02 | Information Disclosure | Source bytes in JSON output | accept | The JSON's purpose is to expose the parsed tree. Identifier slices appear verbatim in `"name":"..."` fields. This is the contract per D-04. Escaping rules prevent injection (Plan 02 helper). |
| T-03-03 | Tampering | Snapshot byte-stability across machines/Zig versions | mitigate | Hand-rolled JSON with alphabetical field order in payloads + fixed top-level key order (`v`, `mode`, `filename`, `root`). Closes RESEARCH Pitfall 5 by NOT using `std.json.Stringify` for the top-level. |
| T-03-04 | Resource Exhaustion | Arena allocator growth on large files | mitigate | Arena uses `page_allocator` (D-02) — single OS-alloc backing many sub-allocations. Page allocator is bounded by virtual memory; pathological-input bound is added in Plan 05 via depth limit. |
| T-03-SC | Tampering | No new external dependencies | accept | This plan uses only Zig 0.16.0 std-lib (no new packages). build.zig.zon `.dependencies` remains empty. |
</threat_model>

<verification>
1. `cd /Users/dunnock/projects/deal-lang/deal && zig build` exits 0.
2. `cd /Users/dunnock/projects/deal-lang/deal && zig build test` exits 0 — full suite (lexer from Plan 02 + new parser_deal tests) green.
3. `cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_deal.snapshot` exits 0 — all 15 AST snapshots byte-match.
4. `cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=expr.precedence` exits 0.
5. `cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=parser_deal.coverage` exits 0; coverage tally on stderr confirms every required NodeKind ≥ 1.
6. `cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml` exits 0 — C ABI still consumable; ffi_smoke now exercises the real parser.
7. `ls deal/tests/snapshots/ast/*.deal.json | wc -l` = 15.
</verification>

<success_criteria>
- Phase 1 ROADMAP success criterion #2 met: 15 .deal files parse through recursive-descent + Pratt, producing stable AST JSON snapshots.
- REQ-phase-1-2-parser-deal acceptance met: all 15 .deal showcase files parse; AST JSON snapshots stable; Pratt expression parser working.
- D-01 (tagged-union AST) honored: `Node = struct { kind, span, payload: union(NodeKind) }`.
- D-02 (per-handle arena lifetime) honored: every Node + interior slice allocated from `handle.arena.allocator()`; `deal_free` releases all of it.
- D-03 (unified NodeKind across .deal and .dealx) honored: NodeKind enumerates BOTH .deal definition kinds AND .dealx comp_* placeholders; Plan 04 fills the comp_* bodies without changing the enum.
- D-04 (JSON shape: v=1, mode, filename, root + kind-tagged span'd nodes) honored: every JSON output starts with `{"v":1,"mode":...,"filename":...,"root":{...}}`.
- D-05 (Zig 0.16.0 standards) honored: no `callconv(.C)` uppercase, no `ArrayList(T).init`, no removed APIs.
- All Validation Architecture rows for parser_deal (lines 947-949 of RESEARCH) green.
</success_criteria>

<output>
Create `.planning/phases/01-zig-compiler-core/01-03-SUMMARY.md` when done, recording:
- Final NodeKind enum count (likely ~60 variants spanning .deal definitions + members + expressions + .dealx placeholders) — for Plan 04 to confirm before adding new variants.
- Production-coverage tally: which NodeKinds were exercised by which showcase file(s); which required synthetic fixtures.
- Whether single-token peek was sufficient (RESEARCH Open Question #1) or whether two-token lookahead was needed for any LL(2) decision points (per grammar README).
- Whether `IDENT ( ... )` was always parsed as Call and left to Phase 2 to reclassify as UnitCall (the recommendation), OR whether the parser distinguishes at parse time. Plan 04 needs to know which is the case (the .dealx `via={CANHarness { connectorType: "Molex MX150" }}` example tokenizes as IDENT + OBJECT_LITERAL — not a call).
- Final alphabetical-vs-declaration-order choice for JSON payload field emission (locked in this plan, must not change in Plans 04/05).
- Whether any of the 87 deal.ebnf productions had to be modified or required clarification — Phase 0 guarantee per RESEARCH assumption A7 says "if a file violates the grammar, fix the file, not the grammar"; record any such deviations.
</output>
