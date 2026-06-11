//! diag.json_roundtrip — D-14 / D-15 lossless JSON gate.
//!
//! Constructs a Diagnostic with EVERY field populated (code, severity,
//! message, span, secondary_spans, fix_it, notes) and asserts every field
//! survives json.emitDiagnostics + std.json.parseFromSlice unchanged.
//! Then verifies the default-field shape (fix_it=null, notes="",
//! secondary_spans=&.{}) emits the expected JSON literals.

const std = @import("std");
const ast = @import("ast");
const json = @import("json");
const diagnostics_mod = @import("diagnostics");

test "diag.json_roundtrip" {
    const gpa = std.testing.allocator;

    // ── Case 1: every field populated ─────────────────────────────
    {
        const secondary = [_]diagnostics_mod.SpanLabel{
            .{ .label = "here", .span = .{ .start = 5, .end = 8 } },
            .{ .label = "and here", .span = .{ .start = 30, .end = 35 } },
        };
        const diag = diagnostics_mod.Diagnostic{
            .code = "E0302",
            .severity = .err,
            .message = "Test message with \"escapes\" and \\backslashes",
            .span = .{ .start = 10, .end = 20 },
            .secondary_spans = secondary[0..],
            .fix_it = .{
                .replacement = "fixed text",
                .replace_span = .{ .start = 10, .end = 20 },
            },
            .notes = "Multi-line notes\nwith\ttabs",
        };

        const arr = [_]diagnostics_mod.Diagnostic{diag};
        const out = try json.emitDiagnostics(gpa, arr[0..]);
        defer gpa.free(out);

        // Parse the resulting JSON and inspect every field.
        const parsed = try std.json.parseFromSlice(std.json.Value, gpa, out, .{});
        defer parsed.deinit();

        const root = parsed.value;
        try std.testing.expect(root == .array);
        try std.testing.expectEqual(@as(usize, 1), root.array.items.len);

        const item = root.array.items[0];
        try std.testing.expect(item == .object);
        const obj = item.object;

        // code
        try std.testing.expectEqualStrings("E0302", obj.get("code").?.string);

        // message
        try std.testing.expectEqualStrings(
            "Test message with \"escapes\" and \\backslashes",
            obj.get("message").?.string,
        );

        // severity
        try std.testing.expectEqualStrings("err", obj.get("severity").?.string);

        // span [10, 20]
        const span = obj.get("span").?.array;
        try std.testing.expectEqual(@as(usize, 2), span.items.len);
        try std.testing.expectEqual(@as(i64, 10), span.items[0].integer);
        try std.testing.expectEqual(@as(i64, 20), span.items[1].integer);

        // secondary_spans — 2 entries
        const sec = obj.get("secondary_spans").?.array;
        try std.testing.expectEqual(@as(usize, 2), sec.items.len);

        const sec0 = sec.items[0].object;
        try std.testing.expectEqualStrings("here", sec0.get("label").?.string);
        const sec0_span = sec0.get("span").?.array;
        try std.testing.expectEqual(@as(i64, 5), sec0_span.items[0].integer);
        try std.testing.expectEqual(@as(i64, 8), sec0_span.items[1].integer);

        const sec1 = sec.items[1].object;
        try std.testing.expectEqualStrings("and here", sec1.get("label").?.string);
        const sec1_span = sec1.get("span").?.array;
        try std.testing.expectEqual(@as(i64, 30), sec1_span.items[0].integer);
        try std.testing.expectEqual(@as(i64, 35), sec1_span.items[1].integer);

        // fix_it
        const fi = obj.get("fix_it").?.object;
        try std.testing.expectEqualStrings("fixed text", fi.get("replacement").?.string);
        const fi_span = fi.get("replace_span").?.array;
        try std.testing.expectEqual(@as(i64, 10), fi_span.items[0].integer);
        try std.testing.expectEqual(@as(i64, 20), fi_span.items[1].integer);

        // notes
        try std.testing.expectEqualStrings(
            "Multi-line notes\nwith\ttabs",
            obj.get("notes").?.string,
        );
    }

    // ── Case 2: defaults — fix_it=null, notes="", secondary_spans=[] ──
    {
        const diag = diagnostics_mod.Diagnostic{
            .code = "E0100",
            .severity = .info,
            .message = "minimal",
            .span = .{ .start = 0, .end = 1 },
        };
        const arr = [_]diagnostics_mod.Diagnostic{diag};
        const out = try json.emitDiagnostics(gpa, arr[0..]);
        defer gpa.free(out);

        // Verify the JSON contains the exact literal shapes.
        try std.testing.expect(std.mem.indexOf(u8, out, "\"fix_it\":null") != null);
        try std.testing.expect(std.mem.indexOf(u8, out, "\"notes\":\"\"") != null);
        try std.testing.expect(std.mem.indexOf(u8, out, "\"secondary_spans\":[]") != null);
        try std.testing.expect(std.mem.indexOf(u8, out, "\"severity\":\"info\"") != null);
    }

    // ── Case 3: empty array ───────────────────────────────────────
    {
        const arr = [_]diagnostics_mod.Diagnostic{};
        const out = try json.emitDiagnostics(gpa, arr[0..]);
        defer gpa.free(out);
        try std.testing.expectEqualStrings("[]", out);
    }
}
