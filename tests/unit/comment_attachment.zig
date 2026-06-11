//! comment_attachment — TDD-first tests for gofmt-style comment attachment
//! to AST declaration nodes (D-28, D-29, D-31).
//!
//! Covers six edge cases from RESEARCH.md §Pattern 2:
//!   Case 1: line comment immediately before decl → leading_comments
//!   Case 2: line comment separated by blank line → NOT leading (dropped in Plan 02-01)
//!   Case 3: block comment immediately before decl → leading_comments
//!   Case 4: doc comment (/**/) immediately before decl → doc field (old path);
//!           leading_comments stays empty (D-29)
//!   Case 5: multiple consecutive comments with no blank line → all attach
//!   Case 6: blank line in middle of comment group → only contiguous group attaches
//!
//! Each test parses a minimal .deal snippet and inspects the comment fields on
//! the produced definition node. Uses parser_deal.parseFile (the internal entry
//! point) directly — NOT the C ABI — per the same pattern as recovery_statement.zig.

const std = @import("std");
const ast = @import("ast");
const parser_deal = @import("parser_deal");
const diagnostics_mod = @import("diagnostics");

const Diagnostic = diagnostics_mod.Diagnostic;

/// Parse a .deal source snippet. diags is arena-backed (use .empty initializer).
fn parse(allocator: std.mem.Allocator, source: []const u8, diags: *std.ArrayList(Diagnostic)) !?*ast.Node {
    return parser_deal.parseFile(allocator, source, diags);
}

/// Extract the first top-level definition node from a deal_file root.
fn firstDef(root: *ast.Node) ?*ast.Node {
    const defs = root.payload.deal_file.definitions;
    if (defs.len == 0) return null;
    return defs[0];
}

/// Helper: extract ElementDef from any definition node.
fn elemDef(node: *ast.Node) *ast.ElementDef {
    return switch (node.payload) {
        .part_def => |*e| e,
        .port_def => |*e| e,
        .action_def => |*e| e,
        .state_def => |*e| e,
        .attribute_def => |*e| e,
        .item_def => |*e| e,
        .interface_def => |*e| e,
        .connection_def => |*e| e,
        .flow_def => |*e| e,
        .allocation_def => |*e| e,
        .requirement_def => |*e| e,
        // constraint_def uses ConstraintDefinition (Phase 05.2), not ElementDef
        .need_def => |*e| e,
        .use_case_def => |*e| e,
        else => unreachable,
    };
}

test "comment_attachment.leading_line_comment" {
    // ─── Case 1: // line comment immediately before decl (no blank line) ───
    // Expected: leading_comments = [Comment{kind=line, ...}], doc_comment = null
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const alloc = arena.allocator();

    var diags: std.ArrayList(Diagnostic) = .empty;
    const source =
        \\// leading comment
        \\part def Foo {}
    ;
    const root = (try parse(alloc, source, &diags)) orelse {
        return error.ParseFailed;
    };
    try std.testing.expectEqual(@as(usize, 0), diags.items.len);

    const def = firstDef(root) orelse return error.NoDefinition;
    const elem = elemDef(def);
    try std.testing.expectEqual(@as(usize, 1), elem.leading_comments.len);
    try std.testing.expectEqual(.line, elem.leading_comments[0].kind);
    try std.testing.expectEqual(@as(?*ast.DocComment, null), elem.doc_comment);
}

test "comment_attachment.leading_blank_line_not_attached" {
    // ─── Case 2: // line comment separated by blank line from decl ─────────
    // Expected per gofmt rule: the blank-separated comment does NOT attach as
    // leading_comments on the next decl. (In Plan 02-01, floating comments are
    // dropped; Plan 02-05 fmt will surface them as trailing on previous decl.)
    // leading_comments must be EMPTY for the part_def.
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const alloc = arena.allocator();

    var diags: std.ArrayList(Diagnostic) = .empty;
    const source =
        \\// floating comment
        \\
        \\part def Foo {}
    ;
    const root = (try parse(alloc, source, &diags)) orelse {
        return error.ParseFailed;
    };

    const def = firstDef(root) orelse return error.NoDefinition;
    const elem = elemDef(def);
    // The blank line between the comment and the declaration means the comment
    // does NOT attach as leading_comments on Foo.
    try std.testing.expectEqual(@as(usize, 0), elem.leading_comments.len);
    try std.testing.expectEqual(@as(?*ast.DocComment, null), elem.doc_comment);
}

