//! AST + SymbolTable → DEAL IR v0 lowering pass (D-22..D-27, REQ-phase-2-2b-ir-lowering).
//!
//! Architecture (three-pass, per RESEARCH §Pattern 7):
//!   Pass 1 (intern):  Walk all AST declarations; build a qualified-path → interned
//!                     string map. Ensures each ID string is allocated exactly once.
//!   Pass 2 (lower):   Walk AST again; allocate IrNode per declaration, carry span
//!                     from AST, source_file from sema SymbolTable, lower payload per
//!                     kind. Emit edges as encountered.
//!   Pass 3 (index):   Build incoming_index + children_index from the edges list for
//!                     O(1) traversal-API queries.
//!
//! Key constraints:
//!   D-25 CRITICAL: IR is comment-free. Lowering MUST NOT copy leading_comments /
//!                  trailing_comments / doc_comment from AST into IR nodes.
//!   D-02: All IR data allocated into the per-handle arena.
//!   D-23: IDs are fully-qualified path strings (dot-separator).
//!   T-02-19: source_file is workspace-relative (passed in from caller).
//!   T-02-20: deal_lower_internal is `pub fn` (no export), NOT in nm -gU output.

const std = @import("std");
const ast = @import("ast");
const ir = @import("ir");
const sema = @import("sema");

/// Internal lowering state — not exposed outside this module.
const Lowerer = struct {
    arena: std.mem.Allocator,
    /// Interned qualified-path strings. Values point into arena memory.
    id_intern: std.StringHashMap([]const u8),
    /// Accumulated edges during Pass 2.
    edges: std.ArrayList(ir.Edge),
    /// Element map being built (passed to Document at end).
    elements: std.StringHashMap(*ir.IrNode),
    /// Current package prefix (dot-joined), arena-owned.
    pkg_prefix: []const u8 = "",
    /// Workspace-relative source file path for the current file.
    source_file: []const u8 = "",
};

/// Lower an AST root (deal_file or dealx_file) plus its resolved symbol table
/// into a DEAL IR v0 Document.
///
/// All IR data is allocated into `arena`. The caller is responsible for the
/// lifetime of `ast_root` — it must outlive the returned Document.
///
/// Returns error.OutOfMemory on allocation failure.
pub fn lower(
    arena: std.mem.Allocator,
    ast_root: *ast.Node,
    symbols: *const sema.SymbolTable,
    source_file: []const u8,
) !*ir.Document {
    var l = Lowerer{
        .arena = arena,
        .id_intern = std.StringHashMap([]const u8).init(arena),
        .edges = .empty,
        .elements = std.StringHashMap(*ir.IrNode).init(arena),
        .source_file = source_file,
    };

    // Determine package prefix from the symbol table.
    if (symbols.package_segments.len > 0) {
        l.pkg_prefix = try std.mem.join(arena, ".", symbols.package_segments);
    }

    // Pass 1: intern all IDs (ensures each qualified path string is allocated once).
    try pass1Intern(&l, ast_root);

    // Pass 2: lower AST nodes → IrNodes + collect edges.
    try pass2Lower(&l, ast_root);

    // Pass 3: build adjacency indexes.
    const doc = try arena.create(ir.Document);
    doc.* = .{
        .elements = l.elements,
        .edges = try l.edges.toOwnedSlice(arena),
        .incoming_index = std.StringHashMap([]ir.EdgeRef).init(arena),
        .children_index = std.StringHashMap([][]const u8).init(arena),
    };
    try pass3BuildIndexes(arena, doc);

    return doc;
}

// ─── Pass 1: ID interning ────────────────────────────────────────────────────

fn pass1Intern(l: *Lowerer, root: *ast.Node) !void {
    switch (root.payload) {
        .deal_file => |f| {
            if (l.pkg_prefix.len > 0) {
                _ = try internId(l, l.pkg_prefix);
            }
            for (f.definitions) |def| {
                try pass1InternDef(l, def, l.pkg_prefix);
            }
        },
        .dealx_file => |f| {
            if (l.pkg_prefix.len > 0) {
                _ = try internId(l, l.pkg_prefix);
            }
            for (f.root_tags) |tag| {
                try pass1InternCompNode(l, tag, l.pkg_prefix);
            }
        },
        else => {},
    }
}

