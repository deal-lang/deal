# DEAL Context Notes

Running notes from DOC-classified sources. Verbatim with source attribution. Not authoritative for decisions, requirements, or constraints — see `decisions.md`, `requirements.md`, `constraints.md`.

---

## Topic: Grammar directory overview

**Source:** `/Users/dunnock/projects/deal-lang/spec/grammar/README.md` (DOC, precedence 3)

DEAL Grammar — Specification. Language: DEAL — Digital Engineering Authoring Language. Version: 0.1.0-draft. Status: Phase 0 complete — ready for Stage 1 parser implementation. Date: 2026-05-18.

### Files in `spec/grammar/`

- `lexical.ebnf` (~370 lines per README header; actual current file is 758 lines after completion) — ~125 token types — shared token definitions for both `.deal` and `.dealx` parsers.
- `deal.ebnf` (1,679 lines) — 87 productions — definition file grammar (`.deal`) — `part def`, `port def`, `requirement def`, etc.
- `dealx.ebnf` (897 lines) — 43 productions — composition file grammar (`.dealx`) — `[<system>]`, `[<connect>]`, `[<satisfy>]`, etc.
- `DESIGN-DECISIONS.md` — 65 locked design decisions — authoritative source for what the syntax IS.
- `maal-layer1-notation-reference.md` — W3C EBNF notation operators — the notation system used in all grammar files.

> NOTE: The README's listed `lexical.ebnf` size (370 lines) reflects an earlier snapshot. The current file is 758 lines. README statistics elsewhere (125 token types, 130 productions total, 78 reserved keywords) remain consistent with the current grammar.

### Grammar architecture (from README)

```
lexical.ebnf (L1)          ← shared token definitions
    ↓              ↓
deal.ebnf (L2/L3)    dealx.ebnf (L4)
    .deal files           .dealx files
    definitions           compositions
```

The lexer uses the same token set for both file types. The parser switches start symbol based on file extension:

- `.deal` → `DealFile` (from `deal.ebnf`)
- `.dealx` → `DealxFile` (from `dealx.ebnf`)

`dealx.ebnf` imports shared productions from `deal.ebnf` (HeaderBlock, Expression, AnnotationStatement, etc.). The dependency is unidirectional — `.deal` files never reference `.dealx` constructs.

### Statistics (from README)

- 130 productions (87 in deal.ebnf + 43 in dealx.ebnf)
- 125 token types (keywords, operators, delimiters, literals)
- 78 reserved keywords (37 global + 41 contextual)
- 65/65 design decisions covered
- 19/19 showcase files parseable
- LL(1) at 8 decision points, LL(2) at 4 (all justified)
- No left recursion — expression grammar uses iterative repetition

### Notation conventions

All grammar files use W3C EBNF notation (XML 1.0 Fifth Edition §6). Conventions:

- `PascalCase` — public production (generates AST node)
- `_PascalCase` — helper production (transparent, no AST node)
- `ALL_CAPS` — terminal token from `lexical.ebnf`
- `"quoted"` — keyword or operator literal
- `[base]` / `[ext]` — SysML v2 inherited vs DEAL extension
- `[L1]` – `[L4]` — grammar layer (Lexical / SysML / Sugar / Composition)

### Verification claims (per README)

The grammar has been verified against:

1. Design decision coverage — all 65 locked decisions trace to grammar productions.
2. Showcase parse coverage — all 19 showcase files in `examples/showcase/` are fully parseable.
3. FIRST set disjointness — all alternation points have disjoint FIRST sets.
4. Cross-grammar consistency — no conflicting definitions between grammar files.
5. Tag balance — `.dealx` grammar enforces matching open/close tags.
6. No orphan productions — every production is reachable from a start symbol.

(See the Integration Verification Report, `deal-grammar-verification-report.html`, for full details. Not yet ingested.)

### Related documents referenced by README (not yet ingested)

| Document                        | Location                                | Purpose                       |
|---------------------------------|-----------------------------------------|-------------------------------|
| Integration Verification Report | `deal-grammar-verification-report.html` | Complete verification results |
| Parser Implementation Guide     | `deal-parser-implementation-guide.html` | Notes for Stage 1 implementer |
| Design Decisions                | `DESIGN-DECISIONS.md`                   | 65 locked syntax decisions    |
| Notation Reference              | `maal-layer1-notation-reference.md`     | W3C EBNF operator reference   |
| Showcase Project                | `examples/showcase/`                    | 19-file test corpus           |

### Next Steps (per README)

The grammar is the input to Stage 1: "Hello World" That Sells (~4–6 weeks):

1. Hand-written Zig lexer + recursive-descent parser
2. `deal fmt` — parser proof-of-life (parse → pretty-print → round-trip)
3. `deal build --target sysml-v2-json` — first codegen backend
4. VS Code TextMate grammar for syntax highlighting
5. Basic LSP for go-to-definition

> NOTE: The README's "Next Steps" describes Stage 1 at a high level; the authoritative roadmap with phase gates and milestones is `DEAL-LANG-ROADMAP.html` (see `requirements.md`).

*Grammar specification produced by grammar-architect workflow, Phases 1–6.*
*65 locked decisions · 130 productions · 19 showcase files · 2026-05-18*

---

## Topic: Document-set cross-references and provenance

These are observed but not authoritative facts about the ingest set, useful to downstream consumers:

- `DESIGN-DECISIONS.md` lives in `spec/grammar/tmp-references/` — the `tmp-references` directory name suggests transient/working location; consider promoting before treating the file as repo-permanent.
- The three EBNF grammar files cross-reference each other in their header comments (`lexical.ebnf` ↔ `deal.ebnf` ↔ `dealx.ebnf`). The actual dependency direction is unidirectional layering (lexical → deal → dealx); the bidirectional `cross_refs` in the classifications JSON are documentation pointers, not import cycles.
- `DEAL-LANG-ROADMAP.html` lives at the project root (`/Users/dunnock/projects/deal-lang/DEAL-LANG-ROADMAP.html`), one level above `spec/grammar/`. It references all three EBNF grammars and the showcase directory.
- Verification and parser-implementation-guide HTML files are referenced but not ingested; both are listed under "Related Documents" in the grammar README.
