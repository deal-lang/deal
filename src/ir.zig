//! DEAL IR v0 types and traversal API (D-22..D-27).
//!
//! This module defines the LOCKED contract surface for Phases 2-6.
//! See spec/ir/v0/schema.json (normative) and spec/ir/v0/README.md (reference).
//!
//! Design decisions (CONTEXT.md):
//!   D-22: IR crosses FFI as JSON via deal_ir_json()
//!   D-23: Element IDs are fully-qualified path strings (e.g. "vehicle.battery.BatteryCell")
//!   D-24: Single workspace-wide graph (not per-package)
//!   D-25: IR is comment-free — no leading_comments/trailing_comments/doc_comment
//!   D-26: LOCKED traversal API: walk, find, references, children, parent
//!   D-27: spec/ir/v0/schema.json is the normative contract
//!
//! Arena allocation: all IR data lives in the per-handle arena (D-02).
//! deal_free() releases the arena; no individual frees needed.

const std = @import("std");
const ast = @import("ast");

/// Reuse the AST Span type (D-15, S-10). Byte-offset span into the source buffer.
/// `extern struct` required for C ABI compatibility.
pub const Span = ast.Span;

// ─── NodeKind ──────────────────────────────────────────────────────────────

/// Semantic category of an IR node. Must stay in sync with spec/ir/v0/schema.json
/// $defs.NodeKind.enum.
pub const NodeKind = enum {
    action_def,
    allocation_def,
    allocate,
    annotation,
    attribute_def,
    attribute_usage,
    connect,
    connection_def,
    calc_def,    // Phase 05.2 Wave 2
    constraint_def,
    expose,
    flow_def,
    interface_def,
    item_def,
    need_def,
    package,
    part_def,
    part_usage,
    port_def,
    port_usage,
    requirement_def,
    satisfy,
    state_def,
    subsystem,
    system,
    traceability_block,
    use_case_def,
    validate,
};

// ─── EdgeKind ──────────────────────────────────────────────────────────────

/// Relationship category for an IR Edge. Must stay in sync with
/// spec/ir/v0/schema.json $defs.EdgeKind.enum.
pub const EdgeKind = enum {
    allocated_to,
    carries,
    connects_via,
    contains,
    derives_from,
    imports,
    redefines,
    satisfies,
    specializes,
    subsets,
    traces,
};

// ─── Edge ──────────────────────────────────────────────────────────────────

/// A directed relationship between two IR nodes.
/// src and dst are fully-qualified path IDs (D-23), arena-owned.
/// JSON key order: dst, kind, src (alphabetical per D-18).
pub const Edge = struct {
    src: []const u8,
    dst: []const u8,
    kind: EdgeKind,
};

/// An edge reference stored in the incoming_index (for references() queries).
pub const EdgeRef = struct {
    /// The edge itself (pointer into the Document's edges slice).
    edge: *const Edge,
};

// ─── Calc-def IR payload types (Phase 05.2 Wave 4) ─────────────────────────

/// A directed parameter in a calc_def node.
/// direction mirrors deal-sim I/O envelope keys (D-12):
///   in-params  == sim `inputs`  dict entries
///   out-params == sim `outputs` dict entries
pub const IrParam = struct {
    /// Simple parameter name (as written in source).
    name: []const u8,
    /// Declared type name (unresolved, as written in source).
    type_name: []const u8,
    /// Parameter direction: "in" | "out" | "inout".
    direction: []const u8,
};

/// Generic precision slot shared at calc-return, attribute-value, and
/// require-threshold sites (D-06/D-08, parse-and-carry only).
/// ONE shared type — never duplicated per attach point (D-08).
pub const IrPrecision = struct {
    /// "sig_figures" | "tolerance_absolute" | "tolerance_relative"
    kind: []const u8,
    /// Serialized value, e.g. "4" for sig 4, "percent(1)" for ±percent(1).
    value: []const u8,
};

/// Payload for calc_def IR nodes.
/// Directed-param shape coincides exactly with deal-sim I/O contract (D-12).
pub const IrCalcDefPayload = struct {
    /// Ordered directed-parameter list (in order as declared).
    params: []const IrParam = &.{},
    /// Declared return type name (unresolved), or null.
    return_type: ?[]const u8 = null,
    /// Optional precision slot from the return contract (D-08, null until Lane A).
    precision: ?IrPrecision = null,
    /// True if any param has direction out or inout (purity flag, D-07).
    is_statement_valued: bool = false,
};

