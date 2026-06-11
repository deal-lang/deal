# Phase 5: Simulation Integration - Context

**Gathered:** 2026-06-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 5 makes the showcase model's verification chain **executable end-to-end**. Three deliverables across the `deal` repo and the `deal-sim` sibling repo:

1. **`deal-sim` Python SDK** (sibling repo, currently README-only) — `DealSimulation` base class with typed `inputs`/`outputs`, a CLI runner doing JSON I/O + schema validation + metadata. Plus a **`deal_sim.zig` SDK** in the `deal` repo for Zig-native simulations.
2. **Orchestration** (Rust CLI) — `deal.sims.toml` registry parser + a `tool`-dispatching orchestrator: `deal simulate <name>` / `--all` / `--stale`, dependency-order execution, content-hash staleness. MATLAB via subprocess adapter; generic subprocess adapter for any tool.
3. **Evidence & verification** — `deal evidence capture` / `baseline v2.1.0`; `deal check --verify` performing SIM-5 three-level verification (structural completeness → criteria evaluation with PASS/FAIL/PARTIAL + margins → evidence freshness) against the captured evidence; a per-requirement coverage report answering "is REQ_BAT_001 verified?".

**The language surface is already built** (Phase 1/2 parse + lower `verification_block`, `satisfy_block`, `criteria`, `evidence`, `compute`, `traceability_block`, `validate` to IR JSON). Phase 5 **consumes** this — it does not redefine it. The showcase already ships `simulations/deal.sims.toml` (3 Python + 2 MATLAB sims), working `deal_sim`-importing Python sims, and the full `traceability.dealx`.

**The phase does NOT ship:** STK adapter + FMI/FMU co-simulation (Phase 6); MATLAB Engine API adapter (subprocess `-batch` is the Phase 5 path); the full `deal-stdlib` numeric precision static-analysis system (sig/±/->, storage selection, E-PREC*/W-FP* — separate stdlib/sema track); general DEAL expression semantics beyond what verification blocks require; an MQL `deal query` language; PyPI publish of `deal-sim` (build + local install only).

</domain>

<decisions>
## Implementation Decisions

> Decisions are numbered D-70 onward, continuing the project-wide D-01..D-69 sequence (Phases 1–4).

### Execution architecture — the JSON Simulation Protocol is the kernel (SIM-1)

- **D-70:** **The "kernel" is a language-neutral JSON *data contract*, not a file-based-subprocess transport.** `spec/sims/v0/` becomes a normative spec artifact (input.json / output.json / metadata JSON Schemas + exit-code conventions + `.deal/sims/<name>/` layout), mirroring `spec/ir/v0/`. Every simulation target (Python, MATLAB, Zig, generic) conforms to this contract; conformance = emitting schema-valid artifacts. Equivalent to Phase 1's C ABI: a language-neutral boundary that keeps the orchestrator tool-agnostic. The Python `deal_sim` package is a *convenience library*, not special.
- **D-71:** **The Rust orchestrator dispatches by the registry `tool` field.** `tool = "python"` → run the entry script (SDK `cli()` handles JSON I/O); `tool = "matlab"` → run the registry `runner` (`matlab -batch`) over a thin .m JSON harness; `tool = "zig"` → in-process call (see D-73); `tool = "generic"`/other → run the registry `runner` command following the protocol. New tools are new adapters / registry entries, never compiler changes.
- **D-72:** **MATLAB = subprocess adapter, `matlab -batch` default (both-via-adapter).** Reconciles ROADMAP SC#2 ("Engine API for Python") against the showcase registry's `runner = "matlab -batch \"…\""`. A runner-interface abstraction ships with the subprocess `-batch` default; Engine API is a future pluggable adapter. **Record an ADR** noting ROADMAP SC#2's Engine-API wording is superseded for Phase 5. When MATLAB is unlicensed/absent (the CI/dev common case), **graceful-skip + record** the attempt + reason (Phase 4 D-58 DOORS pattern). The hard gate is Python + Zig sims running for real; MATLAB proven via structural `deal check --simulations` binding validation.
- **D-73:** **Zig-internal sims run in-process AND emit artifacts — one execution, two outputs.** Because DEAL compiles/links Zig sims (via `deal_sim.zig`), the orchestrator calls them **in-process** for immediate CLI/GUI results, then serializes the *same run's* in-memory input/output/metadata to schema-valid evidence artifacts. The fast result and the audited artifact are the same computed values — they cannot diverge. External sims (Python/MATLAB/STK) stay subprocess+JSON (only transport available to foreign runtimes). Default `@reproducibility: "strict"` (`@setFloatMode(.strict)`) makes the in-process fast path **bit-reproducible and authoritative**.
- **D-74:** **Interactive artifact-write policy = transient live, persist on capture.** Hot-path editor/GUI `deal check --verify` re-runs compute in-process and cache results **transiently** (no per-keystroke disk churn). Durable evidence artifacts are written only on explicit `deal simulate` / `deal evidence capture` / phase gate — keeping the editor snappy and the audit trail intentional.

