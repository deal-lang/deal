# Phase 2: Prove the Pipeline - Pattern Map

**Mapped:** 2026-05-21
**Phase:** 02-prove-the-pipeline
**Files in scope:** 27 (8 modified Zig + 4 new Zig + 5 new Rust + 8 new test/fixture + 2 spec docs)
**Analogs found:** 22 / 27 (5 files have no precedent and are flagged below)

> Read order for planner: this map references files in `deal/src/` and `deal/tests/` exclusively for analogs. RESEARCH.md is referenced for files that have no in-tree precedent.

---

## File Classification (grouped by lane)

| Lane | New/Modified File | Role | Data Flow | Closest Analog | Match |
|------|-------------------|------|-----------|----------------|-------|
| **1. Zig — Semantic Analyzer** | `src/sema.zig` (NEW) | analyzer | AST-walk → diagnostic + symbol-table | `src/parser_deal.zig` (AST walker with diag collector + arena) | role-match |
|  | `tests/unit/sema_corpus.zig` (NEW) | test | parse → check → assert diagnostics | `tests/unit/recovery_corpus.zig` (per-file pin + ADR fallback table) | exact |
|  | `tests/regressions/sema/01..06.deal` (NEW corpus) | fixture | malformed input → diagnostic code | `tests/malformed/m01..m20.*.deal` (Phase 1.5 corpus) | exact |
|  | `src/diagnostics.zig` (MODIFIED — append E2xxx) | analyzer-support | string constants in `Codes` namespace | itself (lines 60-98: E0xxx layout) | exact |
| **2. Zig — IR Builder + JSON transport** | `src/ir.zig` (NEW) | IR-types | tagged-union data model | `src/ast.zig` (Node + NodeKind + Span; arena-allocated) | exact |
|  | `src/lowering.zig` (NEW) | lowering | AST + sema → IR graph | `src/parser_deal.zig` (recursive descent with arena allocation) | role-match |
|  | `src/json.zig` (MODIFIED — add IR emitter + comment fields) | emitter | tree → length-prefixed UTF-8 JSON | itself (lines 28-105: `emitAst`, hand-rolled alphabetical writer) | exact |
|  | `src/lib.zig` (MODIFIED — add `deal_ir_json` + `deal_format`) | C-ABI export | handle → out_ptr/out_len buffer | itself (lines 285-323: `deal_ast_json` / `deal_diagnostics_json`) | exact |
|  | `include/deal.h` (MODIFIED — 2 new symbols) | C-ABI header | declarations + doxygen | itself (lines 229-276: `deal_ast_json` + `deal_diagnostics_json`) | exact |
|  | `tests/unit/determinism_lower_twice.zig` (NEW) | test | parse-lower twice → byte-equal IR JSON | `tests/unit/determinism_parse_twice.zig` | exact |
|  | `tests/unit/property_ir_id_uniqueness.zig` (NEW; planner option) | property | walk IR → assert ID set uniqueness | `tests/unit/property_span_containment.zig` | role-match |
| **3. Zig — Formatter** | `src/fmt.zig` (NEW) | pretty-printer | AST + attached comments → bytes | `src/json.zig` (writer-based serializer with hand-rolled output) | role-match |
|  | `tests/unit/fmt_roundtrip.zig` (NEW) | test | parse → format → parse → AST equality | `tests/unit/parser_deal_snapshot.zig` (per-showcase iteration) | role-match |
| **4. Zig ↔ Rust — C ABI bridge** | `cli/build.rs` (NEW) | build-script | invoke `zig build`, link static, rerun-if-changed | `tests/ffi/build.rs` | exact |
|  | `cli/src/ffi.rs` (NEW) | binding | `extern "C"` decls for 8 symbols | `tests/ffi/src/lib.rs` (6 existing + 2 to add) | exact |
| **5. Rust — SysML v2 Codegen** | `cli/src/sysml_v2.rs` (NEW) | codegen | IR JSON (`serde_json::Value`) → SysML v2 JSON | RESEARCH §Pattern 8 / Example 3 (no in-tree precedent) | **NO ANALOG** |
|  | `tests/golden/sysml-v2/01..08.deal` + `expected.json` (NEW) | fixture | hand-written input + expected output | `tests/snapshots/ast/showcase__*.json` (committed snapshot precedent) | role-match |
| **6. Rust — Offline Schema Validator** | `cli/src/schema_registry.rs` (NEW) | validator | `Retrieve` trait impl + draft-2020-12 validation | RESEARCH §Example 4 (`jsonschema 0.46` `Retrieve` pattern) | **NO ANALOG** |
| **7. Rust — Diagnostic Renderer** | `cli/src/render.rs` (NEW) | renderer | diag JSON → ANSI bytes (`owo-colors` + `anstream`) | `src/json.zig`'s `writeDiagnostic` (lines 804-861) — **inverse direction** | role-match |
| **8. Rust — CLI Shell** | `cli/Cargo.toml` (NEW) | manifest | clap + serde + jsonschema deps | `tests/ffi/Cargo.toml` (workspace-member shape) | role-match |
|  | `deal/Cargo.toml` (NEW workspace root) | manifest | `[workspace]` members | RESEARCH §Pattern 1 (no precedent — `tests/ffi/Cargo.toml` is currently standalone) | **NO ANALOG** |
|  | `cli/src/main.rs` (NEW) | CLI dispatch | `clap` derive → subcommand → ffi call | RESEARCH §Example 1 (no precedent) | **NO ANALOG** |
| **9. Test infrastructure** | `tests/unit/_all.zig` (MODIFIED — register new tests) | umbrella | `comptime { _ = @import(...); }` block | itself (lines 12-37) | exact |
|  | `build.zig` (MODIFIED — `phase-2-gate` + `-fresh` steps) | build-step | step → dependOn(test_step) + shell out to verify script | itself (lines 164-241: `phase-1.5-gate` + `phase-1.5-gate-fresh`) | exact |
|  | `scripts/verify-fresh-worktree.sh` (REUSED) | verification | ephemeral worktree → submodule init → run gate | itself (no edits required — only the build.zig wiring changes) | exact |
| **2-spec docs** | `.planning/decisions/ADR-deal-ir-v0.md` (NEW) | ADR | rationale + alternatives | `.planning/decisions/ADR-phase-1.5-fresh-worktree-verification.md` | role-match |
|  | `spec/ir/v0/schema.json` + `spec/ir/v0/README.md` (NEW) | spec | JSON Schema + Markdown reference | `spec/references/omg-sysml-v2/SysML.json` (target schema shape only — no in-tree authoring precedent) | **NO ANALOG** |

