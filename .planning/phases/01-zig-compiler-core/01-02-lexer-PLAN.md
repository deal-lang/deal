---
phase: 01-zig-compiler-core
plan: 02
type: execute
wave: 2
depends_on:
  - 01-01-foundation
files_modified:
  - deal/src/lexer.zig
  - deal/src/keywords.zig
  - deal/src/source_map.zig
  - deal/src/json.zig
  - deal/tests/unit/lexer_mode_flip.zig
  - deal/tests/unit/lexer_keywords.zig
  - deal/tests/unit/lexer_comments.zig
  - deal/tests/unit/lexer_templates.zig
  - deal/tests/snapshots/tokens/.gitkeep
  - deal/build.zig
autonomous: true
requirements:
  - REQ-phase-1-1-lexer

must_haves:
  decisions_implemented:
    - "D-05: Zig 0.16.0 idioms — no `callconv(.C)`, `ArrayList(T) = .empty` allocator-explicit shape, `std.StaticStringMap(V).initComptime(.{})` enforced by acceptance criteria grep on `keywords.zig` and `lexer.zig`"
    - "D-06: Four-mode lexer dispatch (deal_def, dealx_outer, dealx_tag, dealx_expr_brace) — `next(mode)` / `peek(mode)` branch on mode; `[<` lexes as `l_bracket + lt` in `.deal_def` vs `tag_open` in `.dealx_outer`; verified by `lexer.mode_flip` test"
    - "D-15: `Span = { start: u32, end: u32 }` byte offsets only — every `Token` carries `Span`; tokens have no text slot (text recovered from source slice via span); verified by snapshot JSON shape `\"span\":[s,e]`"
    - "D-16: Reserves E0001..E0099 lexer error code range — documented in `src/diagnostics.zig` namespace comment; lexer doesn't emit diagnostics in Plan 02 but `lexer.comments` test cites E0001 as the unterminated-comment recovery code Plan 05 will emit"
  truths:
    - "All 19 showcase files (15 .deal + 4 .dealx) tokenize through lexer.next() with zero UNKNOWN tokens"
    - "In .deal_def mode, the bytes `[<` produce LBRACKET followed by LT (two separate tokens); in .dealx_outer mode the bytes `[<` produce a single TAG_OPEN"
    - "85 reserved or contextual keyword spellings from lexical.ebnf §5 are recognized as their kw_* tag; identifiers not in that table pass through as ident"
    - "`/**` opens a DOC_COMMENT; `/*` opens a BLOCK_COMMENT; the two are distinct tags"
    - "Template-string `${}` interpolation produces TEMPLATE_HEAD / TEMPLATE_MIDDLE / TEMPLATE_TAIL tokens via a bounded interpolation state stack"
    - "Every token snapshot for the 19 showcase files is byte-stable across consecutive `zig build test -Dtest-filter=lexer.snapshot` runs"
  artifacts:
    - path: "deal/src/lexer.zig"
      provides: "Full stateless lexer implementing lexical.ebnf with `next(mode)` and `peek(mode)`; mode-dependent dispatch for `[<` / `>]` / `/>]`; template `${}` state stack bounded ≤ depth 64"
      contains: "TAG_OPEN"
    - path: "deal/src/keywords.zig"
      provides: "std.StaticStringMap(Tag).initComptime with all 85 reserved/contextual keyword spellings from lexical.ebnf §5"
      contains: "initComptime"
    - path: "deal/src/source_map.zig"
      provides: "Lazy line-start table; lineCol(offset) returns 1-based (line, col)"
      contains: "line_starts"
    - path: "deal/src/json.zig"
      provides: "emitTokens(allocator, source, mode) -> {\"v\":1,\"tokens\":[{\"k\":\"...\",\"span\":[s,e],\"text\":\"...\"}]} for snapshot tests"
      contains: "emitTokens"
    - path: "deal/tests/unit/lexer_mode_flip.zig"
      provides: "Test cases proving `[<` lexes differently in .deal_def vs .dealx_outer"
    - path: "deal/tests/unit/lexer_keywords.zig"
      provides: "Test cases proving 85 keyword spellings resolve to kw_* tags and arbitrary identifiers stay as .ident"
    - path: "deal/tests/unit/lexer_comments.zig"
      provides: "Test cases proving /** vs /* disambiguation and that nested block-comment depth (if grammar allows) is bounded"
    - path: "deal/tests/unit/lexer_templates.zig"
      provides: "Test cases proving template ${} state stack handles arbitrary nesting up to the bound and produces HEAD/MIDDLE/TAIL tokens"
  key_links:
    - from: "deal/src/lexer.zig"
      to: "deal/src/keywords.zig"
      via: "global_keywords.get(slice) lookup after scanning IDENT"
      pattern: "global_keywords\\.get"
    - from: "deal/src/lexer.zig"
      to: "spec/grammar/lexical.ebnf"
      via: "Tag enum mirrors the token productions in lexical.ebnf §5-§14"
      pattern: "TAG_OPEN|LBRACKET|TEMPLATE_HEAD"
    - from: "deal/tests/unit/lexer_*.zig"
      to: "deal/src/lexer.zig"
      via: "@import via build.zig test_module wiring"
      pattern: "@import\\(\"../../src/lexer.zig\"\\)"
