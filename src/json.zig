//! AST + token + diagnostic JSON emission (D-04, D-18).
//!
//! Three emitters, all hand-rolled to guarantee deterministic key order
//! per RESEARCH §Pitfall 5 (std.json.Stringify can reorder fields across
//! Zig versions — issue #25233):
//!
//!   emitAst         — Wave-0 envelope with `root: null` (Plan 03/04 add
//!                     the recursive writeNode pass).
//!   emitTokens      — Plan 02 token snapshot emitter. Output:
//!                       {"v":1,"mode":"<deal|dealx>","tokens":[
//!                         {"k":"<tag>","span":[s,e]},
//!                         {"k":"ident","span":[s,e],"text":"foo"}, ...
//!                       ]}
//!                     Top-level keys ordered: v, mode, tokens.
//!                     Per-token keys ordered: k, span, then optional
//!                     text (alphabetical w.r.t. k/span/text — D-18
//!                     invariant; text comes after span because s<t).
//!   emitDiagnostics — Wave-0 empty array; Plan 05 fills.

const std = @import("std");
const ast = @import("ast");
const diagnostics = @import("diagnostics");
const lexer = @import("lexer");
const sema = @import("sema");
const ir = @import("ir");

/// Emit the AST JSON envelope. Top-level key order per D-18: v, mode,
/// filename, root. Each node uses kind-tagged payload with alphabetical
/// field order per D-18. Hand-rolled writer per RESEARCH §Pitfall 5.
pub fn emitAst(
    allocator: std.mem.Allocator,
    root: ?*ast.Node,
    mode: ast.Mode,
    filename: []const u8,
) ![]const u8 {
    const tag: []const u8 = switch (mode) {
        .deal => "deal",
        .dealx => "dealx",
    };

    var buf: std.ArrayList(u8) = .empty;
    defer buf.deinit(allocator);

    try buf.appendSlice(allocator, "{\"v\":1,\"mode\":\"");
    try buf.appendSlice(allocator, tag);
    try buf.appendSlice(allocator, "\",\"filename\":\"");
    try appendJsonStringEscaped(allocator, &buf, filename);
    try buf.appendSlice(allocator, "\",\"root\":");
    if (root) |r| {
        try writeNode(allocator, &buf, r);
    } else {
        try buf.appendSlice(allocator, "null");
    }
    try buf.append(allocator, '}');

    return try buf.toOwnedSlice(allocator);
}

/// Recursively write a Node as JSON. Field order: k, span, then payload
/// fields in alphabetical order (D-18).
fn writeNode(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), node: *const ast.Node) std.mem.Allocator.Error!void {
    try buf.append(allocator, '{');

    // "k": "<kind>"
    try buf.appendSlice(allocator, "\"k\":\"");
    try buf.appendSlice(allocator, nodekindName(node.kind));
    try buf.appendSlice(allocator, "\"");

    // "span": [start, end]
    try buf.appendSlice(allocator, ",\"span\":[");
    try appendU32(allocator, buf, node.span.start);
    try buf.append(allocator, ',');
    try appendU32(allocator, buf, node.span.end);
    try buf.append(allocator, ']');

    // Payload fields (alphabetical per D-18).
    try writePayload(allocator, buf, &node.payload);

    try buf.append(allocator, '}');
}

/// Write a slice of nodes as a JSON array.
fn writeNodeArray(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), nodes: []*const ast.Node) std.mem.Allocator.Error!void {
    try buf.append(allocator, '[');
    for (nodes, 0..) |n, i| {
        if (i > 0) try buf.append(allocator, ',');
        try writeNode(allocator, buf, n);
    }
    try buf.append(allocator, ']');
}

fn writeNodeArrayMut(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), nodes: []const *ast.Node) std.mem.Allocator.Error!void {
    try buf.append(allocator, '[');
    for (nodes, 0..) |n, i| {
        if (i > 0) try buf.append(allocator, ',');
        try writeNode(allocator, buf, n);
    }
    try buf.append(allocator, ']');
}

fn writeOptNode(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), n: ?*ast.Node) std.mem.Allocator.Error!void {
    if (n) |node| {
        try writeNode(allocator, buf, node);
    } else {
        try buf.appendSlice(allocator, "null");
    }
}

fn writeStringArray(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), items: []const []const u8) !void {
    try buf.append(allocator, '[');
    for (items, 0..) |item, i| {
        if (i > 0) try buf.append(allocator, ',');
        try buf.append(allocator, '"');
        try appendJsonStringEscaped(allocator, buf, item);
        try buf.append(allocator, '"');
    }
    try buf.append(allocator, ']');
}

fn writeStr(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), s: []const u8) !void {
    try buf.append(allocator, '"');
    try appendJsonStringEscaped(allocator, buf, s);
    try buf.append(allocator, '"');
}

fn writeOptStr(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), s: ?[]const u8) !void {
    if (s) |v| {
        try writeStr(allocator, buf, v);
    } else {
        try buf.appendSlice(allocator, "null");
    }
}

/// Write a slice of Comment values as a JSON array.
/// Each comment object has alphabetical key order per D-18: kind, span, text.
fn writeCommentArray(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), comments: []const ast.Comment) !void {
    try buf.append(allocator, '[');
    for (comments, 0..) |c, i| {
        if (i > 0) try buf.append(allocator, ',');
        try buf.appendSlice(allocator, "{\"kind\":\"");
        try buf.appendSlice(allocator, switch (c.kind) {
            .line => "line",
            .block => "block",
        });
        try buf.appendSlice(allocator, "\",\"span\":[");
        try appendU32(allocator, buf, c.span.start);
        try buf.append(allocator, ',');
        try appendU32(allocator, buf, c.span.end);
        try buf.appendSlice(allocator, "],\"text\":");
        try writeStr(allocator, buf, c.text);
        try buf.append(allocator, '}');
    }
    try buf.append(allocator, ']');
}

/// Write a DocComment pointer as null or {"text":"..."} (D-29).
fn writeOptDocComment(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), dc: ?*const ast.DocComment) !void {
    if (dc) |d| {
        try buf.appendSlice(allocator, "{\"text\":");
        try writeStr(allocator, buf, d.text);
        try buf.append(allocator, '}');
    } else {
        try buf.appendSlice(allocator, "null");
    }
}

fn writeBool(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), b: bool) !void {
    try buf.appendSlice(allocator, if (b) "true" else "false");
}

// ─── Behavioral surface helpers (Stage-2 S2.2) ──────────────────────────────

fn endpointName(e: ast.ControlEndpoint) []const u8 {
    return switch (e) {
        .start => "start",
        .done => "done",
        .terminate => "terminate",
    };
}

/// A Guard `{ "expr": node|null, "is_else": bool }` (alphabetical).
fn writeGuard(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), g: ast.Guard) !void {
    try buf.appendSlice(allocator, "{\"expr\":");
    try writeOptNode(allocator, buf, g.expr);
    try buf.appendSlice(allocator, ",\"is_else\":");
    try writeBool(allocator, buf, g.is_else);
    try buf.append(allocator, '}');
}

fn writeOptGuard(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), g: ?ast.Guard) !void {
    if (g) |gg| {
        try writeGuard(allocator, buf, gg);
    } else {
        try buf.appendSlice(allocator, "null");
    }
}

/// SuccessionStep array: each `{ "guard": Guard|null, "ref": node }`.
fn writeSuccessionSteps(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), steps: []const ast.SuccessionStep) !void {
    try buf.append(allocator, '[');
    for (steps, 0..) |s, i| {
        if (i > 0) try buf.append(allocator, ',');
        try buf.appendSlice(allocator, "{\"guard\":");
        try writeOptGuard(allocator, buf, s.guard);
        try buf.appendSlice(allocator, ",\"ref\":");
        try writeNode(allocator, buf, s.ref);
        try buf.append(allocator, '}');
    }
    try buf.append(allocator, ']');
}

/// DecideBranch array: each `{ "guard": Guard, "target": node }`.
fn writeDecideBranches(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), branches: []const ast.DecideBranch) !void {
    try buf.append(allocator, '[');
    for (branches, 0..) |b, i| {
        if (i > 0) try buf.append(allocator, ',');
        try buf.appendSlice(allocator, "{\"guard\":");
        try writeGuard(allocator, buf, b.guard);
        try buf.appendSlice(allocator, ",\"target\":");
        try writeNode(allocator, buf, b.target);
        try buf.append(allocator, '}');
    }
    try buf.append(allocator, ']');
}

fn writeU32(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), n: u32) !void {
    try appendU32(allocator, buf, n);
}

fn writeOptU32(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), n: ?u32) !void {
    if (n) |v| {
        try appendU32(allocator, buf, v);
    } else {
        try buf.appendSlice(allocator, "null");
    }
}

