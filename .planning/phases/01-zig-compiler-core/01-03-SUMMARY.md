---
phase: 01-zig-compiler-core
plan: 03
subsystem: parser-deal
tags: [parser, ast, pratt, json, snapshots]
dependency_graph:
  requires: [01-02-lexer]
  provides: [deal_parse, deal_ast_json, parser_deal, expr, ast-tagged-union]
  affects: [01-04-parser-dealx, 01-05-error-recovery, 01-06-c-abi-and-gate]
tech_stack:
  added: [Pratt-expression-parser, recursive-descent-parser, arena-allocated-AST, hand-rolled-JSON-emitter]
  patterns: [tagged-union-AST, peek2-LL2-lookahead, explicit-error-set-annotations]
key_files:
  created:
    - deal/src/ast.zig (full rewrite — 72-variant NodeKind + exhaustive Payload union)
    - deal/src/expr.zig (Pratt parser, 8 levels OR=10..postfix=80)
    - deal/src/parser_deal.zig (recursive-descent parser, all 87 .deal productions)
    - deal/tests/unit/expr_precedence.zig
    - deal/tests/unit/parser_deal_snapshot.zig
    - deal/tests/unit/parser_deal_coverage.zig
    - deal/tests/snapshots/ast/ (15 committed .deal.json files)
  modified:
    - deal/src/parser.zig (dispatcher for .deal/.dealx)
    - deal/src/parser_dealx.zig (signature updated to match new interface)
    - deal/src/json.zig (real emitAst implementation)
    - deal/src/lib.zig (wire real parser, update test assertions)
    - deal/build.zig (add missing module imports)
    - deal/tests/ffi/tests/parse.rs (update root assertion for real parser)
    - deal/tests/unit/_all.zig (add 3 new test files to umbrella)
decisions:
  - "D-01: tagged-union AST realized — Node = struct { kind, span, payload: union(NodeKind) }"
  - "D-02: per-handle arena owns all nodes; deal_free releases in one shot"
  - "D-03: unified NodeKind covers .deal half + .dealx comp_* placeholders"
  - "D-04: hand-rolled emitAst with fixed key order v, mode, filename, root"
  - "D-05: Zig 0.16.0 idioms — no callconv(.C), no ArrayList.init"
  - "D-08: expr.parseExpression(parser, 0) is the entry for all Expression non-terminals"
  - "D-15: Span on every node"
  - "D-17: enterMode/restoreMode/pushTag/popTag stubs shipped (no-op bodies)"
  - "D-18: alphabetical JSON payload field order locked"
  - "IDENT(args) always parsed as Call, Phase 2 reclassifies unit calls"
metrics:
  duration: "~4h (across session context boundaries)"
  completed: "2026-05-20"
  tasks: 3
  files_created: 22
  files_modified: 7
  tests_added: 11
  snapshots_committed: 15
---

# Phase 1 Plan 3: parser-deal Summary

Full recursive-descent + Pratt parser for all 87 .deal grammar productions; 72-variant tagged-union AST; hand-rolled deterministic JSON emitter; 15 byte-stable showcase snapshots and 3 test gates (precedence, snapshot, coverage) all green.

## What Was Built

### Task 3.1 — Tagged-union AST + Pratt expression parser (c501e09)

`ast.zig` extended to 72 `NodeKind` variants (counted from the enum body):

- File-level: 6 (deal_file, dealx_file, header_block, package_decl, import_decl, export_decl)
- 14 element def kinds (part_def through use_case_def)
- 16 element usage kinds (part_usage through use_case_usage, plus actor_usage and subject_usage)
- 9 structural/type nodes (type_annotation, multiplicity, modifier_list, visibility_wrapper, specialization, redefinition, operator_statement, inline_body, structural_relationship)
- 3 annotation nodes (annotation, doc_comment, annotation_body)
- 3 verification/condition nodes (verification_block, precondition_block, postcondition_block)
- 11 expression nodes (binary, unary, member_access, call, identifier, int/real/string/template/boolean_literal, interpolation)
- 9 dealx composition placeholders (system_block through validate_tag — bodies empty, Plan 04 fills)