---

<objective>
Implement the stateless lexer covering the full `lexical.ebnf` token set with the four-mode dispatch from D-06. The lexer is a thin scanner: the parser passes `mode` on every call. Tokens carry `(tag, span)` only — text is recovered from the source slice on demand (per RESEARCH §"String interning" Open Question #2 recommendation).

Purpose: the lexer is the bottom-most layer of every subsequent wave. Plans 03 and 04 cannot start until tokens lex correctly. The mode-flip rule (D-06) is the single most failure-prone aspect of the dual-grammar architecture; making it observable and tested at the lexer level — before any parser code looks at tokens — eliminates the largest class of latent bugs.

Output: `next(mode)` / `peek(mode)` lex every byte sequence in the 19 showcase files into a defined Tag (zero `.unknown` tokens), the four locked modes produce the documented `[<` mode-flip behavior, keyword recognition uses comptime perfect-hash lookup, and committed token snapshots establish the lexer as the source of truth for downstream parsers.
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
@spec/grammar/lexical.ebnf
@spec/grammar/README.md
@deal/src/lexer.zig
@deal/src/keywords.zig
@deal/src/ast.zig
@deal/src/source_map.zig
@deal/src/json.zig
@deal/build.zig
@spec/references/zig-lang/0.16.0/langref.html

<interfaces>
From deal/src/ast.zig (already committed by Plan 01):
- `pub const Span = extern struct { start: u32, end: u32 };` (D-15)
- `pub const Mode = enum { deal, dealx };` (file-level mode, used by emitTokens for filename echo)

From deal/src/lexer.zig (Plan 01 stub — this plan replaces the body):
- `pub const Mode = enum { deal_def, dealx_outer, dealx_tag, dealx_expr_brace };` (D-06; KEEP this enum and its variant names exactly)
- `pub const Token = struct { tag: Tag, span: ast.Span };` (NO text slot — recover from source via span)
- `pub const Lexer = struct { source: []const u8, pos: u32 = 0 };`
- `pub fn next(self: *Lexer, mode: Mode) Token;` — stateful w.r.t. pos, stateless w.r.t. mode
- `pub fn peek(self: *Lexer, mode: Mode) Token;` — pure; restores pos before returning

From deal/src/keywords.zig (Plan 01 stub):
- `pub const global_keywords = std.StaticStringMap(lexer.Tag).initComptime(.{});` — empty map; this plan populates it
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 2.1: Implement lexer.zig + keywords.zig + source_map.zig</name>
  <read_first>
    - spec/grammar/lexical.ebnf — read all 758 lines. Identify: the 85 reserved/contextual keyword spellings in §5 (locked count per `_Keyword` production lines 459-481; note the line-749 summary comment that says "76" is a stale doc-string and will be corrected in a separate spec PR — the 85-count from the production body is authoritative); the operator/punctuation tokens in §9-§12; the literal forms in §6-§8 (integer, real, string, template); the comment forms in §13; the whitespace/newline rules in §14; the contextual tokens `[<`, `>]`, `/>]`, `[</` introduced in §11 for .dealx mode.
    - spec/grammar/README.md — confirm production counts and the LL(2) decision points
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Pattern 1: Lexer-on-demand" (lines 267-299), §"Pattern 2: Comptime keyword tables" (lines 301-326), §"Pitfall 7: .dealx {...} re-entering" (lines 620-628), §"Pitfall 3: ArrayList allocator-explicit" (lines 578-587), §"Pitfall 5: std.json.Stringify non-deterministic order" (lines 598-608)
    - deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md (D-06 four modes, D-15 Span byte-offsets-only)
    - deal/src/lexer.zig (Plan 01 stub — replace bodies, keep exported signatures)
    - deal/src/keywords.zig (Plan 01 stub)
    - deal/src/source_map.zig (Plan 01 stub)
    - spec/references/zig-lang/0.16.0/langref.html §"std.StaticStringMap" + §"comptime"
  </read_first>
  <files>
    deal/src/lexer.zig,
    deal/src/keywords.zig,
    deal/src/source_map.zig
  </files>
  <action>
Fill in `deal/src/lexer.zig`:

1. Expand the `Tag` enum to cover every token in `lexical.ebnf`. Required variants (group by section of the grammar; do NOT abbreviate names — match the grammar's UPPER_SNAKE literal names lowercased):
   - Structural: `eof`, `newline` (only emitted if grammar treats newlines significantly; lexical.ebnf §14 specifies — read it), `unknown` (kept as a fallback but MUST be empty on showcase corpus).
   - Identifiers/keywords: `ident`, plus one `kw_*` variant per reserved or contextual keyword in §5. The locked count is **85** (pre-resolved at plan time from `spec/grammar/lexical.ebnf` §5 production `_Keyword` lines 459-481; verified by `awk '/^_Keyword[[:space:]]*::=/,/^$/' spec/grammar/lexical.ebnf | grep -oE '"[a-zA-Z@][a-zA-Z_]*"' | sort -u | wc -l` → 85). The file's line-749 summary comment that says "76" is a stale doc-string and will be corrected in a separate spec PR; do NOT rely on the comment. The 85 spellings span: 16 element keywords (`part`, `port`, `action`, `state`, `attribute`, `item`, `interface`, `connection`, `flow`, `allocation`, `requirement`, `constraint`, `need`, `def`, `use`, `case`); 10 modifier keywords (`abstract`, `derived`, `readonly`, `ordered`, `nonunique`, `individual`, `variation`, `portion`, `end`, `ref`); 3 direction (`in`, `out`, `inout`); 3 visibility (`public`, `protected`, `private`); 4 module (`package`, `import`, `export`, `as`); 10 requirement/use-case body (`actor`, `subject`, `precondition`, `postcondition`, `verification`, `accepts`, `rejects`, `threshold`, `operator`, `conditions`); 8 composition tag names (`system`, `subsystem`, `connect`, `expose`, `traceability`, `satisfy`, `validate`, `allocate`); 7 composition attribute names (`from`, `to`, `via`, `carrying`, `method`, `status`, `relationship`); 11 evidence/criteria (`criteria`, `evidence`, `compute`, `maps`, `gap`, `simulation`, `test`, `analysis`, `design`, `inspection`, `demonstration`); 4 logical operators (`AND`, `OR`, `NOT`, `WHERE`); and 9 `@header` field names (`path`, `schema`, `created`, `modified`, `reviewed`, `hash`, `baseline`, `marking`, `by`). Per-category sum: 16+10+3+3+4+10+8+7+11+4+9 = 85. Note: `true`/`false` are emitted as `BOOLEAN_LITERAL` (not keywords), and `Boolean`/`Integer`/`Real`/`String` are emitted as plain `IDENT` (semantic analyzer resolves them later). The 85-count is locked — every entry in `keywords.zig` MUST map to exactly one of these 85 spellings.
   - Literals: `int_literal`, `real_literal`, `string_literal`, `template_head`, `template_middle`, `template_tail`, `unit_call_ident` (if grammar distinguishes; otherwise unit literals are `ident` followed by `(` and a primary expression — defer to grammar text).
   - Operators: `plus`, `minus`, `star`, `slash`, `percent` (if grammar uses), `eq_eq`, `bang_eq`, `lt`, `gt`, `lt_eq`, `gt_eq`, `eq` (assignment), `arrow` (`->` if grammar uses), `fat_arrow` (`=>` if grammar uses), `dot`, `dot_dot` (range), `colon`, `colon_colon` (path separator if grammar uses), `comma`, `semicolon`, `question`.
   - Brackets: `l_paren`, `r_paren`, `l_brace`, `r_brace`, `l_bracket`, `r_bracket`, `template_open` (the `${`), `template_close` (the matching `}` — distinguished from `r_brace` only by the lexer's template state stack).
   - Composition (`.dealx` only — emitted ONLY in `.dealx_outer` or `.dealx_tag` modes): `tag_open` (`[<`), `tag_close_open` (`[</`), `tag_self_close` (`/>]`), `tag_close` (`>]`).
   - Special: `at` (`@` annotation prefix), `hash` (if grammar uses), `dollar` (only as the lead byte of `${` — never standalone).
   - Comments: `doc_comment` (`/** ... */`), `block_comment` (`/* ... */`), `line_comment` (`//` to EOL).

2. Implement `pub fn next(self: *Lexer, mode: Mode) Token`:
   - Skip leading whitespace + line comments + block comments (block comments and doc comments are SKIPPED in `next` and produced separately — see acceptance criteria for whether the parser wants comments preserved; per RESEARCH §Pattern 1 the lexer-on-demand returns DOC comments as tokens so the parser can attach them, but BLOCK comments are skipped. Verify against lexical.ebnf and pick the rule that matches the grammar; document in SUMMARY.).
   - Branch on `mode`:
     - `.deal_def`: never emit `tag_open` / `tag_close_open` / `tag_close` / `tag_self_close`. The sequence `[<` produces `l_bracket` (advance 1), then a SUBSEQUENT call sees `<` as `lt` (advance 1). Same for `>]` → `gt` then `r_bracket`.
     - `.dealx_outer`: at position where `source[pos..pos+2] == "[<"` AND `source[pos..pos+3] != "[</"`, emit `tag_open` and advance 2. Where `source[pos..pos+3] == "[</"`, emit `tag_close_open` and advance 3. Otherwise fall through to the shared lexer logic that produces `package`, `import`, `export`, IDENT, etc. — top-level `.dealx` content is mostly the same as `.deal` MINUS the composition-tag entry points.
     - `.dealx_tag`: inside an open-tag pair. Recognize `>]` (advance 2 → `tag_close`), `/>]` (advance 3 → `tag_self_close`), `=` → `eq`, IDENT for prop names. The `{` here triggers a transition that the PARSER handles by switching to `.dealx_expr_brace` for the next call — the lexer just emits `l_brace`.
     - `.dealx_expr_brace`: identical to `.deal_def` (full deal expression grammar) per D-08. The closing `}` is `r_brace`; the parser pops back to `.dealx_tag` when its inner-frame returns.
   - Inside a template string `\`...${X}...\``: implement a bounded interpolation state stack `template_depth: [64]u8` where each entry counts unmatched braces inside the current `${...}` segment. On `${`, push 0 and emit `template_head` (or `template_middle` if continuing); on every `{` in expression context, increment top; on `}`, decrement top — if it reaches 0, pop and continue scanning string bytes as `template_middle` (until next `${`) or `template_tail` (until the closing backtick). If the stack overflows 64, emit a `unknown` token with span at the offending `${` and a comment in the source code citing T-02-DoS. Bound the depth at 64 explicitly (RESEARCH §Threat Patterns line 1007).
   - On UTF-8 validation: at the top of the function, if `self.pos < self.source.len` and the current byte is the first byte of a multi-byte sequence, do a fast continuation-byte check; if invalid, emit `unknown` with span `[pos, pos+1]` (the full source-level UTF-8 validation happens in `deal_parse` per Plan 06 — the lexer only fails gracefully on malformed bytes mid-stream).

3. Implement `pub fn peek(self: *Lexer, mode: Mode) Token` as: save pos, call next, restore pos, return the token. (Plan 03 may optimize to a one-token cache; Plan 02 keeps the simplest implementation.)

4. After scanning an IDENT slice, look it up in `keywords.global_keywords`: `if (keywords.global_keywords.get(slice)) |kw_tag| return Token{ .tag = kw_tag, .span = sp }; return Token{ .tag = .ident, .span = sp };`. Use `std.mem.eql(u8, ...)` semantics — byte equality only, no case-insensitivity (RESEARCH §Threat T-locale line 1013).

5. Doc comments: `/**` immediately followed by NOT `*` (to distinguish from `/***` which the grammar may or may not treat as still a doc-comment — read lexical.ebnf §13) opens a `doc_comment`. Read until matching `*/`. If EOF reached before `*/`, emit `unknown` with span and a comment citing the unterminated-comment recovery rule.

Fill in `deal/src/keywords.zig` per RESEARCH §Pattern 2 lines 312-322:
- `pub const global_keywords = std.StaticStringMap(lexer.Tag).initComptime(.{ .{ "part", .kw_part }, .{ "def", .kw_def }, ... });` — one entry per reserved keyword from lexical.ebnf §5. Verify the comptime list compiles by adding a `comptime { _ = global_keywords; }` at module scope.
- Add a second table `pub const dealx_tag_names = std.StaticStringMap(TagSemantic).initComptime(...)` where `TagSemantic` is a small enum `{ system, subsystem, component, connect, expose, allocate, traceability, satisfy, validate, ... }` covering every contextual tag name from `dealx.ebnf`. The lexer does NOT use this table; it's exported so Plan 04 (`parser_dealx.zig`) can validate tag names with a comptime lookup instead of an `if/else` chain. Document this dual-purpose in a module comment.

Fill in `deal/src/source_map.zig`:
- Replace the stub `lineCol` with a real implementation: lazily build a `line_starts: ?[]const u32` table (sorted ascending byte offsets where each line begins, including 0 as the first entry). On first call, walk `self.source`, push the position right after every `\n` byte into a `std.ArrayList(u32) = .empty` (Zig 0.16.0 allocator-explicit shape — pass the allocator at every call). Cache the resulting slice on the struct.
- `pub fn lineCol(self: *SourceMap, allocator: std.mem.Allocator, offset: u32) struct { line: u32, col: u32 }` — binary-search the cached `line_starts` for the largest entry ≤ offset; return `(index + 1, offset - line_starts[index] + 1)` with 1-based line and column.

Anti-patterns to actively avoid (cite by ID in any code comment that touches them):
- A1 (RESEARCH line 519): don't hand-roll UTF-8 validation. Use `std.unicode.utf8ValidateSlice` if available; otherwise fail gracefully on a per-byte basis.
- A2 (RESEARCH line 523): do NOT use `ArrayList(T).init(allocator)`. Use `var list: std.ArrayList(T) = .empty;` and pass the allocator on every `.append(allocator, ...)` / `.deinit(allocator)` call.
- A3 (RESEARCH line 525): do NOT pre-tokenize into a `[]Token` slab. Stay lexer-on-demand.
  </action>
  <acceptance_criteria>
    - deal/src/lexer.zig declares variants for all of: tag_open, tag_close_open, tag_self_close, tag_close, l_bracket, r_bracket, lt, gt, template_head, template_middle, template_tail, doc_comment, ident, eof
    - deal/src/lexer.zig contains a `Mode` enum with the four 0.16.0 variants `deal_def`, `dealx_outer`, `dealx_tag`, `dealx_expr_brace` (UNCHANGED from Plan 01)
    - deal/src/keywords.zig contains literal `std.StaticStringMap` and `initComptime` and exactly 85 distinct map entries (verified via grep — count locked by lexical.ebnf §5 `_Keyword` production lines 459-481)
    - deal/src/source_map.zig contains `line_starts` and `binarySearch` (or `lower_bound`-equivalent — `std.sort.upperBound`/`lowerBound` in 0.16.0)
    - No occurrence of `ArrayList(T).init(` (literal) anywhere in this plan's modified files
    - No occurrence of `callconv(.C)` (uppercase) anywhere
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build 2>&1 — exits 0 (the lexer compiles and the library still links)</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && grep -v '^[[:space:]]*//' src/keywords.zig | grep -c '\.kw_' produces exactly 85 (one entry per locked spelling from lexical.ebnf §5 `_Keyword` production)</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build 2>&1 | tail -5</automated>
  </verify>
  <done>
    lexer.zig implements `next(mode)` and `peek(mode)` for all four modes per D-06; keywords.zig declares a comptime perfect-hash table with exactly 85 reserved/contextual keyword spellings from lexical.ebnf §5 `_Keyword` production (count locked at plan time and verified via awk on lines 459-481); source_map.zig lazily computes a line-start table for O(log n) byte→(line, col) conversion. Library still compiles (`zig build` exits 0). No banned 0.16.0 anti-patterns appear (no stored-allocator ArrayList, no callconv uppercase).
  </done>
</task>

<task type="auto">
  <name>Task 2.2: Token JSON emitter + unit tests + snapshot harness wiring</name>
  <read_first>
    - deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md (D-04 AST JSON schema-versioned, kind-tagged; D-15 Span byte-offsets)
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Pitfall 5: std.json.Stringify non-deterministic key order" (lines 598-608), §"Don't Hand-Roll" §"JSON writing" + §"Snapshot test diff" (lines 530-540), §"Open Questions" §3 (snapshot update workflow)
    - deal/src/lexer.zig (just written) and deal/src/json.zig stub
    - deal/build.zig (the `test-filter` mechanism from Plan 01)
    - spec/examples/showcase/ — the 19 showcase files are the snapshot corpus
    - spec/grammar/lexical.ebnf (mode-flip rules in §11)
  </read_first>
  <files>
    deal/src/json.zig,
    deal/tests/unit/lexer_mode_flip.zig,
    deal/tests/unit/lexer_keywords.zig,
    deal/tests/unit/lexer_comments.zig,
    deal/tests/unit/lexer_templates.zig,
    deal/tests/snapshots/tokens/.gitkeep,
    deal/build.zig
  </files>
  <action>
1. Add `pub fn emitTokens(allocator: std.mem.Allocator, source: []const u8, mode: ast.Mode) ![]const u8` to `deal/src/json.zig`. This function lexes the entire source via `Lexer.init(source)` driven by the file-level mode (`.deal_def` if `mode == .deal`; `.dealx_outer` if `mode == .dealx` — the file-level mode-flips into tag/expr modes happen ONLY in the parser, but for token snapshots we lex with the file's outer mode and rely on the parser to drive sub-modes; FOR SNAPSHOTS the simpler choice is to emit the outer-mode token stream and let parser snapshots in Plan 03/04 cover the inner-mode transitions). Output shape per D-04:
   ```
   {"v":1,"mode":"deal","tokens":[{"k":"kw_part","span":[0,4]},{"k":"ident","span":[5,12],"text":"Battery"},...]}
   ```
   - Always include `"v":1` first, then `"mode"`, then `"tokens"`. Within each token: `"k"` first, then `"span"`, then optional `"text"` (only present for `.ident`, `.int_literal`, `.real_literal`, `.string_literal`, `.template_head`, `.template_middle`, `.template_tail`, `.doc_comment` — tokens whose semantic content is the source slice).
   - Hand-roll the writer to guarantee deterministic key order per RESEARCH §Pitfall 5. Do NOT use `std.json.Stringify` for the top-level object (the langref bug #25233 is exactly this case).
   - Escape `"`, `\`, `\n`, `\r`, `\t`, and bytes < 0x20 in the `text` field per RFC 8259 §7. Non-ASCII UTF-8 bytes ≥ 0x80 are emitted as-is (no \uXXXX escaping needed for UTF-8 JSON — the spec allows raw UTF-8).

2. Create `deal/tests/unit/lexer_mode_flip.zig` — Zig test file with `test "lexer.mode_flip"` as the SOLE test name (so `-Dtest-filter=lexer.mode_flip` matches). Test cases:
   - Input `"[<sys>]"` lexed in `.deal_def` mode produces `[ l_bracket, lt, ident(sys), gt, r_bracket, eof ]` (5 tokens + eof).
   - Same input lexed in `.dealx_outer` mode produces `[ tag_open, ident(sys), tag_close, eof ]` (3 tokens + eof).
   - Input `"[</sys>]"` in `.dealx_outer` produces `[ tag_close_open, ident(sys), tag_close, eof ]`.
   - Input `"[</sys>]"` in `.deal_def` produces `[ l_bracket, lt, slash, ident(sys), gt, r_bracket, eof ]` (showing the slash is independent).
   Each case uses `try std.testing.expectEqual` for tag and `try std.testing.expectEqualSlices(u8, expected, source[span.start..span.end])` for the ident text.

3. Create `deal/tests/unit/lexer_keywords.zig` — `test "lexer.keywords"`:
   - For every keyword in `keywords.global_keywords`, lex its bytes in `.deal_def` mode and assert the produced tag matches the table entry.
   - Lex 5 non-keyword identifiers (`foo`, `Battery`, `mySymbol`, `_private`, `CamelCase42`) and assert each produces `.ident`.
   - Use `inline for (...)` over a comptime list to keep the test compact.

4. Create `deal/tests/unit/lexer_comments.zig` — `test "lexer.comments"`:
   - Source `"/** doc */part X"` → first token `doc_comment` with the right span; next `kw_part`; next `ident`.
   - Source `"/* block */part X"` → first token `block_comment` OR (if your implementation skips block comments) the first emitted token is `kw_part` and the block-comment bytes are consumed silently. Pick ONE behavior consistent with the grammar and assert it.
   - Source `"/** unterminated"` (no closing `*/`) → first token is `unknown` with span starting at the opening `/**` and ending at EOF; the test cites E0001 (lexer error code from D-16) as the diagnostic code that Plan 05 will emit for this case.

5. Create `deal/tests/unit/lexer_templates.zig` — `test "lexer.templates"`:
   - Source `` "`hello ${name} world`" `` → tokens `template_head("hello ")`, `template_open`, `ident(name)`, `template_close`, `template_tail(" world")`, `eof`. Verify the template_open / template_close are distinct from regular `l_brace`/`r_brace`.
   - Nested case: `` "`a${\"b\"+c}d`" `` → assert `template_head("a")`, `template_open`, `string_literal("\"b\"")`, `plus`, `ident(c)`, `template_close`, `template_tail("d")`.
   - Bound case: open 65 levels of nested `${${${...}}}` in a single source string (use a Zig compile-time-built string) and assert that lexing produces an `unknown` token at the 65th `${` AND the lexer does not panic. (This tests the threat T-02-DoS bound.)

6. Update `deal/build.zig`: add the unit tests to the test step. The 0.16.0 idiom is to create one `b.addTest(.{ .root_module = b.createModule(.{ .root_source_file = b.path("tests/unit/lexer_mode_flip.zig"), .target = target, .optimize = optimize }) })` per test file, threading the `test_filter` into each, and `test_step.dependOn(&b.addRunArtifact(t).step)`. Alternatively, create one umbrella test file `deal/tests/unit/_all.zig` that `@import`s each test file with `comptime { _ = @import("lexer_mode_flip.zig"); ... }` and wire the single addTest at `tests/unit/_all.zig`. Pick whichever is simpler; document the choice in SUMMARY. The umbrella-file approach lets `-Dtest-filter=lexer.mode_flip` match by test-name substring (`test "lexer.mode_flip"`).

7. Create `deal/tests/snapshots/tokens/.gitkeep` to commit the directory. The first run of `zig build test -Dtest-filter=lexer.snapshot` (Task 2.3) will populate this directory with one `.json` per showcase file.
  </action>
  <acceptance_criteria>
    - deal/src/json.zig contains `pub fn emitTokens` and writes a JSON object with `"v":1` literal as the first byte sequence after `{`
    - deal/tests/unit/lexer_mode_flip.zig declares exactly one `test "lexer.mode_flip"` block
    - deal/tests/unit/lexer_keywords.zig declares exactly one `test "lexer.keywords"` block
    - deal/tests/unit/lexer_comments.zig declares exactly one `test "lexer.comments"` block
    - deal/tests/unit/lexer_templates.zig declares exactly one `test "lexer.templates"` block
    - deal/build.zig wires each unit test file into the test step (verified by grep for the four file paths)
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=lexer.mode_flip 2>&1 — exits 0; output contains "1 passed"</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=lexer.keywords 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=lexer.comments 2>&1 — exits 0</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=lexer.templates 2>&1 — exits 0</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=lexer 2>&1 | tail -10</automated>
  </verify>
  <done>
    Token JSON emitter produces D-04-compliant deterministic JSON; four unit tests cover the four lexer feature gates from RESEARCH §Validation Architecture (lexer.mode_flip, lexer.keywords, lexer.comments, lexer.templates); each is invocable as `zig build test -Dtest-filter=<area>` and each passes; build.zig is wired to discover the tests.
  </done>
</task>

<task type="auto">
  <name>Task 2.3: Showcase token snapshots (the 19-file lex gate)</name>
  <read_first>
    - deal/tests/unit/ (test files just written; we add `lexer_snapshot.zig` alongside them)
    - deal/src/json.zig (emitTokens just added)
    - spec/examples/showcase/ — list contents with `find spec/examples/showcase -name "*.deal" -o -name "*.dealx" | sort`; 15 .deal + 4 .dealx, paths are stable per CONTEXT §Code Context line 124
    - deal/.planning/phases/01-zig-compiler-core/01-RESEARCH.md §"Don't Hand-Roll" §"Snapshot test diff" (lines 530-540), §"Open Questions" §3 (snapshot update workflow)
    - deal/.planning/phases/01-zig-compiler-core/01-VALIDATION.md (manual-only verification table at lines 70-72)
  </read_first>
  <files>
    deal/tests/unit/lexer_snapshot.zig,
    deal/tests/snapshots/tokens/showcase__packages__vehicle__battery.deal.json,
    deal/tests/snapshots/tokens/showcase__packages__vehicle__motor.deal.json,
    deal/tests/snapshots/tokens/showcase__packages__vehicle__behaviors.deal.json,
    deal/tests/snapshots/tokens/showcase__packages__vehicle__components.deal.json,
    deal/tests/snapshots/tokens/showcase__packages__vehicle__index.deal.json,
    deal/tests/snapshots/tokens/showcase__packages__interfaces__electrical.deal.json,
    deal/tests/snapshots/tokens/showcase__packages__interfaces__thermal.deal.json,
    deal/tests/snapshots/tokens/showcase__packages__interfaces__connections.deal.json,
    deal/tests/snapshots/tokens/showcase__packages__interfaces__index.deal.json,
    deal/tests/snapshots/tokens/showcase__packages__requirements__system.deal.json,
    deal/tests/snapshots/tokens/showcase__packages__requirements__needs.deal.json,
    deal/tests/snapshots/tokens/showcase__packages__requirements__index.deal.json,
    deal/tests/snapshots/tokens/showcase__packages__use-cases__driving.deal.json,
    deal/tests/snapshots/tokens/showcase__packages__use-cases__charging.deal.json,
    deal/tests/snapshots/tokens/showcase__packages__use-cases__index.deal.json,
    deal/tests/snapshots/tokens/showcase__model__vehicle.dealx.json,
    deal/tests/snapshots/tokens/showcase__model__traceability.dealx.json,
    deal/tests/snapshots/tokens/showcase__model__variants__sedan.dealx.json,
    deal/tests/snapshots/tokens/showcase__model__variants__performance.dealx.json
  </files>
  <action>
1. Create `deal/tests/unit/lexer_snapshot.zig` with `test "lexer.snapshot"` (SOLE test name, so the filter matches):
   - Define a comptime list `const showcase_files = [_][]const u8{ "packages/vehicle/battery.deal", "packages/vehicle/motor.deal", ..., "model/variants/performance.dealx" };` enumerating all 19 paths (relative to `tests/showcase/`).
   - For each file: read via `std.fs.cwd().readFileAlloc` (or `openFile + readToEndAlloc` if the former isn't available in 0.16.0 — verify per RESEARCH assumption A3) into the test allocator. Detect mode by extension. Call `json.emitTokens(allocator, source, mode)` to produce the actual JSON bytes.
   - Compute the expected snapshot path: `"tests/snapshots/tokens/showcase__" ++ path_with_slashes_replaced_by_double_underscore ++ ".json"`.
   - Behavior:
     - If env var `UPDATE_SNAPSHOTS=1` is set (read via `std.process.getEnvVarOwned`), write the actual JSON to the snapshot path and pass the test. Print `[wrote snapshot] <path>` to stderr.
     - Otherwise: read the committed snapshot file, assert `std.testing.expectEqualStrings(expected, actual)`. On mismatch, write the actual to `<path>.actual` for diff inspection and fail the test.
   - Assert separately that the actual JSON contains ZERO occurrences of `"k":"unknown"` (the success criterion #1 — "zero UNKNOWN tokens"). This is a stronger guard than the snapshot — it catches lexer regressions even if a developer accidentally updates snapshots with new unknown tokens.

2. Wire `lexer_snapshot.zig` into `deal/build.zig` test step (same pattern as Task 2.2; if using the umbrella file approach, add it to `tests/unit/_all.zig`).

3. Run `cd deal && UPDATE_SNAPSHOTS=1 zig build test -Dtest-filter=lexer.snapshot` ONCE to generate the 19 snapshot files. Visually inspect at least 3 (`battery.deal`, `vehicle.dealx`, `traceability.dealx`) to confirm:
   - Top-level shape: `{"v":1,"mode":"deal"|"dealx","tokens":[...]}`
   - No `"k":"unknown"` anywhere in any of the 19 files
   - Spans are monotonically non-decreasing (start of token N+1 ≥ end of token N)
   - The 15 `.deal` files all have `"mode":"deal"`; the 4 `.dealx` all have `"mode":"dealx"`

4. Commit the 19 snapshot files to git so subsequent runs assert byte-equality. Document the snapshot-update workflow (`UPDATE_SNAPSHOTS=1 zig build test`) in the SUMMARY for downstream plans to reuse (Plan 03 will add `lexer_snapshot.zig` → `ast_snapshot.zig` with the same env var convention).

5. Verify the gate criterion programmatically: count `"k":"unknown"` across all 19 snapshots. Required: 0. Use a shell command at acceptance time: `grep -l '"k":"unknown"' deal/tests/snapshots/tokens/*.json` MUST produce empty output.
  </action>
  <acceptance_criteria>
    - All 19 snapshot files exist under deal/tests/snapshots/tokens/ with the exact double-underscore-separated names listed in <files>
    - Each snapshot file starts with the byte sequence `{"v":1,"mode":"`
    - 15 snapshot files contain `"mode":"deal"`; 4 contain `"mode":"dealx"` (verified via grep count)
    - ZERO snapshot files contain the substring `"k":"unknown"` (the lexer gate criterion from CONTEXT.md / ROADMAP success criterion #1)
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=lexer.snapshot 2>&1 — exits 0; output contains "lexer.snapshot... OK"</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && grep -l '"k":"unknown"' tests/snapshots/tokens/*.json 2>&1 — produces zero lines of output</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && ls tests/snapshots/tokens/*.json | wc -l — produces exactly 19</automated>
    - <automated>cd /Users/dunnock/projects/deal-lang/deal && grep -c '"mode":"deal"' tests/snapshots/tokens/*.deal.json | wc -l — produces 15</automated>
  </acceptance_criteria>
  <verify>
    <automated>cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=lexer.snapshot 2>&1 | grep -E "passed|OK" | tail -3</automated>
  </verify>
  <done>
    All 19 showcase files lex through the four-mode dispatch with zero `unknown` tokens (the locked Phase 1 success criterion #1). Snapshots are committed; subsequent runs byte-compare. The snapshot-update workflow (`UPDATE_SNAPSHOTS=1 zig build test`) is documented for Plans 03/04 to reuse. The Phase 1 lexer gate (REQ-phase-1-1-lexer) is met.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Source bytes → lexer | Caller-provided source bytes are walked byte-by-byte; pathological inputs (unbounded `${` nesting, unterminated templates, very long lines) can attempt resource exhaustion. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-02-01 | DoS | Template `${}` interpolation depth | mitigate | Bound `template_depth` stack at 64 entries (RESEARCH line 1007). Past 64, emit `unknown` token; lexer continues without panic. Unit-tested in `lexer.templates` at depth 65. |
| T-02-02 | DoS | Unterminated `/**` doc-comment scanning to EOF | mitigate | The doc-comment scanner emits a single `unknown` token spanning from `/**` to EOF. No retry loop; one pass, bounded by source length. |
| T-02-03 | Information Disclosure | Diagnostic strings containing source bytes | mitigate | The lexer does not produce diagnostics — only Tag values + Spans. Source bytes never enter format strings (RESEARCH line 1012). Diagnostic strings are Plan 05's responsibility. |
| T-02-04 | Tampering | Locale-dependent keyword comparison | mitigate | Keyword lookup uses `std.StaticStringMap.get(slice)` which performs byte-exact comparison via the comptime perfect-hash table. No `std.ascii.eqlIgnoreCase` (RESEARCH line 1013). |
| T-02-SC | Tampering | No new external dependencies | accept | This plan introduces no new packages — only Zig 0.16.0 std-lib (`std.StaticStringMap`, `std.fs`, `std.testing`, `std.unicode.utf8ValidateSlice`). Package legitimacy audit unchanged from Plan 01. |
</threat_model>

<verification>
1. `cd /Users/dunnock/projects/deal-lang/deal && zig build` exits 0 — library still links.
2. `cd /Users/dunnock/projects/deal-lang/deal && zig build test` exits 0 — all unit tests + snapshot tests pass in one invocation.
3. `cd /Users/dunnock/projects/deal-lang/deal && zig build test -Dtest-filter=lexer.snapshot` exits 0 and verifies all 19 snapshots byte-match.
4. `cd /Users/dunnock/projects/deal-lang/deal && grep -l '"k":"unknown"' tests/snapshots/tokens/*.json` produces no output.
5. `cd /Users/dunnock/projects/deal-lang/deal && cargo test --manifest-path tests/ffi/Cargo.toml` still passes (Plan 01's stub FFI tests remain green — the lexer changes did not break the ABI).
</verification>

<success_criteria>
- Phase 1 ROADMAP success criterion #1 met: "A Zig lexer tokenizes all 19 showcase files with zero UNKNOWN tokens; lexer mode selection by file extension works (`[< >]` activates only in `.dealx`)."
- REQ-phase-1-1-lexer acceptance condition met: `libdeal` lexer module produced; token snapshot tests for all 19 showcase files pass with zero UNKNOWN tokens.
- D-06 (four-mode dispatch, lexer-on-demand) is observable through `lexer.mode_flip` test.
- D-15 (Span = byte offsets only, u32 start + u32 end) is enforced by the `extern struct` declaration.
- D-16 error-code range E0001..E0099 is reserved (the lexer doesn't emit diagnostics in this plan but the namespace is documented for Plan 05 to honor).
- The four Validation Architecture rows from RESEARCH lines 942-946 (lexer.snapshot, lexer.mode_flip, lexer.keywords, lexer.comments, lexer.templates) each have a green automated test command.
</success_criteria>

<output>
Create `.planning/phases/01-zig-compiler-core/01-02-SUMMARY.md` when done, recording:
- Confirm `keywords.zig` ships exactly 85 entries (the locked count from `lexical.ebnf` §5 `_Keyword` production lines 459-481 pre-resolved at plan time; the line-749 summary comment that says "76" is stale and is deferred to a separate spec PR for correction). If a future grammar revision changes the count, both this plan's acceptance criteria and `01-RESEARCH.md` must be re-locked together.
- Whether `std.unicode.utf8ValidateSlice` exists in 0.16.0 with that exact name (resolves RESEARCH assumption A1).
- Whether `std.fs.cwd().readFileAlloc` was used or `openFile().readToEndAlloc` (resolves RESEARCH assumption A3).
- Whether doc-comment vs block-comment handling chose "emit doc, skip block" or "emit both" (Plan 03 needs to know).
- The chosen test wiring pattern (per-file `addTest` vs umbrella `tests/unit/_all.zig`).
- File paths of all 19 committed snapshot files (so Plan 03's AST snapshot plan can mirror the naming convention).
</output>
</content>
</invoke>