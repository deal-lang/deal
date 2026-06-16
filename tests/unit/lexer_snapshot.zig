//! lexer.snapshot — the 19-file showcase gate.
//!
//! Lexes every `.deal` and `.dealx` file under `tests/showcase/` (a
//! symlink to `spec/examples/showcase/`) via `json.emitTokens` and either:
//!   - `-Dupdate-snapshots=true`: writes the JSON to
//!     `tests/snapshots/tokens/showcase__<path>.json` (printing the path
//!     to stderr so the developer can review the diff before committing).
//!   - default: compares byte-for-byte against the committed snapshot;
//!     on mismatch writes `<snapshot>.actual` and fails with
//!     `expectEqualStrings` so the diff is visible.
//!
//! Independent of the snapshot, the test asserts that every produced
//! JSON contains ZERO occurrences of `"k":"unknown"` — the locked
//! Phase 1 success criterion #1.

const std = @import("std");
const lexer = @import("lexer");
const ast = @import("ast");
const json = @import("json");
const build_options = @import("build_options");

const showcase_files = [_][]const u8{
    // 15 .deal files
    "packages/vehicle/battery.deal",
    "packages/vehicle/motor.deal",
    "packages/vehicle/behaviors.deal",
    "packages/vehicle/charging-states.deal",
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
    // 4 .dealx files
    "model/vehicle.dealx",
    "model/traceability.dealx",
    "model/variants/sedan.dealx",
    "model/variants/performance.dealx",
};

/// Map a showcase path like `packages/vehicle/battery.deal` to the
/// snapshot filename `showcase__packages__vehicle__battery.deal.json` by
/// replacing `/` with `__`.
fn snapshotName(allocator: std.mem.Allocator, source_path: []const u8) ![]u8 {
    const prefix = "showcase__";
    const suffix = ".json";
    var out: std.ArrayList(u8) = .empty;
    defer out.deinit(allocator);
    try out.appendSlice(allocator, prefix);
    for (source_path) |c| {
        if (c == '/') {
            try out.appendSlice(allocator, "__");
        } else {
            try out.append(allocator, c);
        }
    }
    try out.appendSlice(allocator, suffix);
    return try out.toOwnedSlice(allocator);
}

fn modeOf(path: []const u8) ast.Mode {
    if (std.mem.endsWith(u8, path, ".dealx")) return .dealx;
    return .deal;
}

test "lexer.snapshot" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    for (showcase_files) |rel| {
        const showcase_path = try std.fmt.allocPrint(
            gpa,
            "tests/showcase/{s}",
            .{rel},
        );
        defer gpa.free(showcase_path);

        const source = try cwd.readFileAlloc(io, showcase_path, gpa, .unlimited);
        defer gpa.free(source);

        const actual = try json.emitTokens(gpa, source, modeOf(rel));
        defer gpa.free(actual);

        // Gate criterion #1 — locked Phase 1 success criterion: every
        // showcase file lexes through the four-mode dispatch with zero
        // `.unknown` tokens.
        std.testing.expect(
            std.mem.indexOf(u8, actual, "\"k\":\"unknown\"") == null,
        ) catch |err| {
            std.debug.print(
                "lexer.snapshot FAILED unknown-gate on {s}\n",
                .{rel},
            );
            // Print first ~512 bytes of actual JSON for diagnosis.
            const preview_len = @min(actual.len, 512);
            std.debug.print("  first {d} bytes: {s}\n", .{ preview_len, actual[0..preview_len] });
            return err;
        };

        const snap_name = try snapshotName(gpa, rel);
        defer gpa.free(snap_name);

        const snap_path = try std.fmt.allocPrint(
            gpa,
            "tests/snapshots/tokens/{s}",
            .{snap_name},
        );
        defer gpa.free(snap_path);

        if (build_options.update_snapshots) {
            try cwd.writeFile(io, .{
                .sub_path = snap_path,
                .data = actual,
            });
            std.debug.print("[wrote snapshot] {s}\n", .{snap_path});
            continue;
        }

        // Read the committed snapshot and assert byte-equality.
        const expected = cwd.readFileAlloc(io, snap_path, gpa, .unlimited) catch |err| {
            std.debug.print(
                "lexer.snapshot FAILED reading expected snapshot {s}: {s}\n  hint: regenerate with `zig build test -Dupdate-snapshots=true -Dtest-filter=lexer.snapshot`\n",
                .{ snap_path, @errorName(err) },
            );
            return err;
        };
        defer gpa.free(expected);

        std.testing.expectEqualStrings(expected, actual) catch |err| {
            // Write actual to <snap>.actual for visual diffing.
            const actual_path = try std.fmt.allocPrint(
                gpa,
                "{s}.actual",
                .{snap_path},
            );
            defer gpa.free(actual_path);
            cwd.writeFile(io, .{
                .sub_path = actual_path,
                .data = actual,
            }) catch {};
            std.debug.print(
                "lexer.snapshot MISMATCH for {s}\n  diff: diff {s} {s}\n  regenerate: zig build test -Dupdate-snapshots=true -Dtest-filter=lexer.snapshot\n",
                .{ rel, snap_path, actual_path },
            );
            return err;
        };
    }
}

test "lexer.snapshot spans monotonic" {
    // Cross-check: for every showcase file the spans of consecutive
    // tokens must be non-decreasing (no token's start can be before the
    // previous token's end). This catches lexer regressions where the
    // pos cursor reverses.
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    for (showcase_files) |rel| {
        const showcase_path = try std.fmt.allocPrint(
            gpa,
            "tests/showcase/{s}",
            .{rel},
        );
        defer gpa.free(showcase_path);

        const source = try cwd.readFileAlloc(io, showcase_path, gpa, .unlimited);
        defer gpa.free(source);

        var lex = lexer.Lexer.init(source);
        const start_mode: lexer.Mode = if (std.mem.endsWith(u8, rel, ".dealx"))
            .dealx_outer
        else
            .deal_def;
        var prev_end: u32 = 0;
        while (true) {
            const t = lex.next(start_mode);
            std.testing.expect(t.span.start >= prev_end) catch |err| {
                std.debug.print(
                    "span regression in {s}: tag={s} span=[{d},{d}] prev_end={d}\n",
                    .{ rel, @tagName(t.tag), t.span.start, t.span.end, prev_end },
                );
                return err;
            };
            prev_end = t.span.end;
            if (t.tag == .eof) break;
        }
    }
}
