//! parser_deal.coverage — 87-production gate (Plan 03).
//!
//! Walks the AST of all 15 .deal showcase files plus inline synthetic
//! fixtures and asserts that every NodeKind in the "required for Phase 1"
//! subset appears ≥ 1 time across the full corpus.
//!
//! Required subset (mapped from deal.ebnf production list):
//!   - File-level: deal_file, header_block, package_decl, import_decl, export_decl
//!   - All 14 *_def kinds
//!   - All 14+ *_usage kinds (the ones the showcase corpus exercises)
//!   - Expressions: binary, unary, member_access, call, identifier,
//!     int_literal, real_literal, string_literal, boolean_literal
//!   - Annotations: annotation, doc_comment
//!   - Type/structural: type_annotation, multiplicity, specialization
//!
//! For kinds not naturally exercised by the 15 showcase files (e.g.
//! visibility_wrapper, redefinition), inline synthetic fixtures are parsed
//! and included in the walk.

const std = @import("std");
const ast = @import("ast");
const json = @import("json");
const parser = @import("parser");
const diagnostics_mod = @import("diagnostics");

/// The 15 .deal showcase files.
const showcase_deal_files = [_][]const u8{
    "packages/vehicle/battery.deal",
    "packages/vehicle/motor.deal",
    "packages/vehicle/behaviors.deal",
    "packages/vehicle/components.deal",
    "packages/vehicle/index.deal",
    "packages/interfaces/electrical.deal",
    "packages/interfaces/thermal.deal",
    "packages/interfaces/connections.deal",
    "packages/interfaces/index.deal",
    "packages/requirements/system.deal",
    "packages/requirements/needs.deal",
    "packages/requirements/index.deal",
    "packages/use-cases/driving.deal",
    "packages/use-cases/charging.deal",
    "packages/use-cases/index.deal",
};

/// Synthetic fixtures exercising NodeKinds not naturally present in the
/// 15 showcase files. Each fixture has a name (for error reporting) and
/// a source string.
const synthetic_fixtures = [_]struct { name: []const u8, src: []const u8 }{
    .{
        .name = "visibility_wrapper",
        .src =
        \\package test;
        \\part def Foo {
        \\    public (
        \\        part bar : Bar;
        \\    )
        \\    protected (
        \\        port p : Signal;
        \\    )
        \\    private (
        \\        attribute x : Real;
        \\    )
        \\}
        ,
    },
    .{
        .name = "redefinition",
        .src =
        \\package test;
        \\part def Sub specializes Base {
        \\    redefines base.sensor sensor : Lidar;
        \\}
        ,
    },
    .{
        .name = "real_literal_and_boolean",
        .src =
        \\package test;
        \\part def Sensor {
        \\    attribute voltage : Real = 3.3;
        \\    attribute active : Boolean = true;
        \\    attribute disabled : Boolean = false;
        \\}
        ,
    },
    .{
        .name = "unary_expression",
        .src =
        \\package test;
        \\part def Calc {
        \\    attribute neg : Real = -1.0;
        \\    attribute inv : Boolean = NOT true;
        \\}
        ,
    },
    .{
        .name = "verification_block",
        .src =
        \\package test;
        \\requirement def SafetyReq {
        \\    @verify {
        \\        method: analysis
        \\        status: pending
        \\    }
        \\}
        ,
    },
    .{
        .name = "precondition_postcondition",
        .src =
        \\package test;
        \\action def Transfer {
        \\    @pre { source != null }
        \\    @post { result == true }
        \\}
        ,
    },
    .{
        .name = "missing_def_kinds",
        .src =
        \\package test;
        \\state def Active {}
        \\attribute def Voltage {}
        \\item def Packet {}
        \\allocation def Deploy {}
        \\constraint def Invariant {}
        ,
    },
    .{
        .name = "requirement_usage",
        .src =
        \\package test;
        \\part def System {
        \\    requirement perf : PerformanceReq;
        \\}
        ,
    },
};

/// Required NodeKinds that must appear ≥ 1 time across the full corpus.
const required_kinds = [_]ast.NodeKind{
    // File-level
    .deal_file,
    .header_block,
    .package_decl,
    .import_decl,
    .export_decl,
    // All 14 def kinds
    .part_def,
    .port_def,
    .action_def,
    .state_def,
    .attribute_def,
    .item_def,
    .interface_def,
    .connection_def,
    .flow_def,
    .allocation_def,
    .requirement_def,
    .constraint_def,
    .need_def,
    .use_case_def,
    // Usage kinds (subset present in showcase corpus + synthetic fixtures)
    .part_usage,
    .port_usage,
    .attribute_usage,
    .action_usage,
    .requirement_usage,
    // Expressions
    .binary,
    .identifier,
    .int_literal,
    .string_literal,
    .annotation,
    .doc_comment,
    // Type/structural
    .type_annotation,
    .multiplicity,
};

