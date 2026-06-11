# ADR: DEAL IR v0 — public contract for Phases 2-6

**Status:** LOCKED
**Date:** 2026-05-21
**Locked:** 2026-05-21 (recorded in `.planning/phases/02-prove-the-pipeline/02-IR-LOCK.md`)
**Author:** Phase 2 planner / executor
**Links:** [spec/ir/v0/schema.json](../../spec/ir/v0/schema.json) | [spec/ir/v0/README.md](../../spec/ir/v0/README.md)

---

## Context

FA-1 (Architecture Decision 1) establishes that the DEAL compiler IR is the kernel of the pipeline. Every downstream consumer in Phases 3–6 — LSP (Phase 3), ReqIF emitter (Phase 4), simulation bindings (Phase 5), importers (Phase 6) — must walk or query the resolved element graph. Without a locked IR shape, each phase would have to reverse-engineer the AST and repeat name resolution, creating diverging implementations and fragile cross-phase contracts.

The DEAL source pipeline is:
```
.deal / .dealx sources
  ↓  lexer + parser (Phase 1)
  AST (with comment attachment, Phase 2 Plan 01)
  ↓  semantic analyzer (Phase 2 Plan 02)
  Symbol table
  ↓  lowering pass (Phase 2 Plan 03)   ← THIS ADR
  DEAL IR v0 Document
  ↓  deal_ir_json() C ABI
  IR JSON (UTF-8, length-prefixed)
  ↓  Rust CLI consumers
  SysML v2 JSON | ReqIF | sim bindings | ...
```

SPEC.md REQ #9 acceptance criterion #2 mandates a recorded human review of the IR shape (the IR-LOCK checkpoint, D-37) before codegen (Plan 04) begins. Any breaking IR change after lock requires a new ADR and a bump to `ir_version: "v1"`.

Mid-phase IR shape churn is explicitly forbidden by D-31 (comment attachment lands early for the same reason — downstream consumers see the final shape from day one).

---

## Decision

IR v0 is defined by two normative artifacts that land together per D-27:

1. **`spec/ir/v0/schema.json`** — normative JSON Schema (draft-2020-12) for the IR Document structure
2. **`spec/ir/v0/README.md`** — Markdown reference explaining ID strategy, span carry-over, relationship graph contract, metadata envelope, diagnostic attachment points, and the backend traversal API

The IR implementation lives in:
- `src/ir.zig` — IR types: `NodeKind`, `EdgeKind`, `IrNode`, `Edge`, `Document` + traversal API
- `src/lowering.zig` — AST + SymbolTable → IR Document lowering pass

Transport across the FFI boundary is **JSON via `deal_ir_json(handle, out_ptr, out_len)`** — a 7th C ABI export following the established length-prefixed UTF-8 pattern (D-11). Plan 04 adds `deal_format` as the 8th export.

### ID strategy (D-23)

Element IDs are **fully-qualified path strings** using `.` as the separator. Example:
`sedan_project.vehicle.electrical.BatteryPack.hvOut`

IDs serve as: (a) `@trace` target literals, (b) SysML v2 `$id` URL suffixes, (c) `.deal/index.json` keys, (d) `find(id)` IR lookup keys. Uniqueness is guaranteed by sema's name-resolution check (one of the 6 blocking checks). The `.` separator is also the Go/gofmt package convention and matches PS-2 dot-paths; literal `.` in identifiers are explicitly not present in the current 19-file showcase.

### Single workspace-wide graph (D-24)

The IR is a single workspace-wide graph. Cross-package references are first-class edges (not stringly-typed path references awaiting deferred resolution). `deal build --target sysml-v2` produces one consolidated output file, matching FS-4 "system-wide index" intent. Per-package output is explicitly deferred to a `--target sysml-v2-per-package` flag in a future phase without requiring an IR shape change.

### Comment-free IR (D-25)

