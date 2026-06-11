# ADR: Portion Marking & Classification Model

**Status:** ACCEPTED
**Date:** 2026-06-06
**Phase:** Cross-cutting (lexical grammar + sema + composition)
**Relates to:** FS-2 (`@header` fields incl. `marking`), SD-3 (`<<…>>` delimited-operator precedent), SD-9 (visibility wrappers), CS-2 (composition tags), LM-2/SD-11 (comment lexing), the file-integrity `hash`/`modified` discussion

---

## Context

DEAL files carry a single file-level `marking` field in the `@header` block (e.g.
`marking: Unclassified`), authored by hand and sitting **above** the hashed region. This has
three problems for a digital-engineering authoring language used on controlled programs:

1. **The banner is unprotected.** The body hash covers everything below `@header`, so the
   `marking` field itself is not integrity-protected. A classification marking can be altered
   without breaking the hash.
2. **It is whole-file only.** Real classification practice is *portion marking*: each portion
   carries its own marking and the overall banner is the highest (the "high-water mark"). A
   single file-level label cannot express that a `connection def` is CUI while one of its
   attributes is U.
3. **Views cannot be checked.** A composition view (`.dealx` `[<expose>]`) can project a subset
   of a model that is legitimately *less* classified than the source — but nothing verifies that
   a view declared at a lower marking does not leak a higher-marked portion.

The problem this ADR settles: **how to represent classification markings inside DEAL source so
that (a) markings are integrity-protected, (b) per-portion granularity is supported, (c) the type
checker can verify view/expose downgrades, and (d) the entire feature is optional and
back-compatible — all without conflicting with the locked lexical grammar.**

This is the lattice-based information-flow problem (Denning 1976; Jif): label terms, derive
aggregate labels by lattice join, and reject downgrades unless explicit.

---

## Decision

Adopt an **optional, lattice-based portion-marking system** with four parts:

1. **A self-delimiting mark token `@(…)`** as the syntactic carrier.
2. **Derived banners** — file and view markings are computed by lattice join, not authored.
3. **An explicit, audited `declassify` construct** as the only legal downgrade path.
4. **A fail-closed optionality switch** (`marking-policy`) in `@header`.

### 1 · The mark token: `@(…)`

A portion mark is a single token the lexer scans from `@(` to the matching `)`, treating the
contents as **raw text** — exactly the pattern already used by `DELIMITED_OPERATOR`
(`"<<" OPERATOR_NAME ">>"`, SD-3).

```deal
@(U)                  // unclassified
@(CUI)                // controlled unclassified
@(CUI//SP-PROPIN)     // CUI + category
@(S//NF)              // classified + dissemination control
```

A mark is an optional prefix on any markable node, ahead of the doc comment and annotations:

```ebnf
Declaration  ::= PortionMark? DocComment? Annotation* ElementDef
PortionMark  ::= CLASSIFICATION_MARK          // the @(…) token; entirely optional
```

Markable nodes: `import`, `package`, every `def`, members inside `public( )` / `protected( )` /
`private( )`, and `.dealx` tags (`[<system>]`, `[<subsystem>]`, `[<expose>]`, instances).

### 2 · Lattice rollup, inheritance & aggregation

Markings form a **lattice**, not a `U < CUI < S` total order — CUI categories and classified
compartments are sets ordered by subset, with a defined join and top element. Every node has an
*effective* marking:

```
effective(node) = join( explicitMark(node) , rollup(children) )
```

* An **unmarked** node inherits — effective marking is the rollup of its children and, transitively,
  its enclosing scope.
* An **explicit** mark is a *floor*: it may sit above the children's join. This is how the
  **aggregation rule** is modeled — a set of U portions that is collectively CUI.
* A child may be lower than its parent; the parent banner is the join, hence always ≥ every child.

### 3 · Derived, protected banners

The `@header` `marking` field becomes **compiler-derived and verified**, not authored:

```deal
@header {
    path:            packages/interfaces/connections.deal
    schema:          deal/0.1
    created:         2026-05-16T08:00:00Z by "David Dunnock"
    modified:        2026-05-16T08:00:00Z by "David Dunnock"
    hash:            sha256:9f2c…        // over body, INCLUDING portion marks
    status:          draft
    marking:         CUI                 // DERIVED = join(all portions); compiler-written + verified
    bounds:          U .. CUI            // AUTHORED floor..ceiling (protected); error if exceeded
    marking-policy:  strict              // off | advisory | strict
}

@footer { marking: CUI }                // DERIVED bottom banner; must equal @header marking
```

