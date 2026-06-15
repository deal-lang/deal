//! property.span_containment — locks the span-containment invariant
//! across every AST node of every showcase file plus the synthetic
//! fixtures from parser_deal.coverage.
//!
//! Contract: for every parent-child edge in every AST,
//!   child.span.start >= parent.span.start
//!   child.span.end   <= parent.span.end
//!
//! Purpose: the 19 locked AST snapshots are guarded by byte-equality,
//! but byte-equality alone can hide span regressions — a payload field
//! could be reshuffled to a span outside its parent's range without
//! changing the JSON bytes (because the JSON emitter renders fields
//! independently). This walker locks down the structural invariant
//! against the regression class "AST identical but spans corrupted"
//! that snapshot byte-equality alone would silently allow (per the
//! Phase 1 LEARNINGS lesson "Snapshot byte-equality hides
//! initial-correctness bugs").
//!
//! Corpus:
//!   * 15 .deal showcase files (mirroring parser_deal.coverage)
//!   *  4 .dealx showcase files (mirroring parser_dealx.coverage)
//!   *  7 synthetic .deal fixtures from parser_deal.coverage that
//!     exercise NodeKinds not present in the 19-file corpus
//!     (visibility_wrapper, real_literal/boolean, unary,
//!     verification_block, precondition/postcondition,
//!     missing_def_kinds, requirement_usage).
//!
//! NOTE on `.redefinition` (Phase 1.5 IN-01): the `.redefinition`
//! NodeKind is declared in the AST union but is unconstructed by any
//! parser path in Phase 1.5 — `redefines base.sensor sensor : Lidar;`
//! actually parses to `.operator_statement`, not `.redefinition`.
//! The walker's explicit `.redefinition` arm (see WR-01 fix) is
//! preserved as future-proofing so a Phase 2+ change that begins
//! constructing `.redefinition` nodes cannot silently regress the
//! span-containment invariant. A synthetic fixture targeting this
//! NodeKind was DELIBERATELY removed in iteration 2 of the code-review
//! fix pass because its source was misleading — it produced
//! `.operator_statement` nodes instead.
//!
//! If a span violation is found, std.debug.print emits the offending
//! NodeKind + parent span + child span to stderr before the assertion
//! fires, so the failure points directly at the parser bug to fix.

const std = @import("std");
const ast = @import("ast");
const parser = @import("parser");
const diagnostics_mod = @import("diagnostics");

// ── Showcase file lists ────────────────────────────────────────────────
//
// `showcase_deal_files` mirrors parser_deal_coverage.zig:27-43 verbatim
// so the property test exercises the SAME corpus the coverage test
// already locks down. `showcase_dealx_files` mirrors
// parser_dealx_coverage.zig:27-32 (the 4 .dealx files) but stores the
// paths relative to the showcase root (no `tests/showcase/` prefix)
// to keep the iteration loop uniform with the .deal list.

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

const showcase_dealx_files = [_][]const u8{
    "model/traceability.dealx",
    "model/vehicle.dealx",
    "model/variants/performance.dealx",
    "model/variants/sedan.dealx",
};

// Synthetic .deal fixtures — copied verbatim from parser_deal_coverage.zig
// (lines 48-138). These exercise NodeKinds the 19 showcase files do not
// reach (visibility_wrapper, real_literal, boolean_literal,
// unary, verification_block, precondition_block, postcondition_block,
// requirement_usage, state_def, attribute_def, item_def, allocation_def,
// constraint_def). Keeping them in lockstep with the coverage test means
// the property walker covers every NodeKind the parser actually
// produces, not just the subset the 19 corpus files exercise.
//
// Phase 1.5 IN-01: the `redefinition` fixture present in earlier
// iterations was REMOVED because its source `redefines base.sensor
// sensor : Lidar;` parses to `.operator_statement`, not `.redefinition`
// — no parser path in Phase 1.5 constructs `.redefinition` nodes. The
// walker's explicit `.redefinition` arm remains for forward-compat (see
// top-of-file NOTE).

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

