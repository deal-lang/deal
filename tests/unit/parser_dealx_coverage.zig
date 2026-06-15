//! parser_dealx.coverage — production-coverage gate for the 43
//! dealx.ebnf productions.
//!
//! Walks the AST of every .dealx showcase file (plus synthetic fixtures
//! for productions the showcase doesn't exercise) and tallies NodeKind
//! occurrences. Required kinds must all have count ≥ 1.
//!
//! Mapping (43 dealx.ebnf productions → NodeKind tally):
//!   File:        dealx_file
//!   Structural:  system_block, subsystem_block, component_instance,
//!                expose_tag, allocate_tag
//!   Connections: comp_connect
//!   Traceability: traceability_block, satisfy_block, validate_tag
//!   Inline:      object_literal, template_literal
//!   Expressions: identifier, int_literal, string_literal, member_access
//!                (covered when via={...} contains an expression chain)
//!
//! Synthetic fixtures are appended for productions the 4 showcase files
//! don't exercise (e.g. standalone allocate_tag inside a synthetic mini-
//! file when the showcase pattern wraps it inside traceability).

const std = @import("std");
const ast = @import("ast");
const parser = @import("parser");
const diagnostics_mod = @import("diagnostics");

const showcase_files = [_][]const u8{
    "tests/showcase/model/vehicle.dealx",
    "tests/showcase/model/traceability.dealx",
    "tests/showcase/model/variants/sedan.dealx",
    "tests/showcase/model/variants/performance.dealx",
};

const synthetic_fixtures = [_][]const u8{
    // Standalone allocate outside a traceability block — exercise allocate_tag.
    \\package syn;
    \\[<system Root>]
    \\  [<allocate from="A" to="B" relationship=<<derives>> />]
    \\[</system>]
    ,
    // Template literal in a connect attribute body — exercise template_literal.
    \\package syn2;
    \\[<connect from="a" to="b" via={X { label: `hello ${world}` }} />]
    ,
};

const RequiredKind = struct {
    kind: ast.NodeKind,
    name: []const u8,
};

const required = [_]RequiredKind{
    .{ .kind = .dealx_file, .name = "dealx_file" },
    .{ .kind = .system_block, .name = "system_block" },
    .{ .kind = .subsystem_block, .name = "subsystem_block" },
    .{ .kind = .component_instance, .name = "component_instance" },
    .{ .kind = .expose_tag, .name = "expose_tag" },
    .{ .kind = .allocate_tag, .name = "allocate_tag" },
    .{ .kind = .comp_connect, .name = "comp_connect" },
    .{ .kind = .traceability_block, .name = "traceability_block" },
    .{ .kind = .satisfy_block, .name = "satisfy_block" },
    .{ .kind = .validate_tag, .name = "validate_tag" },
    .{ .kind = .object_literal, .name = "object_literal" },
    .{ .kind = .identifier, .name = "identifier" },
    .{ .kind = .int_literal, .name = "int_literal" },
    .{ .kind = .string_literal, .name = "string_literal" },
    .{ .kind = .template_literal, .name = "template_literal" },
};

