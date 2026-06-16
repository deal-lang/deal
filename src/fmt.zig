//! DEAL canonical source formatter (D-21, Plan 02-05).
//!
//! Walks the AST with attached comments (D-28, D-29) and emits canonical
//! source bytes. Does NOT walk the IR — formatter operates on AST per D-25.
//!
//! Round-trip invariants (RESEARCH §Pattern 3):
//!   1. One space around binary operators (a+b → a + b)
//!   2. Four-space indent (no tabs, no two-space)
//!   3. One blank line between top-level declarations
//!   4. LM-3 single→double quote normalization
//!   5. NO trailing commas (E0122 guard)
//!   6. Comments emitted verbatim — no re-flow (RESEARCH §Anti-Patterns)
//!
//! Export: `emitFormatted(allocator, root) ![]const u8` — same shape as
//! json.emitAst.

const std = @import("std");
const ast = @import("ast");

/// Emit canonical formatted source bytes for the given AST root.
///
/// The returned slice is owned by the caller's allocator.
/// On any allocation failure, returns an error.
///
/// Per D-21 / D-25: walks AST only — does NOT read or touch IR.
pub fn emitFormatted(allocator: std.mem.Allocator, root: ?*ast.Node) ![]const u8 {
    var buf: std.ArrayList(u8) = .empty;
    defer buf.deinit(allocator);

    if (root) |r| {
        var ctx = FormatContext{ .allocator = allocator, .buf = &buf };
        try ctx.writeNode(r, 0);
    }
    // Ensure file ends with a single newline
    if (buf.items.len > 0 and buf.items[buf.items.len - 1] != '\n') {
        try buf.append(allocator, '\n');
    }

    return try buf.toOwnedSlice(allocator);
}

