//! fmt_roundtrip — Round-trip correctness tests for `deal fmt` (Plan 02-05).
//!
//! For each of the 19 showcase files, verifies two properties:
//!
//! 1. **Idempotency**: formatting twice gives the same bytes as formatting once.
//!    format(source) → bytes1; format(format(source)) → bytes2; assert bytes1 == bytes2.
//!    This proves the formatter is stable: it does not keep changing the output on
//!    repeated application ("parse → format → re-parse → format == first format").
//!
//! 2. **Comment preservation**: the number of comment entries in AST JSON is the same
//!    after formatting. Ensures comments are not dropped by the formatter.
//!
//! Why idempotency instead of "parse(source).ast_json == parse(formatted).ast_json"?
//! AST JSON includes source spans (byte offsets). After whitespace normalization the
//! spans of re-parsed nodes differ from the original spans — so a direct AST JSON
//! comparison would always diverge for files that aren't already in canonical form.
//! Idempotency is the correct semantic: the formatter produces a stable canonical
//! form, not an identity function over the original bytes.
//!
//! Uses the PUBLIC C ABI (deal_parse / deal_format / deal_ast_json / deal_free)
//! per CONTEXT.md §"Determinism test": "round-trip must hold at the contract
//! surface."
//!
//! CRITICAL — Pitfall 3 (T-02-29): formatted bytes and AST JSON bytes are
//! arena-owned by the RESPECTIVE handles. Clone BEFORE free:
//!   - Clone formatted bytes from handle_a BEFORE deal_free(handle_a).
//!   - Clone formatted2 bytes from handle_b BEFORE deal_free(handle_b).

const std = @import("std");
const lib = @import("lib");

/// Count occurrences of the D-29 comment attachment markers in an AST JSON
/// string. Returns the number of leading/trailing comment entries
/// (which appear as `"kind":"line"` or `"kind":"block"` inside the JSON
/// comment arrays).
fn countCommentEntries(json_bytes: []const u8) usize {
    var count: usize = 0;
    // Count "kind":"line" and "kind":"block" — each appears once per comment.
    var idx: usize = 0;
    while (idx < json_bytes.len) {
        if (std.mem.startsWith(u8, json_bytes[idx..], "\"kind\":\"line\"") or
            std.mem.startsWith(u8, json_bytes[idx..], "\"kind\":\"block\""))
        {
            count += 1;
            idx += 12;
        } else if (std.mem.startsWith(u8, json_bytes[idx..], "\"doc_comment\":{")) {
            // doc_comment object present → count it as one doc comment.
            count += 1;
            idx += 15;
        } else {
            idx += 1;
        }
    }
    return count;
}

test "fmt_roundtrip.showcase_battery" {
    try runRoundTripTest("tests/showcase/packages/vehicle/battery.deal");
}

test "fmt_roundtrip.showcase_motor" {
    try runRoundTripTest("tests/showcase/packages/vehicle/motor.deal");
}

test "fmt_roundtrip.showcase_behaviors" {
    try runRoundTripTest("tests/showcase/packages/vehicle/behaviors.deal");
}

test "fmt_roundtrip.showcase_components" {
    try runRoundTripTest("tests/showcase/packages/vehicle/components.deal");
}

test "fmt_roundtrip.showcase_vehicle_index" {
    try runRoundTripTest("tests/showcase/packages/vehicle/index.deal");
}

test "fmt_roundtrip.showcase_electrical" {
    try runRoundTripTest("tests/showcase/packages/interfaces/electrical.deal");
}

test "fmt_roundtrip.showcase_thermal" {
    try runRoundTripTest("tests/showcase/packages/interfaces/thermal.deal");
}

test "fmt_roundtrip.showcase_connections" {
    try runRoundTripTest("tests/showcase/packages/interfaces/connections.deal");
}

test "fmt_roundtrip.showcase_interfaces_index" {
    try runRoundTripTest("tests/showcase/packages/interfaces/index.deal");
}

test "fmt_roundtrip.showcase_system_req" {
    try runRoundTripTest("tests/showcase/packages/requirements/system.deal");
}

test "fmt_roundtrip.showcase_needs" {
    try runRoundTripTest("tests/showcase/packages/requirements/needs.deal");
}

test "fmt_roundtrip.showcase_requirements_index" {
    try runRoundTripTest("tests/showcase/packages/requirements/index.deal");
}

test "fmt_roundtrip.showcase_driving" {
    try runRoundTripTest("tests/showcase/packages/use-cases/driving.deal");
}

