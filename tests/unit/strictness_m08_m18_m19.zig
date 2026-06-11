//! strictness.m08 / strictness.m18 / strictness.m19 — Phase 1.5 plan 02
//! strictness pin-tests for the three Phase 1 zero-diagnostic gaps surfaced
//! in 01-VALIDATION.md Validation Audit 2026-05-20.
//!
//! Each test branches on a TWO-OUTCOME disposition (per CONTEXT.md
//! REQ-phase-1.5-4 lock): the malformed file MUST either
//!   (a) produce at least one diagnostic of the documented Codes.e_* code
//!       (m08 → E0003 e_unterminated_comment EXISTING from Phase 1;
//!        m18 → E0121 e_dangling_dot NEW in 01.5-02;
//!        m19 → E0122 e_empty_arg_comma NEW in 01.5-02), OR
//!   (b) have a corresponding ADR file at
//!       `.planning/decisions/ADR-phase-1.5-<topic>.md` explaining why
//!       the input is grammatically acceptable.
//!
//! The current Phase 1.5-02 disposition for all three is (a) — the
//! implementations are in src/lexer.zig (m08), src/expr.zig dot branch
//! (m18), and src/expr.zig parseArgList (m19). The (b) branch exists so
//! a future phase that chooses to re-classify any of the three as accepted
//! can flip the disposition by creating the ADR file without rewriting
//! this test.
//!
//! Byte-offset assertions (impl branch only — skipped when ADR-only):
//!   m08 → span covers `/*` (byte 13) through EOF (byte source.len). The
//!         source is `package foo;\n/* never closed\npart def P {}\n` so
//!         `package foo;\n` is 13 bytes; `/*` starts at offset 13.
//!   m18 → span covers the `.` at byte 41 (m18 source is
//!         `package foo;\npart def P { attribute x = a.; }\n`; `a.` lives
//!         at byte 40, dot at 41).
//!   m19 → span covers the leading `,` at byte 42 (m19 source is
//!         `package foo;\npart def P { attribute x = f(,); }\n`; `(,` lives
//!         at byte 41, comma at 42).
//!
//! Byte offsets are computed at runtime via std.mem.indexOf so the tests
//! are robust to leading-whitespace edits in the malformed fixtures —
//! the byte-offset COMMENTS above are reference values, not hardcoded
//! magic numbers in the asserts.

const std = @import("std");
const ast = @import("ast");
const lib = @import("lib");
const diagnostics_mod = @import("diagnostics");

const Codes = diagnostics_mod.Codes;
const Diagnostic = diagnostics_mod.Diagnostic;

// ── Helpers (mirrored from parser_dealx_tag_balance.zig) ────────────────

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

fn adrExists(cwd: std.Io.Dir, io: std.Io, rel_path: []const u8) bool {
    _ = cwd.statFile(io, rel_path, .{}) catch return false;
    return true;
}

// ── Cases ──────────────────────────────────────────────────────────────

test "strictness.m08_unterminated_block_comment emits e_unterminated_comment" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    const source = try cwd.readFileAlloc(io, "tests/malformed/m08_unterminated_block_comment.deal", gpa, .unlimited);
    defer gpa.free(source);

    const handle = try lib.deal_parse_internal(gpa, source, "m08_unterminated_block_comment.deal");
    defer lib.deal_free_internal(handle);

    const adr_path = ".planning/decisions/ADR-phase-1.5-m08-unterminated-block-comment.md";
    const adr_present = adrExists(cwd, io, adr_path);

    const code_present = countCode(handle.diagnostics.items, Codes.e_unterminated_comment) >= 1;

    if (!code_present and !adr_present) {
        std.debug.print(
            "strictness.m08 FAILED: neither E0003 (e_unterminated_comment) emitted nor ADR at {s}\n",
            .{adr_path},
        );
        return error.TestExpectedEqual;
    }

    // Impl-branch: assert the diagnostic's span is byte-accurate. Skipped
    // when only the ADR is present (the ADR branch carries no span contract).
    if (code_present) {
        const d = firstWithCode(handle.diagnostics.items, Codes.e_unterminated_comment).?;
        // The `/*` token is the FIRST occurrence of "/*" in the source.
        const slash_star_idx_usize = std.mem.indexOf(u8, source, "/*").?;
        const slash_star_idx: u32 = @intCast(slash_star_idx_usize);
        try std.testing.expectEqual(slash_star_idx, d.span.start);
        // Span end = source.len (lexer advances pos to EOF on unterminated).
        try std.testing.expectEqual(@as(u32, @intCast(source.len)), d.span.end);
    }
}

