---
status: resolved
trigger: "zig build phase-1.5-gate exits non-zero; was green at 007311c yesterday, now broken at 87daddd"
created: 2026-05-21T00:00:00Z
updated: 2026-05-21T15:30:00Z
resolved: 2026-05-21T15:30:00Z
resolution_plan: .planning/phases/01.5-parser-strictness-test-gap-resolution-inserted/01.5-05-tests-showcase-submodule-and-verify-harden-PLAN.md
resolution_summary: .planning/phases/01.5-parser-strictness-test-gap-resolution-inserted/01.5-05-SUMMARY.md
resolution_commits: [c0377ff, 7f3e65f, 8b2115d]
resolution: "Closed by Plan 01.5-05: spec/ committed as submodule (pinned fe98f89), tests/showcase committed as relative symlink (../spec/examples/showcase) inside the submodule mount, phase-1.5-gate-fresh ephemeral-worktree gate added, ADR-phase-1.5-fresh-worktree-verification.md locks the invariant for Phases 2..6. Final verification: zig build phase-1.5-gate-fresh EXIT 0, Build Summary 8/8 steps succeeded; 33/33 tests passed."
---

## Current Focus

hypothesis: Missing `tests/showcase/` directory in the worktree — symlink `tests/showcase -> ../../spec/examples/showcase` exists locally in the main repo but is NOT committed to the `deal/` git repo, so it does not exist in a fresh worktree checkout. 9 of 33 tests read files under `tests/showcase/` and fail with `error.FileNotFound`.
test: Reproduce `zig build phase-1.5-gate`, inspect stderr for assertion stacks, confirm with `git ls-tree HEAD tests/` and `git status` in the main repo.
expecting: All 9 failures point at `cwd.readFileAlloc(io, "tests/showcase/...")`; the symlink is shown as "Untracked" in main-repo git status.
next_action: Return ROOT CAUSE FOUND with specialist_hint=general (this is a build/repo-hygiene issue, not a language-specific bug).

## Symptoms

expected: |
  `zig build phase-1.5-gate` completes with EXIT=0; bundles phase-1-gate + the
  full Zig test suite covering every Phase 1.5 deliverable.
actual: |
  Stdout shows informational prints from recovery.corpus (53 files, 173 diagnostics, 10 zero-diag, 0 panics),
  property.span_containment synthetic fixtures producing 3 & 6 diagnostics, c_abi.no_leaks 72 files OK.
  Then: `failed command: ./.zig-cache/o/505f6d591540f497b5bcb42512d8494d/test --cache-dir=./.zig-cache --seed=0x38902efe --listen=-`
  Per-test assertion failure messages NOT in the user's report.
errors: Unknown — need to capture via -Dtest-filter targeted runs
reproduction: cd repo root && zig build phase-1.5-gate
started: Some time between 007311c (green yesterday 5:48 PM) and 87daddd (now)

## Eliminated
<!-- APPEND only -->

- hypothesis: m01 accepted-no-diag pin's `cwd.statFile(".planning/decisions/ADR-...md")` fails because the test binary's cwd differs from the repo root, flipping m01 from accepted-no-diag to a hard failure.
  evidence: recovery.corpus is NOT in the failing-test list at all. The 9 failures are all in OTHER tests, all with `error.FileNotFound` against `tests/showcase/...` paths. `.planning/decisions/` exists in the worktree and is found correctly.
  timestamp: 2026-05-21T00:18:00Z

- hypothesis: Plan 02's E0121 e_dangling_dot / E0122 e_empty_arg_comma strictness gates now fire on the synthetic `verification_block` and `precondition_postcondition` fixtures, and the recovered AST has overlapping or out-of-order spans that fail the property.span_containment child-in-parent assertion.
  evidence: property.span_containment's failure stack points at L409 `cwd.readFileAlloc(io, "tests/showcase/packages/vehicle/battery.deal", ...)` — which is in the .deal showcase loop at L406-433, BEFORE the synthetic-fixture loop at L462. The test never reaches the synthetic fixtures in the worktree environment. The "synthetic fixture X produced N diagnostics" lines in the user's stdout came from the main-repo run (which uses the local symlink) and are informational `std.debug.print` calls that do NOT cause test failure.
  timestamp: 2026-05-21T00:18:00Z

