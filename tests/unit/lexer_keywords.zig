//! lexer.keywords — proves every entry in the 108-spelling keyword table
//! resolves to its declared kw_* tag, and that arbitrary identifiers
//! NOT in the table stay as `.ident`.
//!
//! The plan calls for the test to iterate the table via `inline for`.
//! We use the comptime-known entry list from `keywords.global_keywords`
//! so any future addition/removal automatically updates the test.
//!
//! Phase 05.2 additions: calc/return/require (3 new entries, 85→88).
//! Stage-2 S2.1 additions: 20 behavioral keywords (BH-1..BH-7, 88→108).
//! D-07 guard: `sig` must NOT appear in global_keywords (stays .ident).

const std = @import("std");
const lexer = @import("lexer");
const keywords = @import("keywords");

test "lexer.keywords" {
    const gpa = std.testing.allocator;
    _ = gpa;

    // ─── Part A: Every keyword spelling resolves to its tag ────────
    // StaticStringMap exposes the comptime `.kvs.keys` / `.kvs.values`
    // arrays as runtime slices. We iterate and lex each spelling alone.
    const kvs = keywords.global_keywords.keys();
    const vals = keywords.global_keywords.values();
    try std.testing.expectEqual(@as(usize, 108), kvs.len);
    try std.testing.expectEqual(@as(usize, 108), vals.len);

    var i: usize = 0;
    while (i < kvs.len) : (i += 1) {
        const spelling = kvs[i];
        const expected_tag = vals[i];

        var lex = lexer.Lexer.init(spelling);
        const t0 = lex.next(.deal_def);
        std.testing.expectEqual(expected_tag, t0.tag) catch |err| {
            std.debug.print(
                "keyword `{s}` lexed as `{s}` (expected `{s}`)\n",
                .{ spelling, @tagName(t0.tag), @tagName(expected_tag) },
            );
            return err;
        };
        // Span covers the whole spelling.
        try std.testing.expectEqual(@as(u32, 0), t0.span.start);
        try std.testing.expectEqual(@as(u32, @intCast(spelling.len)), t0.span.end);
        // Next token is EOF.
        const t1 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.eof, t1.tag);
    }

    // ─── Part B: Non-keyword identifiers stay as .ident ────────────
    const non_kws = [_][]const u8{
        "foo",
        "Battery",
        "mySymbol",
        "_private",
        "CamelCase42",
    };
    inline for (non_kws) |spelling| {
        var lex = lexer.Lexer.init(spelling);
        const t = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.ident, t.tag);
        try std.testing.expectEqualSlices(
            u8,
            spelling,
            spelling[t.span.start..t.span.end],
        );
    }

    // ─── Part C: Boolean literals are NOT keywords ─────────────────
    // Per lexical.ebnf §4 BOOLEAN_LITERAL note, `true` / `false` lex
    // as boolean_literal even though they're reserved.
    for ([_][]const u8{ "true", "false" }) |spelling| {
        var lex = lexer.Lexer.init(spelling);
        const t = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.boolean_literal, t.tag);
    }

    // ─── Part D: Primitive type names are NOT keywords ─────────────
    // Per lexical.ebnf §5 comment block (lines 430-432), Boolean /
    // Integer / Real / String are emitted as .ident — the semantic
    // analyzer resolves them as built-in types later.
    for ([_][]const u8{ "Boolean", "Integer", "Real", "String" }) |spelling| {
        var lex = lexer.Lexer.init(spelling);
        const t = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.ident, t.tag);
    }

    // ─── Part E: Phase 05.2 — calc/return/require are global keywords ──
    // SD-21/22: calc, return, require are newly reserved globally.
    try std.testing.expectEqual(
        lexer.Tag.kw_calc,
        keywords.global_keywords.get("calc") orelse @panic("calc not in map"),
    );
    try std.testing.expectEqual(
        lexer.Tag.kw_return,
        keywords.global_keywords.get("return") orelse @panic("return not in map"),
    );
    try std.testing.expectEqual(
        lexer.Tag.kw_require,
        keywords.global_keywords.get("require") orelse @panic("require not in map"),
    );

    // ─── Part F: D-07 guard — sig must NOT be globally reserved ────────
    // `sig` is contextual-only (appears after `=>` in return contracts).
    // It must remain `.ident` in the global keyword table.
    try std.testing.expectEqual(
        @as(?lexer.Tag, null),
        keywords.global_keywords.get("sig"),
    );
}