test "strictness.m18_dangling_dot emits e_dangling_dot" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    const source = try cwd.readFileAlloc(io, "tests/malformed/m18_dangling_dot.deal", gpa, .unlimited);
    defer gpa.free(source);

    const handle = try lib.deal_parse_internal(gpa, source, "m18_dangling_dot.deal");
    defer lib.deal_free_internal(handle);

    const adr_path = ".planning/decisions/ADR-phase-1.5-m18-dangling-dot.md";
    const adr_present = adrExists(cwd, io, adr_path);

    const code_present = countCode(handle.diagnostics.items, Codes.e_dangling_dot) >= 1;

    if (!code_present and !adr_present) {
        std.debug.print(
            "strictness.m18 FAILED: neither E0121 (e_dangling_dot) emitted nor ADR at {s}\n",
            .{adr_path},
        );
        return error.TestExpectedEqual;
    }

    if (code_present) {
        const d = firstWithCode(handle.diagnostics.items, Codes.e_dangling_dot).?;
        // The dot lives at index_of("a.") + 1.
        const a_dot_idx = std.mem.indexOf(u8, source, "a.").?;
        const dot_idx: u32 = @intCast(a_dot_idx + 1);
        try std.testing.expectEqual(dot_idx, d.span.start);
        // The dot token spans a single byte.
        try std.testing.expectEqual(@as(u32, dot_idx + 1), d.span.end);
    }
}

test "strictness.m19_empty_arg_list_comma emits e_empty_arg_comma" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    const source = try cwd.readFileAlloc(io, "tests/malformed/m19_empty_arg_list_comma.deal", gpa, .unlimited);
    defer gpa.free(source);

    const handle = try lib.deal_parse_internal(gpa, source, "m19_empty_arg_list_comma.deal");
    defer lib.deal_free_internal(handle);

    const adr_path = ".planning/decisions/ADR-phase-1.5-m19-empty-arg-comma.md";
    const adr_present = adrExists(cwd, io, adr_path);

    const code_present = countCode(handle.diagnostics.items, Codes.e_empty_arg_comma) >= 1;

    if (!code_present and !adr_present) {
        std.debug.print(
            "strictness.m19 FAILED: neither E0122 (e_empty_arg_comma) emitted nor ADR at {s}\n",
            .{adr_path},
        );
        return error.TestExpectedEqual;
    }

    if (code_present) {
        const d = firstWithCode(handle.diagnostics.items, Codes.e_empty_arg_comma).?;
        // The leading comma lives at index_of("(,") + 1.
        const paren_comma_idx = std.mem.indexOf(u8, source, "(,").?;
        const comma_idx: u32 = @intCast(paren_comma_idx + 1);
        try std.testing.expectEqual(comma_idx, d.span.start);
        // The comma token spans a single byte.
        try std.testing.expectEqual(@as(u32, comma_idx + 1), d.span.end);
    }
}

test "strictness.m19_double_leading_comma_fires_e_empty_arg_comma_twice" {
    // IN-03 iteration-2 correction: parseArgList's leading-comma gate
    // now loops so `f(,,a)` fires E0122 TWICE (once per offending comma)
    // instead of once-and-silent-absorption-of-second-comma. The phantom
    // `.identifier` arg whose `name` is the bytes "," (described in the
    // 01.5-REVIEW IN-03 finding) is no longer produced.
    const gpa = std.testing.allocator;

    // Inline source — no fixture file; double-leading-comma is a
    // recovery-quality contract, not part of the m01..m20 hand-curated
    // malformed corpus. Source layout:
    //   bytes 0..12  = "package foo;"
    //   byte  13     = "\n"
    //   bytes 14..36 = "part def P { attribute"
    //   bytes 37..38 = " x"
    //   bytes 39..40 = " ="
    //   bytes 41..42 = " f"
    //   byte  43     = "("
    //   byte  44     = "," <-- first offending comma
    //   byte  45     = "," <-- second offending comma
    //   byte  46     = "a"
    //   byte  47     = ")"
    //   byte  48     = ";"
    //   byte  49     = " "
    //   byte  50     = "}"
    //   byte  51     = "\n"
    const src = "package foo;\npart def P { attribute x = f(,,a); }\n";
    const handle = try lib.deal_parse_internal(gpa, src, "m19_double_leading.deal");
    defer lib.deal_free_internal(handle);

    // Axis A: TWO E0122 diagnostics (one per comma).
    const count = countCode(handle.diagnostics.items, Codes.e_empty_arg_comma);
    if (count != 2) {
        std.debug.print(
            "strictness.m19_double_leading FAILED: expected 2x E0122, got {d}\n",
            .{count},
        );
        return error.TestExpectedEqual;
    }

    // Axis B: spans match the two comma byte positions.
    const paren_idx = std.mem.indexOf(u8, src, "(,,").?;
    const first_comma: u32 = @intCast(paren_idx + 1);
    const second_comma: u32 = @intCast(paren_idx + 2);

    var seen_first = false;
    var seen_second = false;
    for (handle.diagnostics.items) |d| {
        if (!std.mem.eql(u8, d.code, Codes.e_empty_arg_comma)) continue;
        if (d.span.start == first_comma and d.span.end == first_comma + 1) seen_first = true;
        if (d.span.start == second_comma and d.span.end == second_comma + 1) seen_second = true;
    }
    try std.testing.expect(seen_first);
    try std.testing.expect(seen_second);
}

