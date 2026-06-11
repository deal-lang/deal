# ADR: Graphical View Projection & Bidirectional Sync

**Status:** PROPOSED
**Date:** 2026-06-06
**Updated:** 2026-06-06 — all open questions resolved (GV-9..GV-12 added); view membership, graphical-notation conformance, and views-as-edit-surfaces added (GV-13..GV-15)
**Phase:** 03-intelligence (LSP) / desktop-editor
**Author:** Architecture brainstorm — David
**Relates to:** D-11 (FFI length-prefixed transport), D-15 (AST spans), D-21 (formatter ownership), D-23 (qualified-path IDs), D-24 (single workspace graph), D-25 (comment-free IR); LSP (Phase 3), tree-sitter (Phase 2/3), importers (Phase 6)
**Links:** [ADR-deal-ir-v0.md](./ADR-deal-ir-v0.md) | [spec/ir/v0/README.md](../../spec/ir/v0/README.md) | [include/deal.h](../../include/deal.h)

---

## Context

DEAL must present a graphical view of the system model and let users edit it, while the canonical artifact remains plain `.deal` / `.dealx` text in git. This raises a bidirectional-interface problem in two directions:

- **Forward:** text → graphical view, kept current automatically as the model changes.
- **Reverse:** a change made through the GUI (or any application) flows back into the canonical text or its view metadata without destroying comments, formatting, or layout.

The OMG **SysML v2 Systems Modeling API & Services** answers this with a REST/HTTP + JSON server backed by a git-like model store (Project / Branch / Commit / Element / Relationship / Query, plus an OSLC binding). That architecture exists largely to *be* a versioned model repository so federated tools can share one model. DEAL inverts that premise: the source of truth is text in git, so git already provides the commit graph, branching, diffing, and merging. Reimplementing the SysML API's model-store machinery would rebuild what we deliberately replaced with files.

Two further observations shaped this decision:

1. Existing SysML v2 editors either require a **manual "sync" button**, or they re-render automatically but become **glitchy and laggy** — re-parsing and re-laying-out on every keystroke, and rendering (or blanking) through transient syntax errors mid-edit.
2. The model can be edited from **two distinct surfaces**: the embedded editor in the Tauri desktop app, and any external IDE (VS Code, Zed, JetBrains) editing the files directly.

The pipeline this ADR builds on already exists in locked form: the Zig core exposes `deal_parse`, `deal_ir_json`, and `deal_format` over a narrow C ABI (D-11); the IR carries stable fully-qualified-path IDs (D-23), byte-offset spans (D-15), and a single workspace-wide edge graph (D-24); and the IR is comment-free (D-25), so reverse edits must never regenerate source from the IR.

This ADR captures the architecture for the graphical projection and its synchronization. New decisions are tagged **GV-n** below and should be registered into the central decision registry at lock.

---

## Decision

The graphical view is a **projection of the IR**, never of raw text. One engine owns parsing and decides when renderable IR exists; the view consumes diffs. View geometry lives in a separate, git-diffable sidecar. The reverse direction splits into two cheap, independent paths. The render surface is GPU-accelerated and driven by a Zig→WASM kernel that reuses the existing core.

### GV-1 — The IR is the only render input; qualified path is the universal join key

The renderer never reads `.deal`/`.dealx` text. It reads the IR document produced by `deal_ir_json`. Because every IR node has a stable fully-qualified-path ID (D-23), that single string identifies the same element across four layers:

- the IR node,
- the render kernel's spatial / hit-test index,
- the view-state sidecar rule,
- the source text location (via the node's span, D-15).

A click resolves through the spatial index to a qualified path, and that path is all that is needed to look up geometry, semantics, and source location.

### GV-2 — View state lives in a sidecar, keyed by qualified path

Position, size, collapsed/expanded compartments, edge-routing waypoints, per-view membership, and color overrides are **view state, not model**. They are stored in a sidecar artifact, separate from the semantic source, consistent with the "model is code" philosophy and with the comment-free IR (D-25).

The sidecar is **CSS-shaped** — type-level rules act as defaults, element-level rules as instance overrides — but it is DEAL's own format, not literal CSS, so node geometry is not forced through CSS's style-vs-layout semantics. Keys use the **attribute-selector form** to avoid the collision between dots in qualified paths and CSS class-chaining (`.a.b`):

```
part_def            { fill: surface; min-width: 160 }      /* type default */
[deal-id="vehicle.battery.BatteryPack"] { x: 120; y: 40 }  /* instance override */
```

A diagram can show the same element in more than one view, so geometry is scoped **per view** (one sidecar per diagram/viewpoint, or a `@view {}` scoping block) — not a single flat global stylesheet.

**Keying choice (decided): qualified path (name-based).** Simple and human-readable. The accepted cost is that renames change the key. This is made survivable, not eliminated, by GV-2a/GV-2b rather than by adopting opaque IDs.

- **GV-2a — Rename-aware sidecar rewrite.** A rename performed through the editor or LSP already computes the old→new qualified path. That operation also rewrites matching keys in the sidecar, preserving layout across renames at near-zero cost.
- **GV-2b — Orphan tolerance via auto-layout fallback.** A rename done by raw hand-edit outside tooling may orphan a rule. Orphaned rules fall back to auto-layout (GV-3) instead of breaking; the editor surfaces a non-blocking hint ("view rule for X has no matching element") and offers reattachment.

### GV-3 — Auto-layout base layer + sidecar deltas

Auto-layout is the base layer; the sidecar stores only **deltas** from it. Consequences: a brand-new model with no sidecar renders immediately, sidecar files stay small and merge-friendly, and only newly-added nodes are auto-placed on update — existing nodes keep their positions so the diagram never "jumps around."

### GV-4 — Two reverse paths with different costs

- **Semantic edit** (change a value, add a part, wire a connection) → resolve the target IR node's span (D-15) → splice replacement bytes into that source range → re-parse → `deal_format` (D-21). Comments and all formatting outside the edited span survive. The splice itself is **pure Rust byte manipulation guided by IR spans — it requires no new Zig core code**. Insertions (no existing span) splice at the enclosing container's body span and let `deal_format` normalize placement.
- **Layout edit** (drag, resize, collapse) → write a delta into the view sidecar. **Never touches `.deal` text and never re-parses.** The kernel updates geometry in-memory for instant feedback; the sidecar write is debounced on drop.

Most interactions are layout, so most interactions are cheap.

### GV-5 — Render substrate: Zig→WASM kernel + GPU canvas

For "fast and clean" at large element counts, the frontend hosts a single `<canvas>` and does almost no logic. The Zig core, compiled to **WASM**, takes the IR and owns the visual scene: layout, a spatial index, viewport culling, and emission of draw lists (instance buffers). JS only uploads buffers and issues draw calls — no per-element DOM, which is what kills SVG at scale.

- **Primitives.** Each SysML element kind is a parameterized **draw template** (rounded-rect-with-compartments parameterized by size, title, compartments). Thousands render via *instanced* quads + a glyph atlas. The "clean default component" is a draw template in the kernel, not a DOM node — reusing the engine rather than a separate C codebase.
- **Text.** Crisp labels at all zooms is the main cost of leaving SVG/DOM: use an SDF font atlas or glyph cache, optionally a thin HTML/2D-canvas overlay for the few labels in view. **Prototype the text path first — it is the make-or-break of this approach.**
- **Substrate baseline.** **WebGL2** is the baseline (broadly supported across the OS webviews Tauri uses: WKWebView, WebView2, WebKitGTK). **WebGPU** is a progressive enhancement where available (support is still uneven, especially on WKWebView).
- **Interim on-ramp.** A batched-WebGL library (regl / PixiJS) with layout still in WASM is acceptable as a stepping stone, accepting that the draw loop then lives in JS. The Zig→GL kernel is the principled end state.

### GV-6 — Auto-update sync model (the anti-glitch contract)

Exactly one engine debounces, parses, lowers, and decides whether to render. The view only ever shows a smooth incremental update or the last good frame — never a half-parsed mess.

- **Off-thread, debounced, coalesced.** Parsing/lowering runs on a background worker with a fresh Zig handle per parse (handles are independent), debounced (~150–250 ms idle). If edits outrun parses, drop stale ones (latest-wins). Each result is tagged with a buffer version; results that arrive out of order are discarded.
- **Renderable gate = "usable IR," not "zero diagnostics."** The core emits IR even with semantic diagnostics (unresolved refs are normal mid-edit), so semantic errors still render (show the node, flag the bad edge). Only true *syntactic* breakage that prevents a usable IR triggers the next rule.
- **Hold-last-good on syntax break.** When IR cannot be lowered, keep showing the previous scene and surface a subtle "syncing…" badge. Never blank, never render partial garbage. This single rule eliminates most observed flicker/lag.
- **Incremental, id-keyed diff.** New IR is diffed against the last IR by qualified path (added / removed / changed sets). Only changed nodes are touched; camera and sidecar overrides are preserved. No full rebuild, no re-layout thrash.

### GV-7 — Unified edit sources

- **Embedded Tauri editor (in-process):** on edit → debounce → background parse from the in-memory buffer → IR diff → apply to the WASM scene. No file I/O.
- **External IDE (VS Code / Zed / JetBrains):**
  - **LSP as the shared brain (primary).** The IDE is already an LSP client for highlighting/diagnostics and sends `didChange` on every keystroke, so the DEAL LSP server sees **unsaved, in-flight** edits. The Tauri view subscribes to a custom notification (`deal/irDidUpdate`) carrying the IR diff. One server parses and decides "renderable yet?"; every front-end consumes the same decision, so the diagram can update from edits the IDE has not even saved.
  - **Filesystem watch (universal fallback).** FSEvents / inotify catches saves from any editor, including ones not speaking the LSP. It is **save-only** (no unsaved state) and must be debounced for atomic temp-file swaps.

### GV-8 — Two parse cadences

tree-sitter (already planned) drives **instant, error-tolerant** syntax highlighting and shallow structural hints on every keystroke; the authoritative **Zig parse → IR** runs on the debounce for the graphical model. Instant-but-shallow for text feedback, debounced-but-authoritative for the diagram.

### GV-9 — Sidecar organization: `.dealview`, theme file + one file per view

The view sidecar is a **third extension-selected grammar mode** (`.dealview`) sharing `lexical.ebnf`, exactly as `.deal` / `.dealx` select their grammars by extension (D-12). The lexer, `deal fmt`, and the tree-sitter pipeline extend to it naturally; no foreign CSS parser is embedded.

Organization mirrors CSS architecture:

- A workspace-level **theme file** carries type-level defaults — the house style per element kind (`part_def { ... }`).
- **One file per view/diagram** carries instance geometry (`[deal-id="..."] { x; y; ... }`), registered in `deal.toml`.

Consequences: merge conflicts isolate to the one diagram somebody actually re-laid-out; deleting a view is a file delete; the theme cascades across all views without duplication.

### GV-10 — Opaque identity: inline `@id`, lazy-minted

Stable identity is an **optional standalone annotation** carrying a **UUIDv7** string:

```deal
requirement def R_BatteryRange {
    @id: "0196a3e2-7c41-7d92-b1f4-9e8d2a6c0f33"
    ...
}
```

This is **grammar-legal today** — `StandaloneAnnotation ::= ANNOTATION (":" AnnotationValue)?` with `AnnotationValue ::= Expression`, and a string literal is a valid expression (same shape as `@rationale:`). No grammar change.

- **Minting policy:** lazy + tool-driven. An element receives an `@id` when it first crosses an identity-critical boundary (first ReqIF / SysML baseline export) or when the user pins one explicitly (`deal id pin <qualified.path>`). Elements that never leave the workspace never carry one. Tooling mints; humans never hand-type UUIDs.
- **Sema:** workspace-wide `@id` uniqueness check (new E-code; trivial under D-24's single graph — catches copy-paste duplication), plus a well-formedness warning for malformed UUIDs.
- **IR:** additive optional `stable_id` payload field — explicitly permitted within IR v0 (schema update + golden regeneration + determinism pass).
- **Emitter precedence:** `elementId := @id if present, else UUIDv5(qualified path)` — same rule for ReqIF identifiers. Explicit identity overrides derived identity; elements without `@id` behave exactly as today, so external identity becomes rename-stable precisely for the elements that need it.
- **Bonus recovery:** an orphaned view rule (GV-2b) can be re-attached automatically by matching `@id` after an out-of-band rename, instead of prompting the user.
- **View sidecar keying is unchanged** (GV-2): sidecars stay name-keyed; `@id` exists for external baseline stability, with reattachment as a side benefit.

### GV-11 — `deal/irDidUpdate` payload: delta + snapshot request

The notification carries the **id-keyed diff the engine already computes for GV-6** — `added[]` / `removed[]` / `changed[]` node and edge sets — plus:

- a **monotonic sequence number** (per workspace epoch), and
- a **`status` field** (`clean` | `held`) so the "syncing" badge state flows through the same channel.

Clients obtain a full IR document via a **`deal/ir` request** on connect, and re-request it on sequence-gap detection. This is the standard delta+snapshot pattern (cf. LSP semantic tokens full/delta). Full-document push per update and bare change-pings were both rejected (see Alternatives Record).

### GV-12 — GPU backend: criteria-gated WebGPU promotion over a WebGL2 floor

The kernel's draw lists are **backend-neutral**; the GL/GPU backend is selected at startup by runtime feature detection (`navigator.gpu`). **WebGL2 is the permanent floor.** WebGPU is promoted per platform only when all three gates pass:

1. WebGPU is **default-on** in that platform's webview for the OS versions DEAL supports,
2. the backend passes the **golden-image render tests**, and
3. it shows a **measurable win** over WebGL2 on the 10k-element benchmark (frame time or memory).

No hardcoded support matrix lives in this ADR — the criteria are evergreen. (Context at time of writing: WebGPU is default-on in Safari 26 / macOS Tahoe 26 / iOS 26 — and therefore reaches WKWebView on those OS versions — and has shipped in Chromium/WebView2 since Chrome 113; WebKitGTK has no announced ship date, which is what makes Linux the floor case.)

### GV-13 — View membership: a view is membership + layout

The three-tier model is: **definitions** (`.deal` — components, immutable from the composition's side, configured only via instantiation parameters), **compositions** (`.dealx` — systems/subsystems, immutable from the view's side; variants specialize, never mutate), and **views** (`.dealview` — configurable projections). A view consists of **membership** (what it shows) and **layout** (where/how), both in the sidecar.

Membership is declared with `include {}` / `exclude {}` blocks:

```
include {
    model.EVPlatform.Thermal.**;                       // subtree glob
    model.EVPlatform.EnergyStorage.{coolantIn, coolantOut};  // member set
    kind(requirement) within requirements.system;       // kind filter
    filter <expr>;                                      // predicate filter
}
```

Pattern forms: exact qualified path, `*` (one level), `**` (subtree), `{a, b}` member sets, `kind(...)` optionally scoped by `within`, and predicate `filter` expressions.

**Semantics:**

- **Default:** a co-located view with no `include` block shows everything in its served file (backward compatible with sidecar-only layout files).
- **Edges** render when both endpoints are visible; `dangling-edges: stub | hide` (view property, default `stub`) governs edges that leave the view.
- **No-fabrication invariant:** the `.dealview` grammar has **no element-declaring production** — a view can reference and style, it cannot introduce. Showing something that does not exist in the model is unrepresentable, not merely checked.
- **Validation:** `deal check --views` resolves every `deal-id` and include pattern against the IR (unresolved → diagnostic; same name-resolution machinery as sema), and reports coverage (model elements appearing in zero views).
- **SysML v2 mapping:** `include` lowers to `expose` (MembershipExpose / NamespaceExpose), `filter` to ElementFilterMember (Clause 8.2.2.26) — view membership is exportable to OMG-ecosystem tools. **Layout is never exported** (SysML v2 does not standardize diagram layout interchange).
- Views are no longer strictly 1:1 with source files: non-co-located views registered in `deal.toml [views]` may cut across files (e.g. a thermal view drawing from three subsystems).

### GV-14 — Render vocabulary: SysML v2 Graphical Notation conformance

The kernel's draw templates (GV-5) implement the **SysML v2 Graphical Notation** (Clause 8.2.3, `spec/references/SysML-graphical-bnf.kgbnf`) — node, compartment, and relationship symbol productions parameterized by IR payload — rather than an invented notation.

Rationale: (a) **adoptability** — DEAL diagrams read as standard SysML to any practitioner; (b) **the notation is itself a grammar whose symbol contents are textual-notation fragments**, so the spec normatively defines the graphical↔textual correspondence. DEAL reuses that correspondence: a compartment line maps to a textual production maps to a DEAL construct with a source span, which is precisely what keeps GUI↔text round-trip (GV-4) well-defined. DEAL-specific constructs (e.g. satisfy evidence blocks) get conformant extensions following the notation's annotation patterns.

### GV-15 — Views are edit surfaces, not pictures

A view is also an **interaction surface**: semantic edits made through any view follow the GV-4 splice path into model text. Drawing an `allocate` edge in the traceability view creates a real `[<allocate>]` in the model; it is not a drawing.

Three rules govern this:

1. **Affordances per viewpoint.** Each viewpoint affords the semantic edits whose constructs it renders: traceability → create/delete `allocate` and `satisfy` relationships; structure → components and connects; definition (BDD) → defs and specializations; interface (ICD) → connects with `via`/`carrying`. The renderer offers only the edits appropriate to the view — you cannot accidentally rewire HV power from inside the traceability matrix.
2. **Edit-target rule.** Edits to an *existing* element splice into the file that owns the element's span (`source_file` on the IR node) — clicking `REQ_SYS_001` in the traceability view and editing it edits `packages/requirements/system.deal`. *New* model content created through a view splices into the view's bound `source` file (the co-located file, or the `source` key in `deal.toml [views]`) — a new allocate edge drawn in the traceability view lands in `model/traceability.dealx`.
3. **Membership is visibility, not permission.** `include`/`exclude` controls what renders, not what may be edited through what renders. The no-fabrication invariant constrains the *view artifact*, never the user acting through it: content the user creates becomes model text first, then appears in every view whose membership matches it.

---

## Why not the SysML v2 Systems Modeling API as the primary mechanism

The SysML v2 API is primarily a versioned model **store** plus a network transport. DEAL's source of truth is text in git, which already provides versioning, branching, diffing, and merging — roughly the majority of what that API exists to do. Adopting it as the core mechanism would duplicate the commit/store machinery DEAL replaced with files.

Instead: make the **IR (forward) + span-anchored edit (reverse)** the universal contract, and expose it through transports already on the roadmap — **in-process FFI** (embedded/fast path) and **LSP custom methods / `WorkspaceEdit`** (bidirectional, editor-agnostic). A SysML-v2-API-shaped façade is added later **only** for foreign OMG-ecosystem tools that speak neither C nor LSP, implemented as a thin read-mostly Rust wrapper over the same engine (git as the commit store), never a parallel model store.

---

## Alternatives Record

| Area | Option | Status | Reason |
|------|--------|--------|--------|
| Transport | Reimplement SysML v2 Systems Modeling API as primary | Rejected | Rebuilds a versioned model store that git already provides; heavy server + DB |
| Transport | IR + span-edit over FFI (fast) + LSP (bidirectional) | **Chosen** | Reuses planned components; one engine, thin shells |
| Transport | SysML-API-shaped façade | Deferred | Thin read-mostly wrapper, added only when a foreign tool requires it |
| Reverse edit | Regenerate source text from IR | Rejected | IR is comment-free (D-25); destroys comments and layout |
| Reverse edit | Span-anchored byte splice + `deal_format` | **Chosen** | Preserves comments/formatting; pure Rust, no new Zig core |
| View keying | Opaque stable `@id` token | Deferred | Survives renames but adds identity machinery; revisit if rename pain is high |
| View keying | Qualified path (name-based) | **Chosen** | Simple/readable; rename handled via GV-2a rewrite + GV-2b fallback |
| View format | Literal CSS | Not selected | Familiar, but overloads style-vs-layout semantics; dot/class collision |
| View format | CSS-shaped DEAL sidecar (attribute-selector keys) | **Chosen** | Cascade defaults+overrides without CSS semantic mismatch |
| Render | SVG / DOM | Rejected for scale | Clean and accessible but sluggish past ~1–2k visible elements |
| Render | Zig→WASM kernel + WebGL2 (WebGPU progressive) | **Chosen** | GPU-fast, reuses core, instanced primitives |
| Render | Native GPU canvas outside the webview | Deferred | Highest ceiling, largest commitment; only for tens of thousands of elements |
| Sync trigger | Manual sync button | Rejected | Poor UX; the stated goal is automatic updates |
| Sync trigger | Re-render every keystroke from full parse | Rejected | The observed source of glitch/lag and mid-edit syntax-error rendering |
| Sync trigger | Debounced off-thread parse + hold-last-good + id-keyed diff | **Chosen** | Automatic and smooth; degrades safely on syntax break |
| View org | Single file with `@view {}` blocks | Not selected | Merge conflicts concentrate; file grows unbounded |
| View org | One file per view, no theme layer | Not selected | House styling duplicated across every view |
| View org | Theme file + file-per-view (`.dealview` mode) | **Chosen** | Conflict isolation per diagram; CSS-architecture cascade; reuses lexer/fmt/tree-sitter |
| Identity | Content-hash identity | Rejected | Any edit changes the hash — identity fails at its only job |
| Identity | Identity ledger sidecar | Rejected | Identity leaves the text; generated file drifts and merges badly |
| Identity | Inline `@id` on every element | Not selected | Permanent source noise; large diffs on first format |
| Identity | Inline `@id`, lazy-minted, UUIDv7 | **Chosen** | Grammar-legal today; additive IR field; emitter precedence over UUIDv5(path) |
| LSP payload | Full IR per update | Rejected | Ships the whole workspace graph at edit cadence |
| LSP payload | Bare ping + client pull | Not selected | Discards the computed diff; adds a round-trip per update |
| LSP payload | Delta + sequence + `deal/ir` snapshot | **Chosen** | Reuses the GV-6 diff; standard delta+snapshot recovery pattern |
| GPU backend | WebGPU-first, WebGL2 legacy | Rejected | Blocks on the least-supported platform (Linux WebKitGTK) |
| GPU backend | WebGL2 only, revisit later | Not selected | Leaves performance on the table on WebView2 / macOS 26+ |
| GPU backend | Criteria-gated WebGPU promotion | **Chosen** | Backend-neutral draw lists; runtime detect; evergreen criteria |
| Membership | Model-side `[<view>]` in `.dealx` | Not selected | Curating a diagram would dirty model files, blurring the GV-4 split |
| Membership | Implicit whole-file only (no include) | Rejected | Cannot curate subsets, cut across files, or have N views per source |
| Membership | Sidecar `include`/`exclude` blocks | **Chosen** | View churn never touches the model; lowers to SysML v2 expose/filter for export |
| Expressiveness | Globs + member sets only | Not selected | Views go stale as the model grows; every addition is manual |
| Expressiveness | Globs + sets + kind + predicate filters | **Chosen** | Full representation of the showcase; maps 1:1 to SysML expose + ElementFilterMember |
| Notation | Invented DEAL-specific notation | Rejected | Sacrifices adoptability and the spec-defined graphical↔textual correspondence |
| Notation | SysML v2 Graphical Notation (kgbnf) | **Chosen** | Reads as standard SysML; symbol grammar reuses textual fragments, anchoring GUI↔text round-trip |
| Interaction | Views as read-only pictures | Rejected | Forces all model editing back to raw text; loses the Cameo-class workflow (e.g. drawing trace links) |
| Interaction | Views as edit surfaces (affordances + edit-target rule) | **Chosen** | GV-4 splice path reused; viewpoint-scoped affordances; new content lands in the view's bound source |

---

## Consequences

**Positive**
- "What the system is" and "how I'm looking at it" are versioned separately; both diff and merge in git.
- Qualified path is a single join key across IR, renderer, sidecar, and source — simplifying hit-test, reverse edits, and rename handling.
- The reverse semantic-edit path needs no new Zig core code; the entire bidirectional editor can be prototyped over the existing C ABI.
- One engine decides renderability, so the IDE's highlighting and the Tauri diagram never disagree.
- Hold-last-good + incremental diff directly eliminate the glitch/lag failure modes seen in other tools.

**Negative / costs**
- GPU text rendering (SDF atlas / glyph cache) is real work and is the principal technical risk of GV-5.
- Name-based keying means hand-edits outside tooling can orphan layout (mitigated, not eliminated, by GV-2a/2b).
- The LSP server gains a non-standard responsibility (pushing IR diffs to a non-editor subscriber via a custom notification).
- WebGPU cannot be relied on uniformly across OS webviews yet; WebGL2 is the floor.
- `@id` copy-paste duplication becomes a new user-facing error class (mitigated: sema uniqueness check is workspace-wide and the fix is mechanical).

**Risks to watch**
- Incremental layout stability — added nodes must not perturb existing placements.
- Debounce tuning vs. perceived latency on large files; may later require incremental reparse.
- Sidecar merge conflicts when two people re-layout the same diagram (ordinary text-merge, but worth a UX story).

---

## Verification (to satisfy at implementation)

1. **Round-trip semantic edit:** applying a span-anchored edit then `deal_format` changes only the targeted declaration; all comments and unrelated formatting are byte-identical. (Extends the existing `deal fmt` round-trip fixtures.)
2. **Hold-last-good:** injecting a transient syntax error mid-edit leaves the prior scene rendered and raises the syncing indicator; no blank frame, no exception.
3. **Incremental diff:** a single-attribute change produces a diff touching exactly one node; camera and all sidecar overrides are unchanged.
4. **Rename preservation:** a tool-driven rename rewrites sidecar keys so layout is preserved; a raw-text rename leaves the element auto-laid-out with a reattachment hint.
5. **Cross-surface sync:** an unsaved edit in an external IDE (LSP client) updates the Tauri view via `deal/irDidUpdate`; with no LSP client attached, a save updates it via filesystem watch.
6. **Scale:** a synthetic model of N elements (target the 10k band) renders and pans at interactive frame rates on the WebGL2 path.
7. **Identity stability:** after `deal id pin` and a subsequent rename, the SysML `elementId` and ReqIF identifier are byte-identical across exports; a copy-pasted duplicate `@id` is rejected by sema with the new E-code.
8. **Delta recovery:** a client that misses `deal/irDidUpdate` notifications detects the sequence gap and converges to the correct scene via a `deal/ir` snapshot request.
9. **Backend parity:** golden-image render tests produce equivalent output on WebGL2 and WebGPU backends before WebGPU is promoted on any platform.
10. **Membership resolution:** every include pattern and `deal-id` in the showcase views resolves against the IR; `deal check --views` reports any model element appearing in zero views.
11. **No-fabrication:** the `.dealview` grammar contains no element-declaring production (verified structurally at grammar review); a view referencing a non-existent element produces a diagnostic, never a rendered node.
12. **View export round-trip:** showcase `include` blocks lower to SysML v2 `expose`/filter constructs that validate against the OMG schema.
13. **Edit-through-view:** drawing an allocate edge in the traceability view splices a grammar-valid `[<allocate>]` into `model/traceability.dealx`, and the new edge appears in the view after reparse; editing an existing requirement through the same view modifies `packages/requirements/system.deal`, not the view's source file.

---

## Deferred implementation details

All architectural open questions are resolved (GV-9..GV-12). Remaining details are implementation-scoped and do not affect the decision surface:

- The `.dealview` production set (property vocabulary: `x`, `y`, `w`, `h`, `collapsed`, waypoints, style-token refs) — to be written in the spec repo alongside `deal.ebnf` / `dealx.ebnf`.
- Token resolution chain (exemplified in showcase `model/global.dealview`): rules reference workspace tokens; a workspace token aliases a **built-in slot** (small normative vocabulary every renderer provides: `base`, `accent`, `muted`) or defines a **literal with `light()`/`dark()` variants**. The exact built-in slot list is locked with the grammar.
- Field-level JSON shape of `deal/irDidUpdate` and the `deal/ir` request — to be specified with the Phase 3 LSP custom-method surface.
- The exact E-code assignment for duplicate `@id` and the W-code for malformed UUIDs.
- WebGPU benchmark thresholds (what counts as a "measurable win") — set when the 10k-element benchmark exists.

---

*Proposed 2026-06-06; open questions resolved same day. Register GV-1..GV-15 into the decision registry at lock.*
*Source of truth for: desktop-editor graphical projection, Phase 3 LSP view-sync extension, `@id` stable-identity contract for ReqIF/SysML emitters.*
