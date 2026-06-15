//! parser.behavioral — Stage-2 S2.3 acceptance gate for the ActionBody /
//! StateBody surface (deal.ebnf §9b/§9c).
//!
//! Each case parses an inline .deal source through the internal parser API,
//! asserts ZERO diagnostics + a non-null root, then emits AST JSON and checks
//! that every expected behavioral node kind appears. This proves the parser
//! produces the S2.2 node kinds and that operator-driven LL(2) dispatch
//! (perform vs. succession vs. item-flow) resolves correctly — without
//! disturbing the fixed showcase snapshot corpus.

const std = @import("std");
const ast = @import("ast");
const json = @import("json");
const parser = @import("parser");
const diagnostics_mod = @import("diagnostics");

/// Parse `source`, assert clean parse, emit AST JSON, and assert each string
/// in `must_contain` is present. Caller owns nothing (per-call arena).
fn expectBehavioral(source: []const u8, must_contain: []const []const u8) !void {
    const gpa = std.testing.allocator;

    var arena = std.heap.ArenaAllocator.init(gpa);
    defer arena.deinit();
    const arena_alloc = arena.allocator();

    var diags: std.ArrayList(diagnostics_mod.Diagnostic) = .empty;
    const root = try parser.parseFile(arena_alloc, source, .deal, &diags);

    if (diags.items.len > 0) {
        std.debug.print("parser.behavioral FAILED: {d} diagnostic(s):\n", .{diags.items.len});
        for (diags.items) |d| {
            std.debug.print("  [{s}] {s} at [{d},{d}]\n", .{ d.code, d.message, d.span.start, d.span.end });
        }
        std.debug.print("source:\n{s}\n", .{source});
        return error.TestUnexpectedDiagnostics;
    }
    if (root == null) return error.TestUnexpectedNull;

    const actual = try json.emitAst(gpa, root, .deal, "behavioral.deal");
    defer gpa.free(actual);

    for (must_contain) |needle| {
        if (std.mem.indexOf(u8, actual, needle) == null) {
            std.debug.print(
                "parser.behavioral FAILED: expected `{s}` in AST JSON.\nsource:\n{s}\njson:\n{s}\n",
                .{ needle, source, actual },
            );
            return error.MissingNodeKind;
        }
    }
}

test "parser.behavioral.pins_and_succession" {
    try expectBehavioral(
        \\package p;
        \\action def A {
        \\    in soc : Real;
        \\    out energy : Real;
        \\    action requestTorque;
        \\    action deliverPower;
        \\    start -> requestTorque -> deliverPower -> done;
        \\}
    , &.{
        "\"k\":\"pin_decl\"",
        "\"k\":\"action_usage\"",
        "\"k\":\"succession_chain\"",
        "\"k\":\"control_ref\"",
    });
}

test "parser.behavioral.decide_par_loop_perform" {
    try expectBehavioral(
        \\package p;
        \\action def B {
        \\    action a;
        \\    a -> decide { [x >= 1] -> done [else] -> a };
        \\    monitorTemps -> par { -> heat -> cool } -> done;
        \\    loop while [x < 10] { a -> done; }
        \\    for c in cells { balance(c); }
        \\}
    , &.{
        "\"k\":\"decide_block\"",
        "\"k\":\"par_block\"",
        "\"k\":\"loop_statement\"",
        "\"k\":\"perform_statement\"",
    });
}

test "parser.behavioral.send_accept_assign_flow_bind" {
    try expectBehavioral(
        \\package p;
        \\action def C {
        \\    send Overheat to controller;
        \\    accept PlugInserted;
        \\    assign target := soc + 10;
        \\    deliverPower.delivered ~> measure.sample : Energy;
        \\    bind measure.sample = deliverPower.delivered;
        \\}
    , &.{
        "\"k\":\"send_action\"",
        "\"k\":\"accept_action\"",
        "\"k\":\"assign_action\"",
        "\"k\":\"item_flow_statement\"",
        "\"k\":\"binding_statement\"",
    });
}

test "parser.behavioral.escape_hatch" {
    try expectBehavioral(
        \\package p;
        \\action def D {
        \\    node a : authenticate;
        \\    succession a -> b;
        \\}
    , &.{
        "\"k\":\"escape_node\"",
        "\"k\":\"escape_succession\"",
    });
}

test "parser.behavioral.state_machine" {
    try expectBehavioral(
        \\package p;
        \\state def S {
        \\    entry / startCharge();
        \\    state Idle;
        \\    start -> Idle;
        \\    on TempHigh [temp > 5] / shutdown() -> Fault;
        \\    Fault -> done;
        \\}
    , &.{
        "\"k\":\"state_def\"",
        "\"k\":\"entry_do_exit\"",
        "\"k\":\"state_usage\"",
        "\"k\":\"transition_statement\"",
        "\"k\":\"target_ref\"",
        "\"k\":\"succession_chain\"",
    });
}