// ── .dealx parallels (IN-04 iteration-2 correction) ───────────────────
//
// The three contracts above only exercise the .deal parser path. The
// drain helper IS wired in parser_dealx.parseDealxFile (line 224) and
// the expression parser (src/expr.zig) is SHARED by both parsers, so
// .dealx files with these shapes also fire the same diagnostic codes.
// Add three parallel tests against new .dealx fixtures so a future
// change that diverges the two parsers (e.g. dropping the drain call
// in parser_dealx, or routing dealx-expression contexts to a different
// expression parser) cannot silently break .dealx strictness.
//
// Filename suffix `.dealx` causes lib.deal_parse_internal to take the
// .dealx parser path (see src/lib.zig:110 — handle.mode = .dealx).

test "strictness.m08_dealx_unterminated_block_comment emits e_unterminated_comment" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    const source = try cwd.readFileAlloc(io, "tests/malformed/m08_unterminated_block_comment.dealx", gpa, .unlimited);
    defer gpa.free(source);

    const handle = try lib.deal_parse_internal(gpa, source, "m08_unterminated_block_comment.dealx");
    defer lib.deal_free_internal(handle);

    // No ADR fallback for the .dealx parallel — the impl is shared with .deal,
    // so the contract is impl-only here.
    const count = countCode(handle.diagnostics.items, Codes.e_unterminated_comment);
    if (count < 1) {
        std.debug.print(
            "strictness.m08_dealx FAILED: expected >= 1 E0003, got {d}\n",
            .{count},
        );
        return error.TestExpectedEqual;
    }

    const d = firstWithCode(handle.diagnostics.items, Codes.e_unterminated_comment).?;
    const slash_star_idx_usize = std.mem.indexOf(u8, source, "/*").?;
    const slash_star_idx: u32 = @intCast(slash_star_idx_usize);
    try std.testing.expectEqual(slash_star_idx, d.span.start);
    try std.testing.expectEqual(@as(u32, @intCast(source.len)), d.span.end);
}

test "strictness.m18_dealx_dangling_dot emits e_dangling_dot" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    const source = try cwd.readFileAlloc(io, "tests/malformed/m18_dangling_dot.dealx", gpa, .unlimited);
    defer gpa.free(source);

    const handle = try lib.deal_parse_internal(gpa, source, "m18_dangling_dot.dealx");
    defer lib.deal_free_internal(handle);

    const count = countCode(handle.diagnostics.items, Codes.e_dangling_dot);
    if (count < 1) {
        std.debug.print(
            "strictness.m18_dealx FAILED: expected >= 1 E0121, got {d}\n",
            .{count},
        );
        return error.TestExpectedEqual;
    }

    const d = firstWithCode(handle.diagnostics.items, Codes.e_dangling_dot).?;
    // The dangling dot lives at index_of("a.") + 1 (the source contains
    // `via={a.}`; the dot is the second byte of "a.").
    const a_dot_idx = std.mem.indexOf(u8, source, "a.").?;
    const dot_idx: u32 = @intCast(a_dot_idx + 1);
    try std.testing.expectEqual(dot_idx, d.span.start);
    try std.testing.expectEqual(@as(u32, dot_idx + 1), d.span.end);
}

test "strictness.m19_dealx_empty_arg_list_comma emits e_empty_arg_comma" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    const source = try cwd.readFileAlloc(io, "tests/malformed/m19_empty_arg_list_comma.dealx", gpa, .unlimited);
    defer gpa.free(source);

    const handle = try lib.deal_parse_internal(gpa, source, "m19_empty_arg_list_comma.dealx");
    defer lib.deal_free_internal(handle);

    const count = countCode(handle.diagnostics.items, Codes.e_empty_arg_comma);
    if (count < 1) {
        std.debug.print(
            "strictness.m19_dealx FAILED: expected >= 1 E0122, got {d}\n",
            .{count},
        );
        return error.TestExpectedEqual;
    }

    const d = firstWithCode(handle.diagnostics.items, Codes.e_empty_arg_comma).?;
    // The leading comma lives at index_of("(,") + 1 (the source contains
    // `via={f(,)}`; the comma is the second byte of "(,").
    const paren_comma_idx = std.mem.indexOf(u8, source, "(,").?;
    const comma_idx: u32 = @intCast(paren_comma_idx + 1);
    try std.testing.expectEqual(comma_idx, d.span.start);
    try std.testing.expectEqual(@as(u32, comma_idx + 1), d.span.end);
}
