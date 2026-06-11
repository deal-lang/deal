//! recovery.definition — D-17 definition-tier synchronization tests.
//!
//! Phase 1.5 plan 01 strengthens these cases from `.len >= 1`-style
//! presence checks into the two-axis BDD contract from
//! parser_dealx_tag_balance.zig:
//!
//!   axis A: exact diagnostic code(s) by string equality against
//!     `src/diagnostics.zig` `Codes` constants — never inline literals.
//!   axis B: named-property check on the recovered AST (e.g.
//!     definitions[idx].payload.part_def.name == "Good") at the
//!     expected position.
//!
//! Empirical truths locked in by this file (probed at Phase 1.5-01
//! commit time — DO NOT relax these to inequalities):
//!
//!   Case A — `12345 99999 part def P { }` emits exactly one E0101
//!     (e_expected_definition) plus one E0400 (e_sync_dropped_tokens);
//!     the parser recovers to exactly one part_def named "P" (no
//!     phantom extras).
//!
//!   Case B — `part def A { } !!!stray!!! part def B { }` emits
//!     exactly one E0101 + one E0400 and recovers BOTH part_defs
//!     named "A" and "B".
//!
//!   Case C — `part def Broken { attribute x : T\npart def Good { }`
//!     emits 2 × E0100 + 1 × E0400. The parser does NOT produce a
//!     separate `Good` part_def — it absorbs `part def Good` as a
//!     `part_usage` member named "def" inside the still-open `Broken`
//!     body (the body never finds a closing `;` or `}` before EOF and
//!     the recovery glues the tokens). The locked contract is:
//!       * exactly one part_def, name == "Broken"
//!       * Broken's members contain attribute_usage "x" AND part_usage "def"
//!     If Plan 02 strictness tightens this (e.g. the missing `;` after
//!     `: T` triggers definition-tier sync so `Good` becomes its own
//!     part_def), this expectation flags the change.

const std = @import("std");
const ast = @import("ast");
const parser_deal = @import("parser_deal");
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
    return parser_deal.parseFile(allocator, source, diags);
}

test "recovery.definition" {
    const gpa = std.testing.allocator;

    // ── Case A: garbage between two well-formed definitions.
    //
    // `12345 99999` at top-level triggers the definition-tier sync
    // sweep (D-17). Locked contract:
    //   * exactly one E0101 (e_expected_definition).
    //   * exactly one E0400 (e_sync_dropped_tokens).
    //   * exactly one part_def named "P" — no phantom extras from the
    //     dropped tokens.
    {
        const source =
            "package foo;\n12345 99999 part def P { }";
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(Diagnostic) = .empty;
        const root = (try parse(arena.allocator(), source, &diags)) orelse {
            return error.TestUnexpectedNull;
        };
        // axis A — exactly one E0101 + one E0400.
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, Codes.e_expected_definition));
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, Codes.e_sync_dropped_tokens));
        const df = root.payload.deal_file;
        // axis B — exactly one part_def named "P".
        try std.testing.expectEqual(@as(usize, 1), df.definitions.len);
        const p = df.definitions[0];
        try std.testing.expectEqual(ast.NodeKind.part_def, p.kind);
        try std.testing.expectEqualStrings("P", p.payload.part_def.name);
    }

    // ── Case B: well-formed `A`, stray garbage, well-formed `B`.
    //
    // Locked contract:
    //   * exactly one E0101 + one E0400 for `!!!stray!!!`.
    //   * exactly two part_defs named "A" and "B" — recovery does NOT
    //     produce a phantom third definition.
    {
        const source =
            "package foo;\npart def A { }\n!!!stray!!!\npart def B { }";
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(Diagnostic) = .empty;
        const root = (try parse(arena.allocator(), source, &diags)) orelse {
            return error.TestUnexpectedNull;
        };
        // axis A — exactly one E0101 + one E0400.
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, Codes.e_expected_definition));
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, Codes.e_sync_dropped_tokens));
        const df = root.payload.deal_file;
        // axis B — exactly two part_defs, names "A" and "B".
        try std.testing.expectEqual(@as(usize, 2), df.definitions.len);
        var found_a = false;
        var found_b = false;
        for (df.definitions) |def| {
            if (def.kind == .part_def) {
                if (std.mem.eql(u8, def.payload.part_def.name, "A")) found_a = true;
                if (std.mem.eql(u8, def.payload.part_def.name, "B")) found_b = true;
            }
        }
        try std.testing.expect(found_a);
        try std.testing.expect(found_b);
    }

    // ── Case C: truncated body (missing semicolon after `: T`).
    //
    // The parser cannot distinguish "missing `;` after attribute" from
    // "next definition keyword `part`" inside an open body, so the
    // statement-tier sync absorbs `part def Good` as a `part_usage`
    // member named "def" inside `Broken`.
    //
    // Locked contract (empirical — see file doc-comment):
    //   * countCode(E0100) >= 1 (the missing `;` after `: T`).
    //   * exactly one E0400.
    //   * exactly one part_def, name == "Broken".
    //   * Broken's members include attribute_usage "x" AND part_usage "def".
    {
        const source =
            "package foo;\npart def Broken { attribute x : T\npart def Good { }";
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(Diagnostic) = .empty;
        const root = (try parse(arena.allocator(), source, &diags)) orelse {
            return error.TestUnexpectedNull;
        };
        // axis A — at least one E0100 (the parser emits two here:
        // one at `part` and one at `def`). Asserting `>= 1` rather
        // than exact-2 leaves Plan 02 room to deduplicate without
        // breaking this test, while still locking the
        // "missing-semicolon must produce E0100" contract.
        try std.testing.expect(countCode(diags.items, Codes.e_expected_token) >= 1);
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, Codes.e_sync_dropped_tokens));
        const df = root.payload.deal_file;
        // axis B — exactly one part_def named "Broken".
        try std.testing.expectEqual(@as(usize, 1), df.definitions.len);
        const broken = df.definitions[0];
        try std.testing.expectEqual(ast.NodeKind.part_def, broken.kind);
        try std.testing.expectEqualStrings("Broken", broken.payload.part_def.name);
        // Broken's members contain the attribute "x" AND the recovered
        // `part_usage` named "def" (the `def` token from `part def Good`).
        var found_x = false;
        var found_def = false;
        for (broken.payload.part_def.members) |m| {
            switch (m.payload) {
                .attribute_usage => |au| if (std.mem.eql(u8, au.name, "x")) {
                    found_x = true;
                },
                .part_usage => |pu| if (std.mem.eql(u8, pu.name, "def")) {
                    found_def = true;
                },
                else => {},
            }
        }
        try std.testing.expect(found_x);
        try std.testing.expect(found_def);
    }
}