- hypothesis: recovery.corpus's soft-60% gate computation has a bug that flips m01's accepted-no-diag pin to a hard failure (counting it among the 10 zero-diag files), pushing the test below the 60% threshold.
  evidence: 53 files total, 10 zero-diag → 43 with diagnostics. floor(53 * 60 / 100) = 31. 43 >= 31, so the soft-60% gate PASSES. Furthermore, the m01 pin is enforced by a SEPARATE second loop at L268-331 (the soft gate at L248-256 is purely numeric on the directory walk). recovery.corpus is NOT in the failing-test list.
  timestamp: 2026-05-21T00:18:00Z

- hypothesis: Phase 1.5 commits between 007311c and 87daddd regressed something.
  evidence: `git log 007311c..87daddd` shows only one commit — 87daddd "test(01.5): UAT partial" which only adds the UAT recording markdown file. No source touched. The gate was never actually green on a fresh worktree.
  timestamp: 2026-05-21T00:25:00Z

## Evidence
<!-- APPEND only -->

- timestamp: 2026-05-21T00:05:00Z
  checked: tests/unit/recovery_corpus.zig — soft-60% gate + per-file pin enforcement loop
  found: |
    Two-tier gate. Tier 1: soft-60% — counts files producing >=1 diagnostic. `min_files_with_diag = (paths.len * 60) / 100`. With 53 files: floor(53*60/100) = 31. Observed: 53 - 10 = 43 files-with-diag. 43 >= 31, so soft gate PASSES.
    Tier 2: per-file pin enforcement at L268-331. For each pin in m_pins, reads file, parses, asserts either (a) first diag code matches expected_code, OR (b) expected_code == null + zero diags + statFile of ADR succeeds.
    The m01 pin (L68): expected_code = null, adr_basename = "ADR-phase-1.5-m01-sd10-optional-semicolon.md". Pin loop at L321: `.planning/decisions/{s}` joined to adr_basename and passed to `cwd.statFile`. `cwd = std.Io.Dir.cwd()`.
  implication: |
    recovery.corpus soft-60% gate is NOT the failure trigger. The pin loop OR property.span_containment OR another test fails.

- timestamp: 2026-05-21T00:10:00Z
  checked: tests/unit/property_span_containment.zig — synthetic fixture handling
  found: |
    Test handles diagnostics emitted by synthetic fixtures GRACEFULLY at L477-482 (just logs `synthetic fixture %s produced %d diagnostics`) and STILL CALLS walkRoot. The test only fails if (a) parseFile returns null, OR (b) walkRoot returns error.TestSpanContainmentViolation, OR (c) walkRoot returns error.OutOfMemory.
    The stdout messages "synthetic fixture verification_block produced 3 diagnostics" and "synthetic fixture precondition_postcondition produced 6 diagnostics" are INFORMATIONAL prints (L478-481), NOT assertion failures.
    However — if the parser's RECOVERED AST after emitting those diags has children with spans OUTSIDE parent's span, walkRoot would emit "property.span_containment: violation at NodeKind .X" and fail with TestSpanContainmentViolation. The user-quoted stdout does NOT show such a violation message, but it does NOT show the per-test runner output either.
  implication: |
    The two synthetic fixtures producing diagnostics post-Plan-02 (E0121/E0122 strictness now fires on `@verify`, `@pre`, `@post` annotation bodies) is a SUSPICIOUS signal that something materially changed. Need to actually run the test to see if walkRoot reports a span violation.

- timestamp: 2026-05-21T00:12:00Z
  checked: build.zig phase-1.5-gate composition
  found: |
    `gate_1_5_step.dependOn(gate_step)` + `gate_1_5_step.dependOn(test_step)`. `gate_step` itself dependsOn `test_step`. So phase-1.5-gate just runs `test_step` (unfiltered). The failing command in stdout is `./.zig-cache/o/505f6d591540f497b5bcb42512d8494d/test --cache-dir=./.zig-cache --seed=0x38902efe --listen=-` — a SINGLE test binary running the unfiltered suite.
  implication: |
    The test binary contains all 22 imports from _all.zig. Any one of them with a failing `test "..."` block returns non-zero. Need to identify which specific test fires the failure. Will run with -Dtest-filter targeted to each suspect.