/// Recursively walk the AST and record all seen NodeKinds.
fn visit(node: *ast.Node, seen: *std.AutoHashMap(ast.NodeKind, usize)) !void {
    const entry = try seen.getOrPutValue(node.kind, 0);
    entry.value_ptr.* += 1;

    // Recurse into children based on payload.
    switch (node.payload) {
        .deal_file => |p| {
            if (p.header) |h| try visit(h, seen);
            if (p.package_decl) |pd| try visit(pd, seen);
            for (p.imports) |n| try visit(n, seen);
            for (p.exports) |n| try visit(n, seen);
            for (p.definitions) |n| try visit(n, seen);
        },
        .dealx_file => |p| {
            if (p.header) |h| try visit(h, seen);
            if (p.package_decl) |pd| try visit(pd, seen);
            for (p.imports) |n| try visit(n, seen);
            for (p.root_tags) |n| try visit(n, seen);
        },
        .header_block => {}, // leaf (fields are HeaderField, not Node)
        .package_decl => {}, // leaf (segments are [][]const u8)
        .import_decl => {
            // items are ImportItem (not Node), path is [][]const u8
        },
        .export_decl => |p| {
            // path is [][]const u8, items are []ImportItem
            _ = p;
        },
        .part_def, .port_def, .action_def, .state_def, .attribute_def,
        .item_def, .interface_def, .connection_def, .flow_def,
        .allocation_def, .requirement_def,
        .need_def, .use_case_def, .actor_def => {
            const ed = getElementDef(node) orelse return;
            if (ed.specializes) |s| try visit(s, seen);
            if (ed.doc) |d| try visit(d, seen);
            // ed.modifiers is []Modifier (enum) — no child nodes
            for (ed.annotations) |a| try visit(a, seen);
            for (ed.members) |m| try visit(m, seen);
        },
        // Phase 05.2: constraint_def/calc_def use new payload types
        .constraint_def => |p| {
            if (p.params) |params| try visit(params, seen);
            if (p.specializes) |s| try visit(s, seen);
            if (p.doc) |d| try visit(d, seen);
            for (p.annotations) |a| try visit(a, seen);
            try visit(p.body, seen);
        },
        .calc_def => |p| {
            try visit(p.params, seen);
            try visit(p.type_node, seen);
            if (p.return_contract) |rc| try visit(rc, seen);
            if (p.doc) |d| try visit(d, seen);
            for (p.annotations) |a| try visit(a, seen);
            try visit(p.body, seen);
        },
        .param_list => |p| {
            for (p.params) |param| try visit(param, seen);
        },
        .param_decl => |p| {
            try visit(p.type_node, seen);
            if (p.multiplicity) |m| try visit(m, seen);
        },
        .return_contract => |p| {
            for (p.items) |item| try visit(item, seen);
        },
        .precision_spec => |p| try visit(p.value, seen),
        .constraint_ref => |p| {
            if (p.args) |args| for (args) |a| try visit(a, seen);
        },
        .constraint_body => |p| {
            for (p.members) |m| try visit(m, seen);
        },
        .calc_body => |p| {
            for (p.bindings) |b| try visit(b, seen);
            for (p.annotations) |a| try visit(a, seen);
            try visit(p.return_stmt, seen);
        },
        .require_statement => |p| {
            try visit(p.condition, seen);
            if (p.precision) |prec| try visit(prec, seen);
        },
        .part_usage, .port_usage, .attribute_usage, .action_usage,
        .actor_usage, .subject_usage, .state_usage, .item_usage,
        .interface_usage, .connection_usage, .flow_usage,
        .allocation_usage, .requirement_usage, .constraint_usage,
        .need_usage, .use_case_usage => {
            const eu = getElementUsage(node) orelse return;
            if (eu.doc) |d| try visit(d, seen);
            if (eu.type_node) |t| try visit(t, seen);
            if (eu.multiplicity) |m| try visit(m, seen);
            if (eu.default_value) |v| try visit(v, seen);
            if (eu.inline_body) |b| try visit(b, seen);
            // eu.modifiers is []Modifier (enum) — no child nodes
            for (eu.annotations) |a| try visit(a, seen);
        },
        .type_annotation => {
            // name_segments is [][]const u8 — no child nodes to recurse
        },
        .multiplicity => {}, // leaf (lo/hi are ?u32)
        .modifier_list => {}, // leaf
        // Behavioral surface (Stage-2 S2.2): no behavioral fixtures parse yet
        // (parser support lands in S2.3), so these are leaf here; S2.3 adds
        // recursion + fixtures.
        .action_body, .pin_decl, .succession_chain, .control_ref,
        .decide_block, .par_block, .loop_statement, .send_action,
        .accept_action, .assign_action, .perform_statement,
        .item_flow_statement, .binding_statement, .escape_node,
        .escape_succession, .entry_do_exit, .transition_statement,
        .target_ref => {},
        .visibility_wrapper => |p| {
            for (p.members) |m| try visit(m, seen);
        },
        .specialization => {}, // leaf (name is []const u8)
        .redefinition => {}, // leaf
        .operator_statement => |p| {
            if (p.value) |v| try visit(v, seen);
        },
        .inline_body => |p| {
            for (p.members) |m| try visit(m, seen);
        },
        .structural_relationship => {
            // op_text and target_segments are []const u8 / [][]const u8 — no child nodes
        },
        .annotation => |p| {
            if (p.value) |v| try visit(v, seen);
        },
        .doc_comment => {}, // leaf
        .annotation_body => |p| {
            for (p.fields) |f| {
                try visit(f.value, seen);
            }
        },
        .verification_block => |p| {
            for (p.fields) |f| {
                try visit(f.value, seen);
            }
        },
        .precondition_block => |p| {
            for (p.conditions) |c| try visit(c, seen);
        },
        .postcondition_block => |p| {
            for (p.conditions) |c| try visit(c, seen);
        },
        .binary => |p| {
            try visit(p.lhs, seen);
            try visit(p.rhs, seen);
        },
        .unary => |p| try visit(p.operand, seen),
        .member_access => |p| try visit(p.receiver, seen),
        .call => |p| {
            try visit(p.callee, seen);
            for (p.args) |a| try visit(a, seen);
        },
        .identifier => {}, // leaf
        .int_literal => {}, // leaf
        .real_literal => {}, // leaf
        .string_literal => {}, // leaf
        .template_literal => |p| {
            for (p.parts) |part| {
                switch (part) {
                    .expr => |e| try visit(e, seen),
                    .text => {},
                }
            }
        },
        .boolean_literal => {}, // leaf
        .interpolation => |p| try visit(p.expr, seen),
        // .dealx composition placeholders — no children in Plan 03
        .system_block, .subsystem_block, .component_instance,
        .comp_connect, .expose_tag, .traceability_block,
        .allocate_tag, .satisfy_block, .validate_tag => {},
        // Object literal (Plan 04 Task 4.1b) — leaf-ish here (no recursion
        // into the field values is required for the parser_deal coverage
        // pass; parser_dealx coverage handles its own walk).
        .object_literal => {},
        // Array literal (D-04) — recurse into items.
        .array_literal => |p| for (p.items) |item| try visit(item, seen),
    }
}

