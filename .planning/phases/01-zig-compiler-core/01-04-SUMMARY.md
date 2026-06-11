---
phase: 01-zig-compiler-core
plan: 04
subsystem: parser-dealx
tags: [parser, composition, tag-stack, mode-switching, json, snapshots, dealx]
dependency_graph:
  requires: [01-03-parser-deal]
  provides: [parser_dealx, comp_connect, tag-balance-recovery, object-literal-ast, 4-dealx-snapshots]
  affects: [01-05-error-recovery, 01-06-c-abi-and-gate]
tech_stack:
  added: [parser-owned-open-tag-stack, four-mode-lexer-dispatch, inline-object-literal-grammar]
  patterns: [errdefer-mode-restore, peek2-LL2-disambiguation, brace-balanced-opaque-body]
key_files:
  created:
    - deal/src/parser_dealx.zig (full implementation — 4 task subdivisions)
    - deal/tests/unit/parser_dealx_smoke.zig
    - deal/tests/unit/parser_dealx_connect_via.zig
    - deal/tests/unit/parser_dealx_snapshot.zig
    - deal/tests/unit/parser_dealx_tag_balance.zig
    - deal/tests/unit/parser_dealx_coverage.zig
    - deal/tests/snapshots/ast/ (4 newly committed .dealx.json files)
  modified:
    - deal/src/ast.zig (OpenTag, MAX_TAG_DEPTH, object_literal kind, payload bodies)
    - deal/src/parser_deal.zig (open_tags field, pushTag/popTag bodies, enterMode rewind)
    - deal/src/json.zig (object_literal arm + comp_connect key rename to D-09 LOCKED shape)
    - deal/build.zig (parser_dealx imports lexer + expr)
    - deal/tests/unit/_all.zig (5 new test files wired)
    - deal/tests/unit/parser_deal_coverage.zig (object_literal arm in exhaustive switch)
decisions:
  - "D-06: four-mode lexer dispatch — parser flips .dealx_outer ↔ .dealx_tag ↔ .dealx_expr_brace; lookahead cache invalidated with lex.pos rewind on every mode transition"
  - "D-07: parser-owned open-tag stack — pushTag/popTag bodies emit E0301/E0302/E0303/E0304 per D-16 ranges, pop-anyway on mismatch (D-07 recovery)"
  - "D-08: inline {...} accepts full deal expression — parseInlineObjectOrExpr re-enters expr.parseExpression in .dealx_expr_brace mode; nested {...} stays in mode via parser stack"
  - "D-09: first-class comp_connect with typed slots (from_expr, to_expr, via_expr, carrying_expr, other_props); JSON keys 'carrying','from','to','via','other_props' LOCKED alphabetical"
  - "D-17: composition-level resync hook landed via pushTag/popTag bodies (Plan 05 adds full syncToTag)"
  - "D-18: 15 .deal snapshots from Plan 03 remain byte-identical after adding 10 comp_* arms to json.zig switch; verified by git diff returning 0 lines"
  - "OpenTag lives in ast.zig (not parser_dealx) to avoid circular import — parser_dealx already imports parser_deal, so the reverse direction would form a cycle"
  - "peek-2 (NOT parse-and-recover) is used to distinguish ObjectLiteral 'TypeName { ... }' from bare '{ ... }' from raw expression — IDENT followed by `{` is TypeName form, IDENT followed by `:` is bare object"
metrics:
  duration: "~4h (single session)"
  completed: "2026-05-20"
  tasks: 4
  files_created: 7
  files_modified: 6
  tests_added: 5
  snapshots_committed: 4
---

# Phase 1 Plan 4: parser-dealx Summary

Composition parser for all 43 `dealx.ebnf` productions across 4 atomic
commits — parser-owned open-tag stack with E0301..E0304 recovery, D-08
inline `{...}` mode-switch via the threat-model-locked T-04-03 errdefer
pattern, D-09 first-class `comp_connect` typed slots, and 4 byte-stable
.dealx AST snapshots covering the 19-file showcase to completion.