---

## Pattern Assignments

### Lane 1 — Zig Semantic Analyzer

#### `src/sema.zig` (NEW, analyzer, AST-walk → diagnostic + symbol-table)

**Analog:** `src/parser_deal.zig` (arena + diag collector idiom), plus `src/diagnostics.zig` (collector helpers).

**Imports pattern** — mirror parser_deal.zig top of file:
```zig
const std = @import("std");
const ast = @import("ast");
const diagnostics = @import("diagnostics");
// new module wires into build.zig createSrcModules() exactly like parser_deal.zig
```

**Per-handle arena allocation pattern** (copy from `src/parser_deal.zig:361-382` `parseFile` entry point):
```zig
pub fn analyze(
    arena: std.mem.Allocator,
    root: *ast.Node,
    diag_list: *std.ArrayList(diagnostics.Diagnostic),
) !?*SymbolTable {
    var s = Analyzer{
        .arena = arena,
        .diags = diag_list,
        .symbols = .empty,
    };
    return s.runAllChecks(root) catch |err| {
        std.debug.assert(err == error.OutOfMemory);
        // OOM diagnostic shape mirrors parseFile's E0004 emission
    };
}
```

**Diagnostic emission pattern** (copy from `src/diagnostics.zig:123-189` `DiagnosticCollector`):
```zig
// Sema-side usage — DiagnosticCollector is already arena-aware
var collector = diagnostics.DiagnosticCollector.init(diag_list, arena);
try collector.emitFmt(
    Codes.e_name_not_found,  // NEW constant in src/diagnostics.zig
    .err,
    span,
    "name `{s}` not found in scope",
    .{name},
);
```

**Two-pass walk pattern** — described in RESEARCH §Pattern 4 (Pass A: collect; Pass B: resolve). No exact in-tree analog because the parser is single-pass; planner should treat this as new structure but reuse the arena + `DiagnosticCollector` idioms above.

**E2xxx code declaration pattern** (copy from `src/diagnostics.zig:60-98` Codes namespace):
```zig
// In src/diagnostics.zig — append AFTER the existing E0xxx block:
//   Semantic (E2000..E2499) — Phase 2 D-33 allocation
//   E2000..E2099 — name resolution
pub const e_name_not_found = "E2000";
pub const e_ambiguous_name = "E2001";
// ... each constant carries a one-line doc comment per the existing style at line 61-97
```

---

#### `tests/unit/sema_corpus.zig` (NEW, test)

**Analog:** `tests/unit/recovery_corpus.zig` — per-file pin table with two-outcome (impl OR ADR) disposition is the closest pattern.

**Per-file pin pattern** (copy from `tests/unit/recovery_corpus.zig:41-78` `Pin` struct + `m_pins` array):
```zig
const Pin = struct {
    name: []const u8,
    expected_code: ?[]const u8,
    adr_basename: ?[]const u8 = null,
};

const sema_pins = [_]Pin{
    .{ .name = "01-name-resolution.deal", .expected_code = Codes.e_name_not_found },
    .{ .name = "02-type-check.deal",      .expected_code = Codes.e_type_mismatch },
    .{ .name = "03-multiplicity.deal",    .expected_code = Codes.e_multiplicity_violation },
    .{ .name = "04-specializes.deal",     .expected_code = Codes.e_specialization_cycle },
    .{ .name = "05-trace.deal",           .expected_code = Codes.e_trace_target_not_found },
    .{ .name = "06-import.deal",          .expected_code = Codes.e_unresolved_import },
};
```

**Code-presence assertion helper** (copy from `tests/unit/strictness_m08_m18_m19.zig:48-66` `countCode` + `firstWithCode`).

**Showcase iteration loop** — copy from `tests/unit/recovery_corpus.zig` (full file). Drive the parser via `deal_parse_internal` (test-only helper at `src/lib.zig:173-242`) so leaks are caught by `std.testing.allocator`.

---

#### `tests/regressions/sema/01..06.deal` (NEW fixture corpus)

**Analog:** `tests/malformed/m01..m20.*.deal` — 6 hand-written `.deal` files, each crafted so the first emitted diagnostic is a known E2xxx code. The shape contract is:
- One file per blocking check.
- Each file is otherwise minimal — a header + package + the smallest construct that trips the check.
- Filename encodes the check number (gives loop-friendly iteration).
- The file IS broken by construction; a future change that "fixes" it should fail the corpus test.

---

#### `src/diagnostics.zig` (MODIFIED — add E2xxx Codes block + update D-16 doc comment)

**Analog:** itself. The E0xxx block (lines 60-98) shows the exact shape to extend.

**Insertion pattern** — append a new namespaced block AFTER `h_did_you_mean`:
```zig
// Semantic — E2000..E2499 (Phase 2 D-33; new band, never renumbered)
// Each constant cites its semantic check + the SPEC.md acceptance fixture
// per CONTEXT.md D-33.

// E2000..E2099 — Check #1 name resolution
pub const e_name_not_found = "E2000";          // 01-name-resolution.deal
pub const e_ambiguous_name = "E2001";
// ...

// E2100..E2199 — Check #2 type checking
pub const e_type_mismatch = "E2100";
// ...
```

**D-16 doc comment update** (lines 7-13): extend the header comment block with the new ranges to match the existing convention.

---

### Lane 2 — Zig IR Builder & JSON Transport

#### `src/ir.zig` (NEW, IR types + traversal API)

**Analog:** `src/ast.zig` — tagged-union over kinds with span on every node, arena-allocated. The shape is identical except IR nodes carry `id: []const u8` (fully-qualified path) and are participants in a graph (edges in a sibling adjacency list per RESEARCH §Pattern 6).

**Tagged-union pattern** (copy structure from `src/ast.zig:14-50`):
```zig
const std = @import("std");
const ast = @import("ast");

pub const Span = ast.Span;  // reuse extern struct from ast.zig — same FFI shape

pub const IrNode = struct {
    id: []const u8,             // fully-qualified path per D-23, arena-owned
    kind: NodeKind,
    span: Span,
    source_file: []const u8,    // arena-owned, for diagnostic attach
    payload: Payload,
};

pub const NodeKind = enum {
    package,
    part_def, port_def, action_def, state_def,
    requirement_def, constraint_def, attribute_def,
    // ... see RESEARCH §Pattern 6 for the full list
};

pub const Edge = struct {
    src: []const u8,
    dst: []const u8,
    kind: EdgeKind,
};
```