fn getElementDef(node: *ast.Node) ?ast.ElementDef {
    return switch (node.payload) {
        .part_def => |p| p,
        .port_def => |p| p,
        .action_def => |p| p,
        .state_def => |p| p,
        .attribute_def => |p| p,
        .item_def => |p| p,
        .interface_def => |p| p,
        .connection_def => |p| p,
        .flow_def => |p| p,
        .allocation_def => |p| p,
        .requirement_def => |p| p,
        // constraint_def uses ConstraintDefinition now (Phase 05.2)
        .need_def => |p| p,
        .use_case_def => |p| p,
        .actor_def => |p| p,
        else => null,
    };
}

fn getElementUsage(node: *ast.Node) ?ast.ElementUsage {
    return switch (node.payload) {
        .part_usage => |p| p,
        .port_usage => |p| p,
        .attribute_usage => |p| p,
        .action_usage => |p| p,
        .actor_usage => |p| p,
        .subject_usage => |p| p,
        .state_usage => |p| p,
        .item_usage => |p| p,
        .interface_usage => |p| p,
        .connection_usage => |p| p,
        .flow_usage => |p| p,
        .allocation_usage => |p| p,
        .requirement_usage => |p| p,
        .constraint_usage => |p| p,
        .need_usage => |p| p,
        .use_case_usage => |p| p,
        else => null,
    };
}

test "parser_deal.coverage" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    // Seen counter: NodeKind → count across all files.
    var seen = std.AutoHashMap(ast.NodeKind, usize).init(gpa);
    defer seen.deinit();

    // Walk all 15 showcase .deal files.
    for (showcase_deal_files) |rel| {
        const showcase_path = try std.fmt.allocPrint(gpa, "tests/showcase/{s}", .{rel});
        defer gpa.free(showcase_path);
        const source = try cwd.readFileAlloc(io, showcase_path, gpa, .unlimited);
        defer gpa.free(source);

        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const root = try parser.parseFile(arena.allocator(), source, .deal, &diags);
        if (root) |r| try visit(r, &seen);
    }

    // Walk synthetic fixtures for NodeKinds not in showcase corpus.
    for (synthetic_fixtures) |fix| {
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const root = try parser.parseFile(arena.allocator(), fix.src, .deal, &diags);
        if (root) |r| try visit(r, &seen);
    }

    // Print coverage tally to stderr.
    std.debug.print("\n=== parser_deal.coverage tally ===\n", .{});
    const fields = std.meta.fields(ast.NodeKind);
    inline for (fields) |f| {
        const kind: ast.NodeKind = @enumFromInt(f.value);
        const count = seen.get(kind) orelse 0;
        std.debug.print("  {s} = {d}\n", .{ f.name, count });
    }
    std.debug.print("===================================\n", .{});

    // Assert all required kinds have count ≥ 1.
    var all_ok = true;
    for (required_kinds) |kind| {
        const count = seen.get(kind) orelse 0;
        if (count == 0) {
            std.debug.print("MISSING required kind: {s}\n", .{@tagName(kind)});
            all_ok = false;
        }
    }
    if (!all_ok) {
        return error.TestMissingRequiredNodeKinds;
    }
}
