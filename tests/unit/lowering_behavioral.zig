//! lowering.behavioral — Stage-2 S2.5b acceptance gate.
//!
//! Lowers inline behavioral sources through the PUBLIC C ABI (deal_parse →
//! deal_ir_json) and asserts the emitted IR JSON contains every expected
//! behavioral node/edge kind, including the §4 injected control nodes
//! (decision+merge, fork+join). Proves AST → IR v0.1 lowering + desugaring.

const std = @import("std");
const lib = @import("lib");

fn lowerAndCheck(source: []const u8, needles: []const []const u8) !void {
    const name = "behavioral_lower.deal";
    const handle = lib.deal_parse(source.ptr, source.len, name.ptr, name.len);
    try std.testing.expect(handle != null);
    defer lib.deal_free(handle);

    var ptr: [*]const u8 = undefined;
    var len: usize = 0;
    try std.testing.expect(lib.deal_ir_json(handle, &ptr, &len));
    const ir_json = ptr[0..len];

    for (needles) |n| {
        if (std.mem.indexOf(u8, ir_json, n) == null) {
            std.debug.print(
                "lowering.behavioral FAILED: missing `{s}` in IR JSON.\nsource:\n{s}\nIR:\n{s}\n",
                .{ n, source, ir_json },
            );
            return error.MissingIrKind;
        }
    }
}

test "lowering.behavioral.action_graph" {
    const source =
        \\package p;
        \\action def A {
        \\    in soc : Real;
        \\    action requestTorque;
        \\    action deliverPower;
        \\    action measure;
        \\    start -> requestTorque -> done;
        \\    requestTorque -> decide { [soc >= 1] -> done [else] -> requestTorque };
        \\    deliverPower -> par { -> measure -> requestTorque } -> done;
        \\    loop while [soc < 10] { measure -> done; }
        \\    for c in cells { balance(c); }
        \\    send Overheat to controller;
        \\    accept PlugInserted;
        \\    assign soc := soc + 1;
        \\    deliverPower.delivered ~> measure.sample : Energy;
        \\    bind measure.sample = deliverPower.delivered;
        \\    node auth : authenticate;
        \\    succession auth -> measure;
        \\}
    ;
    try lowerAndCheck(source, &.{
        "\"kind\":\"pin\"",
        "\"kind\":\"action_usage\"",
        "\"kind\":\"control_node\"",
        "\"kind\":\"succession\"",
        "\"kind\":\"decision_node\"",
        "\"kind\":\"merge_node\"",
        "\"kind\":\"fork_node\"",
        "\"kind\":\"join_node\"",
        "\"kind\":\"while_loop_action\"",
        "\"kind\":\"for_loop_action\"",
        "\"kind\":\"perform_action\"",
        "\"kind\":\"send_action\"",
        "\"kind\":\"accept_action\"",
        "\"kind\":\"assign_action\"",
        "\"kind\":\"item_flow\"",
        "\"kind\":\"binding\"",
        "\"implicit\":true",
    });
}

test "lowering.behavioral.state_machine" {
    const source =
        \\package p;
        \\state def S {
        \\    state Idle;
        \\    state Active;
        \\    entry / startCharge();
        \\    start -> Idle;
        \\    on TempHigh [t > 5] / shutdown() -> Active;
        \\    Active -> terminate;
        \\}
    ;
    try lowerAndCheck(source, &.{
        "\"kind\":\"state_usage\"",
        "\"kind\":\"transition\"",
        "\"kind\":\"subaction\"",
        "\"kind\":\"terminate_action\"",
        "\"kind\":\"perform_action\"",
        "\"subaction_kind\":\"entry\"",
    });
}

// IR v0.2 (S3.2): structured behavioral expressions. Guards, assignment values,
// and perform calls lower to operator_expr / feature_ref_expr / literal_expr /
// invocation_expr nodes (see spec/ir/v0.2/expression-mapping.md §1).
test "lowering.behavioral.structured_expressions" {
    const source =
        \\package p;
        \\action def A {
        \\    in soc : Real;
        \\    action requestTorque;
        \\    action measure;
        \\    requestTorque -> decide { [battery.soc >= 80] -> done [else] -> measure };
        \\    assign soc := soc + 1;
        \\    for c in cells { balance(c); }
        \\}
    ;
    try lowerAndCheck(source, &.{
        // the four expression node kinds
        "\"kind\":\"operator_expr\"",
        "\"kind\":\"feature_ref_expr\"",
        "\"kind\":\"literal_expr\"",
        "\"kind\":\"invocation_expr\"",
        // operator symbols (KerML), from `>=` guard and `+` value
        "\"operator\":\">=\"",
        "\"operator\":\"+\"",
        // typed literal (the `80` in the guard)
        "\"literal_kind\":\"integer\"",
        "\"literal_value\":\"80\"",
        // dotted feature path → FeatureChain segments
        "\"referent_segments\":[\"battery\",\"soc\"]",
        // invocation callee for `balance(c)`
        "\"callee_ref\":\"balance\"",
    });
}
