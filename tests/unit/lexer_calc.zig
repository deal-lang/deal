//! lexer_calc — Phase 05.2 Wave 1 token-sequence tests.
//!
//! Verifies:
//!   1. The ± character (UTF-8: 0xC2 0xB1) lexes as a single .plus_minus token
//!      spanning exactly 2 bytes (T-05.2-W1-01 bounds-safe dispatch guard).
//!   2. A minimal `calc def F(x : Real) : Real { return x; }` lexes with
//!      kw_calc/kw_def leading the stream and kw_return present, with ZERO
//!      .unknown tokens (acceptance criteria #2).
//!   3. kw_return and kw_require lex as keywords; sig stays .ident (D-07).

const std = @import("std");
const lexer = @import("lexer");

test "lexer_calc.plus_minus — ± (0xC2 0xB1) is a single .plus_minus token" {
    // ± is U+00B1, encoded as UTF-8: 0xC2 0xB1.
    const pm: []const u8 = "±"; // contains exactly bytes 0xC2 0xB1
    try std.testing.expectEqual(@as(usize, 2), pm.len);
    try std.testing.expectEqual(@as(u8, 0xC2), pm[0]);
    try std.testing.expectEqual(@as(u8, 0xB1), pm[1]);

    var lex = lexer.Lexer.init(pm);
    const t0 = lex.next(.deal_def);
    // Must be exactly one .plus_minus token, NOT two .unknown tokens.
    try std.testing.expectEqual(lexer.Tag.plus_minus, t0.tag);
    try std.testing.expectEqual(@as(u32, 0), t0.span.start);
    try std.testing.expectEqual(@as(u32, 2), t0.span.end);

    // Next token is EOF.
    const t1 = lex.next(.deal_def);
    try std.testing.expectEqual(lexer.Tag.eof, t1.tag);
}

test "lexer_calc.plus_minus — truncated 0xC2 at end-of-buffer is safe" {
    // Truncated ± (only the leading byte 0xC2, no following 0xB1).
    // Must NOT panic or over-read — must emit .unknown for the 1-byte scalar.
    const truncated: []const u8 = &[_]u8{0xC2};
    var lex = lexer.Lexer.init(truncated);
    const t0 = lex.next(.deal_def);
    // The 0xC2 without a valid continuation is NOT ±; must fall through to
    // the generic UTF-8 handler (unknown or single-byte unknown — either
    // is acceptable; must not be .plus_minus and must not panic).
    try std.testing.expect(t0.tag != .plus_minus);
    // Must advance (no infinite loop).
    try std.testing.expect(t0.span.end > t0.span.start);
}

test "lexer_calc.plus_minus — 0xC2 followed by wrong byte is not plus_minus" {
    // 0xC2 followed by 0xB2 (not 0xB1) must NOT emit .plus_minus.
    const not_pm: []const u8 = &[_]u8{ 0xC2, 0xB2 };
    var lex = lexer.Lexer.init(not_pm);
    const t0 = lex.next(.deal_def);
    try std.testing.expect(t0.tag != .plus_minus);
}

test "lexer_calc.minimal_calc_def — kw_calc/kw_def lead, kw_return present, zero .unknown" {
    // Minimal well-formed calc def (SD-21 shape).
    const src = "calc def F(x : Real) : Real { return x; }";

    var lex = lexer.Lexer.init(src);

    // First token: kw_calc
    const t0 = lex.next(.deal_def);
    try std.testing.expectEqual(lexer.Tag.kw_calc, t0.tag);

    // Second token: kw_def
    const t1 = lex.next(.deal_def);
    try std.testing.expectEqual(lexer.Tag.kw_def, t1.tag);

    // Third token: ident "F"
    const t2 = lex.next(.deal_def);
    try std.testing.expectEqual(lexer.Tag.ident, t2.tag);

    // Drain the remainder, track unknown count and whether kw_return appeared.
    var saw_kw_return = false;
    var unknown_count: usize = 0;
    var tok = lex.next(.deal_def);
    while (tok.tag != .eof) : (tok = lex.next(.deal_def)) {
        if (tok.tag == .kw_return) saw_kw_return = true;
        if (tok.tag == .unknown) unknown_count += 1;
    }

    try std.testing.expect(saw_kw_return);
    try std.testing.expectEqual(@as(usize, 0), unknown_count);
}

test "lexer_calc.return_keyword — 'return' lexes as kw_return" {
    var lex = lexer.Lexer.init("return");
    const t = lex.next(.deal_def);
    try std.testing.expectEqual(lexer.Tag.kw_return, t.tag);
}

test "lexer_calc.require_keyword — 'require' lexes as kw_require" {
    var lex = lexer.Lexer.init("require");
    const t = lex.next(.deal_def);
    try std.testing.expectEqual(lexer.Tag.kw_require, t.tag);
}

test "lexer_calc.sig_stays_ident — D-07: 'sig' is NOT a global keyword" {
    // D-07: sig is contextual-only; the lexer must emit .ident for it.
    var lex = lexer.Lexer.init("sig");
    const t = lex.next(.deal_def);
    try std.testing.expectEqual(lexer.Tag.ident, t.tag);
}
