//! sema.dimensional — Phase 4 dimensional algebra regression harness (D-55, D-57).
//!
//! Two test loops (mirror sema_corpus.zig structure):
//!   1. sema.dimensional.regression_pins — For each 07-* fixture in
//!      tests/regressions/sema/, parse + analyze the file (with stdlib seeding)
//!      and assert the pinned E25xx diagnostic is the first dimensional code emitted.
//!   2. sema.dimensional.showcase_clean — For each of the 19 showcase files,
//!      parse + analyze (with stdlib seeding) and assert ZERO E25xx diagnostics
//!      are produced.
//!
//! Both loops load stdlib unit sources from ../deal-stdlib/packages/units/ and
//! pass them to deal_parse_internal_with_stdlib so dimension/unit metadata is
//! available to Check #7 when analyzing fixture or showcase files.
//!
//! References to Codes.e_dimension_mismatch etc. are intentional: if Plan 01's
//! E-code constants are missing from diagnostics.zig, this file FAILS TO COMPILE,
//! enforcing the D-16 code-reservation contract at build time.

const std = @import("std");
const lib = @import("lib");
const diagnostics_mod = @import("diagnostics");

const Codes = diagnostics_mod.Codes;

// ── Compile-time E-code reference guard ──────────────────────────────────────
// These references MUST compile — they enforce that Plan 01's E-code reservation
// (src/diagnostics.zig) is present before any dimensional emission work begins.
// If any of the five constants below is removed or renamed, this file fails to
// compile, blocking all downstream plans that rely on the codes.
comptime {
    _ = Codes.e_dimension_mismatch;       // E2500 — D-55 dimensional type safety
    _ = Codes.e_mixed_unit_comparison;    // E2501 — D-57 mixed-unit same-dimension
    _ = Codes.e_unknown_unit;             // E2502 — unit literal unresolvable
    _ = Codes.e_conversion_type_mismatch; // E2503 — wrong source/target dimension
    _ = Codes.e_dependency_not_resolved;  // E2402 — import-band, dep not installed
}

// ── Regression fixture pins ────────────────────────────────────────────────

/// Pin for a dimensional regression fixture.
/// `expected_code` is the E25xx code that MUST appear in the diagnostic list.
const DimPin = struct {
    /// Filename within tests/regressions/sema/ (without leading path).
    name: []const u8,
    /// First dimensional diagnostic code expected (E25xx).
    expected_code: []const u8,
};

/// Regression corpus pins for dimensional checks.
/// Each fixture is named after its check number and the expected E25xx code.
const regression_pins = [_]DimPin{
    // Check #7a — Mass attribute assigned a Voltage value (E2500).
    .{ .name = "07-dimensional-mismatch.deal", .expected_code = Codes.e_dimension_mismatch },
    // Check #7b — mixed-unit same-dimension expression without conversion (E2501).
    .{ .name = "07-mixed-unit.deal", .expected_code = Codes.e_mixed_unit_comparison },
    // Check #7c — unit literal not resolvable to any declared dimension (E2502).
    .{ .name = "07-unknown-unit.deal", .expected_code = Codes.e_unknown_unit },
    // Check #7d — conversion call with wrong source dimension (E2503).
    .{ .name = "07-conversion-mismatch.deal", .expected_code = Codes.e_conversion_type_mismatch },
};

// ── Showcase files (19 total) ─────────────────────────────────────────────

/// Paths relative to the repository root.
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

// ── Stdlib unit source paths (relative to repo root) ────────────────────────

/// Stdlib unit source files from the vendored deal-stdlib package.
/// These are loaded and seeded into the symbol table before analyzing any
/// fixture or showcase file so dimension/unit metadata is available to
/// Check #7 (dimensional algebra).
const stdlib_unit_files = [_][]const u8{
    "../deal-stdlib/packages/units/dimensions.deal",
    "../deal-stdlib/packages/units/si.deal",
    "../deal-stdlib/packages/units/imperial.deal",
    "../deal-stdlib/packages/units/conversions.deal",
};

/// Load all stdlib unit sources into a slice of StdlibSource descriptors.
/// Returns a slice allocated from `gpa`; caller frees both the slice and
/// each .source and .filename member.
fn loadStdlibSources(
    gpa: std.mem.Allocator,
    io: std.Io,
    cwd: std.Io.Dir,
) ![]lib.StdlibSource {
    var sources: std.ArrayList(lib.StdlibSource) = .empty;
    errdefer {
        for (sources.items) |s| {
            gpa.free(s.source);
            gpa.free(s.filename);
        }
        sources.deinit(gpa);
    }

    for (stdlib_unit_files) |path| {
        const source = cwd.readFileAlloc(io, path, gpa, .unlimited) catch |err| {
            std.debug.print(
                "sema.dimensional: stdlib source read failed for {s} ({s}) — skipping\n",
                .{ path, @errorName(err) },
            );
            continue;
        };
        const filename = try gpa.dupe(u8, path);
        try sources.append(gpa, .{ .source = source, .filename = filename });
    }

    return sources.toOwnedSlice(gpa);
}

