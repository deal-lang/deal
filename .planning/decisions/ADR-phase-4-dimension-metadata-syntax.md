# ADR: Phase 4 Dimension/Unit Metadata Syntax

**Status:** LOCKED
**Date:** 2026-06-06
**Phase:** 04-ecosystem
**Relates to:** D-55, D-56, D-57, Plan 04-03 (stdlib authoring), Plan 04-04 (Zig sema)

---

## Context

D-56 requires dimensional knowledge declared in DEAL source as data the Zig sema reads — no
hardcoded compiler knowledge. The RESEARCH assumption A1 proposed
`@si_base_dimension[M:1, L:0, ...]` bracket-arglist annotation syntax. However, the locked
`spec/grammar/deal.ebnf` (`StandaloneAnnotation` production) only permits:

```
StandaloneAnnotation ::= ANNOTATION ( ":" AnnotationValue )? NL?
AnnotationValue ::= Expression
```

The `Expression` grammar (LogicalOrExpression → ... → PrimaryExpression) does **not** include
`[...]` array literal syntax. Therefore the bracket-arglist form is not grammar-legal.

Three options were evaluated and probe-tested with `deal parse`:

---

## Options Evaluated

### Option B — Standalone annotation with array-literal value (DISQUALIFIED)

```deal
@si_dim: [1, 0, 0, 0, 0, 0, 0]
```

**Verdict: Grammar-illegal.** `PrimaryExpression` does not include array literals. Running
`deal parse` on this form produces `E0100 unexpected token`. **DISQUALIFIED.**

### Option C — Category annotation with AnnotationBody

```deal
@dimension:<<si>> Mass { M: 1  L: 0 ... }
```

**Verdict: Grammar-legal in principle** (`CategoryAnnotation ::= ANNOTATION_PREFIX
DELIMITED_OPERATOR NamespacePath? AnnotationBody?`), but requires inventing a new
`<<si>>` semantic for an existing grammar production without clear meaning. This approach
is more complex and less readable than Option A. **Not selected.**

### Option A — Attribute-def body with integer member defaults (CHOSEN)

```deal
attribute def Mass {
    attribute si_M : Integer = 1;
    attribute si_L : Integer = 0;
    attribute si_T : Integer = 0;
    attribute si_I : Integer = 0;
    attribute si_TH : Integer = 0;
    attribute si_N : Integer = 0;
    attribute si_J : Integer = 0;
}
```

**Verdict: Grammar-legal, parser-verified.** Uses only
`AttributeDefinition ::= "attribute" "def" IDENT StructuralRelationship? DefinitionBody`
with `AttributeUsage` members carrying `DefaultValue` (integer or real literals). Negative
values parse as `UnaryExpression` (`"-"` prefix on integer literal).

---

## Decision: Option A — attribute-def body encoding

### Concrete encodings

#### SI base dimension

```deal
/** Mass — SI base dimension. Exponent vector: M=1 L=0 T=0 I=0 Theta=0 N=0 J=0 */
attribute def Mass {
    attribute si_M : Integer = 1;
    attribute si_L : Integer = 0;
    attribute si_T : Integer = 0;
    attribute si_I : Integer = 0;
    attribute si_TH : Integer = 0;
    attribute si_N : Integer = 0;
    attribute si_J : Integer = 0;
}
```

#### Derived dimension (negative exponents)

```deal
/** Force = M*L*T^-2. Exponent vector: M=1 L=1 T=-2 I=0 Theta=0 N=0 J=0 */
attribute def Force {
    attribute si_M : Integer = 1;
    attribute si_L : Integer = 1;
    attribute si_T : Integer = -2;
    attribute si_I : Integer = 0;
    attribute si_TH : Integer = 0;
    attribute si_N : Integer = 0;
    attribute si_J : Integer = 0;
}
```

#### SI base unit (specializes dimension, factor = 1.0)

```deal
/** kg — SI base unit for Mass. Factor: 1.0 (SI base unit, no conversion). */
attribute def kg <<specializes>> Mass {
    attribute si_factor : Real = 1.0;
}
```

#### Non-base SI unit (scaled prefix)

```deal
/** g — gram, 1/1000 of a kg. Factor: 0.001 (relative to SI base unit). */
attribute def g <<specializes>> Mass {
    attribute si_factor : Real = 0.001;
}
```