fn pass1InternDef(l: *Lowerer, node: *ast.Node, parent_id: []const u8) !void {
    const elem = elemDefOf(node) orelse return;
    const qualified_id = try qualifyId(l.arena, parent_id, elem.name);
    _ = try internId(l, qualified_id);

    // Recurse into direct members.
    for (elem.members) |member| {
        try pass1InternMember(l, member, qualified_id);
    }
    // Recurse into visibility-wrapper members.
    for (elem.members) |member| {
        if (member.kind == .visibility_wrapper) {
            for (member.payload.visibility_wrapper.members) |inner| {
                try pass1InternMember(l, inner, qualified_id);
            }
        }
    }
}

fn pass1InternMember(l: *Lowerer, node: *ast.Node, parent_id: []const u8) !void {
    const elem = elemUsageOf(node) orelse return;
    const qualified_id = try qualifyId(l.arena, parent_id, elem.name);
    _ = try internId(l, qualified_id);
}

fn pass1InternCompNode(l: *Lowerer, node: *ast.Node, parent_id: []const u8) !void {
    switch (node.payload) {
        .system_block => |sb| {
            const n = sb.name orelse return;
            const qualified_id = try qualifyId(l.arena, parent_id, n);
            _ = try internId(l, qualified_id);
            for (sb.children) |c| try pass1InternCompNode(l, c, qualified_id);
        },
        .subsystem_block => |sb| {
            const n = sb.name orelse return;
            const qualified_id = try qualifyId(l.arena, parent_id, n);
            _ = try internId(l, qualified_id);
            for (sb.children) |c| try pass1InternCompNode(l, c, qualified_id);
        },
        .traceability_block => |tb| {
            for (tb.children) |c| try pass1InternCompNode(l, c, parent_id);
        },
        .satisfy_block => |sb| {
            for (sb.children) |c| try pass1InternCompNode(l, c, parent_id);
        },
        else => {},
    }
}

/// Intern a qualified ID string. If already present return the existing slice;
/// otherwise dupe into arena and store.
fn internId(l: *Lowerer, id: []const u8) ![]const u8 {
    if (l.id_intern.get(id)) |existing| {
        return existing;
    }
    const owned = try l.arena.dupe(u8, id);
    try l.id_intern.put(owned, owned);
    return owned;
}

// ─── Pass 2: AST → IrNode lowering ────────────────────────────────────────

fn pass2Lower(l: *Lowerer, root: *ast.Node) !void {
    switch (root.payload) {
        .deal_file => |f| {
            if (l.pkg_prefix.len > 0) {
                try emitPackageNode(l, l.pkg_prefix, root.span);
            }
            for (f.definitions) |def| {
                try pass2LowerDef(l, def, l.pkg_prefix, null);
            }
        },
        .dealx_file => |f| {
            if (l.pkg_prefix.len > 0) {
                try emitPackageNode(l, l.pkg_prefix, root.span);
            }
            for (f.root_tags) |tag| {
                try pass2LowerCompNode(l, tag, l.pkg_prefix);
            }
        },
        else => {},
    }
}

