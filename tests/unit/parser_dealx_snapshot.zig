//! parser_dealx.snapshot — 4-file .dealx AST JSON byte-stability gate.
//!
//! For each of the 4 .dealx showcase files:
//!   - Parses via parser_dealx.parseFile (internal Zig API, not C ABI).
//!   - Emits AST JSON via json.emitAst with mode=.dealx.
//!   - Asserts non-null root and zero parse diagnostics (showcase is
//!     well-formed per Phase 0).
//!   - `-Dupdate-snapshots=true`: writes to tests/snapshots/ast/.
//!   - Default: byte-compares against committed snapshot; on mismatch writes
//!     <snapshot>.actual and fails.

const std = @import("std");
const ast = @import("ast");
const json = @import("json");
const parser = @import("parser");
const diagnostics_mod = @import("diagnostics");
const build_options = @import("build_options");

const showcase_dealx_files = [_][]const u8{
    "model/vehicle.dealx",
    "model/traceability.dealx",
    "model/variants/sedan.dealx",
    "model/variants/performance.dealx",
};

fn snapshotName(allocator: std.mem.Allocator, source_path: []const u8) ![]u8 {
    var out: std.ArrayList(u8) = .empty;
    defer out.deinit(allocator);
    try out.appendSlice(allocator, "showcase__");
    for (source_path) |c| {
        if (c == '/') {
            try out.appendSlice(allocator, "__");
        } else {
            try out.append(allocator, c);
        }
    }
    try out.appendSlice(allocator, ".json");
    return try out.toOwnedSlice(allocator);
}

test "parser_dealx.snapshot" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    for (showcase_dealx_files) |rel| {
        const showcase_path = try std.fmt.allocPrint(gpa, "tests/showcase/{s}", .{rel});
        defer gpa.free(showcase_path);
        const source = try cwd.readFileAlloc(io, showcase_path, gpa, .unlimited);
        defer gpa.free(source);

        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        const arena_alloc = arena.allocator();

        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const root = try parser.parseFile(arena_alloc, source, .dealx, &diags);

        if (root == null) {
            std.debug.print(
                "parser_dealx.snapshot FAILED: null root for {s}\n",
                .{rel},
            );
            return error.TestUnexpectedNull;
        }

        if (diags.items.len > 0) {
            std.debug.print(
                "parser_dealx.snapshot FAILED: {d} diagnostic(s) for {s}:\n",
                .{ diags.items.len, rel },
            );
            for (diags.items) |d| {
                std.debug.print("  [{s}] {s} at [{d},{d}]\n", .{
                    d.code, d.message, d.span.start, d.span.end,
                });
            }
            return error.TestUnexpectedDiagnostics;
        }

        const actual = try json.emitAst(gpa, root, .dealx, rel);
        defer gpa.free(actual);

        const snap_name = try snapshotName(gpa, rel);
        defer gpa.free(snap_name);
        const snap_path = try std.fmt.allocPrint(gpa, "tests/snapshots/ast/{s}", .{snap_name});
        defer gpa.free(snap_path);

        if (build_options.update_snapshots) {
            try cwd.writeFile(io, .{
                .sub_path = snap_path,
                .data = actual,
            });
            std.debug.print("[wrote AST snapshot] {s}\n", .{snap_path});
            continue;
        }

        const expected = cwd.readFileAlloc(io, snap_path, gpa, .unlimited) catch |err| {
            std.debug.print(
                "parser_dealx.snapshot FAILED reading {s}: {s}\n  hint: regenerate with `zig build test -Dupdate-snapshots=true -Dtest-filter=parser_dealx.snapshot`\n",
                .{ snap_path, @errorName(err) },
            );
            return err;
        };
        defer gpa.free(expected);

        std.testing.expectEqualStrings(expected, actual) catch |err| {
            const actual_path = try std.fmt.allocPrint(gpa, "{s}.actual", .{snap_path});
            defer gpa.free(actual_path);
            cwd.writeFile(io, .{
                .sub_path = actual_path,
                .data = actual,
            }) catch {};
            std.debug.print(
                "parser_dealx.snapshot MISMATCH for {s}\n  diff: diff {s} {s}\n  regenerate: zig build test -Dupdate-snapshots=true -Dtest-filter=parser_dealx.snapshot\n",
                .{ rel, snap_path, actual_path },
            );
            return err;
        };
    }
}
