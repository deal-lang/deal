//! Pratt expression parser — Plan 03 implementation (D-08).
//!
//! Implements the 8-level binding-power table from deal.ebnf §19 lines
//! 1549-1606 (RESEARCH §Pattern 3 lines 327-381):
//!
//!   Level | Operators         | Left BP | Right BP
//!   ──────────────────────────────────────────────
//!     1   | OR                |   10    |   11
//!     2   | AND               |   20    |   21
//!     3   | == !=             |   30    |   31
//!     4   | < <= > >=         |   40    |   41
//!     5   | + -               |   50    |   51
//!     6   | * /               |   60    |   61
//!     7   | unary (- NOT !)   |  (70 right-assoc prefix; not in leftBP)
//!     8   | . (member access) |   80    |  (consumed inline)
//!         | ( (call)          |   80    |  (consumed inline)
//!
//! All infix binary levels are LEFT-associative (right = left + 1).
//! Unary prefix has effective BP 70 (right-recursive with min_bp=70).
//! Postfix member-access and call have BP 80 (higher than all binary).
//!
//! Decision: `IDENT(args)` is ALWAYS parsed as a Call node at parse time.
//! Phase 2 semantic analysis reclassifies `V(3.7)` etc. as unit calls.
//! This avoids LL(2) ambiguity and keeps the parser single-token look-ahead.
//! (Documented in SUMMARY as deviation-from-plan recommendation.)

const std = @import("std");
const ast = @import("ast");
const lexer = @import("lexer");
const diagnostics_mod = @import("diagnostics");

// Forward declaration — Parser is defined in parser_deal.zig. Imported
// here by name to break the mutual-import cycle (expr ↔ parser_deal).
// Zig allows anytype parameters to achieve the same effect without
// importing the concrete type, which would create a cycle.
// We use a comptime-typed function so the Zig compiler resolves the
// concrete Parser type at call sites.

/// Return the left binding-power of an infix/postfix operator, or null
/// if the token cannot continue an infix expression.
pub fn leftBindingPower(tag: lexer.Tag) ?u8 {
    return switch (tag) {
        .kw_OR => 10,
        .kw_AND => 20,
        .eq_eq, .bang_eq => 30,
        .lt, .lt_eq, .gt, .gt_eq => 40,
        .plus, .minus => 50,
        .star, .slash => 60,
        // Postfix operators (highest binary precedence)
        .dot => 80,
        .l_paren => 80,
        else => null,
    };
}

/// Right binding-power for a given left BP. All binary levels are
/// left-associative → right = left + 1. Postfix operators are not
/// "right-recursive" in the traditional sense; they are handled inline
/// in the Pratt loop rather than by a recursive call.
pub fn rightBindingPower(left_bp: u8) u8 {
    return left_bp + 1;
}

/// Allocate a new Node in the parser's arena.
fn makeNode(allocator: std.mem.Allocator, kind: ast.NodeKind, span: ast.Span, payload: ast.Payload) !*ast.Node {
    const n = try allocator.create(ast.Node);
    n.* = .{ .kind = kind, .span = span, .payload = payload };
    return n;
}