- timestamp: 2026-05-21T00:18:00Z
  checked: actual `zig build phase-1.5-gate` run with full stderr captured
  found: |
    Exactly 9 tests fail; all 9 fail with `error.FileNotFound` from `std.Io.Dir.openFile` while trying to read paths under `tests/showcase/`. The 9 failing tests are:
      - c_abi_no_leaks.test.c_abi.no_leaks (reads tests/showcase/packages/vehicle/battery.deal …)
      - determinism_parse_twice.test.determinism.parse_twice (reads tests/showcase/packages/…)
      - lexer_snapshot.test.lexer.snapshot
      - lexer_snapshot.test.lexer.snapshot spans monotonic
      - parser_deal_coverage.test.parser_deal.coverage
      - parser_deal_snapshot.test.parser_deal.snapshot
      - parser_dealx_coverage.test.parser_dealx.coverage
      - parser_dealx_snapshot.test.parser_dealx.snapshot
      - property_span_containment.test.property.span_containment

    First failing assertion stack (representative — same shape for all 9):
      .../zig/std/Io/Threaded.zig:4866 — .NOENT => return error.FileNotFound
      .../zig/std/Io/Dir.zig:578 — openFile
      .../zig/std/Io/Dir.zig:1360 — readFileAllocOptions
      .../zig/std/Io/Dir.zig:1338 — readFileAlloc
      tests/unit/property_span_containment.zig:409 — `const source = try cwd.readFileAlloc(io, path, gpa, .unlimited);`
        (path = "tests/showcase/packages/vehicle/battery.deal")
      tests/unit/determinism_parse_twice.zig:66 — re-raises FileNotFound after std.debug.print
      tests/unit/c_abi_no_leaks.zig:66 — same shape

    Build summary line: "Build Summary: 4/8 steps succeeded (1 failed); 24/33 tests passed (9 failed)"
  implication: |
    SUSPECT 2 (synthetic-fixture span violation) is FALSIFIED — property_span_containment fails at L409 reading the FIRST showcase file `tests/showcase/packages/vehicle/battery.deal`, before the synthetic-fixture loop is ever reached. The two "synthetic fixture X produced N diagnostics" stderr lines in the user's report come from the MAIN-REPO run where showcase files exist; they are red-herring informational output that survives across runs because the test stops at L409 in the worktree.
    SUSPECT 1 (m01 ADR statFile cwd mismatch) is FALSIFIED — recovery.corpus is NOT in the failing-test list; the m01 pin's `statFile(".planning/decisions/ADR-phase-1.5-m01-sd10-optional-semicolon.md")` resolves correctly because `.planning/decisions/` exists in the worktree just like it does in main.
    SUSPECT 3 (recovery.corpus soft-60 hard-flip) is FALSIFIED — recovery.corpus is NOT failing. The 10 "0 diagnostics" prints are informational `std.debug.print` calls that DO NOT cause the test to fail; the soft-60% gate is 31/53 and the run has 43/53 with diagnostics — passes comfortably.
    THE ACTUAL ROOT CAUSE is missing `tests/showcase/` directory in the worktree.

- timestamp: 2026-05-21T00:22:00Z
  checked: |
    `git ls-tree HEAD tests/` in BOTH the worktree AND the main repo at `/Users/dunnock/projects/deal-lang/deal`.
    `git status` and `ls -la tests/` in the main repo to inspect the missing path.
  found: |
    `git ls-tree HEAD tests/` returns only: ffi, malformed, snapshots, unit. **No `showcase`.**
    In the main repo, `ls -la tests/` shows: `showcase -> ../../spec/examples/showcase` (a symlink).
    In the main repo, `git status` shows: `tests/showcase` listed under "Untracked files" — i.e. **the symlink has never been committed to the `deal/` git repo.** No commit in `git log --all --full-history -- tests/showcase` references it. It is an entirely local working-tree artifact.
    Resolution: the symlink target `../../spec/examples/showcase` (relative to `deal/tests/`) resolves to `/Users/dunnock/projects/deal-lang/spec/examples/showcase`, which exists as a SEPARATE git repo (`/Users/dunnock/projects/deal-lang/spec/.git`) sibling to `deal/`. So the showcase corpus lives in a different repo that is not a submodule/subtree of `deal/`.
    From the worktree at `deal/.claude/worktrees/agent-a00e64992513aad6e/`:
      - The symlink itself doesn't exist (not in git tree → not checked out)
      - Even if recreated with the same `../../spec/examples/showcase` target, it would resolve to `deal/.claude/worktrees/spec/examples/showcase` which doesn't exist.
  implication: |
    Phase 1.5's exit-gate verification (01.5-VERIFICATION.md L73 "Phase 1.5 umbrella gate exits 0 — PASS") was performed in the main repo where the local-only symlink existed. The gate is NOT actually green on a fresh / worktree checkout because `tests/showcase` is required by 9 of the 33 tests but is not part of the committed source.

