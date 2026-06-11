---
phase: "02"
plan: "03"
subsystem: ir-lowering
tags: [ir, lowering, ffi, json, zig]
dependency_graph:
  requires: [02-01, 02-02]
  provides: [ir-v0-document, deal_ir_json-export]
  affects: [02-04, 03-lsp, 04-reqif, 05-sim, 06-importers]
tech_stack:
  added: [ir.zig, lowering.zig]
  patterns: [three-pass-lowering, arena-allocation, d-25-comment-free-ir, d-26-traversal-api]
key_files:
  created:
    - src/ir.zig
    - src/lowering.zig
    - tests/unit/determinism_lower_twice.zig
    - tests/unit/property_ir_id_uniqueness.zig
    - .planning/decisions/ADR-deal-ir-v0.md
    - spec/ir/v0/schema.json
    - spec/ir/v0/README.md
  modified:
    - src/json.zig
    - src/lib.zig
    - build.zig
    - include/deal.h
    - cli/src/ffi.rs
    - tests/unit/_all.zig
decisions:
  - "IR is comment-free (D-25): leading_comments/trailing_comments/doc_comment remain on AST only"
  - "Single workspace-wide graph (D-24): cross-package edges are first-class, not stringly-typed deferred refs"
  - "JSON transport via deal_ir_json C ABI (D-22): hand-rolled alphabetical key emission, no std.json.Stringify"
  - "deal_lower_internal is pub fn (no export, no callconv.c) — confirmed absent from nm output"
  - "ADR status remains PROPOSED pending IR-LOCK human review (D-37 checkpoint)"
metrics:
  duration: "~90 minutes (continuation from prior session)"
  completed: "2026-05-21"
  tasks_completed: 2
  files_changed: 14
---

# Phase 02 Plan 03: IR v0 Types, Lowering Pass, and deal_ir_json Summary

IR v0 shape locked in spec + implemented in Zig: 27-NodeKind graph with three-pass lowering (AST+sema→IR), deal_ir_json as export #7, determinism-verified JSON emission.

## Tasks Completed

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | IR v0 spec artifacts (ADR + schema.json + README.md) | 3624ea1 | .planning/decisions/ADR-deal-ir-v0.md, spec/ir/v0/schema.json, spec/ir/v0/README.md |
| 2 | IR types + lowering + FFI | 4f42050 | src/ir.zig, src/lowering.zig, src/json.zig, src/lib.zig, build.zig, include/deal.h, cli/src/ffi.rs, tests/unit/determinism_lower_twice.zig, tests/unit/property_ir_id_uniqueness.zig, tests/unit/_all.zig |

Task 3 (IR-LOCK checkpoint) is a `checkpoint:human-verify` — deferred to the orchestrator per plan design.

## What Was Built

### Task 1 — IR v0 Spec Artifacts

- `.planning/decisions/ADR-deal-ir-v0.md` — ADR with `Status: PROPOSED`, recording design decisions D-22..D-27, 6 rejected alternatives (FlatBuffers, per-package IR, MQL DSL, AST v2 bump, free-floating trivia, IR doc-comment field)
- `spec/ir/v0/schema.json` — normative JSON Schema draft-2020-12 defining the IR Document structure (27 NodeKind values, 11 EdgeKind values, all payload variants, AgentMetadata, SimBinding envelopes)
- `spec/ir/v0/README.md` — Markdown reference covering ID strategy (D-23), span carry-over (D-15), relationship graph contract (D-24), metadata envelope, diagnostic attachment points, traversal API (D-26), JSON envelope (D-22+D-18)

Spec artifacts committed to the `deal-lang/spec` submodule repo at commit `73d41e4`; the submodule gitlink in the main tree updated in commit `3624ea1`.

### Task 2 — IR Types + Lowering + FFI

**src/ir.zig** — IR v0 types:
- `NodeKind` enum: 27 variants (action_def through validate)
- `EdgeKind` enum: 11 variants (allocated_to through traces)
- `IrNode`: id, kind, span, source_file, payload — NO comment fields (D-25 compliant)
- `IrPayload`: name, type_ref, direction, modifiers, agent_metadata, simulation_bindings, requirement_ref
- `AgentMetadata`: assumes, concerns, confidence, rationale
- `SimBinding`: operator, target, tool, equation, fidelity, entry
- `Document` + D-26 LOCKED traversal API: walk, find, references, children, parent

**src/lowering.zig** — three-pass AST → IR lowering:
- Pass 1: intern fully-qualified path IDs (one allocation per unique path)
- Pass 2: lower AST nodes → IrNodes + collect Edge list
- Pass 3: build `incoming_index` (EdgeRef per dst) and `children_index` ([][]const u8 per parent)
- `lowerAnnotations`: parses @confidence, @rationale, @assumes, @concerns, @simulation:<<computes/validates against>>

**src/json.zig** additions — hand-rolled IR JSON emission (D-18 alphabetical, D-22 envelope):
- `emitIrJson`: top-level key order `edges, elements, ir_version, v`
- Edges sorted by (src, dst, kind) for deterministic output (T-02-15)
- Elements sorted alphabetically by ID
- Fixed pre-existing `ArrayList([]const u8).init(allocator)` → `.empty` (Zig 0.16 API)

**src/lib.zig** additions:
- `DealHandle.ir_root: ?*ir.Document` + `DealHandle.cached_ir_json: ?[]const u8`
- `pub export fn deal_ir_json(...)` — 7th C ABI export, lazy lowering + JSON emission
- `pub fn deal_lower_internal(...)` — test-only helper (no `export`, no `callconv(.c)`)