**Integrity model — no need to hash the header.** Portion marks live in the hashed body. The
banner must equal `join(all portions)`; the footer must equal the banner. Therefore:

* Tamper a portion → the body `hash` breaks.
* Tamper the top or bottom banner → the derivation check breaks.

Banners are protected *transitively*. This also resolves the open item from the `hash`/`modified`
discussion: the previously unprotected `marking` field is now covered.

### 4 · Views & the downgrade check

In composition (`.dealx`), a view/expose may emit output below the source banner — a *downgrade*,
the locus of every leak. Under `strict`, a view may not emit a marking lower than the join of what
it touches.

```deal
// Clean — no declassification needed
@(U) [<expose battery.serialNo as="serial" />]    // serialNo is @(U) → OK
[<expose battery.cellChem as="chem" />]            // cellChem is @(CUI) → expose inherits CUI

// Rejected — silent downgrade
@(U) [<view PublicSpec>]
    [<expose battery.cellChem />]                  // cellChem is @(CUI)
[</view>]
// ▲ error: view @(U) < content rollup @(CUI)

// Allowed — explicit, audited declassification
@(U) [<view PublicSpec>]
    [<declassify battery.cellChem to="U"
        authority="DD-2026-014"
        reason="chemistry generalized to public class" />]
[</view>]
```

`declassify` is the **only** construct that may lower a marking. It is loud, greppable, and carries
authority + rationale for audit.

### 5 · Optionality — fail closed

Governed by `marking-policy` in `@header` (or its absence):

| Policy | Behaviour |
|--------|-----------|
| `off` / absent | No marks expected; current files valid as-is. Back-compatible default. |
| `advisory` | Marks parsed, banner derived, mismatches are **warnings**; downgrades warn. Migration mode. |
| `strict` | Every node resolves to a mark (unmarked **inherits**; unmarked top level = **error**), banner must match the join, `bounds` enforced, downgrades require `declassify`. |

**Rule:** when enabled, unmarked nodes default to *inherit*, never to U. "Optional + default to
nothing" is exactly how unmarked CUI ships — worse than no system. Fail closed.

### 6 · Library vs. primitive split

* **Ontology → `deal.std.classification` (library, org-overridable).** What `U`/`CUI`/`S`/compartments
  mean and how they order each other varies by authority (DoD / DOE / NATO / corporate) and changes
  over time.
* **Mechanism → language primitive.** Attaching marks, computing the join, deriving banners, and
  enforcing the view check are built in and non-bypassable. A security property you can route around
  is not one.

---

## Options Evaluated

Three carrier syntaxes were evaluated against the locked `spec/grammar/lexical.ebnf`.

### Option A — bare `(U)` portion marks (DISQUALIFIED)

```deal
(CUI) connection def HVDCCable { … }
```

**Verdict: Grammar-illegal / ambiguous.** `(` already opens visibility wrappers
(`public ( … )`, SD-9) and expression grouping. A leading `(` in declaration or member-prefix
position collides with these productions, and `//` inside a bare mark (`(CUI//NF)`) is consumed as a
`SINGLE_LINE_COMMENT` (SD-11), eating the rest of the line and unbalancing the paren. **DISQUALIFIED.**

### Option B — `@CUI` standalone annotation (NOT SELECTED)

```deal
@CUI connection def HVDCCable { … }
```

**Verdict: Grammar-legal but insufficient.** `@CUI` lexes cleanly as `ANNOTATION ::= "@" IDENT`.
However it cannot carry compartments: `@CUI//SP-PROPIN` lexes `@CUI` then `//SP-PROPIN` as a line
comment. Reusing the open `ANNOTATION` space also collides semantically with user annotations
(`@confidence`, `@assumes`, `@concerns`, `@rationale`). **NOT SELECTED.**

### Option C — `@(…)` self-delimiting mark token (CHOSEN)

```deal
@(CUI//SP-PROPIN) connection def HVDCCable { … }
```

**Verdict: Grammar-legal, collision-free.** The lexer scans `@(` → `)` as a single token; the
contents are raw, so `//` never starts a comment and `(` / `)` never enter expression parsing.
`@(` is currently **unreachable**: `ANNOTATION`/`ANNOTATION_PREFIX` require `@` followed by
`IDENT` (`IDENT_START ::= [a-zA-Z_]`), and `(` is not an identifier start — so claiming `@(` as a
new opener conflicts with nothing. It reuses the established self-delimiting-token mechanism (SD-3,
`<<…>>`) and rides DEAL's `@`-means-metadata convention. **CHOSEN.**

---

## Contract for downstream work

### Lexer (`lexical.ebnf` amendment)

Add one token, modeled on `DELIMITED_OPERATOR`:

```ebnf
CLASSIFICATION_MARK ::= "@(" MARK_BODY ")"
MARK_BODY           ::= ( AnyChar - ( ")" | NL ) )+
```

* Longest-match / opener precedence: `@(` → `CLASSIFICATION_MARK`; `@` + `IDENT` `:` →
  `ANNOTATION_PREFIX`; `@` + `IDENT` → `ANNOTATION`. No reordering of existing rules required since
  `@(` is presently unrecognized.
* `MARK_BODY` is raw text (no nested tokenization); the marking string is interpreted by sema
  against the active ontology.

### Parser (`deal.ebnf` / `dealx.ebnf` amendment)

* Add optional `PortionMark?` prefix to the declaration, member, and `.dealx` tag productions.
* `.dealx`: add the `declassify` tag (`to`, `authority`, `reason` attributes) and, if adopted,
  the `[<view>]` tag (see Open Questions).

### `@header` / sema

* `marking` becomes compiler-derived; sema computes `join(all portions)` and verifies the authored
  value (or writes it). Add `bounds` (floor..ceiling) and `marking-policy` (`off|advisory|strict`)
  contextual `@header` fields alongside the existing FS-2 set.
* `@footer { marking: … }` (pending Open Questions) is compiler-maintained and must equal the
  `@header` banner.
* Lattice operations (`join`, ordering, top) read the label set from `deal.std.classification`.
* Downgrade rule: for any view/expose, declared marking MUST be ≥ `join(exposed portions)` unless a
  `declassify` covers the gap. Emit a dedicated diagnostic (e.g. `E26xx leak: view marking below
  content rollup`).

---

## Open Questions

These are explicitly **unresolved** and deferred to a follow-up pass:

1. **`bounds` spelling.** `bounds: U .. CUI` (range) vs separate `floor:` / `ceiling:` fields. The
   `..` form risks visual overlap with `DOTDOT` multiplicity ranges; a two-field form is more
   explicit. *Undecided.*
2. **Bottom banner mechanism.** A real `@footer { … }` block mirroring `@header`, vs a lighter
   closing-banner line/annotation. `@footer` is symmetric and parser-simple but adds a second
   special block. *Undecided.*
3. **`declassify` scope.** `.dealx`-only (downgrades happen at composition boundaries), vs also
   permitted in `.deal` view defs if view definitions are later allowed in definition files.
   *Undecided — depends on whether `.deal` gains a view-def construct.*

---

## Verification

* **Collision analysis (done):** `@(` is unreachable under current `lexical.ebnf`
  (`ANNOTATION`/`ANNOTATION_PREFIX` both require `IDENT` after `@`; `IDENT_START = [a-zA-Z_]`).
  `@(…)` is self-delimiting, so `//` and `()` inside it cannot collide with `SINGLE_LINE_COMMENT`
  (SD-11) or the visibility/grouping parens (SD-9). The chosen form reuses the `<<…>>` token shape
  (SD-3).
* **Negative probes (testable against today's parser):** the DISQUALIFIED bare-`(U)` form and the
  compartmented `@CUI//…` form fail under the current grammar, confirming the rejection rationale.
* **Pending implementation probe:** once the lexer amendment lands, add a parse fixture set
  (mirroring `deal/tests/`) covering: bare mark, compartmented mark, mark on each markable node,
  unmarked-inherits, banner-derivation check, `bounds` violation, and the view-downgrade
  error/`declassify` accept paths. All must `deal parse` / check at exit 0 (positive) or emit the
  expected diagnostic (negative).

---

## Alternatives Record

| Option | Status | Grammar reason |
|--------|--------|----------------|
| Bare `(U)` portion marks | DISQUALIFIED | Collides with `public( )` / grouping parens (SD-9); `//` inside becomes a line comment (SD-11) |
| `@CUI` standalone annotation | Not selected | Legal as `ANNOTATION`, but cannot carry `//` compartments and overloads the user-annotation namespace |
| `@(…)` self-delimiting mark token | **CHOSEN** | `@(` unreachable today; self-delimiting like `<<…>>` (SD-3); compartments + parens safe inside |
| File-level `marking` only (status quo) | Superseded | Whole-file granularity; banner unprotected; no view check |

---

*Accepted 2026-06-06. Core mechanism (`@(…)` token, lattice rollup, derived/protected banners,
`declassify`, fail-closed optionality) is locked. Three sub-decisions remain open (see Open
Questions). Source of truth for: `lexical.ebnf` / `deal.ebnf` / `dealx.ebnf` marking amendments,
`deal.std.classification`, and the sema view-downgrade check.*