fn pass2LowerDef(
    l: *Lowerer,
    node: *ast.Node,
    parent_id: []const u8,
    parent_node_id: ?[]const u8,
) !void {
    // Phase 05.2 Wave 4: full lowering for calc_def (D-12 directed-param shape).
    if (node.kind == .calc_def) {
        return lowerCalcDef(l, node, parent_id, parent_node_id);
    }
    if (node.kind == .constraint_def) {
        // constraint_def lowering: emit a minimal IR node.
        const cdef = node.payload.constraint_def;
        const qualified_id = try qualifyId(l.arena, parent_id, cdef.name);
        const interned_id = try internId(l, qualified_id);
        const ir_node = try l.arena.create(ir.IrNode);
        ir_node.* = .{
            .id = interned_id,
            .kind = .constraint_def,
            .span = node.span,
            .source_file = try l.arena.dupe(u8, l.source_file),
            .payload = ir.IrPayload{
                .name = try l.arena.dupe(u8, cdef.name),
                .modifiers = &.{},
                .direction = cdef.direction,
            },
        };
        try l.elements.put(interned_id, ir_node);
        if (parent_node_id) |pid| {
            try l.edges.append(l.arena, .{
                .src = try internId(l, pid),
                .dst = interned_id,
                .kind = .contains,
            });
        }
        return;
    }
    const ir_kind = astKindToIrKind(node.kind) orelse return;
    const elem = elemDefOf(node) orelse return;

    const qualified_id = try qualifyId(l.arena, parent_id, elem.name);
    const interned_id = try internId(l, qualified_id);

    // Build payload — D-25: NO comment fields.
    var payload = ir.IrPayload{
        .name = try l.arena.dupe(u8, elem.name),
        .modifiers = try lowerModifiers(l, elem.modifiers),
        .direction = elem.direction,
    };

    // Lower annotations: agent_metadata, simulation_bindings.
    try lowerAnnotations(l, elem.annotations, &payload);

    // Lower specialization → edge.
    if (elem.specializes) |spec_node| {
        if (spec_node.kind == .structural_relationship) {
            const spec = spec_node.payload.structural_relationship;
            if (spec.target_segments.len > 0) {
                const target_id = try std.mem.join(l.arena, ".", spec.target_segments);
                try l.edges.append(l.arena, .{
                    .src = interned_id,
                    .dst = target_id,
                    .kind = .specializes,
                });
            }
        }
    }

    const ir_node = try l.arena.create(ir.IrNode);
    ir_node.* = .{
        .id = interned_id,
        .kind = ir_kind,
        .span = node.span,
        .source_file = try l.arena.dupe(u8, l.source_file),
        .payload = payload,
    };
    try l.elements.put(interned_id, ir_node);

    // 'contains' edge from parent node to this definition.
    if (parent_node_id) |pid| {
        try l.edges.append(l.arena, .{
            .src = try internId(l, pid),
            .dst = interned_id,
            .kind = .contains,
        });
    }

    // Recurse into members.
    for (elem.members) |member| {
        try pass2LowerMember(l, member, interned_id);
    }
    for (elem.members) |member| {
        if (member.kind == .visibility_wrapper) {
            for (member.payload.visibility_wrapper.members) |inner| {
                try pass2LowerMember(l, inner, interned_id);
            }
        }
    }
}

/// Full calc_def lowering (Wave 4, D-12 directed-param shape).
/// Emits an IR node carrying directed-param payload in IrPayload.calc_def.
///
/// D-12: params stored as an ORDERED array of {name, type_name, direction}.
/// In-params == deal-sim `inputs`, out-params == deal-sim `outputs` (order preserved).
fn lowerCalcDef(
    l: *Lowerer,
    node: *ast.Node,
    parent_id: []const u8,
    parent_node_id: ?[]const u8,
) !void {
    const calc = node.payload.calc_def;
    const qualified_id = try qualifyId(l.arena, parent_id, calc.name);
    const interned_id = try internId(l, qualified_id);

    // Build IrParam slice from the param_list node (D-12).
    const param_list = calc.params.payload.param_list;
    var ir_params = try l.arena.alloc(ir.IrParam, param_list.params.len);
    var is_statement_valued = false;
    for (param_list.params, 0..) |param_node, i| {
        const pd = param_node.payload.param_decl;
        const dir_str = directionStr(pd.direction);
        if (pd.direction == .out or pd.direction == .inout) {
            is_statement_valued = true;
        }
        const type_name = extractTypeAnnotationName(pd.type_node);
        ir_params[i] = .{
            .name = try l.arena.dupe(u8, pd.name),
            .type_name = try l.arena.dupe(u8, type_name),
            .direction = dir_str,
        };
    }

    // Build return type name from the type_node.
    const return_type_raw = extractTypeAnnotationName(calc.type_node);
    const return_type: ?[]const u8 = if (return_type_raw.len > 0)
        try l.arena.dupe(u8, return_type_raw)
    else
        null;

    // Extract optional precision from the return_contract.
    var precision: ?ir.IrPrecision = null;
    if (calc.return_contract) |rc| {
        if (try extractPrecisionFromReturnContract(l.arena, rc)) |prec| {
            precision = prec;
        }
    }

    // Build IrCalcDefPayload.
    const calc_payload = ir.IrCalcDefPayload{
        .params = ir_params,
        .return_type = return_type,
        .precision = precision,
        .is_statement_valued = is_statement_valued,
    };

    // Lower modifiers.
    const modifiers = try lowerModifiers(l, calc.modifiers);

    // Lower annotations: agent_metadata, simulation_bindings.
    var payload = ir.IrPayload{
        .name = try l.arena.dupe(u8, calc.name),
        .modifiers = modifiers,
        .direction = calc.direction,
        .calc_def = calc_payload,
    };
    try lowerAnnotations(l, calc.annotations, &payload);

    const ir_node = try l.arena.create(ir.IrNode);
    ir_node.* = .{
        .id = interned_id,
        .kind = .calc_def,
        .span = node.span,
        .source_file = try l.arena.dupe(u8, l.source_file),
        .payload = payload,
    };
    try l.elements.put(interned_id, ir_node);

    if (parent_node_id) |pid| {
        try l.edges.append(l.arena, .{
            .src = try internId(l, pid),
            .dst = interned_id,
            .kind = .contains,
        });
    }
}