/// Parse an expression with minimum binding-power `min_bp`.
/// `parser` must be a `*parser_deal.Parser` — typed as anytype to break
/// the import cycle. The Zig compiler resolves the concrete type at each
/// call site; calling this with an incorrect type is a compile error.
pub fn parseExpression(parser: anytype, min_bp: u8) std.mem.Allocator.Error!*ast.Node {
    // T-05-01 DoS bound. parseExpression / parsePrefix / parsePrimary form a
    // mutually-recursive cycle; bumping the counter here catches all three
    // paths since every prefix/primary path either re-enters parseExpression
    // or returns. Emit E0403 once at the boundary and return a sentinel
    // empty Identifier so the caller's loop can synchronize.
    parser.depth += 1;
    defer parser.depth -= 1;
    if (parser.depth > @TypeOf(parser.*).MAX_PARSE_DEPTH) {
        const tok = parser.peek();
        parser.diags.append(parser.arena, .{
            .code = "E0403",
            .severity = .err,
            .message = "expression / definition nesting too deep",
            .span = tok.span,
        }) catch {};
        return makeNode(parser.arena, .identifier, tok.span, .{
            .identifier = .{ .name = "" },
        });
    }

    var left = try parsePrefix(parser);

    while (true) {
        const tok = parser.peek();
        const lbp = leftBindingPower(tok.tag) orelse break;
        if (lbp < min_bp) break;

        if (tok.tag == .dot) {
            // Postfix member access: consume ".", then expect IDENT.
            //
            // Phase 1.5 plan 02 (m18 strictness): the previous Plan 03
            // implementation advanced the next token unconditionally and
            // sliced its bytes as the member name — silently accepting
            // `a.;` as a member-access whose "member" was ";". The grammar
            // (deal.ebnf §PostfixExpression line 1622) requires IDENT
            // after `.`. Now we peek the next token and emit E0121
            // (Codes.e_dangling_dot) on mismatch, then exit the Pratt
            // loop so the statement-tier sync handles the stray token.
            const dot_tok = parser.advance(); // consume "."
            const member_tok = parser.peek();
            if (member_tok.tag != .ident) {
                parser.diags.append(parser.arena, .{
                    .code = diagnostics_mod.Codes.e_dangling_dot,
                    .severity = .err,
                    .message = "expected identifier after `.`",
                    .span = dot_tok.span,
                }) catch {};
                // Do NOT advance — the statement-tier sync will pick up
                // the stray token (e.g. `;`). Exit the Pratt loop with
                // `left` as-is.
                break;
            }
            _ = parser.advance(); // consume the validated IDENT
            const member_text = parser.source[member_tok.span.start..member_tok.span.end];
            const span = ast.Span{ .start = left.span.start, .end = member_tok.span.end };
            left = try makeNode(parser.arena, .member_access, span, .{
                .member_access = .{
                    .receiver = left,
                    .member = member_text,
                },
            });
        } else if (tok.tag == .l_paren) {
            // Postfix function call: consume "(", parse arg list, consume ")".
            _ = parser.advance(); // consume "("
            const args = try parseArgList(parser);
            const close = try parser.expect(.r_paren);
            const span = ast.Span{ .start = left.span.start, .end = close.span.end };
            left = try makeNode(parser.arena, .call, span, .{
                .call = .{
                    .callee = left,
                    .args = args,
                },
            });
        } else {
            // Binary operator.
            const op_tok = parser.advance();
            const op = tokenToBinaryOp(op_tok.tag);
            const rbp = rightBindingPower(lbp);
            const right = try parseExpression(parser, rbp);
            const span = ast.Span{ .start = left.span.start, .end = right.span.end };
            left = try makeNode(parser.arena, .binary, span, .{
                .binary = .{
                    .op = op,
                    .lhs = left,
                    .rhs = right,
                },
            });
        }
    }

    return left;
}

/// Parse a prefix form: unary operator or primary.
fn parsePrefix(parser: anytype) std.mem.Allocator.Error!*ast.Node {
    const tok = parser.peek();
    switch (tok.tag) {
        .minus => {
            _ = parser.advance();
            const start = tok.span.start;
            // Unary prefix BP = 70 (right-recursive).
            const operand = try parseExpression(parser, 70);
            const span = ast.Span{ .start = start, .end = operand.span.end };
            return makeNode(parser.arena, .unary, span, .{
                .unary = .{ .op = .neg, .operand = operand },
            });
        },
        .kw_NOT => {
            _ = parser.advance();
            const start = tok.span.start;
            const operand = try parseExpression(parser, 70);
            const span = ast.Span{ .start = start, .end = operand.span.end };
            return makeNode(parser.arena, .unary, span, .{
                .unary = .{ .op = .not, .operand = operand },
            });
        },
        else => return parsePrimary(parser),
    }
}

