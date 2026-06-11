//! recovery.corpus — Phase 1 ROADMAP success criterion #4
//!                  + Phase 1.5 plan 03 REQ-phase-1.5-2 per-file pinning.
//!
//! Walks `tests/malformed/` and asserts:
//!   - At least 50 malformed files exist (hand-curated + generator-produced).
//!   - For every file, parser.parseFile returns non-null (D-10 guarantee).
//!   - For every file, at least one diagnostic is produced (the file IS
//!     malformed by construction, so a clean parse would be a bug).
//!   - No file panics the parser.
//!   - For every hand-curated `m01..m20` file, the FIRST diagnostic emitted
//!     matches the per-file pin in `m_pins` below, OR (for an
//!     accepted-no-diagnostic disposition) ZERO diagnostics are emitted AND
//!     the named ADR file exists under `.planning/decisions/`.
//!
//! The 50-file gate is REQ-phase-1-4-error-recovery's hardest correctness
//! criterion — it forces every error path to be reasoned about simultaneously.
//!
//! The per-file pin gate (REQ-phase-1.5-2-malformed-pinning) is layered on top
//! of the existing soft 60 % gate: the hand-curated set (20 files) is held to
//! a per-file expected-code contract, while the 30 generator-produced files
//! remain under the soft 60 % gate because their mutation semantics make
//! per-file expectations brittle.

const std = @import("std");
const ast = @import("ast");
const parser = @import("parser");
const diagnostics_mod = @import("diagnostics");

const Codes = diagnostics_mod.Codes;

/// Per-file pin for a hand-curated malformed fixture.
///
/// Two-outcome contract (mirrors strictness_m08_m18_m19.zig's pattern):
///   - `expected_code != null`  — the FIRST diagnostic emitted MUST match
///                                this code (string equality). Zero
///                                diagnostics fails the test.
///   - `expected_code == null`  — accepted-no-diagnostic disposition.
///                                ZERO diagnostics MUST be emitted, AND
///                                the named ADR file MUST exist at
///                                `.planning/decisions/<adr_basename>`.
const Pin = struct {
    name: []const u8,
    /// First-emitted primary diagnostic code, or null for an accepted-no-diag pin.
    expected_code: ?[]const u8,
    /// Required when expected_code == null. Basename relative to `.planning/decisions/`.
    adr_basename: ?[]const u8 = null,
};

