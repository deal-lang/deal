---
created: 2026-06-08T22:46:32.000Z
title: Reconcile EBNF↔parser drift & harden silent-tolerance (audit issues I1–I6)
area: compiler
phase: 6
source: DEAL-Grammar-Drift-Remediation-Analysis.html (2026-06-08 audit + lexer/parser source-trace)
files:
  # spec repo (EBNF + decisions)
  - spec/grammar/deal.ebnf
  - spec/grammar/dealx.ebnf
  - spec/grammar/dealview.ebnf
  - spec/grammar/lexical.ebnf
  - spec/grammar/DESIGN-DECISIONS.md
  # deal repo (parser/lexer/fmt + tests)
  - src/parser_deal.zig
  - src/parser_dealx.zig
  - src/expr.zig
  - src/ast.zig
  - src/fmt.zig
  - src/diagnostics.zig
  - tests/golden/
  - tests/unit/
---

## Problem

A 2026-06-08 audit of the EBNF grammar against the `spec/examples/showcase/*`
corpus found six issues. Tracing them against the **actual** hand-written Zig
lexer/parser (file:line below) inverted the framing: **the parser already
accepts every showcase form — it is consistently more permissive than the
EBNF.** So the drift direction is *EBNF stricter than parser/corpus*, and the
showcase is the trustworthy oracle.

But the parser earns that permissiveness through **silent-tolerance behaviors**
that mask drift and conflict with the Phase 1.5 parser-strictness contract:

- **I1 — annotation-body separator (3-way drift).** EBNF: newline. Parser: only
  `,` is an intentional separator; a `;` is *silently discarded as an unknown
  token* (`parser_deal.zig:1914-1923`). Showcase: uses `;`
  (`battery.deal:40-43`, `@trace` bodies in `vehicle.dealx`).
- **I2 — keyword-as-field-key.** EBNF: key is `IDENT` only. Parser: accepts
  `ident OR any keyword` (`parser_deal.zig:1908`; `isKeyword` incl. `kw_from`
  `:2211`). Showcase: `@flow … { from: … }` (`behaviors.deal:29`). Lexer is flat
  reserved — `from`→`kw_from` always (`keywords.zig:102`), no contextual machinery.
- **I3 — verification field `;`.** EBNF: `;` required. Parser: `;` optional
  (`parser_deal.zig:1966`); also *silently drops* unknown verification keys
  (`:1956-1959`). Showcase: `conditions: { … }` with no trailing `;`
  (`system.deal:107`).
- **I4 — array literals (real structural debt).** EBNF: no array in
  `PrimaryExpression`. Parser: shared expr parser **rejects** `[` and returns a
  bogus identifier node (`expr.zig:201,278`); arrays handled by **two duplicated**
  parsers — `parser_dealx.parseArrayLiteral` (`:809`) and
  `parser_deal.parseVerificationArray` (`:1994`) — both emitting a synthetic
  `call(__array__, items)` AST. Showcase: `messageIds: [...]` (`vehicle.dealx:64`),
  `ambient: [degC(25),...]` (`system.deal`).
- **I5 — `gap { }` opaque vs structured.** EBNF: `GapBlock`/`GapField` productions
  imply structured fields. Parser: body is a brace-balanced **byte scan** stored
  as one opaque `_body` string (`parser_dealx.zig:1230-1247`) — no field parsing,
  separators unchecked. The spec claims structure the parser does not deliver.
- **I6 — stale showcase citations.** Every `Showcase example:` line ref across the
  four EBNF files is off (files grew headers/doc-comments). Includes a
  self-introduced error: `RequireStatement` cites `constraints.deal:L13`, actual
  **L17**.

(I7 — `calc`/`constraint`/`±` unimplemented — is the separate calc/constraint
build-out todo, not drift; see cross-reference below.)

## Cross-cutting theme (most important)

I1, I3, and I4 are all "accept by not noticing" — discarded `;`, dropped unknown
keys, bogus node for stray `[`. Phase 1.5 explicitly hardened the parser against
silent acceptance of malformed input, so these are **regressions against that
contract**. Remediation is not merely syncing the EBNF; it is converting silent
tolerance into either an *explicit, tested accept* or an *explicit diagnostic*.

## Out of scope

- Empirical confirmation that the trace is correct is **Step 0**, not a fix:
  run `deal parse` over the whole showcase; expectation is all existing files
  pass and only the two `analysis/*.deal` (calc/constraint, I7) fail. If anything
  else fails, that issue is a real parser bug and its wave flips — re-plan first.
- The calc/constraint implementation itself (I7) — owned by
  `2026-06-08-implement-calc-constraint-grammar-in-zig.md`.

## Solution — waves (each ≈ one GSD plan)

