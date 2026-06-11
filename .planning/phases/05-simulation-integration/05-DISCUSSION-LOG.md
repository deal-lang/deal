# Phase 5: Simulation Integration - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-08
**Phase:** 5-simulation-integration
**Areas discussed:** Carryover folding, Execution architecture & MATLAB, deal-sim SDK scope & distribution, Evidence cache & staleness, Verify evaluation engine

---

## Carryover Todo — cross-file dimensional E2500

| Option | Description | Selected |
|--------|-------------|----------|
| Fold into Phase 5 | Include cross-file E2500 CLI wiring as Phase 5 scope (Zig algebra already passes; pure analyzeWithExternalTable integration) | ✓ |
| Keep as standalone todo | Leave out of Phase 5 as a separate tracked item | |

**User's choice:** Fold into Phase 5 (D-88).
**Notes:** Verify correctness depends on imported units resolving across files anyway — same wiring.

---

## Execution architecture & MATLAB

### Who invokes each tool (execution architecture)
| Option | Description | Selected |
|--------|-------------|----------|
| Rust dispatches by `tool` | Orchestrator branches on registry `tool`; Python→entry script, MATLAB→`runner`; SDK Python-only | (basis) |
| Funnel all through Python SDK | Rust always calls python -m deal_sim; matlab.py adapter uses Engine API | |

**User's choice:** Opened a design conversation — wants to support Python, MATLAB, STK, **and Zig** sims; asked for the ideal architecture. Resolved to the **language-neutral JSON Simulation Protocol + adapter dispatch** model (D-70/D-71), confirmed "Yes — protocol is the kernel" after reading `ADR-deal-stdlib-numeric-model.md`.
**Notes:** User plans deal.std.* in Zig (precise storage f32/f64). Zig sims attractive for storage precision, no Python runtime, compiled speed, language dogfooding.

### MATLAB execution mechanism
| Option | Description | Selected |
|--------|-------------|----------|
| `matlab -batch` subprocess | Per showcase registry `runner`; Engine API deferred | |
| MATLAB Engine API for Python | Literal ROADMAP SC#2 | |
| Both via adapter, batch default | Abstract runner; subprocess default, Engine API pluggable | ✓ |

**User's choice:** Both via adapter, batch default (D-72).

### Gate behavior when MATLAB absent
| Option | Description | Selected |
|--------|-------------|----------|
| Graceful-skip + record | Phase 4 D-58 pattern; Python(+Zig) are the hard gate | ✓ |
| Mock/stub runner | Canned output.json | |
| Hard-require MATLAB | Gate fails without license | |

**User's choice:** Graceful-skip + record (D-72).

### Gate real-run scope
**User's choice:** "Python, MATLAB, and perhaps a Zig based one" — resolved to Python real + Zig real (in-process) + MATLAB structural/graceful-skip.

### Architecture confirmation (post-ADR-read)
| Option | Description | Selected |
|--------|-------------|----------|
| Protocol is the kernel | Rust dispatches by `tool`; one JSON contract; SDKs are conveniences | ✓ |
| Adopt with refinement | | |
| Python-SDK-centric | Route everything through Python SDK | |

**User's choice:** Yes — protocol is the kernel (D-70). Asked to verify (via ADR) whether Zig sims belong on the same kernel or get special handling → resolved by the hybrid below.

### Protocol as spec artifact
| Option | Description | Selected |
|--------|-------------|----------|
| Normative spec under spec/sims/v0/ | JSON Schemas + README + conventions, like spec/ir/v0/ | ✓ |
| Document in deal-sim README only | Prose only | |

**User's choice:** Normative spec under spec/sims/v0/ (D-70).

### Zig sims — how far in Phase 5
| Option | Description | Selected |
|--------|-------------|----------|
| Prove protocol w/ 1 Zig sim | One small Zig sim through generic/zig adapter | |
| Full deal_sim.zig SDK | Polished Zig convenience SDK parallel to Python | ✓ |
| Defer Zig, keep protocol open | No Zig sim in Phase 5 | |

**User's choice:** Full deal_sim.zig SDK (D-79/D-80).

### Numeric-model ADR scope in Phase 5
| Option | Description | Selected |
|--------|-------------|----------|
| Only deferred-decision #5 | @reproducibility→orchestrator mapping; full precision system stays separate | ✓ |
| Pull precision analysis into Phase 5 | Broader numeric static analysis | |
| Defer @reproducibility entirely | Ignore tiers for now | |

**User's choice:** Only deferred-decision #5 (D-75).

### External-sim reproducibility/precision
| Option | Description | Selected |
|--------|-------------|----------|
| Declare + validate output, record tier | Tier in evidence metadata + output-tolerance validation (advisory) | ✓ |
| Validate output only | No tier recording | |
| No precision handling for external | output.json only | |

**User's choice:** Declare + validate output + record tier (D-76).

### Zig hybrid (freeform — "Can it do both?")
| Option | Description | Selected |
|--------|-------------|----------|
| Uniform kernel + Zig enforcement | Zig through same path; deal_sim.zig injects @setFloatMode | (basis) |
| In-process bypass for Zig | Evaluate in deal check, no JSON | |
| Let me describe a hybrid | | ✓ |

**User's choice:** Hybrid — "have DEAL, when using Zig based sims, run in-process checks for immediate fast results via CLI/GUI **and** ensure you get the artifacts." Resolved to **one execution, two outputs** (D-73): in-process fast result + same-run artifact serialization. Confirmed "Yes — lock it."
**Notes:** External sims stay subprocess+JSON. Reframed the kernel as a *data contract*, transport-agnostic. Bit-reproducibility (strict float mode) makes the fast path authoritative.

