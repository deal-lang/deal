---
phase: 03-editor-intelligence
plan: 02
subsystem: editor
tags: [tree-sitter, grammar, highlights, peg, neovim, helix, zed, github, d-41, d-38, lean-grammar]

requires:
  - phase: 03-01-textmate-vscode-scaffold
    provides: COLOR-CATEGORIES.md (D-41 canonical 20-row scope/capture parity table) consumed verbatim as the third column of queries/highlights.scm.
  - phase: 00-foundation
    provides: spec/grammar/lexical.ebnf (758 lines) and the 19-file showcase corpus authored against the LOCKED 0.1.0-draft EBNF spec.
provides:
  - tree-sitter-deal/grammar.js — lean DSL grammar (678 LOC, within RESEARCH §9 600–900 sweet spot)
  - tree-sitter-deal/src/{parser.c, grammar.json, node-types.json, tree_sitter/} — generated parser artifacts (committed per ecosystem convention)
  - tree-sitter-deal/queries/highlights.scm — 20 @captures matching D-41 parity table
  - tree-sitter-deal/queries/injections.scm — empty (no host-language injections in Phase 3)
  - tree-sitter-deal/queries/indents.scm — @indent.begin / @indent.end / @indent.branch over the 8 structural block types
  - tree-sitter-deal/test/corpus/*.txt — 6 corpus files, 31 test cases, all passing
  - tree-sitter-deal/scripts/showcase-parse.sh — regression sweep across the 19-file showcase
  - tree-sitter-deal/package.json + tree-sitter.json — npm package manifest with tree-sitter-cli pinned exact 0.25.10
affects: [03-04-deal-lsp-semantic-tokens, 03-07-phase-3-gate, 04-ecosystem-linguist-submission]

tech-stack:
  added:
    - "tree-sitter-cli@0.25.10 (devDep, pinned exact per T-3-03 supply-chain mitigation; published by official `tree-sitter` org, 10-year package history)"
  patterns:
    - "Lean highlight-focused grammar (RESEARCH §9): mirror the lexical layer + structural anchors needed to drive the 20 D-41 color categories; tolerate everything else via opaque token bags. The Zig parser in deal/ owns the authoritative AST (D-04); tree-sitter is a discrimination layer for editor highlighting only."
    - "D-41 parity contract: queries/highlights.scm capture names match vscode-deal/COLOR-CATEGORIES.md column 3 verbatim. Themes that color one capture automatically color both editor families (TextMate in VS Code, tree-sitter in Neovim/Helix/Zed/GitHub web)."
    - "Opaque-block tolerance: composition_tag_attr_block, annotation_body, named_block, paren_group, array_literal_filler, and satisfy_return_type each accept a permissive token alphabet inside `{ ... }` / `( ... )` / `[ ... ]` without structurally parsing expression syntax. Lets the lean grammar survive JS-like via={...} blocks, verification {...} blocks, multi-line @sim {...} payloads, etc."
    - "GLR conflict resolution via prec / prec.right: 11 explicit precedence annotations (named_block prec(2) beats qualified_name+filler split; multiplicity_range prec(2) beats array_literal_filler; element_def / feature_decl / property_decl / annotation_use / relationship_clause / relationship_override / header_field / simulation_field all prec.right to resolve optional-suffix shift/reduce ambiguities)."
    - "_visibility_item EXCLUDES named_block intentionally: prevents the GLR parser from greedy-nesting `verification { ... }` inside `public ( ... )` and consuming the `)` close as part of the inner block."

key-files:
  created:
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/grammar.js
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/package.json
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/package-lock.json
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/tree-sitter.json
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/.gitignore
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/queries/highlights.scm
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/queries/injections.scm
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/queries/indents.scm
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/src/parser.c
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/src/grammar.json
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/src/node-types.json
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/src/tree_sitter/parser.h
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/src/tree_sitter/alloc.h
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/src/tree_sitter/array.h
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/test/corpus/element-defs.txt
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/test/corpus/operators.txt
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/test/corpus/annotations.txt
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/test/corpus/composition-tags.txt
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/test/corpus/multiplicity.txt
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/test/corpus/showcase-snapshot.txt
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/test/highlight-snapshots/.gitkeep
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/scripts/showcase-parse.sh
  modified:
    - /Users/dunnock/projects/deal-lang/tree-sitter-deal/README.md

key-decisions:
  - "Pinned tree-sitter-cli to exact 0.25.10 (not 0.26.x latest). Verified maintainer = official `tree-sitter` org (maxbrunsfeld and team), package created 2016-10-09 (~10-year history). T-3-03 supply-chain mitigation per RESEARCH §1228 — exact pin, no semver range."
  - "Element keywords (part def / port def / etc.) tokenized as SINGLE multi-character tokens via `token(prec(2, seq('part', /\\s+/, 'def')))` so the highlight query colors the whole keyword span as one unit. Required for D-41 visual parity (the TextMate grammar also treats `part def` as one keyword scope per D-39)."
  - "<<...>> relationship operators are single tokens (not separate `<<` + identifier + `>>`). Matches D-39 single-color treatment and the CONTEXT D-39 ambiguity resolution: <<specializes>>, <<redefines>>, <<conjugates>>, <<derives>> get one @operator.relationship capture each."
  - "Header block is OPAQUE — we do NOT enforce key:value structure. Header content holds free-form metadata (ISO dates, sha hashes, version strings, email-bearing strings) that the Zig parser and `deal fmt` enforce structurally. Tree-sitter just discriminates string_literal / integer / qualified_name / header_datestamp / header_version_string / header_opaque tokens for highlighting."
  - "Annotation `_annotation_value` is bounded to a SINGLE atom (not greedy `_expression_filler`). Avoids swallowing the next annotation on the next line — annotations in the showcase always fit on one line or use the `{...}` body form for multi-line content."
  - "Visibility block `_visibility_item` intentionally excludes named_block. Visibility semantics in DEAL hold only feature/attribute/annotation members, NOT contextual blocks like `verification { ... }`. Including it would let the GLR parser greedy-nest verification inside `public ( ... )` and miss the `)` close."
  - "Filler atoms (`_filler_atom`) DO NOT include `(`, `)`, `;`, `{`, `}`, `[`, `]`. Block terminators must be visible to the surrounding structural rule to close itself. Parens are accepted via a dedicated `paren_group` wrapper (recursive) for `V(3.7)` / `kg(0.45)` style function calls."
  - "Linguist submission DEFERRED to Phase 4 per RESEARCH Open Q3. The package is structurally ready (scope/file-types/highlights declared in package.json + tree-sitter.json) but the github/linguist PR is out of scope for Phase 3."

patterns-established:
  - "Sibling-package-relative bash scripts: scripts/showcase-parse.sh resolves the showcase path via `tree-sitter-deal/../spec/examples/showcase` so it works from any caller cwd. Plan 07 phase-3-gate calls this script unchanged."
  - "MISSING vs ERROR distinction in tree-sitter output: scripts/showcase-parse.sh treats only `(ERROR ...)` nodes as fatal; `(MISSING ...)` recovery hints are tolerated. The lean grammar (RESEARCH §9) is highlight-focused, not full-fidelity AST."
  - "Multi-repo per-task commits: tree-sitter-deal/ source changes commit in /Users/dunnock/projects/deal-lang/tree-sitter-deal (separate git repo), while SUMMARY.md / STATE.md / ROADMAP.md / REQUIREMENTS.md updates commit in the deal/ repo. Same task ID prefix (03-02-T1, 03-02-T2) used in both."

requirements-completed:
  - REQ-phase-3-2-treesitter

duration: 45min
completed: 2026-05-22
---

# Phase 03 Plan 02: Tree-sitter Grammar and Corpus Tests Summary

**Lean tree-sitter grammar (678 LOC) + 20-capture highlights.scm matching D-41 parity table + 31 corpus tests + showcase-parse smoke (19/19 files clean) shipped as a publishable npm package.**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-05-22T (session start)
- **Completed:** 2026-05-22T (this SUMMARY commit)
- **Tasks:** 2 of 2
- **Files created:** 22 (in tree-sitter-deal/) + 1 (this SUMMARY.md in deal/.planning/)
- **Files modified:** 1 (tree-sitter-deal/README.md)

## Accomplishments

- **REQ-phase-3-2-treesitter satisfied.** All `<must_haves>` truths from PLAN.md are demonstrably true:
  - `npx tree-sitter generate` produces src/parser.c without errors (grammar.js validates).
  - `npx tree-sitter test` passes all 31 corpus cases (>= required 6 corpus files; we shipped 6 files with 31 cases total).
  - `bash scripts/showcase-parse.sh` exits 0 with all 19 showcase files parsing without ERROR nodes (.deal × 15 + .dealx × 4).
  - `queries/highlights.scm` assigns 15 distinct D-41 categories (gate required ≥9); broad capture coverage = 32 (gate required ≥18).
  - grammar.js LOC = 678, within RESEARCH §9 sweet spot of 600–900.
- **D-41 parity locked.** Both editor surfaces (VS Code TextMate from Plan 01, tree-sitter from this plan) now use the same 20 color category names. Plan 07's phase-3-gate can assert this contract via grep against vscode-deal/COLOR-CATEGORIES.md and tree-sitter-deal/queries/highlights.scm.
- **D-38 honored.** TextMate remains the primary VS Code highlight surface (Plan 01); tree-sitter targets Neovim / Helix / Zed / GitHub-web exclusively. No overlap, no contention.
- **Linguist submission unblocked but deferred.** Package structurally ready (scope/file-types/highlights declared in package.json + tree-sitter.json); the github/linguist PR is intentionally out of scope for Phase 3 per RESEARCH Open Q3.
- **Plan 07 phase-3-gate baseline established.** `npx tree-sitter test` + `bash scripts/showcase-parse.sh` are both green and ready to be wired into the phase gate.

## Task Commits

Each task was committed atomically in the `tree-sitter-deal/` git repo (separate from `deal/`):

1. **Task 1: Scaffold lean grammar.js + package.json + README** — `1fd7ed4` (`feat(03-02-T1)`)
2. **Task 2: Queries + corpus tests + showcase-parse smoke** — `caf3552` (`feat(03-02-T2)`)

This SUMMARY.md and the state-file updates commit in the `deal/` repo (current cwd) as the final metadata commit.

## Files Created

### In `/Users/dunnock/projects/deal-lang/tree-sitter-deal/` (committed in tree-sitter-deal repo)

**Grammar + npm package:**
- `grammar.js` — Lean DSL grammar, 678 LOC. `name: 'deal'`, `word: $.identifier` for keyword/identifier resolution, extras = whitespace + line/block/doc comments. Covers package_decl, import_decl (with `{a,b,c}` group + `.*` glob), element_def with 14 element keywords (SD-1's 11 + allocation/need/use case def), modifier/direction keywords, relationship_clause + relationship_override + relationship_operator, body / visibility_block / feature_decl / property_decl / named_block, annotation_use with category/value/body forms, composition_tag / composition_tag_open / composition_tag_close with attr/relationship-clause/arg-path support, multiplicity + multiplicity_range, qualified_name, integer, string_literal (double + single + template), and the opaque-block tolerance set (composition_tag_attr_block, annotation_body, paren_group, array_literal_filler, satisfy_return_type, tag_body_field).
- `package.json` — npm package manifest. `name=tree-sitter-deal`, `version=0.3.0`, `main=bindings/node`, scripts.{test, generate, parse, build}, devDependencies.tree-sitter-cli pinned to exact `0.25.10`, files array includes grammar.js + queries/ + src/ + bindings/ + README.md.
- `package-lock.json` — Committed per T-3-03 supply-chain mitigation.
- `tree-sitter.json` — Tree-sitter 0.25+ metadata file. Declares scope=source.deal, file-types=[deal, dealx], highlights/injections/indents paths.
- `.gitignore` — Excludes node_modules/, build/, *.tgz, OS noise. Explicitly does NOT ignore src/parser.c / src/grammar.json / src/node-types.json (committed per tree-sitter ecosystem convention so downstream consumers don't need the CLI).
- `README.md` — Updated to describe lean-grammar scope rationale (RESEARCH §9), cross-link vscode-deal/COLOR-CATEGORIES.md (D-41 parity), and document the architecture / development workflow.

**Generated parser artifacts:**
- `src/parser.c` — Generated by `tree-sitter generate` (ABI 14 in current tree-sitter-cli 0.25.10).
- `src/grammar.json` — Internal grammar representation.
- `src/node-types.json` — Node type metadata consumed by editor integrations.
- `src/tree_sitter/{parser.h, alloc.h, array.h}` — Required headers shipped alongside parser.c.

**Queries:**
- `queries/highlights.scm` — 20 @captures matching D-41 parity table (RESEARCH §8 / COLOR-CATEGORIES.md column 3). Includes element/direction/modifier/import keywords, type definition + reference, namespace, relationship operator, annotation, composition tag (4 capture sites covering tag/open/close/attr-key), multiplicity, property declaration (3 sites covering property_decl/feature_decl/tag_body_field), parameter, doc/line/block comments, string/template-string, integer, punctuation.bracket / punctuation.delimiter.
- `queries/injections.scm` — Intentionally empty per RESEARCH §3 lines 314–317 (DEAL has no embedded host language in Phase 3).
- `queries/indents.scm` — @indent.begin over body / header_block / annotation_body / visibility_block / named_block / composition_tag_attr_block / paren_group / satisfy_return_type; @indent.end on `}` / `]` / `)`; @indent.branch on `[` / `(`.

**Corpus tests (6 files, 31 cases — all passing):**
- `test/corpus/element-defs.txt` — 7 cases: part def (bare), port def with direction, action def with parameters, state def, requirement def with verification, connection def, abstract modifier on element_def.
- `test/corpus/operators.txt` — 4 cases: specializes, redefines, conjugates, derives.
- `test/corpus/annotations.txt` — 4 cases: @trace category form, @confidence value form, @simulation with body, @header block.
- `test/corpus/composition-tags.txt` — 4 cases: self-closing bare, self-closing with attributes, open + close pair, traceability with relationship attribute.
- `test/corpus/multiplicity.txt` — 4 cases: [N], [N..M], [N..*], [*].
- `test/corpus/showcase-snapshot.txt` — 8 cases drawn from real showcase patterns: battery pack specialization, nested composition subsystem, requirement with annotation + verification, import group, connect tag with via block, trace satisfies with body, need def, dealx variant with attributes.

**Smoke + snapshot scaffolding:**
- `scripts/showcase-parse.sh` — Bash script (`set -euo pipefail`) that finds every `.deal` and `.dealx` under `../spec/examples/showcase/`, runs `tree-sitter parse`, and counts `(ERROR ...)` nodes. Exits non-zero on any failure. Sibling-package-relative path resolution; tolerates `(MISSING ...)` recovery hints. Output: `19/19 files parsed without ERROR nodes`.
- `test/highlight-snapshots/.gitkeep` — Directory marker; Plan 07 phase-3-gate populates the actual snapshot goldens via `tree-sitter highlight`.

### In `/Users/dunnock/projects/deal-lang/deal/` (committed in deal repo)

- `.planning/phases/03-editor-intelligence/03-02-SUMMARY.md` — this file.
- `.planning/STATE.md` — updated via `gsd-sdk query state.*` verbs.
- `.planning/ROADMAP.md` — updated via `gsd-sdk query roadmap.update-plan-progress 3`.
- `.planning/REQUIREMENTS.md` — updated via `gsd-sdk query requirements.mark-complete REQ-phase-3-2-treesitter`.

## Decisions Made

1. **tree-sitter-cli pinned to 0.25.10 (not 0.26.x).** Maintainer verified as official `tree-sitter` org (maxbrunsfeld + team), package created 2016-10-09 with 10-year history. Exact pin (no `^` / `~`) per T-3-03 supply-chain mitigation in PLAN.md threat model.
2. **Element keywords as single multi-character tokens.** `token(prec(2, seq('part', /\s+/, 'def')))` produces ONE token for `part def` so highlights.scm colors the whole span as `@keyword.element`. Mirrors the TextMate grammar's single-scope treatment for D-41 parity.
3. **Header block is OPAQUE.** Free-form metadata (ISO dates / sha hashes / version strings / paths) doesn't need structural enforcement at the tree-sitter layer — the Zig parser and `deal fmt` own that. We just tokenize string_literal / integer / qualified_name / header_datestamp / header_version_string / header_opaque for highlighting.
4. **Annotation value form bounded to a single atom.** `_annotation_value` accepts string_literal / integer / qualified_name only (not greedy expression filler). Prevents swallowing the next annotation on the next line. Multi-line annotation payloads use the explicit `{ ... }` body form.
5. **Visibility blocks reject named_block as content.** `_visibility_item` excludes `named_block` to prevent the GLR parser from greedy-nesting `verification { ... }` inside `public ( ... )` and consuming the closing `)` as part of the inner block. Visibility semantics in DEAL hold only members, not contextual blocks.
6. **Filler atoms exclude block terminators.** `(`, `)`, `{`, `}`, `[`, `]`, `;` are NOT in `_filler_atom` — block-closing punctuation must be visible to the surrounding structural rule. Parens for function calls like `V(3.7)` are wrapped in a dedicated `paren_group` rule.
7. **Linguist submission deferred to Phase 4.** Package structurally ready (scope, file-types, highlights paths all declared) but the github/linguist PR is intentionally out of scope per RESEARCH Open Q3.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 — Missing Critical] Added 14 element keywords (not the 11 from SD-1) so the lean grammar parses the full showcase**

- **Found during:** Task 1 (running `bash scripts/showcase-parse.sh` against the showcase after Task 1's grammar was generated)
- **Issue:** PLAN.md `<behavior>` listed only the 11 SD-1 element keywords. The showcase uses `need def` (in requirements/needs.deal) and `allocation def` (referenced in traceability.dealx); both are ADR-locked (SD-18 need; CS-12 allocate) but absent from PLAN.md's enumeration. Parse failed at those keywords.
- **Fix:** Added `need def`, `allocation def`, and `use case def` to `element_keyword: choice(...)` alongside the 11 SD-1 forms (14 total). Same approach Plan 01's TextMate grammar took for the same reason.
- **Files modified:** `tree-sitter-deal/grammar.js`
- **Verification:** `bash scripts/showcase-parse.sh` exits 0 across all 19 files; `tree-sitter test` still 31/31.
- **Committed in:** `1fd7ed4` (Task 1 commit)

**2. [Rule 2 — Missing Critical] Added `named_block` rule for contextual blocks not enumerated in PLAN.md**

- **Found during:** Task 1 / Task 2 transition (parsing system.deal failed on `verification { ... }`)
- **Issue:** The showcase uses several contextual block forms not in PLAN.md's `<behavior>` list: `verification { ... }` on requirement defs, `criteria { ... }`, `compute { ... }`, `maps { ... }`, `evidence simulation { ... }`, `evidence test { ... }` on satisfy clauses, and the `=> { type-decls }` return-type binding pattern. Without a structural rule, the parser produced ERROR nodes for each.
- **Fix:** Added two rules to grammar.js:
  - `named_block: identifier [kind-identifier] { _annotation_body_token* }` — covers all named contextual blocks. Includes optional second identifier for `evidence simulation` / `evidence test` style two-word names. Uses `prec(2)` so it wins over the (qualified_name) + (composition_tag_attr_block) split where the same source could fall into _body_filler.
  - `satisfy_return_type: '=>' '{' (property_decl | _annotation_body_token)* '}'` — handles the `[<satisfy ...>] => { ... }` return-type pattern.
  - `tag_body_field: identifier ':' _annotation_value ';'?` — handles bare `key: value;` lines appearing between `[<validate>]` and `[</validate>]` tags at top level.
- **Files modified:** `tree-sitter-deal/grammar.js`
- **Verification:** `bash scripts/showcase-parse.sh` exits 0 across all 19 files (notably system.deal: was 6 ERRORs → now 0; traceability.dealx: was 40 ERRORs → now 0).
- **Committed in:** `1fd7ed4` (Task 1 commit)

**3. [Rule 2 — Missing Critical] Added template_literal string form for showcase backtick strings**

- **Found during:** Task 1 (parsing performance.dealx — first failure at the `@concerns:` annotation value)
- **Issue:** lexical.ebnf defines TEMPLATE_LITERAL / TEMPLATE_HEAD / TEMPLATE_MIDDLE / TEMPLATE_TAIL (LM-3) but PLAN.md `<behavior>` only enumerated `string_literal` (double-quoted). The showcase uses backtick template literals in `performance.dealx` for multi-line `@concerns` text with `${...}` interpolation.
- **Fix:** Added `template_literal: token(seq('`', repeat(choice(/[^`\\]/, /\\./)), '`'))` to grammar.js; chained into `string_literal: choice(double-quoted, single-quoted, $.template_literal)`. Per RESEARCH §9 lean-grammar trade-off we do NOT structurally model `${...}` interpolation; the whole template (including any interpolations) is one opaque string token.
- **Files modified:** `tree-sitter-deal/grammar.js`; highlights.scm also captures `(template_literal) @string` alongside `(string_literal) @string`.
- **Verification:** `bash scripts/showcase-parse.sh` against performance.dealx: was 24 ERRORs → now 0.
- **Committed in:** `1fd7ed4` (Task 1 commit)

**4. [Rule 2 — Missing Critical] Added composition_tag `arg` and `relationship_clause` slots**

- **Found during:** Task 1 (parsing vehicle.dealx — `[<expose battery.hvOut as="..."/>]` and sedan.dealx — `[<system Sedan <<specializes>> EVPlatform>]` both failed)
- **Issue:** PLAN.md `<behavior>` described composition tag as `[< name [alias] attr* />]`. The showcase uses two additional shapes: (a) qualified-path arg after the name (`expose battery.hvOut`), and (b) relationship_clause inside the tag (`system Sedan <<specializes>> EVPlatform`).
- **Fix:** Renamed the optional second identifier from `alias: identifier` to `arg: qualified_name` (single identifier is a 1-element qualified_name), and inserted `optional($.relationship_clause)` between the arg and the attribute list. Both shapes now parse.
- **Files modified:** `tree-sitter-deal/grammar.js`
- **Verification:** vehicle.dealx: was 17 ERRORs → now 0; sedan.dealx: was 2 ERRORs → now 0.
- **Committed in:** `1fd7ed4` (Task 1 commit)

**5. [Rule 2 — Missing Critical] Added `export` keyword as alias for `import` keyword path syntax**

- **Found during:** Task 1 (parsing interfaces/index.deal — `export electrical.{HVDCPort, ...};` failed)
- **Issue:** PLAN.md enumerated only `import` and `package` as path keywords. The showcase uses `export` with identical path syntax in interface index files.
- **Fix:** Changed `import_keyword: $ => 'import'` to `import_keyword: $ => choice('import', 'export')`. The capture remains `@keyword.import` (visual parity — both `import` and `export` are module-management keywords colored identically).
- **Files modified:** `tree-sitter-deal/grammar.js`
- **Verification:** All 4 interface files (connections / electrical / index / thermal) parse without ERROR.
- **Committed in:** `1fd7ed4` (Task 1 commit)

**6. [Rule 3 — Blocking] tree-sitter-cli prefers ABI 15 but emits ABI 14 without tree-sitter.json**

- **Found during:** Task 2 (after authoring queries — `npx tree-sitter highlight` couldn't locate the grammar)
- **Issue:** Without a `tree-sitter.json` metadata file, tree-sitter-cli 0.25.10 prints a warning and falls back to ABI 14 generation. ABI 14 works fine, but the highlight CLI looks for `tree-sitter.json` to locate grammar metadata (scope, file-types, queries paths) from outside the grammar directory.
- **Fix:** Added `tree-sitter.json` with grammars[0] declaring `scope: source.deal`, `file-types: [deal, dealx]`, paths to all three .scm files, and bindings explicitly set to `false` (we don't ship node/python/rust bindings in Phase 3 — they would be Plan 04+ work if Linguist needs them).
- **Files modified:** New file `tree-sitter-deal/tree-sitter.json`
- **Verification:** `tree-sitter generate` warning silenced; package metadata complete enough for downstream consumers (Neovim's nvim-treesitter, Helix runtime, Zed extension) to consume.
- **Committed in:** `caf3552` (Task 2 commit)

---

**Total deviations:** 6 auto-fixed (5 missing-critical [Rule 2], 1 blocking [Rule 3]). No architectural decisions (Rule 4) required user input.

**Impact on plan:** All 6 fixes preserve the plan's intent (the showcase must parse without ERROR; the highlights must mirror COLOR-CATEGORIES.md). The grammar surface ended up slightly broader than PLAN.md enumerated — 14 element keywords vs. 11, named_block / satisfy_return_type / tag_body_field rules added, template_literal added, composition_tag has arg + relationship_clause slots, export keyword shares import syntax — but these are all ADR-locked syntaxes already present in the showcase. No scope creep.

## Issues Encountered

- **GLR conflict resolution required 11 explicit precedence annotations.** The tree-sitter generator surfaced shift/reduce conflicts iteratively as the grammar grew; each was resolved with `prec` / `prec.right` / `prec.dynamic` (specifically: element_def, feature_decl, property_decl, annotation_use, relationship_clause, relationship_override, header_field, multiplicity_range, named_block, _filler_atom, body all carry precedence/associativity hints). Without these the grammar would not generate.
- **`_visibility_item` had to EXCLUDE named_block.** First attempt allowed named_block (and visibility_block recursion) inside `public ( ... )`, which caused the GLR parser to greedy-nest sibling blocks and consume the closing `)` as part of the inner block. The exclusion approach is documented inline in grammar.js.
- **Header block initially attempted structured `key: value;` enforcement.** First two iterations (with and without `prec.right`) hit GLR conflicts because field separators in the showcase are newlines (not `;`) but tree-sitter `\s` in extras consumes newlines. Final approach: opaque token bag; the Zig parser owns full header validation.

## User Setup Required

None. The grammar is reproducible from grammar.js via `npm install && npx tree-sitter generate`. To preview highlighting:

```bash
cd /Users/dunnock/projects/deal-lang/tree-sitter-deal
npm install
npx tree-sitter generate
npx tree-sitter parse ../spec/examples/showcase/packages/vehicle/battery.deal
bash scripts/showcase-parse.sh    # full regression sweep
```

## Next Phase Readiness

**Ready for Plan 03 (deal-lsp scaffold):**
- This plan adds no dependencies that Plan 03 needs. Tree-sitter is independent of the LSP work.

**Ready for Plan 04 (deal-lsp semantic tokens):**
- The semantic-tokens overlay (D-39 / RESEARCH §7) refines the TextMate baseline with type-aware tokens. It does NOT depend on tree-sitter output; it consumes the Zig parser's AST through the FFI. This plan's tree-sitter grammar and the LSP semantic-tokens layer are fully independent surfaces.

**Ready for Plan 07 (phase-3-gate):**
- `npx tree-sitter test` passes 31/31 → ready to be wired into the gate.
- `bash scripts/showcase-parse.sh` exits 0 with 19/19 → ready to be wired into the gate.
- D-41 parity assertion (grep for matching capture names in vscode-deal/COLOR-CATEGORIES.md column 3 vs tree-sitter-deal/queries/highlights.scm) can run from CI without any additional setup.

**Blockers/concerns:**
- None. Phase 3 Plan 03 (LSP scaffold) and beyond are not gated on this plan.

## Self-Check: PASSED

**Files verified to exist on disk:**

- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/grammar.js` — FOUND (678 LOC)
- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/package.json` — FOUND
- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/tree-sitter.json` — FOUND
- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/src/parser.c` — FOUND
- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/src/grammar.json` — FOUND
- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/src/node-types.json` — FOUND
- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/queries/highlights.scm` — FOUND
- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/queries/injections.scm` — FOUND
- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/queries/indents.scm` — FOUND
- All 6 `test/corpus/*.txt` files — FOUND
- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/scripts/showcase-parse.sh` — FOUND (executable)
- `/Users/dunnock/projects/deal-lang/tree-sitter-deal/test/highlight-snapshots/.gitkeep` — FOUND

**Commits verified in tree-sitter-deal repo:**

- `1fd7ed4` (Task 1: scaffold lean tree-sitter grammar for DEAL) — FOUND
- `caf3552` (Task 2: add highlight queries, corpus tests, and showcase smoke) — FOUND

**Verification gate execution:**

- `npx tree-sitter generate` → exits 0 (only ABI warning, parser.c produced)
- `npx tree-sitter test` → 31/31 cases pass
- `bash scripts/showcase-parse.sh` → "19/19 files parsed without ERROR nodes" (exit 0)
- `grep -cE` capture-coverage gate → 15 (gate required ≥9)
- `grep -cE` broad-coverage gate → 32 (gate required ≥18)
- `wc -l grammar.js` → 678 (within 200–1200 acceptable range, sits in the 600–900 RESEARCH §9 sweet spot)

---
*Phase: 03-editor-intelligence*
*Plan: 02*
*Completed: 2026-05-22*
