# Phase 2: Prove the Pipeline - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-21
**Phase:** 02-prove-the-pipeline
**Areas discussed:** Codegen language, IR v0 design, Comment-attachment grammar, --json diagnostic schema + E2xxx codes

---

## Codegen language

### Where should the SysML v2 JSON emitter (REQ #4) live?

| Option | Description | Selected |
|--------|-------------|----------|
| Rust | Walks IR via C ABI (`deal_ir_json()` exposed from Zig), emits SysML JSON via serde_json. Mature `jsonschema` crate; aligns with PROJECT.md tech-stack split; pays one IR-serialize round-trip per `deal build`. | ✓ |
| Zig (in libdeal.a) | Emits SysML JSON directly from in-memory IR; zero FFI overhead but must write/vendor offline JSON-Schema validator in Zig; deviates from PROJECT.md. | |
| Split: Zig emits, Rust validates | Zig walks IR and writes SysML JSON via `deal_emit_sysml_v2()`; Rust runs offline schema validation. Keeps emitter near IR but more C ABI surface. | |

**User's choice:** Rust (Recommended)
**Notes:** Honors PROJECT.md tech-stack constraint; mature offline JSON-Schema validation crate ecosystem.

### How should Rust receive the DEAL IR from Zig?

| Option | Description | Selected |
|--------|-------------|----------|
| JSON via new C ABI symbol | `deal_ir_json(handle, out_ptr, out_len)` — extends Phase 1's locked C ABI pattern; serde_json into typed structs. | ✓ |
| FlatBuffers / binary IPC | Schema language with Zig + Rust bindings; zero-copy reads but adds toolchain dependency. | |
| Shared C-struct layout via deal.h | bindgen wraps Zig structs; violates D-10 opaque-handle ownership; brittle. | |

**User's choice:** JSON via new C ABI symbol (Recommended)
**Notes:** Extends D-11 length-prefixed UTF-8 buffer pattern; one serialize/deserialize round-trip per build (acceptable).

### Where does the semantic analyzer (REQ #1, all 6 blocking checks + .deal/index.json) live?

| Option | Description | Selected |
|--------|-------------|----------|
| Zig | New `src/sema.zig`; walks AST in parser arena; feeds AST→IR lowering directly; new diagnostics through existing `Codes` namespace. | ✓ |
| Rust (consumes AST JSON) | Easier to evolve semantics in higher-level code; pays AST-serialize cost per check; diagnostics emitted by Rust. | |
| Split: name resolution in Zig, semantic checks in Rust | Stable symbol-table contract for LSP; doubled diagnostic emission; complex error-code story. | |

**User's choice:** Zig (Recommended)
**Notes:** Honors PROJECT.md "Zig owns the compiler core"; reuses parser arena and existing diagnostic infrastructure.

### Should `deal fmt`'s pretty-printer live in Zig or Rust?

| Option | Description | Selected |
|--------|-------------|----------|
| Zig + new `deal_format()` C ABI export | Zig pretty-printer walks AST (with comments); `deal_format()` is explicitly named in SPEC.md REQ #7 acceptance criterion. | ✓ |
| Rust (consumes AST JSON) | Easier text manipulation; violates PROJECT.md tech-stack split; risks parser/printer drift. | |
| You decide | Defer to researcher. | |

**User's choice:** Zig + new `deal_format()` C ABI export (Recommended)
**Notes:** Pretty-printer + parser stay in sync; SPEC.md REQ #7 acceptance criterion explicitly mentions `deal_format()` C ABI.

---

## IR v0 design

### What is the stable element ID strategy for IR nodes?

| Option | Description | Selected |
|--------|-------------|----------|
| Fully-qualified path string | `sedan_project.vehicle.electrical.BatteryPack.hvOut`; human-readable in diagnostics; mirrors PS-2 dot-paths. | ✓ |
| Content hash (sha256) | Stable across renames if structure unchanged; opaque in diagnostics; breaks under any edit. | |
| Hybrid: path + UUID v7 in @header | Survives renames and reorgs; requires amending FS-2 @header schema; new `deal init` workflow needed. | |

**User's choice:** Fully-qualified path string (Recommended)
**Notes:** Used as @trace targets, SysML $id URL suffixes, .deal/index.json keys, IR find(id) lookups.

### How is the IR organized at the top level?

| Option | Description | Selected |
|--------|-------------|----------|
| Single workspace-wide IR graph | One graph spans the entire `deal.toml` workspace; one consolidated SysML output per build; matches FS-4. | ✓ |
| Per-package IR | One IR per package; parallelizable; cross-package refs become stringly path references. | |
| Per-file IR with explicit cross-file edges | Maps 1:1 to source files; sprawls codegen output across 19 JSON files. | |

**User's choice:** Single workspace-wide IR graph (Recommended)
**Notes:** Cross-package @trace and <<specializes>> resolve as graph traversals; simpler codegen; one $id namespace.

