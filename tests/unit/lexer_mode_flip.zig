//! lexer.mode_flip — proves D-06's central mode-flip rule by lexing
//! the SAME byte sequence in different modes and asserting the token
//! stream differs in exactly the expected way.
//!
//! This test is the single most important guard in Plan 02: the four-mode
//! dispatch is the dual-grammar architecture's hinge. If `[<` ever
//! tokenizes the wrong way, Plans 03 and 04 fail in confusing ways.

const std = @import("std");
const lexer = @import("lexer");

fn lexAll(
    source: []const u8,
    mode: lexer.Mode,
    out: *std.ArrayList(lexer.Token),
    allocator: std.mem.Allocator,
) !void {
    var lex = lexer.Lexer.init(source);
    while (true) {
        const t = lex.next(mode);
        try out.append(allocator, t);
        if (t.tag == .eof) break;
    }
}

test "lexer.mode_flip" {
    const gpa = std.testing.allocator;

    // ─── Case 1: "[<sys>]" in .deal_def ─────────────────────────────
    // Should lex as: l_bracket, lt, ident(sys), gt, r_bracket, eof.
    {
        const source = "[<sys>]";
        var toks: std.ArrayList(lexer.Token) = .empty;
        defer toks.deinit(gpa);
        try lexAll(source, .deal_def, &toks, gpa);

        try std.testing.expectEqual(@as(usize, 6), toks.items.len);
        try std.testing.expectEqual(lexer.Tag.l_bracket, toks.items[0].tag);
        try std.testing.expectEqual(lexer.Tag.lt, toks.items[1].tag);
        try std.testing.expectEqual(lexer.Tag.ident, toks.items[2].tag);
        try std.testing.expectEqualSlices(
            u8,
            "sys",
            source[toks.items[2].span.start..toks.items[2].span.end],
        );
        try std.testing.expectEqual(lexer.Tag.gt, toks.items[3].tag);
        try std.testing.expectEqual(lexer.Tag.r_bracket, toks.items[4].tag);
        try std.testing.expectEqual(lexer.Tag.eof, toks.items[5].tag);
    }

    // ─── Case 2: "[<sys>]" in .dealx_outer ──────────────────────────
    // Should lex as: tag_open, ident(sys), and then the lexer is now
    // INSIDE the tag — but we're still calling with .dealx_outer (the
    // parser would switch to .dealx_tag here). In .dealx_outer the `>`
    // and `]` lex as separate tokens. So we'd get:
    //   tag_open, ident(sys), gt, r_bracket, eof.
    //
    // That's the right behavior because the lexer is stateless w.r.t.
    // mode (D-06): the parser is what would have switched to .dealx_tag
    // after consuming tag_open. We assert ONLY the mode-flip semantics
    // of the OPENING `[<` here; the closing `>]` recognition is the
    // job of Case 3 below.
    {
        const source = "[<sys>]";
        var toks: std.ArrayList(lexer.Token) = .empty;
        defer toks.deinit(gpa);
        try lexAll(source, .dealx_outer, &toks, gpa);

        try std.testing.expectEqual(@as(usize, 5), toks.items.len);
        try std.testing.expectEqual(lexer.Tag.tag_open, toks.items[0].tag);
        try std.testing.expectEqual(lexer.Tag.ident, toks.items[1].tag);
        try std.testing.expectEqualSlices(
            u8,
            "sys",
            source[toks.items[1].span.start..toks.items[1].span.end],
        );
        // In .dealx_outer, `>]` is NOT recognized — that's a .dealx_tag
        // job. So we get gt + r_bracket.
        try std.testing.expectEqual(lexer.Tag.gt, toks.items[2].tag);
        try std.testing.expectEqual(lexer.Tag.r_bracket, toks.items[3].tag);
        try std.testing.expectEqual(lexer.Tag.eof, toks.items[4].tag);
    }

    // ─── Case 3: "sys>]" in .dealx_tag ──────────────────────────────
    // Proves the CLOSING `>]` mode-flip: in .dealx_tag the bytes `>]`
    // collapse to a single tag_close.
    {
        const source = "sys>]";
        var toks: std.ArrayList(lexer.Token) = .empty;
        defer toks.deinit(gpa);
        try lexAll(source, .dealx_tag, &toks, gpa);

        try std.testing.expectEqual(@as(usize, 3), toks.items.len);
        try std.testing.expectEqual(lexer.Tag.ident, toks.items[0].tag);
        try std.testing.expectEqual(lexer.Tag.tag_close, toks.items[1].tag);
        try std.testing.expectEqual(lexer.Tag.eof, toks.items[2].tag);
    }

    // ─── Case 4: "sys/>]" in .dealx_tag ─────────────────────────────
    // Proves the self-close mode-flip: `/>]` is a single tag_self_close.
    {
        const source = "sys/>]";
        var toks: std.ArrayList(lexer.Token) = .empty;
        defer toks.deinit(gpa);
        try lexAll(source, .dealx_tag, &toks, gpa);

        try std.testing.expectEqual(@as(usize, 3), toks.items.len);
        try std.testing.expectEqual(lexer.Tag.ident, toks.items[0].tag);
        try std.testing.expectEqual(lexer.Tag.tag_self_close, toks.items[1].tag);
        try std.testing.expectEqual(lexer.Tag.eof, toks.items[2].tag);
    }

    // ─── Case 5: "[</sys>]" in .dealx_outer ─────────────────────────
    // Should lex as: tag_close_open, ident(sys), gt, r_bracket, eof.
    // (`>]` still doesn't fire in .dealx_outer.)
    {
        const source = "[</sys>]";
        var toks: std.ArrayList(lexer.Token) = .empty;
        defer toks.deinit(gpa);
        try lexAll(source, .dealx_outer, &toks, gpa);

        try std.testing.expectEqual(@as(usize, 5), toks.items.len);
        try std.testing.expectEqual(lexer.Tag.tag_close_open, toks.items[0].tag);
        try std.testing.expectEqual(lexer.Tag.ident, toks.items[1].tag);
        try std.testing.expectEqualSlices(
            u8,
            "sys",
            source[toks.items[1].span.start..toks.items[1].span.end],
        );
        try std.testing.expectEqual(lexer.Tag.gt, toks.items[2].tag);
        try std.testing.expectEqual(lexer.Tag.r_bracket, toks.items[3].tag);
        try std.testing.expectEqual(lexer.Tag.eof, toks.items[4].tag);
    }

    // ─── Case 6: "[</sys>]" in .deal_def ────────────────────────────
    // Should NOT recognize tag_close_open. Expect:
    //   l_bracket, lt, slash, ident(sys), gt, r_bracket, eof. (7 tokens)
    {
        const source = "[</sys>]";
        var toks: std.ArrayList(lexer.Token) = .empty;
        defer toks.deinit(gpa);
        try lexAll(source, .deal_def, &toks, gpa);

        try std.testing.expectEqual(@as(usize, 7), toks.items.len);
        try std.testing.expectEqual(lexer.Tag.l_bracket, toks.items[0].tag);
        try std.testing.expectEqual(lexer.Tag.lt, toks.items[1].tag);
        try std.testing.expectEqual(lexer.Tag.slash, toks.items[2].tag);
        try std.testing.expectEqual(lexer.Tag.ident, toks.items[3].tag);
        try std.testing.expectEqualSlices(
            u8,
            "sys",
            source[toks.items[3].span.start..toks.items[3].span.end],
        );
        try std.testing.expectEqual(lexer.Tag.gt, toks.items[4].tag);
        try std.testing.expectEqual(lexer.Tag.r_bracket, toks.items[5].tag);
        try std.testing.expectEqual(lexer.Tag.eof, toks.items[6].tag);
    }
}