### Numeric-model ADR scope in Phase 5 (narrow slice only)

- **D-75:** **Phase 5 implements only deferred-decision #5 of `ADR-deal-stdlib-numeric-model.md`** — mapping `@reproducibility` tier → the simulate orchestrator. Concretely: (a) Zig-internal sims inherit the model's `@reproducibility` tier as a real `@setFloatMode`/storage selection at compile time; (b) the declared tier is recorded in evidence metadata for all sims. The **full** precision system (sig N / ± / -> ± annotations, storage-selection passes, `E-PREC*`/`W-FP*` codes, range checking, error propagation) is **out of Phase 5** — it stays on the separate stdlib/sema track (deal-stdlib "Stage 3").
- **D-76:** **External sims: declare + validate output + record tier (advisory enforcement).** For Python/MATLAB/STK, DEAL cannot control float mode. It records the declared `@reproducibility` tier + precision in evidence metadata (auditor sees the claim) and validates `output.json` values against declared tolerance/units. DEAL is honest that reproducibility enforcement for external tools is the tool's own responsibility. Only Zig-internal sims get *binding* enforcement (D-73/D-75).

### `deal-sim` SDK scope & distribution (REQ-phase-5-1)

- **D-77:** **Build + local/editable install; defer PyPI publish.** The wheel is built and the orchestrator + gate install it locally (editable/vendored) so `deal simulate` works end-to-end **without a network** — airgap-friendly, consistent with Phase 4 D-67/D-68's "no registry publish yet" posture. Actual PyPI publish is a separate release act, gated later (resolves part of the "first public release timing" deferred item when chosen).
- **D-78:** **Stdlib-only schema validator.** Input/output validation against the declared `inputs`/`outputs` dicts uses **Python stdlib only** — zero required third-party deps, matching the `deal-sim` README contract and the defense airgap posture. `numpy`/`scipy` remain *optional* for sim authors' own math, never required by the SDK core.
- **D-79:** **Adapters shipped in Phase 5: Python (first-class) + MATLAB (subprocess) + generic subprocess + `deal_sim.zig`.** STK and FMI/FMU adapters (sketched in the README) are **deferred to Phase 6** — same scope-trim posture as Phase 4 D-54. Update `deal-sim/README.md` to mark deferred adapters as planned.
- **D-80:** **`deal_sim.zig` lives in the `deal` repo, built by the deal toolchain.** It is tightly coupled to the compiler's `@setFloatMode`/storage machinery and the Rust orchestrator's in-process call path (D-73), so it is co-located with the toolchain and versioned in lockstep (e.g. `src/sim/` or a Zig module the CLI build links). The Python SDK stays in the `deal-sim` sibling repo.

### Evidence cache, baseline & staleness (REQ-phase-5-3)

- **D-81:** **Working evidence cache in `.deal/evidence/` (gitignored per PS-10); baselines tracked.** Transient/working runs live in gitignored `.deal/` (PS-10 already designates it the "simulation I/O cache" — no VCS churn). `deal evidence baseline v2.1.0` writes a durable snapshot to a **tracked** path (e.g. `evidence/baselines/v2.1.0/`) so the V&V reference is in version control and citable. Clean split: churn vs. audit.
- **D-82:** **`deal evidence baseline` produces a frozen snapshot + manifest.** Self-contained: the captured `output.json` set + a manifest recording per-sim content hashes, tool versions, declared `@reproducibility` tier, and the PASS/FAIL/PARTIAL verdicts at capture time. Optionally also git-tagged. Readable for cold CDRL/audit without re-running anything.
- **D-83:** **Staleness = content hash of {resolved input values, sim source file, `deal.sims.toml` entry}.** A sim is stale iff that hash differs from the captured evidence's recorded hash. Deterministic (D-18 discipline), reproducible, defense-auditable — catches both model-input changes *and* sim-logic changes (input-only hashing would miss the latter). `mtime` explicitly rejected (fragile across clone/checkout, non-reproducible). This is also the SIM-5 Level-3 freshness mechanism.
- **D-84:** **Stale evidence → report STALE, don't auto-run.** When `deal check --verify` finds the staleness hash changed, it reports the criterion as **STALE** (distinct from PASS/FAIL/PARTIAL) and exits non-zero in gate mode. Re-running is opt-in via `deal check --verify --run-sims`. Verification never silently executes simulation code — predictable, reproducible V&V.