### Does the IR carry comments and source-formatting information?

| Option | Description | Selected |
|--------|-------------|----------|
| Comments stay on AST only — IR is comment-free | AST carries attached comments per REQ #7; IR is structural-only; `deal fmt` walks AST, SysML emitter walks IR. | ✓ |
| IR carries doc comments only (/** */) as `documentation` fields | Doc comments become typed `documentation: string` on IR nodes; maps to SysML `documentation` ownedRelationships. | |
| IR carries all comments + source spans | Full reversibility; bloats IR; couples IR to source-text concerns. | |

**User's choice:** Comments stay on AST only (Recommended)
**Notes:** Mirrors TypeScript / typical compiler split; future ADR may open SysML `documentation` lowering path.

### What backend traversal/query API does IR v0 expose?

| Option | Description | Selected |
|--------|-------------|----------|
| Visitor pattern + indexed lookups | walk(visitor), find(id), references(id), children(id), parent(id); small surface; serializable. | ✓ |
| Query DSL (foreshadows MQL) | Locks a DSL now; MQL is explicitly deferred per ADR; premature design. | |
| Visitor + read-only graph access (no helpers) | Smallest contract surface; every consumer reimplements id→node lookup. | |

**User's choice:** Visitor pattern + indexed lookups (Recommended)
**Notes:** Covers 95% of codegen + LSP needs; ad-hoc queries are coded as visitors; MQL deferred per ADR.

---

## Comment-attachment grammar

### How are `//` line and `/* */` block comments attached to AST nodes?

