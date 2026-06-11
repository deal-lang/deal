# ADR: Phase 5 MATLAB Subprocess Adapter (D-72)

**Status:** LOCKED
**Date:** 2026-06-08
**Phase:** 05-simulation-integration
**Relates to:** D-72, ROADMAP §Phase 5 SC#2, Plans 05-03, 05-04

---

## Context

ROADMAP §Phase 5 Success Criterion 2 (SC#2) states:

> "`deal simulate --all` discovers simulations via `deal.sims.toml`, resolves the dependency
> graph, runs them in order, and `deal simulate --stale` re-runs only simulations whose inputs
> changed (**MATLAB simulations execute via MATLAB Engine API for Python**)."

The showcase registry (`spec/examples/showcase/simulations/deal.sims.toml`) defines MATLAB
simulations (`motor_efficiency`, `vehicle_dynamics`) with:

```toml
[simulations.motor_efficiency]
tool = "matlab"
runner = "matlab -batch \"...\""
```

This uses the **subprocess** invocation model, not the MATLAB Engine API for Python.
A real tension existed between the ROADMAP's "Engine API" wording and the registry's
`matlab -batch` runner convention.

D-72 (Phase 5 CONTEXT.md decision log, entry 72) chose to resolve this tension explicitly.

---

## Decision

**MATLAB simulations in Phase 5 execute via the subprocess adapter (`matlab -batch`).**

The ROADMAP SC#2 wording "MATLAB Engine API for Python" is **superseded** by this decision.

The subprocess approach is correct for Phase 5 for three reasons:

1. **Registry-first design (D-70, D-71):** The `deal.sims.toml` registry is the normative
   contract. Each simulation declares a `tool` field and an optional `runner` command. The
   orchestrator dispatches by `tool`; for `tool = "matlab"`, it runs the `runner` command as a
   subprocess, captures exit code, and reads `output.json`. The registry already encodes
   `runner = "matlab -batch \"...\""`; changing to Engine API would require changing both the
   registry schema and the orchestrator dispatch path for zero benefit in Phase 5.

2. **Graceful-skip without Engine API (D-72 + D-58 pattern):** MATLAB is unavailable in CI
   and most dev environments. The subprocess path enables graceful-skip + record (same pattern
   as Phase 4 D-58 DOORS skip): `deal simulate` logs the skip reason without failing. Engine
   API requires Python bindings and a MATLAB license to import; subprocess just needs the
   `matlab` binary on PATH (checked at dispatch time, not import time).

3. **Tool-agnostic protocol (D-70):** The JSON data contract (`spec/sims/v0/`) makes MATLAB
   simulations interchangeable with Python and Zig sims. The transport (subprocess vs. Engine
   API) is an implementation detail of the adapter. Phase 5 ships the subprocess adapter;
   Engine API becomes a future pluggable adapter that conforms to the same protocol.

---

## MATLAB Engine API for Python — Deferred to a Future Phase

The MATLAB Engine API for Python (`matlab.engine`) allows calling MATLAB functions directly
from Python without spawning a MATLAB process. Advantages include:

- Faster startup for repeated sim calls (engine reuse across runs)
- Shared memory for large arrays (no serialization to JSON intermediary)
- Richer MATLAB-Python interop (handles, cell arrays)

These advantages are real but are not blocking for Phase 5's goal: prove that MATLAB sims
participate in the `deal simulate` execution graph and produce schema-valid evidence artifacts.

When Engine API becomes the preferred path (e.g. in a future real-time simulation phase), it
would be added as a second adapter under `tool = "matlab-engine"` without changing the existing
subprocess adapter or the JSON data contract.

---

## Impact on ROADMAP SC#2

SC#2 remains satisfied for Phase 5 with the following re-statement:

> "`deal simulate --all` discovers simulations via `deal.sims.toml`, resolves the dependency
> graph, runs them in order, and `deal simulate --stale` re-runs only simulations whose inputs
> changed. MATLAB simulations execute via the subprocess adapter (`matlab -batch`). Engine API
> is a deferred pluggable adapter for a future phase."

The `motor_efficiency` and `vehicle_dynamics` MATLAB sims graceful-skip when MATLAB is absent,
consistent with the Phase 4 D-58 DOORS pattern. This is not a regression: Phase 5's hard gate
is Python + Zig sims running for real; MATLAB is proven via structural binding validation.

---

## Traceability

| Decision | Source | Status |
|----------|--------|--------|
| D-70 | 05-CONTEXT.md | JSON data contract, language-neutral boundary |
| D-71 | 05-CONTEXT.md | Orchestrator dispatches by `tool` field |
| D-72 | 05-CONTEXT.md | MATLAB = subprocess `-batch` default; ADR records supersession |
| D-79 | 05-CONTEXT.md | STK/FMI-FMU adapters deferred to Phase 6 |
| SC#2  | ROADMAP.md | "Engine API" wording superseded by subprocess adapter |
