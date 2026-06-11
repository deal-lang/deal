# Implement deal-stdLib Physical Quantity Types Using Hybrid Sourcing with DEAL-Native Precision Annotation System

---
status: accepted
date: 2026-06-07
decision-makers: David Dunnock
consulted: >-
DEAL grammar-architect session (DESIGN-DECISIONS.md, 65 locked decisions);
SysML v2 stdlib (OMG); Modelica Standard Library (Modelica Association);
uom-rs (iliekturtles); IEEE 754-2008; ISQ/VIM (BIPM); JCGM 100:2008 (GUM)
informed: DEAL language contributors
---

## Context and Problem Statement

DEAL (Digital Engineering Authoring Language) requires a standard library
(`deal-stdLib`) that provides physical quantity types for MBSE modeling. The
library must enable engineers to specify numeric precision in natural terms
(significant figures, absolute tolerance, percentage tolerance) while the
compiler owns storage selection and hardware portability. No existing library
— SysML v2, Modelica, or uom-rs — provides this complete contract: each
contributes a piece but none provides the whole. A decision is needed now
because the `deal.std.units` namespace is already in active use across
showcase files, a bootstrap subset must be available at Stage 1, and the
numeric model underlies every subsequent library design choice.

## Decision Drivers

* **FA-4 (Self-explanatory syntax)** — engineers must not need IEEE 754
  knowledge to use the library; `sig 4`, `± mV(1)`, and `-> ± W(0.1)` must
  be the entire user-facing vocabulary
* **FA-2 (OO library system first-class)** — all quantity types must be real
  DEAL `attribute def` constructs importable and specializable by user models,
  not compiler magic built-ins
* **FA-3 (Dual-purpose defense/aerospace and software)** — core layer must be
  domain-neutral; domain packs must be independently installable
* **Engineering correctness** — return precision claims on derived attributes
  must be statically verifiable against declared input tolerances via error
  propagation; false precision must be a compile error
* **Cross-platform arithmetic reproducibility** — IEEE 754 strict mode is the
  default; FMA contraction, non-associative reduction, and auto-vectorization
  risks must be statically detectable with actionable guidance
* **DoD V&V auditability** — precision budgets must be explicitly declarable;
  `deal.toml` must support a strict mode that treats undeclared precision as
  an error, producing an auditable numeric contract for every interface
* **Performance transparency** — automatic storage upsizing must produce
  a warning with performance impact estimate; it must never be silent
* **Licensing** — all sourced material must be commercially usable without
  copyleft obligation; internal distribution must be unencumbered

## Considered Options

* **Option 1 — Adopt SysML v2 stdlib verbatim** — translate
  `Quantities.sysml` and `QuantityCalculations.sysml` directly to DEAL
  `attribute def` syntax; retain the `TensorQuantityValue →
  VectorQuantityValue → ScalarQuantityValue` hierarchy; translate `calc def`
  operator stubs
* **Option 2 — Adopt Modelica type catalog with DEAL arithmetic** — use
  Modelica's 270+ ISO 31-1992 type names, `quantity` strings, and `unit`
  strings as the catalog; implement arithmetic semantics natively in DEAL;
  Modelica provides the "what", DEAL provides the "how"
* **Option 3 — Port uom-rs ISQ dimension vector system to DEAL** — translate
  uom-rs's 7-integer exponent vector representation (`[M, L, T, I, Θ, N, J]`)
  into DEAL's type system; multiplication adds vectors, division subtracts;
  enforced by `deal check`
* **Option 4 — Build DEAL-native from scratch** — write all type definitions,
  dimension relationships, and numeric semantics from first principles;
  reference ISQ/VIM directly; no external sourcing
* **Option 5 — Hybrid sourcing with DEAL-native precision annotation system**
  — borrow the Modelica type name catalog (BSD-3) and the uom-rs dimension
  vector concept (Apache-2.0/MIT); build DEAL-native precision annotation
  syntax, compiler storage selection, error propagation verification, and
  reproducibility warning system; Zig comptime for simulation runtime

