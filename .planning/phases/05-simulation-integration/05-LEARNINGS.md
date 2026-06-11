---
phase: 5
phase_name: "simulation-integration"
project: "DEAL — Digital Engineering Authoring Language"
generated: "2026-06-09"
counts:
  decisions: 12
  lessons: 9
  patterns: 13
  surprises: 8
missing_artifacts:
  - "VERIFICATION.md"
  - "UAT.md"
---

# Phase 5 Learnings: simulation-integration

## Decisions

### `deal_check_with_stdlib` returns a handle pointer, not a bool
The new C ABI export wraps `analyzeWithExternalTable` and returns `?*anyopaque` (DealHandle\*) rather than the originally-stubbed `bool`. The caller is responsible for `deal_free(handle)`.

**Rationale:** The implementation creates an arena-backed `DealHandle`. Returning a bool would leak the arena — the handle could never be exposed to the caller or freed. NULL + non-zero length guards (ASVS V5, T-05-01/T-05-02) and UTF-8 / >4 GiB source guards were added to match existing C ABI exports.
**Source:** 05-01-SUMMARY.md

### Dual-target `CliError` defined in both `lib.rs` and `main.rs`
`CliError` is defined identically in the library root (`cli/src/lib.rs`) and the binary root (`cli/src/main.rs`).

**Rationale:** Rust gives the library target and the binary target separate `crate::` namespaces. Integration tests consume `deal::simulate::*` and need `deal::CliError` from the lib target, while binary modules reference `crate::CliError` in the bin context. Both definitions are kept identical by convention.
**Source:** 05-03-SUMMARY.md

### The model-value index is built from the AST, not the IR
A shared `ModelValueIndex` (`cli/src/model_values.rs`) walks `deal_ast_json` to resolve model-path → numeric value, rather than reading `deal_ir_json`.

**Rationale:** The IR `attribute_usage` payload carries only `{ name, type_ref, span }` — it has no literal value. The literal `kWh(85)` lives only in the AST: `attribute_usage.default_value` for defs and `component_instance.attrs[].fields[].value` for model instances. The IR could never yield the value, so the resolver had to be AST-backed.
**Source:** 05-08-SUMMARY.md

### D-08-A: unit-call literals canonicalize to bare magnitude
`kWh(85)` resolves to the scalar `85.0` (the unit is discarded), not to an SI base-unit conversion.

**Rationale:** Both operands of a showcase comparison are authored in the same unit and flow through the same shared resolver, so comparing magnitudes (`85 >= 85` → PASS) is correct and symmetric. SI conversion would add asymmetry risk for no benefit at this layer.
**Source:** 05-08-SUMMARY.md

### D-72: MATLAB is a subprocess adapter (`matlab -batch`), superseding the roadmap
An ADR (`ADR-phase-5-matlab-subprocess.md`) records that MATLAB runs via subprocess (`matlab -batch` default), superseding ROADMAP SC#2's "MATLAB Engine API for Python" wording. Engine API is deferred to a future phase as a pluggable adapter.

**Rationale:** Registry-first, tool-agnostic JSON protocol design enables graceful-skip without the Engine API, which is absent in CI.
**Source:** 05-04-SUMMARY.md

### D-73: Zig sims run in-process AND serialize evidence (one execution, two outputs)
The `DealSimulation(T)` comptime wrapper calls `run_with_arena` in-process (fast path) and, when an evidence dir is supplied, also serializes `output.json` + `metadata.json` to disk.

**Rationale:** Avoids a second execution for evidence capture while still producing the on-disk artifacts the evidence/verify chain needs. `@setFloatMode` is derived from the sim's `reproducibility` tier.
**Source:** 05-04-SUMMARY.md

### deal-sim SDK is stdlib-only (D-78)
`deal_sim` input/output validation uses Python stdlib exclusively — no numpy, scipy, or jsonschema imports in the core.

**Rationale:** Airgap-safe / supply-chain minimal (T-05-05). `grep -rE "import (numpy|scipy|jsonschema)"` returns no matches; tests run under stdlib `unittest`.
**Source:** 05-02-SUMMARY.md