| Option | Description | Selected |
|--------|-------------|----------|
| Leading + trailing on declarations only | gofmt-style attachment with blank-line rule; minimal new AST surface; round-trip-friendly. | ✓ |
| Leading-only | Simpler rule; trailing comments either reattach to next node or get dropped (violates REQ #7). | |
| Free-floating trivia list (token-stream order) | Zero AST surface changes; pretty-printer must merge two streams; harder for LSP/sim. | |

**User's choice:** Leading + trailing on declarations only (Recommended)
**Notes:** Matches prettier/gofmt/rustfmt convention; researcher should cite Go's `go/ast`/`go/printer` for canonical rules.

### How does `/** */` JSDoc-style doc comment interact with the new attachment?

| Option | Description | Selected |
|--------|-------------|----------|
| Promote to attached `doc_comment` field on declarations | Reuses existing DocComment struct (ast.zig:393); doc comments queryable as structured metadata; LSP hover gets them directly. | ✓ |
| Treat all three comment kinds identically | Uniform AST shape; loses DocComment @tag structure. | |
| Keep doc_comment as standalone node | Smallest delta; doc-comments appear in AST graph in unexpected places. | |

**User's choice:** Promote to attached `doc_comment` field (Recommended)
**Notes:** Existing DocComment NodeKind union arm stays for rare floating-doc-comment edge case; common case is the attached field.

### How are the 19 existing AST snapshots handled when comment attachment lands?

| Option | Description | Selected |
|--------|-------------|----------|
| One-shot regeneration with reviewed diff | Single dedicated commit; diff reviewed for unexpected churn; determinism + property-span tests stay green. | ✓ |
| Parallel comment-attachment snapshot mode | `--with-comments` mode; two AST JSON shapes violates D-04 single-version intent. | |
| Bump AST JSON schema to `v: 2` | Clean versioning; every consumer must read both versions or pin one; complicates Phase 1's locked v1 contract. | |

**User's choice:** One-shot regeneration with reviewed diff (Recommended)
**Notes:** AST JSON stays at `"v": 1`; only the new comment fields appear in diff; structural fields must remain byte-identical.

### When does the comment-attachment plan land relative to other Phase 2 work?

| Option | Description | Selected |
|--------|-------------|----------|
| Early — before semantic analyzer + IR | Plan 02-01 or 02-02 lands attachment + snapshot refresh; downstream consumers see final AST shape from start. | ✓ |
| Mid-phase — after sema but before IR-lock checkpoint | Sema and lowering ship using current AST; comment attachment lands before fmt; AST consumers written against two shapes mid-phase. | |
| Last — right before deal fmt | Bundled with fmt plan; every AST consumer was written against old shape; risk of late surprises. | |

**User's choice:** Early — before semantic analyzer + IR (Recommended)
**Notes:** Smallest cascading rework risk; only one AST snapshot refresh; SPEC.md REQ #7 explicitly calls out parser-affecting work.

---

## --json diagnostic schema + E2xxx codes

### What shape should `deal check --json` (and other --json output) take?

| Option | Description | Selected |
|--------|-------------|----------|
| Extend Phase 1's diagnostics JSON envelope | `{"v": 1, "command": "...", "diagnostics": [...], "summary": {...}}`; reuses existing diagnostic shape; matches D-04/D-18. | ✓ |
| rustc-style JSON-lines (one diagnostic per line) | Streamable; cargo-compatible; no summary line; harder for single-document tools. | |
| LSP Diagnostic shape directly | Zero translation for Phase 3 LSP; leaks LSP concerns (URIs, version) into CLI output. | |

**User's choice:** Extend Phase 1's diagnostics JSON envelope (Recommended)
**Notes:** Zero re-design; `deal_diagnostics_json()` C ABI stays unchanged — Rust CLI does the envelope-wrap.

### Where in D-16's code ranges do E2xxx semantic error codes sit?

| Option | Description | Selected |
|--------|-------------|----------|
| Allocate E2000..E2499 semantic band, extend D-16 | Sub-bands map 1:1 to REQ #1's 6 blocking checks; huge headroom; clear taxonomy. | ✓ |
| Reuse and extend E0400..E0499 (recovery band) | Conflates recovery and semantic checks; only 100 slots; ambiguous taxonomy. | |
| Flat sequential (next available after E0122) | Simplest; no taxonomy; breaks D-16's banding intent. | |

**User's choice:** Allocate E2000..E2499 semantic band, extend D-16 (Recommended)
**Notes:** E2000..E2099 name-res, E2100..E2199 type, E2200..E2299 multiplicity, E2300..E2349 specializes, E2350..E2399 trace, E2400..E2499 imports. Extension committed in src/diagnostics.zig.

### What SysML v2 viewer is used for the REQ #9 viewer-import smoke test?

| Option | Description | Selected |
|--------|-------------|----------|
| Try multiple, lock the first that works | Priority: OpenMBEE → IncQuery → NoMagic/Cameo → generic JSON-Schema viewer. SPEC.md REQ #9 acceptance pattern. | ✓ |
| Lock OpenMBEE SysML v2 Pilot Implementation now | Deterministic plan; if OpenMBEE rejects output for non-DEAL reasons, plan stalls. | |
| You decide | Defer to researcher. | |

**User's choice:** Try multiple, lock the first that works (Recommended)
**Notes:** First successful import wins; all attempts logged in 02-VERIFICATION.md; documented blocker acceptable only if every attempt fails with a recorded reason.

### How should plans be sliced around the mid-phase IR-lock checkpoint?

| Option | Description | Selected |
|--------|-------------|----------|
| 5-6 plans with checkpoint between Plan 3 and Plan 4 | CLI shell + comment attach → sema → IR + lowering → 🔒 checkpoint → SysML codegen + fixtures → deal fmt → gate. | ✓ |
| Fewer (4) bigger plans — sema+IR+codegen+CLI | Fewer commits; IR-lock checkpoint awkwardly positioned; harder atomic verification. | |
| More (8+) smaller plans — one per requirement | Clean attribution; cross-cutting deps (E2xxx codes shared) create ordering conflicts. | |
| You decide | Defer slicing to /gsd:plan-phase. | |

**User's choice:** 5-6 plans with checkpoint between Plan 3 and Plan 4 (Recommended)
**Notes:** Plans 02-01..02-06 outlined in D-37 of CONTEXT.md. CLI shell early for end-to-end integration testing; comment attachment early per Area 3 decision.

---

## Claude's Discretion

Areas left to researcher / planner judgment, captured in CONTEXT.md `<decisions>` ### Claude's Discretion subsection:

- Rust schema-registry crate selection (`jsonschema` vs `boon` vs `schemars`)
- Cargo workspace layout (cli/ + tests/ffi/ shared workspace? where does bindgen of deal.h happen?)
- Agent-metadata envelope shape in IR (@confidence / @rationale / @assumes / @concerns)
- Simulation-binding representation in IR (@simulation:<<computes>>)
- Doc-comment `documentation` lowering to SysML in Phase 2 (or defer to ADR)
- Blank-line rule edge cases for comment attachment (gofmt-style; cite go/ast)
- `deal_format()` return shape (bytes vs caller-supplied path)

## Deferred Ideas

Captured in CONTEXT.md `<deferred>` section:

- MQL query language for IR (Phase 6+)
- IR carries doc comments as `documentation` field (follow-up ADR if showcase output is missing inline docs)
- AST JSON `"v": 2` schema bump (kept at v: 1; comment-attachment is additive)
- Per-package IR + per-file SysML output (Phase 6 desktop-tooling concern)
- Free-floating trivia list (fallback architecture if structural attachment hits round-trip edge cases)
- FlatBuffers / shared-struct IR transport
- `deal init` / `deal install` / package resolution (Phase 4)
- LSP / VS Code / tree-sitter (Phase 3)
- ReqIF emitter + reverse importers (Phase 4 / Phase 6)
- Simulation / evidence / `deal-sim` (Phase 5)
- WASM packaging of Zig core (evaluate at Phase 2 gate per PRD §11)
- Promote `DESIGN-DECISIONS.md` out of tmp-references/ (Phase 4 docs work)
- Update `grammar/README.md` line-count drift 370→758 (Phase 4 docs work)