- timestamp: 2026-05-21T00:25:00Z
  checked: |
    `git log` between 007311c (yesterday-green) and 87daddd (current HEAD).
  found: |
    87daddd test(01.5): UAT partial - 1 issue (gate failure), 9 blocked, awaiting diagnosis
    007311c docs(01.5): update code review fix report (iter 2, Info findings)
    Only docs/UAT-recording changes between the two — no source touched.
  implication: |
    The hypothesis "something regressed between 007311c and 87daddd" is FALSE. The gate did not regress — it was never actually green in a clean / worktree environment. It only passed in the main repo because of the local-only symlink.

- timestamp: 2026-05-21T00:27:00Z
  checked: |
    `git log --diff-filter=A` for each failing test file to find when each was introduced.
  found: |
    parser_*_snapshot.zig and parser_*_coverage.zig predate Phase 1; the `tests/showcase/` path dependency exists in tests added across phases 1-01 through 1-06.
    `c_abi_no_leaks.zig` introduced in commit 0f82b1e (2026-05-20, phase 01-06).
    `property_span_containment.zig` introduced in commit 2ed9f07 (2026-05-20, phase 01.5-01).
    `determinism_parse_twice.zig` introduced in commit 5effb61 (2026-05-20, phase 01.5-04).
  implication: |
    The dependency on a local-only symlink has existed since Phase 1's earliest snapshot tests. Phase 1.5 inherited the brittleness, did not introduce it. The actual fragility cited as INFO-04 / WR-02 in 01.5-REVIEW.md (substring filter silent-skip) is unrelated.



## Resolution

root_cause: |
  `tests/showcase/` is missing from the `deal/` worktree. In the developer's main checkout, `deal/tests/showcase` is a symbolic link `../../spec/examples/showcase` that points one directory level above the `deal/` repo into a sibling `spec/` git repo (`/Users/dunnock/projects/deal-lang/spec/`). That symlink is **untracked** in the `deal/` repo — `git ls-tree HEAD tests/` returns only `ffi`, `malformed`, `snapshots`, `unit`, and `git log --all --full-history -- tests/showcase` returns empty. So a fresh worktree checkout has no `tests/showcase` directory at all.

  Nine of the test files in `tests/unit/` read files via `cwd.readFileAlloc(io, "tests/showcase/...", ...)`. Without the symlink (or any other `tests/showcase` directory), each of those nine tests aborts with `error.FileNotFound` from `std.Io.Dir.openFile` → `dirOpenFilePosix` `.NOENT => return error.FileNotFound`.

  The 9 failing tests (each named explicitly):
    1. `lexer_snapshot.test.lexer.snapshot`
    2. `lexer_snapshot.test.lexer.snapshot spans monotonic`
    3. `parser_deal_snapshot.test.parser_deal.snapshot` (`parser_deal_snapshot.zig:61`)
    4. `parser_deal_coverage.test.parser_deal.coverage`
    5. `parser_dealx_snapshot.test.parser_dealx.snapshot` (`parser_dealx_snapshot.zig:49`)
    6. `parser_dealx_coverage.test.parser_dealx.coverage` (`parser_dealx_coverage.zig:237`)
    7. `property_span_containment.test.property.span_containment` (`property_span_containment.zig:409`)
    8. `determinism_parse_twice.test.determinism.parse_twice` (`determinism_parse_twice.zig:66`, error printed at `:55-58`)
    9. `c_abi_no_leaks.test.c_abi.no_leaks` (`c_abi_no_leaks.zig:66`, error printed at `:55-58`)

  The user's stdout report showed `recovery.corpus` and `property.span_containment` and `c_abi.no_leaks` **informational** prints because those were leftovers from the developer's main-repo run (where the symlink works). In the actual fresh-worktree failure, `property.span_containment` aborts at its FIRST `cwd.readFileAlloc` call on `tests/showcase/packages/vehicle/battery.deal` (line 409), never reaching the synthetic-fixture loop. The `verification_block` / `precondition_postcondition` "produced N diagnostics" lines and the `c_abi.no_leaks: 72 files parsed and freed` line in the user's report came from a DIFFERENT (main-repo) run and are red herrings.

  Phase 1.5 verification (`01.5-VERIFICATION.md:73` "Phase 1.5 umbrella gate exits 0 — PASS") was performed in the main repo where the local symlink existed. The gate was never actually verified to be green on a fresh checkout / worktree / fresh CI runner.

fix: |
  N/A — goal is find_root_cause_only. Suggested fix direction left for plan-phase --gaps.
verification: N/A
files_changed: []