### deal-sim lives in its own sibling git repo
The `deal_sim` package at `../deal-sim/` is initialized as its own git repository and is not tracked in `deal/`. The `deal/` repo records only SUMMARY/STATE metadata.

**Rationale:** The directory is outside the `deal/` git tree and cannot be staged there. Plan 01 anticipated this; Plan 02 formalized it by committing SDK work to the sibling repo's own history.
**Source:** 05-02-SUMMARY.md

### PEP 517 build backend is `setuptools.build_meta`
`pyproject.toml` uses `build-backend = "setuptools.build_meta"`.

**Rationale:** RESEARCH.md recommended `setuptools.backends.legacy:build`, which does not exist in setuptools 82.0.1. The correct, available backend is `setuptools.build_meta`.
**Source:** 05-02-SUMMARY.md

### Baseline verdict is transcribed from metadata.json, never computed
`deal evidence baseline` reads the `verdict` field from each sim's `metadata.json` (defaulting to `PARTIAL` when absent/invalid) and never calls `crate::verify::evaluate()`.

**Rationale:** Keeps the baseline command free of a Plan-06 (verify engine) dependency, per plan spec. The authoritative verdict-correctness assertion lives in Plan 07's smoke test.
**Source:** 05-05-SUMMARY.md

### D-81: clean gitignore split for evidence
`.deal/evidence/` (and `.deal/captured/`) are gitignored working caches; `evidence/baselines/<tag>/` is tracked.

**Rationale:** Frozen, content-hashed baselines belong in version control as the audit record; transient per-run evidence does not. Enforced by `git check-ignore` assertions and a dedicated split test.
**Source:** 05-05-SUMMARY.md

### Verify evaluator is a recursive-descent evaluator over raw criteria text
The verify engine tokenizes and recursively evaluates criteria strings (`>= <= == AND + - * / max()`) extracted from AST annotation body fields, rather than walking structured AST sub-nodes.

**Rationale:** The IR stores criteria/compute as raw text in annotation body fields, not as structured sub-trees. A tokenizer + recursive-descent parser is the clean fit. `OR`/`NOT` are rejected at the tokenizer level per the D-55 scope guard.
**Source:** 05-06-SUMMARY.md

---

## Lessons

### Dump the real IR/AST before designing any resolution code
The entire Plan 08 design hinged on first dumping the merged IR `elements` map and the AST for the showcase, which revealed (a) the IR has no literals and (b) IR key form (`vehicle.battery.BatteryPack.usableCapacity`) differs from registry model-path form (`EnergyStorage.battery.usableCapacity`). The resolver was built around the *real* key naming, not an assumption.

**Context:** Had the resolver been coded against assumed key shapes, it would have produced `Unmapped`/null silently — exactly the failure that the 05-07 checkpoint surfaced.
**Source:** 05-08-SUMMARY.md

### Verify research recommendations against the actually-installed versions
RESEARCH.md's recommended PEP 517 backend (`setuptools.backends.legacy:build`) did not exist in the installed setuptools 82.0.1.

**Context:** Research is a starting point, not ground truth — pin claims against the real toolchain version before relying on them.
**Source:** 05-02-SUMMARY.md

### `pip` is a shell alias and does not resolve in `zig build`'s non-interactive shell
Bare `pip` (an `alias pip=pip3` on the dev box) failed with exit 127 in the non-interactive shell that `zig build` spawns. Switching to `python3 -m pip` / `python3 -m unittest` fixed it.

**Context:** Gate scripts must use alias-independent invocations; interactive-shell conveniences silently break automation.
**Source:** 05-07-SUMMARY.md

### PEP 668 (externally-managed Homebrew Python) refuses bare editable installs
The fresh-worktree gate hard-failed because Homebrew's PEP-668 interpreter refuses a bare `pip install -e`. The fix: make the step idempotent (short-circuit when `import deal_sim` already works), add a `--break-system-packages --user` fallback, and assert importability afterward.

**Context:** A gate's real invariant is "the package imports," not "a fresh install runs every time." Forcing an install both wasted work and was fatal where the package was already present.
**Source:** 05-07-SUMMARY.md