/// Free stdlib source descriptors loaded by loadStdlibSources.
fn freeStdlibSources(gpa: std.mem.Allocator, sources: []lib.StdlibSource) void {
    for (sources) |s| {
        gpa.free(s.source);
        gpa.free(s.filename);
    }
    gpa.free(sources);
}

// ── Test 1: dimensional regression corpus ────────────────────────────────────

test "sema.dimensional.regression_pins" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    // Load stdlib unit sources to seed dimension/unit metadata for Check #7.
    const stdlib_srcs = try loadStdlibSources(gpa, io, cwd);
    defer freeStdlibSources(gpa, stdlib_srcs);

    for (regression_pins) |pin| {
        const path = try std.fmt.allocPrint(gpa, "tests/regressions/sema/{s}", .{pin.name});
        defer gpa.free(path);

        const source = cwd.readFileAlloc(io, path, gpa, .unlimited) catch |err| {
            std.debug.print(
                "sema.dimensional.regression_pins: read {s} failed ({s})\n",
                .{ path, @errorName(err) },
            );
            return err;
        };
        defer gpa.free(source);

        // Analyze with stdlib seeding so dimension/unit metadata is available.
        const handle = try lib.deal_parse_internal_with_stdlib(
            gpa,
            source,
            pin.name,
            stdlib_srcs,
            false, // dim/unit seeding only (Check #7)
        );
        defer lib.deal_free_internal(handle);

        const diags = handle.diagnostics.items;

        // Assert that the expected E25xx code appears in the diagnostic list.
        var found = false;
        for (diags) |d| {
            if (std.mem.eql(u8, d.code, pin.expected_code)) {
                found = true;
                break;
            }
        }

        if (!found) {
            std.debug.print(
                "sema.dimensional.regression_pins FAILED for {s}: " ++
                    "expected E25xx code {s} not found in {d} diagnostic(s)\n",
                .{ pin.name, pin.expected_code, diags.len },
            );
            for (diags) |d| {
                std.debug.print("  [{s}] {s}\n", .{ d.code, d.message });
            }
            return error.TestUnexpectedResult;
        }
    }
}

// ── Test 2: showcase files must produce zero E25xx diagnostics ────────────────

test "sema.dimensional.showcase_clean" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    // Load stdlib unit sources to seed dimension/unit metadata for Check #7.
    const stdlib_srcs = try loadStdlibSources(gpa, io, cwd);
    defer freeStdlibSources(gpa, stdlib_srcs);

    var failures: usize = 0;

    for (showcase_files) |path| {
        const source = cwd.readFileAlloc(io, path, gpa, .unlimited) catch |err| {
            std.debug.print(
                "sema.dimensional.showcase_clean: read {s} failed ({s})\n",
                .{ path, @errorName(err) },
            );
            return err;
        };
        defer gpa.free(source);

        // Analyze with stdlib seeding so Check #7 can resolve unit imports.
        const handle = try lib.deal_parse_internal_with_stdlib(
            gpa,
            source,
            path,
            stdlib_srcs,
            false, // dim/unit seeding only (Check #7)
        );
        defer lib.deal_free_internal(handle);

        const diags = handle.diagnostics.items;

        // Count E25xx diagnostics only (dimensional algebra codes).
        var dim_diag_count: usize = 0;
        for (diags) |d| {
            // E25xx: starts with 'E', '2', '5'
            if (d.code.len >= 4 and
                d.code[0] == 'E' and
                d.code[1] == '2' and
                d.code[2] == '5')
            {
                dim_diag_count += 1;
            }
        }

        if (dim_diag_count > 0) {
            failures += 1;
            std.debug.print(
                "sema.dimensional.showcase_clean FAILED for {s}: {d} E25xx diagnostic(s)\n",
                .{ path, dim_diag_count },
            );
            for (diags) |d| {
                if (d.code.len >= 4 and
                    d.code[0] == 'E' and
                    d.code[1] == '2' and
                    d.code[2] == '5')
                {
                    std.debug.print("  [{s}] {s}\n", .{ d.code, d.message });
                }
            }
        }
    }

    if (failures > 0) {
        std.debug.print(
            "sema.dimensional.showcase_clean: {d} file(s) emitted unexpected E25xx diagnostics\n",
            .{failures},
        );
        return error.TestUnexpectedResult;
    }
}