### Step 0 — empirical gate (XS, no changes)
`cd deal && zig build run -- parse <each showcase .deal/.dealx>`; confirm the
"parser accepts" claims. Reclassify any surprise failure before proceeding.

### Wave A — doc/spec convergence, NO code (S–M; spec repo only)
- **I2:** add `AnnotationKey ::= IDENT | _Keyword` (or a name-token class) and use
  it for annotation/object field keys in `deal.ebnf`/`dealx.ebnf`.
- **I3:** make the verification-field `;` optional in `deal.ebnf`
  (`… value ";"?`).
- **I1 (spec half):** define `;` as the canonical annotation-field terminator,
  optional before `}` (`AnnotationField ::= IDENT ":" Expression ";"?`).
- **I5 (option b, honest-opaque):** redefine `GapBlock` in `dealx.ebnf` as an
  opaque text block matching the parser, and file a follow-on for structured gap
  (Wave E). Stops the spec from lying.
- **I6:** recompute every `file:Lnn` citation across all four EBNF files
  (scriptable); fix the `RequireStatement` L13→L17.
- DESIGN-DECISIONS.md: note the annotation-separator / keyword-key decisions.
- Bundle with the still-pending calc/constraint spec commit (same repo).

### Wave B — parser strictness hardening (M; deal repo)
- **I1 (parser half):** consume `;` *explicitly* as the field terminator; emit a
  diagnostic on a genuinely-unexpected token instead of the silent skip
  (`parser_deal.zig:1914-1916`). Decide `,` → fmt-normalized alias or dropped.
- **I3:** emit a diagnostic (or explicit, tested accept) for unknown verification
  keys instead of silently dropping (`:1956-1959`).
- `fmt.zig`: normalize annotation bodies to the canonical `;` form.
- Tests: new annotation/verification cases + re-run the malformed corpus
  (per-file error pinning) to prove no new silent acceptance.

### Wave C — array unification (L; deal repo + spec)
- Add `ArrayLiteral ::= "[" (Expression ("," Expression)*)? "]"` to
  `PrimaryExpression` (`deal.ebnf`); redefine `VerificationArray` as alias/removal.
- Add the `l_bracket` atom case to `expr.zig` (single source of truth); delete
  `parser_dealx.parseArrayLiteral` and `parser_deal.parseVerificationArray`,
  delegating both contexts to it.
- Introduce a real `ArrayLiteral` AST node (replace the synthetic `__array__`
  call) — keep `__array__` only if codegen back-compat forces it; update
  IR/codegen + `fmt` accordingly.
- Tests: array goldens in expr + byte-identical output in both old contexts.

### Wave D — feed the calc/constraint build-out
Amend `2026-06-08-implement-calc-constraint-grammar-in-zig.md` so its Wave 2/3
target the ideal forms decided here (annotation `;` terminator, keyword keys,
unified `ArrayLiteral`) — so new productions are built right the first time.

### Wave E (optional) — structured gap (L)
Implement a real `gap` field loop (like satisfy `criteria`) + AST fields, if/when
`deal check --verify` needs to consume gap data. Pairs with I5 option (a).

## Acceptance

1. Step 0 run recorded; no unexpected parse failures (only I7's two files).
2. Every EBNF `Showcase example:` citation matches the live line; a guard (CI
   validator re-reading each cited line, or anchor-only citations) prevents
   recurrence.
3. Annotation bodies: `;`-terminated form is the documented + fmt-normalized
   canonical; parser consumes `;` explicitly and errors on truly-stray tokens
   (no silent discard); keyword keys (`from:`) accepted with a test.
4. Verification fields: `;` optional in EBNF; unknown keys no longer silently
   dropped (diagnostic or tested-explicit accept).
5. Arrays: one `ArrayLiteral` production + one parser path; both showcase array
   contexts produce byte-identical output vs. pre-refactor goldens.
6. `gap` EBNF matches parser reality (opaque now; structured tracked as Wave E).
7. No regression: existing showcase + Phase 2 goldens unchanged;
   `phase-1.5-gate-fresh` stays green; malformed-corpus pins still fire.

## Cross-references
- Analysis: `DEAL-Grammar-Drift-Remediation-Analysis.html` (per-issue ideal +
  affected-surface matrix).
- Sibling todo: `2026-06-08-implement-calc-constraint-grammar-in-zig.md` (I7).
- Strictness contract: Phase 1.5 (ROADMAP) +
  `ADR-phase-1.5-fresh-worktree-verification.md`.

## Scheduling note
Wave A is spec-repo-only and low-risk — bundle with the pending calc/constraint
spec commit. Waves B/C touch the same parser files as the calc/constraint
build-out, so sequence them adjacent (or fold B/C as a prerequisite) to avoid
two passes over `parser_deal.zig`/`expr.zig`/`fmt.zig`. Decide at phase planning.
