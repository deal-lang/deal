---
phase: 1
phase_name: "zig-compiler-core"
project: "DEAL — Digital Engineering Authoring Language"
generated: "2026-05-20"
focus: "Testing gaps surfaced during /gsd-validate-phase audit"
counts:
  decisions: 12
  lessons: 10
  patterns: 9
  surprises: 8
missing_artifacts:
  - "01-UAT.md (no UAT artifact for this phase)"
---

# Phase 1 Learnings: zig-compiler-core

> Extraction focus: testing gaps identified during `/gsd-validate-phase 1`.
> Every category below is filtered through the lens of test quality —
> what worked, what hid behind ✅ checkmarks, and what to do differently
> in Phase 2.

## Decisions

### D-06 Stateless four-mode lexer dispatch
Lexer receives `mode` on every `next()` call; no `Lexer.mode` field. Parser
owns the mode stack.

**Rationale:** Lets the same byte sequence (`[<sys>]`) lex differently per
context without lexer state. Made `lexer.mode_flip` testable as a true
multi-dimensional BDD test (same input, three modes, three different
token streams).
**Source:** 01-02-SUMMARY.md, 01-03-SUMMARY.md

---

### D-07 Parser-owned open-tag stack with pop-anyway recovery
`pushTag`/`popTag` live on the Parser struct (not parser_dealx) because
parser_dealx imports parser_deal — reverse direction would cycle.

**Rationale:** Avoided circular import; placed tag balancing where it can
also drive D-17 sync. Enabled `parser_dealx.tag_balance` to assert all
four tag-balance error codes (E0301–E0304) and their dual-span shapes —
one of the strongest BDD tests in the phase.
**Source:** 01-04-SUMMARY.md

---

### D-08 Inline `{...}` accepts full deal expression via mode-switch with errdefer
`parseInlineObjectOrExpr` re-enters `expr.parseExpression` in
`.dealx_expr_brace`. The T-04-03 errdefer + explicit success-path
`restoreMode` is **mandatory**; plain `defer restoreMode(prev)` is
forbidden.

**Rationale:** Threat-model-locked pattern — plain defer leaks the
previous mode on the error path. Acceptance gate is grep-verified.
**Source:** 01-04-SUMMARY.md

---

### D-14 Full Diagnostic data model with comptime-fmt enforcement
`Diagnostic` carries code, severity, message, span, secondary_spans,
fix_it, notes. `DiagnosticCollector.emitFmt` requires comptime fmt
strings.

**Rationale:** T-05-03 / T-DiagInjection mitigation — source bytes can
never reach `std.fmt` machinery because Zig 0.16.0 rejects non-comptime
fmt at compile time. Enabled `diag.json_roundtrip` as a complete-field
BDD test.
**Source:** 01-05-SUMMARY.md

---

### D-17 Three-tier synchronization (statement / definition / tag)
`syncToStatement`, `syncToDefinition`, `syncToTag` — built on
`std.EnumSet(lexer.Tag)`.

