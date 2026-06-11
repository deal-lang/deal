---
phase: 04-ecosystem
plan: "03"
subsystem: stdlib
tags: [deal-stdlib, dimensional-algebra, si-units, imperial-units, metadata-encoding, adr]

# Dependency graph
requires:
  - phase: 04-ecosystem/04-01
    provides: E-code reservations (E2500..E2503) that sema will emit on dimension mismatch
  - phase: 04-ecosystem/04-02
    provides: DEFAULT_STDLIB_TAG placeholder (v0.1.0) in main.rs that this plan's chosen tag replaces

provides:
  - ADR-phase-4-dimension-metadata-syntax.md — grammar-legal encoding contract (attr-def-body, Option A)
  - deal-stdlib v0.4.0 units package: dimensions.deal, si.deal, imperial.deal, mod.deal
  - deal-stdlib deal.toml and README with Phase 6 deferred-package documentation
  - DEFAULT_STDLIB_TAG = "v0.4.0" (recorded for Plan 04-02 wiring)

affects: [04-04-sema, 04-05-reqif, 04-06-interfaces]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "attribute-def body encoding for dimensional metadata (7 si_M..si_J integer members per dimension)"
    - "<<specializes>> structural relationship to link units to dimensions with si_factor"
    - "to_<unit> attribute defs as explicit conversion call forms (D-57)"

key-files:
  created:
    - .planning/decisions/ADR-phase-4-dimension-metadata-syntax.md
    - ../deal-stdlib/deal.toml
    - ../deal-stdlib/README.md
    - ../deal-stdlib/packages/units/dimensions.deal
    - ../deal-stdlib/packages/units/si.deal
    - ../deal-stdlib/packages/units/imperial.deal
    - ../deal-stdlib/packages/units/mod.deal
  modified: []

key-decisions:
  - "Metadata encoding Option A (attr-def-body): attribute def with si_M..si_J integer member defaults; grammar-legal, probe-verified, negative exponents via unary minus"
  - "Option B (annotation array @si_dim: [1,0,0,...]) DISQUALIFIED: PrimaryExpression has no array literal; deal parse produces E0100"
  - "Option C (category annotation body) not selected: grammar-legal but requires inventing <<si>> semantic"
  - "DEFAULT_STDLIB_TAG = v0.4.0 (locksteps with Phase 4 toolchain, D-52 pairing precedent)"
  - "Sema detection rule: attribute def with all 7 si_M..si_J members = dimension; attribute def <<specializes>> Dim with si_factor = unit"
  - "percent declared under Mass (si_factor=0.01) pending a future Dimensionless dimension in Phase 5+"
  - "in_unit used instead of in for inch to avoid DEAL keyword collision"

patterns-established:
  - "Dimension pattern: attribute def Name { si_M: Integer = N; ...(x7) } — no <<specializes>>"
  - "Unit pattern: attribute def sym <<specializes>> DimName { si_factor: Real = F; }"
  - "Conversion pattern: attribute def to_<sym> <<specializes>> DimName { si_factor: Real = 1.0; }"
  - "Barrel export: mod.deal re-exports all public symbols from sibling .deal files"

requirements-completed: [REQ-phase-4-1-stdlib]

# Metrics
duration: ~45min (prior executor session)
completed: 2026-06-06
---

# Phase 04 Plan 03: Dimension Metadata Syntax ADR + deal-stdlib Units Package Summary

**Grammar-legal attribute-def-body encoding locked in ADR; deal-stdlib v0.4.0 units package (dimensions, SI, imperial, conversions) authored and parse-verified against the locked 0.1.0-draft grammar.**

## Performance

- **Duration:** ~45 min (prior executor session)
- **Started:** 2026-06-06T00:00:00Z
- **Completed:** 2026-06-06
- **Tasks:** 2 (Task 1: ADR decision checkpoint + commit; Task 2: deal-stdlib units package auto)
- **Files created:** 7 (ADR + 6 deal-stdlib files)
- **Files modified:** 0

## Accomplishments

