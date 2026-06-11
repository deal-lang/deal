---
phase: 04-ecosystem
plan: "02"
subsystem: cli
tags: [rust, git2, toml, resolver, deal-install, deal-init, lockfile, dependency-management]

requires:
  - phase: 04-01
    provides: "E2402 error code reserved in diagnostics.zig; Wave 0 scaffolds in place"

provides:
  - "cli/src/resolver.rs — deal.toml parse, git2 clone/checkout at exact ref, deterministic deal.lock (D-66/D-68)"
  - "cli/src/lib.rs — library target exposing resolver module for integration tests"
  - "deal init subcommand — PS-8 directory scaffold + deal.toml with deal-std git dep + D-69 starter model"
  - "deal install subcommand — resolves all deps, vendors git deps to .deal/deps/, writes deal.lock"
  - "E2402 guard in deal check — emits error[E2402] when a declared git dep is uninstalled"

affects: [04-03, 04-04, 04-05]

tech-stack:
  added:
    - "git2 = { version = \"0.21\", features = [\"vendored-libgit2\"] } — libgit2 bindings with vendored libgit2 for CI stability"
    - "tempfile = \"3\" — dev-dependency for integration test temp dirs"
  patterns:
    - "BTreeMap for all serialized maps (D-18 alphabetical-key determinism, T-4-04 mitigation)"
    - "Two-gate path validation: lexical check (no absolute) + post-canonicalize system-path check (T-4-02)"
    - "URL scheme allowlist before any git2 clone call (T-4-03 mitigation)"
    - "canonicalize(project_dir) before any joins in resolve_all (CWE-22 taint source guard)"
    - "lib.rs library target + bin main.rs pattern: exposes resolver for integration tests while keeping binary entry point separate"
    - "Repo-reuse detection checks both .git/HEAD (normal clone) and HEAD (bare clone)"

key-files:
  created:
    - "cli/src/resolver.rs — DealToml/Dependency/LockFile/LockedPackage structs; resolve_git_dep(); resolve_all()"
    - "cli/src/lib.rs — [lib] target exposing pub mod resolver"
    - "cli/tests/resolver_test.rs — 5 integration tests: clone, SHA, determinism, path-dep, traversal rejection"
    - "cli/tests/fixtures/git-dep-bare/.gitkeep — placeholder for bare git repo fixture"
  modified:
    - "cli/Cargo.toml — added git2 (vendored), [lib] target, tempfile dev-dep"
    - "cli/src/main.rs — added resolver/Init/Install/DEFAULT_STDLIB_TAG; E2402 guard in run_check"

key-decisions:
  - "git2 vendored-libgit2 feature: avoids system-libgit2 CI fragility (RESEARCH A4 recommendation)"
  - "lib.rs [lib] target added alongside [[bin]]: required for integration tests to call resolver::resolve_all directly via deal::resolver"
  - "Starter model uses no deal.std imports so deal check passes immediately after deal init before deal install"
  - "DEFAULT_STDLIB_TAG = v0.1.0 as single source of truth const in main.rs (Plan 04 updates this to the real tag)"
  - "E2402 guard in run_check silently skips if deal.toml cannot be parsed — avoids breaking check for projects without manifests"
  - "Repo-reuse detection: check both .git/HEAD (normal clone) and HEAD (bare) — prior agent's bare-only check broke determinism test"
  - "satisfy block placed in model/starter.dealx inside [<traceability>] block (grammar requires satisfy inside traceability, not inside .deal files)"

requirements-completed:
  - REQ-phase-4-4-package-resolution

duration: 47min
completed: "2026-06-06"
---

# Phase 04 Plan 02: Package Resolution Core Summary

**git2-backed deal install with two-gate path security, deterministic deal.lock via BTreeMap, and deal init PS-8 scaffolding with a grammar-clean D-69 starter model**

## Performance

- **Duration:** ~47 min
- **Started:** 2026-06-06T14:30:00Z
- **Completed:** 2026-06-06T15:17:49Z
- **Tasks:** 2
- **Files modified:** 6 created + 2 modified = 8 total

## Accomplishments

- Implemented `cli/src/resolver.rs` with full git2-backed dependency resolution: clone/reuse at exact tag/rev/branch, two-gate T-4-02 path validation, T-4-03 URL scheme allowlist, and BTreeMap-enforced D-18 determinism
- Added `[lib]` target to `cli/Cargo.toml` with `cli/src/lib.rs` so integration tests can call `deal::resolver::resolve_all` directly without shelling out to the binary
- Wired `deal init` (PS-8 scaffold + deal-std git dep + D-69 starter model that parses cleanly pre-install) and `deal install` (downloads + resolves + writes deal.lock) into `main.rs`
- Added E2402 guard to `deal check`: emits `error[E2402]: dependency '{name}' not resolved — run 'deal install'` when a git dep is declared but `.deal/deps/<name>/` is absent

## Task Commits

1. **Task 1: resolver.rs — deal.toml parse, git2 resolution, deal.lock generation** - `528eccc` (feat)
2. **Task 2: Wire deal init + deal install subcommands into main.rs** - `166902f` (feat)