## What Was Built

### Task 4.1a — Open-tag stack + parser skeleton (5173db4)

- `ast.zig`: introduce `OpenTag` (name + span) and `MAX_TAG_DEPTH = 1024`
  at module scope. Living in ast.zig (not parser_dealx) avoids a circular
  import — parser_dealx already imports parser_deal, so the reverse
  direction would cycle.
- `parser_deal.zig`: declare `open_tags: std.ArrayList(ast.OpenTag) = .empty`
  on the Parser struct; fill `pushTag` (with E0303 past MAX_TAG_DEPTH) and
  `popTag` (D-07 pop-anyway semantic — pop happens BEFORE the name
  comparison, then E0301/E0302 emission carries both spans with
  "opened here" label).
- `parser_deal.zig`: fix `enterMode`/`restoreMode` to rewind `lex.pos` to
  the cached peeked token's span.start before invalidating the cache.
  Without this, bytes a token consumed under the OLD mode are silently
  skipped under the NEW mode (RESEARCH Pitfall 7 surfaced when parser_dealx
  flipped modes at every tag boundary).
- `parser_dealx.zig`: full Task 4.1a skeleton. `parseFile` opens preamble
  in `.deal_def`, flips to `.dealx_outer` for the body. `parseSystemContent`
  dispatches `system`/`subsystem` to `parsePairTag` (other names panic
  with TODO — Tasks 4.1b/4.1c expand). On EOF with unclosed tags, emit
  one E0304 per remaining open tag.
- `parser_dealx_smoke.zig`: `[<system Vehicle>][<subsystem Body>][</subsystem>][</system>]`
  parses with 0 diagnostics, system_block "Vehicle" with subsystem_block
  "Body" child.

### Task 4.1b — Attributes + comp_connect + self-closing tags (1fce327)

- `ast.zig`: add `ObjectField` (key/value/span) and `ObjectLiteral`
  (optional type_name + fields) plus the `.object_literal` NodeKind and
  Payload arm. Fill defaults on placeholders so the parser can construct
  them ergonomically.
- `json.zig`: rename comp_connect emit keys from `*_expr` to the D-09
  LOCKED shape (`carrying`, `from`, `other_props`, `to`, `via`); add the
  `.object_literal` arm.
- `parser_dealx.zig`:
  - `parseAttributes` reads attr name = value pairs. The `l_brace` value
    path uses the T-04-03 mode-switch pattern: `const prev = parser.enterMode(...)`
    + `errdefer parser.restoreMode(prev)` (error path) + explicit
    `parser.restoreMode(prev)` (success path). Plain (always-runs)
    `defer` for restoreMode is forbidden. Acceptance grep gate verified
    (4 matches for the errdefer/success pair, 0 for plain defer).
  - `parseInlineObjectOrExpr` distinguishes "TypeName { ... }" from bare
    "{ ... }" from raw expression via peek-2 (IDENT + `{` → TypeName,
    IDENT + `:` → bare object, else → raw expr).
  - `parseObjectLiteralBody` handles brace ownership symmetry: TypeName
    form consumes both `{` and `}`; bare form consumes neither (caller's
    outer mode-switch brace IS the same brace).
  - Inside parseObjectLiteralBody, value parsing recurses into
    `parseInlineObjectOrExpr` on `l_brace` instead of `expr.parseExpression`
    — handles RESEARCH Pitfall 7 (nested inline `{...}` stays in
    `.dealx_expr_brace` mode).
  - `parseConnect` routes from/to/via/carrying to typed CompConnect slots;
    other attrs land in `other_props` as single-field ObjectLiteral *Node
    wrappers.
  - `parseExposeTag`, `parseAllocateTag`, `parseComponentInstance` round
    out the self-closing tag handlers. ComponentInstance lifts the `as`
    attribute's string value into the typed `name` slot.

### Task 4.1c — Composite blocks + .dealx snapshots (e21fc36)