**Rationale:** Each tier has a different follow set; a single sync
function would over- or under-skip. **Trade-off accepted:** sync helpers
wired strategically at top-level loops and `parseDefinitionBody` member
loop rather than at all 70+ `expect()` callsites (Warning #7 pragmatic
fix). This trade-off is the *root cause* of the recovery-test weakness
flagged in the validation audit.
**Source:** 01-05-SUMMARY.md (decisions key, Warning #7)

---

### D-18 Alphabetical JSON payload field order locked
Every payload arm in `json.zig` emits fields alphabetically.

**Rationale:** Pre-commits to deterministic snapshot bytes across the
19-file corpus. **However, this invariant is enforced only by
snapshot-byte-equality — no test asserts that *parsing twice produces
identical JSON*.** The determinism contract is real; its automated
guard is incomplete.
**Source:** 01-03-SUMMARY.md, 01-04-SUMMARY.md

---

### MAX_PARSE_DEPTH = 256 / MAX_TAG_DEPTH = 1024 bound
DoS mitigations T-05-01 (expression recursion) and T-04-01 (tag
nesting).

**Rationale:** Bounded depth at `parseExpression` entry catches all
mutually-recursive cycles (parsePrefix/parsePrimary/parseExpression).
E0303 / E0403 emitted with sentinel returns so callers can synchronize.
`parser_dealx.tag_balance` Case D exercises the 1024-depth bound with
a real 1025-deep input. **No equivalent test exercises the E0403 path** —
m20_deeply_nested_braces.deal exists but `recovery.corpus` doesn't pin
the expected error code per file.
**Source:** 01-05-SUMMARY.md

---

### deal_parse_internal as `pub fn` (not `pub export fn`)
Test-only handle accessor reachable from in-tree tests but absent from
`nm` symbol table.

**Rationale:** Lets `c_abi.no_leaks` use `std.testing.allocator` as the
leak oracle. Symbol-table assertion (`nm | grep _deal_internal | wc -l = 0`)
makes this discoverable as a hard invariant — one of the few places where
the "should-NOT-appear" dimension is enforced.
**Source:** 01-06-SUMMARY.md

---

### IDENT(args) always parsed as Call
LL(2) disambiguation of unit calls (`V(3.7)`) vs function calls
(`foo(x)`) deferred to Phase 2 semantic analysis.

**Rationale:** Plan 04's inline expressions use `IDENT OBJECT_LITERAL`
not `IDENT CALL`, so no grammar conflict at the parse layer. Phase 2
must reclassify by resolving the callee type.
**Source:** 01-03-SUMMARY.md

---

### peek2 (not parse-and-recover) for LL(2) disambiguation
Cheap 2-token lookahead by save/restore of `lex.pos`.

**Rationale:** Required for `path . {` (import) vs `path . ident`
(qualified name) vs `IDENT { …` (object literal) vs `IDENT : …` (bare
object). The original peek2 implementation double-advanced; the bug was
caught by snapshot regression, not by a dedicated peek2 unit test.
**Source:** 01-03-SUMMARY.md (auto-fix #1)

---

### Recovery corpus soft 60% gate (not per-file pinning)
50 malformed files; hard gate is "no panics + parser non-null"; soft
gate is "≥60% files produce ≥1 diagnostic."

**Rationale:** Mutations (swap_pair into strings, drop_byte in comments,
SD-10 optional-semicolon positions) don't always produce syntactic
errors — pinning every file would couple the test to fragile mutation
semantics. **Cost:** the soft gate does not assert *which* diagnostic
appears, so m08 (unterminated block comment) silently slipping through
the parser is invisible to the test.
**Source:** 01-05-SUMMARY.md, recovery_corpus.zig:107

---

### Snapshot tests as regression guards, not behavior specs
19 token + 19 AST snapshots + `-Dupdate-snapshots=true` workflow.

**Rationale:** Locked at human-reviewed initial capture; future runs are
byte-diff. **Limitation explicit in 01-VALIDATION.md "Manual-Only"
section:** initial snapshot correctness depends on reviewer eyes; no
automated check that the snapshot *is right*, only that it is *unchanged*.
**Source:** 01-02-SUMMARY.md, 01-VALIDATION.md (Manual-Only section)

---

## Lessons

### Snapshot byte-equality hides initial-correctness bugs
The 19 AST snapshots were locked from the first green run. If the day-1
parse was wrong, no test will tell us — only later behavior changes are
caught.

**Context:** 01-VALIDATION.md Manual-Only section openly admits the
reviewer is the oracle on initial capture. Phase 2 should add
property-style assertions (e.g., "every node's span lies within its
parent's span") that hold *regardless* of the locked snapshot.
**Source:** 01-VALIDATION.md, /gsd-validate-phase audit

---

### `diags.items.len >= 0` assertions are vacuous and slipped through review
`recovery_statement.zig` Case C and `recovery_definition.zig` Case B
both assert `>= 0` — always true for an unsigned length.

**Context:** Found during the test-quality assessment, not during
execution or code review. Atomic-commit-per-fix workflow and reviewer
attention focused on *new* functionality; weak assertions in *new tests*
were not flagged. Phase 2 reviewer checklist should include "scan new
tests for vacuous comparisons" (regex hit `>= 0` against an unsigned).
**Source:** /gsd-validate-phase audit, recovery_statement.zig:82,
recovery_definition.zig:67

---

### Coverage tests verify presence, not correctness
`parser_deal.coverage` walks the AST and counts NodeKind occurrences.
A parser that consistently produced the *wrong* node would still pass
as long as every kind appeared somewhere.

**Context:** This is by design — coverage is a *complement* to snapshot
+ unit tests, not a substitute. The gap surfaces because the unit-test
layer for `recovery.*` is also weak; together they leave correctness
under-asserted. Phase 2 should add structural-correctness assertions
(e.g., "the inner node of `a OR b AND c` is the AND node" — already
done in `expr.precedence`; extend the pattern to parser_deal).
**Source:** /gsd-validate-phase audit, parser_deal_coverage.zig

---

### Recovery tests claim more than they prove
Multiple `recovery_*` tests have comments like "the second attribute
should still parse cleanly" or "sync recovers at the next `part def`"
but assert only `definitions.len >= 1`.

**Context:** Comments describe *intent*; assertions describe *contract*.
The two diverged. In Phase 2, name the structural property in the
assertion: `expect(df.definitions[1].payload.part_def.name == "Good")`.
**Source:** /gsd-validate-phase audit, recovery_definition.zig

---

### Forward-progress guarantee was added reactively after sync infinite-loop bug
Each sync helper now has a single-advance fallback if sync lands on the
same byte. This came from a real bug during Plan 05 wiring, not from
the threat model.

**Context:** Threat T-05-05 "sync-loop resource mitigation" caught the
*severity* dimension (info-only diagnostic) but not the *liveness*
dimension (no infinite loop). Phase 2 threat-modeling should explicitly
ask "what guarantees forward progress in this loop?"
**Source:** 01-05-SUMMARY.md (forward-progress guarantee)

---

### Arena aliasing leak surfaced via test-allocator, not via review
`deal_parse_internal` captured `const allocator = arena.allocator()`
before copying arena into the handle struct. Subsequent ArrayList page
allocations went to the local arena variable; `handle.arena.deinit()`
missed them.

**Context:** Caught by `c_abi.no_leaks` reporting 5 leaks — the
`std.testing.allocator` was the oracle. Without the leak-detecting
allocator, the bug ships. Lesson: the *kind* of allocator a test uses
is part of the test's correctness — `std.heap.page_allocator` would
have silently passed.
**Source:** 01-06-SUMMARY.md (Auto-fix #1)

---

### "Failed command" stderr noise from `zig build --listen=-` is benign but discoverable as a real failure
`zig build test` exits 0 but emits `failed command: ./test --listen=-`
to stderr when test stdout has content. Easy to misread as a regression.

**Context:** Discovered during validation audit while assessing whether
the recovery.corpus 0-diagnostic files were causing a test failure.
The exit code is the only authoritative signal. Phase 2 CI should
explicitly check `$? == 0` rather than parse stderr for "failed" tokens.
**Source:** 01-06-SUMMARY.md (Known Issues), /gsd-validate-phase audit

---

### Zig 0.16.0 inferred error sets fail on mutual recursion
`json.zig writeNode/writePayload`, `expr.zig parseExpression/parsePrefix`,
and several parser_deal cycles required explicit
`std.mem.Allocator.Error!ReturnType` annotation. Build error message
("dependency loop with length N") is precise.

**Context:** Bit Plans 03 *and* 05. Lesson for Phase 2: any new
mutually-recursive parsing helper needs an explicit error set from
day 1.
**Source:** 01-03-SUMMARY.md (Auto-fix #4), 01-05-SUMMARY.md

---

### Truncated headers panicked the parser despite "robust recovery" intent
`parseHeaderField` had a `source[val_start..val_end]` slice without
bounds checking. The `truncate` mutation strategy in gen-malformed
produced headers where the colon scan ran off EOF.

**Context:** Both parser_deal and parser_dealx had the bug — caught by
recovery.corpus when it tried the first truncated `.deal` file.
Defensive bounds-clamping is now applied everywhere. Lesson: "the
parser must be panic-free on malformed input" is a *property*, not a
test case — generative testing surfaces it; hand-curated cases do not.
**Source:** 01-05-SUMMARY.md (Auto-fix #1)

---

### m08, m18, m19 hand-curated malformed files slip through with 0 diagnostics
Designed to test "unterminated block comment", "dangling dot operator",
"empty arg with comma" — currently all parse without producing any
diagnostic.

**Context:** Surfaced by the validation audit, not by the corpus test
(which permits up to 40% silent files). These are real parser
strictness gaps masquerading as test-tolerance. They belong on a
parser-strictness backlog with per-file expected-error-code pins.
**Source:** /gsd-validate-phase audit, recovery_corpus.zig output

---

## Patterns

### Multi-mode same-input lexer testing
Lex the SAME byte sequence in 2-3 different modes and assert the token
streams differ in exactly the expected way.

**When to use:** Any time the lexer or parser has state that determines
interpretation of identical input. Strongest BDD pattern in the phase.
**Source:** 01-02-SUMMARY.md, lexer_mode_flip.zig (6 cases including
negative-confirmation that `[</...>]` in `.deal_def` does NOT recognize
`tag_close_open`)

---

### Exact-code + exact-count + exact-span diagnostic assertions
`countCode(diags, "E0302") == 1` AND
`firstWithCode("E0302").secondary_spans.len == 1` AND
`secondary_spans[0].label contains "opened here"` AND
`primary.span.start == <known byte offset>`.

**When to use:** Every diagnostic-emitting code path. Stops the "≥1
diagnostic" anti-pattern dead. `parser_dealx.tag_balance` is the
exemplar; extend it to all `recovery_*` tests in Phase 2.
**Source:** 01-04-SUMMARY.md, parser_dealx_tag_balance.zig

---

### Std.testing.allocator as leak oracle
Use `std.testing.allocator` (not `std.heap.page_allocator`) as the
backing allocator. The test runner's automatic leak detection IS the
assertion — non-zero leaked bytes fails the test with
"allocator detected leaks".

**When to use:** Any test that exercises code which allocates and
expects to free. Critical for arena-based code. `c_abi.no_leaks`
catches a class of bugs (arena aliasing) that no other assertion would.
**Source:** 01-06-SUMMARY.md, c_abi_no_leaks.zig

---

### Comptime fmt as injection mitigation
`emitFmt(comptime fmt: []const u8, ...)` — Zig 0.16.0 rejects
non-comptime fmt at compile time, so untrusted source bytes can never
reach the format machinery.

**When to use:** Any diagnostic message that includes user-controlled
text. Document this as the standard Phase 2 pattern when adding
semantic-analysis diagnostics.
**Source:** 01-05-SUMMARY.md (T-05-03 mitigation)

---

### Pop-anyway tag balancing with dual-span diagnostics
On mismatched close tag, pop the open stack BEFORE comparing names,
then emit a single diagnostic carrying both spans (close = primary,
open = secondary with "opened here" label).

**When to use:** Any pair-bracket recovery (`{` / `}`, `[<…>]` /
`[</…>]`, function-arg parens). Single diagnostic vs cascading errors;
parser keeps making progress.
**Source:** 01-04-SUMMARY.md (D-07 pop-anyway)

---

### Forward-progress single-advance fallback in sync loops
After `sync*()` runs, check whether `lex.pos` advanced; if not,
unconditionally consume one token. Prevents infinite loops when the
current token is already in the sync set.

**When to use:** Every recovery loop. Trivially cheap; catches the
liveness bug T-05-05's severity-mitigation does not cover.
**Source:** 01-05-SUMMARY.md

---

### Build-time `-Dflag` over environment variables
`-Dupdate-snapshots=true` via `b.option(...)` + `addOptions/build_options`
beats `UPDATE_SNAPSHOTS=1` because Zig 0.16.0's `std.process` env-var
API requires a `std.Io.Threaded` instance.

**When to use:** Any per-build boolean or string flag in Zig 0.16.0
test infrastructure. Cleaner, doesn't require runtime IO setup.
**Source:** 01-02-SUMMARY.md

---

### Symbol-table assertion as security invariant
`nm -gU libdeal.a | grep _deal_ | wc -l` MUST equal exactly N exports;
`nm | grep _deal_internal | wc -l` MUST equal 0.

**When to use:** Any C ABI library with test-only helpers that must not
leak into the public surface. Compile-time pattern fails closed.
**Source:** 01-06-SUMMARY.md (T-06-01 / Warning #3)

---

### Verifying determinism by re-running snapshot tests twice in CI
The 19 AST snapshots claim byte-equality across re-runs but no
single-test asserts it. The snapshot test itself is the assertion: run
it, then run it again — pass twice means byte-stable.

**When to use:** Whenever a deterministic-output contract is claimed
(D-18 alphabetical-key invariant). Phase 2 should make this an explicit
assertion (parse twice → bytes equal) rather than rely on the
test-runs-twice convention.
**Source:** 01-03-SUMMARY.md (D-18), /gsd-validate-phase audit gap

---

## Surprises

### `peek2` was unnecessary at planning time but became required during snapshot generation
RESEARCH Open Question #1 asked "is LL(2) needed?" and the planning
answer was "probably not." Reality: `path . {` (import braces) vs
`path . ident` (qualified name) required peek2.

**Impact:** Bug introduced in Plan 03 Task 3.2 — peek2 double-advanced
through tokens — only surfaced when snapshot generation hit a real
import-path-with-dots case. Lesson: LL(k) decisions should be re-asked
at every snapshot-add boundary, not just at planning time.
**Source:** 01-03-SUMMARY.md (Open Question resolution + Auto-fix #1)

---

### Hand-rolled JSON writer required even after considering std.json.Stringify
RESEARCH §Pitfall 5 flagged the ordering risk; the team built a
hand-rolled deterministic writer with explicit alphabetical key order.
Surprise: this writer is now ~1200 lines (`json.zig`).

**Impact:** Larger surface than expected, with an exhaustive switch over
72 NodeKind variants — every new payload arm requires an alphabetical
insertion. Worth it: D-18 byte-stability would not hold with
`std.json.Stringify` (object-key order is HashMap-iteration order).
**Source:** 01-03-SUMMARY.md, 01-04-SUMMARY.md

---

### Zig inferred error sets fail more often than expected on mutual recursion
Both Plan 03 (4 cycles) and Plan 05 had to add explicit
`std.mem.Allocator.Error!T` annotations to break "dependency loop with
length N" build errors. Expected once; happened five times.

**Impact:** Compile failures are loud but the fix is mechanical. Phase 2
should pre-emptively annotate any new mutually-recursive parsing helper.
**Source:** 01-03-SUMMARY.md (Auto-fix #4), 01-05-SUMMARY.md

---

### `build.zig.zon` requires a fingerprint not documented in the plan
First `zig build` errored with `invalid fingerprint: ...; if this is a
new or forked package, use this value: 0xe3fec116726111a5`.

**Impact:** Wave 0 (01-01-foundation) had to take the toolchain-assigned
value verbatim. Plans should not assume `.zig.zon` field lists are
complete from documentation alone.
**Source:** 01-01-SUMMARY.md (Deviation #1)

---

### Arena aliasing bug only visible with the leak-detecting allocator
The arena-copy-then-allocate bug in `deal_parse_internal` produced
silent memory leaks. The fix changed `local_arena.allocator()` to
`handle.arena.allocator()` after the struct copy.

**Impact:** Without `c_abi.no_leaks` (which uses
`std.testing.allocator`), the bug would have shipped to Phase 2 and
manifested as slow leaks under real workloads. Test infrastructure
choice (allocator type) was the difference between catching it and
shipping it.
**Source:** 01-06-SUMMARY.md (Auto-fix #1)

---

### gen-malformed PRNG seed `0xDEAL2026` doesn't compile (L is not a hex digit)
Plan suggested `0xDEAL2026`; Zig 0.16.0 rejects. Fix: `0xDEA1_2026`
(L→1, intent preserved visually).

**Impact:** Trivial fix, but representative — copy-paste from planning
to code requires syntactic re-validation. Generic learning: any
"clever" identifier in a plan should be checked against language
lexer rules before commit.
**Source:** 01-05-SUMMARY.md (Auto-fix #2)

---

### tests/showcase symlink resolution broke in worktree context
Relative path `../../../../spec/examples/showcase` resolved incorrectly
because the worktree root is at a different depth than the main
checkout. Fixed with an absolute path symlink.

**Impact:** Worktree-based execution (gsd-executor pattern) needs to
treat relative-path filesystem assets as a portability risk. Phase 2
should prefer either absolute paths (with per-machine setup) or
in-tree copies for any test fixture.
**Source:** 01-06-SUMMARY.md (Auto-fix #4)

---

### Validation audit found that "all green" hid 4 distinct test-quality issues
`/gsd-validate-phase 1` initially saw `nyquist_compliant: true`, then on
actual read found:
1. Two vacuous `diags.items.len >= 0` assertions
2. `recovery.statement` Case A source has both semicolons (well-formed)
3. Three hand-curated malformed files (m08/m18/m19) produce zero diagnostics silently
4. No assertion that the determinism contract (D-18) actually holds across re-parses

**Impact:** The phase IS green by every existing automated gate, AND
the gates themselves under-specify behavior. This is the central
finding: Phase 1 succeeded by its own contracts; Phase 2 must
strengthen the contracts. Recommended remediation backlog:
- Replace `>= 0` with named-property assertions
- Pin per-file expected error codes on the 20 hand-curated malformed files
- Add `parse_twice_equals_self` determinism test
- Strengthen recovery tests to assert recovered AST structure, not just diagnostic count
- Either close m08/m18/m19 parser strictness gaps, or document them as accepted
**Source:** /gsd-validate-phase audit (this session)

---

## Phase 1.5 Resolution

Phase 1.5 closed every Surprise #8 backlog sub-item per REQ-phase-1.5-gate.
For each item below: Resolved entries cite the commit hash and the plan that
delivered the fix; Deferred entries cite the ADR documenting acceptance.

The Surprise #8 backlog comprises the four validation-audit findings
(items 1-4 below — direct observations) and the five remediation-backlog
bullets (items 5-9 below — recommended fixes), spanning the
audit-numbered findings and the audit-recommended remediation list at
`01-LEARNINGS.md` lines 508-525.

| # | Surprise #8 sub-item | Status | Reference |
|---|----------------------|--------|-----------|
| 1 | Audit finding: two vacuous `diags.items.len >= 0` assertions in recovery_statement / recovery_definition | Resolved | `ab8f807` (Plan 01.5-01 Task 1 — strengthened 9 recovery cases to two-axis BDD; grep gate `grep -nE 'len >= 0' tests/unit/*.zig` returns zero matches) |
| 2 | Audit finding: `recovery.statement` Case A source has both semicolons (well-formed) — claimed missing-semi recovery does not actually exercise recovery | Resolved | `ab8f807` (Plan 01.5-01 Task 1 — Case A locked as "well-formed two-attribute body" baseline asserting 0 diagnostics + 2 named members; empirical probe confirmed the alleged missing-semi variant also parses cleanly — the parser is whitespace-tolerant at attribute boundaries, so option (i) is the only honest contract) |
| 3 | Audit finding: m08 (unterminated block comment), m18 (dangling dot), m19 (empty arg with comma) hand-curated malformed files silently produce zero diagnostics | Resolved | `7cbf263` (E0121/E0122 declarations) + `b95e616` (Plan 01.5-02 Task 2 — wires EXISTING E0003 e_unterminated_comment for m08 via lexer-drain; emits NEW E0121 e_dangling_dot for m18 in expr.parseExpression dot branch; emits NEW E0122 e_empty_arg_comma for m19 in expr.parseArgList leading/double/trailing comma gates) + `c3f36da` (Plan 01.5-02 Task 3 — pin the three contracts in strictness_m08_m18_m19.zig). recovery.corpus sync rate 74% → 80%. |
| 4 | Audit finding: no assertion that the D-18 determinism contract actually holds across re-parses | Resolved | `5effb61` (Plan 01.5-04 Task 1 — `determinism.parse_twice` parses each of the 19 showcase files twice via the PUBLIC C ABI (`deal_parse` / `deal_ast_json` / `deal_free`) and asserts byte-identical AST JSON + diagnostics JSON across both parses; gpa.dupes parse-A's JSON before deal_free invalidates the arena-owned pointer) |
| 5 | Audit remediation: replace `>= 0` with named-property assertions | Resolved | `ab8f807` (Plan 01.5-01 Task 1 — supersedes #1; named-property assertions via the `countCode`/`firstWithCode` helpers reference `Codes.e_*` constants, not inline E-code literals) |
| 6 | Audit remediation: pin per-file expected error codes on the 20 hand-curated malformed files | Resolved | `b3c70d8` (Plan 01.5-03 Task 2 — `Pin` struct + 20-entry `m_pins` table in tests/unit/recovery_corpus.zig; 19 impl pins + 1 accepted-no-diag pin for m01; enforcement loop reads each m0X file freshly so the pin contract cannot be silently weakened by a future walk refactor) |
| 7 | Audit remediation: add `parse_twice_equals_self` determinism test | Resolved | `5effb61` (Plan 01.5-04 Task 1 — same commit as #4; named `determinism.parse_twice` per CONTEXT.md REQ-phase-1.5-3 lock; uses PUBLIC C ABI per CONTEXT.md §"Determinism test" — internal-helper byte-equality could pass while the public ABI's JSON cache produces different bytes, which would be a determinism bug the test must catch, not miss) |
| 8 | Audit remediation: strengthen recovery tests to assert recovered AST structure, not just diagnostic count | Resolved | `ab8f807` (Plan 01.5-01 Task 1 — axis-B of every strengthened case asserts a named structural property on the recovered AST: definition name, member kind/name, root_tag count + name) + `2ed9f07` (Plan 01.5-01 Task 2 — new property.span_containment test walks every parent-child edge of 19 showcase + 8 synthetic AST corpora, locking the child-span-inside-parent-span invariant as a structural-correctness property) |
| 9 | Audit remediation: either close m08/m18/m19 strictness gaps OR document them as accepted | Resolved | Same as #3 — `7cbf263` + `b95e616` + `c3f36da` (Plan 01.5-02 — all three got IMPLEMENTATIONS, not ADRs, because the grammar productions unambiguously reject the constructs: lexical.ebnf §BLOCK_COMMENT requires `*/`; deal.ebnf §PostfixExpression L1622 requires IDENT after `.`; deal.ebnf §_ArgumentList L1680 forbids leading/double/trailing commas) |

Out-of-band item documented under ADR (anticipated by the plan's scaffold but
discovered only during empirical pin discovery in Plan 01.5-03):

| # | Item | Status | Reference |
|---|------|--------|-----------|
| 10 | M01 SD-10 optional-semicolon (zero-diagnostic but grammatical — fixture name `m01_missing_semicolon.deal` is a historical artefact predating SD-10 finalisation) | Deferred | `.planning/decisions/ADR-phase-1.5-m01-sd10-optional-semicolon.md` (Plan 01.5-03 — pinned in recovery_corpus.zig with `expected_code = null` + `adr_basename` pointing at the ADR; future W05xx stylistic-warning lane would re-flip this pin) |

### Exit gate (REQ-phase-1.5-gate)

The Phase 1.5 exit gate command (per ROADMAP success criterion #6):

```sh
zig build phase-1-gate && zig build test -Dtest-filter=determinism && zig build test -Dtest-filter=property
```

All three sub-commands exit 0 as of Phase 1.5 completion. The single-command
convenience equivalent `zig build phase-1.5-gate` (introduced in
Plan 01.5-04 Task 2) also exits 0.

Additional cross-file invariants enforced at phase exit:
  - `grep -nE 'len >= 0' tests/unit/*.zig` returns zero matches.
  - `git diff --stat tests/snapshots/` is empty (no snapshot drift).
  - `nm -gU zig-out/lib/libdeal.a | grep -c '_deal_'` returns exactly 6 (C ABI symbol-count invariant from Phase 1).

### Notes for future maintainers

- The Surprise #8 backlog is now closed. Future regressions in any of the 10
  rows above should re-open this section as a new Surprise # in the
  current-phase LEARNINGS file, not silently fall back to "tolerated by the
  soft gate."
- M01's deferral via ADR-phase-1.5-m01-sd10-optional-semicolon is a
  grammatical truth (SD-10), not a strictness gap. If a future phase adds
  a `W0501 missing optional semicolon` warning lane, m01's pin in
  `recovery_corpus.zig` should be updated to expect that warning instead
  of remaining on the accepted-no-diag branch.
- The phase-1.5-gate build step is a SINGLE-COMMAND CONVENIENCE; the
  three-step locked form remains the CI-authoritative gate because it
  makes determinism/property gating surface-grep-able in CI logs.

---
