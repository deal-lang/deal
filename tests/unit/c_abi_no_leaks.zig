//! c_abi.no_leaks — Zero-leak verification for the C ABI arena (Plan 06 Task 6.2).
//!
//! Uses `deal_parse_internal` (pub-but-NOT-@exported test helper defined in
//! src/lib.zig) with `std.testing.allocator` as the arena's backing allocator.
//! Zig 0.16.0's test runner leak-detection automatically reports any bytes that
//! are still allocated at test exit — a non-zero count fails the test.
//!
//! Covers ~70 files: the 19 showcase files (15 .deal + 4 .dealx) PLUS the
//! ≥50-file malformed corpus. Every allocation is freed via `deal_free_internal`.
//!
//! Design note (SUMMARY §c_abi.no_leaks):
//!   `deal_parse_internal` accepts a caller-provided `std.mem.Allocator` so the
//!   arena can be backed by `std.testing.allocator` (which detects leaks).
//!   This is the ONLY reason `deal_parse_internal` exists — it is pub-but-NOT-
//!   @exported, meaning it is reachable from in-tree tests but MUST NOT appear
//!   in the `nm -gU zig-out/lib/libdeal.a` symbol table.
//!
//! T-06-05 (Resource Exhaustion / Memory leak): mitigated by this test.
//! RESEARCH line 1025: std.testing.allocator leak detection as the arena oracle.

const std = @import("std");
const lib = @import("lib");
const json_mod = @import("json");

/// 15 showcase .deal files (from parser_deal_snapshot.zig).
const showcase_deal_files = [_][]const u8{
    "packages/vehicle/battery.deal",
    "packages/vehicle/motor.deal",
    "packages/vehicle/behaviors.deal",
    "packages/vehicle/components.deal",
    "packages/vehicle/index.deal",
    "packages/interfaces/electrical.deal",
    "packages/interfaces/thermal.deal",
    "packages/interfaces/connections.deal",
    "packages/interfaces/index.deal",
    "packages/requirements/system.deal",
    "packages/requirements/needs.deal",
    "packages/requirements/index.deal",
    "packages/use-cases/driving.deal",
    "packages/use-cases/charging.deal",
    "packages/use-cases/index.deal",
};

/// 4 showcase .dealx files (from parser_dealx_snapshot.zig).
const showcase_dealx_files = [_][]const u8{
    "model/vehicle.dealx",
    "model/traceability.dealx",
    "model/variants/sedan.dealx",
    "model/variants/performance.dealx",
};

test "c_abi.no_leaks" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    var file_count: usize = 0;

    // --- Showcase files (19 total) ---
    for (showcase_deal_files) |rel| {
        const showcase_path = try std.fmt.allocPrint(gpa, "tests/showcase/{s}", .{rel});
        defer gpa.free(showcase_path);

        const source = cwd.readFileAlloc(io, showcase_path, gpa, .unlimited) catch |err| {
            std.debug.print("c_abi.no_leaks: read {s} failed ({s})\n", .{ showcase_path, @errorName(err) });
            return err;
        };
        defer gpa.free(source);

        const handle = try lib.deal_parse_internal(gpa, source, rel);

        // Force AST JSON cache to populate (exercises the lazy path).
        _ = json_mod.emitAst(handle.arena.allocator(), handle.ast_root, handle.mode, handle.filename) catch {};

        // Force diagnostics JSON cache to populate.
        _ = json_mod.emitDiagnostics(handle.arena.allocator(), handle.diagnostics.items) catch {};

        lib.deal_free_internal(handle);
        file_count += 1;
    }

    for (showcase_dealx_files) |rel| {
        const showcase_path = try std.fmt.allocPrint(gpa, "tests/showcase/{s}", .{rel});
        defer gpa.free(showcase_path);

        const source = cwd.readFileAlloc(io, showcase_path, gpa, .unlimited) catch |err| {
            std.debug.print("c_abi.no_leaks: read {s} failed ({s})\n", .{ showcase_path, @errorName(err) });
            return err;
        };
        defer gpa.free(source);

        const handle = try lib.deal_parse_internal(gpa, source, rel);

        _ = json_mod.emitAst(handle.arena.allocator(), handle.ast_root, handle.mode, handle.filename) catch {};
        _ = json_mod.emitDiagnostics(handle.arena.allocator(), handle.diagnostics.items) catch {};

        lib.deal_free_internal(handle);
        file_count += 1;
    }

    // --- Malformed corpus (≥50 files) ---
    var malformed_dir = cwd.openDir(io, "tests/malformed", .{ .iterate = true }) catch |err| {
        std.debug.print("c_abi.no_leaks: cannot open tests/malformed/ ({s})\n", .{@errorName(err)});
        return err;
    };
    defer malformed_dir.close(io);

    var iter = malformed_dir.iterate();
    while (try iter.next(io)) |entry| {
        if (entry.kind != .file) continue;
        const name = entry.name;
        if (!std.mem.endsWith(u8, name, ".deal") and !std.mem.endsWith(u8, name, ".dealx")) continue;
        if (std.mem.startsWith(u8, name, "_")) continue; // skip _manifest.txt etc.

        const path = try std.fmt.allocPrint(gpa, "tests/malformed/{s}", .{name});
        defer gpa.free(path);

        const source = cwd.readFileAlloc(io, path, gpa, .unlimited) catch |err| {
            std.debug.print("c_abi.no_leaks: read {s} failed ({s})\n", .{ path, @errorName(err) });
            continue; // skip unreadable files (binary content, e.g. m05)
        };
        defer gpa.free(source);

        const handle = try lib.deal_parse_internal(gpa, source, name);

        // Force both caches to populate before freeing.
        _ = json_mod.emitAst(handle.arena.allocator(), handle.ast_root, handle.mode, handle.filename) catch {};
        _ = json_mod.emitDiagnostics(handle.arena.allocator(), handle.diagnostics.items) catch {};

        lib.deal_free_internal(handle);
        file_count += 1;
    }

    std.debug.print("c_abi.no_leaks: {d} files parsed and freed (19 showcase + malformed corpus)\n", .{file_count});

    // After the loop, std.testing.allocator's leak detector runs automatically
    // at test exit. A non-zero count here would fail the test with:
    //   "allocator detected leaks"
    // The test passes only if every allocation has a matching free.
}