/// Internal state for formatting a single file.
const FormatContext = struct {
    allocator: std.mem.Allocator,
    buf: *std.ArrayList(u8),

    /// Write any string slice directly.
    fn write(ctx: *FormatContext, s: []const u8) !void {
        try ctx.buf.appendSlice(ctx.allocator, s);
    }

    /// Write a single byte.
    fn writeByte(ctx: *FormatContext, b: u8) !void {
        try ctx.buf.append(ctx.allocator, b);
    }

    /// Write indentation (4 * indent spaces).
    fn writeIndent(ctx: *FormatContext, indent: u32) !void {
        var i: u32 = 0;
        while (i < indent) : (i += 1) {
            try ctx.buf.appendSlice(ctx.allocator, "    ");
        }
    }

    /// Write a string value with LM-3 quote normalization:
    ///   - Single-quoted strings → double-quoted.
    ///   - Double-quoted strings stay double-quoted.
    ///   - Backtick strings stay backticked.
    fn writeStringValue(ctx: *FormatContext, value: []const u8) !void {
        // String values in the AST are already the semantic content
        // (without delimiters), stored as slices. To detect the original
        // quote style we would need the original source span. Since we
        // don't have that at this level, we emit all strings as double-
        // quoted — which covers the LM-3 normalization requirement.
        try ctx.buf.append(ctx.allocator, '"');
        try writeStringEscaped(ctx.allocator, ctx.buf, value);
        try ctx.buf.append(ctx.allocator, '"');
    }

    /// Dispatch on NodeKind and call the appropriate writer.
    fn writeNode(ctx: *FormatContext, node: *ast.Node, indent: u32) std.mem.Allocator.Error!void {
        switch (node.payload) {
            .deal_file => |*p| try ctx.writeDealFile(p, indent),
            .dealx_file => |*p| try ctx.writeDealxFile(p, indent),
            .header_block => |*p| try ctx.writeHeaderBlock(p, indent),
            .package_decl => |*p| try ctx.writePackageDecl(p, indent),
            .import_decl => |*p| try ctx.writeImportDecl(p, indent),
            .export_decl => |*p| try ctx.writeExportDecl(p, indent),

            // Element definitions — dispatch by kind
            .part_def => |*p| try ctx.writeElementDef(p, "part def", indent),
            .actor_def => |*p| try ctx.writeElementDef(p, "actor def", indent),
            .port_def => |*p| try ctx.writeElementDef(p, "port def", indent),
            .action_def => |*p| try ctx.writeElementDef(p, "action def", indent),
            .state_def => |*p| try ctx.writeElementDef(p, "state def", indent),
            .attribute_def => |*p| try ctx.writeElementDef(p, "attribute def", indent),
            .item_def => |*p| try ctx.writeElementDef(p, "item def", indent),
            .interface_def => |*p| try ctx.writeElementDef(p, "interface def", indent),
            .connection_def => |*p| try ctx.writeElementDef(p, "connection def", indent),
            .flow_def => |*p| try ctx.writeElementDef(p, "flow def", indent),
            .allocation_def => |*p| try ctx.writeElementDef(p, "allocation def", indent),
            .requirement_def => |*p| try ctx.writeElementDef(p, "requirement def", indent),
            .constraint_def => |*p| try ctx.writeConstraintDef(p, indent),
            .need_def => |*p| try ctx.writeElementDef(p, "need def", indent),
            .use_case_def => |*p| try ctx.writeElementDef(p, "use case def", indent),

            // Usages — dispatch by kind
            .part_usage => |*p| try ctx.writeElementUsage(p, "part", indent, node.kind),
            .port_usage => |*p| try ctx.writeElementUsage(p, "port", indent, node.kind),
            .attribute_usage => |*p| try ctx.writeElementUsage(p, "attribute", indent, node.kind),
            .action_usage => |*p| try ctx.writeElementUsage(p, "action", indent, node.kind),
            .actor_usage => |*p| try ctx.writeElementUsage(p, "actor", indent, node.kind),
            .subject_usage => |*p| try ctx.writeElementUsage(p, "subject", indent, node.kind),
            .state_usage => |*p| try ctx.writeElementUsage(p, "state", indent, node.kind),
            .item_usage => |*p| try ctx.writeElementUsage(p, "item", indent, node.kind),
            .interface_usage => |*p| try ctx.writeElementUsage(p, "interface", indent, node.kind),
            .connection_usage => |*p| try ctx.writeElementUsage(p, "connection", indent, node.kind),
            .flow_usage => |*p| try ctx.writeElementUsage(p, "flow", indent, node.kind),
            .allocation_usage => |*p| try ctx.writeElementUsage(p, "allocation", indent, node.kind),
            .requirement_usage => |*p| try ctx.writeElementUsage(p, "requirement", indent, node.kind),
            .constraint_usage => |*p| try ctx.writeElementUsage(p, "constraint", indent, node.kind),
            .need_usage => |*p| try ctx.writeElementUsage(p, "need", indent, node.kind),
            .use_case_usage => |*p| try ctx.writeElementUsage(p, "use case", indent, node.kind),

            // Type and structural nodes
            .type_annotation => |*p| try ctx.writeTypeAnnotation(p, indent),
            .multiplicity => |*p| try ctx.writeMultiplicity(p),
            .modifier_list => |*p| try ctx.writeModifierList(p),
            .visibility_wrapper => |*p| try ctx.writeVisibilityWrapper(p, indent),
            .specialization => |*p| try ctx.writeSpecialization(p),
            .redefinition => |*p| try ctx.writeRedefinition(p, indent),
            .operator_statement => |*p| try ctx.writeOperatorStatement(p, indent),
            .inline_body => |*p| try ctx.writeInlineBody(p, indent),
            .structural_relationship => |*p| try ctx.writeStructuralRelationship(p),

            // Annotations
            .annotation => |*p| try ctx.writeAnnotation(p, indent),
            .doc_comment => |*p| try ctx.writeDocCommentNode(p),
            .annotation_body => |*p| try ctx.writeAnnotationBody(p, indent),

            // Verification blocks
            .verification_block => |*p| try ctx.writeVerificationBlock(p, indent),
            .precondition_block => |*p| try ctx.writePreconditionBlock(p, indent),
            .postcondition_block => |*p| try ctx.writePostconditionBlock(p, indent),

            // Expressions
            .binary => |*p| try ctx.writeBinary(p, indent),
            .unary => |*p| try ctx.writeUnary(p, indent),
            .member_access => |*p| try ctx.writeMemberAccess(p, indent),
            .call => |*p| try ctx.writeCall(p, indent),
            .identifier => |*p| try ctx.write(p.name),
            .int_literal => |*p| try ctx.write(p.text),
            .real_literal => |*p| try ctx.write(p.text),
            .string_literal => |*p| try ctx.writeStringValue(p.value),
            .template_literal => |*p| try ctx.writeTemplateLiteral(p, indent),
            .boolean_literal => |*p| try ctx.write(if (p.value) "true" else "false"),
            .interpolation => |*p| try ctx.writeInterpolation(p, indent),

            // Composition (.dealx)
            .system_block => |*p| try ctx.writeSystemBlock(p, indent),
            .subsystem_block => |*p| try ctx.writeSubsystemBlock(p, indent),
            .component_instance => |*p| try ctx.writeComponentInstance(p, indent),
            .comp_connect => |*p| try ctx.writeCompConnect(p, indent),
            .expose_tag => |*p| try ctx.writeExposeTag(p, indent),
            .traceability_block => |*p| try ctx.writeTraceabilityBlock(p, indent),
            .allocate_tag => |*p| try ctx.writeAllocateTag(p, indent),
            .satisfy_block => |*p| try ctx.writeSatisfyBlock(p, indent),
            .validate_tag => |*p| try ctx.writeValidateTag(p, indent),
            .object_literal => |*p| try ctx.writeObjectLiteral(p, indent),
            .array_literal => |*p| try ctx.writeArrayLiteral(p, indent),

            // Calc/constraint (Phase 05.2 Wave 2) — real implementations below
            .calc_def => |*p| try ctx.writeCalcDef(p, indent),
            .param_list => |*p| try ctx.writeParamList(p, indent),
            .param_decl => |*p| try ctx.writeParamDecl(p, indent),
            .return_contract => |*p| try ctx.writeReturnContract(p, indent),
            .precision_spec => |*p| try ctx.writePrecisionSpec(p),
            .constraint_ref => |*p| try ctx.writeConstraintRef(p),
            .constraint_body => |*p| try ctx.writeConstraintBody(p, indent),
            .calc_body => |*p| try ctx.writeCalcBody(p, indent),
            .require_statement => |*p| try ctx.writeRequireStatement(p, indent),

            // Behavioral surface (Stage-2 S2.7b). Canonical, idempotent writers
            // matching deal.ebnf §9b/§9c so the behavioral showcase round-trips.
            .action_body => |*p| try ctx.writeActionBody(p, indent),
            .pin_decl => |*p| try ctx.writePinDecl(p, indent),
            .succession_chain => |*p| try ctx.writeSuccessionChain(p, indent),
            .control_ref => |*p| try ctx.writeControlRef(p),
            .target_ref => |*p| try ctx.writeTargetRef(p),
            .decide_block => |*p| try ctx.writeDecideBlock(p, indent),
            .par_block => |*p| try ctx.writeParBlock(p, indent),
            .loop_statement => |*p| try ctx.writeLoopStatement(p, indent),
            .send_action => |*p| try ctx.writeSendAction(p),
            .accept_action => |*p| try ctx.writeAcceptAction(p, indent),
            .assign_action => |*p| try ctx.writeAssignAction(p, indent),
            .perform_statement => |*p| try ctx.writePerformStatement(p, indent),
            .item_flow_statement => |*p| try ctx.writeItemFlowStatement(p, indent),
            .binding_statement => |*p| try ctx.writeBindingStatement(p, indent),
            .escape_node => |*p| try ctx.writeEscapeNode(p, indent),
            .escape_succession => |*p| try ctx.writeEscapeSuccession(p, indent),
            .entry_do_exit => |*p| try ctx.writeEntryDoExit(p, indent),
            .transition_statement => |*p| try ctx.writeTransitionStatement(p, indent),
        }
    }

    // ─── File-level writers ──────────────────────────────────────────────

    fn writeDealFile(ctx: *FormatContext, p: *const ast.DealFile, indent: u32) !void {
        if (p.header) |h| {
            try ctx.writeNode(h, indent);
            try ctx.write("\n");
        }
        if (p.package_decl) |pkg| {
            try ctx.writeNode(pkg, indent);
            try ctx.write("\n");
        }
        if (p.imports.len > 0) {
            try ctx.write("\n");
            for (p.imports) |imp| {
                try ctx.writeNode(imp, indent);
            }
        }
        if (p.exports.len > 0) {
            try ctx.write("\n");
            for (p.exports) |exp| {
                try ctx.writeNode(exp, indent);
            }
        }
        // Write top-level definitions with one blank line between them.
        // Orphan doc-comment nodes (.identifier kind with text starting "/**")
        // are a parser artifact: they represent top-level `/** ... */` doc
        // comments that weren't attached to a definition (e.g. module-level
        // header docs or section separator docs). We emit them to match the
        // idempotent canonical form:
        //   - A blank line before the doc comment (same as any real def).
        //   - NO blank line between the doc comment and the following def
        //     (same as if the doc_comment were attached as doc_node).
        if (p.definitions.len > 0) {
            var prev_was_orphan_doc = false;
            for (p.definitions) |def| {
                const is_orphan_doc = def.kind == .identifier and
                    std.mem.startsWith(u8, def.payload.identifier.name, "/**");
                // Only add the blank-line separator if the previous item was NOT
                // an orphan doc comment (to avoid extra blank between doc and def).
                if (!prev_was_orphan_doc) try ctx.write("\n");
                try ctx.writeDeclarationWithComments(def, indent);
                prev_was_orphan_doc = is_orphan_doc;
            }
        }
    }

    fn writeDealxFile(ctx: *FormatContext, p: *const ast.DealxFile, indent: u32) !void {
        if (p.header) |h| {
            try ctx.writeNode(h, indent);
            try ctx.write("\n");
        }
        if (p.package_decl) |pkg| {
            try ctx.writeNode(pkg, indent);
            try ctx.write("\n");
        }
        if (p.imports.len > 0) {
            try ctx.write("\n");
            for (p.imports) |imp| {
                try ctx.writeNode(imp, indent);
            }
        }
        for (p.root_tags, 0..) |tag, i| {
            try ctx.write("\n");
            try ctx.writeDeclarationWithComments(tag, indent);
            _ = i;
        }
    }

    fn writeHeaderBlock(ctx: *FormatContext, p: *const ast.HeaderBlock, _: u32) !void {
        try ctx.write("@header {\n");
        // Compute the maximum key length for aligned value columns.
        var max_key_len: usize = 0;
        for (p.fields) |f| {
            if (f.key.len > max_key_len) max_key_len = f.key.len;
        }
        for (p.fields) |f| {
            try ctx.write("    ");
            try ctx.write(f.key);
            try ctx.writeByte(':');
            // Pad to align values so they all start at the same column.
            // The canonical form uses: spaces = max_key_len - key_len + 3
            // (minimum 3 spaces after colon, longest key gets exactly 3 spaces).
            const pad = max_key_len - f.key.len + 3;
            for (0..pad) |_| try ctx.writeByte(' ');
            // Preserve header field values verbatim (they may include timestamps etc.)
            try ctx.write(f.value);
            try ctx.write("\n");
        }
        try ctx.write("}");
    }

    fn writePackageDecl(ctx: *FormatContext, p: *const ast.PackageDecl, _: u32) !void {
        try ctx.write("package ");
        for (p.segments, 0..) |seg, i| {
            if (i > 0) try ctx.write(".");
            try ctx.write(seg);
        }
        try ctx.write(";");
    }

    fn writeImportDecl(ctx: *FormatContext, p: *const ast.ImportDecl, _: u32) !void {
        try ctx.write("import ");
        for (p.path, 0..) |seg, i| {
            if (i > 0) try ctx.write(".");
            try ctx.write(seg);
        }
        switch (p.kind) {
            .simple => {
                // No extra syntax after path
            },
            .wildcard => {
                try ctx.write(".*");
            },
            .named => {
                try ctx.write(".{");
                for (p.items, 0..) |item, i| {
                    if (i > 0) try ctx.write(", ");
                    try ctx.write(item.name);
                    if (item.alias) |a| {
                        try ctx.write(" as ");
                        try ctx.write(a);
                    }
                }
                try ctx.write("}");
            },
            .alias => {
                if (p.items.len > 0) {
                    try ctx.write(" as ");
                    try ctx.write(p.items[0].name);
                }
            },
        }
        try ctx.write(";\n");
    }

    fn writeExportDecl(ctx: *FormatContext, p: *const ast.ExportDecl, _: u32) !void {
        try ctx.write("export ");
        try ctx.write(p.module);
        if (p.items.len > 0) {
            try ctx.write(".{");
            for (p.items, 0..) |item, i| {
                if (i > 0) try ctx.write(", ");
                try ctx.write(item);
            }
            try ctx.write("}");
        }
        try ctx.write(";\n");
    }

    // ─── Comment emission ────────────────────────────────────────────────

    /// Emit leading_comments, doc_comment, then the node, then trailing_comments.
    /// Used for all top-level declarations.
    fn writeDeclarationWithComments(ctx: *FormatContext, node: *ast.Node, indent: u32) !void {
        // Extract comment fields if this is an ElementDef or ElementUsage.
        const comments_opt = getNodeComments(node);

        if (comments_opt) |comments| {
            // 1. Emit leading_comments
            for (comments.leading) |c| {
                try ctx.writeIndent(indent);
                try ctx.writeComment(c);
                try ctx.write("\n");
            }
            // 2. Emit doc_comment — prefer the D-29 typed struct; fall back to
            //    the ElementDef.doc node (pre-02-01 path that is currently populated
            //    by the parser).
            if (comments.doc) |dc| {
                try ctx.writeDocComment(dc, indent);
            } else if (comments.doc_node) |dn| {
                // The doc_comment node stores the raw text including /** and */.
                try ctx.writeIndent(indent);
                try ctx.writeNode(dn, indent);
                try ctx.write("\n");
            }
        }

        // 3. Emit the declaration body itself
        try ctx.writeIndent(indent);
        try ctx.writeNode(node, indent);

        if (comments_opt) |comments| {
            // 4. Emit trailing_comments (on the same line)
            for (comments.trailing) |c| {
                try ctx.write(" ");
                try ctx.writeComment(c);
            }
        }

        try ctx.write("\n");
    }

    fn writeComment(ctx: *FormatContext, c: ast.Comment) !void {
        switch (c.kind) {
            .line => {
                // Ensure the comment text starts with //
                if (std.mem.startsWith(u8, c.text, "//")) {
                    try ctx.write(c.text);
                } else {
                    try ctx.write("// ");
                    try ctx.write(c.text);
                }
            },
            .block => {
                if (std.mem.startsWith(u8, c.text, "/*")) {
                    try ctx.write(c.text);
                } else {
                    try ctx.write("/* ");
                    try ctx.write(c.text);
                    try ctx.write(" */");
                }
            },
        }
    }

    fn writeDocComment(ctx: *FormatContext, dc: *const ast.DocComment, indent: u32) !void {
        // Emit verbatim — no re-flow (RESEARCH §Anti-Patterns).
        // The text includes the /** and */ delimiters.
        const text = dc.text;
        if (text.len == 0) return;
        try ctx.writeIndent(indent);
        try ctx.write(text);
        try ctx.write("\n");
    }

    fn writeDocCommentNode(ctx: *FormatContext, p: *const ast.DocComment) !void {
        try ctx.write(p.text);
    }

    // ─── Element definition ───────────────────────────────────────────────

    fn writeElementDef(ctx: *FormatContext, elem: *const ast.ElementDef, keyword: []const u8, indent: u32) !void {
        // Modifiers first (abstract, derived, etc.)
        for (elem.modifiers) |m| {
            try ctx.write(modifierKw(m));
            try ctx.write(" ");
        }

        // Direction for port defs
        if (elem.direction != .none) {
            try ctx.write(directionKw(elem.direction));
            try ctx.write(" ");
        }

        try ctx.write(keyword);
        try ctx.write(" ");
        try ctx.write(elem.name);

        // Specialization
        if (elem.specializes) |spec| {
            try ctx.write(" ");
            try ctx.writeNode(spec, indent);
        }

        // Body — note: elem.doc (the doc comment) is emitted BEFORE the
        // `part def` keyword by writeDeclarationWithComments, NOT inside the body.
        if (elem.members.len > 0 or elem.annotations.len > 0) {
            try ctx.write(" {\n");
            // Annotations first
            for (elem.annotations) |ann| {
                try ctx.writeIndent(indent + 1);
                try ctx.writeNode(ann, indent + 1);
                try ctx.write("\n");
            }
            // Members
            for (elem.members) |member| {
                try ctx.writeDeclarationWithComments(member, indent + 1);
            }
            try ctx.writeIndent(indent);
            try ctx.write("}");
        } else {
            try ctx.write(" {}");
        }
    }

    // ─── Element usage ────────────────────────────────────────────────────

    fn writeElementUsage(ctx: *FormatContext, u: *const ast.ElementUsage, keyword: []const u8, indent: u32, kind: ast.NodeKind) !void {
        _ = kind;
        // Modifiers
        for (u.modifiers) |m| {
            try ctx.write(modifierKw(m));
            try ctx.write(" ");
        }

        // Direction
        if (u.direction != .none) {
            try ctx.write(directionKw(u.direction));
            try ctx.write(" ");
        }

        try ctx.write(keyword);
        try ctx.write(" ");
        try ctx.write(u.name);

        // Type annotation
        if (u.type_node) |tn| {
            try ctx.write(" : ");
            try ctx.writeNode(tn, indent);
        }

        // Multiplicity
        if (u.multiplicity) |m| {
            try ctx.write(" ");
            try ctx.writeNode(m, indent);
        }

        // Default value
        if (u.default_value) |dv| {
            try ctx.write(" = ");
            try ctx.writeNode(dv, indent);
        }

        // D-06 generalized precision: `=> ReturnContract` after default value, before ";"
        if (u.precision) |prec| {
            try ctx.write(" ");
            try ctx.writeNode(prec, indent);
        }

        // Inline body
        if (u.inline_body) |ib| {
            try ctx.write(" ");
            try ctx.writeNode(ib, indent);
            // Inline body ends with '}' — no trailing semicolon needed.
            // (e.g. `out port hvOut : HVDCPort [1] { <<redefines>> ... }`)
            return;
        } else if (u.annotations.len > 0) {
            // If no inline body but has annotations, emit them on separate lines
            try ctx.write(" {\n");
            for (u.annotations) |ann| {
                try ctx.writeIndent(indent + 1);
                try ctx.writeNode(ann, indent + 1);
                try ctx.write("\n");
            }
            try ctx.writeIndent(indent);
            try ctx.write("}");
            return;
        }

        try ctx.write(";");
    }

    // ─── Type and structural nodes ────────────────────────────────────────

    fn writeTypeAnnotation(ctx: *FormatContext, p: *const ast.TypeAnnotation, _: u32) !void {
        for (p.name_segments, 0..) |seg, i| {
            if (i > 0) try ctx.write(".");
            try ctx.write(seg);
        }
    }

    fn writeMultiplicity(ctx: *FormatContext, p: *const ast.Multiplicity) !void {
        // Canonical forms:
        //   [N]     — exact count (lower == upper)
        //   [N..*]  — lower N, unbounded (N > 0)
        //   [*]     — zero or more (lower == 0, unbounded) — short form of [0..*]
        //   [N..M]  — bounded range
        // Note: writing the lower bound BEFORE * is syntactically ambiguous —
        // "[0*]" is parsed as [0..0], not [0..*]. Always use the ".." separator.
        try ctx.write("[");
        if (p.unbounded) {
            if (p.lower > 0) {
                try appendU32(ctx.allocator, ctx.buf, p.lower);
                try ctx.write("..*");
            } else {
                // [0..*] → canonical short form [*]
                try ctx.write("*");
            }
        } else if (p.upper) |upper| {
            try appendU32(ctx.allocator, ctx.buf, p.lower);
            if (upper != p.lower) {
                try ctx.write("..");
                try appendU32(ctx.allocator, ctx.buf, upper);
            }
        } else {
            try appendU32(ctx.allocator, ctx.buf, p.lower);
        }
        try ctx.write("]");
    }

    fn writeModifierList(ctx: *FormatContext, p: *const ast.ModifierList) !void {
        for (p.modifiers, 0..) |m, i| {
            if (i > 0) try ctx.write(" ");
            try ctx.write(modifierKw(m));
        }
    }

    fn writeVisibilityWrapper(ctx: *FormatContext, p: *const ast.VisibilityWrapper, indent: u32) !void {
        try ctx.write(visibilityKw(p.visibility));
        try ctx.write(" (\n");
        for (p.members) |member| {
            try ctx.writeDeclarationWithComments(member, indent + 1);
        }
        try ctx.writeIndent(indent);
        try ctx.write(")");
    }

    fn writeSpecialization(ctx: *FormatContext, p: *const ast.Specialization) !void {
        try ctx.write(p.op_text);
        try ctx.write(" ");
        for (p.target_segments, 0..) |seg, i| {
            if (i > 0) try ctx.write(".");
            try ctx.write(seg);
        }
    }

    fn writeRedefinition(ctx: *FormatContext, p: *const ast.Redefinition, indent: u32) !void {
        try ctx.write("<<");
        try ctx.write(p.op_text);
        try ctx.write(">>");
        try ctx.write(" ");
        for (p.target_segments, 0..) |seg, i| {
            if (i > 0) try ctx.write(".");
            try ctx.write(seg);
        }
        if (p.value) |v| {
            try ctx.write(" = ");
            try ctx.writeNode(v, indent);
        }
        try ctx.write(";");
    }

    fn writeOperatorStatement(ctx: *FormatContext, p: *const ast.OperatorStatement, indent: u32) !void {
        // op_text stores the inner keyword without angle brackets (e.g. "redefines").
        // Re-emit with << >> delimiters so the output is parseable.
        try ctx.write("<<");
        try ctx.write(p.op_text);
        try ctx.write(">>");
        try ctx.write(" ");
        for (p.target_segments, 0..) |seg, i| {
            if (i > 0) try ctx.write(".");
            try ctx.write(seg);
        }
        if (p.value) |v| {
            try ctx.write(" = ");
            try ctx.writeNode(v, indent);
        }
        try ctx.write(";");
    }

    fn writeInlineBody(ctx: *FormatContext, p: *const ast.InlineBody, indent: u32) !void {
        try ctx.write("{\n");
        for (p.members) |member| {
            try ctx.writeDeclarationWithComments(member, indent + 1);
        }
        try ctx.writeIndent(indent);
        try ctx.write("}");
    }

    fn writeStructuralRelationship(ctx: *FormatContext, p: *const ast.StructuralRelationship) !void {
        try ctx.write("<<");
        try ctx.write(p.op_text);
        try ctx.write(">>");
        try ctx.write(" ");
        for (p.target_segments, 0..) |seg, i| {
            if (i > 0) try ctx.write(".");
            try ctx.write(seg);
        }
    }

    // ─── Annotation nodes ─────────────────────────────────────────────────

    fn writeAnnotation(ctx: *FormatContext, p: *const ast.Annotation, indent: u32) !void {
        // Two kinds of annotations (see parser_deal.zig §16):
        //
        // 1. Standalone: `@name` or `@name: value` — category == null
        //    Examples: @confidence: 0.95, @rationale: "text", @assumes: "..."
        //
        // 2. Category: `@category:<<operator>> target {body}` — category != null
        //    The `name` field equals `category` in this case.
        //    Examples: @simulation:<<computes>> thermalRunaway {...}
        //              @trace:<<satisfies>> REQ_01
        if (p.category) |cat| {
            // Category annotation: @category:<<op>> target {body}
            try ctx.write("@");
            try ctx.write(cat);
            try ctx.write(":");
            if (p.operator) |op| {
                try ctx.write("<<");
                try ctx.write(op);
                try ctx.write(">>");
            }
            if (p.target_segments.len > 0) {
                try ctx.write(" ");
                for (p.target_segments, 0..) |seg, i| {
                    if (i > 0) try ctx.write(".");
                    try ctx.write(seg);
                }
            }
            if (p.value) |v| {
                try ctx.write(" ");
                try ctx.writeNode(v, indent);
            }
            if (p.body) |b_node| {
                try ctx.write(" ");
                try ctx.writeNode(b_node, indent);
            }
        } else {
            // Standalone annotation: @name or @name: value
            try ctx.write("@");
            try ctx.write(p.name);
            if (p.value) |v| {
                try ctx.write(": ");
                try ctx.writeNode(v, indent);
            }
            if (p.body) |b_node| {
                try ctx.write(" ");
                try ctx.writeNode(b_node, indent);
            }
        }
    }

    fn writeAnnotationBody(ctx: *FormatContext, p: *const ast.AnnotationBodyNode, indent: u32) !void {
        try ctx.write("{\n");
        for (p.fields) |f| {
            try ctx.writeIndent(indent + 1);
            try ctx.write(f.key);
            try ctx.write(": ");
            try ctx.writeNode(f.value, indent + 1);
            try ctx.write(";\n");
        }
        try ctx.writeIndent(indent);
        try ctx.write("}");
    }

    // ─── Verification blocks ──────────────────────────────────────────────

    fn writeVerificationBlock(ctx: *FormatContext, p: *const ast.VerificationBlock, indent: u32) !void {
        try ctx.write("verification {\n");
        for (p.fields) |f| {
            try ctx.writeIndent(indent + 1);
            try ctx.write(verifFieldKindKw(f.kind));
            try ctx.write(": ");
            try ctx.writeNode(f.value, indent + 1);
            try ctx.write(";\n");
        }
        try ctx.writeIndent(indent);
        try ctx.write("}");
    }

    fn writePreconditionBlock(ctx: *FormatContext, p: *const ast.PreconditionBlock, indent: u32) !void {
        try ctx.write("precondition ");
        try ctx.write(p.name);
        try ctx.write(" {\n");
        for (p.conditions) |cond| {
            try ctx.writeIndent(indent + 1);
            try ctx.writeNode(cond, indent + 1);
            try ctx.write("\n");
        }
        try ctx.writeIndent(indent);
        try ctx.write("}");
    }

    fn writePostconditionBlock(ctx: *FormatContext, p: *const ast.PostconditionBlock, indent: u32) !void {
        try ctx.write("postcondition ");
        try ctx.write(p.name);
        try ctx.write(" {\n");
        for (p.conditions) |cond| {
            try ctx.writeIndent(indent + 1);
            try ctx.writeNode(cond, indent + 1);
            try ctx.write("\n");
        }
        try ctx.writeIndent(indent);
        try ctx.write("}");
    }

    // ─── Expression writers ───────────────────────────────────────────────

    fn writeBinary(ctx: *FormatContext, p: *const ast.Binary, indent: u32) !void {
        // One space around binary operators (formatting invariant #1).
        try ctx.writeNode(p.lhs, indent);
        try ctx.write(" ");
        try ctx.write(binaryOpText(p.op));
        try ctx.write(" ");
        try ctx.writeNode(p.rhs, indent);
    }

    fn writeUnary(ctx: *FormatContext, p: *const ast.Unary, indent: u32) !void {
        try ctx.write(unaryOpText(p.op));
        try ctx.writeNode(p.operand, indent);
    }

    fn writeMemberAccess(ctx: *FormatContext, p: *const ast.MemberAccess, indent: u32) !void {
        try ctx.writeNode(p.receiver, indent);
        try ctx.write(".");
        try ctx.write(p.member);
    }

    fn writeCall(ctx: *FormatContext, p: *const ast.Call, indent: u32) !void {
        try ctx.writeNode(p.callee, indent);
        try ctx.write("(");
        for (p.args, 0..) |arg, i| {
            if (i > 0) try ctx.write(", ");
            try ctx.writeNode(arg, indent);
        }
        try ctx.write(")");
    }

    /// Emit a real array_literal node as `[item, item, ...]` (D-04).
    /// Replaces the former __array__ special-case in writeCall.
    fn writeArrayLiteral(ctx: *FormatContext, p: *const ast.ArrayLiteral, indent: u32) !void {
        try ctx.write("[");
        for (p.items, 0..) |item, i| {
            if (i > 0) try ctx.write(", ");
            try ctx.writeNode(item, indent);
        }
        try ctx.write("]");
    }

    // ─── Calc/constraint writers (Phase 05.2 Wave 2) ─────────────────────

    fn writeConstraintDef(ctx: *FormatContext, p: *const ast.ConstraintDefinition, indent: u32) !void {
        for (p.modifiers) |m| { try ctx.write(modifierKw(m)); try ctx.write(" "); }
        if (p.direction != .none) { try ctx.write(directionKw(p.direction)); try ctx.write(" "); }
        try ctx.write("constraint def ");
        try ctx.write(p.name);
        if (p.params) |params| {
            try ctx.writeNode(params, indent);
        }
        if (p.specializes) |spec| {
            try ctx.write(" ");
            try ctx.writeNode(spec, indent);
        }
        try ctx.write(" ");
        try ctx.writeNode(p.body, indent);
    }

    fn writeCalcDef(ctx: *FormatContext, p: *const ast.CalcDefinition, indent: u32) !void {
        for (p.modifiers) |m| { try ctx.write(modifierKw(m)); try ctx.write(" "); }
        if (p.direction != .none) { try ctx.write(directionKw(p.direction)); try ctx.write(" "); }
        try ctx.write("calc def ");
        try ctx.write(p.name);
        try ctx.writeNode(p.params, indent);
        try ctx.write(" : ");
        try ctx.writeNode(p.type_node, indent);
        if (p.return_contract) |rc| {
            try ctx.write(" ");
            try ctx.writeNode(rc, indent);
        }
        try ctx.write(" ");
        try ctx.writeNode(p.body, indent);
    }

    fn writeParamList(ctx: *FormatContext, p: *const ast.ParameterList, indent: u32) !void {
        try ctx.write("(");
        for (p.params, 0..) |param, i| {
            if (i > 0) try ctx.write(", ");
            try ctx.writeNode(param, indent);
        }
        try ctx.write(")");
    }

    fn writeParamDecl(ctx: *FormatContext, p: *const ast.ParameterDecl, indent: u32) !void {
        // D-06 style: omit "in" direction prefix (normalized away by fmt)
        if (p.direction == .out) { try ctx.write("out "); }
        if (p.direction == .inout) { try ctx.write("inout "); }
        // .in and .none: no prefix
        try ctx.write(p.name);
        try ctx.write(" : ");
        try ctx.writeNode(p.type_node, indent);
        if (p.multiplicity) |m| {
            try ctx.write(" ");
            try ctx.writeNode(m, indent);
        }
    }

    fn writeReturnContract(ctx: *FormatContext, p: *const ast.ReturnContract, indent: u32) !void {
        try ctx.write("=> ");
        for (p.items, 0..) |item, i| {
            if (i > 0) try ctx.write(", ");
            try ctx.writeNode(item, indent);
        }
    }

    fn writePrecisionSpec(ctx: *FormatContext, p: *const ast.PrecisionSpec) !void {
        switch (p.kind) {
            .sig_figures => {
                try ctx.write("sig ");
                try ctx.writeNode(p.value, 0);
            },
            // D-09 normalization: ALWAYS emit ± (U+00B1), never +/-
            .tolerance_absolute, .tolerance_relative => {
                // ± in UTF-8: 0xC2 0xB1
                try ctx.write("\xC2\xB1 ");
                try ctx.writeNode(p.value, 0);
            },
        }
    }

    fn writeConstraintRef(ctx: *FormatContext, p: *const ast.ConstraintRef) !void {
        for (p.name_segments, 0..) |seg, i| {
            if (i > 0) try ctx.write(".");
            try ctx.write(seg);
        }
        if (p.args) |args| {
            try ctx.write("(");
            for (args, 0..) |arg, i| {
                if (i > 0) try ctx.write(", ");
                try ctx.writeNode(arg, 0);
            }
            try ctx.write(")");
        }
    }

    fn writeConstraintBody(ctx: *FormatContext, p: *const ast.ConstraintBody, indent: u32) !void {
        if (p.members.len == 0) {
            try ctx.write("{}");
            return;
        }
        try ctx.write("{\n");
        for (p.members) |member| {
            try ctx.writeIndent(indent + 1);
            try ctx.writeNode(member, indent + 1);
            try ctx.write("\n");
        }
        try ctx.writeIndent(indent);
        try ctx.write("}");
    }

    fn writeCalcBody(ctx: *FormatContext, p: *const ast.CalcBody, indent: u32) !void {
        try ctx.write("{\n");
        for (p.annotations) |ann| {
            try ctx.writeIndent(indent + 1);
            try ctx.writeNode(ann, indent + 1);
            try ctx.write("\n");
        }
        for (p.bindings) |binding| {
            try ctx.writeDeclarationWithComments(binding, indent + 1);
        }
        try ctx.writeIndent(indent + 1);
        try ctx.write("return ");
        try ctx.writeNode(p.return_stmt.payload.require_statement.condition, indent + 1);
        try ctx.write(";\n");
        try ctx.writeIndent(indent);
        try ctx.write("}");
    }

    fn writeRequireStatement(ctx: *FormatContext, p: *const ast.RequireStatement, indent: u32) !void {
        try ctx.write("require ");
        try ctx.writeNode(p.condition, indent);
        if (p.precision) |prec| {
            try ctx.write(" ");
            try ctx.writeNode(prec, indent);
        }
        try ctx.write(";");
    }

    fn writeTemplateLiteral(ctx: *FormatContext, p: *const ast.TemplateLiteralNode, indent: u32) !void {
        try ctx.write("`");
        for (p.parts) |part| {
            switch (part) {
                .text => |t| try ctx.write(t),
                .expr => |e| {
                    try ctx.write("${");
                    try ctx.writeNode(e, indent);
                    try ctx.write("}");
                },
            }
        }
        try ctx.write("`");
    }

    fn writeInterpolation(ctx: *FormatContext, p: *const ast.InterpolationNode, indent: u32) !void {
        try ctx.write("${");
        try ctx.writeNode(p.expr, indent);
        try ctx.write("}");
    }

    // ─── Behavioral surface writers (Stage-2 S2.7b) ──────────────────────
    // Canonical forms per deal.ebnf §9b/§9c; idempotent so the behavioral
    // showcase survives the fmt round-trip gate.

    /// Write `seg.seg.seg` (a dotted QualifiedName from a segment slice).
    fn writeSegs(ctx: *FormatContext, segs: []const []const u8) !void {
        for (segs, 0..) |seg, i| {
            if (i > 0) try ctx.write(".");
            try ctx.write(seg);
        }
    }

    /// Write a Guard: `[expr]` or `[else]`.
    fn writeGuard(ctx: *FormatContext, g: ast.Guard, indent: u32) !void {
        try ctx.write("[");
        if (g.is_else) {
            try ctx.write("else");
        } else if (g.expr) |e| {
            try ctx.writeNode(e, indent);
        }
        try ctx.write("]");
    }

    fn endpointKw(e: ast.ControlEndpoint) []const u8 {
        return switch (e) {
            .start => "start",
            .done => "done",
            .terminate => "terminate",
        };
    }

    /// ActionBody / nested body: `{ members }` (or `{}` when empty).
    fn writeActionBody(ctx: *FormatContext, p: *const ast.ActionBody, indent: u32) !void {
        if (p.members.len == 0) {
            try ctx.write("{}");
            return;
        }
        try ctx.write("{\n");
        for (p.members) |m| {
            try ctx.writeDeclarationWithComments(m, indent + 1);
        }
        try ctx.writeIndent(indent);
        try ctx.write("}");
    }

    /// `<dir> name : Type [mult]? [= default]? ;`
    fn writePinDecl(ctx: *FormatContext, p: *const ast.PinDecl, indent: u32) !void {
        try ctx.write(directionKw(p.direction));
        try ctx.write(" ");
        try ctx.write(p.name);
        try ctx.write(" : ");
        try ctx.writeNode(p.type_node, indent);
        if (p.multiplicity) |m| {
            try ctx.write(" ");
            try ctx.writeNode(m, indent);
        }
        if (p.default_value) |dv| {
            try ctx.write(" = ");
            try ctx.writeNode(dv, indent);
        }
        try ctx.write(";");
    }

    fn writeControlRef(ctx: *FormatContext, p: *const ast.ControlRef) !void {
        try ctx.write(endpointKw(p.endpoint));
    }

    fn writeTargetRef(ctx: *FormatContext, p: *const ast.TargetRef) !void {
        if (p.endpoint) |e| {
            try ctx.write(endpointKw(e));
            return;
        }
        try ctx.writeSegs(p.name_segments);
    }

    /// `ref ( -> [g]? ref )+ ;`
    fn writeSuccessionChain(ctx: *FormatContext, p: *const ast.SuccessionChain, indent: u32) !void {
        for (p.steps, 0..) |st, i| {
            if (i > 0) {
                try ctx.write(" -> ");
                if (st.guard) |g| {
                    try ctx.writeGuard(g, indent);
                    try ctx.write(" ");
                }
            }
            try ctx.writeNode(st.ref, indent);
        }
        try ctx.write(";");
    }

    /// `decide { [g] -> ref … }`
    fn writeDecideBlock(ctx: *FormatContext, p: *const ast.DecideBlock, indent: u32) !void {
        try ctx.write("decide {");
        for (p.branches) |br| {
            try ctx.write(" ");
            try ctx.writeGuard(br.guard, indent);
            try ctx.write(" -> ");
            try ctx.writeNode(br.target, indent);
        }
        try ctx.write(" }");
    }

    /// `par { -> ref … } ( -> ref )?`
    fn writeParBlock(ctx: *FormatContext, p: *const ast.ParBlock, indent: u32) !void {
        try ctx.write("par {");
        for (p.branches) |b| {
            try ctx.write(" -> ");
            try ctx.writeNode(b, indent);
        }
        try ctx.write(" }");
        if (p.exit) |ex| {
            try ctx.write(" -> ");
            try ctx.writeNode(ex, indent);
        }
    }

    /// `loop while/until [g] body` | `for v in expr body`
    fn writeLoopStatement(ctx: *FormatContext, p: *const ast.LoopStatement, indent: u32) !void {
        switch (p.kind) {
            .while_loop, .until_loop => {
                try ctx.write("loop ");
                try ctx.write(if (p.kind == .until_loop) "until " else "while ");
                try ctx.write("[");
                if (p.guard) |g| try ctx.writeNode(g, indent);
                try ctx.write("] ");
                try ctx.writeNode(p.body, indent);
            },
            .for_loop => {
                try ctx.write("for ");
                if (p.var_name) |v| try ctx.write(v);
                try ctx.write(" in ");
                if (p.iterable) |it| try ctx.writeNode(it, indent);
                try ctx.write(" ");
                try ctx.writeNode(p.body, indent);
            },
        }
    }

    /// `send Payload ( to Target )? ;`
    fn writeSendAction(ctx: *FormatContext, p: *const ast.SendAction) !void {
        try ctx.write("send ");
        try ctx.writeSegs(p.payload_segments);
        if (p.target_segments) |t| {
            try ctx.write(" to ");
            try ctx.writeSegs(t);
        }
        try ctx.write(";");
    }

    /// `accept Trigger ( [expr] )? ;`
    fn writeAcceptAction(ctx: *FormatContext, p: *const ast.AcceptAction, indent: u32) !void {
        try ctx.write("accept ");
        try ctx.writeSegs(p.trigger_segments);
        if (p.guard) |g| {
            try ctx.write(" [");
            try ctx.writeNode(g, indent);
            try ctx.write("]");
        }
        try ctx.write(";");
    }

    /// `assign Target := value ;`
    fn writeAssignAction(ctx: *FormatContext, p: *const ast.AssignAction, indent: u32) !void {
        try ctx.write("assign ");
        try ctx.writeSegs(p.target_segments);
        try ctx.write(" := ");
        try ctx.writeNode(p.value, indent);
        try ctx.write(";");
    }

    /// `call(args) ;`
    fn writePerformStatement(ctx: *FormatContext, p: *const ast.PerformStatement, indent: u32) !void {
        try ctx.writeNode(p.call, indent);
        try ctx.write(";");
    }

    /// `source ~> target ( : FlowType )? ;`
    fn writeItemFlowStatement(ctx: *FormatContext, p: *const ast.ItemFlowStatement, indent: u32) !void {
        try ctx.writeNode(p.source, indent);
        try ctx.write(" ~> ");
        try ctx.writeNode(p.target, indent);
        if (p.flow_type_segments) |ft| {
            try ctx.write(" : ");
            try ctx.writeSegs(ft);
        }
        try ctx.write(";");
    }

    /// `bind lhs = rhs ;`
    fn writeBindingStatement(ctx: *FormatContext, p: *const ast.BindingStatement, indent: u32) !void {
        try ctx.write("bind ");
        try ctx.writeNode(p.lhs, indent);
        try ctx.write(" = ");
        try ctx.writeNode(p.rhs, indent);
        try ctx.write(";");
    }

    /// `node Name : Type body? ;`
    fn writeEscapeNode(ctx: *FormatContext, p: *const ast.EscapeNode, indent: u32) !void {
        try ctx.write("node ");
        try ctx.write(p.name);
        try ctx.write(" : ");
        try ctx.writeSegs(p.type_segments);
        if (p.body) |b| {
            try ctx.write(" ");
            try ctx.writeNode(b, indent);
        }
        try ctx.write(";");
    }

    /// `succession src -> [g]? tgt ;`
    fn writeEscapeSuccession(ctx: *FormatContext, p: *const ast.EscapeSuccession, indent: u32) !void {
        try ctx.write("succession ");
        try ctx.writeSegs(p.source_segments);
        try ctx.write(" -> ");
        if (p.guard) |g| {
            try ctx.writeGuard(g, indent);
            try ctx.write(" ");
        }
        try ctx.writeSegs(p.target_segments);
        try ctx.write(";");
    }

    /// `entry|do|exit / behavior ;`
    fn writeEntryDoExit(ctx: *FormatContext, p: *const ast.EntryDoExit, indent: u32) !void {
        try ctx.write(switch (p.kind) {
            .entry => "entry",
            .do_action => "do",
            .exit => "exit",
        });
        try ctx.write(" / ");
        try ctx.writeNode(p.behavior, indent);
        try ctx.write(";");
    }

    /// `on Trigger ( [g] )? ( / effect )? -> Target ;`
    fn writeTransitionStatement(ctx: *FormatContext, p: *const ast.TransitionStatement, indent: u32) !void {
        try ctx.write("on ");
        try ctx.writeSegs(p.trigger_segments);
        if (p.guard) |g| {
            try ctx.write(" [");
            try ctx.writeNode(g, indent);
            try ctx.write("]");
        }
        if (p.effect) |e| {
            try ctx.write(" / ");
            try ctx.writeNode(e, indent);
        }
        try ctx.write(" -> ");
        try ctx.writeNode(p.target, indent);
        try ctx.write(";");
    }

    // ─── Composition (.dealx) writers ─────────────────────────────────────

    fn writeSystemBlock(ctx: *FormatContext, p: *const ast.SystemBlock, indent: u32) !void {
        try ctx.write("[<system");
        if (p.name) |n| {
            try ctx.write(" ");
            try ctx.write(n);
        }
        try ctx.write(">]\n");
        for (p.children) |child| {
            try ctx.write("\n");
            try ctx.writeIndent(indent + 1);
            try ctx.writeDeclarationWithComments(child, indent + 1);
        }
        try ctx.writeIndent(indent);
        try ctx.write("[</system>]");
    }

    fn writeSubsystemBlock(ctx: *FormatContext, p: *const ast.SubsystemBlock, indent: u32) !void {
        try ctx.write("[<subsystem");
        if (p.name) |n| {
            try ctx.write(" ");
            try ctx.write(n);
        }
        try ctx.write(">]\n");
        for (p.children) |child| {
            try ctx.writeIndent(indent + 1);
            try ctx.writeDeclarationWithComments(child, indent + 1);
        }
        try ctx.writeIndent(indent);
        try ctx.write("[</subsystem>]");
    }

    fn writeComponentInstance(ctx: *FormatContext, p: *const ast.ComponentInstance, indent: u32) !void {
        try ctx.write("[<");
        if (p.type_ref) |t| try ctx.write(t);
        if (p.name) |n| {
            try ctx.write(" as=\"");
            try writeStringEscaped(ctx.allocator, ctx.buf, n);
            try ctx.write("\"");
        }
        for (p.attrs) |attr| {
            try ctx.write(" ");
            try ctx.writeTagAttr(attr, indent);
        }
        try ctx.write(" />]");
    }

    fn writeCompConnect(ctx: *FormatContext, p: *const ast.CompConnect, indent: u32) !void {
        try ctx.write("[<connect");
        if (p.from_expr) |from| {
            try ctx.write(" from=");
            try ctx.writeAttrValue(from, indent);
        }
        if (p.to_expr) |to| {
            try ctx.write(" to=");
            try ctx.writeAttrValue(to, indent);
        }
        if (p.via_expr) |via| {
            try ctx.write(" via=");
            try ctx.writeAttrValue(via, indent);
        }
        if (p.carrying_expr) |car| {
            try ctx.write(" carrying=");
            try ctx.writeAttrValue(car, indent);
        }
        for (p.other_props) |prop| {
            try ctx.write(" ");
            try ctx.writeTagAttr(prop, indent);
        }
        try ctx.write(" />]");
    }

    fn writeExposeTag(ctx: *FormatContext, p: *const ast.ExposeTag, indent: u32) !void {
        try ctx.write("[<expose");
        if (p.target) |t| {
            try ctx.write(" ");
            try ctx.write(t);
        }
        for (p.attrs) |attr| {
            try ctx.write(" ");
            try ctx.writeTagAttr(attr, indent);
        }
        try ctx.write(" />]");
    }

    fn writeTraceabilityBlock(ctx: *FormatContext, p: *const ast.TraceabilityBlock, indent: u32) !void {
        try ctx.write("[<traceability>]\n");
        for (p.children) |child| {
            try ctx.writeIndent(indent + 1);
            try ctx.writeDeclarationWithComments(child, indent + 1);
        }
        try ctx.writeIndent(indent);
        try ctx.write("[</traceability>]");
    }

    fn writeAllocateTag(ctx: *FormatContext, p: *const ast.AllocateTag, indent: u32) !void {
        try ctx.write("[<allocate");
        for (p.attrs) |attr| {
            try ctx.write(" ");
            try ctx.writeTagAttr(attr, indent);
        }
        try ctx.write(" />]");
    }

    /// Emit a dealx tag attribute in `key=value` syntax.
    /// Attrs are wrapped in object_literal nodes with a single field.
    /// Values: string_literal → "value", structural_relationship → <<op>>,
    /// identifier → name (unquoted), other → {expr} (braced for parseAttributes).
    fn writeTagAttr(ctx: *FormatContext, attr: *ast.Node, indent: u32) !void {
        if (attr.kind != .object_literal) {
            try ctx.writeNode(attr, indent);
            return;
        }
        const ol = &attr.payload.object_literal;
        for (ol.fields) |f| {
            try ctx.write(f.key);
            try ctx.write("=");
            try ctx.writeAttrValue(f.value, indent);
        }
    }

    /// Emit a tag attribute VALUE in the canonical form that parseAttributes
    /// can re-parse:
    ///   - string_literal  → "value"  (double-quoted)
    ///   - structural_relationship → <<op>>
    ///   - identifier      → name     (unquoted bare identifier)
    ///   - int/real/bool   → literal text (unquoted, parseAttributes handles these)
    ///   - everything else → {expr}   (braced — parseAttributes `.l_brace` case)
    ///
    /// The `{...}` wrapper is required because `parseAttributes` only accepts
    /// bare literals, bare identifiers, and `{expr}` for complex values.
    /// Emitting `kWh(89)` without braces causes parse errors on re-parse.
    fn writeAttrValue(ctx: *FormatContext, value: *ast.Node, indent: u32) !void {
        switch (value.kind) {
            .string_literal => {
                try ctx.writeByte('"');
                try writeStringEscaped(ctx.allocator, ctx.buf, value.payload.string_literal.value);
                try ctx.writeByte('"');
            },
            .structural_relationship => {
                try ctx.write("<<");
                try ctx.write(value.payload.structural_relationship.op_text);
                try ctx.write(">>");
            },
            .identifier => try ctx.write(value.payload.identifier.name),
            .int_literal => try ctx.write(value.payload.int_literal.text),
            .real_literal => try ctx.write(value.payload.real_literal.text),
            .boolean_literal => try ctx.write(if (value.payload.boolean_literal.value) "true" else "false"),
            else => {
                // Complex expressions (call nodes, object_literals, etc.) must
                // be wrapped in braces so parseAttributes re-parses via the
                // `.l_brace` case. Without braces, `kWh(89)` would be parsed
                // as identifier `kWh` and then `(89)` causes E0103 errors.
                try ctx.write("{");
                try ctx.writeNode(value, indent);
                try ctx.write("}");
            },
        }
    }

    fn writeSatisfyBlock(ctx: *FormatContext, p: *const ast.SatisfyBlock, indent: u32) !void {
        // Children layout (set by parseSatisfyBlock):
        //   [0..N): object_literal nodes = opening tag attrs (requirement=, by=, etc.)
        //   [N]:    optional annotation_body node = `=> { ReturnField; ... }` block
        //   [N+1..): annotation nodes = inner sub-blocks (criteria, evidence, etc.)
        //
        // Emit: [<satisfy attr1="v1" ...>]\n [=> { ... }]? inner_blocks [</satisfy>]
        try ctx.write("[<satisfy");
        var body_start: usize = 0;
        for (p.children) |child| {
            if (child.kind == .object_literal and child.payload.object_literal.type_name == null) {
                try ctx.write(" ");
                try ctx.writeTagAttr(child, indent);
                body_start += 1;
            } else break;
        }
        try ctx.write(">]");

        // Optional return type block (annotation_body immediately after attrs).
        const body_children = p.children[body_start..];
        var inner_start: usize = 0;
        if (body_children.len > 0 and body_children[0].kind == .annotation_body) {
            // Emit as `=> { key: TypeName; ... }`
            try ctx.write(" => {\n");
            const ab = &body_children[0].payload.annotation_body;
            for (ab.fields) |f| {
                try ctx.writeIndent(indent + 2);
                try ctx.write(f.key);
                try ctx.write(": ");
                // Value is a string_literal holding the type name
                if (f.value.kind == .string_literal) {
                    try ctx.write(f.value.payload.string_literal.value);
                } else {
                    try ctx.writeNode(f.value, indent + 2);
                }
                try ctx.write(";\n");
            }
            try ctx.writeIndent(indent + 1);
            try ctx.write("}");
            inner_start = 1;
        }
        try ctx.write("\n");

        // Inner sub-blocks (annotation nodes with _body field).
        for (body_children[inner_start..]) |child| {
            try ctx.writeIndent(indent + 1);
            if (child.kind == .annotation) {
                const ann = &child.payload.annotation;
                // Emit as `keyword [sub_keyword] { raw_body }`
                // NOT as `@keyword` — these are satisfy inner blocks, not DEAL annotations.
                try ctx.write(ann.name);
                if (ann.body) |b_node| {
                    if (b_node.kind == .annotation_body) {
                        const ab = &b_node.payload.annotation_body;
                        if (ab.fields.len == 1 and std.mem.eql(u8, ab.fields[0].key, "_body")) {
                            // Raw body — emit verbatim inside `{ ... }`
                            try ctx.write(" {");
                            if (ab.fields[0].value.kind == .string_literal) {
                                try ctx.write(ab.fields[0].value.payload.string_literal.value);
                            }
                            try ctx.write("}");
                        } else {
                            try ctx.write(" ");
                            try ctx.writeNode(b_node, indent + 1);
                        }
                    } else {
                        try ctx.write(" ");
                        try ctx.writeNode(b_node, indent + 1);
                    }
                }
                try ctx.write("\n");
            } else {
                try ctx.writeDeclarationWithComments(child, indent + 1);
            }
        }
        try ctx.writeIndent(indent);
        try ctx.write("[</satisfy>]");
    }

    fn writeValidateTag(ctx: *FormatContext, p: *const ast.ValidateTag, indent: u32) !void {
        // ValidateTag is a PAIR-tag: [<validate ...>] body [</validate>]
        // The parser merges header attrs AND body key:value fields into the same
        // attrs array (all as object_literal nodes). The canonical stable form
        // is to emit all attrs in the opening header (using `=` syntax) with the
        // `>]` close (not self-closing `/>]`), then an empty body, then `[</validate>]`.
        // This round-trips: parseAttributes reads all attrs until `>]`, body loop
        // reads nothing, same attrs reconstructed.
        try ctx.write("[<validate");
        for (p.attrs) |attr| {
            try ctx.write(" ");
            try ctx.writeTagAttr(attr, indent);
        }
        try ctx.write(">]");
        try ctx.write("\n");
        try ctx.writeIndent(indent);
        try ctx.write("[</validate>]");
    }

    fn writeObjectLiteral(ctx: *FormatContext, p: *const ast.ObjectLiteral, indent: u32) !void {
        if (p.type_name) |tn| {
            try ctx.write(tn);
            try ctx.write(" ");
        }
        try ctx.write("{");
        if (p.fields.len > 0) {
            for (p.fields, 0..) |f, i| {
                if (i > 0) try ctx.write(", ");
                try ctx.write(f.key);
                try ctx.write(": ");
                try ctx.writeNode(f.value, indent);
            }
        }
        try ctx.write("}");
    }
};

