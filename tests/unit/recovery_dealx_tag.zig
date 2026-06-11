//! recovery.dealx_tag — D-17 composition-tag-tier synchronization tests.
//!
//! Phase 1.5 plan 01 strengthens these cases into the two-axis BDD
//! contract from parser_dealx_tag_balance.zig:
//!
//!   axis A: exact diagnostic code(s) by string equality against
//!     `src/diagnostics.zig` `Codes` constants — never inline literals.
//!   axis B: named-property check on the recovered AST — root_tags
//!     count + kind + name at expected positions.
//!
//! Empirical truths locked in by this file (probed at Phase 1.5-01
//! commit time):
//!
//!   Case A — `[<system Foo>][</system>] 12345 [<system Bar>][</system>]`
//!     emits exactly one E0101 (e_expected_definition) + one E0400
//!     (e_sync_dropped_tokens); recovers to exactly 2 root_tags, both
//!     system_block, names "Foo" and "Bar".
//!
//!   Case B — `[</orphan>]` (orphan close tag) emits exactly one
//!     E0301 (e_unmatched_close_tag); recovers to exactly 0 root_tags.
//!
//!   Case C — `12345 67890 [<system Foo>][</system>]` emits exactly
//!     one E0101 + one E0400; recovers to exactly 1 root_tag,
//!     system_block, name "Foo".

const std = @import("std");
const ast = @import("ast");
const parser_dealx = @import("parser_dealx");
const diagnostics_mod = @import("diagnostics");

const Codes = diagnostics_mod.Codes;
const Diagnostic = diagnostics_mod.Diagnostic;

// ── BDD helpers (mirrored from parser_dealx_tag_balance.zig). ─────────

fn countCode(diags: []const Diagnostic, code: []const u8) usize {
    var n: usize = 0;
    for (diags) |d| {
        if (std.mem.eql(u8, d.code, code)) n += 1;
    }
    return n;
}

fn firstWithCode(diags: []const Diagnostic, code: []const u8) ?Diagnostic {
    for (diags) |d| {
        if (std.mem.eql(u8, d.code, code)) return d;
    }
    return null;
}

fn parse(allocator: std.mem.Allocator, source: []const u8, diags: *std.ArrayList(Diagnostic)) !?*ast.Node {
    return parser_dealx.parseFile(allocator, source, diags);
}

test "recovery.dealx_tag" {
    const gpa = std.testing.allocator;

    // ── Case A: stray digits between two valid systems.
    //
    // Locked contract:
    //   * exactly one E0101 (e_expected_definition) for the digits
    //     where a composition tag was expected.
    //   * exactly one E0400 (e_sync_dropped_tokens).
    //   * exactly 2 root_tags, both system_block, names "Foo" and "Bar".
    {
        const source =
            "package model;\n[<system Foo>][</system>]   12345   [<system Bar>][</system>]";
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(Diagnostic) = .empty;
        const root = (try parse(arena.allocator(), source, &diags)) orelse {
            return error.TestUnexpectedNull;
        };
        // axis A — exactly one E0101 + one E0400.
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, Codes.e_expected_definition));
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, Codes.e_sync_dropped_tokens));
        const dx = root.payload.dealx_file;
        // axis B — exactly two root_tags, both system_block, named Foo and Bar.
        // Exact `== 2` — anything more means phantom recovery, anything
        // less means the second tag got dropped.
        try std.testing.expectEqual(@as(usize, 2), dx.root_tags.len);
        var found_foo = false;
        var found_bar = false;
        for (dx.root_tags) |tag| {
            try std.testing.expectEqual(ast.NodeKind.system_block, tag.kind);
            const name = tag.payload.system_block.name orelse continue;
            if (std.mem.eql(u8, name, "Foo")) found_foo = true;
            if (std.mem.eql(u8, name, "Bar")) found_bar = true;
        }
        try std.testing.expect(found_foo);
        try std.testing.expect(found_bar);
    }

    // ── Case B: orphan close tag.
    //
    // Locked contract:
    //   * exactly one E0301 (e_unmatched_close_tag).
    //   * primary span starts after `package model;\n` (byte offset 15).
    //   * root_tags.len == 0 — no tag is recovered from a bare close.
    {
        const source = "package model;\n[</orphan>]";
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(Diagnostic) = .empty;
        const root = (try parse(arena.allocator(), source, &diags)) orelse {
            return error.TestUnexpectedNull;
        };
        // axis A — exactly one E0301, span at the orphan close.
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, Codes.e_unmatched_close_tag));
        const d = firstWithCode(diags.items, Codes.e_unmatched_close_tag).?;
        try std.testing.expectEqual(@as(u32, "package model;\n".len), d.span.start);
        // axis B — exactly zero root_tags.
        const dx = root.payload.dealx_file;
        try std.testing.expectEqual(@as(usize, 0), dx.root_tags.len);
    }

    // ── Case C: stray digits before a valid system.
    //
    // Locked contract:
    //   * exactly one E0101 + one E0400 for the leading digits.
    //   * exactly 1 root_tag, system_block, name "Foo".
    {
        const source =
            "package model;\n12345 67890 [<system Foo>][</system>]";
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(Diagnostic) = .empty;
        const root = (try parse(arena.allocator(), source, &diags)) orelse {
            return error.TestUnexpectedNull;
        };
        // axis A — exactly one E0101 + one E0400.
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, Codes.e_expected_definition));
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, Codes.e_sync_dropped_tokens));
        const dx = root.payload.dealx_file;
        // axis B — exactly one root_tag, system_block, name "Foo".
        try std.testing.expectEqual(@as(usize, 1), dx.root_tags.len);
        const tag = dx.root_tags[0];
        try std.testing.expectEqual(ast.NodeKind.system_block, tag.kind);
        try std.testing.expectEqualStrings("Foo", tag.payload.system_block.name.?);
    }
}