/// Parse a primary expression (atom or grouping).
fn parsePrimary(parser: anytype) std.mem.Allocator.Error!*ast.Node {
    const tok = parser.advance();
    const allocator = parser.arena;

    switch (tok.tag) {
        .int_literal => {
            const text = parser.source[tok.span.start..tok.span.end];
            return makeNode(allocator, .int_literal, tok.span, .{
                .int_literal = .{ .text = text },
            });
        },
        .real_literal => {
            const text = parser.source[tok.span.start..tok.span.end];
            return makeNode(allocator, .real_literal, tok.span, .{
                .real_literal = .{ .text = text },
            });
        },
        .string_literal, .unrestricted_name => {
            // Slice off the surrounding quotes for the value.
            const raw = parser.source[tok.span.start..tok.span.end];
            const value = if (raw.len >= 2) raw[1 .. raw.len - 1] else raw;
            return makeNode(allocator, .string_literal, tok.span, .{
                .string_literal = .{ .value = value },
            });
        },
        .template_literal => {
            const raw = parser.source[tok.span.start..tok.span.end];
            // No-interpolation template literal — single text part.
            const text = if (raw.len >= 2) raw[1 .. raw.len - 1] else raw;
            var parts: std.ArrayList(ast.TemplatePart) = .empty;
            try parts.append(allocator, .{ .text = text });
            const owned = try parts.toOwnedSlice(allocator);
            return makeNode(allocator, .template_literal, tok.span, .{
                .template_literal = .{ .parts = owned },
            });
        },
        .template_head => {
            return parseTemplateLiteralFromHead(parser, tok);
        },
        .boolean_literal => {
            const text = parser.source[tok.span.start..tok.span.end];
            const value = std.mem.eql(u8, text, "true");
            return makeNode(allocator, .boolean_literal, tok.span, .{
                .boolean_literal = .{ .value = value },
            });
        },
        .ident => {
            // Check for function-call: peek next token.
            const next = parser.peek();
            if (next.tag == .l_paren) {
                // Function call — IDENT "(" args ")".
                // The Pratt loop also handles `(` as postfix BP=80, so we let
                // the main loop handle it. But here we've already consumed the
                // IDENT; we need to return the identifier node and let the loop
                // pick up the "(" as a postfix operator.
                // This is the correct behavior: return Identifier, then the
                // Pratt loop sees l_paren with BP=80 and wraps it in a Call.
                const name = parser.source[tok.span.start..tok.span.end];
                return makeNode(allocator, .identifier, tok.span, .{
                    .identifier = .{ .name = name },
                });
            } else {
                const name = parser.source[tok.span.start..tok.span.end];
                return makeNode(allocator, .identifier, tok.span, .{
                    .identifier = .{ .name = name },
                });
            }
        },
        .l_paren => {
            // Parenthesized expression.
            const inner = try parseExpression(parser, 0);
            const close = try parser.expect(.r_paren);
            // WR-08: extend inner.span to cover the surrounding parens so
            // diagnostic spans for parenthesized expressions point at the
            // whole `( ... )` form rather than just the inner sub-expression.
            // We deliberately avoid creating a wrapper grouping node — the
            // spec does not require one — but we DO surface the wider span
            // so LSP hover and underline ranges are correct.
            inner.span = .{ .start = tok.span.start, .end = close.span.end };
            return inner;
        },
        .l_bracket => {
            // Array literal `[expr, expr, ...]` (D-04). Single source of truth
            // for bracketed array expressions in both .deal and .dealx contexts.
            // Replaces parser_deal.parseVerificationArray and
            // parser_dealx.parseArrayLiteral (both deleted, D-04).
            // T-05.1-D01 DoS guard: loop terminates on .r_bracket OR .eof.
            // `tok` is the opening `[` — its span is used for E0402 below.
            var items: std.ArrayList(*ast.Node) = .empty;
            while (parser.peek().tag != .r_bracket and parser.peek().tag != .eof) {
                const item = try parseExpression(parser, 0);
                try items.append(allocator, item);
                if (parser.peek().tag == .comma) {
                    const comma_tok = parser.advance(); // consume ","
                    // WR-03: trailing-comma gate — `[a,]` is invalid, matching
                    // parseArgList strictness and the fmt.zig E0122 guard
                    // (invariant #5: "NO trailing commas").
                    if (parser.peek().tag == .r_bracket) {
                        parser.diags.append(parser.arena, .{
                            .code = diagnostics_mod.Codes.e_empty_arg_comma,
                            .severity = .err,
                            .message = "expected element after `,` (trailing comma is not permitted in array literals)",
                            .span = comma_tok.span,
                        }) catch {};
                        break; // close the array cleanly at the `]`
                    }
                }
            }
            // WR-01: if the loop exited on EOF rather than `]`, emit E0402
            // (e_unclosed_bracket) with the span of the opening `[` so the
            // diagnostic points at the unclosed delimiter, not EOF.
            if (parser.peek().tag == .eof) {
                parser.diags.append(parser.arena, .{
                    .code = diagnostics_mod.Codes.e_unclosed_bracket,
                    .severity = .err,
                    .message = "unclosed `[` — array literal is missing `]`",
                    .span = tok.span,
                }) catch {};
            }
            const close = try parser.expect(.r_bracket);
            const span = ast.Span{ .start = tok.span.start, .end = close.span.end };
            return makeNode(allocator, .array_literal, span, .{
                .array_literal = .{ .items = try items.toOwnedSlice(allocator) },
            });
        },
        else => {
            // Unexpected token — return an identifier node for error recovery.
            const name = parser.source[tok.span.start..tok.span.end];
            return makeNode(allocator, .identifier, tok.span, .{
                .identifier = .{ .name = name },
            });
        },
    }
}