/// Write payload fields in alphabetical order per D-18.
/// This is an exhaustive switch — adding a NodeKind without a branch is
/// a compile error (Zig 0.16.0 tagged-union exhaustive switch).
fn writePayload(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), payload: *const ast.Payload) std.mem.Allocator.Error!void {
    switch (payload.*) {
        .deal_file => |p| {
            // Alphabetical: definitions, exports, header, imports, package_decl
            try buf.appendSlice(allocator, ",\"definitions\":");
            try writeNodeArrayMut(allocator, buf, p.definitions);
            try buf.appendSlice(allocator, ",\"exports\":");
            try writeNodeArrayMut(allocator, buf, p.exports);
            try buf.appendSlice(allocator, ",\"header\":");
            try writeOptNode(allocator, buf, p.header);
            try buf.appendSlice(allocator, ",\"imports\":");
            try writeNodeArrayMut(allocator, buf, p.imports);
            try buf.appendSlice(allocator, ",\"package_decl\":");
            try writeOptNode(allocator, buf, p.package_decl);
        },
        .dealx_file => |p| {
            try buf.appendSlice(allocator, ",\"header\":");
            try writeOptNode(allocator, buf, p.header);
            try buf.appendSlice(allocator, ",\"imports\":");
            try writeNodeArrayMut(allocator, buf, p.imports);
            try buf.appendSlice(allocator, ",\"package_decl\":");
            try writeOptNode(allocator, buf, p.package_decl);
            try buf.appendSlice(allocator, ",\"root_tags\":");
            try writeNodeArrayMut(allocator, buf, p.root_tags);
        },
        .header_block => |p| {
            // fields
            try buf.appendSlice(allocator, ",\"fields\":[");
            for (p.fields, 0..) |f, i| {
                if (i > 0) try buf.append(allocator, ',');
                try buf.appendSlice(allocator, "{\"key\":");
                try writeStr(allocator, buf, f.key);
                try buf.appendSlice(allocator, ",\"span\":[");
                try appendU32(allocator, buf, f.span.start);
                try buf.append(allocator, ',');
                try appendU32(allocator, buf, f.span.end);
                try buf.appendSlice(allocator, "],\"value\":");
                try writeStr(allocator, buf, f.value);
                try buf.append(allocator, '}');
            }
            try buf.append(allocator, ']');
        },
        .package_decl => |p| {
            // segments
            try buf.appendSlice(allocator, ",\"segments\":");
            try writeStringArray(allocator, buf, p.segments);
        },
        .import_decl => |p| {
            // items, kind, path
            try buf.appendSlice(allocator, ",\"items\":[");
            for (p.items, 0..) |item, i| {
                if (i > 0) try buf.append(allocator, ',');
                try buf.appendSlice(allocator, "{\"alias\":");
                try writeOptStr(allocator, buf, item.alias);
                try buf.appendSlice(allocator, ",\"name\":");
                try writeStr(allocator, buf, item.name);
                try buf.append(allocator, '}');
            }
            try buf.append(allocator, ']');
            try buf.appendSlice(allocator, ",\"kind\":\"");
            try buf.appendSlice(allocator, switch (p.kind) {
                .simple => "simple",
                .named => "named",
                .wildcard => "wildcard",
                .alias => "alias",
            });
            try buf.append(allocator, '"');
            try buf.appendSlice(allocator, ",\"path\":");
            try writeStringArray(allocator, buf, p.path);
        },
        .export_decl => |p| {
            // items, module
            try buf.appendSlice(allocator, ",\"items\":");
            try writeStringArray(allocator, buf, p.items);
            try buf.appendSlice(allocator, ",\"module\":");
            try writeStr(allocator, buf, p.module);
        },

        // ── Element definitions (alphabetical field order per D-18) ──
        // NOTE: constraint_def removed from this group (Phase 05.2 — ConstraintDefinition has
        // a different struct from ElementDef). constraint_def gets its own arm below.
        .part_def, .port_def, .action_def, .state_def, .attribute_def,
        .item_def, .interface_def, .connection_def, .flow_def,
        .allocation_def, .requirement_def, .need_def,
        .use_case_def, .actor_def => {
            const elem = getElementDef(payload);
            try writeElementDefFields(allocator, buf, elem);
        },

        // ── constraint_def — ConstraintDefinition (Phase 05.2) ───────
        .constraint_def => |p| {
            // annotations, body, direction, doc, doc_comment, leading_comments,
            // modifiers, name, params, specializes (alphabetical per D-18)
            try buf.appendSlice(allocator, ",\"annotations\":");
            try writeNodeArrayMut(allocator, buf, p.annotations);
            try buf.appendSlice(allocator, ",\"body\":");
            try writeNode(allocator, buf, p.body);
            try buf.appendSlice(allocator, ",\"direction\":\"");
            try buf.appendSlice(allocator, directionName(p.direction));
            try buf.appendSlice(allocator, "\",\"doc\":");
            try writeOptNode(allocator, buf, p.doc);
            try buf.appendSlice(allocator, ",\"doc_comment\":");
            try writeOptDocComment(allocator, buf, p.doc_comment);
            try buf.appendSlice(allocator, ",\"leading_comments\":");
            try writeCommentArray(allocator, buf, p.leading_comments);
            try buf.appendSlice(allocator, ",\"modifiers\":[");
            for (p.modifiers, 0..) |m, i| {
                if (i > 0) try buf.append(allocator, ',');
                try buf.append(allocator, '"');
                try buf.appendSlice(allocator, modifierName(m));
                try buf.append(allocator, '"');
            }
            try buf.appendSlice(allocator, "],\"name\":");
            try writeStr(allocator, buf, p.name);
            try buf.appendSlice(allocator, ",\"params\":");
            try writeOptNode(allocator, buf, p.params);
            try buf.appendSlice(allocator, ",\"specializes\":");
            try writeOptNode(allocator, buf, p.specializes);
        },

        // ── calc_def (Phase 05.2) ──────────────────────────────────────
        .calc_def => |p| {
            // annotations, body, direction, doc, doc_comment, leading_comments,
            // modifiers, name, params, return_contract, type_node (alphabetical)
            try buf.appendSlice(allocator, ",\"annotations\":");
            try writeNodeArrayMut(allocator, buf, p.annotations);
            try buf.appendSlice(allocator, ",\"body\":");
            try writeNode(allocator, buf, p.body);
            try buf.appendSlice(allocator, ",\"direction\":\"");
            try buf.appendSlice(allocator, directionName(p.direction));
            try buf.appendSlice(allocator, "\",\"doc\":");
            try writeOptNode(allocator, buf, p.doc);
            try buf.appendSlice(allocator, ",\"doc_comment\":");
            try writeOptDocComment(allocator, buf, p.doc_comment);
            try buf.appendSlice(allocator, ",\"leading_comments\":");
            try writeCommentArray(allocator, buf, p.leading_comments);
            try buf.appendSlice(allocator, ",\"modifiers\":[");
            for (p.modifiers, 0..) |m, i| {
                if (i > 0) try buf.append(allocator, ',');
                try buf.append(allocator, '"');
                try buf.appendSlice(allocator, modifierName(m));
                try buf.append(allocator, '"');
            }
            try buf.appendSlice(allocator, "],\"name\":");
            try writeStr(allocator, buf, p.name);
            try buf.appendSlice(allocator, ",\"params\":");
            try writeNode(allocator, buf, p.params);
            try buf.appendSlice(allocator, ",\"return_contract\":");
            try writeOptNode(allocator, buf, p.return_contract);
            try buf.appendSlice(allocator, ",\"type_node\":");
            try writeNode(allocator, buf, p.type_node);
        },

        // ── param_list (Phase 05.2) ───────────────────────────────────
        .param_list => |p| {
            try buf.appendSlice(allocator, ",\"params\":");
            try writeNodeArrayMut(allocator, buf, p.params);
        },

        // ── param_decl (Phase 05.2) ───────────────────────────────────
        .param_decl => |p| {
            // direction, multiplicity, name, type_node (alphabetical)
            try buf.appendSlice(allocator, ",\"direction\":\"");
            try buf.appendSlice(allocator, directionName(p.direction));
            try buf.appendSlice(allocator, "\",\"multiplicity\":");
            try writeOptNode(allocator, buf, p.multiplicity);
            try buf.appendSlice(allocator, ",\"name\":");
            try writeStr(allocator, buf, p.name);
            try buf.appendSlice(allocator, ",\"type_node\":");
            try writeNode(allocator, buf, p.type_node);
        },

        // ── return_contract (Phase 05.2) ──────────────────────────────
        .return_contract => |p| {
            try buf.appendSlice(allocator, ",\"items\":");
            try writeNodeArrayMut(allocator, buf, p.items);
        },

        // ── precision_spec (Phase 05.2) ───────────────────────────────
        .precision_spec => |p| {
            // kind, value (alphabetical)
            try buf.appendSlice(allocator, ",\"kind\":\"");
            try buf.appendSlice(allocator, switch (p.kind) {
                .sig_figures => "sig_figures",
                .tolerance_absolute => "tolerance_absolute",
                .tolerance_relative => "tolerance_relative",
            });
            try buf.appendSlice(allocator, "\",\"value\":");
            try writeNode(allocator, buf, p.value);
        },

        // ── constraint_ref (Phase 05.2) ───────────────────────────────
        .constraint_ref => |p| {
            // args, name_segments (alphabetical)
            try buf.appendSlice(allocator, ",\"args\":");
            if (p.args) |args| {
                try writeNodeArrayMut(allocator, buf, args);
            } else {
                try buf.appendSlice(allocator, "null");
            }
            try buf.appendSlice(allocator, ",\"name_segments\":");
            try writeStringArray(allocator, buf, p.name_segments);
        },

        // ── constraint_body (Phase 05.2) ──────────────────────────────
        .constraint_body => |p| {
            try buf.appendSlice(allocator, ",\"members\":");
            try writeNodeArrayMut(allocator, buf, p.members);
        },

        // ── calc_body (Phase 05.2) ────────────────────────────────────
        .calc_body => |p| {
            // annotations, bindings, return_stmt (alphabetical)
            try buf.appendSlice(allocator, ",\"annotations\":");
            try writeNodeArrayMut(allocator, buf, p.annotations);
            try buf.appendSlice(allocator, ",\"bindings\":");
            try writeNodeArrayMut(allocator, buf, p.bindings);
            try buf.appendSlice(allocator, ",\"return_stmt\":");
            try writeNode(allocator, buf, p.return_stmt);
        },

        // ── require_statement (Phase 05.2) ────────────────────────────
        .require_statement => |p| {
            // condition, precision (alphabetical)
            try buf.appendSlice(allocator, ",\"condition\":");
            try writeNode(allocator, buf, p.condition);
            try buf.appendSlice(allocator, ",\"precision\":");
            try writeOptNode(allocator, buf, p.precision);
        },

        // ── Behavioral surface (Stage-2 S2.2). Fields alphabetical (D-18). ──
        .action_body => |p| {
            try buf.appendSlice(allocator, ",\"members\":");
            try writeNodeArrayMut(allocator, buf, p.members);
        },
        .pin_decl => |p| {
            // default_value, direction, multiplicity, name, type_node
            try buf.appendSlice(allocator, ",\"default_value\":");
            try writeOptNode(allocator, buf, p.default_value);
            try buf.appendSlice(allocator, ",\"direction\":\"");
            try buf.appendSlice(allocator, directionName(p.direction));
            try buf.appendSlice(allocator, "\",\"multiplicity\":");
            try writeOptNode(allocator, buf, p.multiplicity);
            try buf.appendSlice(allocator, ",\"name\":");
            try writeStr(allocator, buf, p.name);
            try buf.appendSlice(allocator, ",\"type_node\":");
            try writeNode(allocator, buf, p.type_node);
        },
        .succession_chain => |p| {
            try buf.appendSlice(allocator, ",\"steps\":");
            try writeSuccessionSteps(allocator, buf, p.steps);
        },
        .control_ref => |p| {
            try buf.appendSlice(allocator, ",\"endpoint\":\"");
            try buf.appendSlice(allocator, endpointName(p.endpoint));
            try buf.appendSlice(allocator, "\"");
        },
        .decide_block => |p| {
            try buf.appendSlice(allocator, ",\"branches\":");
            try writeDecideBranches(allocator, buf, p.branches);
        },
        .par_block => |p| {
            // branches, exit
            try buf.appendSlice(allocator, ",\"branches\":");
            try writeNodeArrayMut(allocator, buf, p.branches);
            try buf.appendSlice(allocator, ",\"exit\":");
            try writeOptNode(allocator, buf, p.exit);
        },
        .loop_statement => |p| {
            // body, guard, iterable, kind, var_name
            try buf.appendSlice(allocator, ",\"body\":");
            try writeNode(allocator, buf, p.body);
            try buf.appendSlice(allocator, ",\"guard\":");
            try writeOptNode(allocator, buf, p.guard);
            try buf.appendSlice(allocator, ",\"iterable\":");
            try writeOptNode(allocator, buf, p.iterable);
            try buf.appendSlice(allocator, ",\"kind\":\"");
            try buf.appendSlice(allocator, switch (p.kind) {
                .while_loop => "while_loop",
                .until_loop => "until_loop",
                .for_loop => "for_loop",
            });
            try buf.appendSlice(allocator, "\",\"var_name\":");
            try writeOptStr(allocator, buf, p.var_name);
        },
        .send_action => |p| {
            // payload_segments, target_segments
            try buf.appendSlice(allocator, ",\"payload_segments\":");
            try writeStringArray(allocator, buf, p.payload_segments);
            try buf.appendSlice(allocator, ",\"target_segments\":");
            if (p.target_segments) |t| {
                try writeStringArray(allocator, buf, t);
            } else {
                try buf.appendSlice(allocator, "null");
            }
        },
        .accept_action => |p| {
            // guard, trigger_segments
            try buf.appendSlice(allocator, ",\"guard\":");
            try writeOptNode(allocator, buf, p.guard);
            try buf.appendSlice(allocator, ",\"trigger_segments\":");
            try writeStringArray(allocator, buf, p.trigger_segments);
        },
        .assign_action => |p| {
            // target_segments, value
            try buf.appendSlice(allocator, ",\"target_segments\":");
            try writeStringArray(allocator, buf, p.target_segments);
            try buf.appendSlice(allocator, ",\"value\":");
            try writeNode(allocator, buf, p.value);
        },
        .perform_statement => |p| {
            try buf.appendSlice(allocator, ",\"call\":");
            try writeNode(allocator, buf, p.call);
        },
        .item_flow_statement => |p| {
            // flow_type_segments, source, target
            try buf.appendSlice(allocator, ",\"flow_type_segments\":");
            if (p.flow_type_segments) |ft| {
                try writeStringArray(allocator, buf, ft);
            } else {
                try buf.appendSlice(allocator, "null");
            }
            try buf.appendSlice(allocator, ",\"source\":");
            try writeNode(allocator, buf, p.source);
            try buf.appendSlice(allocator, ",\"target\":");
            try writeNode(allocator, buf, p.target);
        },
        .binding_statement => |p| {
            // lhs, rhs
            try buf.appendSlice(allocator, ",\"lhs\":");
            try writeNode(allocator, buf, p.lhs);
            try buf.appendSlice(allocator, ",\"rhs\":");
            try writeNode(allocator, buf, p.rhs);
        },
        .escape_node => |p| {
            // body, name, type_segments
            try buf.appendSlice(allocator, ",\"body\":");
            try writeOptNode(allocator, buf, p.body);
            try buf.appendSlice(allocator, ",\"name\":");
            try writeStr(allocator, buf, p.name);
            try buf.appendSlice(allocator, ",\"type_segments\":");
            try writeStringArray(allocator, buf, p.type_segments);
        },
        .escape_succession => |p| {
            // guard, source_segments, target_segments
            try buf.appendSlice(allocator, ",\"guard\":");
            try writeOptGuard(allocator, buf, p.guard);
            try buf.appendSlice(allocator, ",\"source_segments\":");
            try writeStringArray(allocator, buf, p.source_segments);
            try buf.appendSlice(allocator, ",\"target_segments\":");
            try writeStringArray(allocator, buf, p.target_segments);
        },
        .entry_do_exit => |p| {
            // behavior, kind
            try buf.appendSlice(allocator, ",\"behavior\":");
            try writeNode(allocator, buf, p.behavior);
            try buf.appendSlice(allocator, ",\"kind\":\"");
            try buf.appendSlice(allocator, switch (p.kind) {
                .entry => "entry",
                .do_action => "do",
                .exit => "exit",
            });
            try buf.appendSlice(allocator, "\"");
        },
        .transition_statement => |p| {
            // effect, guard, target, trigger_segments
            try buf.appendSlice(allocator, ",\"effect\":");
            try writeOptNode(allocator, buf, p.effect);
            try buf.appendSlice(allocator, ",\"guard\":");
            try writeOptNode(allocator, buf, p.guard);
            try buf.appendSlice(allocator, ",\"target\":");
            try writeNode(allocator, buf, p.target);
            try buf.appendSlice(allocator, ",\"trigger_segments\":");
            try writeStringArray(allocator, buf, p.trigger_segments);
        },
        .target_ref => |p| {
            // endpoint, name_segments
            try buf.appendSlice(allocator, ",\"endpoint\":");
            if (p.endpoint) |e| {
                try writeStr(allocator, buf, endpointName(e));
            } else {
                try buf.appendSlice(allocator, "null");
            }
            try buf.appendSlice(allocator, ",\"name_segments\":");
            try writeStringArray(allocator, buf, p.name_segments);
        },

        // ── Usages (same field set as definitions) ───────────────────
        .part_usage, .port_usage, .attribute_usage, .action_usage,
        .actor_usage, .subject_usage, .state_usage, .item_usage,
        .interface_usage, .connection_usage, .flow_usage,
        .allocation_usage, .requirement_usage, .constraint_usage,
        .need_usage, .use_case_usage => {
            const usage = getElementUsage(payload);
            try writeElementUsageFields(allocator, buf, usage);
        },

        .type_annotation => |p| {
            try buf.appendSlice(allocator, ",\"name_segments\":");
            try writeStringArray(allocator, buf, p.name_segments);
        },
        .multiplicity => |p| {
            // lower, unbounded, upper
            try buf.appendSlice(allocator, ",\"lower\":");
            try appendU32(allocator, buf, p.lower);
            try buf.appendSlice(allocator, ",\"unbounded\":");
            try writeBool(allocator, buf, p.unbounded);
            try buf.appendSlice(allocator, ",\"upper\":");
            try writeOptU32(allocator, buf, p.upper);
        },
        .modifier_list => |p| {
            try buf.appendSlice(allocator, ",\"modifiers\":[");
            for (p.modifiers, 0..) |m, i| {
                if (i > 0) try buf.append(allocator, ',');
                try buf.append(allocator, '"');
                try buf.appendSlice(allocator, modifierName(m));
                try buf.append(allocator, '"');
            }
            try buf.append(allocator, ']');
        },
        .visibility_wrapper => |p| {
            // members, visibility
            try buf.appendSlice(allocator, ",\"members\":");
            try writeNodeArrayMut(allocator, buf, p.members);
            try buf.appendSlice(allocator, ",\"visibility\":\"");
            try buf.appendSlice(allocator, visibilityName(p.visibility));
            try buf.append(allocator, '"');
        },
        .specialization => |p| {
            // op_text, target_segments
            try buf.appendSlice(allocator, ",\"op_text\":");
            try writeStr(allocator, buf, p.op_text);
            try buf.appendSlice(allocator, ",\"target_segments\":");
            try writeStringArray(allocator, buf, p.target_segments);
        },
        .redefinition => |p| {
            // op_text, target_segments, value
            try buf.appendSlice(allocator, ",\"op_text\":");
            try writeStr(allocator, buf, p.op_text);
            try buf.appendSlice(allocator, ",\"target_segments\":");
            try writeStringArray(allocator, buf, p.target_segments);
            try buf.appendSlice(allocator, ",\"value\":");
            try writeOptNode(allocator, buf, p.value);
        },
        .operator_statement => |p| {
            // op_text, target_segments, value
            try buf.appendSlice(allocator, ",\"op_text\":");
            try writeStr(allocator, buf, p.op_text);
            try buf.appendSlice(allocator, ",\"target_segments\":");
            try writeStringArray(allocator, buf, p.target_segments);
            try buf.appendSlice(allocator, ",\"value\":");
            try writeOptNode(allocator, buf, p.value);
        },
        .inline_body => |p| {
            try buf.appendSlice(allocator, ",\"members\":");
            try writeNodeArrayMut(allocator, buf, p.members);
        },
        .structural_relationship => |p| {
            // op_text, target_segments
            try buf.appendSlice(allocator, ",\"op_text\":");
            try writeStr(allocator, buf, p.op_text);
            try buf.appendSlice(allocator, ",\"target_segments\":");
            try writeStringArray(allocator, buf, p.target_segments);
        },
        .annotation => |p| {
            // body, category, name, operator, target_segments, value
            try buf.appendSlice(allocator, ",\"body\":");
            try writeOptNode(allocator, buf, p.body);
            try buf.appendSlice(allocator, ",\"category\":");
            try writeOptStr(allocator, buf, p.category);
            try buf.appendSlice(allocator, ",\"name\":");
            try writeStr(allocator, buf, p.name);
            try buf.appendSlice(allocator, ",\"operator\":");
            try writeOptStr(allocator, buf, p.operator);
            try buf.appendSlice(allocator, ",\"target_segments\":");
            try writeStringArray(allocator, buf, p.target_segments);
            try buf.appendSlice(allocator, ",\"value\":");
            try writeOptNode(allocator, buf, p.value);
        },
        .doc_comment => |p| {
            try buf.appendSlice(allocator, ",\"text\":");
            try writeStr(allocator, buf, p.text);
        },
        .annotation_body => |p| {
            try buf.appendSlice(allocator, ",\"fields\":[");
            for (p.fields, 0..) |f, i| {
                if (i > 0) try buf.append(allocator, ',');
                // key, span, value (alphabetical)
                try buf.appendSlice(allocator, "{\"key\":");
                try writeStr(allocator, buf, f.key);
                try buf.appendSlice(allocator, ",\"span\":[");
                try appendU32(allocator, buf, f.span.start);
                try buf.append(allocator, ',');
                try appendU32(allocator, buf, f.span.end);
                try buf.appendSlice(allocator, "],\"value\":");
                try writeNode(allocator, buf, f.value);
                try buf.append(allocator, '}');
            }
            try buf.append(allocator, ']');
        },
        .verification_block => |p| {
            try buf.appendSlice(allocator, ",\"fields\":[");
            for (p.fields, 0..) |f, i| {
                if (i > 0) try buf.append(allocator, ',');
                // kind, span, value (alphabetical)
                try buf.appendSlice(allocator, "{\"kind\":\"");
                try buf.appendSlice(allocator, verifFieldKindName(f.kind));
                try buf.appendSlice(allocator, "\",\"span\":[");
                try appendU32(allocator, buf, f.span.start);
                try buf.append(allocator, ',');
                try appendU32(allocator, buf, f.span.end);
                try buf.appendSlice(allocator, "],\"value\":");
                try writeNode(allocator, buf, f.value);
                try buf.append(allocator, '}');
            }
            try buf.append(allocator, ']');
        },
        .precondition_block => |p| {
            // conditions, name
            try buf.appendSlice(allocator, ",\"conditions\":");
            try writeNodeArrayMut(allocator, buf, p.conditions);
            try buf.appendSlice(allocator, ",\"name\":");
            try writeStr(allocator, buf, p.name);
        },
        .postcondition_block => |p| {
            // conditions, name
            try buf.appendSlice(allocator, ",\"conditions\":");
            try writeNodeArrayMut(allocator, buf, p.conditions);
            try buf.appendSlice(allocator, ",\"name\":");
            try writeStr(allocator, buf, p.name);
        },
        .binary => |p| {
            // lhs, op, rhs (alphabetical)
            try buf.appendSlice(allocator, ",\"lhs\":");
            try writeNode(allocator, buf, p.lhs);
            try buf.appendSlice(allocator, ",\"op\":\"");
            try buf.appendSlice(allocator, binaryOpName(p.op));
            try buf.appendSlice(allocator, "\",\"rhs\":");
            try writeNode(allocator, buf, p.rhs);
        },
        .unary => |p| {
            // op, operand
            try buf.appendSlice(allocator, ",\"op\":\"");
            try buf.appendSlice(allocator, unaryOpName(p.op));
            try buf.appendSlice(allocator, "\",\"operand\":");
            try writeNode(allocator, buf, p.operand);
        },
        .member_access => |p| {
            // member, receiver
            try buf.appendSlice(allocator, ",\"member\":");
            try writeStr(allocator, buf, p.member);
            try buf.appendSlice(allocator, ",\"receiver\":");
            try writeNode(allocator, buf, p.receiver);
        },
        .call => |p| {
            // args, callee
            try buf.appendSlice(allocator, ",\"args\":");
            try writeNodeArrayMut(allocator, buf, p.args);
            try buf.appendSlice(allocator, ",\"callee\":");
            try writeNode(allocator, buf, p.callee);
        },
        .array_literal => |p| {
            // items (D-04: replaces call(__array__, items) encoding)
            try buf.appendSlice(allocator, ",\"items\":");
            try writeNodeArrayMut(allocator, buf, p.items);
        },
        .identifier => |p| {
            try buf.appendSlice(allocator, ",\"name\":");
            try writeStr(allocator, buf, p.name);
        },
        .int_literal => |p| {
            try buf.appendSlice(allocator, ",\"text\":");
            try writeStr(allocator, buf, p.text);
        },
        .real_literal => |p| {
            try buf.appendSlice(allocator, ",\"text\":");
            try writeStr(allocator, buf, p.text);
        },
        .string_literal => |p| {
            try buf.appendSlice(allocator, ",\"value\":");
            try writeStr(allocator, buf, p.value);
        },
        .template_literal => |p| {
            // parts
            try buf.appendSlice(allocator, ",\"parts\":[");
            for (p.parts, 0..) |part, i| {
                if (i > 0) try buf.append(allocator, ',');
                switch (part) {
                    .text => |t| {
                        try buf.appendSlice(allocator, "{\"text\":");
                        try writeStr(allocator, buf, t);
                        try buf.append(allocator, '}');
                    },
                    .expr => |e| {
                        try buf.appendSlice(allocator, "{\"expr\":");
                        try writeNode(allocator, buf, e);
                        try buf.append(allocator, '}');
                    },
                }
            }
            try buf.append(allocator, ']');
        },
        .boolean_literal => |p| {
            try buf.appendSlice(allocator, ",\"value\":");
            try writeBool(allocator, buf, p.value);
        },
        .interpolation => |p| {
            try buf.appendSlice(allocator, ",\"expr\":");
            try writeNode(allocator, buf, p.expr);
        },

        // ── Composition placeholders (Plan 04) ───────────────────────
        .system_block => |p| {
            try buf.appendSlice(allocator, ",\"children\":");
            try writeNodeArrayMut(allocator, buf, p.children);
            try buf.appendSlice(allocator, ",\"name\":");
            try writeOptStr(allocator, buf, p.name);
        },
        .subsystem_block => |p| {
            try buf.appendSlice(allocator, ",\"children\":");
            try writeNodeArrayMut(allocator, buf, p.children);
            try buf.appendSlice(allocator, ",\"name\":");
            try writeOptStr(allocator, buf, p.name);
        },
        .component_instance => |p| {
            try buf.appendSlice(allocator, ",\"attrs\":");
            try writeNodeArrayMut(allocator, buf, p.attrs);
            try buf.appendSlice(allocator, ",\"name\":");
            try writeOptStr(allocator, buf, p.name);
            try buf.appendSlice(allocator, ",\"type_ref\":");
            try writeOptStr(allocator, buf, p.type_ref);
        },
        .comp_connect => |p| {
            // D-09 keys (LOCKED). Alphabetical per D-18:
            //   "carrying", "from", "other_props", "to", "via".
            try buf.appendSlice(allocator, ",\"carrying\":");
            try writeOptNode(allocator, buf, p.carrying_expr);
            try buf.appendSlice(allocator, ",\"from\":");
            try writeOptNode(allocator, buf, p.from_expr);
            try buf.appendSlice(allocator, ",\"other_props\":");
            try writeNodeArrayMut(allocator, buf, p.other_props);
            try buf.appendSlice(allocator, ",\"to\":");
            try writeOptNode(allocator, buf, p.to_expr);
            try buf.appendSlice(allocator, ",\"via\":");
            try writeOptNode(allocator, buf, p.via_expr);
        },
        .expose_tag => |p| {
            try buf.appendSlice(allocator, ",\"attrs\":");
            try writeNodeArrayMut(allocator, buf, p.attrs);
            try buf.appendSlice(allocator, ",\"target\":");
            try writeOptStr(allocator, buf, p.target);
        },
        .traceability_block => |p| {
            try buf.appendSlice(allocator, ",\"children\":");
            try writeNodeArrayMut(allocator, buf, p.children);
        },
        .allocate_tag => |p| {
            try buf.appendSlice(allocator, ",\"attrs\":");
            try writeNodeArrayMut(allocator, buf, p.attrs);
        },
        .satisfy_block => |p| {
            try buf.appendSlice(allocator, ",\"children\":");
            try writeNodeArrayMut(allocator, buf, p.children);
        },
        .validate_tag => |p| {
            try buf.appendSlice(allocator, ",\"attrs\":");
            try writeNodeArrayMut(allocator, buf, p.attrs);
        },
        .object_literal => |p| {
            // fields, type_name (alphabetical per D-18)
            try buf.appendSlice(allocator, ",\"fields\":[");
            for (p.fields, 0..) |f, i| {
                if (i > 0) try buf.append(allocator, ',');
                // key, span, value (alphabetical)
                try buf.appendSlice(allocator, "{\"key\":");
                try writeStr(allocator, buf, f.key);
                try buf.appendSlice(allocator, ",\"span\":[");
                try appendU32(allocator, buf, f.span.start);
                try buf.append(allocator, ',');
                try appendU32(allocator, buf, f.span.end);
                try buf.appendSlice(allocator, "],\"value\":");
                try writeNode(allocator, buf, f.value);
                try buf.append(allocator, '}');
            }
            try buf.appendSlice(allocator, "],\"type_name\":");
            try writeOptStr(allocator, buf, p.type_name);
        },
    }
}