## Decision Outcome

Chosen option: **Option 5 — Hybrid sourcing with DEAL-native precision
annotation system**, because it combines the best available prior art for the
parts of the problem that are solved (type naming, dimension tracking) with
DEAL-native innovation for the parts that are not (engineer-facing precision
contracts, static error propagation, reproducibility warnings), while
satisfying all decision drivers and imposing no licensing constraints.

### Consequences

* Good, because engineers interact exclusively with engineering vocabulary —
  `sig 4`, `± mV(1)`, `-> ± W(0.1)`, `± e-6` — and never see IEEE 754
  storage types unless the compiler surfaces a warning
* Good, because the Modelica BSD-3 catalog provides 270+ ISO 31-1992 type
  names immediately, eliminating the risk of diverging from established
  international standards for quantity naming
* Good, because return precision (`-> ±`) on derived attributes creates
  statically verifiable numeric contracts at every interface boundary —
  a capability no existing MBSE tool provides, and directly applicable to
  DoD V&V audit requirements
* Good, because the three-tier reproducibility model (strict / upsize-trim /
  tolerant) gives simulation engineers a principled path to cross-platform
  determinism without mandating software floating-point emulation
* Good, because `deal.toml: precision = "strict"` mode enables programs to
  prove every numeric tolerance was explicitly declared — an auditable
  precision budget for CDRL-submitted models
* Bad, because the compiler must implement non-trivial static analysis:
  storage selection from precision declarations, error propagation through
  derived attribute chains, FMA/associativity pattern detection, and
  intermediate magnitude range checking — this is significant implementation
  work for the Stage 1 compiler
* Bad, because the `-> ±` return precision verification requires the compiler
  to perform symbolic error propagation at parse/check time; chains involving
  non-linear expressions (square root, trigonometric functions) require
  linearization approximations that may produce conservative (pessimistic)
  warnings
* Bad, because borrowing from both Modelica (BSD-3) and uom-rs (Apache-2.0)
  requires retaining two copyright notices in the stdlib source tree and
  accurately attributing which definitions were derived from which source —
  this is a documentation obligation, not a legal barrier, but must be
  maintained
* Neutral, because the Zig comptime runtime for simulation bindings is an
  assumption (A-001); if the DEAL compiler target changes, the storage
  selection and `@setFloatMode` mechanism must be re-evaluated, though the
  DEAL-layer annotation syntax is compiler-target-agnostic
* Neutral, because `deal.std.units` being available without an explicit import
  (proposed decision SL-3) is a separate locked decision; this ADR assumes
  SL-3 is accepted but does not depend on it — if rejected, users add
  `import deal.std.units.*;` to every file

### Confirmation

Implementation is confirmed correct when:

1. All 270+ Modelica-sourced type definitions compile in the DEAL parser
   without error and the copyright attribution header is present in each
   sourced file
2. `deal check` correctly selects `f16`/`f32`/`f64`/`f128` for representative
   declarations across the precision boundary values:
    - `sig 3` → `f32` (requires 10 bits, f32 provides 23) ✓
    - `sig 8` → `f64` (requires 27 bits, f32 provides 23) ✓
    - `± pm(0.1)` at `nm(1550)` → `f64` with `I-PREC001` informational ✓
3. `deal check` emits `E-PREC003` when declared return precision `-> ±`
   cannot be achieved given input tolerances, verified against the showcase
   `HVDCPort` example (Voltage ± V(1), Current ± A(0.1), Power -> ± W(1)
   should error — propagated uncertainty is ±361W at 280kW)
4. `deal check` emits `W-FP001` for three-operand compound expressions and
   `W-FP002` for reduction patterns, and both are suppressed by
   `@reproducibility: "tolerant"`