**Traversal API surface** (D-26 LOCKED — `walk` / `find` / `references` / `children` / `parent`):
```zig
pub const Document = struct {
    elements: std.StringHashMap(*IrNode),
    edges: []Edge,
    incoming_index: std.StringHashMap([]EdgeRef),  // built once at lower-end
    children_index: std.StringHashMap([]const u8),

    pub fn walk(self: *const Document, visitor: anytype) !void { ... }
    pub fn find(self: *const Document, id: []const u8) ?*IrNode { ... }
    pub fn references(self: *const Document, id: []const u8) []const EdgeRef { ... }
    pub fn children(self: *const Document, id: []const u8) []const []const u8 { ... }
    pub fn parent(self: *const Document, id: []const u8) ?[]const u8 { ... }
};
```

No in-tree precedent for `walk(visitor)` — RESEARCH §Pattern 6 is the only reference.

---

#### `src/lowering.zig` (NEW, lowering pass)

**Analog:** `src/parser_deal.zig` — recursive-descent style that allocates into the parser arena. The lowering walks the AST in tree order and allocates IR nodes into the same arena (D-02 per-handle arena).

**Arena reuse pattern** (no new arena — extend `lib.zig:DealHandle` with `ir_root: ?*ir.Document` allocated in the existing handle arena, exactly like `ast_root`).

**ID interning pattern** (RESEARCH §Pattern 7): use `std.StringHashMap([]const u8)` to dedupe fully-qualified path strings. No in-tree precedent — closest is `std.ArrayList`-of-symbols pattern in parser_deal but it does not intern.

**Determinism contract** — same shape as `tests/unit/determinism_parse_twice.zig`: two lowerings of identical AST input MUST produce byte-identical IR JSON. The determinism test in Lane 2 enforces this.

---

#### `src/json.zig` (MODIFIED — add IR emitter + comment fields)

**Analog:** itself. Two patterns to copy:

**Top-level envelope pattern** (copy from `src/json.zig:28-55` `emitAst`):
```zig
pub fn emitIrJson(
    allocator: std.mem.Allocator,
    doc: *const ir.Document,
) ![]const u8 {
    var buf: std.ArrayList(u8) = .empty;
    defer buf.deinit(allocator);

    try buf.appendSlice(allocator, "{\"v\":1,\"edges\":");  // D-04 + D-18 alpha order
    // ... emit edges array, then elements object, then ir_version
    try buf.append(allocator, '}');
    return try buf.toOwnedSlice(allocator);
}
```

**Alphabetical-keyed object pattern** (copy from `src/json.zig:804-861` `writeDiagnostic` — note how every field is written in alphabetical sequence with hand-coded `","key":` separators; this is the D-18 invariant). Pitfall 5 of RESEARCH explicitly forbids `std.json.Stringify` because it can reorder fields — DO NOT switch the IR emitter to it. RESEARCH §Example 2 shows a `std.json.writeStream` variant, but the EXISTING `src/json.zig` chose the hand-rolled approach for D-18 stability; the IR emitter MUST follow the same hand-rolled shape.

**Comment-field extension** — for `leading_comments` / `trailing_comments` / `doc_comment`:
- The fields appear inside each declaration's payload, alphabetically positioned.
- Empty arrays/null are still emitted (consistent with existing payload shape).
- Snapshot regen happens in a dedicated commit per D-30; only these new fields should appear in the diff.

**String escape helper** — reuse `appendJsonStringEscaped` at `src/json.zig:1143-1173` — it already handles all RFC 8259 cases including UTF-8 passthrough.

---

#### `src/lib.zig` (MODIFIED — add `deal_ir_json` + `deal_format` exports)

**Analog:** itself. `deal_ast_json` (lines 285-302) is a near-exact template for `deal_ir_json`; `deal_diagnostics_json` (lines 306-323) shows the same caching pattern. Copy structure for both new exports:

```zig
// Lazy IR-JSON emission (D-22). Same caching contract as deal_ast_json.
pub export fn deal_ir_json(
    handle_ptr: ?*anyopaque,
    out_ptr: *[*]const u8,
    out_len: *usize,
) callconv(.c) bool {
    const p = handle_ptr orelse return false;
    const h = handleCast(p);
    if (h.cached_ir_json == null) {
        // Lazy lowering: if ir_root is null, lower from ast_root first.
        if (h.ir_root == null and h.ast_root != null) {
            h.ir_root = lowering.lower(h.arena.allocator(), h.ast_root.?) catch null;
        }
        const buf = json.emitIrJson(h.arena.allocator(), h.ir_root) catch {
            return false;
        };
        h.cached_ir_json = buf;
    }
    const cached = h.cached_ir_json.?;
    out_ptr.* = cached.ptr;
    out_len.* = cached.len;
    return true;
}
```

**`DealHandle` extension** (copy field-pattern from `src/lib.zig:36-45`):
```zig
pub const DealHandle = struct {
    arena: std.heap.ArenaAllocator,
    ast_root: ?*ast.Node = null,
    ir_root: ?*ir.Document = null,        // NEW
    mode: ast.Mode = .deal,
    source: []const u8 = "",
    filename: []const u8 = "",
    diagnostics: std.ArrayList(diagnostics.Diagnostic) = .empty,
    cached_ast_json: ?[]const u8 = null,
    cached_diag_json: ?[]const u8 = null,
    cached_ir_json: ?[]const u8 = null,   // NEW
    cached_fmt_bytes: ?[]const u8 = null, // NEW
};
```

**`deal_format` shape** — same out_ptr/out_len contract as the JSON accessors; result is canonical source bytes (UTF-8, no NUL), arena-owned, freed by `deal_free`. CONTEXT.md §"Claude's Discretion" item leans toward "returns bytes" — match `deal_ast_json` exactly.

---

#### `include/deal.h` (MODIFIED — add 2 declarations + doxygen)

**Analog:** itself. Lines 229-276 are the exact doxygen template for `deal_ast_json` + `deal_diagnostics_json` — copy the doc block format verbatim for `deal_ir_json` and `deal_format`. Update the error-code reference block (lines 37-68) with the E2000-E2499 band.

**Surface count assertion** — Phase 1.5 established a `nm -gU libdeal.a | wc -l == 6` grep gate (referenced in CONTEXT.md §Specifics). Phase 2's gate raises this to `== 8`. The build.zig phase-2-gate should mirror this check.