test "fmt_roundtrip.showcase_charging" {
    try runRoundTripTest("tests/showcase/packages/use-cases/charging.deal");
}

test "fmt_roundtrip.showcase_use_cases_index" {
    try runRoundTripTest("tests/showcase/packages/use-cases/index.deal");
}

test "fmt_roundtrip.showcase_traceability" {
    try runRoundTripTest("tests/showcase/model/traceability.dealx");
}

test "fmt_roundtrip.showcase_vehicle_dealx" {
    try runRoundTripTest("tests/showcase/model/vehicle.dealx");
}

test "fmt_roundtrip.showcase_performance" {
    try runRoundTripTest("tests/showcase/model/variants/performance.dealx");
}

test "fmt_roundtrip.showcase_sedan" {
    try runRoundTripTest("tests/showcase/model/variants/sedan.dealx");
}

// ─── Phase 05.2 Wave 3: calc/constraint precision attach-point tests ──────────
//
// D-06/D-09: `+/-` (ASCII alias) MUST be normalized to `±` (U+00B1) by the
// formatter at ALL THREE precision attach points:
//   (1) calc return contract   — `calc def Foo(...) : T => +/- percent(1) { ... }`
//   (2) attribute-value suffix — `attribute x : T = expr => +/- 0.01;`
//   (3) require threshold      — `require x <= y => +/- 0.01;`
// The oracle files (calcs.deal, precision.deal) already use canonical `±` form —
// idempotency tests verify the formatter does not disturb them.

test "fmt_roundtrip.analysis_calcs" {
    try runRoundTripTest("tests/showcase/packages/analysis/calcs.deal");
}

test "fmt_roundtrip.analysis_precision" {
    try runRoundTripTest("tests/showcase/packages/analysis/precision.deal");
}

// Verify `+/-` ASCII alias normalizes to `±` at ALL THREE precision attach
// points in a single fmt pass (D-06/D-09 normalization gate).
//
// Source uses the three-character `+/-` alias at:
//   (1) calc return contract
//   (2) attribute-value precision suffix
//   (3) require-threshold precision suffix
//
// After one format pass the output MUST contain `±` at each site and must
// NOT contain the raw `+/-` sequence.
test "fmt_roundtrip.plus_minus_normalization_all_attach_points" {
    const gpa = std.testing.allocator;

    // Source uses `+/-` at all 3 attach points (ASCII alias, D-09).
    const source =
        \\package test.precision;
        \\import deal.std.units.{Mass, Pressure, kg, Pa, percent};
        \\calc def Approx(a : Pressure, b : Pressure) : Real => +/- percent(1) {
        \\    return a / b;
        \\}
        \\part def Body {
        \\    attribute mass : Mass = kg(5) => +/- 0.5;
        \\}
        \\constraint def Gate {
        \\    require mass <= kg(100) => +/- kg(1);
        \\}
    ;
    const path = "test_plus_minus.deal";

    const handle = lib.deal_parse(source.ptr, source.len, path.ptr, path.len);
    if (handle == null) return error.ParseFailed;
    defer lib.deal_free(handle);

    var fmt_ptr: [*]const u8 = undefined;
    var fmt_len: usize = 0;
    if (!lib.deal_format(handle, &fmt_ptr, &fmt_len)) return error.FormatFailed;

    const formatted = fmt_ptr[0..fmt_len];

    // Clone into GPA so we can print on failure without arena lifetime issues.
    const formatted_copy = try gpa.dupe(u8, formatted);
    defer gpa.free(formatted_copy);

    // D-09: `+/-` must NOT appear in formatted output.
    if (std.mem.indexOf(u8, formatted_copy, "+/-") != null) {
        std.debug.print(
            "fmt_roundtrip.plus_minus_normalization: formatted output still contains '+/-':\n{s}\n",
            .{formatted_copy},
        );
        return error.PlusMinusNotNormalized;
    }

    // D-09: `±` (UTF-8 0xC2 0xB1) must appear at LEAST 3 times (one per attach point).
    var pm_count: usize = 0;
    var scan: usize = 0;
    while (scan + 1 < formatted_copy.len) {
        if (formatted_copy[scan] == 0xC2 and formatted_copy[scan + 1] == 0xB1) {
            pm_count += 1;
            scan += 2;
        } else {
            scan += 1;
        }
    }
    if (pm_count < 3) {
        std.debug.print(
            "fmt_roundtrip.plus_minus_normalization: expected >=3 '±' symbols (one per attach point), found {d}:\n{s}\n",
            .{ pm_count, formatted_copy },
        );
        return error.TooFewPlusMinusSymbols;
    }
}