/// Parse a template literal that has already consumed its TEMPLATE_HEAD token.
fn parseTemplateLiteralFromHead(parser: anytype, head_tok: lexer.Token) std.mem.Allocator.Error!*ast.Node {
    const allocator = parser.arena;
    var parts: std.ArrayList(ast.TemplatePart) = .empty;

    // HEAD text (content between opening ` and first ${)
    const head_raw = parser.source[head_tok.span.start..head_tok.span.end];
    // head_raw starts with ` and ends with ${. Strip those.
    const head_text = if (head_raw.len >= 3) head_raw[1 .. head_raw.len - 2] else "";
    try parts.append(allocator, .{ .text = head_text });

    var end_pos = head_tok.span.end;

    while (true) {
        // Parse the expression inside ${...}
        const expr = try parseExpression(parser, 0);
        try parts.append(allocator, .{ .expr = expr });
        end_pos = expr.span.end;

        // Next should be template_middle or template_tail.
        const next = parser.peek();
        if (next.tag == .template_middle) {
            const mid_tok = parser.advance();
            const mid_raw = parser.source[mid_tok.span.start..mid_tok.span.end];
            // middle: } ... ${ — strip leading } and trailing ${
            const mid_text = if (mid_raw.len >= 3) mid_raw[1 .. mid_raw.len - 2] else "";
            try parts.append(allocator, .{ .text = mid_text });
            end_pos = mid_tok.span.end;
        } else if (next.tag == .template_tail) {
            const tail_tok = parser.advance();
            const tail_raw = parser.source[tail_tok.span.start..tail_tok.span.end];
            // tail: } ... ` — strip leading } and trailing `
            const tail_text = if (tail_raw.len >= 2) tail_raw[1 .. tail_raw.len - 1] else "";
            try parts.append(allocator, .{ .text = tail_text });
            end_pos = tail_tok.span.end;
            break;
        } else {
            // Malformed template — break out.
            break;
        }
    }

    const owned = try parts.toOwnedSlice(allocator);
    const span = ast.Span{ .start = head_tok.span.start, .end = end_pos };
    return makeNode(allocator, .template_literal, span, .{
        .template_literal = .{ .parts = owned },
    });
}