- Locked the grammar-legal metadata encoding for dimensional algebra (D-56) — the contract Plan 04-04 Zig sema reads. Option A (attr-def-body) was chosen after probe-testing all three candidates; array-literal annotation (Option B) was disqualified by the locked grammar.
- Authored deal-stdlib v0.4.0 units package: 7 SI base dimensions + 8 derived dimensions (Force, Energy, Power, Voltage, Speed, Resistance, Charge, Duration), all SI base/derived units including every symbol the 19-file showcase imports (kg, kWh, V, A, kW, degC, ohm), imperial units with NIST SP 811 conversion factors, and 8 explicit D-57 conversion call forms (to_kg, to_m, to_s, to_N, to_J, to_W, to_K, to_m_per_s).
- All 4 deal-stdlib unit source files pass `deal parse` with exit 0 (parse-gate verified).
- deal-stdlib README documents Phase 6 deferred packages (rf/, protocols/, standards/, patterns/) per D-54.
- deal.toml sets version = "v0.4.0" — the DEFAULT_STDLIB_TAG for Plan 04-02's main.rs to wire.

## Task Commits

1. **Task 1: Lock the grammar-legal dimension/unit metadata encoding (ADR)** - `a9ad0a3` (docs) — deal repo
2. **Task 2: Author deal-stdlib units package** - `c8db5ef` (feat) — deal-stdlib repo

## Files Created/Modified

- `.planning/decisions/ADR-phase-4-dimension-metadata-syntax.md` — metadata encoding contract; records chosen encoding, field names (si_M..si_J, si_factor), sema detection rules, rejected options with grammar reasons
- `../deal-stdlib/deal.toml` — project manifest: name="deal-std", version="v0.4.0", schema="deal/0.1", workspace packages=["packages/*"]
- `../deal-stdlib/README.md` — ships package tree, usage examples, D-57 conversion examples, Phase 6 deferred-package list
- `../deal-stdlib/packages/units/dimensions.deal` — 7 SI base dimensions + Force/Energy/Power/Voltage/Speed/Resistance/Charge/Duration with 7-exponent ADR-encoded vectors
- `../deal-stdlib/packages/units/si.deal` — 40+ SI units including all showcase imports (kg, kWh, V, A, kW, degC, ohm)
- `../deal-stdlib/packages/units/imperial.deal` — lb, oz, slug, ft, in_unit, mile, yd, lbf, kip, BTU, hp, degF, mph, knot plus 8 D-57 conversion defs
- `../deal-stdlib/packages/units/mod.deal` — barrel export of all public unit and dimension symbols

## Decisions Made

**Option A (attr-def-body) selected as metadata encoding:**
The locked grammar's `StandaloneAnnotation ::= @keyword: Expression` does not include array literals in `Expression`. Probe-testing confirmed Option B (`@si_dim: [1,0,0,0,0,0,0]`) produces E0100. Option A uses `attribute def` with integer member defaults, which parses cleanly. Negative exponents (e.g., T=-2 in Force) work via unary minus on integer literals.

**DEFAULT_STDLIB_TAG = v0.4.0:**
Locksteps with the Phase 4 toolchain release per D-52 pairing precedent. Plan 04-02 wired DEFAULT_STDLIB_TAG = "v0.1.0" as a placeholder — the real tag is v0.4.0 (to be updated in main.rs when the stdlib is tagged).

**percent under Mass:**
No Dimensionless dimension exists in the grammar yet. `percent` is temporarily declared as specializing Mass with si_factor=0.01. Phase 5+ will introduce a Dimensionless dimension.

**in_unit for inch:**
`in` is a DEAL keyword; the inch unit attribute def is named `in_unit` to avoid a parse conflict.

## Deviations from Plan

None — plan executed exactly as written. Task 1 was a human-approved checkpoint decision (attr-def-body chosen and verified); Task 2 was fully automated.

## Issues Encountered

None — all 4 unit source files parsed cleanly on first attempt.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 04-04 (Zig dimensional sema) can begin immediately. The ADR provides the exact field names and detection rules the sema must read (si_M..si_J for dimensions, si_factor for units, <<specializes>> as the unit/dimension link).
- Plan 04-02 main.rs DEFAULT_STDLIB_TAG should be updated from "v0.1.0" to "v0.4.0" when deal-stdlib is tagged.
- The showcase battery.deal imports (kg, kWh, V, A, kW, degC, ohm) are all present in si.deal and exported via mod.deal.

---
*Phase: 04-ecosystem*
*Completed: 2026-06-06*
