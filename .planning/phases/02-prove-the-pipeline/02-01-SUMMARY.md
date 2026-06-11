---
phase: 02-prove-the-pipeline
plan: 01
subsystem: lexer, parser, ast, json, cli, schemas
tags: [comment-attachment, cargo-workspace, cli-scaffold, ast-shape, snapshots, tdd]
dependency_graph:
  requires: [01-06-SUMMARY, 01.5-04-SUMMARY, 01.5-05-SUMMARY]
  provides: [comment-attachment-ast, cargo-workspace, cli-binary, schema-bundle]
  affects: [02-02, 02-03, 02-04, 02-05, 02-06]
tech_stack:
  added: [clap 4.6, anyhow 1, thiserror 2, anstream 1, owo-colors 4, insta 1.47]
  patterns: [gofmt-blank-line-rule, pending-comments-buffer, arena-comment-text, kind-aware-blank-line-detection]
key_files:
  created:
    - Cargo.toml (workspace root)
    - cli/Cargo.toml
    - cli/build.rs
    - cli/src/main.rs
    - cli/src/ffi.rs
    - cli/src/render.rs
    - cli/tests/key_order.rs
    - cli/tests/cli_smoke.rs
    - tests/schemas/SysML.json
    - tests/schemas/KerML.json
    - tests/schemas/README.md
    - tests/unit/comment_attachment.zig
    - scripts/diff-only-new-fields.sh
  modified:
    - src/lexer.zig (comment_line, comment_block tags; skipTrivia whitespace-only)
    - src/ast.zig (Comment struct; leading/trailing/doc fields on ElementDef + ElementUsage)
    - src/parser_deal.zig (pending_comments buffer, nextNonComment, attachLeadingComments)
    - src/json.zig (writeCommentArray, writeOptDocComment, extended def/usage emitters)
    - tests/ffi/Cargo.toml (removed links="deal")
    - tests/unit/lexer_comments.zig (Cases 2+3 expect tokens)
    - tests/unit/_all.zig (registered comment_attachment.zig)
    - tests/snapshots/ast/ (11 of 19 files; new comment fields added)
    - tests/snapshots/tokens/ (9 files; comment tokens now visible)
decisions:
  - "D-28: comment tokens emitted by lexer, not skipped in skipTrivia"
  - "D-29: DocComment struct reused; doc_comment field on ElementDef/ElementUsage is ?*DocComment (existing doc *Node path still active; Plan 02-05 unifies)"
  - "D-30: 19 snapshots regenerated in single dedicated commit 8e05fd3; only new comment fields in diff"
  - "D-31: comment attachment lands in Wave 1 before sema/IR/fmt work"
  - "PATTERNS Lane 8 option (c): cli/ claims links=deal; tests/ffi/ drops it"
  - "attach_from starts at 0 (all attach); blank-line detection is kind-aware (line comment consumed trailing \\n so threshold=1; block comment did not so threshold=2)"
metrics:
  duration: ~3 hours
  completed: 2026-05-21
  tasks: 3
  files: 28
---

# Phase 02 Plan 01: Cargo Workspace + CLI Scaffold + Comment Attachment to AST Summary

Comment attachment to AST (D-28..D-31) + Cargo workspace with 4-subcommand CLI skeleton, bundled OMG schemas, and defensive test infrastructure.

## What Was Built

### Task 1a: Cargo Workspace + CLI Skeleton (commit `10f9189`)

Stood up the Rust side of the DEAL compiler toolchain:

- `Cargo.toml` workspace root with `resolver = "2"`, members `["cli", "tests/ffi"]`, and `[workspace.dependencies]` pinning clap 4.6, serde_json 1 (no `preserve_order`), jsonschema 0.46, anstream 1, owo-colors 4, anyhow 1, thiserror 2, insta 1.47.
- `cli/Cargo.toml`: `name = "deal"`, `links = "deal"` (PATTERNS Lane 8 option (c) — cli/ owns the native lib claim; tests/ffi/ drops it). All deps reference `{ workspace = true }`.
- `cli/build.rs`: invokes `zig build`, emits `cargo::rustc-link-search=native=…` + `cargo::rustc-link-lib=static=deal`. One `.join("..")` from cli/ to deal/ root (tests/ffi/ uses two).
- `cli/src/ffi.rs`: 6 Phase 1 extern "C" decls (deal_parse, deal_free, deal_has_errors, deal_diagnostics_count, deal_ast_json, deal_diagnostics_json). `DealHandle` is `!Send`/`!Sync`. Comment marks where Plan 02-03 appends deal_ir_json + deal_format.
- `cli/src/main.rs`: clap derive CLI; `CliError` with `User(String)` (exit 1) + `Internal(anyhow::Error)` (exit 2); 4 subcommand stubs (Parse/Check/Fmt/Build) each return `Err(CliError::Internal(...))` per D-34; global `--json`/`--color`/`--verbose` flags.
- `cli/src/render.rs`: stub with `unimplemented!()` — Plan 02-02 fills it.
- `tests/ffi/Cargo.toml`: removed `links = "deal"`.