/// Parse a comma-separated argument list until `)`.
/// Returns an owned slice of expression nodes.
///
/// Grammar (deal.ebnf §_ArgumentList line 1680):
///   _ArgumentList ::= Expression ( "," Expression )*
///
/// Phase 1.5 plan 02 (m19 strictness): the grammar disallows leading
/// commas (`f(,a)`), stray double commas (`f(a,,b)`), and trailing
/// commas (`f(a,)`) — every comma must sit BETWEEN two expressions.
/// All three shapes emit Codes.e_empty_arg_comma (E0122) at the
/// offending comma's span. The forward-progress invariant is preserved
/// by consuming the offending comma in every error branch (T-15-02-03
/// mitigation in PLAN threat_model).
pub fn parseArgList(parser: anytype) std.mem.Allocator.Error![]*ast.Node {
    var args: std.ArrayList(*ast.Node) = .empty;
    const allocator = parser.arena;

    const next = parser.peek();
    if (next.tag == .r_paren) {
        return try args.toOwnedSlice(allocator);
    }

    // m19 leading-comma gate: `f(,...)` is invalid per the grammar.
    // Loop so that double-leading-comma shapes (e.g. `f(,,a)`) re-enter
    // the gate cleanly on the second comma instead of falling through
    // to parseExpression — see Phase 1.5 IN-03 iteration-2 correction:
    // without this loop, `f(,,a)` would emit exactly ONE E0122 at the
    // first comma, then parsePrimary's else-arm would silently absorb
    // the second comma as an `.identifier` node whose name is the
    // bytes "," (loose recovery; introduces a phantom comma-named
    // identifier arg). The loop preserves forward progress because
    // every iteration consumes the offending comma via advance().
    while (parser.peek().tag == .comma) {
        const leading = parser.peek();
        parser.diags.append(parser.arena, .{
            .code = diagnostics_mod.Codes.e_empty_arg_comma,
            .severity = .err,
            .message = "expected argument before `,`",
            .span = leading.span,
        }) catch {};
        _ = parser.advance(); // consume the offending comma (forward progress)
        const after = parser.peek();
        // If the call is `f(,)`, the next token is r_paren — return empty.
        if (after.tag == .r_paren) {
            return try args.toOwnedSlice(allocator);
        }
        // If the next token is ALSO a comma (`f(,,...)`), re-enter the
        // gate to fire a second E0122 at the second comma's span.
        if (after.tag == .comma) continue;
        // Otherwise fall through and parse the first real expression.
        break;
    }

    const first = try parseExpression(parser, 0);
    try args.append(allocator, first);

    while (parser.peek().tag == .comma) {
        const comma_tok = parser.advance(); // consume ","
        const after = parser.peek();
        // m19 double-comma gate: `f(a,,b)` is invalid.
        // m19 trailing-comma gate: `f(a,)` is invalid (grammar §_ArgumentList
        // requires Expression after every comma — no trailing comma allowed).
        if (after.tag == .comma or after.tag == .r_paren) {
            parser.diags.append(parser.arena, .{
                .code = diagnostics_mod.Codes.e_empty_arg_comma,
                .severity = .err,
                .message = if (after.tag == .r_paren)
                    "expected argument after `,` (trailing comma is not permitted)"
                else
                    "expected argument between commas",
                .span = comma_tok.span,
            }) catch {};
            // If we hit r_paren, exit the loop — the call closes here.
            if (after.tag == .r_paren) break;
            // Double-comma: skip back to top of loop to consume the next
            // comma (the offending comma we emitted at is already consumed;
            // the next peek().tag == .comma will re-fire the gate cleanly).
            continue;
        }
        const arg = try parseExpression(parser, 0);
        try args.append(allocator, arg);
    }

    return try args.toOwnedSlice(allocator);
}

/// Convert a lexer Tag to its BinaryOp representation.
fn tokenToBinaryOp(tag: lexer.Tag) ast.BinaryOp {
    return switch (tag) {
        .plus => .add,
        .minus => .sub,
        .star => .mul,
        .slash => .div,
        .eq_eq => .eq,
        .bang_eq => .neq,
        .lt => .lt,
        .lt_eq => .le,
        .gt => .gt,
        .gt_eq => .ge,
        .kw_AND => .log_and,
        .kw_OR => .log_or,
        else => .add, // fallback — shouldn't reach
    };
}
