//! parser_deal_strictness — Phase 05.1 plan 03 strictness tests for D-01/D-02/D-03.
//!
//! D-01: `;` is the canonical annotation-body field terminator (consumed
//!       explicitly); `,` as a separator is rejected with E0123.
//! D-02: An unrecognized key inside a `verification { ... }` block emits
//!       E0124 (e_unknown_verification_key) rather than being silently dropped.
//! D-03: A keyword token (`from:`, etc.) is valid as an annotation-body field
//!       key — the existing `isKeyword` path at parser_deal.zig:1908 handles
//!       it, and this test asserts zero diagnostics are emitted and the AST
//!       contains the parsed field.
//!
//! Tests:
//!   (a) D-01 — `;`-terminated annotation body → zero diagnostics
//!   (b) D-01 — `,`-separated annotation body → E0123 as first diagnostic
//!   (c) D-02 — `verification { badkey: ... }` → E0124 as first diagnostic
//!   (d) D-03 — `from:` keyword field key → zero diagnostics + field in AST JSON

const std = @import("std");
const lib = @import("lib");
const diagnostics_mod = @import("diagnostics");

const Codes = diagnostics_mod.Codes;
const Diagnostic = diagnostics_mod.Diagnostic;

// ── Helpers ─────────────────────────────────────────────────────────────────

fn countCode(diags: []const Diagnostic, code: []const u8) usize {
    var n: usize = 0;
    for (diags) |d| {
        if (std.mem.eql(u8, d.code, code)) n += 1;
    }
    return n;
}

// ── D-01 (a): semicolon-terminated annotation body parses clean ──────────────

test "strictness.d01_semicolon_terminated_annotation_body_is_clean" {
    // An annotation body using `;` as the canonical field terminator must
    // produce zero diagnostics.
    // Canonical form from the showcase corpus (behaviors.deal:29):
    //   @flow:<<flows to>> deliverPower { from: requestTorque; }
    const gpa = std.testing.allocator;

    const src =
        \\package foo;
        \\action def DriveMotor {
        \\    action requestTorque;
        \\    action deliverPower;
        \\    @flow:<<flows to>> deliverPower { from: requestTorque; }
        \\}
    ;

    const handle = try lib.deal_parse_internal(gpa, src, "d01_semicolon_test.deal");
    defer lib.deal_free_internal(handle);

    if (handle.diagnostics.items.len != 0) {
        std.debug.print(
            "strictness.d01_semicolon FAILED: expected 0 diagnostics, got {d}; first = {s}\n",
            .{ handle.diagnostics.items.len, handle.diagnostics.items[0].code },
        );
        return error.TestExpectedEqual;
    }
}

// ── D-01 (b): comma-separated annotation body emits E0123 ───────────────────

test "strictness.d01_comma_separator_emits_e0123" {
    // A `,` used as a field separator inside an annotation body must emit
    // E0123 (e_annotation_comma_separator) as the first diagnostic.
    const gpa = std.testing.allocator;

    // Uses a category annotation body so that `parseAnnotationBody` is called.
    // The `,` between the two fields is the malformed separator.
    const src =
        \\package foo;
        \\part def P {
        \\    @trace:<<satisfies>> R { voltage: 12, current: 5 }
        \\}
    ;

    const handle = try lib.deal_parse_internal(gpa, src, "d01_comma_test.deal");
    defer lib.deal_free_internal(handle);

    const count = countCode(handle.diagnostics.items, Codes.e_annotation_comma_separator);
    if (count == 0) {
        const first = if (handle.diagnostics.items.len > 0) handle.diagnostics.items[0].code else "<none>";
        std.debug.print(
            "strictness.d01_comma FAILED: expected >=1x E0123, got {d} diags; first code = {s}\n",
            .{ handle.diagnostics.items.len, first },
        );
        return error.TestExpectedEqual;
    }

    // The first diagnostic must be E0123.
    if (!std.mem.eql(u8, handle.diagnostics.items[0].code, Codes.e_annotation_comma_separator)) {
        std.debug.print(
            "strictness.d01_comma FAILED: first diagnostic is {s}, expected {s}\n",
            .{ handle.diagnostics.items[0].code, Codes.e_annotation_comma_separator },
        );
        return error.TestExpectedEqual;
    }
}