Verification: `cargo build --workspace` → exit 0; `deal --version` → exit 0; all 4 stubs → exit 2.

### Task 1b: Bundled Schemas + Defensive Tests (commit `bbcf0f5`)

- `tests/schemas/SysML.json`: byte-identical copy from `spec/references/omg-sysml-v2/SysML.json` (OMG 20250201; 126,290 lines).
  SHA256: `bb0d8af159cf2cbe4a0df4ed6b903505a57e33d047068e5a50a7008a18d546c5`
- `tests/schemas/KerML.json`: byte-identical copy from `spec/references/omg-kerml-v1/KerML.json` (OMG 20250201; 42,284 lines).
  SHA256: `e454fe4b7c04f3d95874b6c1a4e6ef056ea5874c71fd3d17e3180319c2f58ab2`
- `tests/schemas/README.md`: provenance, source URLs, SHA256 pins, tamper-detection note (Plan 02-04 will SHA256-check at validator construction — T-02-01).
- `cli/tests/key_order.rs`: asserts `serde_json::to_string({"z":1,"a":2}) == {"a":2,"z":1}` — guards Pitfall 1 (preserve_order leak destroys D-18 invariant).
- `cli/tests/cli_smoke.rs`: `version_flag_exits_zero` + `subcommand_stubs_exit_two` — guards D-34 exit-code contract (Pitfall 8); uses `std::process::Command` (no new dep).

### Task 2: Comment Attachment to AST (commits `8e05fd3` + `0338865`)

Full comment attachment infrastructure per D-28..D-31:

**Lexer (`src/lexer.zig`):**
- Added `Tag.comment_line` (`//...\n`, span includes trailing `\n`), `Tag.comment_block` (`/*...*/`)
- `skipTrivia()` now handles only whitespace
- `scanLineComment()` and `scanBlockComment()` emit the respective tokens
- `/` dispatch: `//` → `scanLineComment`, `/*` non-doc → `scanBlockComment`, `/**` → `scanDocComment`

**AST (`src/ast.zig`):**
- Added `pub const Comment = struct { kind: enum { line, block }, text: []const u8, span: Span }`
- Added three fields to `ElementDef` and `ElementUsage`:
  - `leading_comments: []Comment = &.{}`
  - `trailing_comments: []Comment = &.{}`
  - `doc_comment: ?*DocComment = null`
- Existing `DocComment` struct at line 404 is REUSED (D-29); existing `NodeKind.doc_comment` arm unchanged

**Parser (`src/parser_deal.zig`):**
- `pending_comments: std.ArrayList(ast.Comment) = .empty` buffer on `Parser`
- `nextNonComment()`: drains `comment_line`/`comment_block` into `pending_comments`; `doc_comment` tokens returned as-is
- `peek()` and `advance()` call `nextNonComment()`
- `attachLeadingComments(decl_span_start)`: gofmt blank-line rule
  - `attach_from = 0` (all attach by default); blank-line scan raises the index
  - Kind-aware blank-line detection: `comment_line` tokens include trailing `\n` so threshold = 1 newline in gap; `comment_block` tokens do not so threshold = 2
- `attach.leading` and `attach.doc` wired into `parseElementDef`, `parseRequirementDef`, `parseNeedDef`, `parseUseCaseDef`

**JSON emitter (`src/json.zig`):**
- `writeCommentArray()`: emits `[{"kind":"line","span":[s,e],"text":"..."}]` (alphabetical keys per D-18)
- `writeOptDocComment()`: emits `null` or `{"text":"..."}`
- `writeElementDefFields()` extended: annotations, direction, doc, **doc_comment, leading_comments**, members, modifiers, name, specializes, **trailing_comments**
- `writeElementUsageFields()` extended: annotations, default_value, direction, doc, **doc_comment**, inline_body, **leading_comments**, modifiers, multiplicity, name, **trailing_comments**, type_node

**Snapshots (D-30 — ONE dedicated commit `8e05fd3`):**
- Token snapshots: 9 of 19 files updated (comment_line/comment_block tokens now visible in token-snapshot files that contain `//` or `/* */` comments)
- AST snapshots: 11 of 19 files updated (new fields on all def/usage nodes; remaining 8 have no def/usage nodes)
- `diff-only-new-fields.sh` script asserts structural discipline

