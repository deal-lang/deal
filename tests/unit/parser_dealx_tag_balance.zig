//! parser_dealx.tag_balance — D-07 / RESEARCH §Pattern 4 enforcement test.
//!
//! Five cases:
//!   A. Clean match: `[<system Foo>][</system>]` → 0 diagnostics.
//!   B. Mismatched close (E0302): `[<system Foo>][</subsystem>]` → 1 E0302
//!      with both spans (primary = close span; secondary span = open span
//!      with label containing "opened here").
//!   C. Unmatched close (E0301): `[</orphan>]` → 1 E0301 at the close span.
//!   D. Depth bound (E0303): 1025 nested `[<lvl>]` opens — E0303 fires at
//!      depth 1024 (T-04-01 DoS bound) without panic.
//!   E. Unclosed at EOF (E0304): `[<system Foo>]` (no close) → 1 E0304
//!      referring to the open-tag span.

const std = @import("std");
const ast = @import("ast");
const parser_dealx = @import("parser_dealx");
const diagnostics_mod = @import("diagnostics");

fn countCode(diags: []const diagnostics_mod.Diagnostic, code: []const u8) usize {
    var n: usize = 0;
    for (diags) |d| {
        if (std.mem.eql(u8, d.code, code)) n += 1;
    }
    return n;
}

fn firstWithCode(diags: []const diagnostics_mod.Diagnostic, code: []const u8) ?diagnostics_mod.Diagnostic {
    for (diags) |d| {
        if (std.mem.eql(u8, d.code, code)) return d;
    }
    return null;
}

test "parser_dealx.tag_balance" {
    const gpa = std.testing.allocator;

    // ── Case A: clean match — no diagnostics, one system_block "Foo" ──
    {
        const src = "package p;\n[<system Foo>][</system>]";
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const root = (try parser_dealx.parseFile(arena.allocator(), src, &diags)) orelse return error.TestUnexpectedNull;
        if (diags.items.len > 0) {
            std.debug.print("case A: unexpected diagnostics:\n", .{});
            for (diags.items) |d| std.debug.print("  [{s}] {s}\n", .{ d.code, d.message });
            return error.TestUnexpectedDiagnostics;
        }
        try std.testing.expectEqual(@as(usize, 1), root.payload.dealx_file.root_tags.len);
        const sys = root.payload.dealx_file.root_tags[0];
        try std.testing.expectEqual(ast.NodeKind.system_block, sys.kind);
    }

    // ── Case B: mismatched close — E0302 with both spans ─────────────
    {
        const src = "package p;\n[<system Foo>][</subsystem>]";
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        _ = try parser_dealx.parseFile(arena.allocator(), src, &diags);
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, "E0302"));
        const d = firstWithCode(diags.items, "E0302").?;
        try std.testing.expectEqual(@as(usize, 1), d.secondary_spans.len);
        // Secondary span label should contain "opened here".
        try std.testing.expect(std.mem.indexOf(u8, d.secondary_spans[0].label, "opened here") != null);
        // Primary span should cover the `[</subsystem>]` close.
        try std.testing.expect(d.span.start >= "package p;\n[<system Foo>]".len);
        // Secondary span should cover the `[<system Foo>]` open.
        try std.testing.expectEqual(@as(u32, "package p;\n".len), d.secondary_spans[0].span.start);
    }

    // ── Case C: unmatched close — E0301 at orphan close span ──────────
    {
        const src = "package p;\n[</orphan>]";
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        _ = try parser_dealx.parseFile(arena.allocator(), src, &diags);
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, "E0301"));
        const d = firstWithCode(diags.items, "E0301").?;
        // Primary span should cover the orphan `[</orphan>]`.
        try std.testing.expectEqual(@as(u32, "package p;\n".len), d.span.start);
    }

    // ── Case D: depth bound — E0303 fires past MAX_TAG_DEPTH=1024 ─────
    {
        // Build source: 1025 opens of `[<subsystem>]` then 1025 closes of
        // `[</subsystem>]`. We use `subsystem` because it's a pair-tag
        // (calls pushTag) and the name IDENT is optional. The 1025th push
        // hits E0303 (MAX_TAG_DEPTH = 1024).
        var buf: std.ArrayList(u8) = .empty;
        defer buf.deinit(gpa);
        try buf.appendSlice(gpa, "package p;\n");
        var i: u32 = 0;
        while (i < 1025) : (i += 1) {
            try buf.appendSlice(gpa, "[<subsystem>]");
        }
        i = 0;
        while (i < 1025) : (i += 1) {
            try buf.appendSlice(gpa, "[</subsystem>]");
        }

        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        _ = parser_dealx.parseFile(arena.allocator(), buf.items, &diags) catch |err| {
            std.debug.print("case D: parseFile errored with {s}\n", .{@errorName(err)});
            return err;
        };
        try std.testing.expect(countCode(diags.items, "E0303") >= 1);
    }

    // ── Case E: unclosed at EOF — E0304 at the open span ─────────────
    {
        const src = "package p;\n[<system Foo>]";
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        _ = try parser_dealx.parseFile(arena.allocator(), src, &diags);
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, "E0304"));
        const d = firstWithCode(diags.items, "E0304").?;
        // Span should cover the open `[<system Foo>]` (we use the open
        // delimiter span on pushTag).
        try std.testing.expectEqual(@as(u32, "package p;\n".len), d.span.start);
    }
}
