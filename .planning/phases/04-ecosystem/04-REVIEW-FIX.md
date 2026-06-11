---
phase: 04-ecosystem
fixed_at: 2026-06-06T20:55:00Z
review_path: .planning/phases/04-ecosystem/04-REVIEW.md
iteration: 1
findings_in_scope: 12
fixed: 11
skipped: 1
status: partial
---

# Phase 4: Code Review Fix Report

**Fixed at:** 2026-06-06T20:55:00Z
**Source review:** .planning/phases/04-ecosystem/04-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope (critical + warning): 12
- Fixed: 11
- Skipped: 1

Scope was `critical_warning`: CR-01, CR-02, CR-03 and WR-01 through WR-09. The
6 Info findings (IN-01..06) were out of scope and not attempted. IN-04 also
targets a sibling repo (`deal-lang.org`) not reachable from this worktree.

All fixes were applied in an isolated git worktree and committed atomically.
Rust changes verified with `cargo test` (lib + resolver + golden, with the
ReqIF XSD bundle made available via `DEAL_REQIF_SCHEMAS_DIR`); Zig changes
verified with `zig build` (clean) and the `sema.corpus.regression_pins` filter.

> Environmental note: the worktree omits several untracked-but-present files
> from the main repo (`spec/references/omg-reqif/` XSD bundle, `tests/showcase/`
> corpus, and the `../deal-stdlib` sibling). Tests that depend on those files
> (`reqif_schema` SHA tests, `fmt_roundtrip`, `sema.corpus.showcase_clean`,
> `sema.dimensional.regression_pins`) fail only because the inputs are absent —
> not because of any change in this report. Each such suite was re-run with the
> inputs made available (symlink / env var) and passed.

## Fixed Issues

### CR-01: Path-traversal guard defeated by canonicalize() fallback (+ WR-09)

**Files modified:** `cli/src/resolver.rs`
**Commit:** a2f191f
**Applied fix:** Removed the `unwrap_or_else(|_| resolved.clone())` fallback in
`resolve_all` so a path dependency now MUST canonicalize to a real on-disk path
(fail closed). This collapses `..` segments and resolves symlinks before any
security assertion runs, closing the `../../../../etc/shadow` bypass. WR-09 was
fixed in the same commit: `assert_not_system_path` now boundary-anchors each
prefix (exact match or `prefix + "/"`), so legitimate paths like
`/etcetera-project` are no longer false-positively rejected. The existing
`test_path_dep_no_clone` (sibling exists) and `test_path_traversal_rejection`
(`/etc/passwd`) still pass.
**Note:** Logic/security change — flagged for human verification.

### CR-02: Dangling SPEC-OBJECT-REF + no referential-integrity gate

**Files modified:** `cli/src/reqif.rs`, `cli/src/reqif_schema.rs`
**Commit:** 4022877
**Applied fix:** (a) The relation filter in `emit()` now requires BOTH endpoints
to be requirement/need nodes (`matches_req(e.src) && matches_req(e.dst)`), so a
`part_def -> requirement` edge no longer emits a relation that references a
SpecObject filtered out by D-59. (b) Added a referential-integrity pass to
`validate_reqif_xml`: it collects every declared `SPEC-OBJECT` IDENTIFIER and
every `SPEC-OBJECT-REF` target text, then reports a violation for any ref that
does not resolve. Added `extract_identifier` helper. Updated the in-file unit
tests: the old `emit_trace_edge_produces_spec_relation` (which asserted the
dangling ref) was replaced by `emit_trace_edge_between_two_requirements_produces_spec_relation`
(req->req, both refs resolve) and `emit_skips_relation_with_non_requirement_endpoint`
(part->req emits no relation). The golden fixtures already had empty
`<SPEC-RELATIONS>`, so they were unaffected by part (a).
**Note:** Logic change — flagged for human verification.

### CR-03: .reqifz archive not byte-reproducible (default zip timestamp)

**Files modified:** `cli/src/reqif.rs`
**Commit:** 95a63d6
**Applied fix:** `wrap_in_reqifz` now pins the entry's last-modified time via
`zip::DateTime::from_date_and_time(2026, 1, 1, 0, 0, 0)`, matching the
`FIXED_CREATION_TIME` epoch used for the XML header. Added
`reqifz_archive_is_byte_reproducible` asserting two `wrap_in_reqifz` calls
produce byte-identical archives.

### WR-01: All SpecRelations typed as "Satisfies" regardless of edge kind

**Files modified:** `cli/src/reqif.rs`, `tests/golden/reqif/01-requirement-def.expected.reqif`, `tests/golden/reqif/02-trace-relation.expected.reqif`
**Commit:** 2ff1d7c
**Applied fix:** Added a `kind` field to `SpecRelation` and a `relation_type_info`
mapping (satisfies/traces/derives_from/allocated_to -> distinct
SPEC-RELATION-TYPE label + LONG-NAME). `emit()` now emits one
`SPEC-RELATION-TYPE` per distinct kind actually present (sorted for D-18
determinism) and points each relation's `SPEC-RELATION-TYPE-REF` at the matching
type id. Side effect: a document with zero relations no longer declares an
unused SatisfiesRelation type, so both golden fixtures were regenerated from the
emitter output (the only diff is the removal of that now-unused line). Golden
byte-exact tests re-pass.
**Note:** Output-format change — flagged for human verification.