5. `deal check --precision=strict` errors on any attribute without an explicit
   precision declaration, and this mode is activated by
   `[check] precision = "strict"` in `deal.toml`
6. The Modelica BSD-3 and uom-rs Apache-2.0 copyright notices are present
   in `NOTICE.md` at the deal-stdlib repository root

## Pros and Cons of the Options

### Option 1 — Adopt SysML v2 stdlib verbatim

The `TensorQuantityValue → VectorQuantityValue → ScalarQuantityValue`
hierarchy is mathematically sound and ISQ-grounded. The `QuantityDimension`
/ `QuantityPowerFactor` model correctly represents base-quantity dimensional
analysis. The `assert constraint` pattern for `contravariantOrder +
covariantOrder == order` is directly translatable to DEAL constraints.

* Good, because the type hierarchy is peer-reviewed and ISQ-compliant
* Good, because MIT license imposes minimal obligations
* Good, because `isBound` / measurement reference constraint system provides
  a rigorous framework for distinguishing bound and unbound quantities
* Bad, because all `calc def` operators (`+`, `*`, `/`, `**`) are stubs —
  the multiplication rule `Voltage × Current = Power` is not enforced
  anywhere in the source; the compiler (Modelica tool vendor) holds the
  dimension logic internally
* Bad, because there is no unit constructor function syntax — the `[num, mRef]`
  constructor form violates FA-4 self-explanatory principle
* Bad, because there is no uncertainty model — the entire `Quantities` package
  has no field for measurement uncertainty, violating GUM compliance
* Bad, because there is no precision annotation system — nothing in SysML v2
  stdlib corresponds to `sig N`, `± abs`, or `-> ±` return guarantees

### Option 2 — Adopt Modelica type catalog with DEAL arithmetic

Modelica `Units.SI` types are defined as `type Voltage = Real(final
quantity="ElectricPotential", final unit="V")` — named, annotated `Real`
aliases covering all ISO 31-1992 quantities. The catalog is authoritative,
270+ types strong, and BSD-3 licensed. The translation to DEAL `attribute def`
is mechanical.

* Good, because the catalog is the most comprehensive available under a
  permissive license (BSD-3)
* Good, because ISO 31-1992 compliance is inherited from Modelica's
  authoritative source position
* Good, because `min=0` constraints on inherently non-negative quantities
  (Mass, Length, Frequency) translate directly to DEAL `@constraint`
* Good, because Modelica's `Conversions` sub-package provides the imperial
  unit conversion factor table
* Neutral, because Modelica's `displayUnit` pattern is not needed in DEAL
  (display is a tooling-layer concern)
* Bad, because Modelica provides only the catalog — arithmetic semantics,
  dimension equations, unit constructors, precision annotations, and
  reproducibility warnings must all be built in DEAL with no sourcing benefit

### Option 3 — Port uom-rs ISQ dimension vector system to DEAL

uom-rs represents each quantity's dimension as a 7-tuple of integers
`[M, L, T, I, Θ, N, J]` where each element is the power of the corresponding
SI base unit. Multiplication adds the tuples; division subtracts. This is
implemented via Rust's type system and procedural macros with zero runtime
overhead. The SI system ships pre-built and covers the ISQ completely.

* Good, because compile-time dimensional analysis is mathematically rigorous —
  `Voltage × Current = Power` is enforced by the type system, not asserted
* Good, because zero-cost abstraction means dimension checking has no runtime
  overhead
* Good, because the `no_std` feature enables embedded targets
* Good, because Apache-2.0/MIT dual license is commercially clean
* Bad, because direct porting of Rust macro-generated type algebra to DEAL
  attribute definitions produces non-self-explanatory constructs that violate
  FA-4 — users would see dimension vectors in error messages, not quantity
  names
* Bad, because uom-rs has no precision annotation system, no uncertainty
  model, and no return guarantee mechanism
