//! expr.precedence — 8-level Pratt binding-power gate (Plan 03 / D-08).
//!
//! Parses short expression sources and asserts the resulting tree shape
//! matches the documented BP table:
//!
//!   Level | Operators         | Left BP
//!   ──────────────────────────────────
//!     1   | OR                |   10
//!     2   | AND               |   20
//!     3   | == !=             |   30
//!     4   | < <= > >=         |   40
//!     5   | + -               |   50
//!     6   | * /               |   60
//!     7   | unary (- NOT)     |   (prefix, no left BP)
//!     8   | . (member access) |   80
//!         | ( (call)          |   80

const std = @import("std");
const ast = @import("ast");
const diagnostics = @import("diagnostics");
const parser_deal = @import("parser_deal");
const expr = @import("expr");

/// Full harness: returns (arena, node). Caller must call arena.deinit().
fn parseExprArena(
    backing: std.mem.Allocator,
    src: []const u8,
) !struct { arena: std.heap.ArenaAllocator, node: *ast.Node } {
    var arena = std.heap.ArenaAllocator.init(backing);
    errdefer arena.deinit();
    const arena_alloc = arena.allocator();
    var diags: std.ArrayList(diagnostics.Diagnostic) = .empty;
    var p = parser_deal.Parser{
        .arena = arena_alloc,
        .lex = @import("lexer").Lexer.init(src),
        .diags = &diags,
        .source = src,
    };
    const node = try expr.parseExpression(&p, 0);
    return .{ .arena = arena, .node = node };
}

// Helper: assert a node has a given kind.
fn assertKind(node: *ast.Node, expected: ast.NodeKind) !void {
    try std.testing.expectEqual(expected, node.kind);
}

// Helper: assert binary node has given op, return lhs+rhs.
fn assertBinary(node: *ast.Node, expected_op: ast.BinaryOp) !struct { lhs: *ast.Node, rhs: *ast.Node } {
    try std.testing.expectEqual(ast.NodeKind.binary, node.kind);
    try std.testing.expectEqual(expected_op, node.payload.binary.op);
    return .{ .lhs = node.payload.binary.lhs, .rhs = node.payload.binary.rhs };
}

// Helper: assert unary node has given op, return operand.
fn assertUnary(node: *ast.Node, expected_op: ast.UnaryOp) !*ast.Node {
    try std.testing.expectEqual(ast.NodeKind.unary, node.kind);
    try std.testing.expectEqual(expected_op, node.payload.unary.op);
    return node.payload.unary.operand;
}

// Helper: assert identifier has expected name.
fn assertIdent(node: *ast.Node, name: []const u8) !void {
    try std.testing.expectEqual(ast.NodeKind.identifier, node.kind);
    try std.testing.expectEqualStrings(name, node.payload.identifier.name);
}