// ── Walker ─────────────────────────────────────────────────────────────
//
// `visit(child, parent_span)` is the recursive entry. The first action
// asserts the child span is contained in `parent_span` (failure prints
// the offending NodeKind + both spans to stderr). The switch over
// `node.payload` is exhaustive — Zig 0.16.0's union(NodeKind) makes a
// missing arm a compile error, which IS the contract: any NodeKind we
// add later will fail to compile here until its children are recursed.

fn visit(node: *ast.Node, parent_span: ast.Span) anyerror!void {
    if (!(node.span.start >= parent_span.start and node.span.end <= parent_span.end)) {
        std.debug.print(
            "property.span_containment: violation at NodeKind .{s}\n" ++
                "  child.span  = ({d}..{d})\n" ++
                "  parent.span = ({d}..{d})\n",
            .{
                @tagName(node.kind),
                node.span.start, node.span.end,
                parent_span.start, parent_span.end,
            },
        );
        return error.TestSpanContainmentViolation;
    }

    // Recurse into children. The arm structure mirrors
    // parser_deal_coverage.zig::visit + parser_dealx_coverage.zig::visit,
    // but every child descent passes `node.span` as the new parent
    // span — that is the contract assertion.
    switch (node.payload) {
        .deal_file => |p| {
            if (p.header) |h| try visit(h, node.span);
            if (p.package_decl) |pd| try visit(pd, node.span);
            for (p.imports) |n| try visit(n, node.span);
            for (p.exports) |n| try visit(n, node.span);
            for (p.definitions) |n| try visit(n, node.span);
        },
        .dealx_file => |p| {
            if (p.header) |h| try visit(h, node.span);
            if (p.package_decl) |pd| try visit(pd, node.span);
            for (p.imports) |n| try visit(n, node.span);
            for (p.root_tags) |n| try visit(n, node.span);
        },
        .header_block, .package_decl, .import_decl, .export_decl => {
            // Leaves at the Node level — fields are HeaderField/strings/
            // ImportItem, not Nodes. No recursion possible.
        },
        .part_def, .port_def, .action_def, .state_def, .attribute_def,
        .item_def, .interface_def, .connection_def, .flow_def,
        .allocation_def, .requirement_def,
        .need_def, .use_case_def, .actor_def => {
            const ed = getElementDef(node).?;
            if (ed.specializes) |s| try visit(s, node.span);
            if (ed.doc) |d| try visit(d, node.span);
            for (ed.annotations) |a| try visit(a, node.span);
            for (ed.members) |m| try visit(m, node.span);
        },
        // Phase 05.2: constraint_def uses ConstraintDefinition (not ElementDef)
        .constraint_def => |p| {
            if (p.params) |params| try visit(params, node.span);
            if (p.specializes) |s| try visit(s, node.span);
            if (p.doc) |d| try visit(d, node.span);
            for (p.annotations) |a| try visit(a, node.span);
            try visit(p.body, node.span);
        },
        // Phase 05.2: calc_def and related new NodeKinds
        .calc_def => |p| {
            try visit(p.params, node.span);
            try visit(p.type_node, node.span);
            if (p.return_contract) |rc| try visit(rc, node.span);
            if (p.doc) |d| try visit(d, node.span);
            for (p.annotations) |a| try visit(a, node.span);
            try visit(p.body, node.span);
        },
        .param_list => |p| for (p.params) |param| try visit(param, node.span),
        .param_decl => |p| {
            try visit(p.type_node, node.span);
            if (p.multiplicity) |m| try visit(m, node.span);
        },
        .return_contract => |p| for (p.items) |item| try visit(item, node.span),
        .precision_spec => |p| try visit(p.value, node.span),
        .constraint_ref => |p| {
            if (p.args) |args| for (args) |a| try visit(a, node.span);
        },
        .constraint_body => |p| for (p.members) |m| try visit(m, node.span),
        .calc_body => |p| {
            for (p.bindings) |b| try visit(b, node.span);
            for (p.annotations) |a| try visit(a, node.span);
            try visit(p.return_stmt, node.span);
        },
        .require_statement => |p| {
            try visit(p.condition, node.span);
            if (p.precision) |prec| try visit(prec, node.span);
        },
        .part_usage, .port_usage, .attribute_usage, .action_usage,
        .actor_usage, .subject_usage, .state_usage, .item_usage,
        .interface_usage, .connection_usage, .flow_usage,
        .allocation_usage, .requirement_usage, .constraint_usage,
        .need_usage, .use_case_usage => {
            const eu = getElementUsage(node).?;
            if (eu.doc) |d| try visit(d, node.span);
            if (eu.type_node) |t| try visit(t, node.span);
            if (eu.multiplicity) |m| try visit(m, node.span);
            if (eu.default_value) |v| try visit(v, node.span);
            if (eu.inline_body) |b| try visit(b, node.span);
            for (eu.annotations) |a| try visit(a, node.span);
        },
        .type_annotation, .multiplicity, .modifier_list,
        .specialization, .structural_relationship,
        .doc_comment => {
            // Leaves — no child Nodes to recurse into.
        },
        .redefinition => |p| {
            // `.redefinition` carries an optional child Node `value`.
            // It is currently unproduced by any parser path (Phase 1.5
            // grep across src/ confirms only the type declaration and
            // union arm exist), so this arm is dormant today. Wired
            // explicitly so a future Phase 2+ change that begins
            // constructing `.redefinition` nodes cannot silently regress
            // the span-containment invariant for that payload (WR-01).
            if (p.value) |v| try visit(v, node.span);
        },
        .visibility_wrapper => |p| {
            for (p.members) |m| try visit(m, node.span);
        },
        .operator_statement => |p| {
            if (p.value) |v| try visit(v, node.span);
        },
        .inline_body => |p| {
            for (p.members) |m| try visit(m, node.span);
        },
        .annotation => |p| {
            if (p.value) |v| try visit(v, node.span);
            if (p.body) |b| try visit(b, node.span);
        },
        .annotation_body => |p| {
            for (p.fields) |f| try visit(f.value, node.span);
        },
        .verification_block => |p| {
            for (p.fields) |f| try visit(f.value, node.span);
        },
        .precondition_block => |p| {
            for (p.conditions) |c| try visit(c, node.span);
        },
        .postcondition_block => |p| {
            for (p.conditions) |c| try visit(c, node.span);
        },
        .binary => |p| {
            try visit(p.lhs, node.span);
            try visit(p.rhs, node.span);
        },
        .unary => |p| try visit(p.operand, node.span),
        .member_access => |p| try visit(p.receiver, node.span),
        .call => |p| {
            try visit(p.callee, node.span);
            for (p.args) |a| try visit(a, node.span);
        },
        .identifier, .int_literal, .real_literal, .string_literal,
        .boolean_literal => {
            // Literal leaves — no children.
        },
        .template_literal => |p| {
            for (p.parts) |part| {
                switch (part) {
                    .expr => |e| try visit(e, node.span),
                    .text => {},
                }
            }
        },
        .interpolation => |p| try visit(p.expr, node.span),
        // .dealx composition arms — descend into their children.
        .system_block => |p| {
            for (p.children) |c| try visit(c, node.span);
        },
        .subsystem_block => |p| {
            for (p.children) |c| try visit(c, node.span);
        },
        .component_instance => |p| {
            for (p.attrs) |a| try visit(a, node.span);
        },
        .comp_connect => |p| {
            if (p.from_expr) |n| try visit(n, node.span);
            if (p.to_expr) |n| try visit(n, node.span);
            if (p.via_expr) |n| try visit(n, node.span);
            if (p.carrying_expr) |n| try visit(n, node.span);
            for (p.other_props) |n| try visit(n, node.span);
        },
        .expose_tag => |p| {
            for (p.attrs) |a| try visit(a, node.span);
        },
        .traceability_block => |p| {
            for (p.children) |c| try visit(c, node.span);
        },
        .allocate_tag => |p| {
            for (p.attrs) |a| try visit(a, node.span);
        },
        .satisfy_block => |p| {
            for (p.children) |c| try visit(c, node.span);
        },
        .validate_tag => |p| {
            for (p.attrs) |a| try visit(a, node.span);
        },
        .object_literal => |p| {
            for (p.fields) |f| try visit(f.value, node.span);
        },
        // Array literal (D-04) — recurse into items; span covers [ ... ].
        .array_literal => |p| {
            for (p.items) |item| try visit(item, node.span);
        },
    }
}