* Bad, because uom-rs does not provide the Modelica-scale type name catalog —
  many engineering-domain types would need to be added

### Option 4 — Build DEAL-native from scratch

Write all type definitions referencing ISQ/VIM and BIPM SI Brochure directly.
No external sourcing. The type name catalog, dimension relationships, numeric
semantics, and precision annotations are all original DEAL work.

* Good, because no external copyright obligations of any kind
* Good, because the design is unconstrained by choices made in other languages
* Good, because DEAL-specific concerns (unit constructors, precision
  annotations, derived chain verification) can be designed from the ground up
  without translation artifacts
* Bad, because the ISO 31-1992 type name catalog is a solved, standardized
  problem — rebuilding it from the BIPM SI Brochure is pure undifferentiated
  effort with high risk of naming divergence
* Bad, because the dimension relationship table (which multiplications produce
  which result types) is large and error-prone to write from scratch — Torque
  vs Energy disambiguation alone requires careful design
* Bad, because this is the slowest path to a usable stdlib and delays the
  Stage 3 ecosystem milestone

### Option 5 — Hybrid sourcing with DEAL-native precision annotation system

Each external source contributes the component it does best. DEAL-native
innovation is focused on the components that do not exist elsewhere.

**What is borrowed and from where:**

| Component | Source | License | What DEAL adds |
|---|---|---|---|
| Type name catalog (270+ types) | Modelica MSL | BSD-3 | `attribute def` translation, `@dimension` ISQ formula |
| Dimension exponent vectors | uom-rs / ISQ | Apache-2/MIT | Named type disambiguation (Torque ≠ Energy) |
| Physical constants values | NIST CODATA 2022 | Public domain | `deal.std.constants` in DEAL syntax |
| Unit conversion factors | NIST SP 811 | Public domain | `deal.std.units.conversions` |
| Type hierarchy concept | SysML v2 stdlib | MIT | Flattened to `attribute def` per FA-4 |

**What is built natively in DEAL:**

| Component | Description |
|---|---|
| Unit constructor functions | `V(800)`, `kW(250)`, `degC(20)` — callable unit literals |
| `sig N` precision declaration | Significant figures → compiler selects storage |
| `± abs` precision declaration | Absolute tolerance → compiler selects storage |
| `± N%` precision declaration | Percentage tolerance → compiler selects storage |
| `-> ± precision` return guarantee | Verifiable output contract on derived attributes and actions |
| `± e-N` relative precision form | Scientific notation relative precision for wide-range quantities |
| Storage selection algorithm | `required_bits = ceil(sig × log₂(10))` → f16/f32/f64/f128 |
| Error propagation verification | Symbolic propagation through derived chains at check-time |
| W-FP001–W-FP006 warning system | FMA contraction, non-associativity, mixed-type, range, transcendental |
| `@reproducibility` annotation | `"strict"` / `"upsize-f128"` / `"kahan"` / `"tolerant"` tiers |
| `--precision=strict` mode | `deal.toml` flag; undeclared precision is an error |
| `@rangeWarn` threshold | Per-type magnitude ceiling where storage warning fires |
| Dimension equation table | Declared multiplications: `Voltage * Current = Power` |

**Storage selection mapping (compiler-internal, not user-visible):**

| Precision Declared | Required Mantissa Bits | Storage Selected |
|---|---|---|
| sig 1–3 / `± e-3` or coarser | ≤ 10 bits | `f16` |
| sig 4–7 / `± e-7` or coarser | ≤ 23 bits | `f32` |
| sig 8–15 / `± e-15` or coarser | ≤ 52 bits | `f64` |
| sig 16–33 / `± e-33` or coarser | ≤ 112 bits | `f128` |

* Good, because engineers use only engineering vocabulary — zero IEEE 754
  exposure in the happy path
* Good, because the Modelica catalog is authoritative, complete, and
  commercially licensed — no reinvention risk