### A shared FFI arena causes use-after-free when re-serialized
`deal_check_with_stdlib` builds a `stdlib_arena` for the external symbol table; `analyzeWithExternalTable` *shares* (does not copy) entry pointers from it, and `defer stdlib_arena.deinit()` frees them on return. Calling `deal_index_json` on the returned handle then serialized dangling pointers → SIGSEGV (exit 139).

**Context:** Fixed by skipping `deal_index_json` for handles obtained via `deal_check_with_stdlib`. When an FFI boundary shares vs. copies memory, every downstream consumer of the handle must respect the same lifetime.
**Source:** 05-04-SUMMARY.md

### `real_literal` is a distinct AST node from `float_literal`
Decimal sim inputs (`ohm(0.05)`) stayed null until `value_to_f64` was extended to accept `real_literal` nodes — it had only matched `int_literal`/`float_literal`.

**Context:** Decimals encode as `real_literal` (text "0.05") in this AST. Enumerate every numeric-literal node kind explicitly; an incomplete match list fails silently as `None`.
**Source:** 05-08-SUMMARY.md

### Path resolution must handle the query being *longer* than the indexed key
The registry model_path `EnergyStorage.battery.packResistance` (instance hierarchy, 3 segments) is longer than the indexed part-def key `BatteryPack.packResistance` (2 segments). The original suffix logic only matched a query that was a suffix of a key, so it returned `None`. Progressive-tail matching (strip leading query segments, longest tail first) was required.

**Context:** Instance-hierarchy paths and part-def keys differ in length and direction — resolution needs exact → unique-suffix → progressive-tail, with ambiguity (>1 distinct value) → `None`.
**Source:** 05-08-SUMMARY.md

### Protocol-envelope wrapping breaks tests that assert raw scalar keys
Task-1 tests checked `out["heatGenerated"]` directly; Task 2 wrapped output in the spec/sims/v0 envelope (`{"deal_sim_protocol", "exit_code", "outputs", "v"}`), so raw-key access broke. Tests had to assert `out["outputs"]["heatGenerated"]["value"]`. The same envelope/raw mismatch later caused a `ValueError` when `cli.py` passed the whole envelope to `_validate_inputs()`.