// ─── Helper structures ──────────────────────────────────────────────────────

const NodeComments = struct {
    leading: []const ast.Comment,
    trailing: []const ast.Comment,
    /// D-29 typed DocComment struct (may be null if not populated yet).
    doc: ?*const ast.DocComment,
    /// The doc_comment Node from the ElementDef.doc field (the pre-Plan-02-01
    /// doc comment node stored inline before the definition).
    doc_node: ?*ast.Node,
};

/// Extract comment fields from nodes that have them (ElementDef and ElementUsage).
/// Returns null for nodes that don't carry comment fields.
fn getNodeComments(node: *ast.Node) ?NodeComments {
    return switch (node.payload) {
        .part_def => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .actor_def => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .port_def => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .action_def => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .state_def => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .attribute_def => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .item_def => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .interface_def => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .connection_def => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .flow_def => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .allocation_def => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .requirement_def => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        // constraint_def uses ConstraintDefinition (no trailing_comments field — Phase 05.2)
        .constraint_def => |*p| .{ .leading = p.leading_comments, .trailing = &.{}, .doc = p.doc_comment, .doc_node = p.doc },
        // calc_def uses CalcDefinition (no trailing_comments field — Phase 05.2)
        .calc_def => |*p| .{ .leading = p.leading_comments, .trailing = &.{}, .doc = p.doc_comment, .doc_node = p.doc },
        .need_def => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .use_case_def => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .part_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .port_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .attribute_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .action_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .actor_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .subject_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .state_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .item_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .interface_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .connection_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .flow_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .allocation_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .requirement_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .constraint_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .need_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        .use_case_usage => |*p| .{ .leading = p.leading_comments, .trailing = p.trailing_comments, .doc = p.doc_comment, .doc_node = p.doc },
        else => null,
    };
}