/// Extract a type name string from a type_annotation node.
/// Returns the dot-joined name_segments, or an empty string if not applicable.
fn extractTypeAnnotationName(type_node: *ast.Node) []const u8 {
    if (type_node.kind == .type_annotation) {
        const ta = type_node.payload.type_annotation;
        if (ta.name_segments.len > 0) {
            // For simple (unqualified) names, return just the last segment.
            // For qualified names (e.g. deal.std.units.Force), return the last segment.
            return ta.name_segments[ta.name_segments.len - 1];
        }
    }
    return "";
}

/// Extract the first precision_spec from a return_contract node, if present.
fn extractPrecisionFromReturnContract(arena: std.mem.Allocator, rc_node: *ast.Node) !?ir.IrPrecision {
    if (rc_node.kind != .return_contract) return null;
    const rc = rc_node.payload.return_contract;
    for (rc.items) |item| {
        if (item.kind == .precision_spec) {
            const ps = item.payload.precision_spec;
            const kind_str: []const u8 = switch (ps.kind) {
                .sig_figures => "sig_figures",
                .tolerance_absolute => "tolerance_absolute",
                .tolerance_relative => "tolerance_relative",
            };
            // Extract the value text from the value node.
            // D-08: parse-and-carry. For simple literals, carry exact value.
            // For complex expressions (e.g. percent(1) call), carry "<expr>" as a
            // placeholder — Lane A will interpret these properly.
            const value_str = getNodeLiteralText(ps.value) orelse "<expr>";
            return ir.IrPrecision{
                .kind = kind_str,
                .value = try arena.dupe(u8, value_str),
            };
        }
    }
    return null;
}

/// Map an ast.Direction to its string representation.
fn directionStr(dir: ast.Direction) []const u8 {
    return switch (dir) {
        .in => "in",
        .out => "out",
        .inout => "inout",
        .none => "in", // default to "in" for calc params without explicit direction
    };
}

fn pass2LowerMember(l: *Lowerer, node: *ast.Node, parent_id: []const u8) !void {
    const elem = elemUsageOf(node) orelse {
        // Check for annotation nodes that emit edges.
        if (node.kind == .annotation) {
            try lowerAnnotationEdge(l, node.payload.annotation, parent_id);
        } else if (node.kind == .inline_body) {
            for (node.payload.inline_body.members) |m| {
                try pass2LowerMember(l, m, parent_id);
            }
        }
        return;
    };

    const ir_kind = astUsageKindToIrKind(node.kind);
    const qualified_id = try qualifyId(l.arena, parent_id, elem.name);
    const interned_id = try internId(l, qualified_id);

    var payload = ir.IrPayload{
        .name = try l.arena.dupe(u8, elem.name),
        .modifiers = try lowerModifiers(l, elem.modifiers),
        .direction = elem.direction,
    };

    // Resolve type reference.
    if (elem.type_node) |tn| {
        if (tn.kind == .type_annotation) {
            const type_ann = tn.payload.type_annotation;
            if (type_ann.name_segments.len > 0) {
                payload.type_ref = try std.mem.join(l.arena, ".", type_ann.name_segments);
            }
        }
    }

    try lowerAnnotations(l, elem.annotations, &payload);

    const ir_node = try l.arena.create(ir.IrNode);
    ir_node.* = .{
        .id = interned_id,
        .kind = ir_kind,
        .span = node.span,
        .source_file = try l.arena.dupe(u8, l.source_file),
        .payload = payload,
    };
    try l.elements.put(interned_id, ir_node);

    // 'contains' edge from parent.
    try l.edges.append(l.arena, .{
        .src = try internId(l, parent_id),
        .dst = interned_id,
        .kind = .contains,
    });
}