### Interactive artifact-write policy
| Option | Description | Selected |
|--------|-------------|----------|
| Transient live, persist on capture | Live runs cache transiently; durable artifacts on explicit capture/gate | ✓ |
| Always persist artifacts | Every run writes to evidence cache | |
| You decide at plan time | | |

**User's choice:** Transient live, persist on capture (D-74).

---

## deal-sim SDK scope & distribution

### Distribution
| Option | Description | Selected |
|--------|-------------|----------|
| Build + local/editable, defer PyPI | Local install for orchestrator+gate; PyPI a later release act | ✓ |
| Publish to PyPI in Phase 5 | Public release moment | |
| TestPyPI first | | |

**User's choice:** Build + local/editable, defer PyPI (D-77).

### Validation implementation
| Option | Description | Selected |
|--------|-------------|----------|
| Stdlib-only validator | Zero required deps, airgap-clean | ✓ |
| pydantic | Required third-party dep | |
| jsonschema | One dep, reuses protocol schemas | |

**User's choice:** Stdlib-only validator (D-78).

### Adapter scope
| Option | Description | Selected |
|--------|-------------|----------|
| Python + MATLAB + generic + Zig | Defer STK + FMI/FMU | ✓ |
| Python + MATLAB + Zig only | No generic adapter | |
| Everything in the README | STK + FMI/FMU too | |

**User's choice:** Python + MATLAB + generic + Zig (D-79).

### deal_sim.zig home
| Option | Description | Selected |
|--------|-------------|----------|
| In the deal repo, built by deal toolchain | Coupled to @setFloatMode/in-process path | ✓ |
| In the deal-sim sibling repo | All SDKs together | |
| You decide at plan time | | |

**User's choice:** In the deal repo, built by deal toolchain (D-80).

---

## Evidence cache, baseline & staleness

### Evidence location
| Option | Description | Selected |
|--------|-------------|----------|
| Working cache in .deal/, baselines tracked | Gitignored working cache (PS-10); tracked baseline snapshots | ✓ |
| All in .deal/, baseline = git tag | | |
| All committed | | |

**User's choice:** Working cache in .deal/, baselines tracked (D-81).

### Baseline artifact
| Option | Description | Selected |
|--------|-------------|----------|
| Frozen snapshot + manifest | output.json set + hashes/versions/tier/verdicts | ✓ |
| Git tag only | | |
| Manifest only (hashes/verdicts) | No raw payloads | |

**User's choice:** Frozen snapshot + manifest (D-82).

### Staleness key
| Option | Description | Selected |
|--------|-------------|----------|
| Content hash: inputs + sim source + registry | Deterministic; catches model + sim-logic changes | ✓ |
| Input-values hash only | Misses sim-code edits | |
| File mtime | Fragile, non-reproducible | |

**User's choice:** Content hash: inputs + sim source + registry (D-83).

### Stale-evidence verdict behavior
| Option | Description | Selected |
|--------|-------------|----------|
| Report STALE, don't auto-run | Opt-in re-run via --run-sims; verify never silently runs code | ✓ |
| Auto-run stale sims | | |
| You decide at plan time | | |

**User's choice:** Report STALE, don't auto-run (D-84).

---

## Verify evaluation engine

### Where verify evaluates
| Option | Description | Selected |
|--------|-------------|----------|
| Rust orchestrates, Zig owns dimensions | Rust loads IR+output.json; unit compat via cross-file C ABI (same as E2500 wiring) | ✓ |
| All in Rust | Re-implements dimension algebra | |
| All in Zig sema | Chattier ABI for runtime values | |

**User's choice:** Rust orchestrates, Zig owns dimensions (D-85).

### Expression evaluator scope
| Option | Description | Selected |
|--------|-------------|----------|
| Showcase-required subset, rest deferred | D-55 precedent | (steer) |
| Full expression language | Resolves deferred expression ADR | |
| You decide at plan time | | ✓ |

**User's choice:** You decide at plan time — captured as Claude's discretion with a strong steer toward the showcase-required subset (defer general arithmetic).

### Level-2 verdict rubric
| Option | Description | Selected |
|--------|-------------|----------|
| PASS/FAIL/PARTIAL + orthogonal STALE | STALE distinct from PARTIAL; margins from compute{} | ✓ |
| PASS/FAIL/PARTIAL only | Folds staleness into PARTIAL | |
| Let me refine the rubric | | |

**User's choice:** PASS/FAIL/PARTIAL + orthogonal STALE (D-86).

### PM traceability query delivery
| Option | Description | Selected |
|--------|-------------|----------|
| Structured deal check --verify report keyed by REQ | Per-requirement coverage; MQL deferred | ✓ |
| New deal query REQ_BAT_001 command | Early MQL surface | |
| You decide at plan time | | |

**User's choice:** Structured `deal check --verify` report keyed by REQ (D-87).

---

## Claude's Discretion

- Criteria/compute expression evaluator scope (steer: showcase-required subset, defer general arithmetic per D-55).
- Orchestration dependency-graph resolution (explicit `depends_on` vs inferred from input/output `model_path` overlap).
- `deal.sims.toml` schema extensions (e.g. `tool = "zig"`).
- MATLAB `.m` JSON harness convention.
- In-process Zig call mechanism (comptime vs linked routine).
- `spec/sims/v0/` schema shape.
- Plan slicing.

## Deferred Ideas

- STK adapter + FMI/FMU co-simulation → Phase 6.
- MATLAB Engine API adapter → future (subprocess `-batch` is the Phase 5 default).
- Full deal-stdlib numeric precision system (sig/±/->, E-PREC*/W-FP*) → separate stdlib/sema track.
- General DEAL expression semantics → ADR-deferred.
- MQL `deal query` language → ADR-deferred.
- PyPI publish of deal-sim → later release gate.
- GUI/LSP live-verify integration surface → Phase 6.
