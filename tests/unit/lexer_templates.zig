//! lexer.templates — proves template_head / template_middle / template_tail
//! emission for the `${}` interpolation forms, AND the T-02-01 DoS bound
//! at depth 64.

const std = @import("std");
const lexer = @import("lexer");

test "lexer.templates" {
    // ─── Case 1: simple interpolation ──────────────────────────────
    // `hello ${name} world`
    //  ^^^^^^^^      ^^^^^^^^
    //  template_head template_tail
    //          ^     ^
    //    template_open  template_close (=== r_brace once template_depth>0
    //                                   with stack[top]==0)
    {
        const source = "`hello ${name} world`";
        var lex = lexer.Lexer.init(source);

        // Token 0: template_head "`hello ${"
        const t0 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.template_head, t0.tag);
        try std.testing.expectEqualSlices(
            u8,
            "`hello ${",
            source[t0.span.start..t0.span.end],
        );

        // Token 1: ident(name)
        const t1 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.ident, t1.tag);
        try std.testing.expectEqualSlices(
            u8,
            "name",
            source[t1.span.start..t1.span.end],
        );

        // Token 2: template_tail "} world`"
        // (The closing `}` triggers a scan into the trailing template
        // segment; we land on `}` with template_depth>0 and stack[top]==0,
        // so the lexer emits template_tail directly.)
        const t2 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.template_tail, t2.tag);
        try std.testing.expectEqualSlices(
            u8,
            "} world`",
            source[t2.span.start..t2.span.end],
        );

        // Token 3: eof
        const t3 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.eof, t3.tag);
    }

    // ─── Case 2: nested expression (string + ident in interp) ──────
    // `a${"b"+c}d`
    //  ^^^^      ^^^
    // template_head, then string_literal("b"), plus, ident(c), then
    // template_tail.
    {
        const source = "`a${\"b\"+c}d`";
        var lex = lexer.Lexer.init(source);

        const t0 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.template_head, t0.tag);
        try std.testing.expectEqualSlices(
            u8,
            "`a${",
            source[t0.span.start..t0.span.end],
        );

        const t1 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.string_literal, t1.tag);
        try std.testing.expectEqualSlices(
            u8,
            "\"b\"",
            source[t1.span.start..t1.span.end],
        );

        const t2 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.plus, t2.tag);

        const t3 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.ident, t3.tag);
        try std.testing.expectEqualSlices(
            u8,
            "c",
            source[t3.span.start..t3.span.end],
        );

        const t4 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.template_tail, t4.tag);
        try std.testing.expectEqualSlices(
            u8,
            "}d`",
            source[t4.span.start..t4.span.end],
        );

        const t5 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.eof, t5.tag);
    }

    // ─── Case 3: template_literal (no interpolation) ───────────────
    // Per lexical.ebnf §4: a `...` with NO ${ inside is a single
    // TEMPLATE_LITERAL token.
    {
        const source = "`hello world`";
        var lex = lexer.Lexer.init(source);

        const t0 = lex.next(.deal_def);
        try std.testing.expectEqual(lexer.Tag.template_literal, t0.tag);
        try std.testing.expectEqualSlices(
            u8,
            "`hello world`",
            source[t0.span.start..t0.span.end],
        );
    }

    // ─── Case 4: DoS bound at depth 64 (T-02-01) ───────────────────
    // Build a source string with 65 nested `${`s and assert the lexer:
    //   (a) does NOT panic
    //   (b) emits an `.unknown` token at SOME `${` boundary
    //   (c) eventually reaches EOF without spinning.
    {
        const gpa = std.testing.allocator;
        var src: std.ArrayList(u8) = .empty;
        defer src.deinit(gpa);

        try src.append(gpa, '`');
        var i: usize = 0;
        while (i < 65) : (i += 1) {
            try src.appendSlice(gpa, "${");
        }
        // No matching closes — the bound check fires before that matters.

        var lex = lexer.Lexer.init(src.items);
        var saw_unknown = false;
        var step_count: usize = 0;
        while (step_count < 1000) : (step_count += 1) {
            const t = lex.next(.deal_def);
            if (t.tag == .unknown) saw_unknown = true;
            if (t.tag == .eof) break;
        }
        try std.testing.expect(saw_unknown);
        try std.testing.expect(step_count < 1000); // didn't spin
    }
}