`expr.zig` implements the 8-level Pratt table from deal.ebnf §19:

| Level | Operators | Left BP |
|-------|-----------|---------|
| 1 | OR | 10 |
| 2 | AND | 20 |
| 3 | == != | 30 |
| 4 | < <= > >= | 40 |
| 5 | + - | 50 |
| 6 | * / | 60 |
| 7 | unary (- NOT) | prefix BP 70 |
| 8 | . ( postfix | 80 |

### Task 3.2 — Recursive-descent parser + JSON emitter + C ABI wiring (789f496)

`parser_deal.zig`: full Parser struct with `peek/advance/expect/peek2`; `enterMode/restoreMode/pushTag/popTag` no-op stubs; all 87 productions across ~40 functions:
- File structure: parseDealFile, parseOptionalHeaderBlock, parseHeaderBlock, parseHeaderField, parsePackageDecl, parseImportDecl, parseExportDecl
- Definition dispatch: parseDefinition (14-way switch), parseElementDef, parseRequirementDef, parseNeedDef, parseUseCaseDef, makeElementDefPayload
- Definition body: parseDefinitionBody, parseVisibilityWrapper, parseMemberDeclaration (16-way switch), parseUsage, parseAttributeUsage, parseActionUsage, parseActorUsage, parseSubjectUsage, parseUseCaseUsage, makeUsagePayload
- Type/structural: parseOptionalTypeAnnotation, parseOptionalMultiplicity, parseMultiplicity, parseOptionalStructuralRelationship, parseStructuralRelationship, parseOperatorStatement
- Annotations: parseAnnotationStatement, parseCategoryAnnotation, parseStandaloneAnnotation, parseAnnotationBody
- Verification: parseVerificationBlock, parseVerificationValue, parseVerificationArray, parseVerificationArrayItem, parseVerificationObject
- Pre/postconditions: parsePreconditionBlock, parsePostconditionBlock
- Helpers: parseQualifiedNameSegments, parseNamespacePathSegments, isKeyword, tokenText

`json.zig`: `emitAst` with exhaustive `writePayload` switch over all Payload variants; alphabetical field order per D-18; explicit `std.mem.Allocator.Error` on mutually-recursive write functions.

### Task 3.3 — Test suite + snapshots (49d7490)

- `expr_precedence.zig`: 8 test cases covering all adjacant-level transitions in the BP table
- `parser_deal_snapshot.zig`: zero-diagnostic + byte-stable snapshot gate for all 15 .deal showcase files
- `parser_deal_coverage.zig`: NodeKind walker over showcase corpus + synthetic fixtures; required subset (all 14 def, selected usages, expressions, annotations, type/structural) verified ≥ 1 occurrence each
- 15 AST JSON snapshots committed under `tests/snapshots/ast/`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] peek2 consuming wrong token**
- **Found during:** Task 3.3 snapshot generation (import path with dots producing E0100)
- **Issue:** `peek2()` called `lex.next()` twice to get the second token, but after `peek()` sets `self.peeked`, `lex.pos` is already past the first token. The double-advance skipped the second token and returned the third.
- **Fix:** `peek2()` now calls `lex.next()` exactly once after ensuring `peek()` has run, saving/restoring `lex.pos` before/after.
- **Files modified:** `src/parser_deal.zig`
- **Commit:** 49d7490

**2. [Rule 1 - Bug] annotation_prefix (`@category:`) not parsing plain expression value**
- **Found during:** Task 3.3 — `@confidence: 0.95` and `@rationale: "text"` in battery.deal caused E0100 on the value tokens
- **Issue:** `parseCategoryAnnotation` only handled `<<operator>>` + optional `{}` body, not the plain `value` form used in the showcase (`@confidence: 0.95`, `@rationale: "text"`).
- **Fix:** Added `can_be_value` check after operator/body parsing; when no operator and next token is literal/ident, call `expr.parseExpression(p, 0)` as the annotation value.
- **Files modified:** `src/parser_deal.zig`
- **Commit:** 49d7490