// ─── Payload types ────────────────────────────────────────────────────────

/// Optional typed envelope for @confidence, @rationale, @assumes, @concerns.
/// All fields are optional; absent = annotation not present in source.
/// D-25: agent_metadata is part of the IR payload, not a comment field.
pub const AgentMetadata = struct {
    /// @assumes string values from annotations (preserves source order).
    assumes: []const []const u8 = &.{},
    /// @concerns string values (preserves source order).
    concerns: []const []const u8 = &.{},
    /// @confidence float value; null if not specified.
    confidence: ?f64 = null,
    /// @rationale string value; null if not specified.
    rationale: ?[]const u8 = null,
};

/// Operator enum for simulation bindings. Serialized as "computes" or
/// "validates_against" (note: source syntax is "validates against" with space).
pub const SimOperator = enum {
    computes,
    validates_against,
};

/// A simulation binding from @simulation:<<computes>> or
/// @simulation:<<validates against>> annotation.
pub const SimBinding = struct {
    /// Simulation operator (required in source via annotation tag).
    operator: SimOperator,
    /// Target identifier (simulated quantity name).
    target: []const u8,
    /// Optional tool name from annotation body.
    tool: ?[]const u8 = null,
    /// Optional equation string from annotation body.
    equation: ?[]const u8 = null,
    /// Optional fidelity descriptor.
    fidelity: ?[]const u8 = null,
    /// Optional entry point (function name for code-gen targets).
    entry: ?[]const u8 = null,
};

/// Generic per-element payload carried by most node kinds.
/// The subset of fields populated depends on the NodeKind.
pub const IrPayload = struct {
    /// Simple (unqualified) element name.
    name: []const u8 = "",
    /// Optional resolved type reference (fully-qualified path).
    /// Present on usage nodes and port_def nodes.
    type_ref: ?[]const u8 = null,
    /// Port/attribute direction (serialized as "in"/"out"/"inout"/"none").
    direction: ast.Direction = .none,
    /// Modifier keywords present on the declaration.
    modifiers: []const []const u8 = &.{},
    /// Optional agent metadata envelope (from @confidence/@rationale/@assumes/@concerns).
    agent_metadata: ?AgentMetadata = null,
    /// Simulation bindings array (from @simulation:<<computes/validates against>>).
    simulation_bindings: []const SimBinding = &.{},
    /// For satisfy nodes: the satisfied requirement's qualified path.
    requirement_ref: ?[]const u8 = null,
    /// For calc_def nodes: extended directed-param payload (D-12, Wave 4).
    /// Null for all other node kinds.
    calc_def: ?IrCalcDefPayload = null,
};

// ─── IrNode ────────────────────────────────────────────────────────────────

/// A single semantic element in the DEAL IR Document.
/// D-25: NO comment fields (no leading_comments, trailing_comments, doc_comment).
/// D-23: id is the fully-qualified path string.
/// D-15: span carries over from the AST verbatim.
pub const IrNode = struct {
    /// Fully-qualified path ID (D-23), arena-owned.
    id: []const u8,
    kind: NodeKind,
    /// Byte-offset span carried over from the AST (D-15).
    span: Span,
    /// Workspace-relative source file path (T-02-11, T-02-19).
    source_file: []const u8,
    payload: IrPayload,
};

// ─── Document ──────────────────────────────────────────────────────────────