## Files Created/Modified

- `cli/src/resolver.rs` — DealToml/Dependency/LockFile/LockedPackage; resolve_git_dep(); resolve_all() with CWE-22 guard
- `cli/src/lib.rs` — library target, `pub mod resolver`
- `cli/tests/resolver_test.rs` — 5 integration tests covering clone, SHA assertion, determinism, path-dep, traversal rejection
- `cli/tests/fixtures/git-dep-bare/.gitkeep` — placeholder directory committed per plan
- `cli/Cargo.toml` — git2 vendored dep, [lib] target, tempfile dev-dep
- `cli/src/main.rs` — DEFAULT_STDLIB_TAG, Init/Install commands, run_init, run_install, E2402 guard in run_check, pub mod resolver

## Decisions Made

- **git2 vendored-libgit2:** Avoids system-libgit2 CI fragility per RESEARCH A4. Adds ~2MB to binary but eliminates CI dependency on system git2 version.
- **lib.rs target pattern:** Integration tests in `cli/tests/` reference `deal::resolver::resolve_all`. Adding `[lib]` alongside `[[bin]]` is the standard Rust pattern — no alternative avoids requiring a full binary spawn per test.
- **Starter model D-69 structure:** `satisfy` block lives in `model/starter.dealx` inside `[<traceability>]` because the grammar requires satisfy to appear inside a traceability block (not inside `.deal` definitions). This is correct per `dealx.ebnf §8`.
- **DEFAULT_STDLIB_TAG = "v0.1.0":** Placeholder; Plan 04-03 updates to the real tag from Plan 03.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed repo-reuse detection for normal (non-bare) clones**
- **Found during:** Task 1 (resolver_test determinism test failure)
- **Issue:** Prior agent's `dest.join("HEAD").exists()` only works for bare repos. Normal git clones store HEAD at `.git/HEAD`, not `HEAD`. The second `resolve_all` call tried to re-clone into a non-empty directory and failed with `Exists (-4)`.
- **Fix:** Changed check to `dest.join(".git").join("HEAD").exists() || dest.join("HEAD").exists()`
- **Files modified:** `cli/src/resolver.rs`
- **Verification:** `test_lockfile_determinism` passes (was failing before fix)
- **Committed in:** `528eccc`

**2. [Rule 1 - Bug] Fixed `validate_dep_path` function name mismatch**
- **Found during:** Task 1 (build failure — prior agent named function `validate_dep_path_lexical` but called it as `validate_dep_path`)
- **Fix:** Renamed all call sites and unit test references to `validate_dep_path_lexical`
- **Files modified:** `cli/src/resolver.rs`
- **Committed in:** `528eccc`

**3. [Rule 2 - Missing Critical] Wired unused `assert_not_system_path` into path dep resolution**
- **Found during:** Task 1 (dead_code warning; second T-4-02 gate was written but never called)
- **Fix:** Added `assert_not_system_path(&canonical)?` call in the `Dependency::Path` branch after `canonicalize()`
- **Files modified:** `cli/src/resolver.rs`
- **Committed in:** `528eccc`

**4. [Rule 2 - Missing Critical] Added [lib] target to cli/Cargo.toml and created lib.rs**
- **Found during:** Task 1 (resolver_test compilation error — `deal::resolver::resolve_all` not resolvable in a bin-only crate)
- **Issue:** The plan's integration test pattern requires `deal::resolver::resolve_all` which needs a lib target. The cli crate was bin-only.
- **Fix:** Added `[lib]` with `path = "src/lib.rs"` in `cli/Cargo.toml`; created `cli/src/lib.rs` with `pub mod resolver`
- **Files modified:** `cli/Cargo.toml`, created `cli/src/lib.rs`
- **Committed in:** `528eccc`

---

**Total deviations:** 4 auto-fixed (2 Rule 1 bugs, 2 Rule 2 missing critical)
**Impact on plan:** All auto-fixes necessary for correctness, security, and test compilability. No scope creep.

## Issues Encountered

- Shell heredoc (`cat <<EOF`) from macOS was processed by a bat-aliased `cat` that injects ANSI escape codes into the file contents. Created deal.toml files for E2402 testing had ANSI sequences embedded, causing silent toml parse failures during debugging. Fixed by using `printf` to write test files.

## Known Stubs

- `DEFAULT_STDLIB_TAG = "v0.1.0"` — placeholder tag; the real deal-stdlib has not been tagged yet. Plan 04-03 or the stdlib plan will update this constant to the actual release tag.
- `deal install` prints Downloading messages but does not implement progress indication or authentication for private repos.

## Threat Flags

None — all T-4-02/T-4-03/T-4-04 mitigations from the plan's threat model are implemented and tested.

## Next Phase Readiness

- `resolver::resolve_all` is ready for Plan 04-04 (sema) to consume vendored stdlib sources
- `deal init` scaffolds the new-user `deal init → deal install → deal check` flow documented in D-67/D-69
- E2402 guard ensures `deal check` fails fast when deps are not installed

---
*Phase: 04-ecosystem*
*Completed: 2026-06-06*
