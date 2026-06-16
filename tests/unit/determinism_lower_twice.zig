//! determinism.lower_twice — Plan 02-03 IR determinism contract.
//!
//! For each of the 19 showcase files, lowers the source twice in the same
//! process via the PUBLIC C ABI (deal_parse / deal_ir_json / deal_free),
//! and asserts byte-identical IR JSON across both calls.
//!
//! Parallels determinism_parse_twice.zig but validates that the IR lowering
//! pass + JSON emission is also deterministic (T-02-15 from the plan). The
//! JSON emitter sorts edges by (src, dst, kind) and elements alphabetically
//! to guarantee byte stability.
//!
//! CRITICAL: Uses the PUBLIC C ABI (deal_parse + deal_ir_json), NOT the
//! test-only `deal_lower_internal` helper. Determinism must hold at the
//! contract surface (same rationale as determinism_parse_twice.zig).
//!
//! Lifetime note: the JSON buffer returned by deal_ir_json is arena-owned
//! by the handle and invalidated by deal_free(handle). Therefore: copy the
//! first parse's IR JSON into a gpa-owned buffer via gpa.dupe BEFORE calling
//! deal_free(handle_a), so both byte strings live simultaneously for the
//! comparison.

const std = @import("std");
const lib = @import("lib");

/// 19 showcase files — mirrors the corpus used by determinism_parse_twice.
const showcase_files = [_][]const u8{
    "tests/showcase/packages/vehicle/battery.deal",
    "tests/showcase/packages/vehicle/motor.deal",
    "tests/showcase/packages/vehicle/behaviors.deal",
    "tests/showcase/packages/vehicle/charging-states.deal",
    "tests/showcase/packages/vehicle/components.deal",
    "tests/showcase/packages/vehicle/index.deal",
    "tests/showcase/packages/interfaces/electrical.deal",
    "tests/showcase/packages/interfaces/thermal.deal",
    "tests/showcase/packages/interfaces/connections.deal",
    "tests/showcase/packages/interfaces/index.deal",
    "tests/showcase/packages/requirements/system.deal",
    "tests/showcase/packages/requirements/needs.deal",
    "tests/showcase/packages/requirements/index.deal",
    "tests/showcase/packages/use-cases/driving.deal",
    "tests/showcase/packages/use-cases/charging.deal",
    "tests/showcase/packages/use-cases/index.deal",
    "tests/showcase/model/traceability.dealx",
    "tests/showcase/model/vehicle.dealx",
    "tests/showcase/model/variants/performance.dealx",
    "tests/showcase/model/variants/sedan.dealx",
};

test "determinism.lower_twice" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    for (showcase_files) |path| {
        const source = cwd.readFileAlloc(io, path, gpa, .unlimited) catch |err| {
            std.debug.print("determinism.lower_twice: read {s} failed ({s})\n", .{ path, @errorName(err) });
            return err;
        };
        defer gpa.free(source);

        // --- First parse + lower via PUBLIC C ABI ---
        const handle_a = lib.deal_parse(source.ptr, source.len, path.ptr, path.len);
        try std.testing.expect(handle_a != null);

        // Guard against early-exit leaking handle_a.
        var handle_a_freed: bool = false;
        defer if (!handle_a_freed) lib.deal_free(handle_a);

        // Pull IR JSON via deal_ir_json (out-param style).
        var ptr_a: [*]const u8 = undefined;
        var len_a: usize = 0;
        const ok_a = lib.deal_ir_json(handle_a, &ptr_a, &len_a);
        try std.testing.expect(ok_a);

        // Copy into gpa-owned buffer BEFORE deal_free(handle_a) invalidates
        // ptr_a. Without this dupe the comparison below would use a dangling
        // pointer against parse-B's buffer.
        const ir_json_a = try gpa.dupe(u8, ptr_a[0..len_a]);
        defer gpa.free(ir_json_a);

        // Showcase files should produce zero parse diagnostics.
        try std.testing.expectEqual(@as(u32, 0), lib.deal_diagnostics_count(handle_a));

        // Free handle A before starting parse B (same pattern as
        // determinism_parse_twice — releases the page-arena to avoid
        // overlapping arenas across both parses).
        lib.deal_free(handle_a);
        handle_a_freed = true;

        // --- Second parse + lower of the same source ---
        const handle_b = lib.deal_parse(source.ptr, source.len, path.ptr, path.len);
        try std.testing.expect(handle_b != null);
        defer lib.deal_free(handle_b);

        var ptr_b: [*]const u8 = undefined;
        var len_b: usize = 0;
        const ok_b = lib.deal_ir_json(handle_b, &ptr_b, &len_b);
        try std.testing.expect(ok_b);

        try std.testing.expectEqual(@as(u32, 0), lib.deal_diagnostics_count(handle_b));

        // --- Byte-equality assertion ---
        // IR JSON must be byte-identical across the two lowers (T-02-15).
        if (!std.mem.eql(u8, ir_json_a, ptr_b[0..len_b])) {
            std.debug.print(
                "determinism.lower_twice DIVERGED on IR JSON for {s}\n",
                .{path},
            );
            try std.testing.expectEqualStrings(ir_json_a, ptr_b[0..len_b]);
        }
    }
}
