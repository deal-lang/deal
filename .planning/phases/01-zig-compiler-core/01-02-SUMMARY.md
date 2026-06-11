---
phase: 01-zig-compiler-core
plan: 01-02-lexer
subsystem: compiler-frontend
tags: [zig, lexer, lexical-analysis, four-mode-dispatch, static-string-map, snapshot-tests, utf8]

# Dependency graph
requires:
  - phase: 01-01-foundation
    provides: "build.zig + 11 src/*.zig stubs + tests/ harness + Rust FFI scaffold"
provides:
  - "Stateless four-mode lexer (Lexer.next/peek over Mode {deal_def, dealx_outer, dealx_tag, dealx_expr_brace})"
  - "85-entry comptime keyword perfect-hash (std.StaticStringMap) covering every reserved spelling in lexical.ebnf §5"
  - "Deterministic token JSON emitter (json.emitTokens) producing D-04+D-18-compliant snapshot bytes"
  - "Lazy line-start table on SourceMap for O(log n) byte→(line,col) translation"
  - "19 committed showcase token snapshots — Phase 1 success criterion #1 met"
  - "Snapshot-update workflow (-Dupdate-snapshots=true) that Plans 03/04 will reuse for AST snapshots"
affects:
  - 01-03-parser-deal
  - 01-04-parser-dealx
  - 01-05-error-recovery
  - 01-06-c-abi-and-gate

# Tech tracking
tech-stack:
  added:
    - std.StaticStringMap (Zig 0.16.0 comptime perfect-hash)
    - std.unicode.utf8ValidateSlice (graceful degradation on malformed bytes)
    - std.Io.Dir.cwd().readFileAlloc / writeFile (Zig 0.16.0 IO model)
    - addOptions/build_options (build-time flag plumbing)
  patterns:
    - "Stateless mode dispatch — lexer receives mode on every call (D-06); no Lexer.mode field"
    - "Token = (tag, span) only — text recovered from source slice on demand (D-15)"
    - "Bounded interpolation depth stack (T-02-01: template_depth_limit=64)"
    - "Named-module imports (`@import('ast')`) across all src/*.zig — single module graph shared by lib + test targets"
    - "Snapshot harness: read showcase → emit JSON → byte-compare; -D flag updates rather than env var"
    - "Hand-rolled deterministic JSON writer with explicit alphabetical key order (D-18) — std.json.Stringify avoided per RESEARCH §Pitfall 5"

key-files:
  created:
    - tests/unit/lexer_mode_flip.zig
    - tests/unit/lexer_keywords.zig
    - tests/unit/lexer_comments.zig
    - tests/unit/lexer_templates.zig
    - tests/unit/lexer_snapshot.zig
    - tests/unit/_all.zig
    - tests/snapshots/tokens/showcase__packages__vehicle__battery.deal.json
    - tests/snapshots/tokens/showcase__packages__vehicle__motor.deal.json
    - tests/snapshots/tokens/showcase__packages__vehicle__behaviors.deal.json
    - tests/snapshots/tokens/showcase__packages__vehicle__components.deal.json
    - tests/snapshots/tokens/showcase__packages__vehicle__index.deal.json
    - tests/snapshots/tokens/showcase__packages__interfaces__electrical.deal.json
    - tests/snapshots/tokens/showcase__packages__interfaces__thermal.deal.json
    - tests/snapshots/tokens/showcase__packages__interfaces__connections.deal.json
    - tests/snapshots/tokens/showcase__packages__interfaces__index.deal.json
    - tests/snapshots/tokens/showcase__packages__requirements__system.deal.json
    - tests/snapshots/tokens/showcase__packages__requirements__needs.deal.json
    - tests/snapshots/tokens/showcase__packages__requirements__index.deal.json
    - tests/snapshots/tokens/showcase__packages__use-cases__driving.deal.json
    - tests/snapshots/tokens/showcase__packages__use-cases__charging.deal.json
    - tests/snapshots/tokens/showcase__packages__use-cases__index.deal.json
    - tests/snapshots/tokens/showcase__model__vehicle.dealx.json
    - tests/snapshots/tokens/showcase__model__traceability.dealx.json
    - tests/snapshots/tokens/showcase__model__variants__sedan.dealx.json
    - tests/snapshots/tokens/showcase__model__variants__performance.dealx.json
  modified:
    - src/lexer.zig
    - src/keywords.zig
    - src/source_map.zig
    - src/json.zig
    - src/lib.zig
    - src/ast.zig (named imports only)
    - src/diagnostics.zig (named imports only)
    - src/expr.zig (named imports only)
    - src/parser.zig (named imports only)
    - src/parser_deal.zig (named imports only)
    - src/parser_dealx.zig (named imports only)
    - build.zig