fn pass2LowerCompNode(l: *Lowerer, node: *ast.Node, parent_id: []const u8) !void {
    switch (node.payload) {
        .system_block => |sb| {
            const n = sb.name orelse return;
            const qualified_id = try qualifyId(l.arena, parent_id, n);
            const interned_id = try internId(l, qualified_id);

            const ir_node = try l.arena.create(ir.IrNode);
            ir_node.* = .{
                .id = interned_id,
                .kind = .system,
                .span = node.span,
                .source_file = try l.arena.dupe(u8, l.source_file),
                .payload = .{ .name = try l.arena.dupe(u8, n) },
            };
            try l.elements.put(interned_id, ir_node);
            for (sb.children) |c| try pass2LowerCompNode(l, c, interned_id);
        },
        .subsystem_block => |sb| {
            const n = sb.name orelse return;
            const qualified_id = try qualifyId(l.arena, parent_id, n);
            const interned_id = try internId(l, qualified_id);

            const ir_node = try l.arena.create(ir.IrNode);
            ir_node.* = .{
                .id = interned_id,
                .kind = .subsystem,
                .span = node.span,
                .source_file = try l.arena.dupe(u8, l.source_file),
                .payload = .{ .name = try l.arena.dupe(u8, n) },
            };
            try l.elements.put(interned_id, ir_node);
            for (sb.children) |c| try pass2LowerCompNode(l, c, interned_id);
        },
        .traceability_block => |tb| {
            // Emit traceability_block node under the parent scope.
            const tb_id = try std.fmt.allocPrint(l.arena, "{s}.@trace", .{parent_id});
            const interned_id = try internId(l, tb_id);
            const ir_node = try l.arena.create(ir.IrNode);
            ir_node.* = .{
                .id = interned_id,
                .kind = .traceability_block,
                .span = node.span,
                .source_file = try l.arena.dupe(u8, l.source_file),
                .payload = .{},
            };
            try l.elements.put(interned_id, ir_node);
            for (tb.children) |c| try pass2LowerCompNode(l, c, parent_id);
        },
        .satisfy_block => |sb| {
            for (sb.children) |c| try pass2LowerCompNode(l, c, parent_id);
        },
        .allocate_tag => {
            // @allocate tag — relationships are in attrs; emit allocated_to edges.
            // The AllocateTag.attrs are object-field nodes. We look for "from"/"to".
            // For now emit a placeholder; Phase 4 can wire this properly.
        },
        .comp_connect => |cc| {
            // @connect: emit connects_via edge between from_expr and to_expr.
            if (cc.from_expr != null and cc.to_expr != null) {
                const from_txt = getNodePathText(cc.from_expr.?) orelse return;
                const to_txt = getNodePathText(cc.to_expr.?) orelse return;
                try l.edges.append(l.arena, .{
                    .src = try l.arena.dupe(u8, from_txt),
                    .dst = try l.arena.dupe(u8, to_txt),
                    .kind = .connects_via,
                });
            }
        },
        else => {},
    }
}

// ─── Annotation edge lowering ───────────────────────────────────────────────

fn lowerAnnotationEdge(l: *Lowerer, ann: ast.Annotation, parent_id: []const u8) !void {
    // @trace:<<satisfies>> → satisfies edge
    // @trace            → traces edge
    if (!std.mem.eql(u8, ann.name, "trace")) return;

    if (ann.operator != null and
        std.mem.eql(u8, ann.operator.?, "satisfies") and
        ann.target_segments.len > 0)
    {
        const target_id = try std.mem.join(l.arena, ".", ann.target_segments);
        try l.edges.append(l.arena, .{
            .src = try internId(l, parent_id),
            .dst = target_id,
            .kind = .satisfies,
        });
    } else if (ann.target_segments.len > 0) {
        const target_id = try std.mem.join(l.arena, ".", ann.target_segments);
        try l.edges.append(l.arena, .{
            .src = try internId(l, parent_id),
            .dst = target_id,
            .kind = .traces,
        });
    }
}