/// Helper: extract the ElementDef from a definition payload.
fn getElementDef(payload: *const ast.Payload) *const ast.ElementDef {
    return switch (payload.*) {
        .part_def => |*p| p,
        .actor_def => |*p| p,
        .port_def => |*p| p,
        .action_def => |*p| p,
        .state_def => |*p| p,
        .attribute_def => |*p| p,
        .item_def => |*p| p,
        .interface_def => |*p| p,
        .connection_def => |*p| p,
        .flow_def => |*p| p,
        .allocation_def => |*p| p,
        .requirement_def => |*p| p,
        // NOTE: constraint_def removed (Phase 05.2 — ConstraintDefinition != ElementDef)
        .need_def => |*p| p,
        .use_case_def => |*p| p,
        else => unreachable,
    };
}

fn getElementUsage(payload: *const ast.Payload) *const ast.ElementUsage {
    return switch (payload.*) {
        .part_usage => |*p| p,
        .port_usage => |*p| p,
        .attribute_usage => |*p| p,
        .action_usage => |*p| p,
        .actor_usage => |*p| p,
        .subject_usage => |*p| p,
        .state_usage => |*p| p,
        .item_usage => |*p| p,
        .interface_usage => |*p| p,
        .connection_usage => |*p| p,
        .flow_usage => |*p| p,
        .allocation_usage => |*p| p,
        .requirement_usage => |*p| p,
        .constraint_usage => |*p| p,
        .need_usage => |*p| p,
        .use_case_usage => |*p| p,
        else => unreachable,
    };
}

