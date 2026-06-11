//! recovery.statement — D-17 statement-tier synchronization tests.
//!
//! Phase 1.5 plan 01 strengthens these cases from "did the parser emit
//! SOMETHING?" to a two-axis BDD contract mirroring
//! parser_dealx_tag_balance.zig:
//!
//!   axis A (diagnostic): assert the EXACT diagnostic code(s) emitted,
//!     referencing src/diagnostics.zig Codes constants by name (never
//!     inline "E0100" string literals).
//!   axis B (AST shape):  assert a NAMED structural property of the
//!     recovered AST at the expected position (e.g. members[0].name == "ok"),
//!     not just `definitions.len >= 1`.
//!
//! Empirical truths locked in by this file (probed against parser_deal.zig
//! at Phase 1.5-01 commit time — DO NOT relax these to inequalities):
//!
//!   Case A — `attribute a : T; attribute b : T;` is well-formed; the
//!     baseline is "0 diagnostics, 2 attribute_usage members named a/b".
//!     (LEARNINGS Surprise #8 finding #2 — the historic comment falsely
//!     claimed missing-semicolon. We keep this as the well-formed baseline
//!     because the missing-semicolon variant `attribute a : T attribute b`
//!     ALSO parses cleanly to 2 members with no diagnostic — the parser's
//!     attribute-statement boundary is whitespace-tolerant.)
//!
//!   Case B — `12345 99999 ; attribute ok : T;` produces exactly one
//!     E0100 (e_expected_token) + one E0400 (e_sync_dropped_tokens)
//!     and recovers to part_def P with members.len == 1, members[0]
//!     attribute_usage name == "ok".
//!
//!   Case C — `x = 42 y = 7;` produces exactly one E0100 + one E0400.
//!     The recovery is severe: BOTH operator_statements are skipped by
//!     the statement-sync recovery sweep, so part_def P has
//!     members.len == 0. This is the actual contract; the historic
//!     `.len`-vacuous historic assertion admitted any recovery shape
//!     including the empty one. Phase 1.5 LOCKS the empty-recovery
//!     shape as the contract; if Plan 02 strictness work changes the
//!     recovery to retain the `y = 7;` statement, this expectation
//!     is the gate that flags the change.

const std = @import("std");
const ast = @import("ast");
const parser_deal = @import("parser_deal");
const diagnostics_mod = @import("diagnostics");

const Codes = diagnostics_mod.Codes;
const Diagnostic = diagnostics_mod.Diagnostic;

// ── BDD helpers (mirrored from parser_dealx_tag_balance.zig). ─────────
// Kept inline because the helpers are 9 lines total and duplicated across
// only 3 test files; an _diag_helpers.zig module would be more file
// movement than payoff for this small surface area.

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