key-decisions:
  - "Block + line comments are SILENTLY SKIPPED in skipTrivia; only DOC comments emit a token (consistent with RESEARCH §Pattern 1 lexer-on-demand and parser doc-attachment intent)"
  - "Switched ALL src/*.zig from path imports (`@import('ast.zig')`) to NAMED imports (`@import('ast')`) so the lib target and the unit-test target can share a single module graph (Zig 0.16.0 rejects `../../src/*.zig` cross-tree imports)"
  - "Build-time `-Dupdate-snapshots=true` instead of `UPDATE_SNAPSHOTS=1` env var — Zig 0.16.0's std.process env-var API requires a full Io.Threaded instance and a parsed Environ.Map, which is overkill for a per-test boolean"
  - "Umbrella test file (tests/unit/_all.zig) — single `b.addTest` per umbrella keeps the test graph compact and `-Dtest-filter=<name>` still matches by test-name substring"
  - "Multi-byte UTF-8 bytes outside grammar slots: skip the full scalar (using utf8ValidateSlice on the leading-byte slice) and emit a single `.unknown` token — no panic, no stuck cursor"

patterns-established:
  - "Lexer-on-demand mode dispatch: every `next(mode)` is independent; mode lives on the parser frame (D-06)"
  - "Bounded scan: every potentially-unbounded sub-scanner (templates, doc comments) carries a hard limit (template depth ≤ 64; doc comments capped by source length)"
  - "Snapshot-name convention: `showcase__<path-with-slashes-as-double-underscore>.json` — Plans 03/04 will mirror for AST snapshots"
  - "Build-options module for test-runner flags (alternative to env vars while Zig 0.16.0's std.process env API requires Threaded)"

requirements-completed:
  - REQ-phase-1-1-lexer

# Metrics
duration: ~1 session
completed: 2026-05-19
---

# Phase 1 Plan 02: Stateless four-mode lexer with 19-file showcase token snapshot gate

**Full lexer covering lexical.ebnf §1-§14 with parser-driven mode dispatch and a comptime 85-keyword perfect-hash; every showcase file tokenizes with zero `.unknown` and byte-stable JSON snapshots.**

## Performance

- **Duration:** ~1 session
- **Started:** 2026-05-20T00:13:00Z
- **Completed:** 2026-05-20T00:48:34Z
- **Tasks:** 3 / 3
- **Files modified:** 12 src/+build + 6 test/umbrella + 19 snapshot artifacts = 37 files

## Accomplishments

- The four-mode dispatch from D-06 is observable through `lexer.mode_flip`: `[<` lexes as `l_bracket + lt` in `.deal_def` but a single `tag_open` in `.dealx_outer`; `>]` and `/>]` only collapse in `.dealx_tag`.
- The 85-entry keyword table (D-05 / RESEARCH §Pattern 2) is exact — verified per spelling against `lexical.ebnf` §5 production lines 459-481. The line-749 summary comment that says "76" is stale; the production body is authoritative and is being corrected in a separate spec PR (deferred-items entry in CONTEXT).
- Every one of the 19 showcase files (15 `.deal` + 4 `.dealx`) lexes through the four-mode dispatch with ZERO `.unknown` tokens — the locked Phase 1 success criterion #1.
- Token snapshots are byte-stable across consecutive `zig build test -Dtest-filter=lexer.snapshot` runs (hand-rolled JSON writer with deterministic alphabetical key order per D-18; RESEARCH §Pitfall 5 std.json.Stringify ordering risk avoided).
- The unrelated parser stubs, FFI tests, and prior Plan 01 build outputs all continue to work — Plan 02 added bodies and tests without breaking any Wave-0 invariant.