test "expr.precedence" {
    const gpa = std.testing.allocator;

    // Level 1 vs 2: OR (BP 10) < AND (BP 20)
    // "a OR b AND c" → Binary(or, a, Binary(and, b, c))
    {
        var r = try parseExprArena(gpa, "a OR b AND c");
        defer r.arena.deinit();
        const top = try assertBinary(r.node, .log_or);
        try assertIdent(top.lhs, "a");
        const inner = try assertBinary(top.rhs, .log_and);
        try assertIdent(inner.lhs, "b");
        try assertIdent(inner.rhs, "c");
    }

    // Level 2 vs 3: AND (BP 20) < == (BP 30)
    // "a AND b == c" → Binary(and, a, Binary(eq, b, c))
    {
        var r = try parseExprArena(gpa, "a AND b == c");
        defer r.arena.deinit();
        const top = try assertBinary(r.node, .log_and);
        try assertIdent(top.lhs, "a");
        const inner = try assertBinary(top.rhs, .eq);
        try assertIdent(inner.lhs, "b");
        try assertIdent(inner.rhs, "c");
    }

    // Level 3 vs 4: == (BP 30) < > (BP 40)
    // "a == b > c" → Binary(eq, a, Binary(gt, b, c))
    {
        var r = try parseExprArena(gpa, "a == b > c");
        defer r.arena.deinit();
        const top = try assertBinary(r.node, .eq);
        try assertIdent(top.lhs, "a");
        const inner = try assertBinary(top.rhs, .gt);
        try assertIdent(inner.lhs, "b");
        try assertIdent(inner.rhs, "c");
    }

    // Level 4 vs 5: > (BP 40) < + (BP 50)
    // "a > b + c" → Binary(gt, a, Binary(add, b, c))
    {
        var r = try parseExprArena(gpa, "a > b + c");
        defer r.arena.deinit();
        const top = try assertBinary(r.node, .gt);
        try assertIdent(top.lhs, "a");
        const inner = try assertBinary(top.rhs, .add);
        try assertIdent(inner.lhs, "b");
        try assertIdent(inner.rhs, "c");
    }

    // Level 5 vs 6: + (BP 50) < * (BP 60)
    // "a + b * c" → Binary(add, a, Binary(mul, b, c))
    {
        var r = try parseExprArena(gpa, "a + b * c");
        defer r.arena.deinit();
        const top = try assertBinary(r.node, .add);
        try assertIdent(top.lhs, "a");
        const inner = try assertBinary(top.rhs, .mul);
        try assertIdent(inner.lhs, "b");
        try assertIdent(inner.rhs, "c");
    }

    // Level 6 vs 8: * (BP 60) < postfix (BP 80)
    // "a * b.c(d)" → Binary(mul, a, Call(MemberAccess(b,"c"), [d]))
    {
        var r = try parseExprArena(gpa, "a * b.c(d)");
        defer r.arena.deinit();
        const top = try assertBinary(r.node, .mul);
        try assertIdent(top.lhs, "a");
        // rhs should be a Call
        try assertKind(top.rhs, .call);
        const call = top.rhs.payload.call;
        // callee should be MemberAccess(b, "c")
        try assertKind(call.callee, .member_access);
        const ma = call.callee.payload.member_access;
        try assertIdent(ma.receiver, "b");
        try std.testing.expectEqualStrings("c", ma.member);
        // one arg: d
        try std.testing.expectEqual(@as(usize, 1), call.args.len);
        try assertIdent(call.args[0], "d");
    }

    // Unary NOT: "NOT a == b"
    // NOT has effective prefix BP 70. After parsing NOT, parseExpression(70)
    // sees "a == b". Since == has BP 30 < 70, it stops at "a".
    // So result: Binary(eq, Unary(not, a), b)
    {
        var r = try parseExprArena(gpa, "NOT a == b");
        defer r.arena.deinit();
        const top = try assertBinary(r.node, .eq);
        const lhs_unary = try assertUnary(top.lhs, .not);
        try assertIdent(lhs_unary, "a");
        try assertIdent(top.rhs, "b");
    }

    // Postfix vs unary: "-a.b(c)" → Unary(neg, Call(MemberAccess(a,"b"), [c]))
    // Postfix BP 80 > unary BP 70, so after parsing the prefix `-`,
    // the Pratt loop at BP 70 sees `.` (BP 80) and wraps `a` in member_access,
    // then sees `(` (BP 80) and wraps it in a call.
    {
        var r = try parseExprArena(gpa, "-a.b(c)");
        defer r.arena.deinit();
        const operand = try assertUnary(r.node, .neg);
        try assertKind(operand, .call);
        const call = operand.payload.call;
        try assertKind(call.callee, .member_access);
        const ma = call.callee.payload.member_access;
        try assertIdent(ma.receiver, "a");
        try std.testing.expectEqualStrings("b", ma.member);
        try std.testing.expectEqual(@as(usize, 1), call.args.len);
        try assertIdent(call.args[0], "c");
    }
}
