//! parser_dealx.smoke — Task 4.1a smoke test.
//!
//! Verifies the minimal skeleton: a `[<system Vehicle>]` containing one
//! `[<subsystem Body>]` parses through parser_dealx.parseFile and produces
//! a dealx_file root with the expected structure. No attributes, no inner
//! component instances — those belong to Tasks 4.1b / 4.1c.

const std = @import("std");
const ast = @import("ast");
const parser_dealx = @import("parser_dealx");
const diagnostics_mod = @import("diagnostics");

test "parser_dealx.smoke" {
    const gpa = std.testing.allocator;

    const source =
        \\package model;
        \\[<system Vehicle>]
        \\  [<subsystem Body>][</subsystem>]
        \\[</system>]
    ;

    var arena = std.heap.ArenaAllocator.init(gpa);
    defer arena.deinit();

    var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
    const root = (try parser_dealx.parseFile(arena.allocator(), source, &diags)) orelse {
        std.debug.print("parser_dealx.smoke: parseFile returned null\n", .{});
        return error.TestUnexpectedNull;
    };

    if (diags.items.len > 0) {
        std.debug.print("parser_dealx.smoke: unexpected diagnostics:\n", .{});
        for (diags.items) |d| {
            std.debug.print("  [{s}] {s} at [{d},{d}]\n", .{
                d.code, d.message, d.span.start, d.span.end,
            });
        }
        return error.TestUnexpectedDiagnostics;
    }

    try std.testing.expectEqual(ast.NodeKind.dealx_file, root.kind);

    const df = root.payload.dealx_file;
    try std.testing.expect(df.package_decl != null);
    try std.testing.expectEqual(@as(usize, 1), df.root_tags.len);

    const sys = df.root_tags[0];
    try std.testing.expectEqual(ast.NodeKind.system_block, sys.kind);
    const sb = sys.payload.system_block;
    try std.testing.expect(sb.name != null);
    try std.testing.expectEqualStrings("Vehicle", sb.name.?);
    try std.testing.expectEqual(@as(usize, 1), sb.children.len);

    const sub = sb.children[0];
    try std.testing.expectEqual(ast.NodeKind.subsystem_block, sub.kind);
    const ssb = sub.payload.subsystem_block;
    try std.testing.expect(ssb.name != null);
    try std.testing.expectEqualStrings("Body", ssb.name.?);
    try std.testing.expectEqual(@as(usize, 0), ssb.children.len);
}