## Task Commits

Each task was committed atomically:

1. **Task 2.1: Implement lexer.zig + keywords.zig + source_map.zig** — `f783d02` (feat)
2. **Task 2.2: Token JSON emitter + 4 unit tests + module graph rewire** — `62f44f8` (test)
3. **Task 2.3: Showcase token snapshots — 19 files lex with zero unknowns** — `cb4562b` (test)

**Plan metadata commit:** (this commit) — `docs(01-02): complete 01-02-lexer plan`

## Files Created / Modified

### Source (src/)
- `src/lexer.zig` — full Tag enum (~160 variants), `next(mode)` / `peek(mode)`, mode-dependent composition-tag dispatch, template-depth-bounded `${}` scanner, DELIMITED_OPERATOR scanner with embedded spaces, doc-comment scanner, longest-match punctuation scanner, UTF-8 fail-graceful fallback.
- `src/keywords.zig` — 85-entry `global_keywords` table + 8-entry `dealx_tag_names` semantic classifier (Plan 04 consumer).
- `src/source_map.zig` — lazy line-start table + binary-search `lineCol`, with inline `source_map.line_col` tests covering single-line, multi-line, and last-line-mid cases.
- `src/json.zig` — added `emitTokens(allocator, source, mode)` returning `{"v":1,"mode":"...","tokens":[...]}` with deterministic key order; `tagName` switch (not `@tagName`) so future renames don't silently rewrite snapshots; `appendJsonStringEscaped` per RFC 8259 §7.
- `src/lib.zig` — comment-only update to the comptime `_ = lexer; _ = parser;` touch block (lexer is now real but `lib.zig` only references it transitively).
- `src/ast.zig`, `src/diagnostics.zig`, `src/expr.zig`, `src/parser.zig`, `src/parser_deal.zig`, `src/parser_dealx.zig` — switched intra-src imports from path-form `@import("ast.zig")` to named-form `@import("ast")` so the unit-test root module can share the same module graph.

### Build
- `build.zig` — module-graph refactor: `createSrcModules` + `addSrcImports` helpers build the 10-module dependency graph once and attach it to both the library target and the unit-test target. Second `addTest` rooted at `tests/unit/_all.zig`. `-Dupdate-snapshots` boolean wired through an `addOptions`-derived `build_options` module.

### Tests
- `tests/unit/_all.zig` — comptime umbrella that `@import`s each lexer test file.
- `tests/unit/lexer_mode_flip.zig` — 6 cases proving the D-06 mode flip both directions (open `[<` and close `>]`/`/>]`).
- `tests/unit/lexer_keywords.zig` — iterates all 85 keyword spellings; asserts non-keyword identifiers / true / false / Boolean / Integer / Real / String all behave correctly.
- `tests/unit/lexer_comments.zig` — `/**` vs `/*` disambiguation; `//` line comment skip; unterminated `/** → .unknown` to EOF.
- `tests/unit/lexer_templates.zig` — `${}` interpolation HEAD/MIDDLE/TAIL, non-interpolating `template_literal`, T-02-01 DoS bound at depth 65.
- `tests/unit/lexer_snapshot.zig` — drives the 19-file snapshot corpus + a sibling `spans monotonic` cross-check.

### Snapshots
- `tests/snapshots/tokens/showcase__*.json` — 19 byte-stable JSON files (15 `.deal` + 4 `.dealx`).

## Decisions Made