test "comment_attachment.leading_block_comment" {
    // ─── Case 3: /* block comment */ immediately before decl ───────────────
    // Expected: leading_comments = [Comment{kind=block}], doc_comment = null
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const alloc = arena.allocator();

    var diags: std.ArrayList(Diagnostic) = .empty;
    const source = "/* block comment */\npart def Bar {}";
    const root = (try parse(alloc, source, &diags)) orelse {
        return error.ParseFailed;
    };
    try std.testing.expectEqual(@as(usize, 0), diags.items.len);

    const def = firstDef(root) orelse return error.NoDefinition;
    const elem = elemDef(def);
    try std.testing.expectEqual(@as(usize, 1), elem.leading_comments.len);
    try std.testing.expectEqual(.block, elem.leading_comments[0].kind);
    try std.testing.expectEqual(@as(?*ast.DocComment, null), elem.doc_comment);
}

test "comment_attachment.doc_comment_goes_to_doc_field" {
    // ─── Case 4: /** doc comment */ immediately before decl ─────────────────
    // In Plan 02-01, doc_comment tokens are consumed by the EXISTING
    // parseDefinition() path (peek().tag == .doc_comment advances and stores
    // in doc_node → elem.doc). The new elem.doc_comment field (D-29 direct
    // DocComment pointer) is filled by plan 02-05. For now:
    //   - elem.doc is non-null (old path; the doc_comment Node)
    //   - elem.leading_comments is empty (doc tokens are NOT added there per D-29)
    //   - elem.doc_comment is null (new path not yet wired)
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const alloc = arena.allocator();

    var diags: std.ArrayList(Diagnostic) = .empty;
    const source = "/** doc comment */\npart def Baz {}";
    const root = (try parse(alloc, source, &diags)) orelse {
        return error.ParseFailed;
    };
    try std.testing.expectEqual(@as(usize, 0), diags.items.len);

    const def = firstDef(root) orelse return error.NoDefinition;
    const elem = elemDef(def);
    // The doc_comment token appears in elem.doc (the *Node field), not
    // elem.doc_comment (the *DocComment field added in Plan 02-01).
    try std.testing.expectEqual(@as(usize, 0), elem.leading_comments.len);
    try std.testing.expect(elem.doc != null);
}

test "comment_attachment.contiguous_group_attaches" {
    // ─── Case 5: multiple consecutive line comments, no blank line before decl ─
    // Expected: ALL comments in the contiguous group attach as leading_comments.
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const alloc = arena.allocator();

    var diags: std.ArrayList(Diagnostic) = .empty;
    const source =
        \\// comment one
        \\// comment two
        \\part def Multi {}
    ;
    const root = (try parse(alloc, source, &diags)) orelse {
        return error.ParseFailed;
    };

    const def = firstDef(root) orelse return error.NoDefinition;
    const elem = elemDef(def);
    try std.testing.expectEqual(@as(usize, 2), elem.leading_comments.len);
    try std.testing.expectEqual(.line, elem.leading_comments[0].kind);
    try std.testing.expectEqual(.line, elem.leading_comments[1].kind);
}

test "comment_attachment.blank_splits_group" {
    // ─── Case 6: blank line in the middle of a comment group ────────────────
    // Given: comment A, blank line, comment B, no blank, decl.
    // Expected: comment A is NOT attached (blank line broke the chain);
    //           comment B IS attached as leading_comments.
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const alloc = arena.allocator();

    var diags: std.ArrayList(Diagnostic) = .empty;
    const source =
        \\// comment before blank
        \\
        \\// comment after blank
        \\part def Split {}
    ;
    const root = (try parse(alloc, source, &diags)) orelse {
        return error.ParseFailed;
    };

    const def = firstDef(root) orelse return error.NoDefinition;
    const elem = elemDef(def);
    // Only the last contiguous group (comment after blank) attaches.
    try std.testing.expectEqual(@as(usize, 1), elem.leading_comments.len);
    // The attached comment contains "after blank" in its text.
    try std.testing.expect(std.mem.indexOf(u8, elem.leading_comments[0].text, "after blank") != null);
}