// Stage-2 S2.7b: the behavioral surface must format idempotently (the fmt
// writers are exercised here before the showcase migration uses them).
test "fmt_roundtrip.behavioral_idempotent" {
    const gpa = std.testing.allocator;
    const source =
        \\package p;
        \\action def Charge {
        \\    in soc : Real;
        \\    action negotiate;
        \\    action deliverPower { out delivered : Real; }
        \\    start -> negotiate -> done;
        \\    negotiate -> decide { [soc >= 80] -> done  [else] -> negotiate };
        \\    negotiate -> par { -> deliverPower } -> done;
        \\    loop while [soc < 100] { deliverPower -> done; }
        \\    for c in cells { negotiate(c); }
        \\    accept PlugInserted;
        \\    assign soc := soc + 1;
        \\    send Overheat to controller;
        \\    deliverPower.delivered ~> negotiate.x;
        \\    bind deliverPower.delivered = negotiate.x;
        \\    node faultMon : negotiate;
        \\    succession faultMon -> [soc < 0] deliverPower;
        \\}
        \\state def S {
        \\    in t : Real;
        \\    state Idle;
        \\    state Active;
        \\    entry / startUp();
        \\    start -> Idle;
        \\    on PlugIn -> Active;
        \\    on Hot [t > 5] / shutdown() -> Idle;
        \\    Active -> done;
        \\}
    ;
    const path = "behavioral_fmt.deal";

    const ha = lib.deal_parse(source.ptr, source.len, path.ptr, path.len);
    if (ha == null) return error.ParseFailed;
    defer lib.deal_free(ha);
    var p1: [*]const u8 = undefined;
    var l1: usize = 0;
    if (!lib.deal_format(ha, &p1, &l1)) return error.FormatFailed;
    const b1 = try gpa.dupe(u8, p1[0..l1]);
    defer gpa.free(b1);

    const hb = lib.deal_parse(b1.ptr, b1.len, path.ptr, path.len);
    if (hb == null) {
        std.debug.print("fmt_roundtrip.behavioral: re-parse failed:\n{s}\n", .{b1});
        return error.ReParseFailed;
    }
    defer lib.deal_free(hb);
    var p2: [*]const u8 = undefined;
    var l2: usize = 0;
    if (!lib.deal_format(hb, &p2, &l2)) return error.FormatFailed2;
    const b2 = p2[0..l2];

    if (!std.mem.eql(u8, b1, b2)) {
        std.debug.print("fmt_roundtrip.behavioral: NOT idempotent.\nB1:\n{s}\nB2:\n{s}\n", .{ b1, b2 });
        try std.testing.expectEqualStrings(b1, b2);
    }
}