* Good, because DEAL-native `-> ±` return precision provides statically
  verifiable numeric contracts that no other MBSE tool offers
* Good, because the three-tier reproducibility model is actionable —
  engineers are guided to a solution, not just warned of a problem
* Good, because `deal.toml` strict mode produces DoD-auditable precision
  budgets without imposing that burden on non-V&V workflows
* Good, because the upsize-then-trim strategy for FMA non-determinism
  (computed: upsizing to f128 absorbs the ≤1 ULP FMA difference for simple
  expressions) is a practical, zero-user-effort solution for the common case
* Neutral, because catastrophic cancellation cannot be resolved by upsizing
  and requires explicit user annotation (`@reproducibility: "tolerant"` or
  expression restructuring) — this is correct behavior, not a limitation
* Bad, because compiler implementation complexity is high: five distinct
  static analysis passes are required (storage selection, range checking,
  derived chain propagation, FMA pattern detection, reduction detection)
* Bad, because symbolic error propagation for non-linear derived expressions
  uses linearization (first-order Taylor expansion) which is conservative —
  some warnings may be pessimistic

## More Information

### Precision Annotation Syntax Reference

```deal
// Significant figures
attribute voltage : Voltage [1] = V(28.0) sig 4;

// Absolute tolerance
attribute busVoltage : Voltage [1] = V(28.0) ± mV(100);

// Percentage tolerance
attribute supplyVoltage : Voltage [1] = V(12.0) ± 5%;

// Relative precision (scientific notation)
attribute efficiency : Real [1] ± e-4;       // accurate to 1 part in 10,000

// Return precision guarantee on derived attribute
derived attribute maxPower : Power =
    voltage * maxCurrent -> ± W(0.1);

// Return precision on action
action def measureVoltage -> ± mV(1) { ... }

// Reproducibility tier annotation
derived attribute totalEnergy : Energy =
    sum(powerReadings) -> ± kWh(0.001) {
    @reproducibility: "kahan"
}
```

### Compiler Warning Reference

| Code | Trigger | Severity | Resolution |
|---|---|---|---|
| `I-PREC001` | Declared precision borderline for selected storage | Info | Compiler upsizes automatically |
| `W-PREC002` | Derived chain requires storage upsize from declared tier | Warning | Accept upsize or declare wider precision on input |
| `E-PREC003` | Return precision `-> ±` unachievable from input tolerances | Error | Relax return claim or tighten inputs |
| `W-RANGE001` | Value magnitude approaches storage exponent limit | Warning | Upsize storage or restructure value |
| `W-FP001` | Compound expression — FMA contraction risk | Warning | `@reproducibility: "upsize-f128"` or split expression |
| `W-FP002` | Accumulation / reduction — non-associativity risk | Warning | `@reproducibility: "kahan"` or `"tolerant"` |
| `W-FP003` | Derived chain of 3+ operands — intermediate evaluation order | Warning | Annotate chain root with `@reproducibility: "strict"` |
| `W-FP004` | `f16` compute on target without native f16 FPU | Warning | Use `f32` storage or accept software emulation |
| `W-FP005` | Mixed precision types in one expression | Warning | Explicit precision declaration on all inputs |
| `W-FP006` | Transcendental function (`sqrt`, `sin`, etc.) in expression | Warning | Linearization approximation used for error bounds |
| `E-PREC004` | `--precision=strict` mode: undeclared precision | Error | Add `sig N`, `± abs`, or `± N%` to attribute |

### Reproducibility Tier Reference

| Tier | Annotation | Behavior | Use Case |
|---|---|---|---|
| Strict | `@reproducibility: "strict"` or default | `@setFloatMode(.strict)`, FMA disabled | Simulation V&V baselines |
| Upsize-trim | `@reproducibility: "upsize-f128"` | f128 intermediate, trim to declared storage | Compound expressions with FMA risk |
| Kahan | `@reproducibility: "kahan"` | Compensated summation algorithm | Long reductions, running totals |
| Tolerant | `@reproducibility: "tolerant"` | All optimizations permitted | Real-time simulation, embedded, GPU |