/// Write ElementDef fields in alphabetical order (D-18, D-28, D-29):
/// annotations, direction, doc, doc_comment, leading_comments, members,
/// modifiers, name, specializes, trailing_comments
fn writeElementDefFields(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), elem: *const ast.ElementDef) !void {
    try buf.appendSlice(allocator, ",\"annotations\":");
    try writeNodeArrayMut(allocator, buf, elem.annotations);
    try buf.appendSlice(allocator, ",\"direction\":\"");
    try buf.appendSlice(allocator, directionName(elem.direction));
    try buf.appendSlice(allocator, "\",\"doc\":");
    try writeOptNode(allocator, buf, elem.doc);
    try buf.appendSlice(allocator, ",\"doc_comment\":");
    try writeOptDocComment(allocator, buf, elem.doc_comment);
    try buf.appendSlice(allocator, ",\"leading_comments\":");
    try writeCommentArray(allocator, buf, elem.leading_comments);
    try buf.appendSlice(allocator, ",\"members\":");
    try writeNodeArrayMut(allocator, buf, elem.members);
    try buf.appendSlice(allocator, ",\"modifiers\":[");
    for (elem.modifiers, 0..) |m, i| {
        if (i > 0) try buf.append(allocator, ',');
        try buf.append(allocator, '"');
        try buf.appendSlice(allocator, modifierName(m));
        try buf.append(allocator, '"');
    }
    try buf.appendSlice(allocator, "],\"name\":");
    try writeStr(allocator, buf, elem.name);
    try buf.appendSlice(allocator, ",\"specializes\":");
    try writeOptNode(allocator, buf, elem.specializes);
    try buf.appendSlice(allocator, ",\"trailing_comments\":");
    try writeCommentArray(allocator, buf, elem.trailing_comments);
}