---

#### `tests/unit/determinism_lower_twice.zig` (NEW)

**Analog:** `tests/unit/determinism_parse_twice.zig` (full file). Copy verbatim with three changes:
1. Replace `lib.deal_ast_json` calls with `lib.deal_ir_json`.
2. Replace test name to `determinism.lower_twice`.
3. Update the file doc comment to cite REQ #3 acceptance (per CONTEXT.md §Specifics: "Two byte-equal lowering passes of the same source produce byte-identical IR").

**Critical contract** (from `tests/unit/determinism_parse_twice.zig:14-28`): use the PUBLIC C ABI (`deal_parse` + `deal_ir_json` + `deal_free`), NOT the test-only internal helper. The buffer lifetime contract (dupe the first parse's bytes before freeing handle A) is identical.

---

### Lane 3 — Zig Formatter

#### `src/fmt.zig` (NEW, pretty-printer)

**Analog:** `src/json.zig` (writer-based serializer). The shape is a hand-rolled byte emitter walking a tree. The fmt printer walks the AST (not the IR per D-25) and emits canonical source bytes.

**Writer pattern** (copy from `src/json.zig` `emitAst` + `writeNode`):
- Function signature `emitFormatted(allocator, root) ![]const u8` — same as `emitAst`.
- Internal helpers take `(allocator, *std.ArrayList(u8), node)` — same as `writeNode`.
- Final `toOwnedSlice` returns arena-owned bytes — same lifetime contract.

**Comment-walk pattern** — at every declaration node:
1. Emit `leading_comments[]` first.
2. Emit `doc_comment` (if present).
3. Emit the declaration body.
4. Emit `trailing_comments[]` after.

Per RESEARCH §Pattern 3: "Phase 2 simplification: because D-28 attaches comments STRUCTURALLY to AST nodes (not as missed spans), the printer never needs to scan the original source for 'missed' comments." This means there is no in-tree analog for the comment-walk specifically (Phase 1 has no formatter); the pattern is dictated by D-28 attachment + RESEARCH §Pattern 3.

**Round-trip invariants** (RESEARCH §Pattern 3): one space around binaries, four-space indent, exactly one blank line between top-level decls, normalize quotes per LM-3, NEVER introduce trailing commas (E0122 would fire on re-parse).

---

#### `tests/unit/fmt_roundtrip.zig` (NEW, round-trip test)

**Analog:** `tests/unit/parser_deal_snapshot.zig` (full file) + `tests/unit/determinism_parse_twice.zig` (byte-equality assertion).

**Iteration pattern** (copy from `parser_deal_snapshot.zig:54-95` — iterate the 19-file showcase, parse each, format, re-parse, compare).

**AST equality assertion** — pseudo-code:
```zig
const ast_a = parse(source);
const formatted = fmt.emitFormatted(arena_alloc, ast_a);
const ast_b = parse(formatted);
// Assert byte-equal AST JSON (which proves structural identity).
try std.testing.expectEqualStrings(
    try json.emitAst(gpa, ast_a, .deal, rel),
    try json.emitAst(gpa, ast_b, .deal, rel),
);
// Assert every comment from ast_a survives in ast_b at its original attachment point.
// (Comment preservation is REQ #7 acceptance — extend with a walker that compares
// leading_comments/trailing_comments/doc_comment counts and spans.)
```

The 19-file iteration list is identical to `parser_deal_snapshot.zig:20-36` + `parser_dealx_snapshot.zig` (4 .dealx files).

---

### Lane 4 — Zig ↔ Rust C ABI bridge

#### `cli/build.rs` (NEW, Cargo build script)

**Analog:** `tests/ffi/build.rs` — exact match. Copy verbatim with a single adjustment (the path levels back to `deal/` are different — `cli/` is one level below `deal/`, not two like `tests/ffi/`).

**Full template** (from `tests/ffi/build.rs:17-50`):
```rust
fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR unset");
    // cli/ is one level below deal/ — adjust `..` count vs tests/ffi/build.rs
    let deal_dir = PathBuf::from(&manifest_dir)
        .join("..")
        .canonicalize()
        .expect("failed to canonicalize deal/ directory");

    let status = Command::new("zig")
        .arg("build")
        .current_dir(&deal_dir)
        .status()
        .expect("failed to invoke `zig build` — is zig 0.16.0 on PATH?");
    assert!(status.success(), "`zig build` failed in {}", deal_dir.display());

    let lib_dir = deal_dir.join("zig-out").join("lib");
    println!("cargo::rustc-link-search=native={}", lib_dir.display());
    println!("cargo::rustc-link-lib=static=deal");

    let src_dir = deal_dir.join("src");
    let include_dir = deal_dir.join("include");
    println!("cargo::rerun-if-changed={}", src_dir.display());
    println!("cargo::rerun-if-changed={}", include_dir.display());
    println!("cargo::rerun-if-changed={}/build.zig", deal_dir.display());
}
```

**Cargo 1.77+ namespace** (`cargo::rustc-...` double-colon) — keep, established in `tests/ffi/build.rs`.

---

#### `cli/src/ffi.rs` (NEW, hand-written FFI bindings)

**Analog:** `tests/ffi/src/lib.rs` — exact match for the existing 6 symbols; extend with 2 new declarations.

**Full extern block** (extend `tests/ffi/src/lib.rs:17-42`):
```rust
#![allow(non_camel_case_types)]

#[repr(C)]
pub struct DealHandle {
    _opaque: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

extern "C" {
    pub fn deal_parse(/* ... unchanged from Phase 1 ... */) -> *mut DealHandle;
    pub fn deal_free(handle: *mut DealHandle);
    pub fn deal_has_errors(handle: *mut DealHandle) -> bool;
    pub fn deal_diagnostics_count(handle: *mut DealHandle) -> u32;
    pub fn deal_ast_json(/* ... */) -> bool;
    pub fn deal_diagnostics_json(/* ... */) -> bool;

    // Phase 2 additions (mirror deal_ast_json signature):
    pub fn deal_ir_json(
        handle: *mut DealHandle,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    ) -> bool;

    pub fn deal_format(
        handle: *mut DealHandle,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    ) -> bool;
}
```

**`!Send` / `!Sync` marker** — preserve from `tests/ffi/src/lib.rs:14`. The handle remains thread-affine per D-13.

**Decision per RESEARCH "Alternatives Considered":** stick with hand-written bindings for Phase 2 (8 symbols only); `bindgen` becomes worthwhile if `deal.h` grows further.

---

### Lane 5 — Rust SysML v2 Codegen

#### `cli/src/sysml_v2.rs` (NEW, IR → SysML v2 JSON)

**NO IN-TREE ANALOG.** Closest external pattern is RESEARCH §Pattern 8 (mapping table) + RESEARCH §Example 3 (UUID-from-path code excerpt). Planner should treat this file as new construction guided by RESEARCH.

**Reference patterns to follow:**

1. **`serde_json::Map` for alphabetical keys** — RESEARCH §Example 3 (verbatim). `Map<String, Value>` backed by `BTreeMap` is alphabetical by default; DO NOT enable `preserve_order` feature.

2. **UUID-from-path** (RESEARCH §Example 3):
```rust
const SYSML_NAMESPACE: Uuid = Uuid::from_u128(0x0e72_4d54_7e1a_4b32_9e8c_8e62_2d8f_1234);
fn deal_id_to_uuid(qualified_path: &str) -> Uuid {
    Uuid::new_v5(&SYSML_NAMESPACE, qualified_path.as_bytes())
}
```

3. **Mapping table** — copy from RESEARCH §Pattern 8 directly into a match-based dispatcher:
```rust
fn emit_element(elem: &IrElement) -> serde_json::Value {
    match elem.kind {
        IrKind::PartDef       => emit_part_definition(elem),
        IrKind::PortDef       => emit_port_definition(elem),
        IrKind::PortUsage     => emit_port_usage(elem),
        IrKind::RequirementDef => emit_requirement_definition(elem),
        // ... full table at RESEARCH lines 625-640
    }
}
```

4. **Single-file workspace output (D-24)** — one `showcase.sysml-v2.json` per `deal build`. RESEARCH §System Architecture Diagram makes this explicit.

**SysON convention** (RESEARCH §Pattern 8): `@id` is a synthesized UUID v4, `elementId` = `@id`, `qualifiedName` carries the DEAL `.`-path with `::` substitution.

---

#### `tests/golden/sysml-v2/01..08.*` (NEW corpus)

**Analog:** `tests/snapshots/ast/showcase__*.json` is the closest committed-snapshot precedent. The shape contract for Phase 2 golden fixtures:

- 5-8 paired files: `<id>-<name>.deal` (or `.dealx` for #8) + `<id>-<name>.expected.json`.
- File names per CONTEXT.md §Specifics: `01-part-def.deal`, `02-port-usage.deal`, `03-specialization.deal`, `04-attribute-usage.deal`, `05-requirement-def.deal`, `06-trace-link.deal`, `07-package.deal`, `08-dealx-composition.dealx`.
- Each `.deal` is hand-written, NOT generated.
- Each `expected.json` is hand-written, NOT regenerated from current output.
- Both byte-exact match AND schema-valid assertions per REQ #6.

**Test driver** — see Lane 5 driver below (`cli/tests/golden_sysml_v2.rs`).

---

### Lane 6 — Rust Offline Schema Validator

#### `cli/src/schema_registry.rs` (NEW, jsonschema Retrieve impl)

**NO IN-TREE ANALOG.** Closest external pattern is RESEARCH §Example 4 (full template at lines 928-972).

**Full template** (RESEARCH §Example 4 — copy verbatim and adapt path):
```rust
use jsonschema::Retrieve;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct LocalBundleRetriever {
    bundle: HashMap<String, Value>,
}

impl LocalBundleRetriever {
    pub fn new(spec_references_dir: &PathBuf) -> anyhow::Result<Self> {
        let mut bundle = HashMap::new();
        let sysml: Value = serde_json::from_slice(
            &std::fs::read(spec_references_dir.join("omg-sysml-v2/SysML.json"))?
        )?;
        let kerml: Value = serde_json::from_slice(
            &std::fs::read(spec_references_dir.join("omg-kerml-v1/KerML.json"))?
        )?;
        index_schema_ids(&sysml, &mut bundle);
        index_schema_ids(&kerml, &mut bundle);
        Ok(Self { bundle })
    }
}

impl Retrieve for LocalBundleRetriever {
    fn retrieve(&self, uri: &jsonschema::Uri<String>)
        -> Result<Value, Box<dyn std::error::Error + Send + Sync>>
    {
        self.bundle.get(uri.as_str()).cloned()
            .ok_or_else(|| format!("Schema not in offline bundle: {uri}").into())
    }
}
```

**Cargo features** (RESEARCH §Standard Stack line 175): `cargo add jsonschema --no-default-features` — explicitly NO `reqwest` pulled in. Offline-by-construction.

**Fallback crate** (RESEARCH alternatives row): if `jsonschema` 0.46 hits a perf/correctness issue on the 126K-line `SysML.json`, `boon` 0.6 is the documented fallback.

---

### Lane 7 — Rust Diagnostic Renderer

#### `cli/src/render.rs` (NEW, diagnostic JSON → ANSI text)

**Analog (inverse direction):** `src/json.zig:804-861` `writeDiagnostic` defines the canonical Diagnostic JSON shape; the renderer's job is to consume that shape and emit human-readable bytes. Role-match — same data, opposite direction.

**Diagnostic shape to consume** (from `src/json.zig:776-784` doc comment):
```
{
  "code": "<E/W/H code>",
  "fix_it": null | { "replace_span": [s, e], "replacement": "<text>" },
  "message": "<human-readable>",
  "notes": "<text>",
  "secondary_spans": [{"label": "...", "span": [s, e]}, ...],
  "severity": "err" | "warn" | "info" | "hint",
  "span": [start, end]
}
```

**Decision per RESEARCH §Alternatives row** (lines 162-166): hand-roll rather than pull `codespan-reporting` / `miette` / `ariadne`. Use `owo-colors` for styling + `anstream` for TTY-aware writes:

```rust
// Pseudo-code shape:
use owo_colors::OwoColorize;
use anstream::AutoStream;

pub fn render_diagnostic(
    out: &mut AutoStream<std::io::Stderr>,
    source: &str,
    diag: &Diagnostic,
) -> std::io::Result<()> {
    let prefix = match diag.severity.as_str() {
        "err" => format!("{}: ", "error".red().bold()),
        "warn" => format!("{}: ", "warning".yellow().bold()),
        _ => format!("{}: ", diag.severity.as_str()),
    };
    writeln!(out, "{}{} [{}]", prefix, diag.message, diag.code)?;
    // ... emit source snippet + carets per Phase 1 diagnostic shape
}
```

**Anti-pattern to avoid** (RESEARCH §Anti-Patterns): never use `println!` — always go through `anstream::stdout()` / `anstream::stderr()` so `--color={auto,always,never}` works correctly on non-TTY destinations.

---

### Lane 8 — Rust CLI Shell

#### `cli/Cargo.toml` (NEW)

**Analog:** `tests/ffi/Cargo.toml` (workspace-member precedent; specifically the `links = "deal"` field which Cargo uses for de-dup of `-l deal` across workspace crates).

**Cross-cutting** — this crate becomes a workspace member alongside the existing `tests/ffi/`. The new workspace root `deal/Cargo.toml` is a CLAUDE'S DISCRETION item per CONTEXT.md; RESEARCH §Pattern 1 recommends a workspace with members `["cli", "tests/ffi"]`.

**Critical field — `links`:** `tests/ffi/Cargo.toml:10` sets `links = "deal"`. Cargo enforces "exactly one crate in a dependency graph claims a given native library." If both `cli/` and `tests/ffi/` set `links = "deal"`, Cargo refuses to build. Resolution options:
- (a) only `tests/ffi/` sets `links = "deal"`, and `cli/` depends on `tests/ffi/` to inherit the link.
- (b) move `links = "deal"` to a dedicated `deal-ffi/` crate that both `cli/` and `tests/ffi/` depend on.
- (c) drop the `links` field from `tests/ffi/` and let `cli/` claim it.

Planner should pick one of these in Plan 02-01; RESEARCH does not lock the choice. **Recommended**: option (b) — `cli/src/ffi.rs` becomes its own published binding crate.

**Dependency set** (RESEARCH §Standard Stack, lines 119-131): pin in workspace root, reference from `cli/Cargo.toml`:
```toml
[dependencies]
clap = { workspace = true, features = ["derive"] }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
jsonschema = { workspace = true }
anstream = { workspace = true }
owo-colors = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
insta = { workspace = true, features = ["json"] }
```

---

#### `deal/Cargo.toml` (NEW workspace root)

**NO IN-TREE ANALOG.** RESEARCH §Pattern 1 lines 384-397 is the only reference.

**Full template** (RESEARCH §Pattern 1):
```toml
[workspace]
resolver = "2"
members = ["cli", "tests/ffi"]

[workspace.dependencies]
clap = { version = "4.6", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
jsonschema = { version = "0.46", default-features = false }
anstream = "1"
owo-colors = "4"
anyhow = "1"
thiserror = "2"
insta = { version = "1.47", features = ["json"] }
```

**Lock file location** — per RESEARCH §Pattern 1: commit `deal/Cargo.lock` (workspace root). This replaces the existing `tests/ffi/Cargo.lock` — verify with planner.

---

#### `cli/src/main.rs` (NEW, clap dispatch)

**NO IN-TREE ANALOG.** RESEARCH §Example 1 (lines 808-866) is the only reference.

**Full template** (RESEARCH §Example 1 — copy verbatim and extend with `run(cli)` body):
```rust
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "deal", version, about = "DEAL language compiler driver")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true, default_value = "auto")]
    color: ColorMode,
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Command {
    Parse { paths: Vec<std::path::PathBuf> },
    Check { paths: Vec<std::path::PathBuf> },
    Fmt { paths: Vec<std::path::PathBuf> },
    Build {
        #[arg(long)]
        target: BuildTarget,
        #[arg(long)]
        validate: bool,
        paths: Vec<std::path::PathBuf>,
    },
}

#[derive(ValueEnum, Clone)] enum BuildTarget { SysmlV2 }
#[derive(ValueEnum, Clone, Copy)] enum ColorMode { Auto, Always, Never }

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) if e.is_user() => std::process::ExitCode::from(1),  // D-34
        Err(_) => std::process::ExitCode::from(2),                  // D-34
    }
}
```

**Exit-code policy (D-34)** — `0` success / `1` user error / `2` internal error. The `run` function returns a `Result<(), CliError>` with an `is_user()` discriminator method.

**`--json` envelope assembly (D-32)** — when `--json` is set, the Rust CLI wraps Phase 1's existing `deal_diagnostics_json` output in:
```json
{ "v": 1, "command": "check", "deal_version": "<x>",
  "diagnostics": [...], "summary": { "errors": N, "warnings": N, "hints": N } }
```
Per CONTEXT.md D-32 / RESEARCH §Don't-Hand-Roll line 687: pass through Zig's diagnostics JSON verbatim and only wrap the envelope around it. Do NOT re-serialize Phase 1's diagnostic shape from typed Rust structs (key-order drift risk).

---

### Lane 9 — Test infrastructure

#### `tests/unit/_all.zig` (MODIFIED — register new tests)

**Analog:** itself. The pattern is a single `comptime { _ = @import("xxx.zig"); }` block (lines 12-37). Append the new test files:
```zig
_ = @import("determinism_lower_twice.zig");  // NEW
_ = @import("fmt_roundtrip.zig");            // NEW
_ = @import("sema_corpus.zig");              // NEW
// _ = @import("property_ir_id_uniqueness.zig");  // OPTIONAL per planner
```

---

#### `build.zig` (MODIFIED — add `phase-2-gate` + `phase-2-gate-fresh` steps)

**Analog:** itself. Lines 164-241 are the exact template — `phase-1.5-gate` + `phase-1.5-gate-fresh` show the wiring.

**Phase-2 gate step** (copy structure from `build.zig:164-208`):
```zig
const gate_2_step = b.step("phase-2-gate", "Run the full Phase 2 exit-gate Zig test suite");
gate_2_step.dependOn(gate_1_5_step);  // inherit Phase 1.5 (per D-35 — extends, not replaces)
gate_2_step.dependOn(test_step);
// NOTE: the Rust-side Phase 2 gate (cargo test --workspace) cannot live inside
// build.zig — see the existing build.zig:152-156 comment block for the same
// rationale Phase 1's gate used.
```

**Phase-2 fresh gate** (copy structure from `build.zig:230-240`):
```zig
const gate_2_fresh_step = b.step(
    "phase-2-gate-fresh",
    "Run phase-2-gate inside a freshly-created ephemeral worktree with submodule init (per ADR-phase-1.5-fresh-worktree-verification)",
);
const run_fresh_2 = b.addSystemCommand(&[_][]const u8{
    "bash",
    "scripts/verify-fresh-worktree.sh",
    "phase-2-gate",  // pass the gate step name as arg — script needs a small extension to accept it
});
gate_2_fresh_step.dependOn(&run_fresh_2.step);
```

**Script extension** — `scripts/verify-fresh-worktree.sh` currently hard-codes `zig build phase-1.5-gate` (line 105). Plan 02-06 should generalize it to accept a gate-step name argument so the same script serves all future `-fresh` siblings (Phase 2 + 3 + ...).

---

#### `scripts/verify-fresh-worktree.sh` (REUSED with minor extension)

**Analog:** itself. The ephemeral-worktree + submodule-init + run-gate machinery (lines 65-118) needs only one change: replace the hard-coded `zig build phase-1.5-gate` with a parameter. The existing dirty-tree check, EXIT trap, and showcase-materialization sanity check (lines 90-97) are preserved verbatim.

---

### Lane "spec docs"

#### `.planning/decisions/ADR-deal-ir-v0.md` (NEW)

**Analog:** `.planning/decisions/ADR-phase-1.5-fresh-worktree-verification.md` (referenced by CONTEXT.md §canonical_refs).

**Shape contract** (D-27):
- Rationale (the why).
- Alternatives rejected (this discussion's options — at minimum: FlatBuffers transport rejected; per-package IR rejected; MQL query DSL deferred).
- Links to the normative spec (`spec/ir/v0/schema.json` + `spec/ir/v0/README.md`).

**The discussion log** (`.planning/phases/02-prove-the-pipeline/02-DISCUSSION-LOG.md`) is the source for alternatives rejected.

---

#### `spec/ir/v0/schema.json` + `spec/ir/v0/README.md` (NEW)

**NO IN-TREE ANALOG.** Closest reference shape is `spec/references/omg-sysml-v2/SysML.json` (the target schema), but DEAL has never authored its own JSON Schema before.

**Schema authoring guidance:**
- JSON Schema draft-2020-12 (matches OMG schemas and `jsonschema` 0.46 default).
- Cover every IR `NodeKind` listed in RESEARCH §Pattern 6.
- Provide envelope, then `$defs` for each kind's payload.
- Loadable by any standard JSON-Schema validator — useful for downstream consumer validation.

**README authoring guidance** (CONTEXT.md D-27 list):
- ID strategy (per D-23): fully-qualified path strings with `.` separator.
- Span carry-over from AST (per D-15: u32 byte offsets).
- Relationship graph contract (per RESEARCH §Pattern 6: edges in adjacency list, not inline).
- Metadata envelope shape (`@confidence` / `@rationale` / `@assumes` / `@concerns` — see RESEARCH §Pattern 6 "agent_metadata" sub-section).
- Diagnostic attachment points (span carries back to AST source position).
- Backend traversal API contract (per D-26: `walk` / `find` / `references` / `children` / `parent`).

Both files live in the `spec/` submodule (which `tests/showcase/` already symlinks into); Phase 2 lands them via a submodule commit that the deal repo bumps.

---

## Shared Patterns (cross-cutting)

### Pattern S-1: Per-handle arena allocation (D-02 LOCKED)

**Source:** `src/lib.zig:36-45` (`DealHandle` struct) + `src/lib.zig:73-160` (`deal_parse` arena-init flow) + `src/lib.zig:259-264` (`deal_free`).

**Apply to:** All NEW Zig modules (`sema.zig`, `ir.zig`, `lowering.zig`, `fmt.zig`). Every allocation goes through `handle.arena.allocator()`. No new arenas. Single `deal_free` releases everything.

**Concrete excerpt** (lines 92-102):
```zig
var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
const init_alloc = arena.allocator();
const handle = init_alloc.create(DealHandle) catch { arena.deinit(); return null; };
handle.* = .{ .arena = arena, .source = "", .filename = "" };
const alloc = handle.arena.allocator();  // ALL subsequent allocs go through this
```

---

### Pattern S-2: Alphabetical-key JSON emission (D-18 LOCKED)

**Source:** `src/json.zig:42-55` (`emitAst` envelope) + `src/json.zig:804-861` (`writeDiagnostic` showing alphabetical field order).

**Apply to:** Every JSON producer — `src/json.zig` IR emitter, `src/sema.zig` `.deal/index.json` writer, `cli/src/sysml_v2.rs` SysML emitter, `cli/src/main.rs` `--json` envelope.

**Concrete excerpt** (lines 42-55):
```zig
try buf.appendSlice(allocator, "{\"v\":1,\"mode\":\"");
// ... v (1st), mode (2nd), filename (3rd), root (4th) — alphabetical
```

**Rust side equivalent** (RESEARCH §Pitfall 1 lines 695-707): `serde_json::Value::Object` is a `BTreeMap` by default — alphabetical. Do NOT enable `preserve_order` feature.

---

### Pattern S-3: Diagnostic emission via `DiagnosticCollector`

**Source:** `src/diagnostics.zig:123-189`.

**Apply to:** `src/sema.zig` (all 6 checks), any new Zig module that needs to emit diagnostics. Never construct `Diagnostic{...}` literals — go through the collector to ensure arena lifetimes.

**Concrete excerpt** (lines 173-188):
```zig
pub fn emitFmt(
    self: *DiagnosticCollector,
    code: []const u8,
    severity: Severity,
    span: ast.Span,
    comptime fmt: []const u8,  // SECURITY: comptime — T-DiagInjection mitigation
    args: anytype,
) !void {
    const msg = try std.fmt.allocPrint(self.arena, fmt, args);
    try self.list.append(self.arena, .{ .code = code, .severity = severity,
                                         .message = msg, .span = span });
}
```

---

### Pattern S-4: Length-prefixed UTF-8 C ABI output (D-11 LOCKED)

**Source:** `src/lib.zig:285-323` (`deal_ast_json`, `deal_diagnostics_json`) + `include/deal.h:229-276`.

**Apply to:** Both new C ABI exports (`deal_ir_json`, `deal_format`). Out-params are `(*[*]const u8, *usize)`. Buffer is arena-owned, NOT NUL-terminated. Lazy + cached on first call.

**Concrete excerpt** (lines 285-302):
```zig
pub export fn deal_ast_json(
    handle_ptr: ?*anyopaque,
    out_ptr: *[*]const u8,
    out_len: *usize,
) callconv(.c) bool {
    const p = handle_ptr orelse return false;
    const h = handleCast(p);
    if (h.cached_ast_json == null) {
        const buf = json.emitAst(h.arena.allocator(), h.ast_root, h.mode, h.filename) catch return false;
        h.cached_ast_json = buf;
    }
    const cached = h.cached_ast_json.?;
    out_ptr.* = cached.ptr;
    out_len.* = cached.len;
    return true;
}
```

---

### Pattern S-5: Hand-rolled JSON writer (RESEARCH §Pitfall 1)

**Source:** `src/json.zig` (full file — never uses `std.json.Stringify`).

**Apply to:** IR JSON emitter in `src/json.zig`, `.deal/index.json` writer in `src/sema.zig`.

**Reason** (from `src/json.zig:1-18` doc comment): `std.json.Stringify` can reorder fields across Zig versions (upstream issue #25233). Hand-rolled emission with explicit alphabetical sequencing is the only way to enforce D-18.

**Pattern excerpt** — see Pattern S-2.

---

### Pattern S-6: Two-outcome pin-test contract (impl OR ADR)

**Source:** `tests/unit/strictness_m08_m18_m19.zig:1-66` + `tests/unit/recovery_corpus.zig:31-78`.

**Apply to:** `tests/unit/sema_corpus.zig` per-check fixture pinning.

**Concrete excerpt** (`recovery_corpus.zig:41-47`):
```zig
const Pin = struct {
    name: []const u8,
    expected_code: ?[]const u8,   // null = accepted-no-diag disposition
    adr_basename: ?[]const u8 = null,  // required when expected_code == null
};
```

The two-outcome lets future ADRs flip the disposition without rewriting the test.

---

### Pattern S-7: Test-only internal helpers (NOT exported to C ABI)

**Source:** `src/lib.zig:173-252` (`deal_parse_internal`, `deal_free_internal`).

**Apply to:** Any new test-only entry point in `src/lib.zig` (e.g., `deal_lower_internal` for testing the lowering pass with `std.testing.allocator` for leak detection).

**Concrete excerpt** (lines 165-172 doc comment):
> "Test-only entry point — NOT exported to the C ABI (no `callconv(.c)`, no `export` keyword). [...] MUST NOT appear in `nm -gU zig-out/lib/libdeal.a` — Zig only exports `pub export fn`; a bare `pub fn` is reachable from in-tree tests but NOT emitted as a public dynamic symbol."

The Plan 06 grep gate (`nm -gU` count) extends from 6 to 8 — internal helpers stay invisible.

---

### Pattern S-8: Fresh-worktree gate sibling (Phase 1.5 ADR LOCKED for Phases 2..6)

**Source:** `build.zig:210-241` + `scripts/verify-fresh-worktree.sh` (full file).

**Apply to:** `build.zig` Phase 2 — add `phase-2-gate-fresh` sibling per CONTEXT.md D-35.

**The script's invariant** (lines 56-66): refuses to run on a dirty main tree because the gate verifies COMMITTED state only. Mirrors the false-GREEN remediation that birthed Phase 1.5 Plan 05.

---

### Pattern S-9: 19-file showcase iteration loop

**Source:** `tests/unit/parser_deal_snapshot.zig:20-36` (15 .deal) + `tests/unit/property_span_containment.zig:58-80` (15 .deal + 4 .dealx) + `tests/unit/determinism_parse_twice.zig:36-56` (19 combined).

**Apply to:** Every new whole-corpus test (`fmt_roundtrip.zig`, `determinism_lower_twice.zig`, `sema_corpus.zig` for the showcase-clean check that all 19 files exit 0).

**Concrete excerpt** — see the verbatim string array at `determinism_parse_twice.zig:36-56`.

---

### Pattern S-10: AST → IR span carry-over

**Source:** `src/ast.zig:19-22` (`Span` extern struct — u32 start + u32 end). Reuse identically in `src/ir.zig`.

**Apply to:** Every IR node — span is the same `[start, end]` byte offset as the originating AST node. The IR README (per CONTEXT.md D-27) documents this as a contract.

**Concrete excerpt** (lines 17-22):
```zig
pub const Span = extern struct {
    start: u32,
    end: u32,
};
```

---

## No Analog Found

Files with no close in-tree match. Planner should use RESEARCH.md patterns directly (cited line ranges shown).

| File | Role | Why No Analog | Nearest External Pattern |
|------|------|---------------|--------------------------|
| `cli/src/sysml_v2.rs` | SysML v2 codegen | Phase 1 has no Rust-side data emitter — every previous emitter is in Zig | RESEARCH §Pattern 8 (mapping table) + §Example 3 (UUID-from-path) |
| `cli/src/schema_registry.rs` | JSON-Schema validator | First JSON-Schema validator in repo; first `Retrieve` trait impl | RESEARCH §Example 4 (full template, lines 928-972) |
| `cli/src/main.rs` | CLI dispatch | First Rust binary in repo (`tests/ffi/` is a lib crate, not a binary) | RESEARCH §Example 1 (full template, lines 808-866) |
| `deal/Cargo.toml` workspace root | Workspace manifest | Phase 1's `tests/ffi/Cargo.toml` is standalone; never a workspace | RESEARCH §Pattern 1 (lines 384-397) |
| `spec/ir/v0/schema.json` + `spec/ir/v0/README.md` | IR v0 normative spec | DEAL has never authored a JSON Schema before | OMG `SysML.json` (shape only) + CONTEXT.md D-27 contract |

---

## Metadata

**Analog search scope:** `deal/src/*.zig`, `deal/include/*.h`, `deal/tests/{unit,ffi,malformed,snapshots}/`, `deal/build.zig`, `deal/scripts/`, `deal/.planning/decisions/`.

**Files scanned:** 11 Zig source files (8,200 LOC total), 1 C header (283 LOC), 24 unit-test files, 2 FFI test files, `build.zig` (364 LOC), `tests/ffi/{Cargo.toml,build.rs,src/lib.rs}`, `scripts/verify-fresh-worktree.sh`.

**Pattern extraction date:** 2026-05-21

**Plan-slicing alignment** (per CONTEXT.md D-37): patterns above are tagged by lane so the planner can drop them into Plans 02-01 (Lanes 4 + 8 + comment-attachment portion of Lane 2), 02-02 (Lane 1), 02-03 (Lanes 2 + spec docs), 02-04 (Lanes 5 + 6 + golden fixtures from Lane 5), 02-05 (Lanes 3 + 7), 02-06 (Lane 9).
