//! sema.behavioral — Stage-2 S2.4 acceptance gate for behavioral resolution.
//!
//! Drives the full parse+sema pipeline (lib.deal_parse_internal) over behavioral
//! sources and asserts:
//!   - a well-formed action/state body emits NO behavioral resolution diagnostics;
//!   - an undeclared succession / transition target → E2700;
//!   - an unknown pin type → E2100 (type_mismatch, reused);
//!   - an unknown item-flow `: FlowType` → E2701.

const std = @import("std");
const lib = @import("lib");
const diagnostics_mod = @import("diagnostics");

const Codes = diagnostics_mod.Codes;

// Compile-time guard: the S2.4 E-codes must exist before resolution work.
comptime {
    _ = Codes.e_behavioral_step_not_found; // E2700
    _ = Codes.e_behavioral_flow_type_not_found; // E2701
}

fn analyzeSource(gpa: std.mem.Allocator, source: []const u8, filename: []const u8) !*lib.DealHandle {
    return lib.deal_parse_internal(gpa, source, filename);
}

fn assertHasDiag(handle: *lib.DealHandle, code: []const u8) !void {
    for (handle.diagnostics.items) |d| {
        if (std.mem.eql(u8, d.code, code)) return;
    }
    std.debug.print("sema.behavioral FAIL: expected diagnostic {s} but got:\n", .{code});
    for (handle.diagnostics.items) |d| std.debug.print("  [{s}] {s}\n", .{ d.code, d.message });
    return error.TestUnexpectedResult;
}

fn assertNoDiag(handle: *lib.DealHandle, code: []const u8) !void {
    for (handle.diagnostics.items) |d| {
        if (std.mem.eql(u8, d.code, code)) {
            std.debug.print("sema.behavioral FAIL: expected NO {s} but found: [{s}] {s}\n", .{ code, d.code, d.message });
            return error.TestUnexpectedResult;
        }
    }
}

test "sema.behavioral.wellformed_no_behavioral_diags" {
    const gpa = std.testing.allocator;
    const source =
        \\package test.behavioral;
        \\action def A {
        \\    in soc : Real;
        \\    out energy : Real;
        \\    action requestTorque;
        \\    action deliverPower;
        \\    action measure;
        \\    start -> requestTorque -> deliverPower -> done;
        \\    deliverPower.delivered ~> measure.sample : Energy;
        \\}
        \\state def S {
        \\    state Idle;
        \\    state Fault;
        \\    entry / startCharge();
        \\    start -> Idle;
        \\    on TempHigh [temp > 5] / shutdown() -> Fault;
        \\    Fault -> done;
        \\}
    ;
    const handle = try analyzeSource(gpa, source, "wellformed.deal");
    defer lib.deal_free_internal(handle);

    try assertNoDiag(handle, Codes.e_behavioral_step_not_found);
    try assertNoDiag(handle, Codes.e_behavioral_flow_type_not_found);
    try assertNoDiag(handle, Codes.e_type_mismatch);
}

test "sema.behavioral.undeclared_succession_step" {
    const gpa = std.testing.allocator;
    const source =
        \\package test.behavioral;
        \\action def A {
        \\    action a;
        \\    a -> nonexistent -> done;
        \\}
    ;
    const handle = try analyzeSource(gpa, source, "undeclared_step.deal");
    defer lib.deal_free_internal(handle);
    try assertHasDiag(handle, Codes.e_behavioral_step_not_found);
}

test "sema.behavioral.unknown_pin_type" {
    const gpa = std.testing.allocator;
    const source =
        \\package test.behavioral;
        \\action def B {
        \\    in x : Nonexistent;
        \\}
    ;
    const handle = try analyzeSource(gpa, source, "unknown_pin_type.deal");
    defer lib.deal_free_internal(handle);
    try assertHasDiag(handle, Codes.e_type_mismatch);
}

test "sema.behavioral.unknown_flow_type" {
    const gpa = std.testing.allocator;
    const source =
        \\package test.behavioral;
        \\action def C {
        \\    action a;
        \\    action b;
        \\    a.x ~> b.y : Nonexistent;
        \\}
    ;
    const handle = try analyzeSource(gpa, source, "unknown_flow_type.deal");
    defer lib.deal_free_internal(handle);
    try assertHasDiag(handle, Codes.e_behavioral_flow_type_not_found);
}

test "sema.behavioral.undeclared_transition_target" {
    const gpa = std.testing.allocator;
    const source =
        \\package test.behavioral;
        \\state def S {
        \\    state Idle;
        \\    on T -> Ghost;
        \\}
    ;
    const handle = try analyzeSource(gpa, source, "undeclared_transition.deal");
    defer lib.deal_free_internal(handle);
    try assertHasDiag(handle, Codes.e_behavioral_step_not_found);
}
