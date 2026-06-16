//! determinism.parse_twice — REQ-phase-1.5-3 contract enforcement.
//!
//! For each of the 19 showcase files, parses the source twice in the same
//! process via the PUBLIC C ABI (deal_parse / deal_ast_json / deal_free),
//! and asserts byte-identical AST JSON across both parses. Also asserts
//! diagnostics-JSON byte-equality (Claude's-discretion add-on per
//! CONTEXT.md §determinism — trivial cost).
//!
//! Locks the D-18 alphabetical-JSON-key invariant against the regression
//! class "AST identical but JSON ordering perturbed" — which the locked
//! snapshot tests do not catch because they only compare the FIRST parse
//! against the locked snapshot, not parse A against parse B.
//!
//! CRITICAL: Uses the PUBLIC C ABI (deal_parse), NOT the test-only
//! internal helper. Per CONTEXT.md §"Determinism test (REQ-phase-1.5-3)":
//!   "Test uses `deal_parse` (the public C ABI) not internal helpers —
//!    determinism must hold at the contract surface."
//! Internal-helper byte-equality could pass while the public ABI's JSON
//! cache produces different bytes — that would be a determinism bug the
//! test must catch, not miss. The Plan 01.5-04 grep gate enforces this
//! by requiring zero occurrences of the test-only-helper symbol name
//! anywhere in this file.
//!
//! Lifetime note: the JSON buffer returned by deal_ast_json is arena-owned
//! by the handle and invalidated by deal_free(handle). Therefore: copy the
//! first parse's JSON into a gpa-owned buffer via gpa.dupe BEFORE calling
//! deal_free(handle_a) AND BEFORE starting parse B, so both byte strings
//! live simultaneously for the comparison.

const std = @import("std");
const lib = @import("lib");

/// 22 showcase files — 18 .deal + 4 .dealx — mirrors the corpus used by
/// parser_deal.coverage and c_abi.no_leaks. Paths are relative to the
/// repository root (cwd at test time).
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
    // Phase-05.2 oracle files (SD-21/22/23 — calc/constraint/precision)
    "tests/showcase/packages/analysis/calcs.deal",
    "tests/showcase/packages/analysis/constraints.deal",
    "tests/showcase/packages/analysis/precision.deal",
    "tests/showcase/model/traceability.dealx",
    "tests/showcase/model/vehicle.dealx",
    "tests/showcase/model/variants/performance.dealx",
    "tests/showcase/model/variants/sedan.dealx",
};

test "determinism.parse_twice" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    for (showcase_files) |path| {
        const source = cwd.readFileAlloc(io, path, gpa, .unlimited) catch |err| {
            std.debug.print("determinism.parse_twice: read {s} failed ({s})\n", .{ path, @errorName(err) });
            return err;
        };
        defer gpa.free(source);

        // --- First parse via PUBLIC C ABI ---
        const handle_a = lib.deal_parse(source.ptr, source.len, path.ptr, path.len);
        try std.testing.expect(handle_a != null);

        // WR-04: handle_a's arena is allocated by page_allocator (not
        // std.testing.allocator) so it would not be flagged by the test
        // leak detector, but a `try` failure between here and the
        // explicit free below would still leak the page-mapped arena
        // for the remainder of the test process. Guard the free with a
        // deferred fallback that releases the handle on any early
        // error-return; the explicit pre-parse-B free sets the guard
        // flag so the defer becomes a no-op on the happy path.
        var handle_a_freed: bool = false;
        defer if (!handle_a_freed) lib.deal_free(handle_a);

        // Pull AST JSON out via deal_ast_json (out-param style).
        var ptr_a: [*]const u8 = undefined;
        var len_a: usize = 0;
        const ok_a = lib.deal_ast_json(handle_a, &ptr_a, &len_a);
        try std.testing.expect(ok_a);

        // Copy the bytes into a gpa-owned buffer BEFORE deal_free(handle_a)
        // invalidates ptr_a. Without this dupe the byte-equality check
        // below would compare a dangling pointer against parse-B's buffer.
        const json_a = try gpa.dupe(u8, ptr_a[0..len_a]);
        defer gpa.free(json_a);

        // Same for diagnostics JSON (Claude's-discretion addon — trivially
        // cheap once we already have a handle in hand).
        var dptr_a: [*]const u8 = undefined;
        var dlen_a: usize = 0;
        const dok_a = lib.deal_diagnostics_json(handle_a, &dptr_a, &dlen_a);
        try std.testing.expect(dok_a);
        const diag_a = try gpa.dupe(u8, dptr_a[0..dlen_a]);
        defer gpa.free(diag_a);

        // Showcase files are well-formed — every file should emit zero
        // diagnostics. The diagnostics JSON byte-equality check would
        // still be meaningful if a file regressed, but the empty-array
        // shape is the expected normal output.
        try std.testing.expectEqual(@as(u32, 0), lib.deal_diagnostics_count(handle_a));

        // Free handle A — this invalidates ptr_a / dptr_a, but json_a /
        // diag_a still live in gpa-owned memory. We MUST free handle_a
        // BEFORE starting parse B so the page-arena from parse A is
        // released before parse B's arena is mapped (lifetime contract
        // in the file's doc-comment); the deferred fallback above only
        // fires if a try-failure above causes early return.
        lib.deal_free(handle_a);
        handle_a_freed = true;

        // --- Second parse of the same source via PUBLIC C ABI ---
        const handle_b = lib.deal_parse(source.ptr, source.len, path.ptr, path.len);
        try std.testing.expect(handle_b != null);
        defer lib.deal_free(handle_b);

        var ptr_b: [*]const u8 = undefined;
        var len_b: usize = 0;
        const ok_b = lib.deal_ast_json(handle_b, &ptr_b, &len_b);
        try std.testing.expect(ok_b);

        var dptr_b: [*]const u8 = undefined;
        var dlen_b: usize = 0;
        const dok_b = lib.deal_diagnostics_json(handle_b, &dptr_b, &dlen_b);
        try std.testing.expect(dok_b);

        try std.testing.expectEqual(@as(u32, 0), lib.deal_diagnostics_count(handle_b));

        // --- Byte-equality assertions ---
        // AST JSON must be byte-identical across the two parses (D-18).
        if (!std.mem.eql(u8, json_a, ptr_b[0..len_b])) {
            std.debug.print(
                "determinism.parse_twice DIVERGED on AST JSON for {s}\n",
                .{path},
            );
            // expectEqualStrings will emit a precise diff into the test
            // output before failing the test.
            try std.testing.expectEqualStrings(json_a, ptr_b[0..len_b]);
        }

        // Diagnostics JSON must also be byte-identical (Claude's-discretion
        // addon per CONTEXT.md §determinism — trivial cost).
        if (!std.mem.eql(u8, diag_a, dptr_b[0..dlen_b])) {
            std.debug.print(
                "determinism.parse_twice DIVERGED on diagnostics JSON for {s}\n",
                .{path},
            );
            try std.testing.expectEqualStrings(diag_a, dptr_b[0..dlen_b]);
        }
    }
}