### WR-02: E2402 not-installed guard only fires when a directory arg is passed

**Files modified:** `cli/src/main.rs`
**Commit:** 1a8e97a
**Applied fix:** Added `find_deal_toml_root`, which walks up from each input path
(file or directory, plus cwd as a fallback origin) looking for `deal.toml`,
mirroring how `infer_output_path` walks up for `Cargo.toml`. `run_check` now uses
it instead of `paths.iter().find(|p| p.is_dir())`, so the E2402 gate fires for
`deal check packages/foo.deal` regardless of cwd or whether a directory arg was
passed.
**Note:** Logic change — flagged for human verification.

### WR-07: FS-3 identity gate silently passes when git absent or whitespace

**Files modified:** `cli/src/main.rs`
**Commit:** c6d5f4d
**Applied fix:** `check_fs3_identity` now trims stdout and rejects a
whitespace-only name, and fails closed (returns an Internal error) when the git
binary is missing or the invocation fails — a provenance gate must not pass when
the committer identity cannot be established.
**Note:** Behavior change — flagged for human verification.

### WR-08: ReqIF depth / inside_core_content tracking is order-fragile

**Files modified:** `cli/src/reqif_schema.rs`
**Commit:** 4579bc7
**Applied fix:** The `Event::End` handler now clears `inside_core_content` only
when the closing element name is `CORE-CONTENT` AND `depth == core_content_depth`
(the depth recorded when the scope opened), so a nested element also named
`CORE-CONTENT` cannot prematurely clear the flag. The `Event::Empty` handler now
resets `inside_core_content` immediately after processing a self-closing
`<CORE-CONTENT/>`, since such an element has no children and no matching End
event. Removed the stray `let _ = core_content_depth;` discard. All structural
validator tests re-pass.

### WR-04: Specialization cycle detection dead for indirect cycles

**Files modified:** `src/sema.zig`
**Commit:** 03a94aa
**Applied fix:** Added a `specializes_map` (bare name -> target name) to the
`Analyzer`, populated during Pass A. `checkSpecializationCycle` was rewritten to
follow the chain through this map with its own visited set, detecting indirect
cycles (A specializes B, B specializes A), not just the direct self-reference.
The walk is bounded by symbol-table size. The direct self-cycle regression
(`04-specializes.deal`, E2300) in `sema.corpus.regression_pins` still passes.
**Note:** Logic change — flagged for human verification (indirect-cycle path
verified by construction + compile; the live regression fixture covers only the
direct case).

### WR-05: Duplicate-declaration check keys on bare name; wrong location format

**Files modified:** `src/sema.zig`
**Commit:** 03a94aa
**Applied fix:** The E2002 duplicate check now keys on the fully-qualified `id`
(not the bare `elem.name`), so same-named definitions in different packages are
no longer false-positive duplicates. The diagnostic was relabelled from a
misleading `line:col`-style `"{d}:{d}"` (raw byte offsets) to
`"first declared at byte offset {d}"`. The bare-name alias is now only inserted
when no local definition already claims it (imported aliases are still replaced),
preventing a later same-named definition from silently clobbering the first's
alias.
**Note:** Logic change — flagged for human verification. (Committed together with
WR-04 and WR-06 because all three edits live in `src/sema.zig` and the tool
stages at whole-file granularity.)

### WR-06: isTraceAnnotation has a tautological / dead second clause

**Files modified:** `src/sema.zig`
**Commit:** 03a94aa
**Applied fix:** Reduced `name == "trace" or (name == "trace" and operator != null)`
to `name == "trace"`. The second clause was a strict subset of the first and
therefore dead. DEAL trace annotations (`@trace`, `@trace:<<satisfies>>`, ...)
all carry the name "trace" with the kind in `ann.operator`, so the simplified
predicate is behavior-preserving. (Committed together with WR-04/WR-05.)

## Skipped Issues

### WR-03: Unknown-unit (E2502) false positive for any non-unit function call

**File:** `src/sema.zig:1128-1143`
**Reason:** skipped — the suggested fix conflicts with a tracked, currently-green
regression contract. The review's fix is to emit E2502 only when the surrounding
context is dimensional (the declared type resolves to a dimension). But the
tracked fixture `tests/regressions/sema/07-unknown-unit.deal` declares
`attribute legacyRange : Real [1] = furlongs(400);` and the test
`tests/unit/sema_dimensional.zig` (`07-unknown-unit.deal -> e_unknown_unit`)
**expects E2502 in a `Real` (non-dimensional) context**. Gating E2502 on
dimensional context would suppress the diagnostic this fixture requires, breaking
a green test. The alternative (an explicit unit-constructor-name allowlist) would
not flag the unknown `furlongs` either, and any args-count / naming heuristic to
distinguish `furlongs(x)` from `max(a, b)` is speculative and risks new
regressions. Resolving this properly requires redesigning both the fixture's
intended contract and the unit-detection heuristic together — out of scope for a
clean, non-regressing targeted fix. Left for human decision.
**Original issue:** `checkCallDimension` emits E2502 for any callee not in the
symbol table whenever a `call` appears as an attribute default, so a legitimate
non-unit call (e.g. `max(a, b)`) in a unit context would be flagged as an
"unknown unit".

---

_Fixed: 2026-06-06_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