/// Write ElementUsage fields in alphabetical order (D-18, D-28, D-29):
/// annotations, default_value, direction, doc, doc_comment, inline_body,
/// leading_comments, modifiers, multiplicity, name, trailing_comments, type_node
fn writeElementUsageFields(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), u: *const ast.ElementUsage) !void {
    try buf.appendSlice(allocator, ",\"annotations\":");
    try writeNodeArrayMut(allocator, buf, u.annotations);
    try buf.appendSlice(allocator, ",\"default_value\":");
    try writeOptNode(allocator, buf, u.default_value);
    try buf.appendSlice(allocator, ",\"direction\":\"");
    try buf.appendSlice(allocator, directionName(u.direction));
    try buf.appendSlice(allocator, "\",\"doc\":");
    try writeOptNode(allocator, buf, u.doc);
    try buf.appendSlice(allocator, ",\"doc_comment\":");
    try writeOptDocComment(allocator, buf, u.doc_comment);
    try buf.appendSlice(allocator, ",\"inline_body\":");
    try writeOptNode(allocator, buf, u.inline_body);
    try buf.appendSlice(allocator, ",\"leading_comments\":");
    try writeCommentArray(allocator, buf, u.leading_comments);
    try buf.appendSlice(allocator, ",\"modifiers\":[");
    for (u.modifiers, 0..) |m, i| {
        if (i > 0) try buf.append(allocator, ',');
        try buf.append(allocator, '"');
        try buf.appendSlice(allocator, modifierName(m));
        try buf.append(allocator, '"');
    }
    try buf.appendSlice(allocator, "],\"multiplicity\":");
    try writeOptNode(allocator, buf, u.multiplicity);
    try buf.appendSlice(allocator, ",\"name\":");
    try writeStr(allocator, buf, u.name);
    // D-06/D-08: only emit "precision" when non-null to preserve backward-compatible
    // snapshot byte-stability for attribute/usage nodes without a precision contract.
    if (u.precision) |prec| {
        try buf.appendSlice(allocator, ",\"precision\":");
        try writeNode(allocator, buf, prec);
    }
    try buf.appendSlice(allocator, ",\"trailing_comments\":");
    try writeCommentArray(allocator, buf, u.trailing_comments);
    try buf.appendSlice(allocator, ",\"type_node\":");
    try writeOptNode(allocator, buf, u.type_node);
}

fn nodekindName(k: ast.NodeKind) []const u8 {
    return switch (k) {
        .deal_file => "deal_file",
        .dealx_file => "dealx_file",
        .header_block => "header_block",
        .package_decl => "package_decl",
        .import_decl => "import_decl",
        .export_decl => "export_decl",
        .part_def => "part_def",
        .actor_def => "actor_def",
        .port_def => "port_def",
        .action_def => "action_def",
        .state_def => "state_def",
        .attribute_def => "attribute_def",
        .item_def => "item_def",
        .interface_def => "interface_def",
        .connection_def => "connection_def",
        .flow_def => "flow_def",
        .allocation_def => "allocation_def",
        .requirement_def => "requirement_def",
        .constraint_def => "constraint_def",
        .need_def => "need_def",
        .use_case_def => "use_case_def",
        .part_usage => "part_usage",
        .port_usage => "port_usage",
        .attribute_usage => "attribute_usage",
        .action_usage => "action_usage",
        .actor_usage => "actor_usage",
        .subject_usage => "subject_usage",
        .state_usage => "state_usage",
        .item_usage => "item_usage",
        .interface_usage => "interface_usage",
        .connection_usage => "connection_usage",
        .flow_usage => "flow_usage",
        .allocation_usage => "allocation_usage",
        .requirement_usage => "requirement_usage",
        .constraint_usage => "constraint_usage",
        .need_usage => "need_usage",
        .use_case_usage => "use_case_usage",
        .type_annotation => "type_annotation",
        .multiplicity => "multiplicity",
        .modifier_list => "modifier_list",
        .visibility_wrapper => "visibility_wrapper",
        .specialization => "specialization",
        .redefinition => "redefinition",
        .operator_statement => "operator_statement",
        .inline_body => "inline_body",
        .structural_relationship => "structural_relationship",
        .annotation => "annotation",
        .doc_comment => "doc_comment",
        .annotation_body => "annotation_body",
        .verification_block => "verification_block",
        .precondition_block => "precondition_block",
        .postcondition_block => "postcondition_block",
        .binary => "binary",
        .unary => "unary",
        .member_access => "member_access",
        .call => "call",
        .identifier => "identifier",
        .int_literal => "int_literal",
        .real_literal => "real_literal",
        .string_literal => "string_literal",
        .template_literal => "template_literal",
        .boolean_literal => "boolean_literal",
        .interpolation => "interpolation",
        .array_literal => "array_literal",
        // Calc/constraint (Phase 05.2 Wave 2)
        .calc_def => "calc_def",
        .param_list => "param_list",
        .param_decl => "param_decl",
        .return_contract => "return_contract",
        .precision_spec => "precision_spec",
        .constraint_ref => "constraint_ref",
        .constraint_body => "constraint_body",
        .calc_body => "calc_body",
        .require_statement => "require_statement",
        // Behavioral surface (Stage-2 S2.2)
        .action_body => "action_body",
        .pin_decl => "pin_decl",
        .succession_chain => "succession_chain",
        .control_ref => "control_ref",
        .decide_block => "decide_block",
        .par_block => "par_block",
        .loop_statement => "loop_statement",
        .send_action => "send_action",
        .accept_action => "accept_action",
        .assign_action => "assign_action",
        .perform_statement => "perform_statement",
        .item_flow_statement => "item_flow_statement",
        .binding_statement => "binding_statement",
        .escape_node => "escape_node",
        .escape_succession => "escape_succession",
        .entry_do_exit => "entry_do_exit",
        .transition_statement => "transition_statement",
        .target_ref => "target_ref",
        .system_block => "system_block",
        .subsystem_block => "subsystem_block",
        .component_instance => "component_instance",
        .comp_connect => "comp_connect",
        .expose_tag => "expose_tag",
        .traceability_block => "traceability_block",
        .allocate_tag => "allocate_tag",
        .satisfy_block => "satisfy_block",
        .validate_tag => "validate_tag",
        .object_literal => "object_literal",
    };
}

fn binaryOpName(op: ast.BinaryOp) []const u8 {
    return switch (op) {
        .add => "add",
        .sub => "sub",
        .mul => "mul",
        .div => "div",
        .eq => "eq",
        .neq => "neq",
        .lt => "lt",
        .le => "le",
        .gt => "gt",
        .ge => "ge",
        .log_and => "log_and",
        .log_or => "log_or",
    };
}

fn unaryOpName(op: ast.UnaryOp) []const u8 {
    return switch (op) {
        .neg => "neg",
        .not => "not",
        .bang => "bang",
    };
}