// ── D-02 (c): unknown verification key emits E0124 ──────────────────────────

test "strictness.d02_unknown_verification_key_emits_e0124" {
    // An unrecognized key inside `verification { ... }` must emit E0124
    // (e_unknown_verification_key) rather than silently dropping the token.
    const gpa = std.testing.allocator;

    const src =
        \\package foo;
        \\requirement def R {
        \\    verification { badkey: true; }
        \\}
    ;

    const handle = try lib.deal_parse_internal(gpa, src, "d02_badkey_test.deal");
    defer lib.deal_free_internal(handle);

    const count = countCode(handle.diagnostics.items, Codes.e_unknown_verification_key);
    if (count == 0) {
        const first = if (handle.diagnostics.items.len > 0) handle.diagnostics.items[0].code else "<none>";
        std.debug.print(
            "strictness.d02_badkey FAILED: expected >=1x E0124, got {d} diags; first code = {s}\n",
            .{ handle.diagnostics.items.len, first },
        );
        return error.TestExpectedEqual;
    }

    // The first diagnostic must be E0124.
    if (!std.mem.eql(u8, handle.diagnostics.items[0].code, Codes.e_unknown_verification_key)) {
        std.debug.print(
            "strictness.d02_badkey FAILED: first diagnostic is {s}, expected {s}\n",
            .{ handle.diagnostics.items[0].code, Codes.e_unknown_verification_key },
        );
        return error.TestExpectedEqual;
    }
}

// ── D-03 (d): keyword field key (`from:`) parses correctly ──────────────────

test "strictness.d03_keyword_field_key_from_is_accepted" {
    // `from:` uses the keyword token `kw_from` as an annotation field key.
    // The `isKeyword` path at parser_deal.zig:1908 accepts it explicitly.
    // Oracle: spec/examples/showcase/packages/vehicle/behaviors.deal:29
    //   @flow:<<flows to>> deliverPower { from: requestTorque; }
    //
    // Assertion 1: zero diagnostics (the keyword-as-field-key form is valid).
    // Assertion 2: the AST JSON contains `"from"` confirming the field was
    //             parsed into the annotation_body, not silently skipped.
    const gpa = std.testing.allocator;

    const src =
        \\package foo;
        \\action def DriveMotor {
        \\    action requestTorque;
        \\    action deliverPower;
        \\    @flow:<<flows to>> deliverPower { from: requestTorque; }
        \\}
    ;
    const filename = "d03_from_key_test.deal";

    // Assertion 1: zero diagnostics via deal_parse_internal.
    {
        const handle = try lib.deal_parse_internal(gpa, src, filename);
        defer lib.deal_free_internal(handle);

        if (handle.diagnostics.items.len != 0) {
            std.debug.print(
                "strictness.d03_from_key FAILED: expected 0 diagnostics, got {d}; first = {s}\n",
                .{ handle.diagnostics.items.len, handle.diagnostics.items[0].code },
            );
            return error.TestExpectedEqual;
        }
    }

    // Assertion 2: AST JSON contains `"from"` as a field key.
    // Use the PUBLIC C ABI (deal_parse + deal_ast_json) so the JSON
    // serialization layer is exercised end-to-end.
    {
        const h = lib.deal_parse(src.ptr, src.len, filename.ptr, filename.len);
        if (h == null) return error.TestUnexpectedNull;
        defer lib.deal_free(h);

        var json_ptr: [*]const u8 = undefined;
        var json_len: usize = 0;
        const ok = lib.deal_ast_json(h, &json_ptr, &json_len);
        if (!ok) {
            std.debug.print("strictness.d03_from_key FAILED: deal_ast_json returned false\n", .{});
            return error.TestExpectedEqual;
        }
        const json = json_ptr[0..json_len];

        // The annotation_body serialization includes `"from"` as a string
        // field key when the keyword-as-field-key accept path fires.
        if (std.mem.indexOf(u8, json, "\"from\"") == null) {
            std.debug.print(
                "strictness.d03_from_key FAILED: `\"from\"` key not found in AST JSON\n",
                .{},
            );
            return error.TestExpectedEqual;
        }
    }
}
