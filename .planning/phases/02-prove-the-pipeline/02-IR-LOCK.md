# Phase 2 IR-LOCK Checkpoint Record

**Plan:** 02-03 (IR v0 + lowering)
**Decision recorded:** D-37 — IR-LOCK checkpoint between Plan 03 and Plan 04
**ADR:** [ADR-deal-ir-v0.md](../../decisions/ADR-deal-ir-v0.md)
**Spec artifacts:**
- `spec/ir/v0/schema.json` (normative JSON Schema, draft-2020-12)
- `spec/ir/v0/README.md` (human-readable reference)

## Outcome

**Status:** APPROVED
**Approved on:** 2026-05-21
**Approver:** Project owner (you@example.com)
**Approval channel:** Interactive `/gsd-execute-phase 2` session — AskUserQuestion response "Approved as-is — proceed to Wave 4"

## Surface reviewed at lock

| Element | Count | Values |
|---------|-------|--------|
| `NodeKind` | 27 | action_def, allocation_def, allocate, annotation, attribute_def, attribute_usage, connect, connection_def, constraint_def, expose, flow_def, interface_def, item_def, need_def, package, part_def, part_usage, port_def, port_usage, requirement_def, satisfy, state_def, subsystem, system, traceability_block, use_case_def, validate |
| `EdgeKind` | 11 | allocated_to, carries, connects_via, contains, derives_from, imports, redefines, satisfies, specializes, subsets, traces |
| `IrNode` fields | 5 | `id` (dot-path), `kind`, `span`, `source_file`, `payload` — no comment fields per D-25 |
| `IrPayload` variants | 7 | `name`, `type_ref`, `direction`, `modifiers`, `agent_metadata`, `simulation_bindings`, `requirement_ref` |
| JSON envelope | — | `{ edges, elements, ir_version: "v0", v: 1 }` (alphabetical keys per D-18) |

## Verification at lock

- `zig build` exits 0 on `daa9427`
- `cargo build --workspace` exits 0
- `nm -gU libdeal.a` shows exactly 7 `deal_*` exports (Phase 1's 6 + `deal_ir_json`)
- `deal_lower_internal` absent from public ABI (test-only helper confirmed)
- `determinism_lower_twice.zig`: two lowering passes byte-identical IR JSON via public ABI
- `property_ir_id_uniqueness.zig`: no duplicate IDs across showcase + DEALx
- IR JSON contains no `leading_comments` / `trailing_comments` / `doc_comment` keys (D-25 verified by grep)

## Consequence

The IR v0 shape is **public contract** as of this commit. Any future change to:
- A `NodeKind` or `EdgeKind` value (add/remove/rename)
- An `IrNode` field
- The `IrPayload` shape
- The JSON envelope keys

…requires a NEW ADR and an `ir_version: "v1"` bump. Backwards-compatible additions (e.g. a new payload field on a single NodeKind) MAY land within v0 only if downstream consumers (Phases 3–6) and the schema both accept them — adjudicated by the lead at the time.

## Next

Plans 02-04 (SysML v2 codegen — consumes IR JSON via FFI) and 02-05 (`deal fmt` — consumes AST, not IR) are unblocked.