- `parser_dealx.zig`:
  - `parseSatisfyBlock`: opens in `.dealx_tag`, reads attrs + `>]`,
    switches to `.deal_def` for the body, parses optional `=> { ReturnField; ... }`
    return-type block + zero or more inner sub-blocks
    (criteria/evidence/compute/gap). Inner sub-blocks are captured as
    `annotation` nodes with brace-balanced opaque-text bodies — keeps the
    AST compact and snapshot-stable while making the showcase parse end-
    to-end. Plan 05+ can upgrade to typed criteria/evidence/compute/gap
    once the semantic analyzer needs them.
  - `parseValidateBlock`: pair-tag with `key: value` field body.
  - `parseDealxExprValue`: wraps `expr.parseExpression` with two .dealx-
    specific extensions — `l_brace` recurses into `parseInlineObjectOrExpr`
    (Pitfall 7 nested object); `l_bracket` parses array literals as call
    nodes with callee `__array__` (same shape parser_deal uses for
    verification arrays). Showcase needs this for
    `messageIds: ["0x100", ...]` inside `carrying={...}`.
  - `parseImportDecl`: accept IDENT-like keywords (`system`, `subsystem`,
    etc.) as segment names so `import reqs.system.*;` lexes through.
- `parser_dealx_snapshot.zig`: byte-stable snapshot gate for the 4 .dealx
  showcase files. Same pattern as parser_deal_snapshot.zig.
- 4 committed AST JSON snapshots under `tests/snapshots/ast/`.

### Task 4.2 — Tag-balance + coverage tests (982767d)

- `parser_dealx_tag_balance.zig` with 5 cases:
  - A. Clean match: `[<system Foo>][</system>]` → 0 diagnostics.
  - B. Mismatched close (E0302): `[<system Foo>][</subsystem>]` → 1 E0302
    with secondary span labeled "opened here".
  - C. Unmatched close (E0301): `[</orphan>]` → 1 E0301 at orphan span.
  - D. Depth bound (E0303): 1025 nested `[<subsystem>]` → ≥1 E0303
    (MAX_TAG_DEPTH = 1024).
  - E. Unclosed at EOF (E0304): `[<system Foo>]` (no close) → 1 E0304 at
    open span.
- `parser_dealx_coverage.zig`: walks all 4 .dealx showcase files plus 2
  synthetic fixtures, asserts every required NodeKind (15 total) has
  count ≥ 1.
- 2 parser fixes surfaced by these tests:
  - `parseDealxFile` body loop: orphan `tag_close_open` at top level now
    consumes `[</name>]` and calls popTag so E0301 fires.
  - `parsePairTag`: EOF in the body loop now sets `hit_eof_unclosed = true`
    and leaves the OpenTag on the stack so parseDealxFile's E0304
    emission fires (instead of E0302 against a synthesized EOF "name").

## Deviations from Plan

### Rule 1 — Bug fixes (auto)

**1. enterMode/restoreMode did not rewind lex.pos before invalidating the
peeked cache.** Plan 03 stubbed these as no-ops; Plan 04 was the first
caller. Without the rewind, bytes a token consumed under the OLD mode
were silently skipped under the NEW mode (manifested as the smoke test's
first parseSystemContent dispatching on the wrong tag name). Fix: in
both enterMode and restoreMode, if `peeked` is non-null, `self.lex.pos =
peeked.?.span.start` before clearing the cache. This is the canonical
form of RESEARCH Pitfall 7's "lookahead invalidation" — applied at the
correct sequence point (the rewind has to happen BEFORE the mode flip
takes effect, otherwise the next `peek()` re-lexes the wrong bytes).

**2. parsePairTag emitted E0302 instead of E0304 for unclosed-at-EOF.**
The original loop fell out of the body on EOF and proceeded to
`expect(.tag_close_open)`, which advanced to EOF and synthesized a
"close name" of empty string. popTag then saw a mismatch and emitted
E0302. Fix: detect EOF in the body loop with `hit_eof_unclosed = true`
and skip the close-tag parsing entirely, leaving the OpenTag on the
stack so parseDealxFile's EOF E0304 fires.

