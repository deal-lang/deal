//! c_abi.invalid_utf8 — Tests that deal_parse returns a non-null handle
//! carrying exactly one E0001 diagnostic (and no AST root) for each of five
//! canonical invalid-UTF-8 byte sequences (Plan 06 Task 6.2).
//!
//! Uses deal_parse_internal (pub-but-NOT-exported) so std.testing.allocator
//! backs the arena — this also exercises the leak-detection path.
//!
//! Security reference: T-06-01 / V5 ASVS — std.unicode.utf8ValidateSlice
//! guard at the top of deal_parse / deal_parse_internal.

const std = @import("std");
const lib = @import("lib");
const json_mod = @import("json");
const diagnostics_mod = @import("diagnostics");

test "c_abi.invalid_utf8" {
    const gpa = std.testing.allocator;

    // Five canonical invalid-UTF-8 byte sequences.
    const cases = [_]struct {
        name: []const u8,
        bytes: []const u8,
    }{
        // Case A: lone continuation byte (0x80 has no preceding leading byte).
        .{ .name = "lone_continuation", .bytes = &[_]u8{0x80} },
        // Case B: invalid start byte (0xFF is never valid in UTF-8).
        .{ .name = "invalid_start_0xff", .bytes = &[_]u8{0xFF} },
        // Case C: truncated 2-byte sequence (0xC3 alone, no continuation).
        .{ .name = "truncated_2byte", .bytes = &[_]u8{0xC3} },
        // Case D: overlong encoding (U+0000 encoded as 2 bytes — RFC 3629 §3).
        .{ .name = "overlong_null", .bytes = &[_]u8{ 0xC0, 0x80 } },
        // Case E: valid prefix followed by invalid byte.
        .{ .name = "valid_prefix_then_invalid", .bytes = &[_]u8{ 'a', 'b', 0xFF, 'c' } },
    };

    for (cases) |c| {
        const filename = "test.deal";
        const handle = try lib.deal_parse_internal(gpa, c.bytes, filename);
        defer lib.deal_free_internal(handle);

        // D-10: handle must be non-null (deal_parse_internal returns !*DealHandle,
        // so the try above ensures we only reach here on success).

        // D-10: has_errors must be true — invalid UTF-8 always produces E0001.
        try std.testing.expect(handle.diagnostics.items.len > 0);
        try std.testing.expectEqual(@as(usize, 1), handle.diagnostics.items.len);

        // The single diagnostic must be E0001.
        const diag = handle.diagnostics.items[0];
        try std.testing.expectEqualStrings(diagnostics_mod.Codes.e_invalid_utf8, diag.code);

        // The parser was NOT invoked — ast_root must be null.
        try std.testing.expectEqual(@as(?*@import("ast").Node, null), handle.ast_root);

        // Emit diagnostics JSON and confirm "E0001" is present.
        const diag_json = try json_mod.emitDiagnostics(gpa, handle.diagnostics.items);
        defer gpa.free(diag_json);
        try std.testing.expect(std.mem.indexOf(u8, diag_json, "E0001") != null);

        // Also verify via the C ABI accessors (opaque path).
        const opaque_h: ?*anyopaque = @ptrCast(handle);
        try std.testing.expect(lib.deal_has_errors(opaque_h));
        try std.testing.expectEqual(@as(u32, 1), lib.deal_diagnostics_count(opaque_h));

        var ptr: [*]const u8 = undefined;
        var len: usize = 0;
        try std.testing.expect(lib.deal_diagnostics_json(opaque_h, &ptr, &len));
        const json_slice = ptr[0..len];
        try std.testing.expect(std.mem.indexOf(u8, json_slice, "E0001") != null);
    }
}
