# ADR: Document Generation — Model-Bound Reports as IR Projections

**Status:** PROPOSED
**Date:** 2026-06-06
**Phase:** 04-ecosystem (emitters) / desktop-editor
**Author:** Architecture brainstorm — David
**Relates to:** D-11 (FFI length-prefixed transport), D-15 (AST spans), D-21 (formatter ownership), D-23 (qualified-path IDs), D-24 (single workspace graph), D-25 (comment-free IR); SysML v2 emitter & ReqIF emitter (Phase 4); LSP (Phase 3)
**Links:** [ADR-deal-ir-v0.md](./ADR-deal-ir-v0.md) | [ADR-graphical-view-projection-and-sync.md](./ADR-graphical-view-projection-and-sync.md) | [spec/ir/v0/README.md](../../spec/ir/v0/README.md) | [spec/ir/v0/schema.json](../../spec/ir/v0/schema.json)

---

## Context

Engineering programs must emit human-readable documents from the model: specifications, requirements lists, interface descriptions, trade summaries, review packages — as DOCX (corporate review), PDF (controlled release), and XLSX (tabular data). DEAL's premise is that the `.deal`/`.dealx` text is the single source of truth, so these documents must be **derived artifacts**: regenerated from the model, never a second place where facts are authored or maintained.

The incumbent this replaces is Cameo's Report Wizard (DocBook/Velocity templates over the UML model). Its failure modes are well understood and are what this ADR is designed to avoid:

1. **Binding by navigation, not by identity.** Templates walk the model graph by position or by element reference. When an element is moved, renamed, or deleted, the binding silently resolves to nothing — the output gets a blank field, not an error. Failures are discovered by eyeballing the rendered document.
2. **Opaque, un-reviewable templates.** The template is a binary `.docx` with embedded Velocity. It cannot be diffed, code-reviewed, or merged, and small model edits routinely break it in ways no tool reports.
3. **Logic in the template.** Loops, conditionals, and computed values live in the template, so the template accrues business logic that belongs in the model and becomes its own fragile artifact.

DEAL is unusually well-positioned to fix all three by construction, because the substrate already exists in locked form. The Zig core exposes `deal_ir_json` over the C ABI (D-11). The IR carries a stable **fully-qualified-path ID** on every node (D-23, e.g. `vehicle.battery.BatteryCell.nominalVoltage`), with uniqueness guaranteed by sema's name-resolution check. Relationships live in a single workspace-wide **edge adjacency list** (D-24) with kinds `contains`, `satisfies`, `specializes`, `derives_from`, `traces`, etc. Every node carries a byte-offset **span** (D-15). The SysML v2 and ReqIF emitters (Phase 4) already establish the pattern: a pure-Rust consumer walks the IR JSON and writes an output format — none re-parse source.

Document generation is therefore the same shape of problem already solved twice, plus a binding layer. This ADR follows the philosophy of the graphical-view ADR: **the document is a projection of the IR, never of raw text**, joined by qualified path. New decisions are tagged **DG-n** and should be registered into the decision registry at lock.

---

## Decision

Author documents as **plain-text templates** that reference IR nodes by qualified path through a **restricted, declarative binding language**. A `deal report` step resolves bindings against the IR, **fails the build on any unresolved reference**, and produces a **format-neutral document model** that fans out to DOCX, PDF, and XLSX through swappable renderers. Templates are text in git; computed values live in the model; documents are one-way derived artifacts.

### DG-1 — The document is a projection of the IR; qualified path is the binding key

The template engine never reads `.deal`/`.dealx` text. Its only input is the IR document from `deal_ir_json`. Every binding is expressed against a node's stable fully-qualified-path ID (D-23). That single string is the universal join key — the same property GV-1 relies on for the graphical view — so a binding, a diagnostic, and a source location all resolve through one identifier. This is the structural antidote to Cameo's navigation-based binding: references are by **identity**, not by traversal from a root.

### DG-2 — Restricted declarative binding language, not a template programming language

The binding language permits exactly three things and no more:

- **Field access** on an IR node: `vehicle.battery.BatteryCell.nominalVoltage.value`, `…​.type_ref`, `…​.rationale` (agent metadata).
- **Graph queries** built from the existing edge kinds (D-24): `children(id)`, `references(id)`, `satisfies(id)`, `specializes(id)`, `traces(id)` — thin wrappers over the adjacency indexes the IR already exposes (`children_index`, `incoming_index`).
- **Structural iteration and projection** over a query result: an `each(query){ … }` block that maps a sub-template across results, and a `table(query, columns:[…])` projection. Iteration is **data-driven repetition** (one section per requirement), not control flow.

Explicitly **excluded:** conditionals, arithmetic, string manipulation, user-defined variables, arbitrary expressions. If a document needs a computed quantity (a margin, a rollup, a derived limit), that value is declared in the `.deal` model — where it is typed, reviewable, and reused by every consumer — and the template merely *names* it. This is the discipline that keeps the document a faithful projection and keeps logic out of the fragile layer.

Proposed surface syntax — inline `{{ … }}` delimiters wrapping a restricted DEAL expression, embedded in Markdown or Typst prose:

```markdown
## {{ vehicle.battery.BatteryPack.name }}

Nominal cell voltage: {{ vehicle.battery.BatteryCell.nominalVoltage.value }} V

### Requirements satisfied
{{ each(satisfies(vehicle.battery.BatteryPack)) {
- **{{ it.name }}** — {{ it.text.value }}
} }}

### Attributes
{{ table(children(vehicle.battery.BatteryPack), columns:[name, type_ref, value]) }}
```

### DG-3 — Build-time reference resolution and validation (the anti-Cameo core)

`deal report` resolves **every** binding against the IR before rendering. An unresolved reference — a renamed element, a deleted attribute, a typo in a path — is a **hard error with a span-located diagnostic** (the template span plus, where relevant, the nearest IR node), exactly like a type error. It is never a silently blank field. This check is also exposed through `deal check` so broken templates fail CI alongside broken models. This single decision converts Cameo's most expensive failure mode (discovered visually, late) into a compile error (discovered mechanically, immediately).

### DG-4 — Format-neutral document model, then renderer fan-out

Resolution produces one intermediate **document model** (a small structured doc AST: headings, paragraphs, runs, tables, references). Renderers consume that model; the resolver is written once.

- **DOCX** via Pandoc with a `reference.docx` carrying corporate styles (the format reviewers mark up).
- **PDF** via the same Pandoc path, or via **Typst** where controlled layout, equations, or drawings matter — Typst can also ingest the IR JSON directly (`json("model.ir.json")`) as an alternative authoring path for layout-heavy templates.
- **XLSX** from `table(…)` projections through a dedicated tabular emitter (a spreadsheet is not a natural Pandoc target, so it gets its own backend reading the same projections).

No format gets its own template system; adding a format means adding a renderer over the existing document model.

### DG-5 — Dependency manifest for staleness and impact analysis

Because resolution already touches every referenced node, it emits a **manifest** of `document-field → IR-path` links as a side artifact — the same edge-graph philosophy applied to documents. Consequences: after a model change, tooling can report exactly which documents are stale and which bindings broke, and a reverse query answers "which documents depend on this element?" before a rename. This is traceability Cameo cannot provide.

### DG-6 — One-way generation; documents are derived, never authored