// ─── Pass 3: Build adjacency indexes ──────────────────────────────────────

fn pass3BuildIndexes(arena: std.mem.Allocator, doc: *ir.Document) !void {
    // ── incoming_index: dst_id → []EdgeRef ───────────────────────────────

    // Count edges per destination.
    var dst_counts = std.StringHashMap(usize).init(arena);
    for (doc.edges) |*edge| {
        const gop = try dst_counts.getOrPutValue(edge.dst, 0);
        gop.value_ptr.* += 1;
    }

    // Allocate slices.
    var incoming_map = std.StringHashMap([]ir.EdgeRef).init(arena);
    var dst_it = dst_counts.iterator();
    while (dst_it.next()) |entry| {
        const slice = try arena.alloc(ir.EdgeRef, entry.value_ptr.*);
        try incoming_map.put(entry.key_ptr.*, slice);
    }

    // Fill with EdgeRefs.
    var fill_counts = std.StringHashMap(usize).init(arena);
    for (doc.edges) |*edge| {
        const slice_ptr = incoming_map.getPtr(edge.dst).?;
        const idx = (try fill_counts.getOrPutValue(edge.dst, 0)).value_ptr.*;
        slice_ptr.*[idx] = ir.EdgeRef{ .edge = edge };
        (try fill_counts.getOrPutValue(edge.dst, 0)).value_ptr.* = idx + 1;
    }

    doc.incoming_index = incoming_map;

    // ── children_index: src_id → [][]const u8 (child IDs) ───────────────

    // Collect child IDs per parent via 'contains' edges.
    var children_lists = std.StringHashMap(std.ArrayList([]const u8)).init(arena);
    for (doc.edges) |edge| {
        if (edge.kind != .contains) continue;
        const gop = try children_lists.getOrPut(edge.src);
        if (!gop.found_existing) {
            gop.value_ptr.* = .empty;
        }
        try gop.value_ptr.append(arena, edge.dst);
    }

    // Convert to owned slices.
    var children_map = std.StringHashMap([][]const u8).init(arena);
    var cl_it = children_lists.iterator();
    while (cl_it.next()) |entry| {
        const owned_slice = try entry.value_ptr.toOwnedSlice(arena);
        try children_map.put(entry.key_ptr.*, owned_slice);
    }
    doc.children_index = children_map;
}

// ─── Annotation lowering ──────────────────────────────────────────────────