fn modifierName(m: ast.Modifier) []const u8 {
    return switch (m) {
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

fn directionName(d: ast.Direction) []const u8 {
    return switch (d) {
        .none => "none",
        .in => "in",
        .out => "out",
        .inout => "inout",
    };
}

fn visibilityName(v: ast.Visibility) []const u8 {
    return switch (v) {
        .public => "public",
        .protected => "protected",
        .private => "private",
    };
}

fn verifFieldKindName(k: ast.VerifFieldKind) []const u8 {
    return switch (k) {
        .accepts => "accepts",
        .rejects => "rejects",
        .threshold => "threshold",
        .operator => "operator",
        .conditions => "conditions",
    };
}

/// Emit the diagnostics JSON as a top-level array. Each diagnostic object
/// has alphabetical key order per D-18:
///   code, fix_it, message, notes, secondary_spans, severity, span
///
/// - severity emits as the lowercase tag name ("err" / "warn" / "info" / "hint")
/// - span emits as [start, end]
/// - secondary_spans emits as [{"label":"...","span":[s,e]}, ...] (alphabetical)
/// - fix_it emits as null or {"replace_span":[s,e],"replacement":"..."}
/// - notes always emits as a string (possibly empty)
///
/// Hand-rolled writer per RESEARCH §Pitfall 5.
pub fn emitDiagnostics(
    allocator: std.mem.Allocator,
    diags: []const diagnostics.Diagnostic,
) ![]const u8 {
    var buf: std.ArrayList(u8) = .empty;
    defer buf.deinit(allocator);

    try buf.append(allocator, '[');
    for (diags, 0..) |d, i| {
        if (i > 0) try buf.append(allocator, ',');
        try writeDiagnostic(allocator, &buf, d);
    }
    try buf.append(allocator, ']');

    return try buf.toOwnedSlice(allocator);
}

fn writeDiagnostic(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), d: diagnostics.Diagnostic) !void {
    try buf.append(allocator, '{');

    // code
    try buf.appendSlice(allocator, "\"code\":");
    try writeStr(allocator, buf, d.code);

    // fix_it
    try buf.appendSlice(allocator, ",\"fix_it\":");
    if (d.fix_it) |fi| {
        try buf.append(allocator, '{');
        try buf.appendSlice(allocator, "\"replace_span\":[");
        try appendU32(allocator, buf, fi.replace_span.start);
        try buf.append(allocator, ',');
        try appendU32(allocator, buf, fi.replace_span.end);
        try buf.appendSlice(allocator, "],\"replacement\":");
        try writeStr(allocator, buf, fi.replacement);
        try buf.append(allocator, '}');
    } else {
        try buf.appendSlice(allocator, "null");
    }

    // message
    try buf.appendSlice(allocator, ",\"message\":");
    try writeStr(allocator, buf, d.message);

    // notes
    try buf.appendSlice(allocator, ",\"notes\":");
    try writeStr(allocator, buf, d.notes);

    // secondary_spans
    try buf.appendSlice(allocator, ",\"secondary_spans\":[");
    for (d.secondary_spans, 0..) |sl, i| {
        if (i > 0) try buf.append(allocator, ',');
        try buf.append(allocator, '{');
        try buf.appendSlice(allocator, "\"label\":");
        try writeStr(allocator, buf, sl.label);
        try buf.appendSlice(allocator, ",\"span\":[");
        try appendU32(allocator, buf, sl.span.start);
        try buf.append(allocator, ',');
        try appendU32(allocator, buf, sl.span.end);
        try buf.appendSlice(allocator, "]}");
    }
    try buf.append(allocator, ']');

    // severity
    try buf.appendSlice(allocator, ",\"severity\":");
    try writeStr(allocator, buf, severityName(d.severity));

    // span
    try buf.appendSlice(allocator, ",\"span\":[");
    try appendU32(allocator, buf, d.span.start);
    try buf.append(allocator, ',');
    try appendU32(allocator, buf, d.span.end);
    try buf.append(allocator, ']');

    try buf.append(allocator, '}');
}

fn severityName(s: diagnostics.Severity) []const u8 {
    return switch (s) {
        .err => "err",
        .warn => "warn",
        .info => "info",
        .hint => "hint",
    };
}

/// Drive `lexer.next(mode)` over the whole source until EOF and emit a
/// deterministic JSON object describing every token. The outer mode is
/// picked from `file_mode`:
///   `.deal`  → start in `.deal_def`
///   `.dealx` → start in `.dealx_outer`
///
/// Plans 03/04 will exercise the inner modes (`.dealx_tag`,
/// `.dealx_expr_brace`) through parser snapshots; Plan 02's token
/// snapshots stay at the file's outer mode so the snapshot harness is a
/// pure lexer test that doesn't depend on parser state.
///
/// `text` is included for tokens whose semantic content is the raw source
/// slice — identifiers, literals, doc-comments, delimited operators,
/// annotations. Span-only tokens (punctuation, keywords) omit `text` to
/// keep snapshots compact.
///
/// Hand-rolled writer per RESEARCH §Pitfall 5 — std.json.Stringify is
/// not used at the top level to avoid issue #25233 reordering surprises.
pub fn emitTokens(
    allocator: std.mem.Allocator,
    source: []const u8,
    file_mode: ast.Mode,
) ![]const u8 {
    var buf: std.ArrayList(u8) = .empty;
    defer buf.deinit(allocator);

    const mode_str: []const u8 = switch (file_mode) {
        .deal => "deal",
        .dealx => "dealx",
    };
    const start_mode: lexer.Mode = switch (file_mode) {
        .deal => .deal_def,
        .dealx => .dealx_outer,
    };

    try buf.appendSlice(allocator, "{\"v\":1,\"mode\":\"");
    try buf.appendSlice(allocator, mode_str);
    try buf.appendSlice(allocator, "\",\"tokens\":[");

    var lex = lexer.Lexer.init(source);
    var first: bool = true;
    while (true) {
        const tok = lex.next(start_mode);
        if (!first) {
            try buf.append(allocator, ',');
        }
        first = false;

        // {"k":"<tag>","span":[s,e]
        try buf.appendSlice(allocator, "{\"k\":\"");
        try buf.appendSlice(allocator, tagName(tok.tag));
        try buf.appendSlice(allocator, "\",\"span\":[");
        try appendU32(allocator, &buf, tok.span.start);
        try buf.append(allocator, ',');
        try appendU32(allocator, &buf, tok.span.end);
        try buf.append(allocator, ']');

        // Optional "text" field — only for tokens carrying semantic
        // source content. Always appears AFTER "span" so per-object
        // field order remains: k, span, text (alphabetical — D-18).
        if (tagCarriesText(tok.tag) and tok.span.end > tok.span.start) {
            try buf.appendSlice(allocator, ",\"text\":\"");
            try appendJsonStringEscaped(
                allocator,
                &buf,
                source[tok.span.start..tok.span.end],
            );
            try buf.append(allocator, '"');
        }

        try buf.append(allocator, '}');

        if (tok.tag == .eof) break;
    }

    try buf.appendSlice(allocator, "]}");
    return try buf.toOwnedSlice(allocator);
}

/// Map a Tag value to its snake_case string form. Manual switch (not
/// `@tagName`) so renames don't silently break committed snapshots.
fn tagName(t: lexer.Tag) []const u8 {
    return switch (t) {
        .eof => "eof",
        .unknown => "unknown",
        .ident => "ident",
        .unrestricted_name => "unrestricted_name",

        // Keywords — same spellings as the enum variant. Using @tagName
        // here is safe because the variant names are stable per D-05.
        .kw_part => "kw_part",
        .kw_port => "kw_port",
        .kw_action => "kw_action",
        .kw_state => "kw_state",
        .kw_attribute => "kw_attribute",
        .kw_item => "kw_item",
        .kw_interface => "kw_interface",
        .kw_connection => "kw_connection",
        .kw_flow => "kw_flow",
        .kw_allocation => "kw_allocation",
        .kw_requirement => "kw_requirement",
        .kw_constraint => "kw_constraint",
        .kw_calc => "kw_calc",
        .kw_need => "kw_need",
        .kw_def => "kw_def",
        .kw_use => "kw_use",
        .kw_case => "kw_case",
        .kw_abstract => "kw_abstract",
        .kw_derived => "kw_derived",
        .kw_readonly => "kw_readonly",
        .kw_ordered => "kw_ordered",
        .kw_nonunique => "kw_nonunique",
        .kw_individual => "kw_individual",
        .kw_variation => "kw_variation",
        .kw_portion => "kw_portion",
        .kw_end => "kw_end",
        .kw_ref => "kw_ref",
        .kw_in => "kw_in",
        .kw_out => "kw_out",
        .kw_inout => "kw_inout",
        .kw_public => "kw_public",
        .kw_protected => "kw_protected",
        .kw_private => "kw_private",
        .kw_package => "kw_package",
        .kw_import => "kw_import",
        .kw_export => "kw_export",
        .kw_as => "kw_as",
        .kw_actor => "kw_actor",
        .kw_subject => "kw_subject",
        .kw_precondition => "kw_precondition",
        .kw_postcondition => "kw_postcondition",
        .kw_verification => "kw_verification",
        .kw_accepts => "kw_accepts",
        .kw_rejects => "kw_rejects",
        .kw_threshold => "kw_threshold",
        .kw_operator => "kw_operator",
        .kw_conditions => "kw_conditions",
        .kw_system => "kw_system",
        .kw_subsystem => "kw_subsystem",
        .kw_connect => "kw_connect",
        .kw_expose => "kw_expose",
        .kw_traceability => "kw_traceability",
        .kw_satisfy => "kw_satisfy",
        .kw_validate => "kw_validate",
        .kw_allocate => "kw_allocate",
        .kw_from => "kw_from",
        .kw_to => "kw_to",
        .kw_via => "kw_via",
        .kw_carrying => "kw_carrying",
        .kw_method => "kw_method",
        .kw_status => "kw_status",
        .kw_relationship => "kw_relationship",
        .kw_criteria => "kw_criteria",
        .kw_evidence => "kw_evidence",
        .kw_compute => "kw_compute",
        .kw_maps => "kw_maps",
        .kw_gap => "kw_gap",
        .kw_simulation => "kw_simulation",
        .kw_test => "kw_test",
        .kw_analysis => "kw_analysis",
        .kw_design => "kw_design",
        .kw_inspection => "kw_inspection",
        .kw_demonstration => "kw_demonstration",
        .kw_AND => "kw_AND",
        .kw_OR => "kw_OR",
        .kw_NOT => "kw_NOT",
        .kw_WHERE => "kw_WHERE",
        .kw_path => "kw_path",
        .kw_schema => "kw_schema",
        .kw_created => "kw_created",
        .kw_modified => "kw_modified",
        .kw_reviewed => "kw_reviewed",
        .kw_hash => "kw_hash",
        .kw_baseline => "kw_baseline",
        .kw_marking => "kw_marking",
        .kw_by => "kw_by",
        .kw_return => "kw_return",
        .kw_require => "kw_require",

        // Behavioral surface keywords (BH-1..BH-7, Stage-2 S2.1)
        .kw_decide => "kw_decide",
        .kw_par => "kw_par",
        .kw_loop => "kw_loop",
        .kw_while => "kw_while",
        .kw_until => "kw_until",
        .kw_for => "kw_for",
        .kw_send => "kw_send",
        .kw_accept => "kw_accept",
        .kw_assign => "kw_assign",
        .kw_bind => "kw_bind",
        .kw_node => "kw_node",
        .kw_succession => "kw_succession",
        .kw_on => "kw_on",
        .kw_entry => "kw_entry",
        .kw_do => "kw_do",
        .kw_exit => "kw_exit",
        .kw_else => "kw_else",
        .kw_start => "kw_start",
        .kw_done => "kw_done",
        .kw_terminate => "kw_terminate",

        .int_literal => "int_literal",
        .real_literal => "real_literal",
        .string_literal => "string_literal",
        .boolean_literal => "boolean_literal",
        .template_head => "template_head",
        .template_middle => "template_middle",
        .template_tail => "template_tail",
        .template_literal => "template_literal",

        .delimited_operator => "delimited_operator",
        .annotation_prefix => "annotation_prefix",
        .annotation => "annotation",
        .sysml_specializes => "sysml_specializes",
        .sysml_redefines => "sysml_redefines",
        .sysml_references => "sysml_references",
        .sysml_conjugates => "sysml_conjugates",
        .item_flow => "item_flow",

        .l_brace => "l_brace",
        .r_brace => "r_brace",
        .l_paren => "l_paren",
        .r_paren => "r_paren",
        .l_bracket => "l_bracket",
        .r_bracket => "r_bracket",
        .semicolon => "semicolon",
        .comma => "comma",
        .dot => "dot",
        .dotdot => "dotdot",
        .coloncolon => "coloncolon",
        .colon => "colon",
        .colon_eq => "colon_eq",
        .eq => "eq",
        .arrow => "arrow",
        .thin_arrow => "thin_arrow",
        .gt => "gt",
        .lt => "lt",
        .gt_eq => "gt_eq",
        .lt_eq => "lt_eq",
        .eq_eq => "eq_eq",
        .bang_eq => "bang_eq",
        .plus => "plus",
        .plus_minus => "plus_minus",
        .minus => "minus",
        .star => "star",
        .slash => "slash",

        .tag_open => "tag_open",
        .tag_close_open => "tag_close_open",
        .tag_close => "tag_close",
        .tag_self_close => "tag_self_close",

        // D-28 comment tokens (all three emitted as tokens, not skipped)
        .comment_line => "comment_line",
        .comment_block => "comment_block",
        .doc_comment => "doc_comment",
    };
}

/// Should this token's text be included in the snapshot? Identifiers and
/// literals carry source-derived semantic content; punctuation and
/// keywords do not (the tag name fully determines the source slice).
/// Including text on every token would bloat snapshots ~3x with no
/// information gain.
fn tagCarriesText(t: lexer.Tag) bool {
    return switch (t) {
        .ident,
        .unrestricted_name,
        .int_literal,
        .real_literal,
        .string_literal,
        .boolean_literal,
        .template_head,
        .template_middle,
        .template_tail,
        .template_literal,
        .delimited_operator,
        .annotation_prefix,
        .annotation,
        .comment_line,
        .comment_block,
        .doc_comment,
        .unknown,
        => true,
        else => false,
    };
}

fn appendU32(
    allocator: std.mem.Allocator,
    buf: *std.ArrayList(u8),
    value: u32,
) !void {
    // std.fmt.allocPrint -> append -> free; cheaper to format directly
    // into a small stack buffer for the common case (most spans < 10
    // chars). u32 max = 4294967295 = 10 chars.
    var tmp: [10]u8 = undefined;
    const slice = std.fmt.bufPrint(&tmp, "{d}", .{value}) catch unreachable;
    try buf.appendSlice(allocator, slice);
}

/// JSON-escape per RFC 8259 §7. UTF-8 bytes ≥ 0x80 are passed through
/// Emit the `.deal/index.json` shape from a SymbolTable.
///
/// Top-level key order per D-18 (alphabetical):
///   deal_version, elements, imports_graph, v
///
/// `elements` entries are emitted in alphabetical key order per D-18 (WARNING-01).
/// `deal_version` is the DEAL compiler version string (passed in).
///
/// Hand-rolled writer per RESEARCH §Pitfall 1 (std.json.Stringify can reorder
/// fields — issue #25233). Do NOT use std.json.Stringify here.
pub fn writeIndexJson(
    allocator: std.mem.Allocator,
    table: *const sema.SymbolTable,
    deal_version: []const u8,
) ![]const u8 {
    var buf: std.ArrayList(u8) = .empty;
    defer buf.deinit(allocator);

    // Open top-level object. Key order: deal_version, elements, imports_graph, v
    try buf.appendSlice(allocator, "{\"deal_version\":");
    try writeStr(allocator, &buf, deal_version);

    // elements: collect all non-imported entries and sort keys alphabetically.
    try buf.appendSlice(allocator, ",\"elements\":{");
    {
        // Collect element IDs that are locally-declared (not imports).
        var elem_ids: std.ArrayList([]const u8) = .empty;
        defer elem_ids.deinit(allocator);
        var it = table.entries.iterator();
        while (it.next()) |entry| {
            const e = entry.value_ptr.*;
            if (e.kind == .imported or e.kind == .imported_wildcard_pkg) continue;
            // Only add if key equals the qualified id (skip simple-name aliases).
            if (!std.mem.eql(u8, entry.key_ptr.*, e.id)) continue;
            try elem_ids.append(allocator, entry.key_ptr.*);
        }
        // Sort alphabetically (D-18 + WARNING-01 mitigation).
        std.mem.sort([]const u8, elem_ids.items, {}, struct {
            fn lt(_: void, a_s: []const u8, b_s: []const u8) bool {
                return std.mem.lessThan(u8, a_s, b_s);
            }
        }.lt);
        // Emit each element entry.
        for (elem_ids.items, 0..) |id, i| {
            if (i > 0) try buf.append(allocator, ',');
            // Key is the element id (quoted, escaped).
            try buf.append(allocator, '"');
            try appendJsonStringEscaped(allocator, &buf, id);
            try buf.appendSlice(allocator, "\":{");
            // Per-element fields (alphabetical): id, kind, source_file, span
            const entry = table.entries.get(id).?;
            try buf.appendSlice(allocator, "\"id\":");
            try writeStr(allocator, &buf, entry.id);
            try buf.appendSlice(allocator, ",\"kind\":\"");
            try buf.appendSlice(allocator, symbolKindName(entry.kind));
            try buf.appendSlice(allocator, "\",\"source_file\":");
            try writeStr(allocator, &buf, entry.source_file);
            try buf.appendSlice(allocator, ",\"span\":[");
            try appendU32(allocator, &buf, entry.span.start);
            try buf.append(allocator, ',');
            try appendU32(allocator, &buf, entry.span.end);
            try buf.appendSlice(allocator, "]}");
        }
    }
    try buf.append(allocator, '}');

    // imports_graph array.
    try buf.appendSlice(allocator, ",\"imports_graph\":[");
    for (table.imports_graph, 0..) |edge, i| {
        if (i > 0) try buf.append(allocator, ',');
        // Fields (alphabetical): from_file, import_path, is_wildcard, items
        try buf.appendSlice(allocator, "{\"from_file\":");
        try writeStr(allocator, &buf, edge.from_file);
        try buf.appendSlice(allocator, ",\"import_path\":");
        try writeStr(allocator, &buf, edge.import_path);
        try buf.appendSlice(allocator, ",\"is_wildcard\":");
        try buf.appendSlice(allocator, if (edge.is_wildcard) "true" else "false");
        try buf.appendSlice(allocator, ",\"items\":[");
        for (edge.items, 0..) |item, j| {
            if (j > 0) try buf.append(allocator, ',');
            try writeStr(allocator, &buf, item);
        }
        try buf.appendSlice(allocator, "]}");
    }
    try buf.append(allocator, ']');

    // v: schema version.
    try buf.appendSlice(allocator, ",\"v\":1}");

    return try buf.toOwnedSlice(allocator);
}

fn symbolKindName(kind: sema.SymbolKind) []const u8 {
    return switch (kind) {
        .package => "package",
        .part_def => "part_def",
        .actor_def => "actor_def",
        .port_def => "port_def",
        .action_def => "action_def",
        .state_def => "state_def",
        .attribute_def => "attribute_def",
        // Check #7 dimensional kinds — serialized as "attribute_def" for index compatibility.
        .dimension_def => "dimension_def",
        .unit_def => "unit_def",
        .item_def => "item_def",
        .interface_def => "interface_def",
        .connection_def => "connection_def",
        .flow_def => "flow_def",
        .allocation_def => "allocation_def",
        .requirement_def => "requirement_def",
        .constraint_def => "constraint_def",
        .calc_def => "calc_def",
        .need_def => "need_def",
        .use_case_def => "use_case_def",
        .imported => "imported",
        .imported_wildcard_pkg => "imported_wildcard_pkg",
    };
}

/// raw (the spec allows raw UTF-8 — only the seven required escapes plus
/// control bytes get \-escaped).
fn appendJsonStringEscaped(
    allocator: std.mem.Allocator,
    buf: *std.ArrayList(u8),
    s: []const u8,
) !void {
    for (s) |c| {
        switch (c) {
            '"' => try buf.appendSlice(allocator, "\\\""),
            '\\' => try buf.appendSlice(allocator, "\\\\"),
            '\n' => try buf.appendSlice(allocator, "\\n"),
            '\r' => try buf.appendSlice(allocator, "\\r"),
            '\t' => try buf.appendSlice(allocator, "\\t"),
            0x08 => try buf.appendSlice(allocator, "\\b"),
            0x0C => try buf.appendSlice(allocator, "\\f"),
            else => {
                if (c < 0x20) {
                    // Other control bytes → \u00XX.
                    var tmp: [6]u8 = undefined;
                    const hex = std.fmt.bufPrint(
                        &tmp,
                        "\\u{x:0>4}",
                        .{c},
                    ) catch unreachable;
                    try buf.appendSlice(allocator, hex);
                } else {
                    try buf.append(allocator, c);
                }
            },
        }
    }
}

// ─── IR v0 JSON emitter (D-22, Plan 02-03) ────────────────────────────────

/// Emit an IR Document as JSON per the D-22 envelope shape.
///
/// Top-level key order (alphabetical per D-18): edges, elements, ir_version, v.
/// Per-edge keys (alphabetical): dst, kind, src.
/// Per-element keys (alphabetical): kind, payload, source_file, span.
/// Per-payload keys (alphabetical): see emitIrPayload.
///
/// Hand-rolled writer per RESEARCH §Pitfall 1.
/// D-25: NO comment fields emitted at any level.
/// T-02-15: Determinism guaranteed by sorted element keys + sorted edge list.
pub fn emitIrJson(
    allocator: std.mem.Allocator,
    doc: *const ir.Document,
) ![]const u8 {
    var buf: std.ArrayList(u8) = .empty;
    defer buf.deinit(allocator);

    // Top-level open.
    // Key order: edges, elements, ir_version, v
    try buf.appendSlice(allocator, "{\"edges\":[");

    // Edges: sort by (src, dst, kind) for deterministic output (T-02-15).
    var edge_list = try allocator.alloc(*const ir.Edge, doc.edges.len);
    defer allocator.free(edge_list);
    for (doc.edges, 0..) |*edge, i| {
        edge_list[i] = edge;
    }
    std.mem.sort(*const ir.Edge, edge_list, {}, struct {
        fn lt(_: void, a: *const ir.Edge, b: *const ir.Edge) bool {
            const cmp_src = std.mem.order(u8, a.src, b.src);
            if (cmp_src != .eq) return cmp_src == .lt;
            const cmp_dst = std.mem.order(u8, a.dst, b.dst);
            if (cmp_dst != .eq) return cmp_dst == .lt;
            const a_kind = @intFromEnum(a.kind);
            const b_kind = @intFromEnum(b.kind);
            return a_kind < b_kind;
        }
    }.lt);

    for (edge_list, 0..) |edge, i| {
        if (i > 0) try buf.append(allocator, ',');
        try emitIrEdge(allocator, &buf, edge);
    }
    try buf.appendSlice(allocator, "],\"elements\":{");

    // Elements: collect IDs and sort alphabetically (D-18 + T-02-15 determinism).
    var elem_ids: std.ArrayList([]const u8) = .empty;
    defer elem_ids.deinit(allocator);
    var it = doc.elements.iterator();
    while (it.next()) |entry| {
        try elem_ids.append(allocator, entry.key_ptr.*);
    }
    std.mem.sort([]const u8, elem_ids.items, {}, struct {
        fn lt(_: void, a_s: []const u8, b_s: []const u8) bool {
            return std.mem.lessThan(u8, a_s, b_s);
        }
    }.lt);

    for (elem_ids.items, 0..) |id, i| {
        if (i > 0) try buf.append(allocator, ',');
        try buf.append(allocator, '"');
        try appendJsonStringEscaped(allocator, &buf, id);
        try buf.appendSlice(allocator, "\":");
        const node = doc.elements.get(id).?;
        try emitIrNode(allocator, &buf, node);
    }

    // IR v0.1 (S2.5): behavioral surface is additive over v0. The v0.1 schema
    // accepts both strings; the toolchain now emits "v0.1".
    try buf.appendSlice(allocator, "},\"ir_version\":\"v0.1\",\"v\":1}");

    return try buf.toOwnedSlice(allocator);
}

/// Emit a single IR Edge as JSON. Key order: dst, kind, src (alphabetical).
fn emitIrEdge(
    allocator: std.mem.Allocator,
    buf: *std.ArrayList(u8),
    edge: *const ir.Edge,
) !void {
    try buf.appendSlice(allocator, "{\"dst\":");
    try writeStr(allocator, buf, edge.dst);
    // Behavioral edge payload (optional, alphabetical: flow_type, guard before kind).
    if (edge.flow_type) |ft| {
        try buf.appendSlice(allocator, ",\"flow_type\":");
        try writeStr(allocator, buf, ft);
    }
    if (edge.guard) |g| {
        try buf.appendSlice(allocator, ",\"guard\":");
        try writeStr(allocator, buf, g);
    }
    try buf.appendSlice(allocator, ",\"kind\":\"");
    try buf.appendSlice(allocator, edgeKindName(edge.kind));
    try buf.appendSlice(allocator, "\",\"src\":");
    try writeStr(allocator, buf, edge.src);
    if (edge.subaction_kind) |sk| {
        try buf.appendSlice(allocator, ",\"subaction_kind\":");
        try writeStr(allocator, buf, sk);
    }
    try buf.append(allocator, '}');
}

/// Emit a single IR Node as JSON. Key order: kind, payload, source_file, span
/// (alphabetical per D-18).
fn emitIrNode(
    allocator: std.mem.Allocator,
    buf: *std.ArrayList(u8),
    node: *const ir.IrNode,
) !void {
    try buf.appendSlice(allocator, "{\"kind\":\"");
    try buf.appendSlice(allocator, irNodeKindName(node.kind));
    try buf.appendSlice(allocator, "\",\"payload\":");
    try emitIrPayload(allocator, buf, &node.payload);
    try buf.appendSlice(allocator, ",\"source_file\":");
    try writeStr(allocator, buf, node.source_file);
    try buf.appendSlice(allocator, ",\"span\":[");
    try appendU32(allocator, buf, node.span.start);
    try buf.append(allocator, ',');
    try appendU32(allocator, buf, node.span.end);
    try buf.appendSlice(allocator, "]}");
}

/// Emit the payload object for an IR node. Keys emitted alphabetically (D-18).
/// D-25: NO comment fields emitted.
///
/// Payload key order (all optional; emitted only if non-null/non-empty):
///   agent_metadata, direction, is_statement_valued (calc_def only),
///   modifiers, name, params (calc_def only), precision (calc_def only),
///   requirement_ref, return_type (calc_def only), simulation_bindings, type_ref
fn emitIrPayload(
    allocator: std.mem.Allocator,
    buf: *std.ArrayList(u8),
    payload: *const ir.IrPayload,
) !void {
    try buf.append(allocator, '{');
    var first = true;

    // agent_metadata (alphabetical: before direction)
    if (payload.agent_metadata) |meta| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"agent_metadata\":");
        try emitAgentMetadata(allocator, buf, &meta);
    }

    // callee_ref (behavioral; alphabetical: after agent_metadata)
    if (payload.callee_ref) |v| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"callee_ref\":");
        try writeStr(allocator, buf, v);
    }

    // direction (only if != none)
    if (payload.direction != .none) {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"direction\":\"");
        try buf.appendSlice(allocator, directionName(payload.direction));
        try buf.append(allocator, '"');
    }

    // effect_ref, endpoint, guard_expr, implicit (behavioral; alphabetical)
    if (payload.effect_ref) |v| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"effect_ref\":");
        try writeStr(allocator, buf, v);
    }
    if (payload.endpoint) |v| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"endpoint\":");
        try writeStr(allocator, buf, v);
    }
    if (payload.guard_expr) |v| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"guard_expr\":");
        try writeStr(allocator, buf, v);
    }
    if (payload.implicit) {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"implicit\":true");
    }

    // is_statement_valued (calc_def only; alphabetical: between direction and modifiers)
    if (payload.calc_def) |cdp| {
        if (cdp.is_statement_valued) {
            try emitComma(allocator, buf, &first);
            try buf.appendSlice(allocator, "\"is_statement_valued\":true");
        }
    }

    // iterable_expr, loop_kind, loop_var (behavioral; alphabetical)
    if (payload.iterable_expr) |v| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"iterable_expr\":");
        try writeStr(allocator, buf, v);
    }
    if (payload.loop_kind) |v| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"loop_kind\":");
        try writeStr(allocator, buf, v);
    }
    if (payload.loop_var) |v| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"loop_var\":");
        try writeStr(allocator, buf, v);
    }

    // modifiers
    if (payload.modifiers.len > 0) {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"modifiers\":[");
        for (payload.modifiers, 0..) |mod, i| {
            if (i > 0) try buf.append(allocator, ',');
            try buf.append(allocator, '"');
            try appendJsonStringEscaped(allocator, buf, mod);
            try buf.append(allocator, '"');
        }
        try buf.append(allocator, ']');
    }

    // name (always emitted)
    if (payload.name.len > 0) {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"name\":");
        try writeStr(allocator, buf, payload.name);
    }

    // params (calc_def only; alphabetical: after name, before precision)
    if (payload.calc_def) |cdp| {
        if (cdp.params.len > 0) {
            try emitComma(allocator, buf, &first);
            try buf.appendSlice(allocator, "\"params\":[");
            for (cdp.params, 0..) |param, i| {
                if (i > 0) try buf.append(allocator, ',');
                // Key order: direction, name, type_name (alphabetical)
                try buf.appendSlice(allocator, "{\"direction\":");
                try writeStr(allocator, buf, param.direction);
                try buf.appendSlice(allocator, ",\"name\":");
                try writeStr(allocator, buf, param.name);
                try buf.appendSlice(allocator, ",\"type_name\":");
                try writeStr(allocator, buf, param.type_name);
                try buf.append(allocator, '}');
            }
            try buf.append(allocator, ']');
        }
    }

    // payload_ref (behavioral; alphabetical: after params, before precision)
    if (payload.payload_ref) |v| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"payload_ref\":");
        try writeStr(allocator, buf, v);
    }

    // precision (calc_def only; alphabetical: after params, before requirement_ref)
    if (payload.calc_def) |cdp| {
        if (cdp.precision) |prec| {
            try emitComma(allocator, buf, &first);
            try buf.appendSlice(allocator, "\"precision\":{\"kind\":");
            try writeStr(allocator, buf, prec.kind);
            try buf.appendSlice(allocator, ",\"value\":");
            try writeStr(allocator, buf, prec.value);
            try buf.append(allocator, '}');
        }
    }

    // referent_ref (behavioral; alphabetical: after precision, before requirement_ref)
    if (payload.referent_ref) |v| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"referent_ref\":");
        try writeStr(allocator, buf, v);
    }

    // requirement_ref
    if (payload.requirement_ref) |req_ref| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"requirement_ref\":");
        try writeStr(allocator, buf, req_ref);
    }

    // return_type (calc_def only; alphabetical: after requirement_ref, before simulation_bindings)
    if (payload.calc_def) |cdp| {
        if (cdp.return_type) |rt| {
            try emitComma(allocator, buf, &first);
            try buf.appendSlice(allocator, "\"return_type\":");
            try writeStr(allocator, buf, rt);
        }
    }

    // simulation_bindings
    if (payload.simulation_bindings.len > 0) {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"simulation_bindings\":[");
        for (payload.simulation_bindings, 0..) |sim, i| {
            if (i > 0) try buf.append(allocator, ',');
            try emitSimBinding(allocator, buf, &sim);
        }
        try buf.append(allocator, ']');
    }

    // source_ref, target_ref, trigger_ref (behavioral; alphabetical, before type_ref)
    if (payload.source_ref) |v| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"source_ref\":");
        try writeStr(allocator, buf, v);
    }
    if (payload.target_ref) |v| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"target_ref\":");
        try writeStr(allocator, buf, v);
    }
    if (payload.trigger_ref) |v| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"trigger_ref\":");
        try writeStr(allocator, buf, v);
    }

    // type_ref
    if (payload.type_ref) |type_ref| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"type_ref\":");
        try writeStr(allocator, buf, type_ref);
    }

    // value_expr (behavioral; alphabetical: last, after type_ref)
    if (payload.value_expr) |v| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"value_expr\":");
        try writeStr(allocator, buf, v);
    }

    try buf.append(allocator, '}');
}