/// The root has no parent — pass its own span as parent_span so the
/// containment assertion holds trivially at the top of the recursion.
fn walkRoot(root: *ast.Node) !void {
    try visit(root, root.span);
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
        // constraint_def uses ConstraintDefinition (Phase 05.2), not ElementDef
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

test "property.span_containment" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    // ── 15 .deal showcase files ────────────────────────────────────
    for (showcase_deal_files) |rel| {
        const path = try std.fmt.allocPrint(gpa, "tests/showcase/{s}", .{rel});
        defer gpa.free(path);
        const source = try cwd.readFileAlloc(io, path, gpa, .unlimited);
        defer gpa.free(source);

        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const root = (try parser.parseFile(arena.allocator(), source, .deal, &diags)) orelse {
            std.debug.print("property.span_containment: parser returned null on {s}\n", .{path});
            return error.TestUnexpectedNull;
        };
        if (diags.items.len > 0) {
            // Showcase files are well-formed by Phase 1 contract; a
            // diagnostic here is a separate regression to surface
            // elsewhere. The property test only asserts span
            // containment — log to stderr but do not fail.
            std.debug.print(
                "property.span_containment: WARNING showcase {s} produced {d} diagnostics\n",
                .{ path, diags.items.len },
            );
        }
        walkRoot(root) catch |err| {
            std.debug.print("  ...while walking {s}\n", .{path});
            return err;
        };
    }

    // ── 4 .dealx showcase files ────────────────────────────────────
    for (showcase_dealx_files) |rel| {
        const path = try std.fmt.allocPrint(gpa, "tests/showcase/{s}", .{rel});
        defer gpa.free(path);
        const source = try cwd.readFileAlloc(io, path, gpa, .unlimited);
        defer gpa.free(source);

        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const root = (try parser.parseFile(arena.allocator(), source, .dealx, &diags)) orelse {
            std.debug.print("property.span_containment: parser returned null on {s}\n", .{path});
            return error.TestUnexpectedNull;
        };
        if (diags.items.len > 0) {
            std.debug.print(
                "property.span_containment: WARNING showcase {s} produced {d} diagnostics\n",
                .{ path, diags.items.len },
            );
        }
        walkRoot(root) catch |err| {
            std.debug.print("  ...while walking {s}\n", .{path});
            return err;
        };
    }

    // ── 8 synthetic .deal fixtures ─────────────────────────────────
    for (synthetic_fixtures) |fix| {
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const root = (try parser.parseFile(arena.allocator(), fix.src, .deal, &diags)) orelse {
            std.debug.print(
                "property.span_containment: parser returned null on synthetic fixture {s}\n",
                .{fix.name},
            );
            return error.TestUnexpectedNull;
        };
        // Synthetic fixtures MAY emit diagnostics (e.g. the
        // verification_block / precondition_postcondition fixtures
        // exercise edge syntax) — we tolerate those for the property
        // walk, same as showcase files.
        if (diags.items.len > 0) {
            std.debug.print(
                "property.span_containment: synthetic fixture {s} produced {d} diagnostics\n",
                .{ fix.name, diags.items.len },
            );
        }
        walkRoot(root) catch |err| {
            std.debug.print("  ...while walking synthetic fixture {s}\n", .{fix.name});
            return err;
        };
    }
}