test "recovery.statement" {
    const gpa = std.testing.allocator;

    // ── Case A: well-formed two-attribute body — baseline shape.
    //
    // Historic comment claimed "missing semicolon between attribute
    // members"; LEARNINGS Surprise #8 finding #2 corrected this — the
    // source is well-formed. We keep this case as the WELL-FORMED
    // baseline so that the strict shape `0 diagnostics, 2 named members`
    // is a regression gate for any future strictness change.
    {
        const source =
            "package foo;\npart def P { attribute a : T; attribute b : T; }";
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(Diagnostic) = .empty;
        const root = (try parse(arena.allocator(), source, &diags)) orelse {
            return error.TestUnexpectedNull;
        };
        // axis A — zero diagnostics; well-formed baseline.
        try std.testing.expectEqual(@as(usize, 0), diags.items.len);
        const df = root.payload.deal_file;
        // axis B — exactly one part_def named "P" with members [a, b].
        try std.testing.expectEqual(@as(usize, 1), df.definitions.len);
        const p = df.definitions[0];
        try std.testing.expectEqual(ast.NodeKind.part_def, p.kind);
        try std.testing.expectEqualStrings("P", p.payload.part_def.name);
        try std.testing.expectEqual(@as(usize, 2), p.payload.part_def.members.len);
        try std.testing.expectEqual(ast.NodeKind.attribute_usage, p.payload.part_def.members[0].kind);
        try std.testing.expectEqualStrings("a", p.payload.part_def.members[0].payload.attribute_usage.name);
        try std.testing.expectEqual(ast.NodeKind.attribute_usage, p.payload.part_def.members[1].kind);
        try std.testing.expectEqualStrings("b", p.payload.part_def.members[1].payload.attribute_usage.name);
    }

    // ── Case B: garbage tokens inside a definition body.
    //
    // `12345 99999 ;` is dropped by the statement-tier sync sweep
    // (D-17). The contract is:
    //   * exactly one E0100 (e_expected_token) — "unexpected token in
    //     definition body" — fires at the start of the garbage.
    //   * exactly one E0400 (e_sync_dropped_tokens) — info-severity
    //     recovery marker.
    //   * the part_def has exactly one member, attribute_usage named "ok".
    // The loop-to-find-the-named-member shape protects against the
    // garbage tokens being silently absorbed as a phantom member with
    // a different index in the future.
    {
        const source =
            "package foo;\npart def P { 12345 99999 ; attribute ok : T; }";
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(Diagnostic) = .empty;
        const root = (try parse(arena.allocator(), source, &diags)) orelse {
            return error.TestUnexpectedNull;
        };
        // axis A — exactly one E0100 for the garbage + one E0400 sync marker.
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, Codes.e_expected_token));
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, Codes.e_sync_dropped_tokens));
        // The E0100 must point at the garbage tokens, not at the `attribute` keyword.
        const e100 = firstWithCode(diags.items, Codes.e_expected_token).?;
        // "package foo;\npart def P { " is 26 bytes; "12345" starts there.
        try std.testing.expectEqual(@as(u32, 26), e100.span.start);
        // axis B — exactly one part_def named "P" with the recovered member "ok".
        const df = root.payload.deal_file;
        try std.testing.expectEqual(@as(usize, 1), df.definitions.len);
        const p = df.definitions[0];
        try std.testing.expectEqualStrings("P", p.payload.part_def.name);
        // Loop-find the "ok" member (do not hardcode the index — the
        // recovery shape may move it as Plan 02 strictness evolves).
        var found_ok = false;
        for (p.payload.part_def.members) |m| {
            if (m.kind == .attribute_usage and std.mem.eql(u8, m.payload.attribute_usage.name, "ok")) {
                found_ok = true;
                break;
            }
        }
        try std.testing.expect(found_ok);
    }

    // ── Case C: missing semicolon between operator-statements.
    //
    // `x = 42 y = 7;` — the statement-sync recovery sweep drops BOTH
    // operator-statements because the missing `;` after `42` derails
    // the body parser at the first statement boundary and the sync
    // helper consumes through the next `;` (which is at the end of
    // `y = 7;`). The CURRENT contract is:
    //   * exactly one E0100 + one E0400.
    //   * part_def P has members.len == 0 (severe recovery).
    // If Plan 02 tightens parser strictness so the `y = 7;` statement
    // is retained as a member, this exact-zero assertion is the gate
    // that flags the contract change.
    {
        const source =
            "package foo;\npart def P { x = 42 y = 7; }";
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        var diags: std.ArrayList(Diagnostic) = .empty;
        const root = (try parse(arena.allocator(), source, &diags)) orelse {
            return error.TestUnexpectedNull;
        };
        // axis A — exactly one E0100 + one E0400.
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, Codes.e_expected_token));
        try std.testing.expectEqual(@as(usize, 1), countCode(diags.items, Codes.e_sync_dropped_tokens));
        // axis B — recovered to a single part_def P with no members
        // (locked empirical truth — see file doc-comment above).
        const df = root.payload.deal_file;
        try std.testing.expectEqual(@as(usize, 1), df.definitions.len);
        const p = df.definitions[0];
        try std.testing.expectEqual(ast.NodeKind.part_def, p.kind);
        try std.testing.expectEqualStrings("P", p.payload.part_def.name);
        try std.testing.expectEqual(@as(usize, 0), p.payload.part_def.members.len);
    }
}
