# Phase 5: Simulation Integration - Research

**Researched:** 2026-06-08
**Domain:** Simulation orchestration, evidence capture, verification evaluation, Python SDK packaging, Rust-Zig FFI, content-addressed caching
**Confidence:** HIGH (all key findings verified against source code and authoritative references)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-70:** The "kernel" is a language-neutral JSON data contract, not a file-based-subprocess transport. `spec/sims/v0/` becomes a normative spec artifact (input.json / output.json / metadata JSON Schemas + exit-code conventions + `.deal/sims/<name>/` layout), mirroring `spec/ir/v0/`. Every simulation target conforms to this contract. Equivalent to Phase 1's C ABI.

**D-71:** The Rust orchestrator dispatches by the registry `tool` field. `tool = "python"` → run the entry script (SDK `cli()` handles JSON I/O); `tool = "matlab"` → run the registry `runner` (`matlab -batch`) over a thin .m JSON harness; `tool = "zig"` → in-process call (see D-73); `tool = "generic"`/other → run the registry `runner` command following the protocol.

**D-72:** MATLAB = subprocess adapter, `matlab -batch` default (both-via-adapter). A runner-interface abstraction ships with the subprocess `-batch` default; Engine API is a future pluggable adapter. When MATLAB is unlicensed/absent (the CI/dev common case), graceful-skip + record the attempt + reason (Phase 4 D-58 DOORS pattern). Hard gate: Python + Zig sims running for real; MATLAB proven via structural `deal check --simulations` binding validation.

**D-73:** Zig-internal sims run in-process AND emit artifacts — one execution, two outputs. The orchestrator calls them in-process for immediate CLI/GUI results, then serializes the same run's in-memory input/output/metadata to schema-valid evidence artifacts. Default `@reproducibility: "strict"` (`@setFloatMode(.strict)`) makes the in-process fast path bit-reproducible and authoritative. External sims (Python/MATLAB/STK) stay subprocess+JSON.

**D-74:** Interactive artifact-write policy = transient live, persist on capture. Hot-path editor/GUI `deal check --verify` re-runs compute in-process and caches results transiently (no per-keystroke disk churn). Durable evidence artifacts are written only on explicit `deal simulate` / `deal evidence capture` / phase gate.

**D-75:** Phase 5 implements only deferred-decision #5 of ADR-deal-stdlib-numeric-model.md — mapping `@reproducibility` tier → the simulate orchestrator. Concretely: (a) Zig-internal sims inherit the model's `@reproducibility` tier as a real `@setFloatMode`/storage selection at compile time; (b) the declared tier is recorded in evidence metadata for all sims. The full precision system is out of Phase 5.

**D-76:** External sims: declare + validate output + record tier (advisory enforcement). DEAL records the declared `@reproducibility` tier + precision in evidence metadata and validates `output.json` values against declared tolerance/units. Advisory only for external tools.

**D-77:** Build + local/editable install; defer PyPI publish. The wheel is built and the orchestrator + gate install it locally (editable/vendored) so `deal simulate` works end-to-end without a network.

**D-78:** Stdlib-only schema validator. Input/output validation against the declared `inputs`/`outputs` dicts uses Python stdlib only — zero required third-party deps.

**D-79:** Adapters shipped in Phase 5: Python (first-class) + MATLAB (subprocess) + generic subprocess + `deal_sim.zig`. STK and FMI/FMU adapters deferred to Phase 6.

**D-80:** `deal_sim.zig` lives in the `deal` repo, built by the deal toolchain. It is tightly coupled to the compiler's `@setFloatMode`/storage machinery and the Rust orchestrator's in-process call path (D-73), co-located with the toolchain and versioned in lockstep (e.g. `src/sim/` or a Zig module the CLI build links).

**D-81:** Working evidence cache in `.deal/evidence/` (gitignored per PS-10); baselines tracked. Transient/working runs live in gitignored `.deal/`; `deal evidence baseline v2.1.0` writes a durable snapshot to a tracked path (e.g. `evidence/baselines/v2.1.0/`) so the V&V reference is in version control.

**D-82:** `deal evidence baseline` produces a frozen snapshot + manifest. Self-contained: the captured `output.json` set + a manifest recording per-sim content hashes, tool versions, declared `@reproducibility` tier, and the PASS/FAIL/PARTIAL verdicts at capture time. Optionally also git-tagged.

**D-83:** Staleness = content hash of {resolved input values, sim source file, `deal.sims.toml` entry}. Deterministic (D-18 discipline), reproducible, defense-auditable — catches both model-input changes AND sim-logic changes. `mtime` explicitly rejected.

**D-84:** Stale evidence → report STALE, don't auto-run. When `deal check --verify` finds the staleness hash changed, it reports the criterion as STALE (distinct from PASS/FAIL/PARTIAL) and exits non-zero in gate mode. Re-running is opt-in via `deal check --verify --run-sims`.

**D-85:** Rust orchestrates; Zig owns dimensions. Rust loads IR criteria/compute + `output.json`, resolves `evidence` maps, drives freshness + the report. Unit/dimension compatibility + conversions reuse the Zig dimensional engine via the cross-file C ABI (`analyzeWithExternalTable`). No second dimension engine in Rust.

**D-86:** Level-2 verdict rubric (SIM-5): PASS / FAIL / PARTIAL + orthogonal STALE. PASS = all criteria true AND evidence complete AND fresh. FAIL = a criterion evaluates false. PARTIAL = explicit `status="partial"`, or unmapped/missing return fields, or a declared `gap{}` block. STALE is a separate freshness flag layered on top.

