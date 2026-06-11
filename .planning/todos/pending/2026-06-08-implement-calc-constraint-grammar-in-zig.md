---
created: 2026-06-08T21:01:09.000Z
title: Implement calc / constraint defs (SD-21/22/23) in the Zig compiler
area: compiler
phase: 6
source: spec/grammar/2026-06-08-calc-constraint-grammar-design.md (locked design); spec commit adding SD-21/22/23
files:
  - src/lexer.zig
  - src/keywords.zig
  - src/ast.zig
  - src/parser_deal.zig
  - src/expr.zig
  - src/fmt.zig
  - src/sema.zig
  - src/diagnostics.zig
  - src/ir.zig
  - src/lowering.zig
  - src/json.zig
  - cli/src/sysml_v2.rs
  - tests/golden/
  - tests/unit/
---

## Problem

The grammar **spec** now defines first-class calculations and constraints
(SD-21 `calc def`, SD-22 `constraint def` + `require`, SD-23 `=>` return
contract + precision vocabulary) in `spec/grammar/{lexical,deal}.ebnf`, with
showcase files `examples/showcase/packages/analysis/{calcs,constraints}.deal`.

But the hand-written Zig compiler does **not** implement any of it. `deal parse`
on the new showcase files fails — there is no `calc`/`return`/`require`/`sig`
keyword, no `±` token, and no productions in the parser/AST for calc/constraint
bodies or the `=>` return contract. So the showcase files are currently
grammar-spec coverage only (README is annotated as such), and the in-language
compute surface from `DEAL-KerML-Coverage-Gap-Analysis.html` §4.1 cannot be used.

This is the implementation half of the brainstorm executed on 2026-06-08. The
design is locked; this todo is purely the Zig (and SysML-codegen) build-out.

### Explicitly OUT OF SCOPE (deferred)

- **Precision *enforcement*** — storage selection (f16–f128), error propagation,
  FP-determinism warnings. This is numeric-model **Lane A (N-01..N-04)**. Here we
  only **parse and carry** `PrecisionSpec`; sema records it on the IR node and
  does nothing else (the "parse now, enforce later" decision, SD-23 / D4).
- **`out`-parameter assignment body** — the statement-valued multi-output calc
  body (design §10 open micro-decision). `out`/`inout` stay accepted in the
  parameter list (KerML-fidelity hook), but a calc body remains
  `LocalBinding* ReturnStatement`; an `out`-param body is a clean parse error
  until that sub-decision lands.

## Solution

Build the feature down the existing pipeline (lexer → parser/AST → fmt/sema →
IR/lowering → codegen), one wave per layer, gated by the showcase files as the
parse/round-trip oracle. Suggested wave decomposition (each wave ≈ one GSD plan):

### Wave 1 — Lexer + keywords  (files: src/keywords.zig, src/lexer.zig)
- Add reserved words: `calc`, `return`, `require` (global); `sig` (contextual —
  only inside a `=>` return contract). Keep `src/keywords.zig` in sync with
  `spec/grammar/lexical.ebnf §5` (it is the cited source of truth).
- Add the `±` token (PLUS_MINUS, UTF-8 multibyte) and the `+/-` ASCII alias
  recognized in precision context; `deal fmt` normalizes `+/-` → `±` (Wave 3).
- **Must-have:** token-snapshot tests cover `calc def`, `return`, `require`,
  `sig`, `±`, `+/-`; existing token snapshots unchanged (no regression).

### Wave 2 — AST + parser  (files: src/ast.zig, src/parser_deal.zig, src/expr.zig)
- AST nodes: `CalcDefinition` (name, params, return_type, return_contract?, body),
  `ParameterDecl` (direction=in default, name, type, mult?), `ReturnStatement`,
  `ReturnContract` (list of `ContractItem`), `PrecisionSpec`, `ConstraintRef`;
  extend `ConstraintDefinition` (params?, body) + `RequireStatement`.
- Parser: calc signature → `ParameterList` → `TypeAnnotation` → optional `=>`
  `ReturnContract` list → `CalcBody`; constraint def with optional params + body
  of `require`/`LocalBinding`/annotations. Reuse `FunctionCallExpression` for
  calc/constraint invocation and `_ArgumentList` for `ConstraintRef`.
- **Disambiguation (FIRST sets per design §5):** after `=>`, `{` → satisfy return
  block (`.dealx` only); `sig`/`±`/IDENT → calc/constraint contract. After a
  constraint IDENT: `(` params, `<<` specialization, `{` body — disjoint.
- **Must-have:** AST-JSON golden for `analysis/calcs.deal` + `constraints.deal`;
  parse-twice determinism holds; the two showcase files `deal parse` exit 0.

### Wave 3 — Formatter + sema  (files: src/fmt.zig, src/sema.zig, src/diagnostics.zig)
- `deal fmt`: emit calc/constraint/return-contract; normalize `+/-`→`±` and drop a
  redundant leading `in`; parse→fmt→parse round-trip stable.
- Sema, new diagnostic range (reserve **E26xx** for calc/constraint):
  - **Purity rule (SD-21/D7):** a calc with only `in` params + one `return` is
    expression-valued; any `out` param makes it statement-valued. Error if an
    expression-position call targets a statement-valued calc.
  - Dimensional check: `return` expression vs declared return type; argument
    dimensions vs parameter types at call sites (reuse `sema_dimensional`).
  - `require`/`ConstraintRef` resolution; constraint calls type-check to Boolean.
  - **Precision: carry only** — attach `PrecisionSpec` to the node, no enforcement.