**Context:** When an output contract gains an envelope, every consumer (tests and the SDK's own input path) must unwrap it. Add explicit Shape-A/Shape-B detection rather than assuming a flat dict.
**Source:** 05-02-SUMMARY.md, 05-03-SUMMARY.md

### Recursive `read_dir` in test fixtures trips the CWE-22 semgrep gate
The E2E showcase-copy helper used `std::fs::read_dir` with filesystem-derived path components, tripping the semgrep CWE-22 path-traversal rule and blocking the pre-commit hook. It was replaced with a frozen list of compile-time string-literal paths covering all 26 required showcase files — without shrinking coverage to dodge the rule, and without a `--no-verify` bypass.

**Context:** Taint-free path construction (compile-time literals only) is the project's established mitigation; satisfy the security rule by changing the code, per CLAUDE.md's no-silent-bypass policy.
**Source:** 05-07-SUMMARY.md

---

## Patterns

### C ABI export lifecycle: ArenaAllocator + DealHandle + entry guards
Every C ABI entry point: NULL + non-zero-length consistency checks on all ptr/len pairs before any dereference, UTF-8 validation, source-size guard, arena-backed handle returned to caller for explicit `deal_free`.

**When to use:** Any new `deal_*` C ABI export crossing the Zig/Rust FFI boundary.
**Source:** 05-01-SUMMARY.md

### Wave-0 TDD RED stubs: `#[ignore]` + `@unittest.skip`
Scaffold the full test surface first as ignored/skipped stubs targeting future plans, then flip each to GREEN as its implementation lands.

**When to use:** Multi-wave phases where later plans implement targets scaffolded early; gives a complete, compiling test surface from day one.
**Source:** 05-01-SUMMARY.md, 05-02-SUMMARY.md

### BTreeMap key order + byte-identical-twice test for stable JSON (D-18)
Use `BTreeMap` (Rust) / `sort_keys=True` (Python) on all emitted JSON, and assert that two runs over identical input produce byte-identical output.

**When to use:** Any evidence/manifest/artifact JSON that must be reproducible, hashable, or diff-stable in version control.
**Source:** 05-05-SUMMARY.md, 05-02-SUMMARY.md

### SHA-256 staleness key over ordered inputs + source bytes + registry section
`compute_staleness_key` hashes BTreeMap-ordered inputs JSON + sim source file bytes + the registry TOML section. Any change to inputs, source, or registry flips the key.

**When to use:** Detecting whether cached simulation evidence is fresh vs. stale without re-running the sim (D-83/D-84).
**Source:** 05-03-SUMMARY.md

### Deterministic Kahn's topological sort
Build an output-path → producer index, add producer→consumer edges (plus explicit `depends_on`), sort the zero-in-degree queue (D-18), and detect cycles via `order.len() < registry.len()`.

**When to use:** Ordering a dependency graph where determinism matters (sim dispatch order, build order).
**Source:** 05-03-SUMMARY.md

### Graceful-skip adapter (`ErrorKind::NotFound` → skip.json)
Missing tools (MATLAB absent, unknown tool, zig stub) write a `skip.json` record and return `SimResult::Skipped` — never `Err`. Mirrors the D-58 DOORS precedent.

**When to use:** Orchestrating external tools that may be absent in CI; the gate must continue rather than hard-fail (D-72).
**Source:** 05-03-SUMMARY.md, 05-02-SUMMARY.md

### `OnceLock<Validator>` for schema validation
Lazily build a jsonschema `Validator` once against the pinned schema, with a structural fallback if the schema file is unavailable. Matches the existing `schema_registry.rs` pattern.

**When to use:** Repeated validation against a fixed schema where per-call compilation would be wasteful.
**Source:** 05-03-SUMMARY.md

### `Command::new` + arg vector, never shell interpolation
Spawn subprocesses via `std::process::Command::new(cmd).args(...)`, splitting runner strings by whitespace — never `sh -c` with interpolated input (T-05-07). Combine with a path-alphabet guard (`^[a-zA-Z0-9._]+$`, T-05-06).

**When to use:** Any subprocess dispatch driven by registry/config-supplied command strings.
**Source:** 05-03-SUMMARY.md

### Clone-Before-Free across FFI boundaries
Clone diagnostic bytes returned by an FFI call before calling `deal_free` on the handle (T-05-10 / T-05-17).

**When to use:** Any time Rust reads data owned by a Zig handle that will be freed; prevents use-after-free.
**Source:** 05-04-SUMMARY.md, 05-06-SUMMARY.md

### `EvalValue::Unmapped` propagation → PARTIAL, never panic
An unresolved field yields `EvalValue::Unmapped`, which propagates through every operator chain (`AND`, comparisons, `max()`); any unmapped operand makes the whole expression unmapped, which feeds a `PARTIAL` verdict instead of panicking.

**When to use:** Evaluators over partially-available data where "missing" is a first-class, non-fatal outcome.
**Source:** 05-06-SUMMARY.md

### One shared resolver across both engines
`crate::model_values::ModelValueIndex` is used identically by `verify.rs` and `simulate.rs`, so model_path ↔ value resolution is guaranteed consistent across the verify and simulate paths.

**When to use:** When two subsystems must agree on how an identifier resolves; share the resolver rather than duplicating matching logic.
**Source:** 05-08-SUMMARY.md

### CWE-22 taint-free test fixtures via compile-time string literals
Build corpus-copy file lists from a frozen list of compile-time string-literal paths — no `read_dir`, no filesystem-derived path components — so semgrep p/rust reports 0 findings.

**When to use:** Integration tests that stage a file corpus and must pass a path-traversal security gate.
**Source:** 05-07-SUMMARY.md

### `write_file_atomic` (tmp + rename)
All evidence/manifest writes go through a temp-file-then-rename helper for crash safety.

**When to use:** Writing artifacts that must never be observed half-written (evidence, baselines, indexes).
**Source:** 05-05-SUMMARY.md

---

## Surprises

### The IR carries zero literal values
The merged IR `elements` map exposed no attribute literals at all — `kWh(85)` exists only in the AST. The verify engine's evidence binding and simulate's IR-key lookup both silently produced `Unmapped`/null.

**Impact:** Forced a complete redesign in Plan 08 — a new AST-backed `ModelValueIndex` shared by both engines — and was the root cause of the all-PARTIAL checkpoint failure.
**Source:** 05-08-SUMMARY.md

### A green end-to-end chain produced zero real verdicts
At the 05-07 human-verify checkpoint, the full chain (`simulate → capture → baseline → check --simulations → check --verify`) ran green and exit 0, yet reported `0 PASS, 0 FAIL, 8 PARTIAL` — every requirement defaulted to PARTIAL because no values resolved. The pipeline "worked" while the substance was empty.

**Impact:** Surfaced only because a human checkpoint asserted verdict *correctness*, not mere presence. Plan 08 closed the gap to `2 PASS, 0 FAIL, 6 PARTIAL` and the gate was tightened to assert REQ_BAT_001=PASS (no more `|| true` swallow).
**Source:** 05-07-SUMMARY.md, 05-08-SUMMARY.md

### RESEARCH-recommended PEP 517 backend did not exist
`setuptools.backends.legacy:build` is not a module in setuptools 82.0.1; the build only worked after switching to `setuptools.build_meta`.

**Impact:** A documented research recommendation was wrong against the live toolchain — caught at build time, cheap to fix, but a reminder to verify before trusting.
**Source:** 05-02-SUMMARY.md

### `deal_index_json` on a stdlib-check handle SIGSEGV'd (exit 139)
Indexing the handle returned by `deal_check_with_stdlib` serialized a symbol table whose entry pointers had already been freed when the shared `stdlib_arena` was deinitialized.

**Impact:** A non-obvious cross-arena lifetime bug; fixed by skipping index-building for those handles, at the cost of omitting their entries from the workspace index (acceptable — project files are also parsed via `deal_parse`).
**Source:** 05-04-SUMMARY.md

### Bare `pip` exited 127 under `zig build`
What worked interactively (`pip`, a shell alias) was simply "command not found" in the non-interactive shell `zig build` spawns, hard-failing the fresh gate.

**Impact:** Two separate gate-portability fixes (`python3 -m pip`, then PEP-668 idempotent install) before the fresh worktree ran the full smoke chain to PASS.
**Source:** 05-07-SUMMARY.md

### `phase-5-gate-fresh` failed solely on a pre-existing Phase-4 test
The fresh gate's only red test was `sema.dimensional.regression_pins` (E2503, `07-conversion-mismatch.deal`) — a Phase-4 carryover that fails identically on bare HEAD with no worktree. The entire Phase-5 chain ran green in-worktree.

**Impact:** Required explicitly proving (via git-stash and bare `-Dtest-filter`) that the failure was independent of all Phase-5 work, so it could be accepted as carryover rather than treated as a Phase-5 regression.
**Source:** 05-07-SUMMARY.md, deferred-items.md

### Two plans collapsed into one commit because the tests demanded it
In Plan 06, Tasks 1 (evaluator) and 2 (verdict engine) were committed together — the TDD RED stubs in `verify_engine.rs` drove both implementations in a single pass; splitting them would have left a non-compiling intermediate state.

**Impact:** A clean deviation from the task-per-commit norm, justified by the test surface coupling the two units. 15/15 integration tests green.
**Source:** 05-06-SUMMARY.md

### Plan durations spanned 5 minutes to ~2 hours
Evidence capture (Plan 05, ~5 min) and the SDK (Plan 02, ~17 min) were fast and deviation-light, while the orchestrator (Plan 03, ~2 h) and the gap-closure (Plan 08, ~2 h) absorbed most of the effort — the latter two also carried all the Rule-3 blocking auto-fixes.

**Impact:** Effort concentrated in the cross-language resolution and dispatch glue, not in the individual SDK/evidence components — useful signal for estimating similar integration phases.
**Source:** 05-02/05-03/05-05/05-08-SUMMARY.md (metrics)

---