Generation flows **model → document only**. There is no round-trip from an edited DOCX back into the model. Editing a generated document is editing a build output; the next regeneration overwrites it. (This deliberately differs from the graphical view's reverse-edit path in GV-4: the view is an interactive editing surface keyed to spans, whereas a report is a release artifact. Conflating the two is what makes round-trip document tooling brittle.) All authoring happens in `.deal`/`.dealx`.

### DG-7 — Templates are text, versioned, reviewed

Templates are plain UTF-8 files in git (e.g. `*.deal.md`, `*.deal.typ`). They diff, merge, and code-review like source. A template change is a reviewable commit, not an opaque GUI edit. This directly answers Cameo failure mode 2.

### DG-8 — Reuse the existing emitter pipeline; pure Rust over the IR, no new Zig core

`deal report` is a Rust consumer of `deal_ir_json`, structurally identical to the SysML v2 and ReqIF emitters and sharing their IR-deserialization layer. It requires **no new Zig core code** — the binding resolver, document model, and renderer adapters are all Rust over the already-locked IR transport (D-11). Pandoc and Typst are invoked as external renderers; only the XLSX backend is in-process.

### DG-9 — IR prerequisite: literal value capture (BLOCKING)

IR v0 payloads carry `name`, `type_ref`, `direction`, `modifiers`, and `agent_metadata`, but **not the literal/assigned value** of an attribute. In the showcase, `requirement def REQ_X { attribute text : String [1] = "Battery must charge in under 60 minutes."; }` lowers to a node with no field holding that string; likewise a default like `nominalVoltage = 3.7` is not in the IR. A report generator's whole purpose is to print these values, so document generation **cannot be implemented until the IR carries them.** This ADR requires a companion IR amendment adding a value field (e.g. `value` / `default` on the usage payloads, with type and source-literal form preserved). The amendment is small and self-contained, but it gates DG-2 field access on values and must land first. It is recorded here as a discovered dependency, not resolved by this ADR.

---

## Why not the obvious alternatives

**Why not a general template engine (Jinja/Velocity/Handlebars)?** Those are Turing-complete or nearly so, which is precisely what lets logic migrate into the template and rot there. DG-2's restricted language is a deliberate non-feature: by making computation impossible in the template, it forces derived values back into the model where they belong. The cost — you cannot "just add a conditional in the doc" — is the intended constraint.

**Why not author DOCX/XLSX directly with field codes or content controls?** That reproduces Cameo: an opaque binary template, bindings invisible to review, no build-time validation, and three parallel template systems (one per format). DG-4's format-neutral model gives one reviewable source and many outputs.

**Why not round-trip (edit the Word doc, sync back)?** Round-trip requires a reverse mapping from rendered prose to model elements, which is ambiguous and is the source of brittleness in bidirectional document tools. DG-6 keeps documents strictly derived; the interactive editing surface is the graphical view (separate ADR), not the report.

**Why not the SysML v2 API's view/viewpoint mechanism?** Same reasoning as the graphical-view ADR: that machinery assumes a versioned model store and network transport, which git-as-source-of-truth already replaces. The IR-projection contract is lighter and reuses components already on the roadmap.

---

## Alternatives Record

| Area | Option | Status | Reason |
|------|--------|--------|--------|
| Binding key | Navigation/traversal from a root (Cameo-style) | Rejected | Silent blanks on rename/move/delete; the core fragility being designed out |
| Binding key | Qualified-path identity (D-23) | **Chosen** | Stable join key; uniqueness guaranteed by sema; same key as IR/view/source |
| Binding language | General template engine (Jinja/Velocity) | Rejected | Lets logic rot into the template; not reviewable as data |
| Binding language | Restricted field-access + graph-query + projection | **Chosen** | Declarative projection; computation forced back into the model |
| Validation | Render and inspect visually | Rejected | Cameo's late, manual failure discovery |
| Validation | Build-time resolution; unresolved ref = hard error | **Chosen** | Broken bindings fail like type errors, in CI |
| Output | Author each format (DOCX/XLSX) directly | Rejected | Opaque templates, no validation, N parallel systems |
| Output | Format-neutral doc model → renderer fan-out | **Chosen** | One reviewable source; add a format = add a renderer |
| PDF renderer | Pandoc only | Acceptable | Simplest; corporate styling via `reference.docx` |
| PDF renderer | Typst (optional, layout-heavy) | **Chosen (additive)** | Controlled layout/equations; can read IR JSON directly |
| Direction | Round-trip (edit doc → model) | Rejected | Ambiguous reverse mapping; brittleness of bidirectional doc tools |
| Direction | One-way model → document | **Chosen** | Document is a derived release artifact |
| Engine placement | New logic in Zig core | Rejected | Unnecessary; IR transport already exists (D-11) |
| Engine placement | Pure-Rust consumer of `deal_ir_json` | **Chosen** | Mirrors SysML/ReqIF emitters; no new core surface |
| Value capture | Read values from re-parsed source | Rejected | Violates "IR is the only input" (DG-1); re-introduces a second path |
| Value capture | Amend IR to carry literal/default values | **Chosen (prerequisite)** | Required for DG-2; small self-contained IR change, must land first |

---

## Consequences

**Positive**
- Broken bindings are compile errors with spans, not blank cells found by eye — the headline improvement over Cameo.
- Templates are text: diffable, reviewable, mergeable, owned by engineers in git.
- One resolver, many outputs; corporate DOCX styling via `reference.docx`, controlled PDF via Typst, tabular XLSX from the same projections.
- The dependency manifest gives document-level impact analysis and staleness detection for free.
- Reuses the locked IR transport and the emitter pattern; no new Zig core code.
- "What the system is" stays in one place; documents can never drift from the model because they are regenerated, never maintained.

**Negative / costs**
- The restricted binding language cannot express in-document logic; teams accustomed to "just add a conditional" must instead model the derived value. This is intended but is a real workflow change.
- A document model expressive enough for real specs (nested tables, cross-references, figures, numbering) is non-trivial to design well; under-designing it pushes formatting back into templates.
- External renderer dependencies (Pandoc, optionally a Typst toolchain) enter the build.
- DG-9 blocks: value capture must be added to the IR before any of this ships.

**Risks to watch**
- Scope creep in the binding language — every "just one more query/conditional" erodes DG-2; the restriction must be defended.
- Document-model fidelity for complex corporate templates (styles, ToC, cross-refs) may strain the Pandoc path and pull toward Typst sooner.
- Manifest staleness detection is only as good as binding granularity; coarse bindings under-report impact.

---

## Verification (to satisfy at implementation)

1. **Resolution failure is a build error:** a template referencing a non-existent or renamed path fails `deal report`/`deal check` with a span-located diagnostic; no document is produced and no field is silently blank.
2. **Projection fidelity:** a golden model renders to a document whose every bound value byte-matches the IR; changing a model value and regenerating changes exactly that value in the output.
3. **Restriction enforcement:** a template containing a conditional or arithmetic expression is rejected by the binding parser, not silently evaluated.
4. **Fan-out parity:** the same template + model produces DOCX, PDF, and (for `table(…)`) XLSX from one document model; bound values agree across all three.
5. **Manifest correctness:** the emitted `field → path` manifest lists every IR path actually referenced; deleting a referenced element flips the dependent document to "stale/broken" before regeneration.
6. **No new core surface:** `deal report` links only against the existing C ABI exports; no new Zig symbol is required (parallels the ReqIF/SysML emitter builds).
7. **Value capture (DG-9 gate):** once the IR amendment lands, `attribute text = "…"` and numeric defaults appear in the IR and resolve through `{{ …​.value }}`; round-trip golden tests cover both `.deal` and `.dealx`.

---

## Open questions / deferred

- **DG-9 IR amendment shape:** exact field name(s) and whether to carry both a typed value and the verbatim source literal (needed to render units/precision faithfully). Likely its own short ADR + IR-LOCK touch.
- **Template file conventions:** extensions (`*.deal.md`, `*.deal.typ`), project layout, and how a "report set" (multiple documents from one model) is declared.
- **Document-model surface:** the minimal node set for real specs — cross-references, auto-numbering, figure/diagram embedding (and whether graphical-view exports feed in here).
- **Diagram inclusion:** whether `deal report` can embed rendered views from the graphical projection (shared qualified-path key makes this natural) or defers it.
- **Iteration ergonomics:** whether `each`/`table` cover real reports or a small, fixed set of additional projections (grouping, sorting) is warranted without reopening DG-2.
- **CLI shape:** `deal report <template> --target docx|pdf|xlsx` vs. a report-set manifest driving a batch build.

---

*Proposed 2026-06-06. Register DG-1..DG-9 into the decision registry at lock. DG-9 is a blocking prerequisite requiring a companion IR amendment.*
*Source of truth for: Phase 4 document-generation emitter, `deal report` CLI, template/binding spec.*
