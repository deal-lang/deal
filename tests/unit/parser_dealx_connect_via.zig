//! parser_dealx.connect_via — Task 4.1b D-09 first-class comp_connect gate.
//!
//! Three cases:
//!   1. `via={CANHarness { connectorType: "Molex MX150" }}` — via_expr is a
//!      non-null ObjectLiteral with type_name="CANHarness" and one field.
//!   2. Both `via` and `carrying` populated.
//!   3. Nested inline objects (RESEARCH Pitfall 7) — via={Outer { inner:
//!      {nested: 1} }} — no panic, recursive structure intact.

const std = @import("std");
const ast = @import("ast");
const parser_dealx = @import("parser_dealx");
const diagnostics_mod = @import("diagnostics");

fn parseSource(arena: std.mem.Allocator, source: []const u8, diags: *std.ArrayList(diagnostics_mod.Diagnostic)) !?*ast.Node {
    return parser_dealx.parseFile(arena, source, diags);
}

fn firstConnect(root: *ast.Node) ?*ast.Node {
    if (root.kind != .dealx_file) return null;
    for (root.payload.dealx_file.root_tags) |n| {
        if (n.kind == .comp_connect) return n;
        // Allow connect nested in a system_block child (none of these
        // test sources nest, but be safe).
    }
    return null;
}

test "parser_dealx.connect_via" {
    const gpa = std.testing.allocator;

    // ── Case 1: via={CANHarness { connectorType: "Molex MX150" }} ─────
    {
        const src =
            \\package model;
            \\[<connect from="a" to="b" via={CANHarness { connectorType: "Molex MX150" }} />]
        ;
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const root = (try parseSource(arena.allocator(), src, &diags)) orelse return error.TestUnexpectedNull;
        if (diags.items.len > 0) {
            std.debug.print("case 1: unexpected diagnostics:\n", .{});
            for (diags.items) |d| std.debug.print("  [{s}] {s}\n", .{ d.code, d.message });
            return error.TestUnexpectedDiagnostics;
        }
        const conn = firstConnect(root) orelse return error.TestExpectedConnect;
        const cc = conn.payload.comp_connect;
        try std.testing.expect(cc.from_expr != null);
        try std.testing.expectEqual(ast.NodeKind.string_literal, cc.from_expr.?.kind);
        try std.testing.expectEqualStrings("a", cc.from_expr.?.payload.string_literal.value);
        try std.testing.expect(cc.to_expr != null);
        try std.testing.expectEqualStrings("b", cc.to_expr.?.payload.string_literal.value);
        try std.testing.expect(cc.via_expr != null);
        try std.testing.expectEqual(ast.NodeKind.object_literal, cc.via_expr.?.kind);
        const via_ol = cc.via_expr.?.payload.object_literal;
        try std.testing.expect(via_ol.type_name != null);
        try std.testing.expectEqualStrings("CANHarness", via_ol.type_name.?);
        try std.testing.expectEqual(@as(usize, 1), via_ol.fields.len);
        try std.testing.expectEqualStrings("connectorType", via_ol.fields[0].key);
        try std.testing.expect(cc.carrying_expr == null);
        try std.testing.expectEqual(@as(usize, 0), cc.other_props.len);
    }

    // ── Case 2: both via and carrying ─────────────────────────────────
    {
        const src =
            \\package model;
            \\[<connect from="a" to="b" via={X {}} carrying={Y {}} />]
        ;
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const root = (try parseSource(arena.allocator(), src, &diags)) orelse return error.TestUnexpectedNull;
        if (diags.items.len > 0) {
            std.debug.print("case 2: unexpected diagnostics:\n", .{});
            for (diags.items) |d| std.debug.print("  [{s}] {s}\n", .{ d.code, d.message });
            return error.TestUnexpectedDiagnostics;
        }
        const conn = firstConnect(root) orelse return error.TestExpectedConnect;
        const cc = conn.payload.comp_connect;
        try std.testing.expect(cc.via_expr != null);
        try std.testing.expect(cc.carrying_expr != null);
    }

    // ── Case 3: nested inline objects (Pitfall 7) ─────────────────────
    {
        const src =
            \\package model;
            \\[<connect via={Outer { inner: {nested: 1} }} />]
        ;
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const root = (try parseSource(arena.allocator(), src, &diags)) orelse return error.TestUnexpectedNull;
        if (diags.items.len > 0) {
            std.debug.print("case 3: unexpected diagnostics:\n", .{});
            for (diags.items) |d| std.debug.print("  [{s}] {s}\n", .{ d.code, d.message });
            return error.TestUnexpectedDiagnostics;
        }
        const conn = firstConnect(root) orelse return error.TestExpectedConnect;
        const cc = conn.payload.comp_connect;
        try std.testing.expect(cc.via_expr != null);
        try std.testing.expectEqual(ast.NodeKind.object_literal, cc.via_expr.?.kind);
        const outer = cc.via_expr.?.payload.object_literal;
        try std.testing.expect(outer.type_name != null);
        try std.testing.expectEqualStrings("Outer", outer.type_name.?);
        try std.testing.expectEqual(@as(usize, 1), outer.fields.len);
        try std.testing.expectEqualStrings("inner", outer.fields[0].key);
        // The inner value should be a bare ObjectLiteral (no type prefix)
        // with one field "nested" → int_literal 1.
        const inner_val = outer.fields[0].value;
        try std.testing.expectEqual(ast.NodeKind.object_literal, inner_val.kind);
        const inner_ol = inner_val.payload.object_literal;
        try std.testing.expect(inner_ol.type_name == null);
        try std.testing.expectEqual(@as(usize, 1), inner_ol.fields.len);
        try std.testing.expectEqualStrings("nested", inner_ol.fields[0].key);
        try std.testing.expectEqual(ast.NodeKind.int_literal, inner_ol.fields[0].value.kind);
    }
}