**TDD unit tests (`tests/unit/comment_attachment.zig`):**
Six cases covering the gofmt blank-line rule:
1. Line comment immediately before decl → `leading_comments`
2. Line comment separated by blank line → NOT attached
3. Block comment immediately before decl → `leading_comments`
4. Doc comment `/**` → `elem.doc` field (existing path); `leading_comments` empty
5. Multiple consecutive comments → all attach as `leading_comments`
6. Blank line in middle of comment group → only contiguous post-blank group attaches

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] attach_from initialized to comments.len (nothing attaches)**
- Found during: Task 2, TDD tests (Cases 1, 3, 5, 6 all failed with `leading_comments.len == 0`)
- Issue: `attachLeadingComments()` initialized `attach_from = comments.len` with a comment saying "start assuming all attach" — but `comments[attach_from..]` with `attach_from == len` is an empty slice. The backward scan would only SET `attach_from` when a blank line was found; without a blank line, it left `attach_from == len` (nothing attached).
- Fix: Changed initialization to `attach_from = 0` (all attach by default); the scan increases it when a blank line is found
- Files: `src/parser_deal.zig`
- Commit: `0338865`

**2. [Rule 1 - Bug] hasBlankLineBetween: wrong threshold for comment_line vs comment_block**
- Found during: Task 2, TDD test Case 3 (block comment) failed with `leading_comments.len == 0`
- Issue: `comment_line` tokens include their trailing `\n` in the span; so the gap between `cmt.span.end` and the next token starts AFTER that `\n`. A blank line in this gap = 1 `\n`. But `comment_block` tokens do NOT include trailing whitespace; the gap includes the `\n` that terminates the comment's line. A blank line after a block comment = 2 `\n` characters in the gap.
- Fix: Added `consumed_trailing_newline: bool` parameter; `comment_line` uses threshold 1, `comment_block` uses threshold 2
- Files: `src/parser_deal.zig`
- Commit: `0338865`

**3. [Rule 1 - Bug] Zig 0.16 ArrayList initialization: .init(alloc) not available**
- Found during: Task 2, first compile of `comment_attachment.zig`
- Issue: Used `std.ArrayList(Diagnostic).init(alloc)` — not available in Zig 0.16 where `ArrayList` uses the `.empty` default-value idiom
- Fix: Changed to `var diags: std.ArrayList(Diagnostic) = .empty`
- Files: `tests/unit/comment_attachment.zig`

## Key Decisions

1. **Links conflict (Lane 8 option (c))**: `cli/` claims `links = "deal"`; `tests/ffi/` drops it. Cargo allows exactly one crate per native library; cli/ is the canonical consumer. If tests/ffi/ needs its own native symbols in future, switch to option (b) (separate sub-crate) via ADR.

2. **doc_comment field vs doc node**: The new `doc_comment: ?*DocComment = null` field on `ElementDef` is null in Plan 02-01. The existing `doc: ?*ast.Node` path in `parseDefinition()` continues to handle `/** */` tokens (peek().tag == .doc_comment branch). Plan 02-05 will unify these two paths. This preserves all 19 snapshot values.

3. **attach_from semantics**: Start at 0 (all attach); scan backwards and increase when blank line found. This is the correct gofmt-rule interpretation: the default is "attach", the exception is "blank line separates".

4. **kind-aware blank-line detection**: `comment_line` spans include `\n`; `comment_block` spans do not. Threshold in gap must differ by kind. This is a lexer contract documented in `hasBlankLineBetween`.

## Snapshot Regen Record

Dedicated commit for all 19 snapshots: `8e05fd3`

Diff discipline: `scripts/diff-only-new-fields.sh` exits 0 — only `doc_comment`, `leading_comments`, and `trailing_comments` fields appear in the diff. No structural drift.

All 19 showcase files: `leading_comments: []`, `trailing_comments: []`, `doc_comment: null` on every def/usage node. The standalone `// ═══...` separator comments in connections.deal and other files are separated from definitions by blank lines, so they are correctly not attached (floating; dropped in Plan 02-01; Plan 02-05 fmt will surface them).

## Verification Results

- `zig build` → exit 0
- `zig build test` → exit 0 (all tests pass, including 6 new comment_attachment tests)
- `zig build test -Dtest-filter=determinism` → exit 0
- `zig build test -Dtest-filter=property` → exit 0
- `zig build phase-1.5-gate` → exit 0 (regression invariant GREEN)
- `cargo build --workspace` → exit 0
- `deal --version` → exit 0
- All 4 subcommand stubs → exit 2 with stderr "not yet implemented"
- `cargo test --workspace` → exit 0 (key_order + cli_smoke + ffi tests all green)
- `bash scripts/diff-only-new-fields.sh tests/snapshots/ast/showcase__*.json` → exit 0

## Self-Check: PASSED

Files created/modified all exist and are committed. All commits verified in git log:
- `10f9189` — Task 1a: Cargo workspace + CLI scaffold
- `bbcf0f5` — Task 1b: bundled OMG schemas + defensive tests
- `8e05fd3` — Task 2 (core): comment attachment to AST + 19 snapshots regenerated
- `0338865` — Task 2 (completion): comment attachment tests + gofmt rule bugfixes + diff-only script