/// Emit an AgentMetadata object. Key order: assumes, concerns, confidence,
/// rationale (alphabetical per D-18). Omit null/empty fields (D-18).
fn emitAgentMetadata(
    allocator: std.mem.Allocator,
    buf: *std.ArrayList(u8),
    meta: *const ir.AgentMetadata,
) !void {
    try buf.append(allocator, '{');
    var first = true;

    if (meta.assumes.len > 0) {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"assumes\":[");
        for (meta.assumes, 0..) |s, i| {
            if (i > 0) try buf.append(allocator, ',');
            try writeStr(allocator, buf, s);
        }
        try buf.append(allocator, ']');
    }

    if (meta.concerns.len > 0) {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"concerns\":[");
        for (meta.concerns, 0..) |s, i| {
            if (i > 0) try buf.append(allocator, ',');
            try writeStr(allocator, buf, s);
        }
        try buf.append(allocator, ']');
    }

    if (meta.confidence) |conf| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"confidence\":");
        // Emit float: use Zig's float formatting. Max ~20 chars.
        var tmp: [32]u8 = undefined;
        const s = std.fmt.bufPrint(&tmp, "{d}", .{conf}) catch unreachable;
        try buf.appendSlice(allocator, s);
    }

    if (meta.rationale) |rat| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"rationale\":");
        try writeStr(allocator, buf, rat);
    }

    try buf.append(allocator, '}');
}