**D-87:** The PM query (SC#5) is delivered as a structured `deal check --verify` report keyed by requirement. For each `REQ_*`: satisfaction status, supporting evidence, margins, stale flags — human-readable + JSON.

**D-88:** Wire cross-file dimensional E2500 into `deal check` CLI (carryover from Phase 4 SC-1). The Zig algebra is proven; the fix is `analyzeWithExternalTable` C ABI wiring. Acceptance: the 04-HUMAN-UAT Test 2 scenario makes `deal check` exit non-zero with `error[E2500]` via the CLI, plus a CLI-level integration test.

### Claude's Discretion

- **Criteria/compute expression evaluator scope** (strong steer: implement exactly what the verification blocks + showcase require — comparisons `>=`, `<=`, `==`, boolean `AND`/`OR`/`NOT`, arithmetic `+`, `-`, `*`, `/`, field-path refs, stdlib calls `max`/`min`, unit literals, percent — and defer general expression semantics per D-55 precedent).
- **Orchestration dependency-graph resolution** — how `deal simulate --all` derives execution order (explicit `depends_on` in `deal.sims.toml` vs. inferred from input/output `model_path` overlap between sims).
- **`deal.sims.toml` schema extensions** — any new fields needed (e.g. `tool = "zig"`, reproducibility tier overrides, `runner` for generic) beyond the showcase's existing shape.
- **MATLAB `.m` JSON harness convention** — the thin wrapper that lets a `matlab -batch` script read `input.json` / write `output.json`.
- **In-process Zig call mechanism** — comptime-known function vs. statically/dynamically linked routine; the exact Rust↔Zig in-process boundary for D-73.
- **`spec/sims/v0/` schema shape** — field names, metadata envelope, exit-code table (follow `spec/ir/v0/` conventions and D-18 determinism).
- **Plan slicing** — likely parallelizable tracks.

### Deferred Ideas (OUT OF SCOPE)

- STK adapter + FMI/FMU co-simulation → Phase 6
- MATLAB Engine API adapter → future pluggable adapter
- Full deal-stdlib numeric precision system (sig/±/->, E-PREC*/W-FP*) → separate stdlib/sema track
- General DEAL expression semantics → ADR-deferred
- MQL `deal query` language → ADR-deferred
- PyPI publish of deal-sim → later release gate
- GUI/LSP live-verify integration surface → Phase 6
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REQ-phase-5-simulation | `deal simulate --all` runs showcase Python/MATLAB sims, captures evidence, `deal check --verify` evaluates criteria | Showcase corpus exists and is complete; SDK shape derived from `battery_thermal.py` oracle; orchestration patterns researched |
| REQ-phase-5-1-deal-sim-sdk | `DealSimulation` base class with typed inputs/outputs; CLI runner reads `input.json`, validates, writes `output.json` + metadata | deal-sim README sketches `src/deal_sim/{simulation,cli,validation,metadata}.py`; editable-install pattern verified against Python packaging standards |
| REQ-phase-5-2-orchestration | `deal.sims.toml` parser + dependency graph + staleness detection + `deal simulate` command | Showcase `deal.sims.toml` is the oracle; TOML parsing via `toml` crate already in workspace; dependency resolution patterns documented |
| REQ-phase-5-3-evidence-verification | `deal evidence capture/baseline`; `deal check --verify` with three-level verification | IR lowering of verification blocks already complete; evidence cache layout and staleness algorithm documented; verify engine patterns from `sysml_v2.rs`/`reqif.rs` |
| REQ-phase-5-gate | Phase gate: showcase sims run, produce evidence, verification criteria evaluate, PM can query REQ_BAT_001 | Gate structure follows phase-4-gate precedent; fresh-worktree script extension documented |
</phase_requirements>

---

## Summary

Phase 5 makes the showcase model's verification chain executable end-to-end. The foundation is already built: the DEAL language surface for verification blocks, satisfy blocks, criteria, evidence, compute, and traceability is parsed and lowered to IR JSON by the Zig compiler (Phases 1/2). Phase 5 connects that static model to running simulations by building three interlocking pieces: the `deal-sim` Python SDK in the sibling repo, the Rust CLI orchestration layer, and the verification evaluation engine.

The critical insight from examining the codebase is that `analyzeWithExternalTable` in `src/sema.zig` is NOT currently exposed via the C ABI — it is internal/test-only. The Phase 5 verify engine (D-85) and the E2500 carryover (D-88) both need this function reachable from Rust. Exposing it as a new C ABI function is the single most important new Zig-Rust seam in Phase 5. Everything else is largely Rust orchestration + Python SDK work layered onto the existing CLI patterns.

The showcase corpus is complete and acts as the oracle: 5 simulations in `deal.sims.toml` (3 Python, 2 MATLAB), 8 satisfy blocks in `traceability.dealx`, and requirement contracts in `system.deal`. The expression evaluator scope is narrow and well-defined by examining these files: the verification criteria use only `>=`, `<=`, `AND`, field path refs (`REQ_SYS_001.minRange`), and computed intermediates. The `max()` stdlib call appears in REQ_BAT_002's compute block. That is the complete required grammar.

**Primary recommendation:** Parallelize three tracks — (1) Python SDK + `spec/sims/v0/` protocol spec, (2) Rust orchestrator + TOML parser + adapter dispatch + staleness, (3) Zig `deal_sim.zig` + new C ABI export for `analyzeWithExternalTable` + E2500 CLI wiring — then converge on (4) evidence cache + baseline, (5) verify engine, (6) gate.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `deal.sims.toml` parsing | Rust CLI | — | Follows existing TOML parsing in `resolver.rs`; Rust owns all manifest/config parsing |
| Dependency-graph ordering | Rust CLI | — | Topological sort is pure data transformation over registry entries |
| Subprocess dispatch (Python/MATLAB/generic) | Rust CLI | — | `std::process::Command` is the established subprocess pattern |
| In-process Zig sim execution | Zig core → Rust CLI | — | Zig owns the compiled sim; Rust owns the call boundary and artifact serialization |
| Evidence cache I/O | Rust CLI | — | File system operations are Rust's domain; follows `.deal/` management pattern |
| Content-hash staleness | Rust CLI | — | SHA-256 over input values + file bytes; pure Rust, no Zig needed |
| Unit/dimension compatibility | Zig core (via C ABI) | Rust CLI | D-85: Zig owns dimensional truth; Rust drives orchestration |
| Expression evaluation (criteria) | Rust CLI | — | Narrow grammar over IR values; no dimensional semantics beyond unit-compatibility calls to Zig |
| Verification report generation | Rust CLI | — | Follows `sysml_v2.rs`/`reqif.rs` IR-walk pattern |
| Python SDK base class | Python (`deal-sim`) | — | Airgapped, stdlib-only, no Rust involvement |
| SDK input/output validation | Python (`deal-sim`) | — | Stdlib-only dict type checking |
| MATLAB `.m` JSON harness | MATLAB script | Rust CLI (runner) | Thin wrapper; Rust only launches it |
| `@reproducibility` → float mode | Zig core | Rust (metadata recording) | Zig comptime knows `@setFloatMode`; Rust records declared tier in evidence |
| Baseline manifest generation | Rust CLI | — | Snapshot + hash manifest; follows D-18 alphabetical key discipline |

---

## Standard Stack

### Core (Existing — Already in Workspace)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `clap` | 4.6 | CLI subcommand surface | Already in workspace; adds `Simulate`, `Evidence`, `--verify` flags |
| `serde_json` | 1 | JSON parse/emit for IR + output.json | Already in workspace; D-18 alphabetical-key invariant enforced |
| `toml` | 0.10.2 | Parse `deal.sims.toml` registry | Already in workspace (`resolver.rs` uses it for `deal.toml`) |
| `anyhow` | 1 | Error handling | Already in workspace |
| `sha2` | 0.11.0 | Content-hash staleness (SHA-256) | Established crates.io crate; deterministic; auditable |

[VERIFIED: crates.io — confirmed via `cargo search` on 2026-06-08]

### Supporting (New Additions)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `sha2` | 0.11.0 | SHA-256 for staleness keys (D-83) | Add to workspace deps alongside uuid |
| `walkdir` | 2.5.0 | Recursive file glob for evidence cache scan | Used in evidence capture + baseline manifest |
| `build` (Python) | 1.5.0 | Build `deal-sim` wheel for local install | Python packaging tool; confirmed on PyPI |
| `setuptools` (Python) | 82.0.1 | Backend for `deal-sim` wheel | Already installed; required by `build` |

[VERIFIED: crates.io for sha2, walkdir — confirmed via `cargo search` on 2026-06-08]
[VERIFIED: PyPI for build, setuptools — confirmed via `pip index versions` on 2026-06-08]

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `sha2` (Rust) | `std::hash` | std::hash is not cryptographic and not byte-stable across process restarts; SHA-256 is the auditable choice for defense |
| `walkdir` | `std::fs::read_dir` recursion | `walkdir` handles symlinks + depth limits; the evidence cache layout is shallow so either works; `walkdir` is cleaner |
| Python `build` tool | `flit`, `hatch` | `build` is the PEP 517 reference frontend; matches the D-77 "build + local install" intent without extra tooling |

**Installation (Rust additions to workspace):**
```toml
# In Cargo.toml [workspace.dependencies]
sha2 = "0.11"
walkdir = "2"
```

**Python SDK local install:**
```bash
cd deal-sim && pip install -e .
```

---

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| sha2 | crates.io | ~8 yrs | High | github.com/RustCrypto/hashes | OK | Approved |
| walkdir | crates.io | ~8 yrs | Very high | github.com/BurntSushi/walkdir | OK | Approved |
| serde_json | crates.io | ~9 yrs | Very high | github.com/serde-rs/json | OK | Approved |
| toml | crates.io | ~9 yrs | Very high | github.com/toml-rs/toml | OK | Approved |
| anyhow | crates.io | ~6 yrs | Very high | github.com/dtolnay/anyhow | OK | Approved |
| clap | crates.io | ~9 yrs | Very high | github.com/clap-rs/clap | OK | Approved |
| build (Python) | PyPI | ~5 yrs | High | github.com/pypa/build | [ASSUMED] | Approved — confirmed on PyPI |
| deal-sim (Python) | PyPI | N/A | N/A | Does not exist on PyPI | N/A | To be created; not a registry install |
| deal-sim (crates.io) | crates.io | N/A | N/A | Does not exist | [SLOP] | REMOVED — not a Rust package; the name `deal-sim` on crates.io is flagged [SLOP] by slopcheck; the Python package is created from scratch in the sibling repo |

**Packages removed due to slopcheck [SLOP] verdict:** `deal-sim` (Rust) — not applicable; `deal-sim` is a Python package created from scratch, not pulled from any registry.
**Packages flagged as suspicious [SUS]:** none.

Note: `build` (Python PyPI) was confirmed present on PyPI via `pip index versions build` but not verified via slopcheck (Python ecosystem). [ASSUMED] tag applied; standard PEP 517 frontend from the Python Packaging Authority.

---

## Architecture Patterns

### System Architecture Diagram

```
deal.sims.toml registry
        |
        v
 [Rust: deal simulate]
    TOML parser
    dependency-graph resolver (topological sort)
    staleness checker (SHA-256 content hash)
        |
     dispatch by tool=
        |
    +---+---+---+
    |       |   |
  python  matlab  zig (in-process)
  adapter  adapter  |
    |       |       v
  spawn   spawn  [Zig: deal_sim.zig]
  entry.py runner  @setFloatMode
    |       |       serialize to JSON
    |       |       (one execution, two outputs)
    v       v       |
 input.json + output.json  <--- JSON Simulation Protocol (spec/sims/v0/)
    |
    v
 .deal/evidence/<sim-name>/
    output.json
    metadata.json (hash, tool version, @reproducibility tier, timestamp)
        |
        |-- deal evidence capture --> evidence/baselines/v2.1.0/ (tracked)
        |
        v
 [Rust: deal check --verify]
    load IR criteria/compute/evidence-maps (deal_ir_json)
    resolve evidence bindings -> output.json values
    staleness check (compare stored hash vs current)
    expression evaluator (showcase grammar subset)
    Zig dimension check (analyzeWithExternalTable via new C ABI)
        |
        v
 verification report (per REQ_* ID)
    PASS / FAIL / PARTIAL + STALE flag + margins
    human-readable + JSON (D-87)
```

### Recommended Project Structure

```
deal/ (existing Rust CLI workspace)
├── src/
│   ├── sim/
│   │   ├── deal_sim.zig       # Zig SDK — DealSimulation struct, @setFloatMode, artifact emission
│   │   └── zig_runner.zig     # In-process runner: call deal_sim fn, serialize evidence
│   └── sema.zig               # + new C ABI export: analyzeWithExternalTableFFI
├── cli/src/
│   ├── main.rs                # + Simulate, Evidence subcommands; --verify, --simulations, --stale, --run-sims flags on Check
│   ├── simulate.rs            # Orchestration: TOML parse, dep-graph, staleness, dispatch
│   ├── evidence.rs            # Evidence cache I/O, capture, baseline manifest
│   ├── verify.rs              # Expression evaluator, three-level verdict, per-REQ report
│   └── sims_protocol.rs       # Schema validation for output.json against spec/sims/v0/
├── spec/
│   └── sims/v0/               # NORMATIVE — new
│       ├── input.schema.json
│       ├── output.schema.json
│       ├── metadata.schema.json
│       └── README.md
└── scripts/
    └── verify-fresh-worktree.sh  # Extend: add deal-sim symlink + python install step

deal-sim/ (sibling repo)
├── src/deal_sim/
│   ├── __init__.py
│   ├── simulation.py     # DealSimulation base class
│   ├── cli.py            # CLI runner: argparse, read input.json, call run(), write output.json
│   ├── validation.py     # Stdlib-only type/unit checking
│   ├── metadata.py       # Timestamp, duration, tool version
│   └── adapters/
│       ├── matlab.py     # subprocess -batch adapter
│       └── generic.py    # generic subprocess adapter
├── tests/
├── pyproject.toml
└── README.md             # Update: mark STK/FMI deferred
```

### Pattern 1: Topological Sort for Simulation Dependency Resolution

**What:** `deal simulate --all` must execute sims in dependency order so a sim that consumes another's `model_path` output sees fresh values.

**When to use:** Always when the registry has sims whose `inputs[].model_path` references a field that is also an `outputs[].model_path` of another sim.

**Research conclusion — explicit `depends_on` vs. inferred from `model_path` overlap:**

Examining the showcase registry: `range_model` depends on `EnergyStorage.battery.usableCapacity` as an input. `battery_thermal` writes `EnergyStorage.battery.heatGenerated`. These are different paths. `motor_thermal` reads `Propulsion.motor.peakPower` and `motor_efficiency` writes `Propulsion.motor.peakPower` (via `deratedPower` → `peakPower`). This is a real dependency. `vehicle_dynamics` reads `Propulsion.motor.peakTorque` and writes `zerotoSixty`; `motor_efficiency` also reads `peakTorque`.

**Recommendation: infer from `model_path` overlap with `depends_on` as explicit override.** The showcase already has implicit dependencies discoverable by comparing `inputs[].model_path` against other sims' `outputs[].model_path`. Explicit `depends_on` is a schema extension the planner should add to `deal.sims.toml` for cases where inference fails or the user wants to override. Cycle detection is required; standard Kahn's algorithm on a StringHashMap-keyed adjacency list is sufficient for the showcase scale (5 nodes).

**Implementation:**
```rust
// Infer: for each sim, collect all output model_paths from other sims.
// If any input.model_path matches an output.model_path of sim B, add edge B -> self.
// Run Kahn's algorithm for topological order; cycle = E-code error.
fn build_dep_graph(registry: &SimsRegistry) -> Result<Vec<String>, CliError> {
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    // ... Kahn's algorithm
}
```

[VERIFIED: codebase analysis — showcase registry paths examined directly]

### Pattern 2: Content-Hash Staleness Key (D-83)

**What:** A sim is stale iff SHA-256(resolved_input_values + sim_source_bytes + registry_entry_toml) differs from the hash recorded in the sim's evidence metadata.

**When to use:** Every `deal simulate --stale` and every `deal check --verify` staleness check.

**Key insight:** The hash must include the sim source file bytes, not just the model inputs. Without this, editing `battery_thermal.py` and re-running would silently reuse stale evidence. The showcase's `evidence simulation { source: "..."; }` provides the path; the orchestrator reads and hashes that file.

```rust
use sha2::{Sha256, Digest};

fn compute_staleness_key(
    resolved_inputs: &serde_json::Value,   // sorted JSON of resolved input values
    sim_source_path: &Path,               // path from registry entry
    registry_entry_toml: &str,            // the [simulations.name] TOML section as string
) -> String {
    let mut h = Sha256::new();
    // D-18 determinism: serialize inputs with sorted keys
    h.update(serde_json::to_vec(resolved_inputs).unwrap()); // already BTreeMap-ordered
    h.update(std::fs::read(sim_source_path).unwrap_or_default());
    h.update(registry_entry_toml.as_bytes());
    format!("{:x}", h.finalize())
}
```

[ASSUMED — SHA-256 approach is standard practice; specific field ordering confirmed from D-83 decision]

### Pattern 3: Python SDK CLI Runner Protocol

**What:** `DealSimulation.cli()` is the entry point invoked by `python entry.py`. It reads `input.json`, validates inputs, calls `run(inputs) -> dict`, validates outputs, writes `output.json` + `metadata.json`.

**Oracle:** `battery_thermal.py` is the reference. The SDK must make this exact file run without modification.

```python
# Source: deal-sim/README.md + battery_thermal.py oracle
class DealSimulation:
    inputs: dict  # {"param_name": {"type": "Real", "unit": "ohm"}}
    outputs: dict # {"param_name": {"type": "Real", "unit": "W"}}

    def run(self, inputs: dict) -> dict:
        raise NotImplementedError

    @classmethod
    def cli(cls):
        import argparse, json, sys, time, pathlib
        parser = argparse.ArgumentParser()
        parser.add_argument("--input", default="input.json")
        parser.add_argument("--output", default="output.json")
        parser.add_argument("--metadata", default="metadata.json")
        args = parser.parse_args()

        with open(args.input) as f:
            raw_inputs = json.load(f)

        instance = cls()
        # Validate inputs against cls.inputs schema (stdlib-only)
        validated = instance._validate_inputs(raw_inputs)

        t0 = time.time()
        result = instance.run(validated)
        elapsed = time.time() - t0

        # Validate outputs against cls.outputs schema
        instance._validate_outputs(result)

        with open(args.output, "w") as f:
            json.dump(sorted_dict(result), f, indent=2)  # D-18 sorted keys

        with open(args.metadata, "w") as f:
            json.dump({
                "deal_sim_version": ...,
                "duration_s": elapsed,
                "reproducibility_tier": "tolerant",  # external default
                "timestamp": ...
            }, f, indent=2)
```

[VERIFIED: battery_thermal.py oracle examined in-repo on 2026-06-08]

### Pattern 4: New C ABI Export for analyzeWithExternalTable (D-85, D-88)

**What:** Phase 5 needs `analyzeWithExternalTable` reachable from Rust to (a) wire E2500 into the CLI check path (D-88) and (b) drive dimensional compatibility checks in the verify engine (D-85).

**Current state:** `analyzeWithExternalTable` is declared `pub` in `src/sema.zig` (line 260) but is documented "NOT exported to the C ABI — internal/test use only." It takes an arena allocator, which cannot cross the C boundary directly.

**Recommended approach:** Add a new C ABI function `deal_check_dimension_compat` that accepts two IR JSON strings (the pre-built merged unit table + the file to check) and returns a diagnostics JSON array. This is the pattern already established by `deal_parse` → `deal_diagnostics_json`. The Rust side builds the merged external table from stdlib sources (already done for D-49 in `run_check`) and calls this new entry point with each project file.

```zig
// src/lib.zig addition
export fn deal_check_with_stdlib(
    source_ptr: [*]const u8, source_len: usize,
    filename_ptr: [*]const u8, filename_len: usize,
    stdlib_ir_ptr: [*]const u8, stdlib_ir_len: usize, // pre-built stdlib unit table JSON
    out_diag_ptr: *[*]const u8, out_diag_len: *usize,
) bool {
    // Deserialize stdlib unit table, call analyzeWithExternalTable
    // return diagnostics JSON
}
```

[VERIFIED: sema.zig examined directly — analyzeWithExternalTable signature confirmed at line 260]

### Pattern 5: Verify Engine Expression Evaluator (Claude's Discretion)

**What:** `deal check --verify` evaluates `criteria {}` and `compute {}` blocks in the IR.

**Showcase grammar inventory** — examining all 8 satisfy blocks in `traceability.dealx`:

| Operation | Used in | Example |
|-----------|---------|---------|
| `>=` | REQ_SYS_001, REQ_BAT_001, REQ_MOT_001, REQ_THM_001 | `actualRange >= REQ_SYS_001.minRange` |
| `<=` | REQ_SYS_002, REQ_SYS_003, REQ_BAT_002, REQ_BAT_003 | `actual060 <= REQ_SYS_002.maxTime` |
| `AND` | REQ_BAT_003, REQ_THM_001 | `actualMaxTemp <= REQ_BAT_003.maxTemp AND actualMinTemp >= REQ_BAT_003.minTemp` |
| `-` subtraction | Multiple compute blocks | `margin = actualRange - REQ_SYS_001.minRange` |
| `/` division | Multiple compute blocks | `marginPercent = margin / REQ_SYS_001.minRange * 100` |
| `*` multiply | Multiple compute blocks | `* 100` |
| `max(...)` | REQ_BAT_002 compute | `worstCase = max(chargeTime_25C, chargeTime_neg10C, chargeTime_neg30C)` |
| field path ref | All | `REQ_SYS_001.minRange`, `EnergyStorage.battery.usableCapacity` |

**Conclusion:** The showcase requires exactly: comparison operators (`>=`, `<=`, `==`), logical AND (no OR in showcase), arithmetic (`+`, `-`, `*`, `/`), field path references (dot-separated), one stdlib call (`max`). This is the full grammar; defer everything else.

**Recommended evaluator:** A simple recursive-descent evaluator over IR values (not a parser — the IR is already structured). Field path resolution walks the merged IR elements map. `max` is a special-cased built-in.

[VERIFIED: traceability.dealx and system.deal examined directly on 2026-06-08]

### Pattern 6: MATLAB JSON Harness (Claude's Discretion)

**What:** The D-72 subprocess adapter runs `matlab -batch "motor_efficiency"` which must find `input.json` and write `output.json`.

**Convention:** The `.m` file reads `input.json` via MATLAB's `jsondecode(fileread('input.json'))` and writes output via `writelines(jsonencode(output), 'output.json')`. The harness is minimal; MATLAB's batch mode handles the rest.

```matlab
% motor_efficiency.m  (thin JSON harness wrapping the actual physics)
% Called by: matlab -batch "motor_efficiency"
% Input:  input.json in the current working directory
% Output: output.json in the current working directory

data = jsondecode(fileread('input.json'));
peakTorque = data.peakTorque;
maxSpeed = data.maxSpeed;

% [actual physics here]
efficiencyMap = compute_efficiency(peakTorque, maxSpeed);

output.efficiencyMap = efficiencyMap;
writelines(jsonencode(output), 'output.json');
```

The Rust adapter sets `--workdir` to the sim-specific working directory before spawning MATLAB.

[ASSUMED — MATLAB jsondecode/jsonencode are standard MATLAB built-ins since R2016b; exact syntax from training knowledge; no official docs verification performed]

### Pattern 7: Zig In-Process Call Mechanism (Claude's Discretion)

**What:** The `deal_sim.zig` SDK and the Rust in-process call boundary for D-73.

**Recommended approach — statically linked Zig sim function:**

Each Zig sim exports a C-callable function matching a known signature:
```zig
// In deal_sim.zig
pub const SimFn = fn (input_json: [*]const u8, input_len: usize,
                       out_json: *[*]const u8, out_len: *usize) callconv(.C) bool;

pub fn DealSimulation(comptime T: type) type {
    return struct {
        pub export fn sim_run(
            input_json: [*]const u8, input_len: usize,
            out_json: *[*]const u8, out_len: *usize,
        ) callconv(.C) bool {
            // @setFloatMode based on @reproducibility annotation
            const mode = if (@hasDecl(T, "reproducibility")) T.reproducibility else .strict;
            @setFloatMode(mode);
            // deserialize input, call T.run(), serialize output
        }
    };
}
```

The Rust orchestrator calls `sim_run` via FFI (same `deal-ffi` crate pattern), then serializes the returned JSON to the evidence artifact. The Zig sim must be compiled into `libdeal.a` alongside the other exports, OR as a separate object that the CLI links. The simplest approach: add the Zig sim to `build.zig`'s library target when `tool = "zig"` is registered.

For Phase 5 gate: ship one canonical Zig sim (e.g. a `range_model.zig` equivalent) to prove the in-process path.

[ASSUMED — Zig `@setFloatMode` confirmed in ADR-deal-stdlib-numeric-model.md; exact comptime dispatch pattern is a recommendation, not verified against Zig 0.16.0 docs]

### Anti-Patterns to Avoid

- **mtime-based staleness:** Non-reproducible across clone/checkout. Use SHA-256 content hash (D-83).
- **Auto-running sims on `deal check --verify`:** Violates D-84. Always report STALE and require explicit opt-in via `--run-sims`.
- **Third-party Python deps in SDK core:** Violates D-78. `numpy`/`scipy` are optional for sim authors, never required by `deal_sim` itself.
- **Re-implementing dimensional algebra in Rust:** Violates D-85. All unit/dimension checks route through `analyzeWithExternalTable` via C ABI.
- **Folding STALE into PARTIAL:** Violates D-86. STALE is orthogonal to the PASS/FAIL/PARTIAL verdict and must be a separate flag.
- **General expression language scope creep:** The showcase requires only the operators catalogued in Pattern 5. Implementing a full expression language violates the D-55 precedent.
- **Writing evidence artifacts on every `deal check --verify`:** Violates D-74. Evidence persists only on `deal simulate` / `deal evidence capture` / phase gate.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Content hashing | Custom hash | `sha2` crate (SHA-256) | Cryptographically stable, byte-deterministic, auditable for DoD |
| TOML parsing | Custom parser | `toml` crate (already in workspace) | TOML edge cases (multi-line strings, inline tables) are non-trivial |
| JSON Schema validation | Custom validator | `jsonschema` crate (already in workspace) | Used by `schema_registry.rs` already; same pattern for `spec/sims/v0/` |
| Topological sort | Custom BFS | Kahn's algorithm (20 lines) | Well-understood; cycle detection is built-in |
| Python wheel build | Manual zipfile | `build` (PyPI) | PEP 517 standard; handles metadata, entry points, editable install |

**Key insight:** The existing CLI codebase already has all the patterns needed. The new code is wiring, not invention.

---

## Common Pitfalls

### Pitfall 1: analyzeWithExternalTable Not in C ABI

**What goes wrong:** The D-85 verify engine and D-88 E2500 CLI wiring both need `analyzeWithExternalTable` from Rust. A plan that treats it as already available via the C ABI will silently fail at compile time.

**Why it happens:** The function is `pub` in Zig (accessible to test harnesses) but explicitly NOT exported (`export fn`). The comment on line 259 of `src/sema.zig` says "NOT exported to the C ABI — internal/test use only."

**How to avoid:** Plan Wave 0 or Wave 1 must add a new `export fn deal_check_with_stdlib(...)` that wraps `analyzeWithExternalTable`. This is the prerequisite for both D-85 and D-88.

**Warning signs:** Build errors like "undefined symbol: deal_check_with_stdlib" in the Rust test harness.

### Pitfall 2: deal-sim Sibling Not Materialized in Fresh Worktree

**What goes wrong:** `phase-5-gate-fresh` calls `verify-fresh-worktree.sh` which symlinks sibling repos. The script currently symlinks `tree-sitter-deal vscode-deal deal-stdlib deal-lang.org` (line 165). `deal-sim` is not in this list.

**Why it happens:** The script was written before `deal-sim` was in scope. `deal simulate` will call `python entry.py` which requires `deal_sim` to be importable.

**How to avoid:** Add `deal-sim` to the sibling list in `verify-fresh-worktree.sh` line 165, and add a `pip install -e ../deal-sim` step to the gate.

**Warning signs:** `ModuleNotFoundError: No module named 'deal_sim'` in gate runs.

### Pitfall 3: Evidence Cache Path vs. Baseline Path

**What goes wrong:** `.deal/evidence/` is gitignored (PS-10 designates `.deal/` as the sim I/O cache). If evidence artifacts end up in `evidence/baselines/v2.1.0/` (tracked), the gitignore must be carefully scoped.

**Why it happens:** `.deal/` is gitignored globally; `evidence/baselines/` is a sibling directory that must be tracked.

**How to avoid:** Working evidence cache lives in `.deal/evidence/<sim-name>/`. Baseline snapshots live in `evidence/baselines/v2.1.0/` at the repo root (separate from `.deal/`). The `.gitignore` covers `.deal/` only.

**Warning signs:** `git status` shows untracked `evidence/` files that should be committed, or committed `.deal/` files that should be gitignored.

### Pitfall 4: D-18 Sort Order in Evidence Artifacts

**What goes wrong:** Evidence JSON artifacts (output.json, metadata.json, manifest.json) must follow the D-18 alphabetical-key invariant for byte-stable hashing. If any artifact uses `HashMap`-ordered keys, the staleness hash becomes non-deterministic.

**Why it happens:** `serde_json::json!{}` macro uses insertion order; `serde_json::to_string` with a `Value::Object` backed by `IndexMap` is not alphabetical.

**How to avoid:** Use `BTreeMap` when building JSON objects in Rust (same pattern as workspace index merge in `run_check`). Python side: use `sort_keys=True` in `json.dump`. Document explicitly in `spec/sims/v0/README.md`.

**Warning signs:** Staleness hash changes without any model or code edits; test flakiness in staleness detection.

### Pitfall 5: MATLAB Absent in CI — Gate Must Not Fail Hard

**What goes wrong:** `deal simulate --all` in CI includes MATLAB sims. If MATLAB is not licensed, the gate must not exit non-zero for MATLAB skips.

**Why it happens:** D-72 requires graceful-skip + record, not hard failure. The Phase 4 D-58 DOORS precedent: record attempt + reason.

**How to avoid:** When the MATLAB subprocess fails with a "not found" or license error, write a skip record to `.deal/evidence/motor_efficiency/skip.json` with `{"skipped": true, "reason": "matlab not found"}`. `deal check --simulations` (structural validation) should pass; `deal check --verify` for MATLAB-backed criteria should emit STALE (not FAIL).

**Warning signs:** CI gate fails on `deal simulate --all` due to MATLAB license error.

### Pitfall 6: Input Resolution — model_path vs. Actual IR Values

**What goes wrong:** `deal.sims.toml` `inputs[].model_path` paths like `EnergyStorage.battery.packResistance` must resolve against the model's actual attribute values in the IR, not just structurally validate. If the orchestrator only checks that the path exists (not that it has a value), `input.json` may contain `null` values.

**Why it happens:** The IR carries attribute values as literals in the `value` field. The orchestrator must traverse the IR elements map using the path-string ID convention (D-23: `package.element.member`) to extract the resolved numeric value.

**How to avoid:** Use the workspace IR (from `deal_ir_json` over the project files) as the source of truth for input values. Map `model_path` → D-23 qualified path → IR element value. Log a diagnostic if any required sim input cannot be resolved.

**Warning signs:** `input.json` contains `null` for fields that should have numeric values; sim runs succeed but produce unexpected outputs.

---

## Code Examples

### spec/sims/v0/ Protocol Schema (Recommended Shape)

```json
// input.json — inputs to the simulation
{
  "deal_sim_protocol": "v0",
  "inputs": {
    "packResistance": {"value": 0.05, "unit": "ohm"},
    "totalCurrent": {"value": 250.0, "unit": "A"},
    "coolantFlowRate": {"value": 30.0, "unit": "L/min"}
  }
}

// output.json — outputs from the simulation
{
  "deal_sim_protocol": "v0",
  "outputs": {
    "heatGenerated": {"value": 3125.0, "unit": "W"},
    "coolantOutTemp": {"value": 22.45, "unit": "degC"}
  },
  "exit_code": 0
}

// metadata.json — execution record
{
  "deal_sim_protocol": "v0",
  "deal_sim_version": "0.1.0",
  "duration_s": 0.012,
  "reproducibility_tier": "strict",
  "sim_source_hash": "sha256:abc123...",
  "timestamp": "2026-06-08T12:00:00Z",
  "tool": "python",
  "tool_version": "3.14.5"
}
```

All keys alphabetical (D-18). Mirrors `spec/ir/v0/` conventions.

[VERIFIED: spec/ir/v0/README.md conventions examined; field names follow the ID strategy + metadata envelope patterns]

### `deal.sims.toml` Schema Extensions Needed

The showcase registry already covers most fields. Required additions:

```toml
# New fields for Phase 5 (additions to existing shape)
[simulations.range_model_zig]
tool = "zig"                      # NEW: Zig in-process dispatch
entry = "src/sim/range_model.zig" # path to the Zig sim module
class = "RangeModel"              # Zig struct name
reproducibility = "strict"        # NEW: override default @reproducibility tier
binds_to = "model/vehicle.dealx::EVPlatform"
inputs = [...]
outputs = [...]

[simulations.custom_tool]
tool = "generic"                  # NEW: generic subprocess
runner = "./scripts/my_sim.sh"   # arbitrary command
# runner receives: --input <path> --output <path> --metadata <path>
depends_on = ["battery_thermal"] # NEW: explicit dependency override
```

[VERIFIED: showcase registry examined directly; extensions derived from D-71/D-79 decisions]

### Kahn's Topological Sort for Dependency Resolution

```rust
// Source: standard Kahn's algorithm over the sim registry
fn topological_order(registry: &HashMap<String, SimEntry>) -> Result<Vec<String>, CliError> {
    // Build output_paths -> sim_name index
    let mut output_producer: HashMap<String, String> = HashMap::new();
    for (name, sim) in registry {
        for out in &sim.outputs {
            if let Some(mp) = &out.model_path {
                output_producer.insert(mp.clone(), name.clone());
            }
        }
    }

    // Build adjacency: for each sim, find sims it depends on
    let mut in_degree: HashMap<String, usize> = registry.keys().map(|k| (k.clone(), 0)).collect();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for (name, sim) in registry {
        for inp in &sim.inputs {
            if let Some(mp) = &inp.model_path {
                if let Some(producer) = output_producer.get(mp) {
                    if producer != name {
                        adj.entry(producer.clone()).or_default().push(name.clone());
                        *in_degree.entry(name.clone()).or_default() += 1;
                    }
                }
            }
        }
        // Also check explicit depends_on
        for dep in sim.depends_on.iter().flatten() {
            adj.entry(dep.clone()).or_default().push(name.clone());
            *in_degree.entry(name.clone()).or_default() += 1;
        }
    }

    // Kahn's BFS
    let mut queue: Vec<String> = in_degree.iter()
        .filter(|(_, &d)| d == 0)
        .map(|(k, _)| k.clone())
        .collect();
    queue.sort(); // D-18 deterministic ordering
    let mut order = Vec::new();
    // ... process queue, detect cycles
    if order.len() < registry.len() {
        return Err(CliError::User("Circular dependency in deal.sims.toml".into()));
    }
    Ok(order)
}
```

[ASSUMED — Kahn's algorithm is standard CS; implementation pattern derived from project's existing BTreeMap/HashMap patterns]

### `deal check --verify` Report Shape (D-87)

```json
{
  "command": "verify",
  "deal_version": "0.1.0",
  "requirements": {
    "REQ_BAT_001": {
      "verdict": "PASS",
      "stale": false,
      "criteria_results": [
        {"expr": "actualCapacity >= REQ_BAT_001.minCapacity", "result": true,
         "lhs": {"value": 85.0, "unit": "kWh"}, "rhs": {"value": 75.0, "unit": "kWh"}}
      ],
      "compute_results": {
        "margin": {"value": 10.0, "unit": "kWh"},
        "marginPercent": {"value": 13.33, "unit": null}
      },
      "evidence": [
        {"type": "design", "status": "mapped", "binding": "EnergyStorage.battery.usableCapacity"}
      ]
    },
    "REQ_BAT_002": {
      "verdict": "PARTIAL",
      "stale": false,
      "gap": {"description": "30-min target met at 25°C; untested below 0°C", "risk": "high"}
    }
  },
  "summary": {"pass": 5, "fail": 0, "partial": 2, "stale": 0},
  "v": 1
}
```

[VERIFIED: traceability.dealx and system.deal examined; D-86/D-87 decisions applied; envelope follows D-32 alphabetical-key convention]

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| MATLAB Engine API for Python (ROADMAP SC#2) | subprocess `-batch` with adapter abstraction (D-72) | Phase 5 context decision 2026-06-08 | ADR required to record supersession; Engine API remains a future pluggable adapter |
| Single-file C ABI (`deal_parse` per file) | Multi-file with external table seeding | Phase 5 (D-88) | Cross-file dimensional errors now surfaced via CLI |
| No Zig simulation story | `deal_sim.zig` SDK + in-process call | Phase 5 (D-73/D-80) | Zig becomes a first-class simulation language |

**Deprecated/outdated:**

- ROADMAP SC#2 "MATLAB Engine API for Python" — superseded by D-72 (subprocess `-batch` default). ADR to record this supersession is a Phase 5 deliverable.
- The comment in `run_check` (main.rs line ~229): "the current C ABI processes each file independently via deal_parse... Full cross-file symbol sharing requires the analyzeWithExternalTable path..." — this becomes outdated when D-88 lands. Update the comment.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `analyzeWithExternalTable` needs a new C ABI wrapper function (it is not currently exported) | Standard Stack / Pitfall 1 | If it is somehow already reachable, the new-export task is unnecessary — but adding it is safe; it would just be unused |
| A2 | MATLAB `jsondecode`/`jsonencode` are available in the target MATLAB version for the `.m` harness pattern | Pattern 6 | Available since MATLAB R2016b; if an older version is required, `loadjson`/`savejson` from JSONLab would be needed |
| A3 | `deal-sim` Python package does NOT currently exist on PyPI | Package Legitimacy Audit | Confirmed `pip index versions deal-sim` returns ERROR on 2026-06-08; the package is created from scratch |
| A4 | Kahn's algorithm topological sort is sufficient for the showcase (5 nodes, at most 2-3 edges) | Pattern 1 | Correct for showcase; would need generalization only if registry grows to hundreds of sims |
| A5 | `@setFloatMode` in Zig 0.16.0 accepts `.strict` as an enum value | Pattern 7 | ADR-deal-stdlib-numeric-model.md documents this as an assumption (A-001); Zig 0.16.0 is confirmed installed |
| A6 | Python `build` package (PyPI) is the correct PEP 517 frontend for wheel building | Standard Stack | Confirmed on PyPI; standard PyPA tooling since 2021; very low risk |
| A7 | Evidence baselines should live at `evidence/baselines/<tag>/` at repo root (tracked), not inside `.deal/` (gitignored) | Pattern 3 / Pitfall 3 | Consistent with D-81; the exact path is Claude's discretion — planner may adjust |

---

## Open Questions (RESOLVED)

> All three resolved at plan time. RESOLVED markers below cite the plan that locked each decision.

1. **In-process Zig boundary: single libdeal.a vs. separate object** — **RESOLVED (05-04 Task 1):** add the canonical Zig sim to the existing `libdeal` step in `build.zig` with a compile-time flag; no second link target.
   - What we know: `deal_sim.zig` must be compiled and callable from Rust; Phase 5 ships one canonical Zig sim.
   - What's unclear: Whether the Zig sim function should be compiled into the existing `libdeal.a` (requires `build.zig` awareness of sim files) or compiled as a separate `.a` that the CLI links additionally.
   - Recommendation: Add the canonical Zig sim to `build.zig` as an additional source in the existing `libdeal` step with a compile-time flag; simpler than a second link target and follows the existing lib pattern.

2. **`deal evidence` as a top-level command or subcommand of `deal`** — **RESOLVED (05-01 Task 2):** nested `Command::Evidence { subcommand: EvidenceCommand }` with `Capture` and `Baseline { tag }` variants.
   - What we know: D-81/D-82 describe `deal evidence capture` and `deal evidence baseline v2.1.0`.
   - What's unclear: Whether `evidence` is a `Command::Evidence { subcommand: EvidenceSubcommand }` with `capture` and `baseline` as nested variants, or separate `Command::EvidenceCapture` / `Command::EvidenceBaseline`.
   - Recommendation: Nested subcommand (`deal evidence capture`, `deal evidence baseline <tag>`) following the pattern of how `deal build --target` is structured. Keeps the CLI surface clean.

3. **`model_path` resolution against IR — which file set to parse?** — **RESOLVED (05-03 Task 2):** run `deal_ir_json` over the project files for the full IR at orchestration time; do not rely on `.deal/index.json`.
   - What we know: Inputs like `EnergyStorage.battery.packResistance` are qualified paths into the model. The orchestrator needs their resolved numeric values.
   - What's unclear: Whether the orchestrator runs a full `deal check` parse to get the IR, or uses a cached `.deal/index.json`.
   - Recommendation: Run `deal_ir_json` over the project files to get the full IR at orchestration time (same as `deal build`), then resolve `model_path` strings against the IR elements map using D-23 path-string IDs. Do NOT depend on `.deal/index.json` being current.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Python 3.10+ | deal-sim SDK, Python sim runner | Yes | 3.14.5 | — |
| pip | deal-sim local install | Yes | 26.1.1 | — |
| Zig | deal_sim.zig, in-process path | Yes | 0.16.0 | — |
| Cargo | Rust CLI compilation | Yes | 1.93.0 | — |
| git | Fresh-worktree gate | Yes | 2.54.0 | — |
| MATLAB | motor_efficiency + vehicle_dynamics sims | Not verified | — | Graceful-skip + record per D-72 |
| `build` (Python) | deal-sim wheel build | Installed | 1.5.0 | — |

**Missing dependencies with no fallback:**
- None that block the hard gate. Python + Zig sims are the hard gate; MATLAB is graceful-skip.

**Missing dependencies with fallback:**
- MATLAB: graceful-skip per D-72. `deal check --simulations` (structural binding validation) runs without MATLAB. MATLAB-backed criteria report STALE in `deal check --verify`.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework (Zig) | Zig built-in test runner (`zig build test`) |
| Framework (Rust) | Cargo test (`cargo test --workspace`) |
| Framework (Python) | Python `unittest` / stdlib test runner |
| Config file | `build.zig` (Zig), `Cargo.toml` (Rust), `pyproject.toml` (Python) |
| Quick run command | `zig build test -Dtest-filter=sim && cargo test -p deal --lib simulate` |
| Full suite command | `zig build phase-5-gate && cargo test --workspace` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REQ-phase-5-1-deal-sim-sdk | `DealSimulation.cli()` reads `input.json`, validates, runs, writes `output.json` | unit (Python) | `python -m pytest deal-sim/tests/ -x` | No — Wave 0 |
| REQ-phase-5-1-deal-sim-sdk | Schema validation rejects wrong-type input | unit (Python) | `python -m pytest deal-sim/tests/test_validation.py -x` | No — Wave 0 |
| REQ-phase-5-2-orchestration | TOML parser reads showcase `deal.sims.toml` correctly | unit (Rust) | `cargo test -p deal simulate::tests::parse_registry` | No — Wave 0 |
| REQ-phase-5-2-orchestration | Topological sort produces correct order for showcase sims | unit (Rust) | `cargo test -p deal simulate::tests::dep_order` | No — Wave 0 |
| REQ-phase-5-2-orchestration | Staleness hash changes when source file changes | unit (Rust) | `cargo test -p deal simulate::tests::staleness_key` | No — Wave 0 |
| REQ-phase-5-2-orchestration | `deal simulate battery_thermal` runs Python sim end-to-end | integration | `cargo test -p deal --test simulate_integration` | No — Wave 0 |
| REQ-phase-5-3-evidence-verification | STALE reported when hash changes (D-84) | unit (Rust) | `cargo test -p deal verify::tests::stale_detection` | No — Wave 0 |
| REQ-phase-5-3-evidence-verification | PARTIAL verdict for REQ_BAT_002 gap block | unit (Rust) | `cargo test -p deal verify::tests::partial_verdict` | No — Wave 0 |
| REQ-phase-5-3-evidence-verification | PASS verdict for REQ_BAT_001 with design evidence | unit (Rust) | `cargo test -p deal verify::tests::pass_verdict` | No — Wave 0 |
| REQ-phase-5-3-evidence-verification | AND operator in criteria (REQ_BAT_003) | unit (Rust) | `cargo test -p deal verify::tests::and_criteria` | No — Wave 0 |
| D-88 (E2500 carryover) | `deal check` exits non-zero with E2500 for cross-file dimension mismatch | integration | `cargo test -p deal --test e2500_cli` | No — Wave 0 |
| REQ-phase-5-gate | `deal simulate --all` + `deal check --verify` full showcase green | gate | `zig build phase-5-gate` | No — Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p deal --lib` + `python -m pytest deal-sim/tests/ -x`
- **Per wave merge:** `cargo test --workspace && zig build test`
- **Phase gate:** `zig build phase-5-gate` (full suite green before `/gsd:verify-work`)

### Wave 0 Gaps

- [ ] `deal-sim/tests/test_simulation.py` — covers REQ-phase-5-1-deal-sim-sdk SDK contract
- [ ] `deal-sim/tests/test_validation.py` — covers SDK validation
- [ ] `cli/tests/simulate_integration.rs` — end-to-end Python sim execution
- [ ] `cli/tests/e2500_cli.rs` — D-88 cross-file E2500 CLI integration test
- [ ] `cli/src/simulate.rs` — new module (Wave 0 stub)
- [ ] `cli/src/evidence.rs` — new module (Wave 0 stub)
- [ ] `cli/src/verify.rs` — new module (Wave 0 stub)
- [ ] `spec/sims/v0/` — protocol schema directory (Wave 0 scaffolding)
- [ ] `build.zig` additions — `phase-5-gate` + `phase-5-gate-fresh` build steps
- [ ] Python build tool: `pip install build` — already installed (1.5.0)

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes | Stdlib-only dict type check (D-78); JSON schema validation via `jsonschema` crate |
| V6 Cryptography | yes (narrow) | SHA-256 for staleness keys — `sha2` crate; no custom hash |

### Known Threat Patterns for This Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Subprocess injection via `runner` field in TOML | Tampering | The `runner` value is treated as a command + args via `std::process::Command::new` (not shell-interpolated); CWE-78 path argument is quoted |
| Path traversal in `model_path` values | Tampering | D-23 qualified path IDs contain no `/` or `..`; validate against `^[a-zA-Z0-9._]+$` before filesystem operations |
| evidence cache poisoning (stale hash collision) | Repudiation | SHA-256 pre-image resistance sufficient for audit; frozen baseline manifest is git-tracked |
| sim source file arbitrary execution | Elevation of Privilege | D-84: `deal check --verify` never executes sim code silently; `--run-sims` is the explicit opt-in; no auto-execute on stale |
| Hallucinated package installation | Tampering | slopcheck run confirmed `deal-sim` does not exist on crates.io; Python `deal-sim` is a new package created from scratch (not installed from PyPI) |

---

## Sources

### Primary (HIGH confidence)

- `spec/examples/showcase/simulations/deal.sims.toml` — Registry oracle; all 5 sim entries examined directly
- `spec/examples/showcase/model/traceability.dealx` — Verification oracle; all 8 satisfy blocks examined; criteria grammar catalogued
- `spec/examples/showcase/simulations/thermal/battery_thermal.py` — SDK oracle; `DealSimulation` base class contract confirmed
- `spec/examples/showcase/packages/requirements/system.deal` — Requirement contracts; `verification {}` block structure confirmed
- `cli/src/main.rs` — Existing CLI pattern confirmed: clap subcommand structure, run_check, FFI pattern, D-32 envelope
- `src/sema.zig` lines 255–293 — `analyzeWithExternalTable` signature + "NOT exported to C ABI" note confirmed
- `tests/unit/sema_dimensional.zig` — E2500 regression harness; confirmed algebra is proven, C ABI gap confirmed
- `scripts/verify-fresh-worktree.sh` lines 165 — Sibling list confirmed; `deal-sim` not yet included
- `build.zig` — `phase-4-gate` / `phase-4-gate-fresh` precedent confirmed; Phase 5 extends these
- `Cargo.toml` — Workspace dependencies confirmed (clap 4.6, serde_json 1, toml, anyhow, jsonschema 0.46, sha2 absent)
- `.planning/phases/05-simulation-integration/05-CONTEXT.md` — All 19 locked decisions D-70..D-88 read
- `.planning/decisions/ADR-deal-stdlib-numeric-model.md` — Reproducibility tiers and `@setFloatMode` confirmed
- `spec/ir/v0/README.md` — ID strategy + metadata envelope conventions for `spec/sims/v0/` design

### Secondary (MEDIUM confidence)

- `deal-sim/README.md` — Architecture sketch (README-only repo); `src/deal_sim/` structure planned but not yet implemented
- `cargo search sha2 walkdir serde_json toml anyhow clap` — Package existence confirmed on crates.io 2026-06-08
- `pip index versions build setuptools wheel` — Python packaging tools confirmed on PyPI 2026-06-08
- `slopcheck install deal-sim serde_json toml anyhow clap` — slopcheck 0.6.1 run on 2026-06-08; `deal-sim` flagged SLOP on crates.io (correct — it's a Python package); others OK

### Tertiary (LOW confidence)

- MATLAB `jsondecode`/`jsonencode` harness convention — From training knowledge; confirmed available since R2016b; not verified against current MATLAB documentation

---

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — all Rust crates verified via `cargo search`; Python packages verified via `pip index versions`; all existing packages confirmed in workspace `Cargo.toml`
- Architecture: HIGH — directly derived from existing CLI patterns and codebase examination
- Pitfalls: HIGH — confirmed against actual code (analyzeWithExternalTable not in C ABI verified at line 259; fresh-worktree sibling list verified at line 165)
- Expression evaluator scope: HIGH — enumerated from direct examination of all 8 satisfy blocks in `traceability.dealx`

**Research date:** 2026-06-08
**Valid until:** 2026-07-08 (30 days; ecosystem stable)