**build.zig** — added `ir` and `lowering` modules; wired: ir←ast, lowering←ast/ir/sema, json←ir, all modules exposed via `addSrcImports`

**include/deal.h** — `deal_ir_json` declaration with full doxygen; E2xxx error code band added

**cli/src/ffi.rs** — `deal_ir_json` extern "C" declaration; comment notes Plan 02-04 adds `deal_format`

**Tests** — `determinism_lower_twice.zig` (byte-equality via public ABI) and `property_ir_id_uniqueness.zig` (no-duplicate-ID property via deal_lower_internal); both registered in `_all.zig`

## Verification Results

| Check | Result |
|-------|--------|
| `zig build` | PASS (exits 0) |
| `cargo build --workspace` | PASS (exits 0) |
| nm export count | 7 `deal_*` exports — PASS |
| deal_lower_internal absent from nm | PASS (0 occurrences) |
| `zig build test` 31/43 | Same as baseline (12 file-read failures are pre-existing worktree issue — spec submodule not initialized) |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Duplicate `directionName` function in json.zig**
- **Found during:** Task 2 first build attempt
- **Issue:** The prior session's IR JSON additions included a new `directionName` function identical to the existing one at line 797. Zig does not allow duplicate struct-scope function names.
- **Fix:** Removed the duplicate at the end of the file; the existing `directionName` at line 797 is already correct and used by both the old payload emission and the new `emitIrPayload`.
- **Files modified:** `src/json.zig`
- **Commit:** 4f42050

**2. [Rule 1 - Bug] `?*ast.Node` passed where `*ast.Node` required in deal_ir_json**
- **Found during:** Task 2 second build attempt
- **Issue:** `h.ast_root` is `?*ast.Node` but `lowering.lower` takes `*ast.Node`. Also `h.index_root` is `?*sema.SymbolTable` but lowering takes `*const sema.SymbolTable`.
- **Fix:** Added `orelse return false` unwrapping for both optional fields before calling `lowering.lower`.
- **Files modified:** `src/lib.zig`
- **Commit:** 4f42050

**3. [Rule 1 - Bug] `deal_lower_internal` passes `?*ast.Node` to `lowering.lower`**
- **Found during:** Task 2, same build pass
- **Issue:** `parser.parseFile` returns `!?*ast.Node`; the result `ast_root_opt` needed unwrapping before passing to `lowering.lower` which takes `*ast.Node`.
- **Fix:** `const ast_root = ast_root_opt orelse return error.ParseFailed;`
- **Files modified:** `src/lib.zig`
- **Commit:** 4f42050

**4. [Rule 1 - Bug] `*[]ir.EdgeRef` indexed directly instead of dereferencing**
- **Found during:** Task 2 third build attempt
- **Issue:** `incoming_map.getPtr(edge.dst)` returns `?*[]ir.EdgeRef`. The code did `slice[idx] = ...` but `slice` is `*[]ir.EdgeRef`, not `[]ir.EdgeRef`. Zig doesn't auto-deref for indexing.
- **Fix:** Changed to `slice_ptr.*[idx] = ...`
- **Files modified:** `src/lowering.zig`
- **Commit:** 4f42050

**5. [Rule 1 - Bug] `ArrayList([]const u8).init(allocator)` not valid in Zig 0.16**
- **Found during:** Task 2 fourth build attempt
- **Issue:** The new code used `.init(allocator)` for `ArrayList([]const u8)` which is not valid in Zig 0.16's explicit-allocator API. Also found the pre-existing `writeIndexJson` used the same invalid pattern.
- **Fix:** Changed both occurrences (lines 1220 and 1390 in json.zig) to `.empty` + explicit allocator on each call (`append(allocator, ...)`, `deinit(allocator)`).
- **Files modified:** `src/json.zig`
- **Commit:** 4f42050

## Known Stubs

None — the IR lowering pass is fully wired. The lowering pass intentionally leaves `@trace` / `@connect` semantics as partial (edges are emitted for what's parseable; cross-file reference resolution is a Plan 06 concern).

## Threat Flags

None. No new network endpoints, auth paths, or file access patterns introduced. The `deal_ir_json` export follows the same NULL-guard and arena-ownership model as `deal_ast_json`.

## Checkpoint: IR-LOCK (Task 3)

Task 3 is `type="checkpoint:human-verify"`. Per plan design and D-37, the IR shape requires a recorded human review before codegen (Plan 04) begins. The executor stops here.

**ADR status is `PROPOSED` — it must NOT be changed to `LOCKED` by the executor.**

The orchestrator must deliver the IR-LOCK checkpoint to the user for review.

## Self-Check: PASSED

Files verified present:
- src/ir.zig: FOUND
- src/lowering.zig: FOUND
- src/json.zig (modified): FOUND
- src/lib.zig (modified): FOUND
- build.zig (modified): FOUND
- include/deal.h (modified): FOUND
- cli/src/ffi.rs (modified): FOUND
- tests/unit/determinism_lower_twice.zig: FOUND
- tests/unit/property_ir_id_uniqueness.zig: FOUND
- .planning/decisions/ADR-deal-ir-v0.md: FOUND
- .planning/phases/02-prove-the-pipeline/02-03-SUMMARY.md: FOUND

Commits verified:
- 3624ea1 (Task 1): FOUND
- 4f42050 (Task 2): FOUND