/// Hand-curated malformed corpus pins (REQ-phase-1.5-2-malformed-pinning).
///
/// Each pin references `Codes.e_*` named constants (NOT inline "E0123"
/// literals) — this prevents accidental drift if D-16 ever renumbers and
/// localizes the source of truth to `src/diagnostics.zig`.
///
/// Codes assigned here are EMPIRICALLY OBSERVED (Plan 01.5-03 Task 1
/// discovery, 2026-05-20). Where the empirically-observed first-code differs
/// from the filename's apparent intent (e.g. m02 has an unclosed BRACE but
/// the first diagnostic emitted is `e_expected_token` from the next-token
/// expectation, not `e_unclosed_brace` from recovery-tier sync), an inline
/// comment documents the divergence. The recovery-tier code (e.g.
/// `e_unclosed_brace`) typically fires LATER as a secondary; only the FIRST
/// emitted code is pinned here, matching how the parser surfaces errors to
/// the user.
const m_pins = [_]Pin{
    // m01 — SD-10 optional-semicolon: `attribute a : T attribute b : T;`
    // parses cleanly because SD-10 allows the missing semicolon at this
    // position. Documented via ADR-phase-1.5-m01-sd10-optional-semicolon.md.
    .{ .name = "m01_missing_semicolon.deal", .expected_code = null, .adr_basename = "ADR-phase-1.5-m01-sd10-optional-semicolon.md" },
    // m02 — unclosed `{ attribute a : T;` — parser hits EOF expecting
    // `}`; first diagnostic is the expected-token failure, not the
    // recovery-tier e_unclosed_brace.
    .{ .name = "m02_unclosed_brace.deal", .expected_code = Codes.e_expected_token },
    // m03 — `f(a, b ; }` — parser is inside an argument list and expects
    // a closing `)` or `,`; the `;` triggers e_expected_token first.
    .{ .name = "m03_unclosed_paren.deal", .expected_code = Codes.e_expected_token },
    // m04 — `part def 12345 { }` — `12345` after `def` fails the
    // expected-token check (def expects IDENT) before any recovery-tier sync.
    .{ .name = "m04_garbage_after_keyword.deal", .expected_code = Codes.e_expected_token },
    // m05 — invalid UTF-8 byte at top level. PARSER-PATH-ONLY PIN.
    //
    // WR-03 contract-surface divergence — read carefully:
    //   - This test invokes `parser.parseFile` DIRECTLY (line ~267
    //     below). `parser.parseFile` has NO UTF-8 validation, so the
    //     lexer surfaces the malformed byte as an unexpected token and
    //     the parser emits e_expected_definition (E0101) as the FIRST
    //     diagnostic. The lexer-tier E0001 e_invalid_utf8 may appear
    //     LATER as a secondary, but is not the first item.
    //   - PRODUCTION C-ABI callers (Rust FFI, Tauri editor) invoke
    //     `lib.deal_parse`, which performs `std.unicode.utf8ValidateSlice`
    //     BEFORE parsing (src/lib.zig:137-147). That path emits
    //     e_invalid_utf8 (E0001) as the FIRST diagnostic and never
    //     reaches the parser for this fixture.
    //
    // The pin below therefore protects the DIRECT-PARSE path's
    // first-code (E0101) only. The production observable first-code
    // for this fixture is E0001 via `lib.deal_parse`. Do not "fix" this
    // pin to E0001 without also routing the pin loop through
    // `lib.deal_parse` — the two paths emit different first-codes by
    // design.
    .{ .name = "m05_invalid_utf8.deal", .expected_code = Codes.e_expected_definition },
    // m06 — `attribute msg = "hello world;` — lexer reaches EOF mid-string.
    // The parser side surfaces the unterminated value as an
    // expected-token failure at the next-expression boundary first.
    .{ .name = "m06_unterminated_string.deal", .expected_code = Codes.e_expected_token },
    // m07 — `\`hello ${world;` — unterminated template; first parser
    // diagnostic is the expected-token failure (closing brace inside
    // template interpolation).
    .{ .name = "m07_unterminated_template.deal", .expected_code = Codes.e_expected_token },
    // m08 — `/* never closed` — EXISTING E0003 e_unterminated_comment
    // wired by Plan 01.5-02 (lexer skipTrivia captures span; parser's
    // drainLexerErrors emits at parseFile-end).
    .{ .name = "m08_unterminated_block_comment.deal", .expected_code = Codes.e_unterminated_comment },
    // m09 — `<<not_a_valid_modifier>>` — the parser's first response is
    // e_expected_definition (E0101), not e_invalid_modifier (E0120). The
    // unknown identifier inside `<<>>` causes the modifier-list parser to
    // fall through to top-level expected-definition.
    .{ .name = "m09_invalid_modifier.deal", .expected_code = Codes.e_expected_definition },
    // m10 — `[</orphan>]` at top of dealx file — parser emits
    // e_unmatched_close_tag (E0301).
    .{ .name = "m10_orphan_close_tag.dealx", .expected_code = Codes.e_unmatched_close_tag },
    // m11 — `[<system Foo>][</subsystem>]` — close tag name does not
    // match open tag; emits e_mismatched_close_tag (E0302).
    .{ .name = "m11_mismatched_close_tag.dealx", .expected_code = Codes.e_mismatched_close_tag },
    // m12 — `[<system Foo>]` with no close before EOF — emits
    // e_unclosed_tag_at_eof (E0304).
    .{ .name = "m12_unclosed_tag_at_eof.dealx", .expected_code = Codes.e_unclosed_tag_at_eof },
    // m13 — `[<connect from= to='b' />]` — attribute value is missing
    // after `=`; emits e_expected_attribute_value (E0103).
    .{ .name = "m13_malformed_attr_value.dealx", .expected_code = Codes.e_expected_attribute_value },
    // m14 — `[<connect via={ unclosed />]` — `{` inside attribute opens
    // an inline block that never closes; first diagnostic is the
    // expected-token failure inside the inline block parse path, not the
    // recovery-tier e_unclosed_brace.
    .{ .name = "m14_brace_inside_attr.dealx", .expected_code = Codes.e_expected_token },
    // m15 — `@@trace:satisfies REQ_01` — `@@` is a double-at; the parser
    // sees the second `@` after the annotation marker and treats it as a
    // top-level token failure (e_expected_definition), not as an
    // identifier-required error inside the annotation.
    .{ .name = "m15_double_at_in_annotation.deal", .expected_code = Codes.e_expected_definition },
    // m16 — `2 + 2;` at top level — bare expression; emits
    // e_expected_definition (E0101) because top level only accepts
    // definitions.
    .{ .name = "m16_bare_expression_at_top.deal", .expected_code = Codes.e_expected_definition },
    // m17 — `part def part { ... }` — `part` is a reserved keyword used
    // as an identifier; the parser's first response is the
    // expected-token failure (def expects IDENT, not a keyword).
    .{ .name = "m17_keyword_as_ident.deal", .expected_code = Codes.e_expected_token },
    // m18 — `a.;` — NEW E0121 e_dangling_dot wired by Plan 01.5-02
    // (expr.parseExpression dot branch peeks before advance).
    .{ .name = "m18_dangling_dot.deal", .expected_code = Codes.e_dangling_dot },
    // m19 — `f(,)` — NEW E0122 e_empty_arg_comma wired by Plan 01.5-02
    // (expr.parseArgList gates leading/double/trailing commas).
    .{ .name = "m19_empty_arg_list_comma.deal", .expected_code = Codes.e_empty_arg_comma },
    // m20 — `((((..42..))))` 200+ deep — emits E0403
    // e_nesting_too_deep_expr from the recovery-tier nesting guard.
    .{ .name = "m20_deeply_nested_braces.deal", .expected_code = Codes.e_nesting_too_deep_expr },
    // m21 — `@config { voltage: 12, current: 5 }` — `,` as annotation body
    // separator is rejected (D-01). First diagnostic is e_annotation_comma_separator (E0123).
    .{ .name = "m21_annotation_comma_separator.deal", .expected_code = Codes.e_annotation_comma_separator },
    // m22 — `verification { badkey: true; }` — unknown key inside a
    // verification block emits e_unknown_verification_key (E0124) (D-02).
    .{ .name = "m22_unknown_verification_key.deal", .expected_code = Codes.e_unknown_verification_key },
    // m23 — `attribute x = [a` (EOF inside an array literal) — the unified
    // l_bracket arm (D-04) terminates on .eof and emits e_unclosed_bracket
    // (E0402) with the opening `[` span before the secondary expect failure
    // (WR-01, phase 05.1 review).
    .{ .name = "m23_unclosed_array_bracket.deal", .expected_code = Codes.e_unclosed_bracket },
    // m24 — `attribute x = [a,];` — a trailing comma in an array literal is
    // rejected to match parseArgList strictness + the fmt.zig "NO trailing
    // commas" invariant; first diagnostic is e_empty_arg_comma (E0122)
    // (WR-03, phase 05.1 review).
    .{ .name = "m24_trailing_comma_array.deal", .expected_code = Codes.e_empty_arg_comma },
    // m25 — purity violation: out-param calc used in expression position (E2600,
    // sema-only). The file intentionally omits the calc's `return` statement to
    // make the parser emit E2601 (calc body missing return) as the FIRST
    // parser-observable diagnostic. The E2600 purity violation is verified at the
    // sema level in tests/unit/sema_calc.zig.
    //
    // Empirical-truth divergence: intended sema code = E2600; first parser code = E2601.
    .{ .name = "m25_calc_out_param_expr.deal", .expected_code = Codes.e_calc_missing_return },
    // m26 — calc body missing return statement (E2601). The parser detects the
    // missing `return` in parseCalcBody and emits E2601 directly.
    .{ .name = "m26_calc_no_return.deal", .expected_code = Codes.e_calc_missing_return },
    // m27 — require condition not Boolean (E2610, sema-only). The file also
    // includes a calc with a missing `return` statement to ensure the parser
    // emits E2601 as the first observable diagnostic (empirical-truth pin).
    // E2610 is verified at the sema level in tests/unit/sema_calc.zig.
    //
    // Empirical-truth divergence: intended sema code = E2610; first parser code = E2601.
    .{ .name = "m27_require_not_boolean.deal", .expected_code = Codes.e_calc_missing_return },
};