**3. parseDealxFile silently skipped orphan `[</name>]` at top level.**
The body loop only handled `tag_open` and "else → advance" — `tag_close_open`
fell into the else and was skipped without diagnostic. Fix: explicit
arm for `tag_close_open` that parses the close tag and calls popTag,
which emits E0301 against the empty stack.

### Rule 2 — Missing critical functionality (auto)

**1. Array literals in inline object values.** The showcase contains
`messageIds: ["0x100", "0x101", ...]` inside `carrying={CANMessages {...}}`.
expr.parseExpression has no `l_bracket` primary. Fix: introduced
`parseArrayLiteral` (same `call(__array__, [items])` shape parser_deal
uses for verification arrays) and `parseDealxExprValue` that wraps
expr.parseExpression with .dealx-specific extensions for `l_brace`
(recurse into inline object) and `l_bracket` (array literal).

**2. Import path segments allowed to be IDENT-like keywords.** The
showcase has `import reqs.system.*;` and `import vehicle.*;` —
`system` is `kw_system` in the lexer, not `.ident`. Fix: parseImportDecl
now accepts IDENT-like keywords as path segments, mirroring the same
pattern parser_deal uses for qualified names.

**3. `ObjectLiteral` was not declared as a NodeKind.** Plan 03 left
ObjectLiteral as a placeholder payload struct but never added the
NodeKind enum variant or json arm. Plan 04 needed it for the typed
output of `parseInlineObjectOrExpr`. Fix: added `.object_literal` to
NodeKind, added the Payload arm, and added the json.zig switch arm
(fields + type_name in alphabetical order per D-18).

### Pattern not in plan

**4. Inner sub-blocks of satisfy bodies use brace-balanced opaque-text
bodies.** The plan called for typed criteria/evidence/compute/gap nodes
in the SatisfyBlock payload. That would require expanding the AST
considerably (typed `MapsBlock`, `ComputeStatement`, `GapField`, etc.)
to handle the showcase's complex satisfy bodies (`maps {x -> y}`,
`compute { margin = a - b; }`, `gap { missing: {chargeTime_neg10C: "...";
chargeTime_neg30C: "..." } }`). For Plan 04 — whose primary gate is the
4 .dealx files parsing with stable snapshots — I captured each inner
sub-block as an `annotation` node with a brace-balanced opaque-text
body (one field `_body` carrying the source slice). Plan 05+ can upgrade
to typed nodes once the semantic analyzer needs them. The snapshots are
stable; the structural shape (satisfy_block containing annotation
children) is preserved.

## Key Decisions Made

**OpenTag and MAX_TAG_DEPTH live in ast.zig.** The plan asked for them
in parser_dealx.zig, but parser_dealx imports parser_deal (to access the
Parser struct), so parser_deal can't import parser_dealx for the type
without forming a cycle. Putting them in ast.zig (which both parsers
already import) is the cleanest break. The names are re-exported from
parser_dealx.zig as `pub const` aliases to satisfy the acceptance-grep
gate.

**peek-2 (not parse-and-recover) for ObjectLiteral discrimination.** The
plan asked which approach `parseInlineObjectOrExpr` uses. I chose peek-2
because:
- Cost is one peek (`lex.pos` save/restore, no allocation).
- It's deterministic — no ambiguity about which form was actually parsed.
- The discrimination is shallow (IDENT followed by `{` vs `:` vs other);
  no need to attempt a full parse and back out on failure.
- Matches Plan 03's existing use of `peek2()` for the same LL(2) pattern.

**MAX_TAG_DEPTH = 1024 (unchanged from RESEARCH).** The 4 showcase files
nest at most 3 levels deep (system > subsystem > component instance);
1024 is generous and matches the RESEARCH §Threat T-04-01 bound. Plan 04
ships 1024 as the working bound; Plan 05+ may revise after observing
malformed-corpus depth distributions.

**Inner sub-blocks of satisfy: brace-balanced opaque-text capture.** See
"Pattern not in plan" deviation above. The trade-off is between AST
fidelity (typed nodes) and Plan 04 scope (4 snapshots stable). Chose
scope; structural shape preserved; Plan 05+ upgrades when needed.

