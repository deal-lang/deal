# DEAL Constraints / Specification Intel

Per run notes, each W3C EBNF grammar file is captured as a single `protocol`-type constraint — the grammar as a whole IS the contract for its file type, not its individual productions. Detailed production-level extraction would duplicate the source files; reference them directly when needed.

Source SPECs (all precedence 1, manifest override):

- `/Users/dunnock/projects/deal-lang/spec/grammar/lexical.ebnf`
- `/Users/dunnock/projects/deal-lang/spec/grammar/deal.ebnf`
- `/Users/dunnock/projects/deal-lang/spec/grammar/dealx.ebnf`

All three are W3C EBNF (XML 1.0 Fifth Edition §6) and are at version `0.1.0-draft`.

---

## CONSTRAINT-lexical-grammar

- **Source:** `/Users/dunnock/projects/deal-lang/spec/grammar/lexical.ebnf`
- **Type:** protocol (grammar / lexical contract)
- **Title:** DEAL Lexical Grammar — `lexical.ebnf`
- **Scope:** All tokens shared by both the `.deal` (definition) and `.dealx` (composition) parsers. The lexer switches mode based on file extension, but all token types are defined here. ~125 token types covering Unicode-aware character classes, comments, identifiers, keywords (37 global + 41 contextual = 78 reserved), structural operators (`<<...>>`), composition tags (`[< >]`), numeric/string literals (including backtick-template strings per LM-3), and delimiters.
- **Layer:** L1 (foundation)
- **Status:** Phase 2 of grammar-architect workflow — COMPLETE (per `README.md`)
- **Design decisions implemented (from file header):** LM-1, LM-2, LM-3, SD-3, SD-5, SD-10, SD-11, SD-12, CS-2, NC-1
- **Notes:** 758 lines. Source of truth for the Phase 1 lexer requirement (`REQ-phase-1-1-lexer`). Defines the shared token alphabet that both grammars depend on.

## CONSTRAINT-definition-grammar

- **Source:** `/Users/dunnock/projects/deal-lang/spec/grammar/deal.ebnf`
- **Type:** protocol (grammar / syntactic contract)
- **Title:** DEAL Definition Grammar — `deal.ebnf`
- **Scope:** Grammar for `.deal` definition files. 87 productions covering: file structure, `@header`, package/import/export, all element definitions (`part`, `port`, `action`, `state`, `requirement`, `need`, `use case`, `interface`, `connection`, `flow`, `allocation`, `attribute`, `item`, `constraint`), element bodies, structural operators, annotations, expressions, and type annotations. Depends on `lexical.ebnf` for all token definitions.
- **Layer:** L2/L3 (SysML-aligned + sugar)
- **Status:** Phase 3 of grammar-architect workflow — COMPLETE (per `README.md`)
- **Design decisions implemented (from file header):** FS-1, FS-2, FS-3, PS-2, PS-3, PS-4, PS-6, SD-1 through SD-20, CS-5, CS-15
- **Notes:** 1679 lines. Source of truth for the Phase 1 `.deal` parser requirement (`REQ-phase-1-2-parser-deal`). Pratt expression grammar uses iterative repetition (no left recursion). LL(1) at most decision points; LL(2) at 4 justified points.

## CONSTRAINT-composition-grammar

- **Source:** `/Users/dunnock/projects/deal-lang/spec/grammar/dealx.ebnf`
- **Type:** protocol (grammar / syntactic contract)
- **Title:** DEAL Composition Grammar — `dealx.ebnf`
- **Scope:** Grammar for `.dealx` composition files. 43 productions covering: `[<system>]`, `[<subsystem>]`, component instances (self-closing tags with `as=` instance naming and `prop={...}` attributes), `[<connect>]` (with `via=`/`carrying=` separation), `[<expose>]`, `[<traceability>]`, `[<satisfy>]` with typed `=> {...}` returns, `[<validate>]`, `[<allocate>]`, inline definitions, and annotations within composition blocks.
- **Layer:** L4 (composition)
- **Status:** Phase 4 of grammar-architect workflow — COMPLETE (per `README.md`)
- **Design decisions implemented (from file header):** CS-1 through CS-16, SD-17, SD-19, FS-1, FS-2, FS-3, PS-2, PS-4
- **Shared productions imported from `deal.ebnf`:** HeaderBlock, PackageDeclaration, ImportDeclaration, Expression, QualifiedName, NamespacePath, TypeAnnotation, Multiplicity, AnnotationStatement, CategoryAnnotation, StandaloneAnnotation, AnnotationBody, AnnotationField, Definition, MemberDeclaration, FunctionCallExpression, ConditionExpression, DefaultValue.
- **Notes:** 897 lines. Source of truth for the Phase 1 `.dealx` parser requirement (`REQ-phase-1-3-parser-dealx`). Stack-based tag balancing is the parser's responsibility (specified by the grammar's open/close tag productions).

---

## Cross-grammar layering

```
lexical.ebnf (L1)         ← shared token definitions
    ↓              ↓
deal.ebnf (L2/L3)    dealx.ebnf (L4)
   .deal files          .dealx files
   definitions          compositions
```

The dependency is unidirectional: `.deal` files never reference `.dealx` constructs. `dealx.ebnf` imports shared productions from `deal.ebnf` (the import is documented in the dealx.ebnf header — see CONSTRAINT-composition-grammar above for the shared production list).

## Verification status (per `grammar/README.md`)

- Design decision coverage — all 65 locked decisions trace to grammar productions.
- Showcase parse coverage — all 19 showcase files in `examples/showcase/` fully parseable.
- FIRST set disjointness — all alternation points have disjoint FIRST sets.
- Cross-grammar consistency — no conflicting definitions between grammar files.
- Tag balance — `.dealx` grammar enforces matching open/close tags.
- No orphan productions — every production is reachable from a start symbol.

These verification claims appear in the Integration Verification Report referenced at `deal-grammar-verification-report.html` (not yet ingested).

---

*Total: 3 protocol-level constraints + cross-grammar layering note.*