fn visit(node: *ast.Node, seen: *std.AutoHashMap(ast.NodeKind, usize)) std.mem.Allocator.Error!void {
    const entry = try seen.getOrPutValue(node.kind, 0);
    entry.value_ptr.* += 1;

    switch (node.payload) {
        .dealx_file => |p| {
            if (p.header) |h| try visit(h, seen);
            if (p.package_decl) |pd| try visit(pd, seen);
            for (p.imports) |n| try visit(n, seen);
            for (p.root_tags) |n| try visit(n, seen);
        },
        .system_block => |p| {
            for (p.children) |c| try visit(c, seen);
        },
        .subsystem_block => |p| {
            for (p.children) |c| try visit(c, seen);
        },
        .traceability_block => |p| {
            for (p.children) |c| try visit(c, seen);
        },
        .satisfy_block => |p| {
            for (p.children) |c| try visit(c, seen);
        },
        .validate_tag => |p| {
            for (p.attrs) |a| try visit(a, seen);
        },
        .component_instance => |p| {
            for (p.attrs) |a| try visit(a, seen);
        },
        .comp_connect => |p| {
            if (p.from_expr) |n| try visit(n, seen);
            if (p.to_expr) |n| try visit(n, seen);
            if (p.via_expr) |n| try visit(n, seen);
            if (p.carrying_expr) |n| try visit(n, seen);
            for (p.other_props) |n| try visit(n, seen);
        },
        .expose_tag => |p| {
            for (p.attrs) |a| try visit(a, seen);
        },
        .allocate_tag => |p| {
            for (p.attrs) |a| try visit(a, seen);
        },
        .object_literal => |p| {
            for (p.fields) |f| try visit(f.value, seen);
        },
        // Plan 03 .deal kinds appear in shared expression contexts.
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
        .template_literal => |p| {
            for (p.parts) |part| {
                switch (part) {
                    .expr => |e| try visit(e, seen),
                    .text => {},
                }
            }
        },
        .interpolation => |p| try visit(p.expr, seen),
        .annotation => |p| {
            if (p.value) |v| try visit(v, seen);
            if (p.body) |b| try visit(b, seen);
        },
        .annotation_body => |p| {
            for (p.fields) |f| try visit(f.value, seen);
        },
        // Leaf-ish — no child Node descents needed for coverage:
        .package_decl, .import_decl, .export_decl, .header_block,
        .type_annotation, .multiplicity, .modifier_list,
        .specialization, .redefinition, .operator_statement,
        .structural_relationship, .doc_comment,
        .identifier, .int_literal, .real_literal, .string_literal,
        .boolean_literal => {},
        // Behavioral surface (Stage-2 S2.2) — .deal-only; never appears in
        // .dealx fixtures. Leaf here (parser support + fixtures in S2.3).
        .action_body, .pin_decl, .succession_chain, .control_ref,
        .decide_block, .par_block, .loop_statement, .send_action,
        .accept_action, .assign_action, .perform_statement,
        .item_flow_statement, .binding_statement, .escape_node,
        .escape_succession, .entry_do_exit, .transition_statement,
        .target_ref => {},
        // Other kinds that aren't expected in .dealx but might appear
        // via the shared Pratt expression parser; descend if they have
        // child Nodes.
        .deal_file => |p| {
            if (p.header) |h| try visit(h, seen);
            if (p.package_decl) |pd| try visit(pd, seen);
            for (p.imports) |n| try visit(n, seen);
            for (p.exports) |n| try visit(n, seen);
            for (p.definitions) |n| try visit(n, seen);
        },
        .part_def, .port_def, .action_def, .state_def, .attribute_def,
        .item_def, .interface_def, .connection_def, .flow_def,
        .allocation_def, .requirement_def,
        .need_def, .use_case_def, .actor_def => {
            // ElementDef shape — descend into members for completeness.
            const ed: ast.ElementDef = switch (node.payload) {
                .part_def => |x| x,
                .port_def => |x| x,
                .action_def => |x| x,
                .state_def => |x| x,
                .attribute_def => |x| x,
                .item_def => |x| x,
                .interface_def => |x| x,
                .connection_def => |x| x,
                .flow_def => |x| x,
                .allocation_def => |x| x,
                .requirement_def => |x| x,
                .need_def => |x| x,
                .use_case_def => |x| x,
                .actor_def => |x| x,
                else => unreachable,
            };
            for (ed.annotations) |a| try visit(a, seen);
            for (ed.members) |m| try visit(m, seen);
        },
        // Phase 05.2: constraint_def/calc_def with new payload types
        .constraint_def => |p| {
            if (p.params) |params| try visit(params, seen);
            if (p.specializes) |s| try visit(s, seen);
            for (p.annotations) |a| try visit(a, seen);
            try visit(p.body, seen);
        },
        .calc_def => |p| {
            try visit(p.params, seen);
            try visit(p.type_node, seen);
            if (p.return_contract) |rc| try visit(rc, seen);
            for (p.annotations) |a| try visit(a, seen);
            try visit(p.body, seen);
        },
        .param_list => |p| for (p.params) |param| try visit(param, seen),
        .param_decl => |p| {
            try visit(p.type_node, seen);
            if (p.multiplicity) |m| try visit(m, seen);
        },
        .return_contract => |p| for (p.items) |item| try visit(item, seen),
        .precision_spec => |p| try visit(p.value, seen),
        .constraint_ref => |p| {
            if (p.args) |args| for (args) |a| try visit(a, seen);
        },
        .constraint_body => |p| for (p.members) |m| try visit(m, seen),
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
            const eu: ast.ElementUsage = switch (node.payload) {
                .part_usage => |x| x,
                .port_usage => |x| x,
                .attribute_usage => |x| x,
                .action_usage => |x| x,
                .actor_usage => |x| x,
                .subject_usage => |x| x,
                .state_usage => |x| x,
                .item_usage => |x| x,
                .interface_usage => |x| x,
                .connection_usage => |x| x,
                .flow_usage => |x| x,
                .allocation_usage => |x| x,
                .requirement_usage => |x| x,
                .constraint_usage => |x| x,
                .need_usage => |x| x,
                .use_case_usage => |x| x,
                else => unreachable,
            };
            if (eu.type_node) |t| try visit(t, seen);
            if (eu.multiplicity) |m| try visit(m, seen);
            if (eu.default_value) |v| try visit(v, seen);
            if (eu.inline_body) |b| try visit(b, seen);
        },
        .visibility_wrapper => |p| {
            for (p.members) |m| try visit(m, seen);
        },
        .inline_body => |p| {
            for (p.members) |m| try visit(m, seen);
        },
        .verification_block => |p| {
            for (p.fields) |f| try visit(f.value, seen);
        },
        .precondition_block => |p| for (p.conditions) |c| try visit(c, seen),
        .postcondition_block => |p| for (p.conditions) |c| try visit(c, seen),
        // Array literal (D-04) — recurse into items.
        .array_literal => |p| for (p.items) |item| try visit(item, seen),
    }
}

test "parser_dealx.coverage" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    var seen = std.AutoHashMap(ast.NodeKind, usize).init(gpa);
    defer seen.deinit();

    // Walk all 4 showcase .dealx files.
    for (showcase_files) |path| {
        const source = try cwd.readFileAlloc(io, path, gpa, .unlimited);
        defer gpa.free(source);

        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const root = (try parser.parseFile(arena.allocator(), source, .dealx, &diags)) orelse continue;
        try visit(root, &seen);
    }

    // Walk synthetic fixtures.
    for (synthetic_fixtures) |src| {
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const root = (try parser.parseFile(arena.allocator(), src, .dealx, &diags)) orelse continue;
        try visit(root, &seen);
    }

    // Print tally for visibility.
    std.debug.print("=== parser_dealx.coverage tally ===\n", .{});
    for (required) |req| {
        const n = seen.get(req.kind) orelse 0;
        std.debug.print("  {s} = {d}\n", .{ req.name, n });
    }
    std.debug.print("===================================\n", .{});

    // Gate: every required kind has count ≥ 1.
    var failures: usize = 0;
    for (required) |req| {
        const n = seen.get(req.kind) orelse 0;
        if (n == 0) {
            std.debug.print(
                "parser_dealx.coverage FAILED: NodeKind .{s} has count 0\n",
                .{req.name},
            );
            failures += 1;
        }
    }
    if (failures > 0) return error.TestUnexpectedCoverage;
}