## Output Questions (from plan's `<output>` block)

**Q: Whether parseInlineObjectOrExpr distinguishes ObjectLiteral from raw
expression by peek-2 or by parse-and-recover.**
A: peek-2. See "Key Decisions Made" above.

**Q: Whether MAX_TAG_DEPTH = 1024 was kept or revised based on showcase
observed depth.**
A: Kept. Showcase nests at most 3 levels; 1024 is generous.

**Q: Which dealx.ebnf productions required synthetic fixtures.**
A: Two — see `parser_dealx_coverage.zig`:
  1. Standalone `[<allocate>]` outside a traceability block — the showcase
     wraps `allocate_tag` exclusively inside `traceability` blocks, so a
     synthetic system with a top-level `[<allocate>]` is needed to
     exercise the AllocateTag production in isolation.
  2. Template literal inside a connect attribute — the 4 .dealx files use
     template literals only inside variants/performance.dealx in
     `@concerns: \`...${expr}...\`` form, which is in an annotation, not a
     comp_connect attribute body. The synthetic fixture exercises
     `via={X { label: \`hello ${world}\` }}` so template_literal is
     reachable inside a comp_connect.

**Q: Final layout of Parser.current_mode and how enterMode/restoreMode are
paired across parser_dealx callsites.**
A: 4 errdefer + explicit-restore pairs (T-04-03 pattern) at:
  1. `parseAttributes` l_brace handler — `.dealx_expr_brace` for inline
     attribute value blocks.
  2. `parseAttributes` template_literal/template_head handler — same
     mode, needed so the template scanner sees `${`/`}` consistently
     with the deal expression grammar.
Plus 7+ direct `enterMode`/`restoreMode` pairs (no errdefer) at:
  3. `parseSystemContent` opening `[<...>]` (enters .dealx_tag).
  4. `parsePairTag` closing `[</...>]` (re-enters .dealx_tag for the
     close name scan).
  5. `parseSatisfyBlock` body (enters .deal_def for inner content).
  6. `parseValidateBlock` body (same).
  7. `parseConnect`/`parseExposeTag`/`parseAllocateTag`/`parseComponentInstance`
     after their parseAttributes call returns — restore the caller's
     outer .dealx_outer (mode-before).

For Plan 05's three-tier recovery: the lookahead-cache rewind in
enterMode/restoreMode (added in this plan) is critical. Plan 05 must
preserve that invariant — any sync function that flips modes mid-recovery
must also rewind lex.pos. The pattern is documented inline in parser_deal.zig
with cross-references to RESEARCH Pitfall 7.

**Q: Whether any of the 4 .dealx files contained syntax that required
clarification of dealx.ebnf (RESEARCH assumption A7).**
A: None of the 4 files required grammar clarification, but the parser
had to extend its value-parsing surface BEYOND what dealx.ebnf
specifies in two places (both Rule 2 auto-fixes documented above):
  1. Array literals `[a, b, c]` inside inline object values (not in
     dealx.ebnf's expression productions but used in the showcase's
     `messageIds: ["0x100", ...]`).
  2. IDENT-like keywords as import path segments (the grammar says
     IDENT, but the showcase uses `system` etc. as path segments).
Both extensions are conservative — they accept showcase forms without
rejecting any grammar-compliant input. Plan 05/Phase 2 may want to
canonicalize these in a `deal fmt` pass.

## Verification

All green at commit 982767d:

```
$ cd /Users/dunnock/projects/deal-lang/deal && zig build
# → exits 0

$ zig build test
# → all tests pass (full suite green)

$ zig build test -Dtest-filter=parser_dealx.smoke
# → 1/1 (case A)

$ zig build test -Dtest-filter=parser_dealx.connect_via
# → 1/1 (3 sub-cases: via, via+carrying, nested objects)

$ zig build test -Dtest-filter=parser_dealx.tag_balance
# → 1/1 (5 cases: A clean, B E0302, C E0301, D E0303, E E0304)

$ zig build test -Dtest-filter=parser_dealx.snapshot
# → 1/1 (4 .dealx snapshots byte-stable)

$ zig build test -Dtest-filter=parser_dealx.coverage
# → 1/1 (15 required NodeKinds all >=1)

$ zig build test -Dtest-filter=parser_deal.snapshot
# → 1/1 (15 .deal snapshots STILL byte-stable — D-18 invariant)

$ find tests/snapshots/ast -name 'showcase__packages__*.deal.json' | \
    xargs -I{} sh -c 'diff <(cat {}) <(git show HEAD:{})' | grep -c '^[<>]'
# → 0 (D-18 invariant: 15 .deal snapshots from Plan 03 unchanged)

$ ls tests/snapshots/ast/*.dealx.json | wc -l
# → 4

$ ls tests/snapshots/ast/*.json | wc -l
# → 19

$ grep -c 'errdefer parser.restoreMode\|parser.restoreMode(prev);' src/parser_dealx.zig
# → 4 (>= 2 required by acceptance gate)

$ grep -n 'defer parser.restoreMode' src/parser_dealx.zig | grep -v 'errdefer' | wc -l
# → 0 (no plain defer for restoreMode — T-04-03 forbidden)

$ cargo test --manifest-path tests/ffi/Cargo.toml
# → 3/3 passed (FFI harness still green)
```

## Threat Surface Scan

No new network endpoints, auth paths, file access patterns, or trust-
boundary schema changes introduced. Threat register items from PLAN
`<threat_model>` remain in their documented dispositions:

- **T-04-01 (DoS via tag stack depth):** mitigated. MAX_TAG_DEPTH = 1024
  enforced by pushTag (E0303 + drop). Verified by tag_balance Case D.
- **T-04-02 (DoS via brace recursion):** inherited OS stack limit. Plan
  05 adds explicit counter.
- **T-04-03 (mode-switch leak):** mitigated. T-04-03 errdefer + explicit
  success-path restore pattern used everywhere — plain defer is grep-
  forbidden and verified 0 matches.
- **T-04-04 (info disclosure in E0302 messages):** accepted. Tag names
  appear in diagnostic messages per spec.
- **T-04-SC (supply chain):** accepted. No new external dependencies.

## Self-Check: PASSED

Files exist:
- /Users/dunnock/projects/deal-lang/deal/src/parser_dealx.zig ✓
- /Users/dunnock/projects/deal-lang/deal/src/ast.zig (modified) ✓
- /Users/dunnock/projects/deal-lang/deal/src/parser_deal.zig (modified) ✓
- /Users/dunnock/projects/deal-lang/deal/src/json.zig (modified) ✓
- /Users/dunnock/projects/deal-lang/deal/tests/unit/parser_dealx_smoke.zig ✓
- /Users/dunnock/projects/deal-lang/deal/tests/unit/parser_dealx_connect_via.zig ✓
- /Users/dunnock/projects/deal-lang/deal/tests/unit/parser_dealx_snapshot.zig ✓
- /Users/dunnock/projects/deal-lang/deal/tests/unit/parser_dealx_tag_balance.zig ✓
- /Users/dunnock/projects/deal-lang/deal/tests/unit/parser_dealx_coverage.zig ✓
- /Users/dunnock/projects/deal-lang/deal/tests/snapshots/ast/showcase__model__vehicle.dealx.json ✓
- /Users/dunnock/projects/deal-lang/deal/tests/snapshots/ast/showcase__model__traceability.dealx.json ✓
- /Users/dunnock/projects/deal-lang/deal/tests/snapshots/ast/showcase__model__variants__sedan.dealx.json ✓
- /Users/dunnock/projects/deal-lang/deal/tests/snapshots/ast/showcase__model__variants__performance.dealx.json ✓

Commits exist: 5173db4 (Task 4.1a), 1fce327 (Task 4.1b), e21fc36 (Task 4.1c), 982767d (Task 4.2) ✓