/// Core idempotency test:
///   parse(source) → format → bytes1
///   parse(bytes1) → format → bytes2
///   assert bytes1 == bytes2 (byte-equal)
///   assert comment_count(ast_json(parse(bytes1))) == comment_count(ast_json(parse(bytes2)))
fn runRoundTripTest(path: []const u8) !void {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    // --- Read source ---
    const source = cwd.readFileAlloc(io, path, gpa, .unlimited) catch |err| {
        std.debug.print("fmt_roundtrip: read {s} failed ({s})\n", .{ path, @errorName(err) });
        return err;
    };
    defer gpa.free(source);

    // ═══════════════ PASS 1: parse(source) → format → bytes1 ═══════════════

    const handle_a = lib.deal_parse(source.ptr, source.len, path.ptr, path.len);
    if (handle_a == null) {
        std.debug.print("fmt_roundtrip: deal_parse returned null for {s}\n", .{path});
        return error.ParseFailed;
    }
    var handle_a_freed = false;
    defer if (!handle_a_freed) lib.deal_free(handle_a);

    const diag_count_a = lib.deal_diagnostics_count(handle_a);
    if (diag_count_a > 0) {
        std.debug.print("fmt_roundtrip: {s} has {d} diagnostics on first parse\n", .{ path, diag_count_a });
    }

    // Format pass 1
    var fmt1_ptr: [*]const u8 = undefined;
    var fmt1_len: usize = 0;
    const fmt1_ok = lib.deal_format(handle_a, &fmt1_ptr, &fmt1_len);
    if (!fmt1_ok) {
        std.debug.print("fmt_roundtrip: deal_format (pass 1) failed for {s}\n", .{path});
        lib.deal_free(handle_a);
        handle_a_freed = true;
        return error.FormatFailed;
    }

    // Clone bytes1 BEFORE freeing handle_a (Pitfall 3 — T-02-29)
    const bytes1 = try gpa.dupe(u8, fmt1_ptr[0..fmt1_len]);
    defer gpa.free(bytes1);

    // Get AST JSON for comment count comparison (pass 1)
    var ast_a_ptr: [*]const u8 = undefined;
    var ast_a_len: usize = 0;
    const ast_a_ok = lib.deal_ast_json(handle_a, &ast_a_ptr, &ast_a_len);
    if (!ast_a_ok) {
        lib.deal_free(handle_a);
        handle_a_freed = true;
        return error.AstJsonFailed;
    }
    const comments_a = countCommentEntries(ast_a_ptr[0..ast_a_len]);

    lib.deal_free(handle_a);
    handle_a_freed = true;

    // ═══════════════ PASS 2: parse(bytes1) → format → bytes2 ═══════════════

    const handle_b = lib.deal_parse(bytes1.ptr, bytes1.len, path.ptr, path.len);
    if (handle_b == null) {
        std.debug.print("fmt_roundtrip: re-parse (pass 2) returned null for {s}\n", .{path});
        const preview_len = @min(bytes1.len, 400);
        std.debug.print("fmt_roundtrip: bytes1 preview:\n{s}\n", .{bytes1[0..preview_len]});
        return error.ReParseFailed;
    }
    defer lib.deal_free(handle_b);

    const diag_count_b = lib.deal_diagnostics_count(handle_b);
    if (diag_count_b > diag_count_a) {
        std.debug.print(
            "fmt_roundtrip: pass-2 parse of {s} has MORE diagnostics ({d}) than pass-1 ({d})\n",
            .{ path, diag_count_b, diag_count_a },
        );
        var dj_ptr: [*]const u8 = undefined;
        var dj_len: usize = 0;
        if (lib.deal_diagnostics_json(handle_b, &dj_ptr, &dj_len)) {
            std.debug.print("diagnostics (first 800 bytes): {s}\n", .{dj_ptr[0..@min(dj_len, 800)]});
        }
    }

    // Format pass 2
    var fmt2_ptr: [*]const u8 = undefined;
    var fmt2_len: usize = 0;
    const fmt2_ok = lib.deal_format(handle_b, &fmt2_ptr, &fmt2_len);
    if (!fmt2_ok) {
        std.debug.print("fmt_roundtrip: deal_format (pass 2) failed for {s}\n", .{path});
        return error.FormatFailed2;
    }
    const bytes2 = fmt2_ptr[0..fmt2_len]; // arena-owned — do NOT free; will die with handle_b

    // ═══════════════ IDEMPOTENCY ASSERTION ═══════════════

    if (!std.mem.eql(u8, bytes1, bytes2)) {
        std.debug.print(
            "fmt_roundtrip: IDEMPOTENCY FAILED for {s} — bytes1.len={d} bytes2.len={d}\n",
            .{ path, bytes1.len, bytes2.len },
        );
        // Print diff context: find first divergence
        const min_len = @min(bytes1.len, bytes2.len);
        var first_diff: usize = 0;
        while (first_diff < min_len and bytes1[first_diff] == bytes2[first_diff]) {
            first_diff += 1;
        }
        const ctx_start = if (first_diff >= 80) first_diff - 80 else 0;
        const ctx_end = @min(first_diff + 200, @min(bytes1.len, bytes2.len));
        std.debug.print("fmt_roundtrip: first divergence at byte {d}\n", .{first_diff});
        std.debug.print("bytes1[{d}..{d}]: {s}\n", .{ ctx_start, ctx_end, bytes1[ctx_start..@min(ctx_end, bytes1.len)] });
        std.debug.print("bytes2[{d}..{d}]: {s}\n", .{ ctx_start, ctx_end, bytes2[ctx_start..@min(ctx_end, bytes2.len)] });
        try std.testing.expectEqualStrings(bytes1, bytes2);
    }

    // ═══════════════ COMMENT PRESERVATION ASSERTION ═══════════════

    var ast_b_ptr: [*]const u8 = undefined;
    var ast_b_len: usize = 0;
    const ast_b_ok = lib.deal_ast_json(handle_b, &ast_b_ptr, &ast_b_len);
    if (!ast_b_ok) return error.AstJsonBFailed;
    const comments_b = countCommentEntries(ast_b_ptr[0..ast_b_len]);

    if (comments_a != comments_b) {
        std.debug.print(
            "fmt_roundtrip: comment count DIVERGED for {s}: pass1={d} pass2={d}\n",
            .{ path, comments_a, comments_b },
        );
        try std.testing.expectEqual(comments_a, comments_b);
    }
}
