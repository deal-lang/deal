# Changelog — deal (core compiler)

All notable changes to the DEAL compiler core (Zig engine + Rust CLI/LSP). This
project tracks the staged language implementation; dates are the working-tree
dates of the changes, not formal releases.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added — LSP P3: editor polish (folding, inlay badges, code lens, doc links)

Four additive capabilities, each a thin consumer of the P1 mapping table and the
P2 binding index — no compiler changes.

- **Folding ranges** (`folding.rs`) — packages, `*_def` bodies, behavioral
  blocks, and doc comments fold; doc comments use the `Comment` fold kind.
- **Inlay hints** (`inlay.rs`) — inline `«Metaclass»` badges after each
  declaration name (e.g. `«PartDefinition»`), from `sysml_mapping::resolve`;
  range-scoped and client-toggleable.
- **Code lens** (`code_lens.rs`) — a "N references" lens above every `*_def`,
  counted from the P2 reverse index, routed through the `deal.showReferences`
  extension bridge command.
- **Document links** (`doc_links.rs`) — `import P.{N}` items link to N's
  declaring file (via the index's new `import`-ref-kind link map), the only
  navigation affordance for import items (goto-definition cannot reach them).

All four advertised, handler-wired, exercised by `lsp-smoke`, and covered by
unit + integration tests. New index queries: `elements_in_file`,
`import_links_in_file`.

### Added — LSP P2: cross-file resolution, navigation, rename, IDE features

Builds compiler-exact cross-file name resolution and the editor features that
depend on it, plus the remaining IDE capabilities.

- **Authoritative binding index (ADR-0003)** — `sema` now resolves every
  reference against the workspace-merged declaration set and emits a
  per-reference record `{ from_span, resolved_path, ref_kind }` into the
  IR/index envelope; the LSP ingests these into an exact `resolved_path →
  [sites]` reverse index. Cross-file names bind by the prefix-subtree-unique
  rule (`import P.{N}`), with ambiguity surfaced as `E2001` rather than guessed.
- **Precise name spans** — the parser/AST carry terminal-name spans
  (`terminal_span`, `target_span`, `ElementDef.name_span`, `ImportItem.name_span`)
  so navigation and rename act on the exact identifier token, never the
  `<<…>>` operator or whole declaration.
- **Find-references + document highlight** — `textDocument/references` and
  `documentHighlight` return compiler-exact cross-file / in-file sites
  (declaration + `specializes` + `type_ref` + named `import` items).
- **Rename (verified)** — `textDocument/rename` + `prepareRename` edit every
  reference kind, including `import` statements, refuse renames into a colliding
  declaration or a dependency, and run a verify-before-return re-analysis of the
  post-edit workspace, refusing any rename that would introduce a new error.
- **documentSymbol** — hierarchical per-file outline built from AST containment
  with the P1 element-kind icon map.
- **signatureHelp** — `calc` / `constraint` invocations show the callee
  signature (cross-file resolved) with the active parameter tracked by a
  depth-aware comma count.
- **codeAction** — unresolved-reference diagnostics (`E2000`) offer a
  did-you-mean quick-fix that replaces the typo with the nearest workspace
  symbol by edit distance.
- **Expression + constraint hover** — parameterized `constraint def`s render
  their full signature, and expression nodes (`binary`/`unary`/`call`/
  `identifier`/`member_access`/literals) surface their SysML v2 / KerML
  expression metaclass via the spec's expression rows and literal variants.

Deferred follow-ups: parameter goto-definition (local scope) and semantic
tokens for literals/operators.

### Added — LSP P1: drift closure, SysML-mapping hover, behavioral tokens

Brings the language server back in step with the language and surfaces the
SysML v2 target of every construct in the editor.

- **Element-kind parity** — the four element definitions that had drifted out of
  LSP coverage (`allocation def`, `need def`, `use case def`, `actor def`) plus
  `calc def` are now wired across completion, hover labels, semantic tokens, and
  the workspace-symbol icon map (`map_symbol_kind`). Completion now offers all 16
  element keywords (was 11).
- **SysML v2 mapping (`spec/ir/sysml-mapping.json`)** — single machine-readable
  DEAL-node-kind → SysML v2 / KerML table, consolidating the locked behavioral
  (`v0.1`) and expression (`v0.2`) contracts with the structural emitter arms.
  Metaclass clauses confirmed via closure-bounded `sysml-v2-wiki` / `kerml-wiki`
  retrieval (several stale emitter-comment clauses corrected, e.g.
  `PartDefinition` 8.3.11.2, KerML `Function` 8.3.4.7.4).
- **Enriched hover (`lsp/src/sysml_mapping.rs`, `hover.rs`)** — hover now appends
  the target metaclass, governing clause, and KerML basis (plus injected control
  nodes / `«actor»` marker / dotted qualifiedName where applicable). Embedded via
  `include_str!` from the `spec` submodule; unmapped kinds degrade to the prior
  signature+doc output.
- **Behavioral semantic tokens** — `decide`/`par`/`loop`/`for`/`send`/`accept`/
  `assign`/`bind`/`on`/`entry`·`do`·`exit`/`state`/`succession` lead keywords are
  now tokenized; the behavioral surface no longer renders as undifferentiated
  text.
- **Emitter (`cli/src/sysml_v2.rs`)** — `use_case_def` → `UseCaseDefinition`
  (8.3.25.3) and `allocation_def` → `AllocationDefinition` (8.3.15.2) now emit
  their real metaclasses instead of the generic element fallthrough. A new drift
  test asserts `sysml-mapping.json` agrees with the emitter for structural kinds.
- **Grammar (`spec/grammar/deal.ebnf`)** — `UseCaseDefinition` /
  `AllocationDefinition` normalization notes annotated with the wiki-verified
  metaclass + clause.
- **CI (`.github/workflows/ci.yml`)** — per-commit push/PR gate: `zig build` +
  `cargo fmt`/`clippy`/`test` on the Rust workspace (previously the LSP was tested
  only on release tags).

### Added — Stage 3: structured behavioral expressions (IR v0.2)

Behavioral guards, assignment values, accept payloads, and loop bounds now lower
to structured IR expression nodes and emit as schema-valid SysML v2 / KerML
`Expression` trees — completing the 1:1 behavioral mapping. A guard `[soc >= 80]`
emits as an `OperatorExpression(">=")` over a `FeatureReferenceExpression(soc)`
and a `LiteralInteger(80)`.

- **IR (`ir.zig`)** — 4 additive expression `NodeKind`s (`operator_expr`,
  `feature_ref_expr`, `literal_expr`, `invocation_expr`) + payload fields
  (`operator`, `literal_kind`, `literal_value`, `referent_segments`,
  `trigger_kind`). The guard/value/iterable/effect/callee slots now hold an
  expr-node id; `Edge.guard` keeps `"else"` as a sentinel. `ir_version` → `"v0.2"`
  (`json.zig`); the JSON serializer gained the new kinds + fields.
- **Lowering (`lowering.zig`)** — `lowerExpr` recursively lifts AST expression
  subtrees (binary/unary → `operator_expr` with the DEAL→KerML operator-symbol
  table; identifier/member_access → `feature_ref_expr`; literals →
  `literal_expr`; call → `invocation_expr`), owned via `contains` with
  deterministic structural ids (`…op0`/`op1`/`argN`). All 9 prior text-capture
  sites now produce structured expressions.
- **Emitter (`cli/src/sysml_v2.rs`)** — emit arms for OperatorExpression
  (8.3.4.8.17), FeatureReferenceExpression/FeatureChainExpression (8.3.4.8.5/.4),
  Literal{Boolean,Integer,Rational,String} (8.3.4.8.9/.12/.13/.14), and
  InvocationExpression (8.3.4.8.8) / TriggerInvocationExpression (SysML
  8.3.17.17). Transition guards wire as a `kind=guard` TransitionFeatureMembership
  owning a Boolean-valued `Expression` (8.3.18.8/.9), completing the
  trigger/guard/effect triad. Only non-derived slots authored; clauses cited.
- **Goldens** — `11-action-behavior` / `12-state-machine` regenerated with guards
  present (OMG SysML 20250201-valid); new guard-focused fixture
  `13-structured-guards` (covers FeatureChainExpression). `zig build test`,
  `cargo test`, and `deal build --validate` green; determinism, id-uniqueness,
  and fmt idempotence preserved.

### Added — Stage 2: behavioral surface (BH-1..BH-7)

The full behavioral surface now flows end to end: parse → semantic analysis →
IR → lowering → SysML v2 emission, with the showcase exercising it.

- **Lexer** — behavioral tokens (`->` `~>` `:=`) and 20 reserved keywords
  (`decide`, `par`, `loop`, `while`, `until`, `for`, `send`, `accept`, `assign`,
  `bind`, `node`, `succession`, `on`, `entry`, `do`, `exit`, `else`, `start`,
  `done`, `terminate`).
- **Parser** — `ActionBody` / `StateBody` (deal.ebnf §9b/§9c) with
  operator-driven LL(2) dispatch: pins, sub-actions (incl. bodies), succession
  chains, control endpoints, `decide` / `par` blocks, `loop`/`for`,
  `send`/`accept`/`assign`, perform calls, item flows, bindings, the
  `node`/`succession` escape hatch, `entry`/`do`/`exit`, transitions, and
  state-body parameters.
- **Sema** — behavioral resolution: pin/escape/flow type checks, dimensional +
  D7-purity checks on behavioral expressions, and per-body control-flow
  reference resolution (`E2700` step-not-declared, `E2701` flow-type-not-found).
- **IR v0.1** — additive superset: 16 behavioral node kinds, 4 edge kinds
  (succession/item_flow/binding/subaction) with payload, new `IrPayload`
  behavioral fields; `ir_version` now `"v0.1"`. (`spec/ir/v0.1/`.)
- **Lowering** — AST → IR v0.1 with §4 desugaring (decide → DecisionNode +
  implicit MergeNode; par → ForkNode + implicit JoinNode; loops; transitions;
  entry/do/exit subaction edges); deterministic synthetic-node ids.
- **fmt** — canonical, idempotent writers for the entire behavioral surface.
- **SysML v2 emitter** — every behavioral node/edge → its metaclass, each
  grounded in a closure-bounded wiki lookup with the clause cited in code
  (ActionDefinition/Usage, control/decision/merge/fork/join nodes,
  Terminate/Send/Accept/Assignment/Perform/While/For action usages, StateUsage,
  TransitionUsage with trigger/effect memberships, SuccessionAsUsage, FlowUsage,
  BindingConnectorAsUsage, StateSubactionMembership, pin ReferenceUsage +
  FeatureTyping).
- **Goldens** — `11-action-behavior` and `12-state-machine` SysML v2 fixtures,
  validated against the OMG SysML 20250201 schema.
- **Showcase** — `behaviors.deal` rewritten in real behavioral syntax and
  `charging-states.deal` added (state machine), both round-tripping through the
  full pipeline.

### Known limitations

- Behavioral guards / assignment values / payloads are carried as source text in
  the IR and are **not** yet emitted as structured SysML `Expression` trees
  (Stage-3 candidate; see `spec/ir/v0.1/FUTURE-structured-expressions.md`).

## [v2.1.0] — Stage 1: compiler core

Parser, semantic analyzer (incl. dimensional analysis), formatter, IR v0,
SysML v2 / ReqIF backends, `calc` / `constraint` (SD-21/22/23), project and
dependency tooling, and the simulation-evidence pipeline — all gated by the test
suite. CLI: `deal parse | check | fmt | build | init | install | simulate |
evidence`.