1. **Block + line comments are SILENTLY SKIPPED in `skipTrivia`; doc comments emit `.doc_comment` tokens.** Documented for Plan 03 consumers — the parser will attach `.doc_comment` tokens to the next definition; block / line comments are discarded at lex time and never reach the parser. This matches the RESEARCH §Pattern 1 lexer-on-demand recommendation and the lexical.ebnf §2 normalization note "not in AST (unless retained for formatting)".

2. **Named-module imports across all src/*.zig** (DEVIATION from plan, minor): see Task 2.2 deviation below. Switched from `@import("ast.zig")` to `@import("ast")` everywhere so the lib target (root: `src/lib.zig`) and the unit-test target (root: `tests/unit/_all.zig`) can share a single module-graph object instead of rebuilding it twice or working around Zig 0.16.0's "import of file outside module path" sandbox restriction.

3. **Build-time `-Dupdate-snapshots=true` instead of `UPDATE_SNAPSHOTS=1` env var** (DEVIATION from plan): see Task 2.3 deviation below. Zig 0.16.0's env-var access requires a full `Io.Threaded` instance and a parsed `Environ.Map`, neither of which is naturally available inside a unit test. Build-time options threaded through `b.addOptions` + a `build_options` module are simpler and equivalently usable (`zig build test -Dupdate-snapshots=true -Dtest-filter=lexer.snapshot`).

4. **Token snapshots emitted at the file's OUTER mode only** (`.deal_def` for .deal, `.dealx_outer` for .dealx). Inner-mode transitions (`.dealx_tag`, `.dealx_expr_brace`) are the parser's responsibility; Plans 03/04 exercise them through parser snapshots. Captures the lexer-only contract cleanly without depending on not-yet-implemented parser state.

5. **Single `peek(mode)` implementation: save pos + template stack, call next, restore.** Plan 02 keeps the simplest implementation per the plan; Plan 03 may add a one-token cache on the parser frame if profiling shows the redundant scan dominates the parse cost.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Named-module imports across all src/*.zig**
- **Found during:** Task 2.2 (wiring unit-test root to consume `src/lexer.zig` etc.)
- **Issue:** Zig 0.16.0 rejects `@import("../../src/lexer.zig")` from `tests/unit/lexer_mode_flip.zig` with "import of file outside module path". Path imports only resolve within the module that owns the file tree.
- **Fix:** Switched every `@import("foo.zig")` inside `src/*.zig` to `@import("foo")` (named form), then wired the 10-module dependency graph once in `build.zig` (`createSrcModules` / `addSrcImports` helpers) and attached it to both the library target and the unit-test target. Tests now use `@import("lexer")` etc., backed by the same module objects the library uses.
- **Files modified:** all `src/*.zig` + `build.zig` + `tests/unit/lexer_*.zig`
- **Verification:** `zig build` exits 0, `zig build test --summary all` shows 8/8 tests pass across both targets.
- **Committed in:** `62f44f8` (Task 2.2 commit)

**2. [Rule 3 - Blocking] Build-option `-Dupdate-snapshots` instead of `UPDATE_SNAPSHOTS=1` env var**
- **Found during:** Task 2.3 (snapshot harness implementation)
- **Issue:** RESEARCH §Open Question 3 recommended `UPDATE_SNAPSHOTS=1` env var, but reading env vars in Zig 0.16.0 requires a full `Io.Threaded` instance + a parsed `Environ.Map` — neither is naturally available inside a `test "..."` block.
- **Fix:** Wired a `-Dupdate-snapshots` boolean option in `build.zig` via `b.addOptions()`, exposed it as `@import("build_options").update_snapshots`. The snapshot harness reads it as a comptime-known constant. Workflow becomes `zig build test -Dupdate-snapshots=true -Dtest-filter=lexer.snapshot`.
- **Files modified:** `build.zig` + `tests/unit/lexer_snapshot.zig`
- **Verification:** snapshots regenerate cleanly with the flag; subsequent runs without the flag byte-compare against the regenerated baseline. Documented for downstream plans in this Decisions section.
- **Committed in:** `cb4562b` (Task 2.3 commit)

**3. [Rule 2 - Missing critical functionality] Added `template_literal` tag and `spans monotonic` cross-check test**
- **Found during:** Task 2.2 (writing `lexer.templates`)
- **Issue:** The plan's Tag enum list omits `template_literal` (non-interpolating backtick strings like `` `hello` ``); `lexical.ebnf` §4 explicitly defines `TEMPLATE_LITERAL` as a distinct token.
- **Fix:** Added `template_literal` to the Tag enum and its scanner; covered in `lexer.templates` Case 3. Also added a `lexer.snapshot spans monotonic` cross-check (sibling test in `lexer_snapshot.zig`) that catches lexer pos-cursor regressions across the showcase corpus — a defense-in-depth gate beyond the bytewise snapshot comparison.
- **Files modified:** `src/lexer.zig`, `tests/unit/lexer_templates.zig`, `tests/unit/lexer_snapshot.zig`
- **Verification:** lexer.templates Case 3 passes; spans-monotonic test passes for all 19 files.
- **Committed in:** `62f44f8` + `cb4562b`

**4. [Rule 2 - Missing critical functionality] Added `delimited_operator` to handle `<<allocated to>>` etc.**
- **Found during:** Task 2.1 (sanity-checking the showcase corpus against the Tag enum)
- **Issue:** The plan's Tag-enum bullet list omits the `delimited_operator` for `<< ... >>` (DELIMITED_OPERATOR per lexical.ebnf §6). The showcase corpus uses ~30 distinct spellings including space-bearing ones: `<<specializes>>`, `<<allocated to>>`, `<<partially satisfies>>`, `<<verified by>>`, `<<derives from>>`, `<<flows to>>`, etc.
- **Fix:** Added `delimited_operator` Tag variant, dedicated `scanDelimitedOperator` that accepts `[a-zA-Z ]+` between `<<` and `>>` per the grammar's OPERATOR_NAME production. Branches BEFORE the punctuation scanner so `<<` never starts a chain of `lt` tokens.
- **Files modified:** `src/lexer.zig`, `src/json.zig` (tagName + tagCarriesText switches)
- **Verification:** `lexer.snapshot` passes on every showcase file containing `<<...>>` (battery.deal, motor.deal, vehicle.dealx, etc.).
- **Committed in:** `f783d02` (Task 2.1 commit)

**5. [Rule 2 - Missing critical functionality] Added `annotation_prefix` / `annotation` / `boolean_literal` / SysML aliases**
- **Found during:** Task 2.1 (full corpus pass)
- **Issue:** Plan listed only the operators / brackets / comments — the broader corpus uses `@confidence:`, `@assumes`, `true` / `false`, and (per lexical.ebnf §6) the SysML alias tokens `:>`, `:>>`, `::>`, `~`. None of these are in the plan's bullet list but the gate of "zero `.unknown` tokens" required them.
- **Fix:** Added `annotation_prefix`, `annotation`, `boolean_literal`, `sysml_specializes`, `sysml_redefines`, `sysml_references`, `sysml_conjugates` to the Tag enum; dedicated `scanAnnotation` handler; longest-match punctuation order in `scanPunctuation` covers the SysML aliases before bare `:` / `~`.
- **Files modified:** `src/lexer.zig`, `src/json.zig`
- **Verification:** zero `.unknown` across 19 snapshots (the locked gate).
- **Committed in:** `f783d02` (Task 2.1 commit)

**6. [Rule 2 - Missing critical functionality] `unrestricted_name` token (single-quoted name production)**
- **Found during:** Task 2.1 (covering `lexical.ebnf` §3 UNRESTRICTED_NAME)
- **Issue:** Plan didn't list `unrestricted_name` as a tag, but the grammar defines `'use case'`-style identifiers. Although the 19-file corpus doesn't currently use them, omitting the token would have left a latent bug for Plan 03.
- **Fix:** Added `unrestricted_name` and `scanQuotedSlice` (shared with `string_literal` via the same code path; the parser distinguishes by context).
- **Files modified:** `src/lexer.zig`, `src/json.zig`
- **Verification:** corpus snapshots unchanged; future single-quoted name input lexes cleanly.
- **Committed in:** `f783d02`

---

**Total deviations:** 6 auto-fixed (3 missing-critical-functionality enum extensions, 1 missing-critical test, 2 blocking infrastructure)
**Impact on plan:** All deviations were necessary to meet the locked gate ("zero `.unknown` on the corpus"); none expanded scope. The named-module-import switch is the only one that touches existing committed files (the rest are additive), and it's mechanically equivalent — the lib target's behavior is unchanged.

## Issues Encountered

None. All gates passed first try after Task 2.2's blocking-fix module-graph rewire.

## RESEARCH Assumption Resolutions

- **A1 — `std.unicode.utf8ValidateSlice` exact name in Zig 0.16.0.** **RESOLVED.** Confirmed at `/opt/homebrew/Cellar/zig/0.16.0_1/lib/zig/std/unicode.zig:231`:
  ```zig
  pub fn utf8ValidateSlice(input: []const u8) bool { ... }
  ```
  Used in `src/lexer.zig` `scanPunctuation`'s high-bit fallback to validate a leading-byte sequence before advancing past it as a single `.unknown`.

- **A3 — `std.fs.cwd().readFileAlloc` vs `openFile().readToEndAlloc`.** **RESOLVED with a 0.16.0-specific twist.** `std.fs` is deprecated in 0.16.0 (`/opt/homebrew/Cellar/zig/0.16.0_1/lib/zig/std/fs.zig` line 5: "Deprecated, use `std.Io.Dir.path`"). The current API is `std.Io.Dir.cwd().readFileAlloc(io, sub_path, gpa, limit)` where `io` comes from `std.testing.io` and `limit` is `Io.Limit.unlimited`. The harness in `tests/unit/lexer_snapshot.zig` uses exactly this shape.

- **A2 — `ArrayList(T) = .empty` allocator-explicit pattern.** Continued from Plan 01; exercised in `src/source_map.zig`'s `buildLineStarts` and `src/json.zig`'s `emitTokens`. No `ArrayList(T).init(allocator)` anywhere (grep gate green).

## Known Stubs

None — all Plan 02 code paths are real implementations. The parser stubs in `src/parser*.zig` and `src/expr.zig` remain stubs (per their respective plan ownership: Plan 03 fills `parser.zig` + `parser_deal.zig` + `expr.zig`; Plan 04 fills `parser_dealx.zig`). The `comptime { _ = lexer; _ = parser; }` touch block in `src/lib.zig` stays in place until Plan 03 wires `deal_parse → parser.parseFile` for real.

## Snapshot File Inventory

For Plan 03's AST snapshot plan to mirror the naming convention:

```
tests/snapshots/tokens/
  showcase__model__traceability.dealx.json
  showcase__model__variants__performance.dealx.json
  showcase__model__variants__sedan.dealx.json
  showcase__model__vehicle.dealx.json
  showcase__packages__interfaces__connections.deal.json
  showcase__packages__interfaces__electrical.deal.json
  showcase__packages__interfaces__index.deal.json
  showcase__packages__interfaces__thermal.deal.json
  showcase__packages__requirements__index.deal.json
  showcase__packages__requirements__needs.deal.json
  showcase__packages__requirements__system.deal.json
  showcase__packages__use-cases__charging.deal.json
  showcase__packages__use-cases__driving.deal.json
  showcase__packages__use-cases__index.deal.json
  showcase__packages__vehicle__battery.deal.json
  showcase__packages__vehicle__behaviors.deal.json
  showcase__packages__vehicle__components.deal.json
  showcase__packages__vehicle__index.deal.json
  showcase__packages__vehicle__motor.deal.json
```

Pattern: `showcase__<path>.json` with every `/` replaced by `__`.

## Verification Results

```
$ zig build                                                         → libdeal.a + deal.h
$ zig build test --summary all                                      → 8/8 tests pass
    - lib.zig stub-compiles (2)
    - lexer.mode_flip + lexer.keywords + lexer.comments + lexer.templates (4)
    - lexer.snapshot + lexer.snapshot spans monotonic (2)
$ zig build test -Dtest-filter=lexer.mode_flip                      → 1/1
$ zig build test -Dtest-filter=lexer.keywords                       → 1/1
$ zig build test -Dtest-filter=lexer.comments                       → 1/1
$ zig build test -Dtest-filter=lexer.templates                      → 1/1
$ zig build test -Dtest-filter=lexer.snapshot                       → 2/2 (snapshot + monotonic)
$ cargo test --manifest-path tests/ffi/Cargo.toml                   → 3/3 (no regression)
$ grep -l '"k":"unknown"' tests/snapshots/tokens/*.json             → empty (locked gate)
$ ls tests/snapshots/tokens/*.json | wc -l                           → 19
$ grep -c '"mode":"deal"' tests/snapshots/tokens/*.deal.json | wc -l → 15
$ grep -c '"mode":"dealx"' tests/snapshots/tokens/*.dealx.json | wc -l → 4
$ grep -v '^[[:space:]]*//' src/keywords.zig | grep -c '\.kw_'      → 85 (locked)
$ grep -c 'callconv(\.C)' src/*.zig                                 → 0 (uppercase banned)
$ grep -c 'ArrayList(.*)\.init(' src/*.zig                          → 0 (no stored-allocator)
```

## Decisions Implemented (cross-ref to CONTEXT)

- **D-05** Zig 0.16.0 idioms: `callconv(.c)` lowercase, `ArrayList(T) = .empty`, `std.StaticStringMap(V).initComptime(.{})` — all green per grep gates.
- **D-06** Four-mode dispatch — observable through `lexer.mode_flip`.
- **D-15** `Span = { start: u32, end: u32 }` byte offsets only — every Token carries Span, no text slot, snapshot JSON uses `"span":[s,e]`.
- **D-16** E0001..E0099 lexer error code range reserved — `lexer.comments` cites E0001 as the diagnostic code Plan 05 will emit for unterminated-comment recovery.
- **D-18** Alphabetical field order — token JSON `{k, span, text}` is alphabetical; top-level `{v, mode, tokens}` is the canonical envelope order.

## Next Phase Readiness

- The lexer is ready for Plan 03 (parser-deal). The parser will call `Lexer.init(source)` then drive `Lexer.next(mode)` per its grammar; the four modes are observable and tested.
- The snapshot infrastructure (snapshot naming convention, `-Dupdate-snapshots` flag, byte-compare with `.actual` write-back on mismatch) is reusable by Plan 03's `ast_snapshot` test.
- `keywords.dealx_tag_names` is in place for Plan 04 to validate composition-tag heads with a comptime hash lookup.
- No outstanding RESEARCH assumptions blocking Plan 03.

---

*Phase: 01-zig-compiler-core*
*Plan: 01-02-lexer*
*Completed: 2026-05-19*

## Self-Check: PASSED

- All 19 snapshot files exist on disk: confirmed via `ls tests/snapshots/tokens/*.json | wc -l → 19`.
- Three task commits exist: `f783d02`, `62f44f8`, `cb4562b` — verified via `git rev-parse --short HEAD~{0,1,2}`.
- `zig build && zig build test && cargo test --manifest-path tests/ffi/Cargo.toml` all green at the time of writing.
- Locked gate "zero `.unknown` tokens across the corpus" is met: `grep -l '"k":"unknown"' tests/snapshots/tokens/*.json → empty`.
- All four `-Dtest-filter=lexer.{mode_flip,keywords,comments,templates}` invocations pass individually.
- Plan 02's deviation list is documented above (6 auto-fixed, 0 architectural).