#### Imperial unit (with conversion factor)

```deal
/** lb — pound-mass. Factor: 0.453592 kg. */
attribute def lb <<specializes>> Mass {
    attribute si_factor : Real = 0.453592;
}
```

#### Explicit conversion call form (D-57)

```deal
/** to_kg — explicit conversion from a Mass quantity to kg-scale value (D-57).
 *  Usage: to_kg(lb(3300))
 *  The function form (SD-13/PS-6) is achieved via the attribute def being callable
 *  as a unit literal per the DEAL function-call convention. The sema reads si_factor
 *  to determine the conversion ratio.
 */
attribute def to_kg <<specializes>> Mass {
    attribute si_factor : Real = 1.0;
}
```

---

## Contract for Plan 04-04 (Zig sema)

The Zig sema's dimensional algebra (D-55/D-56) reads `attribute def` declarations from
the stdlib. The exact field names the sema MUST read are:

### For dimension definitions (no `<<specializes>>`):

| Field name | Type | Meaning |
|------------|------|---------|
| `si_M` | Integer | Mass exponent |
| `si_L` | Integer | Length exponent |
| `si_T` | Integer | Time exponent |
| `si_I` | Integer | Electric current exponent |
| `si_TH` | Integer | Thermodynamic temperature exponent |
| `si_N` | Integer | Amount of substance exponent |
| `si_J` | Integer | Luminous intensity exponent |

**Detection rule:** An `attribute def` is a dimension definition if it declares ALL SEVEN
`si_M..si_J` members. It has NO `<<specializes>>` operator, OR it specializes another
dimension def (which also has the 7 exponents — inheriting/overriding is user's choice).

### For unit definitions (`<<specializes>>` a dimension):

| Field name | Type | Meaning |
|------------|------|---------|
| `si_factor` | Real | Conversion factor to SI base unit (e.g., 1.0 for kg, 0.001 for g, 0.453592 for lb) |

**Detection rule:** An `attribute def` is a unit definition if it has `<<specializes>>`
pointing to a dimension def AND declares `si_factor`.

**Note on `to_<unit>` conversion forms:** These are unit defs with `si_factor = 1.0`
specializing the target dimension (e.g., `to_kg <<specializes>> Mass`). The sema recognizes
a call `to_kg(x)` where `x` has dimension Mass as an explicit conversion call (D-57),
producing a Mass value in kg-scale. The presence of `si_factor = 1.0` on `to_kg` tells
the sema that the output is in kg-normalized form.

### Sema algorithm (D-55/D-56)

1. Parse all stdlib unit/dimension defs into a symbol table
2. For each `attribute def` with all 7 `si_M..si_J` members → register as dimension with
   exponent vector `[si_M, si_L, si_T, si_I, si_TH, si_N, si_J]`
3. For each `attribute def <<specializes>> DimName` with `si_factor` member → register as
   unit with dimension ref + factor
4. At check time: evaluate unit expressions by combining dimension vectors per SD-13/D-55
   arithmetic rules; emit E2500 on mismatch, E2501 on mixed-unit same-dimension without
   explicit `to_<unit>()` call

---

## Verification that encoding parses

Run:
```bash
for f in ../deal-stdlib/packages/units/*.deal; do
  cargo run -q -p deal -- parse "$f" >/dev/null || { echo "PARSE FAIL: $f"; exit 1; }
done
echo "UNITS_PARSE_OK"
```

All files MUST exit 0.

---

## Alternatives Record

| Option | Status | Grammar reason |
|--------|--------|----------------|
| `@si_base_dimension[M:1, L:0, ...]` (array annotation) | DISQUALIFIED | `PrimaryExpression` has no `[...]` array literal; `deal parse` → E0100 |
| `@dimension:<<si>> Mass { M: 1 L: 0 ... }` (category body) | Not selected | Grammar-legal but requires inventing semantic meaning for `<<si>>`; Option A is simpler and more readable |
| `attribute def` body with `si_M..si_J` integer defaults | **CHOSEN** | Grammar-legal, probe-verified, supports negative exponents via unary negation |

---

*Locked by: Plan 04-03 executor, 2026-06-06*
*Source of truth for: Plan 04-04 Zig sema implementation (D-55/D-56)*