// ─── String helpers ─────────────────────────────────────────────────────────

/// Append bytes with JSON-style string escaping for embedded quotes/backslashes/etc.
fn writeStringEscaped(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), s: []const u8) !void {
    for (s) |c| {
        switch (c) {
            '"' => try buf.appendSlice(allocator, "\\\""),
            '\\' => try buf.appendSlice(allocator, "\\\\"),
            '\n' => try buf.appendSlice(allocator, "\\n"),
            '\r' => try buf.appendSlice(allocator, "\\r"),
            '\t' => try buf.appendSlice(allocator, "\\t"),
            else => try buf.append(allocator, c),
        }
    }
}

fn appendU32(allocator: std.mem.Allocator, buf: *std.ArrayList(u8), n: u32) !void {
    var tmp: [10]u8 = undefined;
    const s = std.fmt.bufPrint(&tmp, "{d}", .{n}) catch unreachable;
    try buf.appendSlice(allocator, s);
}

// ─── Keyword helpers ────────────────────────────────────────────────────────

fn modifierKw(m: ast.Modifier) []const u8 {
    return switch (m) {
        .abstract => "abstract",
        .derived => "derived",
        .readonly => "readonly",
        .ordered => "ordered",
        .nonunique => "nonunique",
        .individual => "individual",
        .variation => "variation",
        .portion => "portion",
        .end_kw => "end",
        .ref_kw => "ref",
    };
}