- **Must-have:** unit tests for a purity violation, a calc return dimensional
  mismatch, and a resolved constraint reference; `sema_dimensional` stays green.

### Wave 4 — IR + lowering + codegen  (files: src/ir.zig, src/lowering.zig, src/json.zig, cli/src/sysml_v2.rs)
- IR: add `NodeKind.calc_def` and `NodeKind.constraint_def`; carry params (with
  direction), return type, purity flag, `require` expressions, and a
  `precision{kind,value}` slot (per **ADR-deal-ir-v0** extension / numeric N-02 —
  values null until Lane A). Keep `spec/ir/v0/schema.json` in sync.
- Lowering: AST → IR for calc/constraint.
- Codegen (KerML mapping from design §7): `calc_def` → SysML/KerML **Function**
  (Calculation) with directed params + `result`; `constraint_def` → **Predicate**;
  `=>` precision contract → a **metadata feature** (preserved, not executed —
  lossless round-trip). Extend `cli/src/sysml_v2.rs` emit + golden fixtures.
- **Must-have:** IR-JSON + SysML-v2 golden fixtures for one calc and one
  constraint; existing Phase 2 SysML goldens unchanged for un-affected models.

### Wave 5 — Editor surfaces (parallel after Wave 2; lower priority)
- `tree-sitter-deal/grammar.js`: calc/constraint rules for highlighting.
- `vscode-deal/syntaxes/`: add `calc`/`return`/`require`/`sig` keywords.
- `lsp/src/`: semantic tokens + hover for the new keywords.

## Acceptance

1. `deal parse` exits 0 on `examples/showcase/packages/analysis/{calcs,constraints}.deal`.
2. `deal fmt` round-trips both files losslessly; `+/-` normalizes to `±`.
3. Sema: purity violation and calc-return dimensional mismatch each emit an E26xx
   diagnostic; a correct calc/constraint model checks clean.
4. One calc and one constraint produce stable SysML-v2 + IR-JSON golden fixtures.
5. No regression: the existing 19 `.deal`/`.dealx` showcase files and all Phase 2
   golden fixtures are unchanged; `phase-1.5-gate-fresh` style fresh-worktree gate
   stays green.
6. Precision enforcement and out-param bodies remain explicitly deferred (above),
   referenced from `ADR-deal-stdlib-numeric-model` and design §10 respectively.

## Scheduling note

Spans lexer→codegen like Phase 2, so it does not fit cleanly inside Phase 5 (sim)
or Phase 6 (app). Recommend either a dedicated inserted phase (e.g.
`05.x-language-calc-constraint`) or pairing Wave 3's precision-carry with
numeric-model **Lane A N-01/N-02**, since the `PrecisionSpec` production is the
shared seed. Decide at phase-planning time; this todo is the registration.

## ⚑ Spec submodule recall (MUST re-pin in Phase 6)

During Phase 5 execution (2026-06-08) the user advanced the `spec` submodule to the
calc/constraint grammar line, but it had to be **temporarily reverted** so Phase 5's
simulation gate could complete (the current Zig compiler cannot parse the new
showcase grammar, and that line dropped the `sims/v0` schemas Phase 5 depends on).

Exact commits to recall when this todo is executed:

| Ref | Repo | Commit | Meaning |
|-----|------|--------|---------|
| `7ce59bc` | `spec` submodule | `grammar: add calc/constraint defs + => return contract (SD-21/22/23)` | The grammar+showcase line this todo implements. **On `spec` origin/main.** |
| `d554d74` | `deal` (parent) | `spec: bump submodule to calc/constraint grammar (SD-21/22/23)` | The user's gitlink bump to `7ce59bc`. Superseded for Phase 5 but kept in history. |
| `1fb43ca` | `spec` submodule | `feat(05-01): add spec/sims/v0/ normative simulation protocol schemas` | Phase 5's prior pin: `fc533e9` (Phase-4 showcase) **+ `sims/v0`**. Superseded by `dd3bb78` (see below). |
| `dd3bb78` | `spec` submodule | `feat(showcase): add input values to value-less sim-input attributes` | **CURRENT PIN (05-08).** `1fb43ca` + literal values for packResistance/totalCurrent/flowRate/windingResistance/magnetTemp so `deal simulate` resolves sim inputs for real. This is the sims/v0 + showcase-input-values line. |

**Phase 6 action:** re-pin the `spec` submodule to a commit that is the **merge of
`7ce59bc` (calc/constraint) and `dd3bb78` (sims/v0 + showcase input values)** — i.e.
land `sims/v0` + the 05-08 input values onto the calc/constraint line (or rebase
`dd3bb78` onto `7ce59bc`) so both the Phase 5 simulation protocol and the Phase 6
grammar coexist. Then bump the parent gitlink (re-applying the intent of `d554d74`).
Until the Zig compiler implements calc/constraint (this todo), the gitlink must stay
at `dd3bb78`.

**Push note:** `dd3bb78` (and its ancestor `1fb43ca`) are local submodule commits
that may not be on the `spec` remote. Push them (or their `sims/v0` + showcase-input
content) to `spec` origin before any fresh clone / `phase-5-gate-fresh` run, or the
fresh-worktree submodule init will fail to resolve the gitlink.