### `deal.toml` Configuration

```toml
[check]
# Require explicit precision declaration on all attributes
# Undeclared precision is an error (E-PREC004)
precision = "strict"          # default: "default" (stdLib defaults apply)

# Reproducibility default for simulation bindings
reproducibility = "strict"    # default: "strict"

# Warn when storage is automatically upsized
warn-upsize = true            # default: true
```

### Sourcing Attribution Requirements

The following attribution must appear in `NOTICE.md` at the
`deal-stdlib` repository root:

```
deal-stdLib type name catalog derived in part from the Modelica Standard
Library, Copyright (c) 1998-2020, Modelica Association and contributors.
Used under the BSD 3-Clause License.
https://github.com/modelica/ModelicaStandardLibrary/blob/master/LICENSE

deal-stdLib dimension vector concept derived in part from uom-rs,
Copyright (c) iliekturtles contributors.
Used under the Apache License 2.0 / MIT License.
https://github.com/iliekturtles/uom

Physical constants sourced from NIST CODATA 2022.
Unit conversion factors sourced from NIST SP 811.
Both are in the public domain.
```

### Approved Assumptions

| ID | Assumption | Impact if Wrong |
|---|---|---|
| A-001 | DEAL compiler implemented in Zig | `@setFloatMode` mechanism and comptime constants must be re-evaluated for alternative compiler target; DEAL annotation syntax unchanged |
| A-002 | deal-stdLib ships as Stage 3; bootstrap subset at Stage 1 | Bootstrap subset scope must be explicitly defined before Stage 1 compiler work begins |

### Deferred Decisions

The following are out of scope for this ADR and require separate decisions:

* Exact scope of the Stage 1 bootstrap unit subset (which units ship
  with the compiler before deal-stdLib Stage 3)
* Whether `deal.std.units` is available without import (proposed SL-3) —
  this ADR assumes SL-3 accepted; if rejected, users must import explicitly
* Torque vs Energy semantic disambiguation implementation — named types
  sharing a dimension require a concrete disambiguation mechanism in the
  compiler's dimension equation table
* DEAL IR schema for carrying precision metadata through to code generation
  backends
* `deal simulate` orchestrator integration — how `@reproducibility` tier
  annotations map to backend simulation tool configuration

### Related Decisions

* DEAL Design Decision FA-2 — OO library system first-class
* DEAL Design Decision FA-3 — Dual-purpose defense/aerospace and software
* DEAL Design Decision FA-4 — Self-explanatory syntax principle
* DEAL Design Decision CS-4 — Physical/logical connection separation
* DEAL Design Decision CS-5 — `connection def` / `flow def` keywords
* Proposed DEAL stdlib decision SL-1 — unit constructors only (no bare literals)
* Proposed DEAL stdlib decision SL-3 — `deal.std.units` always available
* Proposed DEAL stdlib decision SL-7 — SI canonical, imperial opt-in

### Standards Compliance

| Standard | Relevance | Compliance |
|---|---|---|
| IEEE 754-2008 | Floating-point format and arithmetic semantics | Compliant — strict mode default, optimized mode opt-in |
| ISQ/VIM (BIPM) | International System of Quantities — dimension definitions | Compliant — dimension vectors per ISQ 7-base-quantity system |
| JCGM 100:2008 (GUM) | Uncertainty expression | Partial — `± abs` annotation consistent with GUM k=1 standard uncertainty; full GUM coverage is a future extension |
| ISO 31-1992 | Quantity and unit naming | Compliant — inherited from Modelica catalog sourcing |
| NIST CODATA 2022 | Physical constants values | Compliant — constants sourced directly |
| NIST SP 811 | Unit conversion factors | Compliant — conversions sourced directly |