The IR carries NO comment fields. `leading_comments`, `trailing_comments`, and `doc_comment` remain on the AST layer only. `deal fmt` walks the AST (which carries the comment attachment from Plan 02-01); the SysML v2 emitter walks the IR. This mirrors the TypeScript compiler split (AST has trivia, IR doesn't). Future doc-comment lowering to SysML `documentation` ownedRelationship is explicitly deferred to a follow-up ADR.

### Traversal API (D-26 LOCKED)

The backend traversal API is a small, stable surface of five methods:
- `walk(visitor)` — pre/post-order visitor over the entire element graph
- `find(id) -> ?*IrNode` — O(1) lookup by fully-qualified path
- `references(id) -> []const EdgeRef` — incoming edges (indexed for O(1))
- `children(id) -> []const []const u8` — direct children by `contains` edge (indexed)
- `parent(id) -> ?[]const u8` — inferred from qualified path prefix

No query DSL (MQL) in v0. Ad-hoc queries are coded as visitors passed to `walk`.

### JSON envelope (D-22 + D-18)

```json
{
  "edges": [{"dst": "...", "kind": "...", "src": "..."}, ...],
  "elements": {
    "<qualified-path>": {
      "kind": "<NodeKind>",
      "payload": { ...alphabetical... },
      "source_file": "<workspace-relative>",
      "span": [<start_u32>, <end_u32>]
    },
    ...
  },
  "ir_version": "v0",
  "v": 1
}
```

Top-level keys alphabetical: `edges, elements, ir_version, v`. Per-node keys alphabetical: `kind, payload, source_file, span`. Per-edge keys alphabetical: `dst, kind, src`. Payload keys alphabetical per kind. This follows D-18 (alphabetical key ordering) and D-04 (schema-versioned JSON with `"v"` as a top-level field). Hand-rolled emission per RESEARCH §Pitfall 1 (no `std.json.Stringify`).

---

## Alternatives Rejected

### FlatBuffers / shared-struct IR transport (rejected in D-22 discussion)

FlatBuffers would provide zero-copy shared-memory access from Rust without a serialize/deserialize round-trip. **Rejected** because: (a) adds a build-time schema codegen toolchain dependency that conflicts with the "portable without special tooling" requirement; (b) violates D-10's opaque-handle ownership model; (c) FlatBuffer schemas are their own DSL — the IR v0 spec would live in two places; (d) Phase 2's build budget (~3K IR elements, ~5K edges) makes the JSON round-trip cost negligible (<100ms).

### Per-package IR + per-file SysML output (rejected in D-24 discussion)

Per-package IR with one SysML JSON file per package would parallelize lowering and let per-file analysis begin before the whole workspace is loaded. **Rejected** because: (a) cross-package `<<specializes>>` and `@trace:<<satisfies>>` edges would become unresolved string references requiring a second-pass resolver in the SysML emitter; (b) the 19-file showcase is a single system — splitting the output across 7 package-level files misrepresents the system topology; (c) future incremental rebuilds (Phase 3 LSP concern) can operate on the same single-graph shape with per-element dirty-marking.

### MQL query DSL in v0 (rejected in D-26 discussion)

A MQL (model query language) DSL would let Phases 3–6 express declarative queries (`FIND part_def WHERE specializes = "ThermallyManaged"`). **Rejected** because: (a) PROJECT.md has an explicit ADR deferring MQL to a later phase; (b) committing a query DSL in v0 creates a second public contract surface alongside the IR shape itself; (c) visitor-pattern ad-hoc queries cover every codegen and LSP access pattern in Phases 2–4 without DSL overhead.

### AST JSON `"v": 2` schema bump for comment attachment (rejected in D-30 discussion)

Bumping the AST JSON schema version to `v: 2` when adding comment fields would give consumers a clean migration path. **Rejected** because: (a) comment attachment fields are ADDITIVE — existing consumers that don't read `leading_comments`/`trailing_comments`/`doc_comment` continue to work correctly with v1; (b) 19 snapshot files would need regeneration AND version-flag updates in every consumer; (c) the IR is the public contract surface (not the AST JSON), so the only consumer of the AST JSON version bump would be tests.

### Free-floating trivia list (rejected in D-28 discussion)

Storing comments in a token-stream-order trivia list (array indexed by token position) rather than structurally attaching them to declarations would preserve every comment without attachment heuristics. **Rejected** because: (a) the primary consumer of comments is `deal fmt`, which needs to know which comment precedes which declaration to reproduce the source layout; (b) a token-index trivia list forces every consumer to re-derive the parent-declaration relationship; (c) structural attachment per the gofmt blank-line rule is the established convention in Go, Rust (rustfmt), and TypeScript.

### IR carrying doc comments as `documentation` field (rejected in D-25 discussion)

Allowing `/** */` doc comments to flow into the IR as an optional `documentation: ?[]const u8` field on IR nodes would enable the SysML v2 emitter to emit SysML `documentation` ownedRelationships without a separate AST walk. **Rejected** because: (a) it contradicts the compiler-split principle that IR is comment-free; (b) `deal fmt` would need to walk the IR for doc comments AND the AST for `//` line comments, creating two comment-location sources of truth; (c) a follow-up ADR can add `documentation` lowering when SysML integration is mature enough to evaluate the mapping (there is no urgent need in Phase 2 since the showcase's `/** */` content is engineering rationale, not formal SysML documentation).

---

## Consequences

### Positive

- Phases 3–6 have a stable, schema-documented input surface. Any IR consumer can validate its input against `spec/ir/v0/schema.json` before processing.
- The `walk/find/references/children/parent` API covers all Phase 2–4 access patterns without a DSL.
- JSON transport is debuggable (human-readable), consistent with D-04/D-18, and requires no extra toolchain.
- The `ir_version: "v0"` envelope field gives consumers a migration path when v1 ships.
- The IR-LOCK checkpoint (D-37) records the review before codegen begins, preventing mid-phase shape churn.

### Negative / Risks

- Breaking IR changes (new required field, renamed NodeKind) require a new ADR and a `ir_version: "v1"` bump. Every consumer (Rust emitter, LSP, ReqIF emitter, sim bindings) must be updated.
- JSON serialize/deserialize round-trip across the FFI adds ~10–50ms latency per `deal build` invocation for large workspaces (acceptable for Phase 2's showcase; Phase 6+ incremental rebuild is a separate concern).
- Qualified-path IDs are sensitive to package refactoring. Renaming a package segment changes all IDs in that subtree, breaking `@trace` literals in other files (this is a language-level concern, not an IR-level one, but it surfaces here because the IR is where IDs stabilize).

---

## Implementation Notes

The lowering pass consumes the `sema.SymbolTable` (from Plan 02-02's `handle.index_root`) rather than re-resolving names. This is a hard requirement: the sema pass is the single source of truth for which names resolve and which don't. The lowering pass reads `SymbolEntry.id` (the fully-qualified path) and `SymbolEntry.source_file` (workspace-relative path) from the symbol table and carries them verbatim into IR nodes.

Arena allocation: all IR data lives in the per-handle arena (D-02). `deal_free()` releases the arena, which frees every IR node, edge, and interned ID string in one shot.

The `deal_lower_internal` test-only helper is a `pub fn` (no `export`, no `callconv(.c)`) and MUST NOT appear in `nm -gU libdeal.a`. The C ABI surface stays at exactly 7 exports after this plan (Plan 04 adds `deal_format` → 8).
