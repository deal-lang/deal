//! sema.corpus — Phase 2 semantic analyzer regression tests.
//!
//! Two test loops:
//!   1. sema.corpus.regression — For each fixture in tests/regressions/sema/,
//!      parse + analyze the file and assert that at least one diagnostic with
//!      the pinned error code is emitted (SEMA-EXPECTED contract).
//!   2. sema.corpus.showcase_clean — For each of the 19 showcase files, parse +
//!      analyze and assert ZERO semantic diagnostics are produced. This is the
//!      "showcase files must always be sema-clean" contract (REQ-phase-2-1).
//!
//! Both tests use `deal_parse_internal` (the pub-but-NOT-exported test helper)
//! via the `lib` module so they exercise the same sema pipeline as production.
//!
//! Error-code constants come from `diagnostics.Codes` (D-16 namespace)
//! so a code renumber is caught at compile time rather than silently passing.

const std = @import("std");
const lib = @import("lib");
const diagnostics_mod = @import("diagnostics");

const Codes = diagnostics_mod.Codes;

// ── Regression fixture pins ────────────────────────────────────────────────

/// Pin for a regression fixture: the file MUST produce at least one
/// diagnostic whose code matches `expected_code`.
const Pin = struct {
    /// Filename within tests/regressions/sema/ (without leading path).
    name: []const u8,
    /// Primary diagnostic code that MUST appear in the diagnostic list.
    expected_code: []const u8,
};

/// Regression corpus pins.
/// Each fixture is named after its check number and expected diagnostic code.
const regression_pins = [_]Pin{
    // Check #1 — name resolution: <<specializes>> target not declared/imported.
    .{ .name = "01-name-resolution.deal", .expected_code = Codes.e_name_not_found },
    // Check #2 — type checking: attribute type annotation not declared/imported.
    .{ .name = "02-type-check.deal", .expected_code = Codes.e_type_mismatch },
    // Check #3 — multiplicity: lower bound exceeds upper bound.
    .{ .name = "03-multiplicity.deal", .expected_code = Codes.e_multiplicity_violation },
    // Check #4 — specialization cycle: A <<specializes>> A (self-loop).
    .{ .name = "04-specializes.deal", .expected_code = Codes.e_specialization_cycle },
    // Check #5 — trace target not found: @trace:<<satisfies>> GhostRequirement.
    .{ .name = "05-trace.deal", .expected_code = Codes.e_trace_target_not_found },
    // Check #6 — import resolution: bare single-segment import clashes with local decl.
    .{ .name = "06-import.deal", .expected_code = Codes.e_unresolved_import },
};

// ── Showcase files (19 total) ─────────────────────────────────────────────

/// Paths relative to the repository root.
const showcase_files = [_][]const u8{
    "tests/showcase/packages/vehicle/battery.deal",
    "tests/showcase/packages/vehicle/motor.deal",
    "tests/showcase/packages/vehicle/behaviors.deal",
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

// ── Test 1: regression corpus ─────────────────────────────────────────────

test "sema.corpus.regression" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    for (regression_pins) |pin| {
        const path = try std.fmt.allocPrint(gpa, "tests/regressions/sema/{s}", .{pin.name});
        defer gpa.free(path);

        const source = cwd.readFileAlloc(io, path, gpa, .unlimited) catch |err| {
            std.debug.print(
                "sema.corpus.regression: read {s} failed ({s})\n",
                .{ path, @errorName(err) },
            );
            return err;
        };
        defer gpa.free(source);

        const handle = try lib.deal_parse_internal(gpa, source, pin.name);
        defer lib.deal_free_internal(handle);

        const diags = handle.diagnostics.items;

        // Assert at least one diagnostic matches the pinned code.
        var found = false;
        for (diags) |d| {
            if (std.mem.eql(u8, d.code, pin.expected_code)) {
                found = true;
                break;
            }
        }

        if (!found) {
            std.debug.print(
                "sema.corpus.regression FAILED for {s}:\n" ++
                    "  expected a diagnostic with code {s}\n" ++
                    "  got {d} diagnostic(s):",
                .{ pin.name, pin.expected_code, diags.len },
            );
            for (diags) |d| {
                std.debug.print(" {s}", .{d.code});
            }
            std.debug.print("\n", .{});
            return error.TestUnexpectedResult;
        }
    }
}

// ── Test 2: showcase files must produce zero sema diagnostics ─────────────

test "sema.corpus.showcase_clean" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    var failures: usize = 0;

    for (showcase_files) |path| {
        const source = cwd.readFileAlloc(io, path, gpa, .unlimited) catch |err| {
            std.debug.print(
                "sema.corpus.showcase_clean: read {s} failed ({s})\n",
                .{ path, @errorName(err) },
            );
            return err;
        };
        defer gpa.free(source);

        const handle = try lib.deal_parse_internal(gpa, source, path);
        defer lib.deal_free_internal(handle);

        const diags = handle.diagnostics.items;

        // Count sema diagnostics only (E2xxx / W2xxx / H2xxx codes start with "E2", "W2", "H2").
        var sema_diag_count: usize = 0;
        for (diags) |d| {
            const is_sema_e = d.code.len >= 2 and d.code[0] == 'E' and d.code[1] == '2';
            const is_sema_w = d.code.len >= 2 and d.code[0] == 'W' and d.code[1] == '2';
            const is_sema_h = d.code.len >= 2 and d.code[0] == 'H' and d.code[1] == '2';
            if (is_sema_e or is_sema_w or is_sema_h) sema_diag_count += 1;
        }

        if (sema_diag_count > 0) {
            failures += 1;
            std.debug.print(
                "sema.corpus.showcase_clean FAILED for {s}: {d} sema diagnostic(s)\n",
                .{ path, sema_diag_count },
            );
            for (diags) |d| {
                const is_sema_e = d.code.len >= 2 and d.code[0] == 'E' and d.code[1] == '2';
                const is_sema_w = d.code.len >= 2 and d.code[0] == 'W' and d.code[1] == '2';
                const is_sema_h = d.code.len >= 2 and d.code[0] == 'H' and d.code[1] == '2';
                if (is_sema_e or is_sema_w or is_sema_h) {
                    std.debug.print("  [{s}] {s}\n", .{ d.code, d.message });
                }
            }
        }
    }

    if (failures > 0) {
        std.debug.print(
            "sema.corpus.showcase_clean: {d} file(s) failed sema-clean check\n",
            .{failures},
        );
        return error.TestUnexpectedResult;
    }
}