**3. [Rule 1 - Bug] export module name rejecting keyword tokens**
- **Found during:** Task 3.3 — `export system.{...}` in requirements/index.deal failed because `system` is `kw_system`, not `ident`.
- **Fix:** `parseExportDecl` now uses `p.advance()` + manual check `(tag == .ident OR isKeyword(tag))` instead of `p.expect(.ident)`.
- **Files modified:** `src/parser_deal.zig`
- **Commit:** 49d7490

**4. [Rule 1 - Bug] Zig inferred error set dependency loops**
- **Found during:** Task 3.2 first build
- **Issue:** Zig 0.16.0 cannot infer error sets for mutually-recursive functions (writeNode/writePayload/writeNodeArrayMut in json.zig; parseExpression/parsePrefix in expr.zig; parseMemberDeclaration/parseUsage/parseInlineBody in parser_deal.zig; parseVerificationValue/Object/Array). Build error: "dependency loop with length N".
- **Fix:** Annotate all functions in each cycle with explicit `std.mem.Allocator.Error!ReturnType`. Also fixed `parseFile` catch handler (unreachable `else` prong removed since only OutOfMemory is possible once the explicit annotation propagates).
- **Files modified:** `src/json.zig`, `src/expr.zig`, `src/parser_deal.zig`
- **Commit:** 789f496

**5. [Rule 2 - Missing critical] circular import avoidance**
- **Found during:** Task 3.2 design
- **Issue:** The plan specified `parseFile(handle: *lib.DealHandle)` but `lib.zig` imports `parser.zig` which imports `parser_deal.zig`, creating a compile-time cycle.
- **Fix:** Changed `parseFile` signature to explicit parameters `(arena, source, diag_list)` across `parser_deal.zig`, `parser.zig`, and `parser_dealx.zig`. Updated `lib.zig` call site accordingly.
- **Files modified:** `src/parser_deal.zig`, `src/parser.zig`, `src/parser_dealx.zig`, `src/lib.zig`
- **Commit:** 789f496

**6. [Rule 2 - Missing critical] Rust FFI smoke test assertion stale**
- **Found during:** Task 3.3 FFI verification
- **Issue:** `tests/ffi/tests/parse.rs` asserted `"root":null` but the real parser produces a non-null `deal_file` root for empty source.
- **Fix:** Updated assertion to check `"root":{` (non-null root node present).
- **Files modified:** `tests/ffi/tests/parse.rs`
- **Commit:** 49d7490

## Key Decisions Made

**IDENT(args) always parsed as Call at parse time.** The plan recommended this approach over LL(2) disambiguation of unit calls (e.g. `V(3.7)` vs `foo(x)`). Plan 04's `.dealx` inline expressions do not use this pattern (they use `IDENT OBJECT_LITERAL` not `IDENT CALL`), so there is no conflict. Phase 2 semantic analysis will reclassify unit calls by checking if the callee resolves to a unit type.

**Alphabetical JSON payload field order (D-18) is locked.** `json.zig` exhaustive switch emits all fields alphabetically per arm. The 15 committed snapshots are byte-stable under this convention. Plans 04/05 must not reorder existing fields when adding new payload arms.

**Single-token peek was NOT sufficient for all LL(2) decision points.** `peek2()` was required for:
1. Import path continuation (`deal.std.units.{` — `.` followed by `{` vs `.` followed by `ident`)
2. Package/qualified name continuation (`.` followed by ident vs `.` followed by semicolon/other)
3. `parseQualifiedNameSegments` in multiple contexts

The plan's RESEARCH Open Question #1 is now resolved: **LL(2) lookahead IS needed**, specifically for the `path_segments . {` pattern. `peek2()` is cheap (saves/restores `lex.pos`, no allocation) and correct.

## Production Coverage Tally (across 15 showcase files + 6 synthetic fixtures)

Required kinds all hit ≥1:

| Kind | Count | Source |
|------|-------|--------|
| deal_file | 23 | showcase (15) + synthetic (8) |
| header_block | 15 | showcase |
| package_decl | 23 | all files |
| import_decl | 14 | showcase |
| export_decl | 11 | showcase |
| part_def | 17 | showcase + synthetic |
| port_def | 5 | interfaces/ |
| action_def | 5 | behaviors.deal |
| state_def | 1 | synthetic |
| attribute_def | 1 | synthetic |
| item_def | 1 | synthetic |
| interface_def | 2 | interfaces/ |
| connection_def | 4 | connections.deal |
| flow_def | 4 | connections.deal |
| allocation_def | 1 | synthetic |
| requirement_def | 9 | requirements/ |
| constraint_def | 1 | synthetic |
| need_def | 5 | needs.deal |
| use_case_def | 4 | use-cases/ |
| part_usage | 3 | battery.deal |
| port_usage | 26 | components.deal |
| attribute_usage | 164 | all component files |
| action_usage | 32 | behaviors.deal |
| requirement_usage | 1 | synthetic |
| binary | 25 | showcase |
| identifier | 173 | showcase |
| int_literal | 50 | showcase |
| real_literal | 31 | showcase |
| string_literal | 62 | showcase |
| boolean_literal | 5 | showcase |
| annotation | 48 | showcase |
| doc_comment | 46 | showcase |
| type_annotation | 202 | showcase |
| multiplicity | 182 | showcase |

Kinds NOT in required set (Plan 04 or future plans):
- `template_literal`, `interpolation` — not in 15 showcase .deal files (Plan 04 may introduce)
- `modifier_list` — no node emitted (modifiers are stored as `[]Modifier` enum values on ElementDef/ElementUsage, not as a separate modifier_list node)
- `specialization` — no node emitted (specialization stored as direct field on ElementDef, not a separate node)
- All 9 `.dealx` comp_* kinds — Plan 04

## Verification

All green at commit 49d7490:

```
$ cd /Users/dunnock/projects/deal-lang/deal && zig build
# → exits 0

$ zig build test --summary all
# → 11/11 tests passed

$ zig build test -Dtest-filter=expr.precedence
# → 1/1 tests passed

$ zig build test -Dtest-filter=parser_deal.snapshot
# → 1/1 tests passed (byte-stable on second run)

$ zig build test -Dtest-filter=parser_deal.coverage
# → 1/1 tests passed (all required NodeKinds ≥ 1)

$ ls tests/snapshots/ast/*.deal.json | wc -l
# → 15

$ cargo test --manifest-path tests/ffi/Cargo.toml
# → 3/3 passed (ffi_smoke, ffi_dealx_mode_detection, ffi_free_null_is_safe)

$ grep -c 'callconv(.C)' src/*.zig
# → 0 (uppercase banned per D-05)
```

## Threat Surface Scan

No new network endpoints, auth paths, file access patterns, or trust-boundary schema changes introduced. The plan's threat register items T-03-01 through T-03-SC remain in their documented dispositions:
- T-03-01 (DoS via recursion depth): natural recursion, Plan 05 adds depth bound
- T-03-02 (source bytes in JSON): accepted per D-04 contract
- T-03-03 (snapshot byte-stability): mitigated by hand-rolled JSON + alphabetical field order
- T-03-04 (arena growth): page_allocator backing, Plan 05 adds depth limit

## Self-Check: PASSED

Files exist:
- /Users/dunnock/projects/deal-lang/deal/src/ast.zig ✓
- /Users/dunnock/projects/deal-lang/deal/src/expr.zig ✓
- /Users/dunnock/projects/deal-lang/deal/src/parser_deal.zig ✓
- /Users/dunnock/projects/deal-lang/deal/src/json.zig ✓
- /Users/dunnock/projects/deal-lang/deal/tests/unit/expr_precedence.zig ✓
- /Users/dunnock/projects/deal-lang/deal/tests/unit/parser_deal_snapshot.zig ✓
- /Users/dunnock/projects/deal-lang/deal/tests/unit/parser_deal_coverage.zig ✓
- /Users/dunnock/projects/deal-lang/deal/tests/snapshots/ast/ (15 files) ✓

Commits exist: c501e09, 789f496, 49d7490 ✓