fn lowerAnnotations(l: *Lowerer, annotations: []*ast.Node, payload: *ir.IrPayload) !void {
    var assumes: std.ArrayList([]const u8) = .empty;
    var concerns: std.ArrayList([]const u8) = .empty;
    var confidence: ?f64 = null;
    var rationale: ?[]const u8 = null;
    var sim_bindings: std.ArrayList(ir.SimBinding) = .empty;

    for (annotations) |ann_node| {
        if (ann_node.kind != .annotation) continue;
        const ann = ann_node.payload.annotation;

        if (std.mem.eql(u8, ann.name, "confidence")) {
            if (ann.value) |val| {
                if (getNodeLiteralText(val)) |t| {
                    confidence = std.fmt.parseFloat(f64, t) catch null;
                }
            }
        } else if (std.mem.eql(u8, ann.name, "rationale")) {
            if (ann.value) |val| {
                if (getNodeLiteralText(val)) |t| {
                    rationale = try l.arena.dupe(u8, stripQuotes(t));
                }
            }
        } else if (std.mem.eql(u8, ann.name, "assumes")) {
            if (ann.value) |val| {
                if (getNodeLiteralText(val)) |t| {
                    try assumes.append(l.arena, try l.arena.dupe(u8, stripQuotes(t)));
                }
            }
        } else if (std.mem.eql(u8, ann.name, "concerns")) {
            if (ann.value) |val| {
                if (getNodeLiteralText(val)) |t| {
                    try concerns.append(l.arena, try l.arena.dupe(u8, stripQuotes(t)));
                }
            }
        } else if (ann.category != null and std.mem.eql(u8, ann.category.?, "simulation")) {
            const op_str = ann.operator orelse continue;
            const operator: ir.SimOperator = if (std.mem.eql(u8, op_str, "computes"))
                .computes
            else if (std.mem.eql(u8, op_str, "validates against") or
                std.mem.eql(u8, op_str, "validates_against"))
                .validates_against
            else
                continue;

            const target = if (ann.target_segments.len > 0)
                try std.mem.join(l.arena, ".", ann.target_segments)
            else
                continue;

            var sim = ir.SimBinding{
                .operator = operator,
                .target = target,
            };

            if (ann.body) |body_node| {
                if (body_node.kind == .annotation_body) {
                    for (body_node.payload.annotation_body.fields) |field| {
                        const v = getNodeLiteralText(field.value) orelse continue;
                        const vs = stripQuotes(v);
                        if (std.mem.eql(u8, field.key, "equation")) {
                            sim.equation = try l.arena.dupe(u8, vs);
                        } else if (std.mem.eql(u8, field.key, "tool")) {
                            sim.tool = try l.arena.dupe(u8, vs);
                        } else if (std.mem.eql(u8, field.key, "fidelity")) {
                            sim.fidelity = try l.arena.dupe(u8, vs);
                        } else if (std.mem.eql(u8, field.key, "entry")) {
                            sim.entry = try l.arena.dupe(u8, vs);
                        }
                    }
                }
            }

            try sim_bindings.append(l.arena, sim);
        }
    }

    const has_meta = assumes.items.len > 0 or concerns.items.len > 0 or
        confidence != null or rationale != null;
    if (has_meta) {
        payload.agent_metadata = ir.AgentMetadata{
            .assumes = try assumes.toOwnedSlice(l.arena),
            .concerns = try concerns.toOwnedSlice(l.arena),
            .confidence = confidence,
            .rationale = rationale,
        };
    }

    if (sim_bindings.items.len > 0) {
        payload.simulation_bindings = try sim_bindings.toOwnedSlice(l.arena);
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────

fn emitPackageNode(l: *Lowerer, id: []const u8, span: ast.Span) !void {
    const interned_id = try internId(l, id);
    const simple_name = blk: {
        const dot_pos = std.mem.lastIndexOf(u8, id, ".") orelse break :blk id;
        break :blk id[dot_pos + 1 ..];
    };

    const ir_node = try l.arena.create(ir.IrNode);
    ir_node.* = .{
        .id = interned_id,
        .kind = .package,
        .span = span,
        .source_file = try l.arena.dupe(u8, l.source_file),
        .payload = .{ .name = try l.arena.dupe(u8, simple_name) },
    };
    try l.elements.put(interned_id, ir_node);
}

/// Combine parent_id + "." + name into a qualified ID in the arena.
/// If parent_id is empty, returns a copy of name.
fn qualifyId(arena: std.mem.Allocator, parent_id: []const u8, name: []const u8) ![]const u8 {
    if (parent_id.len == 0) {
        return try arena.dupe(u8, name);
    }
    return try std.fmt.allocPrint(arena, "{s}.{s}", .{ parent_id, name });
}

fn lowerModifiers(l: *Lowerer, mods: []ast.Modifier) ![]const []const u8 {
    if (mods.len == 0) return &.{};
    var result = try l.arena.alloc([]const u8, mods.len);
    for (mods, 0..) |m, i| {
        result[i] = switch (m) {
            .abstract => "abstract",
            .derived => "derived",
            .readonly => "readonly",
            .ordered => "ordered",
            .nonunique => "nonunique",
            .individual => "individual",
            .variation => "variation",
            .portion => "portion",
            .end_kw => "end",
            .ref_kw => "ref",
        };
    }
    return result;
}

/// Extract text from a literal/identifier AST node.
fn getNodeLiteralText(node: *ast.Node) ?[]const u8 {
    return switch (node.payload) {
        .string_literal => |s| s.value,
        .int_literal => |i| i.text,
        .real_literal => |r| r.text,
        .identifier => |id| id.name,
        .boolean_literal => |b| if (b.value) "true" else "false",
        else => null,
    };
}

/// Extract a dot-path string from an identifier or member_access node.
/// Used for @connect from/to expressions.
fn getNodePathText(node: *ast.Node) ?[]const u8 {
    return switch (node.payload) {
        .identifier => |id| id.name,
        .string_literal => |s| s.value,
        else => null,
    };
}

/// Strip surrounding double-quotes from a string literal value.
fn stripQuotes(s: []const u8) []const u8 {
    if (s.len >= 2 and s[0] == '"' and s[s.len - 1] == '"') {
        return s[1 .. s.len - 1];
    }
    return s;
}

/// Return the ElementDef payload if the node is a definition kind.
/// NOTE: constraint_def removed here (Phase 05.2 — ConstraintDefinition != ElementDef).
/// Use constraintDefOf() for constraint_def nodes.
fn elemDefOf(node: *ast.Node) ?ast.ElementDef {
    return switch (node.payload) {
        .part_def => |e| e,
        .port_def => |e| e,
        .action_def => |e| e,
        .state_def => |e| e,
        .attribute_def => |e| e,
        .item_def => |e| e,
        .interface_def => |e| e,
        .connection_def => |e| e,
        .flow_def => |e| e,
        .allocation_def => |e| e,
        .requirement_def => |e| e,
        // constraint_def uses ConstraintDefinition — see constraintDefOf()
        .need_def => |e| e,
        .use_case_def => |e| e,
        else => null,
    };
}

/// Return the ConstraintDefinition payload if the node is a constraint_def.
fn constraintDefOf(node: *ast.Node) ?ast.ConstraintDefinition {
    return switch (node.payload) {
        .constraint_def => |c| c,
        else => null,
    };
}

/// Return the ElementUsage payload if the node is a usage kind.
fn elemUsageOf(node: *ast.Node) ?ast.ElementUsage {
    return switch (node.payload) {
        .part_usage => |e| e,
        .port_usage => |e| e,
        .attribute_usage => |e| e,
        .action_usage => |e| e,
        .actor_usage => |e| e,
        .subject_usage => |e| e,
        .state_usage => |e| e,
        .item_usage => |e| e,
        .interface_usage => |e| e,
        .connection_usage => |e| e,
        .flow_usage => |e| e,
        .allocation_usage => |e| e,
        .requirement_usage => |e| e,
        .constraint_usage => |e| e,
        .need_usage => |e| e,
        .use_case_usage => |e| e,
        else => null,
    };
}

/// Map an AST definition NodeKind to the corresponding IR NodeKind.
fn astKindToIrKind(kind: ast.NodeKind) ?ir.NodeKind {
    return switch (kind) {
        .part_def => .part_def,
        .port_def => .port_def,
        .action_def => .action_def,
        .state_def => .state_def,
        .attribute_def => .attribute_def,
        .item_def => .item_def,
        .interface_def => .interface_def,
        .connection_def => .connection_def,
        .flow_def => .flow_def,
        .allocation_def => .allocation_def,
        .requirement_def => .requirement_def,
        .constraint_def => .constraint_def,
        .need_def => .need_def,
        .use_case_def => .use_case_def,
        .calc_def => .calc_def,  // Phase 05.2
        else => null,
    };
}

/// Map an AST usage NodeKind to the corresponding IR NodeKind.
fn astUsageKindToIrKind(kind: ast.NodeKind) ir.NodeKind {
    return switch (kind) {
        .part_usage => .part_usage,
        .port_usage => .port_usage,
        .attribute_usage => .attribute_usage,
        .action_usage => .part_usage, // no action_usage in IR
        .actor_usage => .part_usage,
        .subject_usage => .part_usage,
        .state_usage => .state_def,
        .item_usage => .part_usage,
        .interface_usage => .interface_def,
        .connection_usage => .connect,
        .flow_usage => .flow_def,
        .allocation_usage => .allocate,
        .requirement_usage => .requirement_def,
        .constraint_usage => .constraint_def,
        .need_usage => .need_def,
        .use_case_usage => .use_case_def,
        else => .annotation, // fallback for unexpected usage kinds
    };
}