/// Workspace-wide IR Document (D-24 — single graph for the entire workspace).
/// All data is arena-allocated via the owning DealHandle's arena.
///
/// LOCKED traversal API (D-26):
///   walk(visitor)             — pre/post-order visitor over all elements
///   find(id) ?*IrNode         — O(1) hash lookup by qualified path
///   references(id) []EdgeRef  — O(1) incoming edges via incoming_index
///   children(id) [][]const u8 — O(1) direct children via children_index
///   parent(id) ?[]const u8    — inferred from qualified path prefix
pub const Document = struct {
    /// Map from fully-qualified path ID → IrNode pointer (arena-owned).
    elements: std.StringHashMap(*IrNode),
    /// Flat adjacency list of all edges in the workspace graph.
    edges: []Edge,
    /// Index: id → []EdgeRef (all incoming edges to that node). O(1) lookup.
    incoming_index: std.StringHashMap([]EdgeRef),
    /// Index: id → [][]const u8 (child IDs via 'contains' edges). O(1).
    children_index: std.StringHashMap([][]const u8),

    /// Pre-order + post-order visitor traversal over all elements.
    ///
    /// `visitor` is duck-typed via `anytype`. It must expose:
    ///   fn preOrder(self: *@TypeOf(visitor), node: *const IrNode) anyerror!void
    ///   fn postOrder(self: *@TypeOf(visitor), node: *const IrNode) anyerror!void
    ///
    /// Walk order is deterministic: elements are iterated in insertion order.
    /// For snapshot-stable output, callers that need alphabetical order should
    /// sort the element keys first.
    pub fn walk(self: *const Document, visitor: anytype) !void {
        var it = self.elements.iterator();
        while (it.next()) |entry| {
            const node = entry.value_ptr.*;
            try visitor.preOrder(node);
            try visitor.postOrder(node);
        }
    }

    /// Resolve a fully-qualified path ID to its IrNode. O(1) hash lookup.
    /// Returns null if the ID is not present in the document.
    pub fn find(self: *const Document, id: []const u8) ?*IrNode {
        return self.elements.get(id);
    }

    /// All incoming edges to the node with the given ID. O(1) index lookup.
    /// Returns an empty slice if the ID has no incoming edges or is not present.
    pub fn references(self: *const Document, id: []const u8) []const EdgeRef {
        return self.incoming_index.get(id) orelse &.{};
    }

    /// Direct children of the node with the given ID (via 'contains' edges).
    /// Returns child IDs as a slice of strings. O(1) lookup.
    pub fn children(self: *const Document, id: []const u8) []const []const u8 {
        return self.children_index.get(id) orelse &.{};
    }

    /// Parent of the node with the given ID, inferred from the qualified path prefix.
    /// Returns null for top-level package nodes (no `.` in id).
    ///
    /// Example: "vehicle.battery.BatteryCell.nominalVoltage"
    ///   → parent = "vehicle.battery.BatteryCell"
    ///
    /// NOTE: This does not verify that the returned parent ID is present in the
    /// document. Cross-file references may have a parent that isn't in this
    /// single-file lowering. Callers should use `find(parent_id)` to check.
    pub fn parent(self: *const Document, id: []const u8) ?[]const u8 {
        _ = self;
        const dot_pos = std.mem.lastIndexOf(u8, id, ".") orelse return null;
        return id[0..dot_pos];
    }
};

// ─── Tests ─────────────────────────────────────────────────────────────────

test "ir: Document.parent basic" {
    var doc: Document = .{
        .elements = std.StringHashMap(*IrNode).init(std.testing.allocator),
        .edges = &.{},
        .incoming_index = std.StringHashMap([]EdgeRef).init(std.testing.allocator),
        .children_index = std.StringHashMap([][]const u8).init(std.testing.allocator),
    };
    defer doc.elements.deinit();
    defer doc.incoming_index.deinit();
    defer doc.children_index.deinit();

    // Top-level package: no dot → null parent
    try std.testing.expectEqual(@as(?[]const u8, null), doc.parent("vehicle"));

    // Nested element: returns prefix
    const p = doc.parent("vehicle.battery.BatteryCell");
    try std.testing.expect(p != null);
    try std.testing.expectEqualStrings("vehicle.battery", p.?);

    // Two-level element
    const p2 = doc.parent("vehicle.battery");
    try std.testing.expect(p2 != null);
    try std.testing.expectEqualStrings("vehicle", p2.?);
}

test "ir: Document.find missing key returns null" {
    var doc: Document = .{
        .elements = std.StringHashMap(*IrNode).init(std.testing.allocator),
        .edges = &.{},
        .incoming_index = std.StringHashMap([]EdgeRef).init(std.testing.allocator),
        .children_index = std.StringHashMap([][]const u8).init(std.testing.allocator),
    };
    defer doc.elements.deinit();
    defer doc.incoming_index.deinit();
    defer doc.children_index.deinit();

    try std.testing.expectEqual(@as(?*IrNode, null), doc.find("nonexistent.id"));
}
