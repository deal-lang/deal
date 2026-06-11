## Conflict Detection Report

### BLOCKERS (0)

No locked-decision contradictions, no LOCKED-vs-LOCKED ADR conflicts, no low-confidence UNKNOWN classifications, no destructive cross-reference cycles. Synthesis proceeded across all 6 documents.

### WARNINGS (0)

No competing requirement acceptance variants. The PRD (`DEAL-LANG-ROADMAP.html`) is the sole requirements source; no second PRD exists in the ingest set to produce divergent acceptance criteria. Phase requirements and ADR decisions cover disjoint scopes (decisions = WHAT the language is; requirements = WHEN/HOW the implementation ships).

### INFO (8)

[INFO] Single consolidated ADR yields 65 individual decisions
  Note: `DESIGN-DECISIONS.md` was classified as a single ADR but is a consolidated decision log. Per run notes, each `### XX-N: title [LOCKED]` header (FA-1..FA-5, NC-1, LM-1..LM-3, FS-1..FS-4, PS-1..PS-10, SD-1..SD-20, CS-1..CS-16, SIM-1..SIM-5) was extracted as its own decision entry in `decisions.md` with the decision code preserved as the ID. Total: 64 LOCKED + 1 CAPTURED (SD-6 is CAPTURED, not LOCKED — all others LOCKED).

[INFO] EBNF grammars captured as protocol-level constraints
  Note: The three W3C EBNF grammar files (`lexical.ebnf`, `deal.ebnf`, `dealx.ebnf`) were each captured as one `protocol`-type constraint entry rather than extracting every production. Per run notes, the grammar as a whole IS the contract for its file type. Each entry attributes which design decisions it implements per the file header comments (e.g., `lexical.ebnf` implements LM-1, LM-2, LM-3, SD-3, SD-5, SD-10, SD-11, SD-12, CS-2, NC-1).

[INFO] Grammar cross-reference graph contains documentation cycles, not semantic cycles
  Note: Cross-ref scan found that `lexical.ebnf`, `deal.ebnf`, and `dealx.ebnf` mutually reference each other in their classification `cross_refs` lists. Inspection of the actual file headers and the grammar README shows the dependency is unidirectional layering (lexical → deal → dealx; `dealx.ebnf` imports shared productions from `deal.ebnf`; `.deal` files never reference `.dealx` constructs). The bidirectional cross-refs in JSON are documentation pointers, not parse/import cycles. No synthesis loop risk; no BLOCKER triggered.

[INFO] Auto-resolved: roadmap defers to ADR on implementation staging
  Note: `DESIGN-DECISIONS.md` §Implementation Staging contains a phase list with a self-attributed note: "Ordering authority: Execution order is now maintained in `DEAL-LANG-ROADMAP.html`. The phase list below records the current roadmap shape at decision-record granularity; if detailed milestone ordering differs, the roadmap supersedes this section." Standard precedence (ADR > SPEC > PRD > DOC) would put ADR first, but the ADR explicitly cedes ordering authority to the PRD. Recorded: phase ordering and milestone-level detail are sourced from the PRD (`requirements.md`); strategic phase shape mirrors the ADR.

[INFO] Auto-resolved: roadmap's Stage 1 high-level steps subsumed by PRD Phase 1 milestones
  Note: `grammar/README.md` §Next Steps lists a 5-step "Stage 1: 'Hello World' That Sells (~4–6 weeks)" plan: hand-written Zig lexer + RD parser, `deal fmt`, `deal build --target sysml-v2-json`, VS Code TextMate grammar, basic LSP for go-to-definition. The PRD (`DEAL-LANG-ROADMAP.html`) breaks the same work into more detailed Phase 1 (lexer + parser + error recovery + C ABI), Phase 2 (formatter + SysML v2 codegen + CLI), Phase 3 (TextMate + tree-sitter + LSP). PRD wins on precedence (PRD > DOC). README's Stage 1 captured as context only.

[INFO] README size statistic for lexical.ebnf is stale
  Note: `grammar/README.md` cites `lexical.ebnf` as 370 lines. The current file is 758 lines (per re-ingest after user completed the grammar). Other README statistics (125 token types, 130 productions total, 78 reserved keywords, 65/65 design decision coverage, 19/19 showcase parseable) remain consistent with the current grammar. Recorded in `context.md`.

[INFO] DESIGN-DECISIONS.md lives in a `tmp-references` directory
  Note: The ADR source path is `/Users/dunnock/projects/deal-lang/spec/grammar/DESIGN-DECISIONS.md`. The `tmp-references` directory name suggests transient/working location. Synthesis treats the file as authoritative per its `manifest_override: true` classification, but downstream may want to promote it to a permanent location.

[INFO] SD-6 is the only [CAPTURED] decision; treated as soft commitment, not LOCKED
  Note: All other decision codes in `DESIGN-DECISIONS.md` are marked `[LOCKED]`. SD-6 ("Relationship categories") is `[CAPTURED]` — the enumeration is partial and full enumeration is in the deferred-decisions list. `decisions.md` preserves the CAPTURED status separately from LOCKED; downstream planning should not assume SD-6's enumeration is exhaustive.