### `deal check --verify` evaluation engine (REQ-phase-5-3)

- **D-85:** **Rust orchestrates; Zig owns dimensions.** Rust loads IR criteria/compute + `output.json`, resolves `evidence` maps, drives freshness + the report (runtime-artifact orchestration = the CLI's job). **Unit/dimension compatibility + conversions reuse the Zig dimensional engine via the cross-file C ABI (`analyzeWithExternalTable`)** — a single source of dimensional truth, and the *same* wiring the folded E2500 carryover (D-88) requires. No second dimension engine in Rust.
- **D-86:** **Level-2 verdict rubric (SIM-5): PASS / FAIL / PARTIAL + orthogonal STALE.** PASS = all criteria true AND evidence complete AND fresh. FAIL = a criterion evaluates false. PARTIAL = explicit `status="partial"`, or unmapped/missing return fields, or a declared `gap{}` block. STALE (D-84) is a separate freshness flag layered on top. Margins surfaced from `compute{}` values.
- **D-87:** **The PM query (SC#5) is delivered as a structured `deal check --verify` report keyed by requirement.** For each `REQ_*`: satisfaction status, supporting evidence, margins, stale flags — human-readable + JSON. Answers "is REQ_BAT_001 verified?" by ID. A dedicated MQL-style `deal query` surface stays ADR-deferred.

### Claude's Discretion

The following are intentionally NOT locked — researcher and planner have latitude:

- **Criteria/compute expression evaluator scope** (user said "you decide at plan time"). **Strong steer:** implement exactly what the verification blocks + showcase require — comparisons (`>=`, `<=`, `==`), boolean `AND`/`OR`/`NOT`, arithmetic (`+`, `-`, `*`, `/`), field-path refs (`REQ_SYS_001.minRange`, `EnergyStorage.battery.usableCapacity`), stdlib calls (`max`/`min`), unit literals, percent — and **defer general expression semantics** per the D-55 precedent. Document the supported grammar subset.
- **Orchestration dependency-graph resolution** — how `deal simulate --all` derives execution order (explicit `depends_on` in `deal.sims.toml` vs. inferred from input/output `model_path` overlap between sims). Not discussed; planner's call against the showcase registry.
- **`deal.sims.toml` schema extensions** — any new fields needed (e.g. `tool = "zig"`, reproducibility tier overrides, `runner` for generic) beyond the showcase's existing shape.
- **MATLAB `.m` JSON harness convention** — the thin wrapper that lets a `matlab -batch` script read `input.json` / write `output.json`.
- **In-process Zig call mechanism** — comptime-known function vs. statically/dynamically linked routine; the exact Rust↔Zig in-process boundary for D-73.
- **`spec/sims/v0/` schema shape** — field names, metadata envelope, exit-code table (follow `spec/ir/v0/` conventions and D-18 determinism).
- **Plan slicing** — likely parallelizable tracks: (Python SDK + protocol spec) ∥ (Rust orchestrator + adapters) ∥ (`deal_sim.zig` + in-process path); then (evidence cache + baseline) → (`deal check --verify` engine + E2500 wiring) → (gate). Final slicing is the planner's call.

### Folded Todos

- **Wire cross-file dimensional E2500 into `deal check` CLI** (`2026-06-08-wire-cross-file-dimensional-e2500-into-deal-check-cli.md`, folded). Phase 4 SC-1 was human-accepted with a deferred gap: `deal check` does not emit `error[E2500]` for cross-file dimension mismatches end-to-end via the CLI, because the single-file `deal_parse` path registers imported units as `.imported` (not `unit_def`), so `checkCallDimension` gracefully skips. The Zig algebra itself is proven (`sema_dimensional.zig`, all 4 E25xx pins pass). **Fits Phase 5 directly:** D-85's verify engine needs cross-file, dimension-aware operand comparison (criteria compare imported-unit-typed values), and the fix is the same `analyzeWithExternalTable` C ABI wiring. Acceptance: the 04-HUMAN-UAT Test 2 scenario (`import deal.std.units.{kg, V}` then `attribute mass : Mass = V(800);`) makes `deal check` exit non-zero with `error[E2500]` via the CLI, plus a CLI-level integration test.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project authority
- `.planning/PROJECT.md` — Tech stack constraints (Python `deal-sim` on PyPI; Zig core; Rust CLI/orchestration). Locked decisions: **SIM-1..SIM-5** (JSON I/O, SDK shape, `deal.sims.toml` registry, CLI command surface, three-level verification), **SD-20** (requirement verification contract), **CS-9..CS-16** (traceability/satisfy/criteria/evidence/compute/validate/method-type-checking), **PS-8/PS-10** (`simulations/` + `deal.sims.toml` layout; `.deal/` gitignored sim I/O cache), **FA-5** (simulation integration first-class), **FA-1** (IR is the kernel — D-70 mirrors this for sims).
- `.planning/REQUIREMENTS.md` §Phase 5 — REQ-phase-5-simulation, REQ-phase-5-1-deal-sim-sdk, REQ-phase-5-2-orchestration, REQ-phase-5-3-evidence-verification, REQ-phase-5-gate.
- `.planning/ROADMAP.md` §"Phase 5: Simulation Integration" — Goal + 5 success criteria. Note SC#2's "MATLAB Engine API for Python" is superseded by D-72 (subprocess `-batch` default; ADR to record).
- `.planning/STATE.md` — Phase 4 complete 2026-06-06 (9/9 plans).

### Numeric model (Phase 5 slice = deferred-decision #5)
- `.planning/decisions/ADR-deal-stdlib-numeric-model.md` — **Binding for D-75/D-76.** Reproducibility tiers (strict/upsize-f128/kahan/tolerant) → Zig `@setFloatMode`/storage. Phase 5 implements ONLY its deferred-decision #5 ("`deal simulate` orchestrator integration — how `@reproducibility` tier annotations map to backend simulation tool configuration"). Assumption A-001 (DEAL compiler in Zig) underpins the in-process Zig path. The full precision static-analysis system is NOT Phase 5.

### Inherited locked decisions (Phases 0–4)
- `spec/grammar/DESIGN-DECISIONS.md` — Consolidated ADR (64 LOCKED + 1 CAPTURED). Phase 5 leans on SIM-1..SIM-5, SD-20, CS-9..CS-16, PS-8/PS-10, FA-1/FA-5.
- `.planning/phases/02-prove-the-pipeline/02-CONTEXT.md` — D-22 (`deal_ir_json` transport — verify engine reads it), D-23 (path-string IDs), D-26 (IR traversal API), D-32/D-33/D-34 (diagnostic envelope, E-code bands, exit codes — E2500 band, new sim/verify codes extend these).
- `.planning/phases/04-ecosystem/04-CONTEXT.md` — D-55/D-56/D-57 (dimensional algebra in Zig sema, 7-exponent vectors, explicit conversions) — the engine D-85 reuses; D-66 (`.deal/deps/` vendoring); the `analyzeWithExternalTable` cross-file seeding the E2500 carryover (D-88) and verify engine need.
- `.planning/decisions/ADR-deal-ir-v0.md` + `spec/ir/v0/schema.json` + `spec/ir/v0/README.md` — Normative IR v0; verify engine walks this shape; `spec/sims/v0/` (D-70) mirrors its conventions.
- `.planning/decisions/ADR-phase-1.5-fresh-worktree-verification.md` — **Binding.** Phase 5 MUST add `phase-5-gate` + `phase-5-gate-fresh`; `zig build phase-5-gate-fresh` must exit 0 in an ephemeral worktree. The gate spans the `deal-sim` sibling + Python/Zig sim execution — extend `scripts/verify-fresh-worktree.sh` accordingly.

### Grammar & integration oracle
- `spec/grammar/{lexical,deal,dealx}.ebnf` — LOCKED `0.1.0-draft`. Verification-block / satisfy / criteria / evidence / compute / validate grammar already covered.
- `spec/examples/showcase/simulations/deal.sims.toml` — **The registry oracle.** 5 sims (3 Python: battery_thermal, motor_thermal, range_model; 2 MATLAB: motor_efficiency, vehicle_dynamics). Defines the registry schema the orchestrator parses (`tool`, `entry`, `class`, `runner`, `binds_to`, `annotation`, `inputs`/`outputs`, `auto_run`).
- `spec/examples/showcase/model/traceability.dealx` — **The verification oracle.** `[<satisfy>]` blocks with `criteria`/`evidence simulation|test|design|analysis`/`compute`/`gap`, plus `[<validate>]`. `deal check --verify` must evaluate these. Note the REQ_BAT_002 PARTIAL case (status="partial" + gap block) exercises D-86.
- `spec/examples/showcase/packages/requirements/system.deal` — `requirement def` + `verification { accepts/rejects/threshold/operator/conditions }` contracts (SD-20). Method type-checking (CS-16) against satisfy `method=`.
- `spec/examples/showcase/simulations/thermal/battery_thermal.py` — Reference Python sim: `from deal_sim import DealSimulation`, typed `inputs`/`outputs`, `run(self, inputs) -> dict`, `Class.cli()`. The SDK must make this exact file run.

### SDK repo (Phase 5 implementation target)
- `deal-sim/README.md` — Sibling repo (README-only). Sketches `src/deal_sim/{simulation,cli,validation,metadata}.py` + `adapters/{matlab,stk,generic}.py`, Python 3.10+, stdlib-only core. D-79 trims STK/FMI-FMU; update README to mark deferred.

### Implementation precedents (this repo)
- `cli/src/main.rs` — `clap` subcommand surface; Phase 5 adds `Simulate`, `Evidence`, and `--verify`/`--simulations`/`--run-sims`/`--stale` flags on `check`/`simulate`. Note the existing `run_check` comment ("the current C ABI processes each file independently via deal_parse") — the E2500 carryover (D-88) replaces this with `analyzeWithExternalTable`.
- `cli/src/sysml_v2.rs` / `cli/src/reqif.rs` — IR-JSON-walk emitter precedent; the verify report generator follows the same shape.
- `src/sema.zig` + `src/diagnostics.zig` — Dimensional algebra (Check #7, E2500..E2503) the verify engine reuses; new sim/verify E-code band extends the documented bands. `analyzeWithExternalTable` entry point.
- `sema_dimensional.zig` — Existing green harness exercising the E25xx algebra with stdlib seeding — reference for the E2500 CLI wiring (D-88); the algebra is already proven there.
- `src/{ast,json,lowering}.zig` — Already lower `verification_block`/`satisfy_block`/`criteria`/`evidence`/`compute`/`traceability_block` to IR JSON — the verify engine's input.
- `build.zig` — `phase-4-gate`/`phase-4-gate-fresh` precedents for `phase-5-gate(-fresh)`.
- `scripts/verify-fresh-worktree.sh` — Reuse/extend for the Phase 5 fresh-worktree gate (now spans `deal-sim` + sim execution).

### External references — researcher to fetch
- **MATLAB `-batch` invocation + headless JSON I/O conventions** — for the D-72 subprocess adapter + `.m` harness.
- **Python packaging (`pyproject.toml`, editable/local install, wheel build)** — for D-77 build-and-local-install (no PyPI publish).
- **Zig `@setFloatMode` + float storage semantics** — for D-73/D-75 reproducibility enforcement and the in-process call path.
- **Content-addressed caching / hashing conventions** — for D-83 deterministic staleness keys.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Language surface already parsed + lowered** — `verification_block`/`satisfy_block`/`criteria`/`evidence`/`compute`/`traceability_block`/`validate` already produce IR JSON (`src/json.zig`, `src/ast.zig`). Phase 5 *consumes* this; no new grammar/parser work.
- **Zig dimensional engine (D-55/D-56, E2500..E2503)** — `src/sema.zig` Check #7 + `sema_dimensional.zig` harness; the verify engine (D-85) and the E2500 carryover (D-88) both reuse it via `analyzeWithExternalTable`.
- **`cli/src/*` emitter + schema-validation pattern** — `sysml_v2.rs`/`reqif.rs` (IR walk → output) and `schema_registry.rs`/`reqif_schema.rs` (offline schema validation, SHA256 verification) are the precedent for the sims protocol validator + verify report.
- **Showcase simulation corpus is complete** — `deal.sims.toml` + 5 sim files + `traceability.dealx` + requirement contracts already exist as the oracle; the work is making them *run*, not authoring them.

### Established Patterns
- **Zig owns sema/dimensional/float-mode; Rust owns CLI/codegen/orchestration; Python is a convenience SDK** — D-71/D-80/D-85 keep this split. The in-process Zig path (D-73) is the one new cross-language seam.
- **D-18 deterministic output** — staleness hashes (D-83), evidence manifests (D-82), and any new JSON artifacts follow byte-stable/alphabetical-key discipline.
- **E-code bands documented in `diagnostics.zig`** — new sim/verify codes allocated alongside the existing dimensional band.
- **Phase gates `phase-N-gate` + `phase-N-gate-fresh`** — Phase 5's gate spans the `deal-sim` sibling + real Python/Zig sim execution; MATLAB graceful-skip when unlicensed.
- **Graceful-skip + record for unavailable external tools** — Phase 4 D-58 (DOORS) is the template for D-72 (MATLAB).

### Integration Points
- **Orchestrator → SDK (Python)**: Rust spawns `python entry.py`; SDK `cli()` reads `input.json` / writes `output.json` + metadata.
- **Orchestrator → Zig (in-process)**: Rust calls the DEAL-compiled `deal_sim.zig` routine directly, then serializes the result to evidence artifacts (D-73). New seam.
- **Orchestrator → MATLAB/generic**: Rust runs the registry `runner` subprocess over a JSON harness (D-72).
- **Verify engine → IR + evidence**: Rust reads `deal_ir_json` (criteria/compute/evidence-maps/thresholds) + cached `output.json`, calls Zig for unit/dimension compatibility (D-85).
- **Model attributes → sim inputs**: `deal.sims.toml` `inputs[].model_path` resolves against the model's attribute values to build `input.json` (cross-file resolution — same dependency the E2500 wiring closes).

</code_context>

<specifics>
## Specific Ideas

- **"Can it do both?"** — the Zig hybrid (D-73) is explicitly *one execution, two outputs*: fast in-process result for CLI/GUI **and** the same run's evidence artifacts. Not a compromise — the artifact is a byproduct of the fast run, so they can't diverge.
- **Zig as a first-class sim language** — user's motivation: precise f32/f64 storage, no Python runtime dep, compiled speed, and dogfooding the same language as `deal-std` + the compiler. A Zig sim is the strongest proof the protocol is genuinely language-neutral.
- **Defense-audience posture throughout** — airgap-friendly (no-network local install D-77, stdlib-only validator D-78), deterministic/auditable (content-hash staleness D-83, frozen baseline manifests D-82), and "verify never silently runs code" (D-84). Mirrors Phase 4's explicit-conversions-only stance.
- **The protocol as a spec artifact** (`spec/sims/v0/`, D-70) is the durable deliverable that makes multi-language real — treat it like the IR v0 lock.
- **MATLAB tension is real and recorded** — ROADMAP SC#2 says Engine API; showcase registry says `matlab -batch`. D-72 chooses subprocess + ADR; don't silently "fix" the registry to Engine API.

</specifics>

<deferred>
## Deferred Ideas

- **STK adapter + FMI/FMU co-simulation** — sketched in `deal-sim/README.md`; Phase 6 (heavy AGI/Ansys STK + FMI deps). Protocol (D-70) admits them without rework.
- **MATLAB Engine API for Python adapter** — future pluggable adapter; subprocess `-batch` is the Phase 5 default (D-72).
- **Full `deal-stdlib` numeric precision system** — sig N / ± / -> ± annotations, storage selection (f16/f32/f64/f128), error propagation, `E-PREC*`/`W-FP*` warning families. Separate stdlib/sema track ("deal-stdlib Stage 3"); Phase 5 takes only the `@reproducibility`→orchestrator slice (D-75).
- **General DEAL expression semantics** — arithmetic/comparison/logical beyond what verification blocks need; remains ADR-deferred (Phase 5 implements only the showcase-required subset).
- **MQL `deal query` language** — the PM query is delivered via the `deal check --verify` per-REQ report (D-87); a dedicated query language stays ADR-deferred.
- **PyPI publish of `deal-sim`** — Phase 5 builds + installs locally (D-77); production PyPI publish is a later release gate (ties to the "first public release timing" deferred item).
- **GUI/LSP live-verify integration surface** — the transient-live policy (D-74) anticipates it, but the desktop editor itself is Phase 6.

### Reviewed Todos (not folded)
None — the single matching todo (cross-file E2500) was folded (D-88).

</deferred>

---

*Phase: 5-simulation-integration*
*Context gathered: 2026-06-08*
*Sources: PROJECT.md, REQUIREMENTS.md §Phase 5, ROADMAP.md, STATE.md, 04-CONTEXT.md, ADR-deal-stdlib-numeric-model.md, ADR-deal-ir-v0.md, ADR-phase-1.5-fresh-worktree-verification.md, showcase simulations/ + traceability.dealx + requirements, deal-sim/README.md, cli/src/{main,sysml_v2}.rs, src/{sema,ast,json,lowering}.zig, sema_dimensional.zig*