/// Emit a SimBinding object. Key order: entry, equation, fidelity, operator,
/// target, tool (alphabetical per D-18). Omit null fields.
fn emitSimBinding(
    allocator: std.mem.Allocator,
    buf: *std.ArrayList(u8),
    sim: *const ir.SimBinding,
) !void {
    try buf.append(allocator, '{');
    var first = true;

    if (sim.entry) |entry| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"entry\":");
        try writeStr(allocator, buf, entry);
    }

    if (sim.equation) |eq| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"equation\":");
        try writeStr(allocator, buf, eq);
    }

    if (sim.fidelity) |fid| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"fidelity\":");
        try writeStr(allocator, buf, fid);
    }

    try emitComma(allocator, buf, &first);
    try buf.appendSlice(allocator, "\"operator\":\"");
    try buf.appendSlice(allocator, switch (sim.operator) {
        .computes => "computes",
        .validates_against => "validates_against",
    });
    try buf.append(allocator, '"');

    try emitComma(allocator, buf, &first);
    try buf.appendSlice(allocator, "\"target\":");
    try writeStr(allocator, buf, sim.target);

    if (sim.tool) |tool| {
        try emitComma(allocator, buf, &first);
        try buf.appendSlice(allocator, "\"tool\":");
        try writeStr(allocator, buf, tool);
    }

    try buf.append(allocator, '}');
}

fn emitComma(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), first: *bool) !void {
    if (!first.*) {
        try buf.append(allocator, ',');
    }
    first.* = false;
}

fn edgeKindName(kind: ir.EdgeKind) []const u8 {
    return switch (kind) {
        .allocated_to => "allocated_to",
        .carries => "carries",
        .connects_via => "connects_via",
        .contains => "contains",
        .derives_from => "derives_from",
        .imports => "imports",
        .redefines => "redefines",
        .satisfies => "satisfies",
        .specializes => "specializes",
        .subsets => "subsets",
        .traces => "traces",
        // Behavioral surface (IR v0.1, S2.5)
        .succession => "succession",
        .item_flow => "item_flow",
        .binding => "binding",
        .subaction => "subaction",
    };
}

fn irNodeKindName(kind: ir.NodeKind) []const u8 {
    return switch (kind) {
        .action_def => "action_def",
        .actor_def => "actor_def",
        .allocation_def => "allocation_def",
        .allocate => "allocate",
        .annotation => "annotation",
        .attribute_def => "attribute_def",
        .attribute_usage => "attribute_usage",
        .calc_def => "calc_def",
        .connect => "connect",
        .connection_def => "connection_def",
        .constraint_def => "constraint_def",
        .expose => "expose",
        .flow_def => "flow_def",
        .interface_def => "interface_def",
        .item_def => "item_def",
        .need_def => "need_def",
        .package => "package",
        .part_def => "part_def",
        .part_usage => "part_usage",
        .port_def => "port_def",
        .port_usage => "port_usage",
        .requirement_def => "requirement_def",
        .satisfy => "satisfy",
        .state_def => "state_def",
        .subsystem => "subsystem",
        .system => "system",
        .traceability_block => "traceability_block",
        .use_case_def => "use_case_def",
        .validate => "validate",
        // Behavioral surface (IR v0.1, S2.5)
        .action_usage => "action_usage",
        .terminate_action => "terminate_action",
        .send_action => "send_action",
        .accept_action => "accept_action",
        .assign_action => "assign_action",
        .perform_action => "perform_action",
        .while_loop_action => "while_loop_action",
        .for_loop_action => "for_loop_action",
        .decision_node => "decision_node",
        .merge_node => "merge_node",
        .fork_node => "fork_node",
        .join_node => "join_node",
        .control_node => "control_node",
        .state_usage => "state_usage",
        .transition => "transition",
        .pin => "pin",
    };
}


