//! lexer.comments — proves /** disambiguates from /*, that block + line
//! comments are SKIPPED in skipTrivia, and that unterminated doc comments
//! produce an `.unknown` token (the recovery shape Plan 05 will lift
//! into diagnostic code E0001 from the D-16 lexer range).

const std = @import("std");
const lexer = @import("lexer");

test "lexer.comments" {
    // ─── Case 1: /** doc */ → doc_comment + kw_part + ident ────────
    {
        const source = "/** doc */part X";
        var lex = lexer.Lexer.init(source);

        const t0 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.doc_comment, t0.tag);
        try std.testing.expectEqual(@as(u32, 0), t0.span.start);
        try std.testing.expectEqual(@as(u32, 10), t0.span.end);
        try std.testing.expectEqualSlices(
            u8,
            "/** doc */",
            source[t0.span.start..t0.span.end],
        );

        const t1 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.kw_part, t1.tag);

        const t2 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.ident, t2.tag);
        try std.testing.expectEqualSlices(
            u8,
            "X",
            source[t2.span.start..t2.span.end],
        );
    }

    // ─── Case 2: /* block */ → comment_block token + kw_part ───────
    // D-28 (Plan 02-01): BLOCK comments are now EMITTED as comment_block
    // tokens (not skipped). The parser's advance() helper buffers them and
    // attachComments() attaches them to declaration nodes.
    {
        const source = "/* block */part X";
        var lex = lexer.Lexer.init(source);

        const t0 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.comment_block, t0.tag);
        try std.testing.expectEqual(@as(u32, 0), t0.span.start);
        try std.testing.expectEqual(@as(u32, 11), t0.span.end);
        try std.testing.expectEqualSlices(
            u8,
            "/* block */",
            source[t0.span.start..t0.span.end],
        );

        const t1 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.kw_part, t1.tag);
        try std.testing.expectEqualSlices(
            u8,
            "part",
            source[t1.span.start..t1.span.end],
        );

        const t2 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.ident, t2.tag);
        try std.testing.expectEqualSlices(
            u8,
            "X",
            source[t2.span.start..t2.span.end],
        );
    }

    // ─── Case 3: // line comment → comment_line token + kw_part ────
    // D-28 (Plan 02-01): LINE comments are now EMITTED as comment_line
    // tokens (including the trailing newline in the span).
    {
        const source = "// hello\npart X";
        var lex = lexer.Lexer.init(source);

        const t0 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.comment_line, t0.tag);
        try std.testing.expectEqual(@as(u32, 0), t0.span.start);
        // span includes the trailing newline
        try std.testing.expectEqual(@as(u32, 9), t0.span.end);
        try std.testing.expectEqualSlices(
            u8,
            "// hello\n",
            source[t0.span.start..t0.span.end],
        );

        const t1 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.kw_part, t1.tag);
        try std.testing.expectEqualSlices(
            u8,
            "part",
            source[t1.span.start..t1.span.end],
        );
    }

    // ─── Case 4: /** unterminated → unknown spanning /** to EOF ────
    // This is the recovery shape that Plan 05 will translate into a
    // diagnostic carrying error code E0001 (lexer range from D-16:
    // E0001..E0099). The lexer itself only emits the .unknown token;
    // the diagnostic emission happens in the parser frame.
    {
        const source = "/** unterminated";
        var lex = lexer.Lexer.init(source);

        const t0 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.unknown, t0.tag);
        try std.testing.expectEqual(@as(u32, 0), t0.span.start);
        try std.testing.expectEqual(@as(u32, @intCast(source.len)), t0.span.end);

        const t1 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.eof, t1.tag);
    }

    // ─── Case 5: /** with stars in body */ — body containing `*` ───
    // The scanner stops at the first `*/` regardless of how many stars
    // precede it. Matches lexical.ebnf §2 DOC_COMMENT production.
    {
        const source = "/** a * b ** c */";
        var lex = lexer.Lexer.init(source);

        const t0 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.doc_comment, t0.tag);
        try std.testing.expectEqual(@as(u32, 0), t0.span.start);
        try std.testing.expectEqual(@as(u32, @intCast(source.len)), t0.span.end);
    }
}