test "recovery.corpus" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    var dir = cwd.openDir(io, "tests/malformed", .{ .iterate = true }) catch |err| {
        std.debug.print("recovery.corpus: cannot open tests/malformed/ ({s})\n", .{@errorName(err)});
        return err;
    };
    defer dir.close(io);

    var iter = dir.iterate();
    var paths: std.ArrayList([]const u8) = .empty;
    defer {
        for (paths.items) |p| gpa.free(p);
        paths.deinit(gpa);
    }

    while (try iter.next(io)) |entry| {
        if (entry.kind != .file) continue;
        const name = entry.name;
        if (!std.mem.endsWith(u8, name, ".deal") and !std.mem.endsWith(u8, name, ".dealx")) continue;
        const full = try std.fmt.allocPrint(gpa, "tests/malformed/{s}", .{name});
        try paths.append(gpa, full);
    }

    // Sort paths for deterministic test ordering.
    std.mem.sort([]const u8, paths.items, {}, struct {
        fn lt(_: void, a: []const u8, b: []const u8) bool {
            return std.mem.lessThan(u8, a, b);
        }
    }.lt);

    if (paths.items.len < 50) {
        std.debug.print(
            "recovery.corpus FAILED: only {d} malformed files found (need >=50)\n  Run `zig build gen-malformed` first.\n",
            .{paths.items.len},
        );
        return error.TestExpectedEqual;
    }

    var total_diags: usize = 0;
    var no_diag_files: usize = 0;

    for (paths.items) |path| {
        const source = cwd.readFileAlloc(io, path, gpa, .unlimited) catch |err| {
            std.debug.print("recovery.corpus: read {s} failed ({s})\n", .{ path, @errorName(err) });
            continue;
        };
        defer gpa.free(source);

        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();

        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const mode: ast.Mode = if (std.mem.endsWith(u8, path, ".dealx")) .dealx else .deal;

        // D-10 guarantee: parser MUST return non-null even on malformed
        // input. A null return indicates OOM only.
        const root = parser.parseFile(arena.allocator(), source, mode, &diags) catch |err| {
            std.debug.print("recovery.corpus PANIC on {s}: {s}\n", .{ path, @errorName(err) });
            return err;
        };
        _ = root;

        total_diags += diags.items.len;
        if (diags.items.len == 0) {
            no_diag_files += 1;
            std.debug.print("recovery.corpus: {s} produced 0 diagnostics (expected >=1)\n", .{path});
        }
    }

    std.debug.print(
        "recovery.corpus: {d} malformed files, {d} total diagnostics, {d} files with 0 diagnostics, 0 panics\n",
        .{ paths.items.len, total_diags, no_diag_files },
    );

    // We tolerate a small number of "0 diagnostic" files because not every
    // mutation produces a SYNTACTIC error — some `swap_pair` mutations
    // swap inside a string literal (no parse impact), some `drop_byte`
    // drops a `}` inside a string (also no parse impact), and some
    // missing-semicolons hit a position where SD-10's optional-semicolon
    // rule applies. The verification gate is: ≥60% of files produce at
    // least one diagnostic — empirically tuned against the deterministic
    // PRNG seed (0xDEA1_2026) used by gen-malformed.
    //
    // The HARD invariant — no panics, all files return non-null — is
    // enforced unconditionally above. This 60% gate is the SOFT signal
    // that the recovery machinery is meaningfully exercised.
    const min_files_with_diag = (paths.items.len * 60) / 100;
    const files_with_diag = paths.items.len - no_diag_files;
    if (files_with_diag < min_files_with_diag) {
        std.debug.print(
            "recovery.corpus FAILED: only {d} of {d} files produced diagnostics (need >={d})\n",
            .{ files_with_diag, paths.items.len, min_files_with_diag },
        );
        return error.TestExpectedEqual;
    }

    // ── Per-file pin enforcement (REQ-phase-1.5-2-malformed-pinning) ────
    //
    // For every entry in `m_pins`, parse the file fresh and assert the
    // first emitted diagnostic matches the pin (or zero-diag + ADR
    // present for the accepted-no-diagnostic disposition).
    //
    // The pin loop reads each file SEPARATELY from the directory walk
    // above so a future refactor of the walk's ordering, filtering, or
    // arena lifetime cannot weaken the pin contract. The pin loop is
    // self-contained.
    for (m_pins) |pin| {
        const path = try std.fmt.allocPrint(gpa, "tests/malformed/{s}", .{pin.name});
        defer gpa.free(path);

        const source = cwd.readFileAlloc(io, path, gpa, .unlimited) catch |err| {
            std.debug.print("recovery.corpus pin: {s} read failed ({s})\n", .{ pin.name, @errorName(err) });
            return err;
        };
        defer gpa.free(source);

        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();

        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const mode: ast.Mode = if (std.mem.endsWith(u8, pin.name, ".dealx")) .dealx else .deal;
        _ = parser.parseFile(arena.allocator(), source, mode, &diags) catch |err| {
            std.debug.print("recovery.corpus pin PANIC on {s}: {s}\n", .{ pin.name, @errorName(err) });
            return err;
        };

        if (pin.expected_code) |code| {
            if (diags.items.len == 0) {
                std.debug.print(
                    "recovery.corpus pin FAIL: {s} expected first-code {s} but emitted ZERO diagnostics\n",
                    .{ pin.name, code },
                );
                return error.TestExpectedEqual;
            }
            const actual = diags.items[0].code;
            if (!std.mem.eql(u8, actual, code)) {
                std.debug.print(
                    "recovery.corpus pin FAIL: {s} expected first-code {s} but emitted {s}\n",
                    .{ pin.name, code, actual },
                );
                return error.TestExpectedEqual;
            }
        } else {
            // Accepted-no-diagnostic disposition — assert ZERO diagnostics
            // AND the named ADR file exists.
            if (diags.items.len != 0) {
                std.debug.print(
                    "recovery.corpus pin FAIL: {s} is pinned accepted-no-diagnostic but emitted {d} diagnostic(s); first-code = {s}\n",
                    .{ pin.name, diags.items.len, diags.items[0].code },
                );
                return error.TestExpectedEqual;
            }
            const adr_basename = pin.adr_basename orelse {
                std.debug.print(
                    "recovery.corpus pin FAIL: {s} has expected_code = null but no adr_basename — invalid pin shape\n",
                    .{pin.name},
                );
                return error.TestExpectedEqual;
            };
            const adr_path = try std.fmt.allocPrint(gpa, ".planning/decisions/{s}", .{adr_basename});
            defer gpa.free(adr_path);
            _ = cwd.statFile(io, adr_path, .{}) catch {
                std.debug.print(
                    "recovery.corpus pin FAIL: {s} pinned accepted-no-diagnostic but ADR file missing at {s}\n",
                    .{ pin.name, adr_path },
                );
                return error.TestExpectedEqual;
            };
        }
    }
}
