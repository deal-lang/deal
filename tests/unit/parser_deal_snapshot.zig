//! parser_deal.snapshot — 15-file AST JSON byte-stability gate (Plan 03).
//!
//! For each of the 15 .deal showcase files:
//!   - Parses the source via parser.parseFile (internal Zig API, not C ABI).
//!   - Emits AST JSON via json.emitAst.
//!   - Asserts zero diagnostics on the well-formed showcase corpus.
//!   - Asserts non-null root (every .deal file must produce a tree).
//!   - `-Dupdate-snapshots=true`: writes to tests/snapshots/ast/.
//!   - Default: byte-compares against committed snapshot; on mismatch writes
//!     <snapshot>.actual and fails.

const std = @import("std");
const ast = @import("ast");
const json = @import("json");
const parser = @import("parser");
const diagnostics_mod = @import("diagnostics");
const build_options = @import("build_options");

/// The 15 original .deal showcase files plus 3 Phase-05.2 oracle files (18 total).
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
    // Phase-05.2 oracle files (SD-21/22/23 — calc/constraint/precision)
    "packages/analysis/calcs.deal",
    "packages/analysis/constraints.deal",
    "packages/analysis/precision.deal",
};

/// Map "packages/vehicle/battery.deal" → "showcase__packages__vehicle__battery.deal.json"
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

test "parser_deal.snapshot" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    for (showcase_deal_files) |rel| {
        // Read source file.
        const showcase_path = try std.fmt.allocPrint(gpa, "tests/showcase/{s}", .{rel});
        defer gpa.free(showcase_path);
        const source = try cwd.readFileAlloc(io, showcase_path, gpa, .unlimited);
        defer gpa.free(source);

        // Parse via internal API using a per-file arena.
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        const arena_alloc = arena.allocator();

        var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
        const root = try parser.parseFile(arena_alloc, source, .deal, &diags);

        // Gate: every showcase .deal file must produce a non-null root.
        if (root == null) {
            std.debug.print(
                "parser_deal.snapshot FAILED: null root for {s}\n",
                .{rel},
            );
            return error.TestUnexpectedNull;
        }

        // Gate: well-formed showcase files must have zero parse diagnostics.
        if (diags.items.len > 0) {
            std.debug.print(
                "parser_deal.snapshot FAILED: {d} diagnostic(s) for {s}:\n",
                .{ diags.items.len, rel },
            );
            for (diags.items) |d| {
                std.debug.print("  [{s}] {s} at [{d},{d}]\n", .{
                    d.code, d.message, d.span.start, d.span.end,
                });
            }
            return error.TestUnexpectedDiagnostics;
        }

        // Emit AST JSON.
        const actual = try json.emitAst(gpa, root, .deal, rel);
        defer gpa.free(actual);

        // Compute snapshot path.
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

        // Read committed snapshot and byte-compare.
        const expected = cwd.readFileAlloc(io, snap_path, gpa, .unlimited) catch |err| {
            std.debug.print(
                "parser_deal.snapshot FAILED reading {s}: {s}\n  hint: regenerate with `zig build test -Dupdate-snapshots=true -Dtest-filter=parser_deal.snapshot`\n",
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
                "parser_deal.snapshot MISMATCH for {s}\n  diff: diff {s} {s}\n  regenerate: zig build test -Dupdate-snapshots=true -Dtest-filter=parser_deal.snapshot\n",
                .{ rel, snap_path, actual_path },
            );
            return err;
        };
    }
}
