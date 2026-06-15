//! Lexer tests for the behavioral surface tokens (Stage-2 S2.1):
//!   - `~>` → .item_flow (BH-6), longest-match over `~` (.sysml_conjugates)
//!   - `:=` → .colon_eq  (assign-action), longest-match over `:`/`:>`/`::`
//!   - the 20 behavioral keywords promote IDENT → kw_* (flat-reserved)
//!
//! Mirrors the lexer_calc.zig pattern: Lexer.init(src) + lex.next(.deal_def).

const std = @import("std");
const lexer = @import("lexer");

fn first(src: []const u8) lexer.Token {
    var lex = lexer.Lexer.init(src);
    return lex.next(.deal_def);
}

// ─── ~> item flow (BH-6) ─────────────────────────────────────────────────────

test "lexer_behavioral.item_flow — `~>` is a single .item_flow token (2 bytes)" {
    const t = first("~>");
    try std.testing.expectEqual(lexer.Tag.item_flow, t.tag);
    try std.testing.expectEqual(@as(u32, 0), t.span.start);
    try std.testing.expectEqual(@as(u32, 2), t.span.end);
}

test "lexer_behavioral.item_flow — bare `~` stays .sysml_conjugates (longest-match)" {
    const t = first("~ x");
    try std.testing.expectEqual(lexer.Tag.sysml_conjugates, t.tag);
    try std.testing.expectEqual(@as(u32, 1), t.span.end);
}

test "lexer_behavioral.item_flow — `a ~> b` token stream" {
    var lex = lexer.Lexer.init("a ~> b");
    try std.testing.expectEqual(lexer.Tag.ident, lex.next(.deal_def).tag);
    try std.testing.expectEqual(lexer.Tag.item_flow, lex.next(.deal_def).tag);
    try std.testing.expectEqual(lexer.Tag.ident, lex.next(.deal_def).tag);
    try std.testing.expectEqual(lexer.Tag.eof, lex.next(.deal_def).tag);
}

// ─── := assign-action operator ───────────────────────────────────────────────

test "lexer_behavioral.colon_eq — `:=` is a single .colon_eq token (2 bytes)" {
    const t = first(":=");
    try std.testing.expectEqual(lexer.Tag.colon_eq, t.tag);
    try std.testing.expectEqual(@as(u32, 2), t.span.end);
}

test "lexer_behavioral.colon_eq — regressions: `:`, `:>`, `::` unaffected" {
    try std.testing.expectEqual(lexer.Tag.colon, first(": x").tag);
    try std.testing.expectEqual(lexer.Tag.sysml_specializes, first(":> Base").tag);
    try std.testing.expectEqual(lexer.Tag.coloncolon, first(":: x").tag);
}

test "lexer_behavioral.colon_eq — `assign x := e` token stream" {
    var lex = lexer.Lexer.init("assign x := e");
    try std.testing.expectEqual(lexer.Tag.kw_assign, lex.next(.deal_def).tag);
    try std.testing.expectEqual(lexer.Tag.ident, lex.next(.deal_def).tag);
    try std.testing.expectEqual(lexer.Tag.colon_eq, lex.next(.deal_def).tag);
    try std.testing.expectEqual(lexer.Tag.ident, lex.next(.deal_def).tag);
}

// ─── behavioral keywords ─────────────────────────────────────────────────────

test "lexer_behavioral.keywords — each behavioral spelling promotes to kw_*" {
    const cases = .{
        .{ "decide", lexer.Tag.kw_decide },
        .{ "par", lexer.Tag.kw_par },
        .{ "loop", lexer.Tag.kw_loop },
        .{ "while", lexer.Tag.kw_while },
        .{ "until", lexer.Tag.kw_until },
        .{ "for", lexer.Tag.kw_for },
        .{ "send", lexer.Tag.kw_send },
        .{ "accept", lexer.Tag.kw_accept },
        .{ "assign", lexer.Tag.kw_assign },
        .{ "bind", lexer.Tag.kw_bind },
        .{ "node", lexer.Tag.kw_node },
        .{ "succession", lexer.Tag.kw_succession },
        .{ "on", lexer.Tag.kw_on },
        .{ "entry", lexer.Tag.kw_entry },
        .{ "do", lexer.Tag.kw_do },
        .{ "exit", lexer.Tag.kw_exit },
        .{ "else", lexer.Tag.kw_else },
        .{ "start", lexer.Tag.kw_start },
        .{ "done", lexer.Tag.kw_done },
        .{ "terminate", lexer.Tag.kw_terminate },
    };
    inline for (cases) |c| {
        try std.testing.expectEqual(c[1], first(c[0]).tag);
    }
}

test "lexer_behavioral.keywords — `accept` (action) is distinct from `accepts` (method list)" {
    try std.testing.expectEqual(lexer.Tag.kw_accept, first("accept").tag);
    try std.testing.expectEqual(lexer.Tag.kw_accepts, first("accepts").tag);
}

test "lexer_behavioral.succession — `start -> done` token stream" {
    var lex = lexer.Lexer.init("start -> done");
    try std.testing.expectEqual(lexer.Tag.kw_start, lex.next(.deal_def).tag);
    try std.testing.expectEqual(lexer.Tag.thin_arrow, lex.next(.deal_def).tag);
    try std.testing.expectEqual(lexer.Tag.kw_done, lex.next(.deal_def).tag);
    try std.testing.expectEqual(lexer.Tag.eof, lex.next(.deal_def).tag);
}