fn directionKw(d: ast.Direction) []const u8 {
    return switch (d) {
        .none => "",
        .in => "in",
        .out => "out",
        .inout => "inout",
    };
}

fn visibilityKw(v: ast.Visibility) []const u8 {
    return switch (v) {
        .public => "public",
        .protected => "protected",
        .private => "private",
    };
}

fn binaryOpText(op: ast.BinaryOp) []const u8 {
    return switch (op) {
        .add => "+",
        .sub => "-",
        .mul => "*",
        .div => "/",
        .eq => "==",
        .neq => "!=",
        .lt => "<",
        .le => "<=",
        .gt => ">",
        .ge => ">=",
        .log_and => "AND",
        .log_or => "OR",
    };
}

fn unaryOpText(op: ast.UnaryOp) []const u8 {
    return switch (op) {
        .neg => "-",
        .not => "!",
        .bang => "!",
    };
}

fn verifFieldKindKw(k: ast.VerifFieldKind) []const u8 {
    return switch (k) {
        .accepts => "accepts",
        .rejects => "rejects",
        .threshold => "threshold",
        .operator => "operator",
        .conditions => "conditions",
    };
}

// ─── Tests ──────────────────────────────────────────────────────────────────

test "fmt.empty_source" {
    const allocator = std.testing.allocator;
    const result = try emitFormatted(allocator, null);
    defer allocator.free(result);
    try std.testing.expectEqualStrings("", result);
}
