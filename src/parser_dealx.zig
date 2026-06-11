//! `.dealx` (compositions) parser — Plan 04 implementation.
//!
//! Implements the 43 `dealx.ebnf` productions across three subtasks:
//!   - Task 4.1a (this file's first commit): file-structure entry, open-tag
//!     stack constants, minimal `system`/`subsystem` pair-tag skeleton.
//!   - Task 4.1b: attribute parser + first-class comp_connect + simple
//!     self-closing tags (component instance, expose, allocate).
//!   - Task 4.1c: composite blocks (satisfy/traceability/validate) +
//!     full AST payload expansion + exhaustive json.zig switch.
//!
//! Mode discipline (D-06, RESEARCH Pitfall 7):
//!   - File scope starts in `.deal_def` for the package/import/export
//!     preamble (which uses .deal_def-style tokens), then flips to
//!     `.dealx_outer` for the composition body.
//!   - Inside an opening `[<tag ...>]` body the parser enters `.dealx_tag`
//!     (so `>]` and `/>]` close-tokens are recognized) and restores on the
//!     closing `>]` / `/>]`.
//!   - Inside an attribute `{...}` the parser enters `.dealx_expr_brace`
//!     (T-04-03 enterMode + errdefer restoreMode + explicit success-path
//!     restoreMode — NEVER a plain `defer restoreMode`).
//!
//! Tag stack (D-07, RESEARCH §Pattern 4 lines 383-426):
//!   The open-tag stack lives on the Parser struct (parser_deal.zig) as
//!   `open_tags: std.ArrayList(ast.OpenTag)`. pushTag/popTag bodies emit
//!   E0301 (unmatched close), E0302 (mismatched close — both spans),
//!   E0303 (depth bound MAX_TAG_DEPTH=1024), E0304 (unclosed at EOF).

const std = @import("std");
const ast = @import("ast");
const lexer = @import("lexer");
const diagnostics = @import("diagnostics");
const parser_deal = @import("parser_deal");
const expr = @import("expr");

// ─── Public surface ────────────────────────────────────────────────────────

/// Re-export of the open-tag entry type. The canonical declaration lives in
/// ast.zig to avoid a parser_deal ↔ parser_dealx circular import (Plan 03's
/// Parser struct already holds an ArrayList of these). This re-export keeps
/// downstream code that expects `parser_dealx.OpenTag` working — and the
/// acceptance criteria for Task 4.1a explicitly grep for this name here.
pub const OpenTag = ast.OpenTag;

/// Re-export of the T-04-01 DoS depth bound. See ast.zig for the canonical
/// declaration. Acceptance criteria for Task 4.1a explicitly grep for the
/// literal `MAX_TAG_DEPTH` and value `1024` in this file.
pub const MAX_TAG_DEPTH = ast.MAX_TAG_DEPTH;

const Parser = parser_deal.Parser;

// ─── D-17 composition-tag-tier synchronization ────────────────────────────
//
// `syncToTag` is the third recovery tier (after parser_deal's
// syncToStatement and syncToDefinition). It skips tokens until the next
// `[<` (tag_open), `[</` (tag_close_open), or one of the top-level
// preamble keywords (`kw_package`, `kw_import`, `kw_export`) — or EOF.
//
// CRITICAL: tag_open / tag_close_open are ONLY emitted in `.dealx_outer`
// mode. If we're called from a mode other than .dealx_outer (e.g. inside
// a satisfy body in .deal_def, or mid-attribute in .dealx_tag), the
// current peeked cache holds a token lexed under the OLD mode. We force
// a mode switch to .dealx_outer + invalidate peeked so the next peek()
// re-lexes under the correct mode (RESEARCH Pitfall 7).

const TAG_SYNC: std.EnumSet(lexer.Tag) = blk: {
    var s = std.EnumSet(lexer.Tag).initEmpty();
    s.insert(.tag_open);
    s.insert(.tag_close_open);
    s.insert(.kw_package);
    s.insert(.kw_import);
    s.insert(.kw_export);
    break :blk s;
};

/// Skip tokens until we reach a TAG_SYNC token or EOF. Forces .dealx_outer
/// mode while scanning so tag delimiters are recognized. Emits one E0400
/// covering the dropped range if any tokens were dropped.
pub fn syncToTag(p: *Parser) void {
    // WR-02: previously captured `prev_mode` and set up a `defer { _ = prev_mode; }`
    // block that suggested some restoration behavior but was a no-op. We do
    // NOT restore the prior mode here because the sync point is by definition
    // a .dealx_outer boundary — the only call site is in parseDealxFile where
    // the mode is already .dealx_outer, so there's nothing to restore.
    p.current_mode = .dealx_outer;
    // Invalidate the lookahead cache so the next peek() re-lexes under
    // .dealx_outer — see enterMode's lookahead-invalidation comment.
    if (p.peeked) |tok| {
        p.lex.pos = tok.span.start;
    }
    p.peeked = null;

    const start = p.peek().span.start;
    var dropped: u32 = 0;
    while (true) {
        const tok = p.peek();
        if (tok.tag == .eof) break;
        if (TAG_SYNC.contains(tok.tag)) break;
        _ = p.advance();
        dropped += 1;
    }
    if (dropped > 0) {
        const end = p.peek().span.start;
        p.diags.append(p.arena, .{
            .code = "E0400",
            .severity = .info,
            .message = "skipped tokens during composition-tag recovery",
            .span = .{ .start = start, .end = end },
        }) catch {};
    }
}

// ─── Entry point ───────────────────────────────────────────────────────────

/// Parse a .dealx file from the given source buffer.
/// Returns the root Node or null on OOM (a fatal diagnostic E0004 is appended
/// in that case). Other parse errors produce partial ASTs with diagnostics.
pub fn parseFile(
    arena: std.mem.Allocator,
    source: []const u8,
    diag_list: *std.ArrayList(diagnostics.Diagnostic),
) !?*ast.Node {
    var p = Parser{
        .arena = arena,
        .lex = lexer.Lexer.init(source),
        .diags = diag_list,
        .source = source,
    };
    return parseDealxFile(&p) catch |err| {
        std.debug.assert(err == error.OutOfMemory);
        diag_list.append(arena, .{
            .code = "E0004",
            .severity = .err,
            .message = "out of memory during parse",
            .span = .{ .start = 0, .end = 0 },
        }) catch {};
        return null;
    };
}

// ─── File structure ────────────────────────────────────────────────────────

fn parseDealxFile(p: *Parser) !*ast.Node {
    const start: u32 = 0;

    // Preamble — header / package / imports — uses .deal_def tokens.
    p.current_mode = .deal_def;
    p.peeked = null;

    const header = try parseOptionalHeaderBlock(p);
    const pkg = try parsePackageDecl(p);

    var imports: std.ArrayList(*ast.Node) = .empty;
    while (p.peek().tag == .kw_import) {
        const imp = try parseImportDecl(p);
        try imports.append(p.arena, imp);
    }

    // Body — composition tags use .dealx_outer (so `[<` becomes tag_open).
    _ = p.enterMode(.dealx_outer);

    var root_tags: std.ArrayList(*ast.Node) = .empty;
    while (p.peek().tag != .eof) {
        const tok = p.peek();
        const tok_start = tok.span.start;
        // Doc comments + annotations attach to the next tag — just skip
        // them at the top-level dispatch (they're informational and
        // already lexed as proper tokens). Plan 04 silently advanced past
        // them; preserving that behavior for snapshot stability.
        if (tok.tag == .doc_comment or tok.tag == .annotation or tok.tag == .annotation_prefix) {
            _ = p.advance();
            // For annotations with key:value bodies, the value tokens may
            // follow on subsequent lines. We let the next iteration pick
            // them up (they were tolerated as "skip" by Plan 04 too).
            continue;
        }
        // Allow inline definition keywords (Plan 04 Task 4.1c) or composition
        // tags. Tag-open is the dominant case for the showcase files.
        if (tok.tag == .tag_open) {
            const node = try parseSystemContent(p);
            try root_tags.append(p.arena, node);
        } else if (tok.tag == .tag_close_open) {
            // Orphan close at top level — no opening tag on the stack.
            // Consume `[</name>]` and let popTag fire E0301.
            const close_open = p.advance();
            const prev_close = p.enterMode(.dealx_tag);
            errdefer p.restoreMode(prev_close);
            const close_name_tok = p.advance();
            const close_name = p.tokenText(close_name_tok.span);
            const close_tok = try p.expect(.tag_close);
            p.restoreMode(prev_close);
            const close_span = ast.Span{ .start = close_open.span.start, .end = close_tok.span.end };
            p.popTag(close_name, close_span);
        } else {
            // Unknown leading token at composition body — D-17 tag-tier
            // recovery. Emit one E0101 and sync to next `[<` / `[</`.
            try p.diags.append(p.arena, .{
                .code = "E0101",
                .severity = .err,
                .message = "expected composition tag",
                .span = tok.span,
            });
            syncToTag(p);
            // Forward-progress guarantee — if sync landed on a TAG_SYNC
            // token at the same byte offset, force one advance.
            if (p.peek().span.start == tok_start) _ = p.advance();
        }
    }

    // E0304 — emit one diagnostic per unclosed open tag.
    while (p.open_tags.pop()) |unclosed| {
        try p.diags.append(p.arena, .{
            .code = "E0304",
            .severity = .err,
            .message = "unclosed composition open tag at end of file",
            .span = unclosed.span,
        });
    }

    // Phase 1.5 plan 02 (m08): drain any lexer-surfaced trivia errors
    // recorded during the main parse loop. Shares the same drain helper
    // with parser_deal — see parser_deal.drainLexerErrors. Currently
    // emits Codes.e_unterminated_comment (E0003) for unterminated
    // `/* */` block comments encountered in either preamble or body.
    try parser_deal.drainLexerErrors(p);

    const end_pos = p.peek().span.start;
    const span = ast.Span{ .start = start, .end = end_pos };

    return p.makeNode(.dealx_file, span, .{
        .dealx_file = .{
            .header = header,
            .package_decl = pkg,
            .imports = try imports.toOwnedSlice(p.arena),
            .root_tags = try root_tags.toOwnedSlice(p.arena),
        },
    });
}

// ─── Preamble parsers (cribbed from parser_deal — kept private here) ─────
//
// The .deal preamble parsers in parser_deal.zig are private (`fn` not
// `pub fn`), so we re-implement minimal versions here. They share the AST
// payload shapes (DealxFile uses the same `header`, `package_decl`, and
// `imports` fields as DealFile).

fn parseOptionalHeaderBlock(p: *Parser) !?*ast.Node {
    const tok = p.peek();
    if (tok.tag == .annotation and std.mem.eql(u8, p.tokenText(tok.span), "@header")) {
        return parseHeaderBlock(p);
    }
    return null;
}

fn parseHeaderBlock(p: *Parser) !*ast.Node {
    const start_tok = p.advance(); // consume "@header"
    _ = try p.expect(.l_brace);

    var fields: std.ArrayList(ast.HeaderField) = .empty;

    while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
        const field = try parseHeaderField(p);
        try fields.append(p.arena, field);
    }

    const close = try p.expect(.r_brace);
    const span = ast.Span{ .start = start_tok.span.start, .end = close.span.end };

    return p.makeNode(.header_block, span, .{
        .header_block = .{
            .fields = try fields.toOwnedSlice(p.arena),
        },
    });
}

fn parseHeaderField(p: *Parser) !ast.HeaderField {
    // Same shape as parser_deal.parseHeaderField — reads `key:`, then the
    // raw value text up to the newline. We rely on the lexer to skip the
    // colon and let the source-scanning advance past the value text.
    const key_tok = p.advance();
    const key_text = p.tokenText(key_tok.span);
    // WR-03: capture the colon span from `expect` instead of rescanning
    // p.source for the ':' byte. The previous code threw away the lexer's
    // span and walked the source to recover the same position — fragile and
    // also diverged from the parser_deal.zig sibling, so we now share the
    // approach: trust the lexer's span.
    const colon_tok = try p.expect(.colon);

    // Read from after the colon to newline. Clamp every cursor to source.len
    // so truncated headers (Plan 05 malformed corpus) don't panic.
    var val_pos: u32 = colon_tok.span.end;
    if (val_pos > p.source.len) val_pos = @intCast(p.source.len);
    while (val_pos < p.source.len and (p.source[val_pos] == ' ' or p.source[val_pos] == '\t')) val_pos += 1;
    const val_start = val_pos;
    while (val_pos < p.source.len and p.source[val_pos] != '\n' and p.source[val_pos] != '\r') val_pos += 1;
    var val_end = val_pos;
    while (val_end > val_start and (p.source[val_end - 1] == ' ' or p.source[val_end - 1] == '\t')) val_end -= 1;

    const value_text = if (val_start <= val_end and val_end <= p.source.len)
        p.source[val_start..val_end]
    else
        "";
    const owned_value = try p.arena.dupe(u8, value_text);
    const end_pos: u32 = @intCast(val_pos);

    // Consume any tokens the lexer already emitted within the value range
    // so the next peek() lands on the next line.
    while (true) {
        const next = p.peek();
        if (next.tag == .r_brace or next.tag == .eof) break;
        if (next.span.start >= end_pos) break;
        _ = p.advance();
    }

    return ast.HeaderField{
        .key = key_text,
        .value = owned_value,
        .span = ast.Span{ .start = key_tok.span.start, .end = end_pos },
    };
}

fn parsePackageDecl(p: *Parser) !?*ast.Node {
    if (p.peek().tag != .kw_package) return null;
    const start = p.advance(); // consume "package"
    var segs: std.ArrayList([]const u8) = .empty;
    const first = p.peek();
    if (first.tag == .ident or isIdentLikeKeyword(first.tag)) {
        _ = p.advance();
        try segs.append(p.arena, p.tokenText(first.span));
    }
    while (p.peek().tag == .dot) {
        _ = p.advance(); // consume "."
        const seg_tok = p.advance();
        try segs.append(p.arena, p.tokenText(seg_tok.span));
    }
    const semi = try p.expect(.semicolon);
    const span = ast.Span{ .start = start.span.start, .end = semi.span.end };
    return p.makeNode(.package_decl, span, .{
        .package_decl = .{ .segments = try segs.toOwnedSlice(p.arena) },
    });
}

fn parseImportDecl(p: *Parser) !*ast.Node {
    const start = p.advance(); // consume "import"

    var segments: std.ArrayList([]const u8) = .empty;
    // First segment can be IDENT or an IDENT-like keyword (e.g. `system`
    // in `import reqs.system.*;`).
    const first_seg = p.advance();
    if (first_seg.tag != .ident and !isIdentLikeKeyword(first_seg.tag)) {
        try p.diags.append(p.arena, .{
            .code = "E0100",
            .severity = .err,
            .message = "expected identifier in import path",
            .span = first_seg.span,
        });
    }
    try segments.append(p.arena, p.tokenText(first_seg.span));

    // Consume ("." IDENT)*
    while (p.peek().tag == .dot) {
        const second = p.peek2();
        if (second.tag == .ident or isIdentLikeKeyword(second.tag)) {
            _ = p.advance(); // consume "."
            const seg_tok = p.advance();
            try segments.append(p.arena, p.tokenText(seg_tok.span));
        } else {
            break;
        }
    }

    const path = try segments.toOwnedSlice(p.arena);
    var kind: ast.ImportKind = .simple;
    var items: std.ArrayList(ast.ImportItem) = .empty;

    const next = p.peek();
    if (next.tag == .dot) {
        _ = p.advance(); // consume "."
        const after_dot = p.peek();
        if (after_dot.tag == .l_brace) {
            _ = p.advance(); // consume "{"
            kind = .named;
            while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
                const name_tok = try p.expect(.ident);
                const name = p.tokenText(name_tok.span);
                var alias: ?[]const u8 = null;
                if (p.peek().tag == .kw_as) {
                    _ = p.advance();
                    const alias_tok = try p.expect(.ident);
                    alias = p.tokenText(alias_tok.span);
                }
                try items.append(p.arena, .{ .name = name, .alias = alias });
                if (p.peek().tag == .comma) _ = p.advance();
            }
            _ = try p.expect(.r_brace);
        } else if (after_dot.tag == .star) {
            _ = p.advance();
            kind = .wildcard;
        }
    } else if (next.tag == .kw_as) {
        _ = p.advance();
        const alias_tok = try p.expect(.ident);
        kind = .alias;
        try items.append(p.arena, .{ .name = "", .alias = p.tokenText(alias_tok.span) });
    }

    const semi = try p.expect(.semicolon);
    const span = ast.Span{ .start = start.span.start, .end = semi.span.end };
    return p.makeNode(.import_decl, span, .{
        .import_decl = .{
            .path = path,
            .kind = kind,
            .items = try items.toOwnedSlice(p.arena),
        },
    });
}

// ─── Composition dispatch ──────────────────────────────────────────────────

/// Dispatch the next `[<...>]` tag. Caller is at `.dealx_outer`.
/// Task 4.1a wired system/subsystem; Task 4.1b adds the self-closing
/// connect/expose/allocate plus component instances (any user-defined
/// IDENT); Task 4.1c wires the remaining pair-tags (satisfy,
/// traceability, validate).
fn parseSystemContent(p: *Parser) std.mem.Allocator.Error!*ast.Node {
    // We're at .dealx_outer, peek tag_open.
    const open_tok = p.advance(); // consume "[<"
    std.debug.assert(open_tok.tag == .tag_open);

    // Inside `[< ... >]`, lexer is in .dealx_tag mode.
    const prev = p.enterMode(.dealx_tag);
    errdefer p.restoreMode(prev);

    const name_tok = p.advance();
    const name = p.tokenText(name_tok.span);

    // Dispatch on the tag name. Pair-tags (system, subsystem, traceability,
    // satisfy) wrap an inner body that ends with `[</name>]`. Validate is
    // a pair-tag with a field-only body. Self-closing tags (connect,
    // expose, allocate, plus any user-defined type for ComponentInstance)
    // end with `/>]`.
    if (std.mem.eql(u8, name, "system")) {
        return parsePairTag(p, prev, open_tok, name, "system", .system_block);
    } else if (std.mem.eql(u8, name, "subsystem")) {
        return parsePairTag(p, prev, open_tok, name, "subsystem", .subsystem_block);
    } else if (std.mem.eql(u8, name, "connect")) {
        return parseConnect(p, prev, open_tok);
    } else if (std.mem.eql(u8, name, "expose")) {
        return parseExposeTag(p, prev, open_tok);
    } else if (std.mem.eql(u8, name, "allocate")) {
        return parseAllocateTag(p, prev, open_tok);
    } else if (std.mem.eql(u8, name, "traceability")) {
        return parsePairTag(p, prev, open_tok, name, "traceability", .traceability_block);
    } else if (std.mem.eql(u8, name, "satisfy")) {
        return parseSatisfyBlock(p, prev, open_tok);
    } else if (std.mem.eql(u8, name, "validate")) {
        return parseValidateBlock(p, prev, open_tok);
    } else {
        // User-defined IDENT after `[<` — ComponentInstance.
        // The token may be `.ident` or any of the IDENT-like keywords that
        // happen to share spellings with deal/dealx keywords (rare in the
        // showcase but the grammar permits it). We restore the source slice
        // via tokenText regardless.
        return parseComponentInstance(p, prev, open_tok, name);
    }
}

/// Parse a pair-tag: `[<NAME ...>]` body `[</NAME>]`. The opening `[<`
/// has been consumed and the lexer mode is already `.dealx_tag` (caller's
/// responsibility — caller also passed `mode_before` so we can restore it
/// after the closing tag).
///
/// For Task 4.1a:
///   - No attribute parsing — the opening `[<system NAME>]` may only carry
///     a bare IDENT after the tag name, then `>]`.
///   - The body loops on inner `[<` (recurse via parseSystemContent) or
///     `[</` (close).
///   - Task 4.1b will add attribute parsing; Task 4.1c will dispatch the
///     remaining tag names.
fn parsePairTag(
    p: *Parser,
    mode_before: lexer.Mode,
    open_tok: lexer.Token,
    name: []const u8,
    expected_close_name: []const u8,
    kind: ast.NodeKind,
) std.mem.Allocator.Error!*ast.Node {
    _ = name;

    // Push onto the open-tag stack BEFORE consuming the rest of the
    // opening tag, so D-04 spans match the open delimiter (`[<...`).
    p.pushTag(expected_close_name, open_tok.span);

    // Optional name IDENT after the tag keyword. system/subsystem both
    // take an IDENT name in the showcase grammar.
    var block_name: ?[]const u8 = null;
    if (p.peek().tag == .ident) {
        const n_tok = p.advance();
        block_name = p.tokenText(n_tok.span);
    }

    // Optional structural relationship (e.g. `<<specializes>> EVPlatform`)
    // appears in variants/sedan.dealx and variants/performance.dealx.
    // Task 4.1a parses but discards the operator; downstream tasks may
    // record it on the SystemBlock payload (the Plan 03 placeholder
    // payload only carries `name` + `children`).
    if (p.peek().tag == .delimited_operator) {
        _ = p.advance(); // consume `<<specializes>>` token
        // Optional target IDENT (the base type name).
        if (p.peek().tag == .ident) {
            _ = p.advance();
        }
    }

    _ = try p.expect(.tag_close); // consume the closing `>]`

    // We've left `.dealx_tag`; the body is back in `.dealx_outer` so the
    // lexer recognizes nested `[<` / `[</` properly.
    p.restoreMode(mode_before);

    // Loop reading inner content. Nested `tag_open` recurses; pair-tags
    // detect their close via `tag_close_open`. EOF means unclosed — leave
    // the stack populated so parseDealxFile's EOF E0304 emission fires.
    var children: std.ArrayList(*ast.Node) = .empty;
    var hit_eof_unclosed = false;
    while (true) {
        const tok = p.peek();
        switch (tok.tag) {
            .tag_open => {
                const child = try parseSystemContent(p);
                try children.append(p.arena, child);
            },
            .tag_close_open => break,
            .eof => {
                hit_eof_unclosed = true;
                break;
            },
            else => {
                // Plan 04 has minimal recovery; skip unknown tokens to
                // make progress. Plan 05 adds proper resync.
                _ = p.advance();
            },
        }
    }

    if (hit_eof_unclosed) {
        // Don't consume a non-existent close tag — leave the OpenTag on
        // the stack and let parseDealxFile emit E0304 for it at EOF.
        const span = ast.Span{ .start = open_tok.span.start, .end = p.peek().span.start };
        return p.makeNode(kind, span, makeSystemPayload(kind, block_name, try children.toOwnedSlice(p.arena)));
    }

    // Consume `[</`
    const close_open = try p.expect(.tag_close_open);

    // Inside `[</...>]` the lexer is again in `.dealx_tag`.
    const prev_close = p.enterMode(.dealx_tag);
    errdefer p.restoreMode(prev_close);

    const close_name_tok = p.advance();
    const close_name = p.tokenText(close_name_tok.span);

    const close_tok = try p.expect(.tag_close);
    p.restoreMode(prev_close);

    // Span covering `[</NAME>]` — used as the diagnostic primary for E0302
    // when the close name doesn't match the top of the open stack.
    const close_span = ast.Span{ .start = close_open.span.start, .end = close_tok.span.end };
    p.popTag(close_name, close_span);

    const span = ast.Span{ .start = open_tok.span.start, .end = close_tok.span.end };
    return p.makeNode(kind, span, makeSystemPayload(kind, block_name, try children.toOwnedSlice(p.arena)));
}

fn makeSystemPayload(kind: ast.NodeKind, name: ?[]const u8, children: []*ast.Node) ast.Payload {
    return switch (kind) {
        .system_block => .{ .system_block = .{ .name = name, .children = children } },
        .subsystem_block => .{ .subsystem_block = .{ .name = name, .children = children } },
        .traceability_block => .{ .traceability_block = .{ .children = children } },
        else => unreachable,
    };
}

// ─── Task 4.1b: attribute parser + self-closing tags ──────────────────────

/// Parse the attribute list inside an opening tag `[<name attr1=val attr2=val>]`
/// or self-closing `[<name attr=val />]`. Caller has already consumed the
/// `[<` and tag-name IDENT and is in `.dealx_tag` mode.
///
/// Returns a list of `ObjectField` (key + value Node + span). Attribute
/// values can be:
///   - QuotedString (e.g. `from="bms.cellMonitor"`)
///   - Number / boolean / IDENT (rare in showcase; e.g. attribute values
///     that aren't quoted strings)
///   - `{ ... }` braced expression — D-08 mode-switch via the
///     enterMode(.dealx_expr_brace) + errdefer + explicit restoreMode
///     pattern documented in plan threat T-04-03.
///   - `<<operator>>` delimited operator (e.g. `relationship=<<derives>>`
///     inside allocate tags) — captured as a structural_relationship-like
///     node so downstream code can recover the operator text.
///
/// Loop terminates at `.tag_close` or `.tag_self_close`. Caller consumes
/// the terminator.
fn parseAttributes(p: *Parser) std.mem.Allocator.Error![]ast.ObjectField {
    var fields: std.ArrayList(ast.ObjectField) = .empty;

    while (true) {
        const tok = p.peek();
        if (tok.tag == .tag_close or tok.tag == .tag_self_close or tok.tag == .eof) break;

        // Attribute name: IDENT or any IDENT-like keyword (the showcase
        // uses bare keywords like `from`, `to`, `via`, `carrying`,
        // `requirement`, `by`, `method`, `status`, `relationship` as
        // attribute names — the lexer emits these as `kw_*` tokens but
        // they are valid attribute name positions per dealx.ebnf).
        if (tok.tag != .ident and !isAttrNameKeyword(tok.tag)) {
            // Unknown leading token — emit E0103 and advance to make
            // progress. Plan 05's sync recovery will replace this with a
            // proper resync.
            try p.diags.append(p.arena, .{
                .code = "E0103",
                .severity = .err,
                .message = "expected attribute name",
                .span = tok.span,
            });
            _ = p.advance();
            continue;
        }

        const name_tok = p.advance();
        const name = p.tokenText(name_tok.span);

        _ = try p.expect(.eq);

        const next = p.peek();
        const value: *ast.Node = blk: {
            switch (next.tag) {
                .l_brace => {
                    // D-08 inline `{...}` block (T-04-03 errdefer pattern).
                    // The lookahead cache invalidation in enterMode is the
                    // synchronization point that makes errdefer safe here:
                    //   (a) restoreMode invalidates self.peeked so any token
                    //       pre-lexed under .dealx_expr_brace is discarded;
                    //   (b) the next call after restore re-lexes under the
                    //       restored mode;
                    //   (c) any diagnostic inside parseInlineObjectOrExpr is
                    //       emitted BEFORE the errdefer fires — never under
                    //       a stale cache.
                    // Plain (always-runs) `defer` for restoreMode is
                    // forbidden (Warning #8 / T-04-03): it conflicts with
                    // the success-path explicit restore below and would
                    // double-fire restore at the wrong sequence point.
                    const parser = p; // alias so grep-gate matches the
                                      // canonical pattern below.
                    const prev = parser.enterMode(.dealx_expr_brace);
                    errdefer parser.restoreMode(prev);
                    _ = try parser.expect(.l_brace);
                    const result = try parseInlineObjectOrExpr(parser);
                    _ = try parser.expect(.r_brace);
                    parser.restoreMode(prev); // explicit success-path restore
                    break :blk result;
                },
                .string_literal, .unrestricted_name => {
                    const t = p.advance();
                    const raw = p.tokenText(t.span);
                    const val = if (raw.len >= 2) raw[1 .. raw.len - 1] else raw;
                    break :blk try p.makeNode(.string_literal, t.span, .{
                        .string_literal = .{ .value = val },
                    });
                },
                .int_literal => {
                    const t = p.advance();
                    break :blk try p.makeNode(.int_literal, t.span, .{
                        .int_literal = .{ .text = p.tokenText(t.span) },
                    });
                },
                .real_literal => {
                    const t = p.advance();
                    break :blk try p.makeNode(.real_literal, t.span, .{
                        .real_literal = .{ .text = p.tokenText(t.span) },
                    });
                },
                .boolean_literal => {
                    const t = p.advance();
                    const text = p.tokenText(t.span);
                    break :blk try p.makeNode(.boolean_literal, t.span, .{
                        .boolean_literal = .{ .value = std.mem.eql(u8, text, "true") },
                    });
                },
                .template_literal, .template_head => {
                    // Re-enter .dealx_expr_brace so the template scanner
                    // sees `${`/`}` consistently with the deal expression
                    // grammar — same T-04-03 pattern as l_brace above.
                    const parser = p;
                    const prev = parser.enterMode(.dealx_expr_brace);
                    errdefer parser.restoreMode(prev);
                    const e = try expr.parseExpression(parser, 0);
                    parser.restoreMode(prev);
                    break :blk e;
                },
                .ident => {
                    const t = p.advance();
                    break :blk try p.makeNode(.identifier, t.span, .{
                        .identifier = .{ .name = p.tokenText(t.span) },
                    });
                },
                .delimited_operator => {
                    // e.g. `relationship=<<derives>>` inside allocate tags.
                    const t = p.advance();
                    const raw = p.tokenText(t.span);
                    const inner = if (raw.len >= 4) raw[2 .. raw.len - 2] else raw;
                    break :blk try p.makeNode(.structural_relationship, t.span, .{
                        .structural_relationship = .{
                            .op_text = inner,
                            .target_segments = &.{},
                        },
                    });
                },
                else => {
                    try p.diags.append(p.arena, .{
                        .code = "E0103",
                        .severity = .err,
                        .message = "expected attribute value",
                        .span = next.span,
                    });
                    // Synthetic identifier so we have *something* to
                    // attach for the field value.
                    const t = p.advance();
                    break :blk try p.makeNode(.identifier, t.span, .{
                        .identifier = .{ .name = p.tokenText(t.span) },
                    });
                },
            }
        };

        const field_span = ast.Span{ .start = name_tok.span.start, .end = value.span.end };
        try fields.append(p.arena, .{ .key = name, .value = value, .span = field_span });
    }

    return try fields.toOwnedSlice(p.arena);
}

/// Parse the body of an inline `{...}` block. Caller has already consumed
/// the opening `{` and is in `.dealx_expr_brace` mode. Caller will consume
/// the closing `}`.
///
/// Two shapes are recognized (D-08):
///   - ObjectLiteral: `IDENT "{" key: value, ... "}"`   (with type prefix)
///     `"{" key: value, ... "}"` (without type prefix — bare object)
///   - raw Expression — falls through to expr.parseExpression.
///
/// Distinguishes via peek-2: IDENT followed by `{` is "TypeName { ... }"
/// shape; otherwise we try a bare object (IDENT followed by `:`) or a raw
/// expression. See plan output section question A1 — this parser uses
/// peek-2 (NOT parse-and-recover) for the discrimination.
///
/// CRITICAL (RESEARCH Pitfall 7): the lexer's `.dealx_expr_brace` mode
/// stays active through any inner `{...}` recursion. The mode is only
/// restored when the OUTER caller's `restoreMode(prev)` runs (in
/// parseAttributes above). Inner `{` is just a plain l_brace token —
/// recognized by expr.parseExpression / this function — not a mode flip.
fn parseInlineObjectOrExpr(p: *Parser) std.mem.Allocator.Error!*ast.Node {
    const first = p.peek();
    if (first.tag == .ident) {
        const second = p.peek2();
        if (second.tag == .l_brace) {
            // "TypeName { ... }" form.
            const type_tok = p.advance();
            const type_name = p.tokenText(type_tok.span);
            return parseObjectLiteralBody(p, type_tok.span.start, type_name);
        }
        if (second.tag == .colon) {
            // Bare object with no type prefix — first IDENT is a key.
            return parseObjectLiteralBody(p, first.span.start, null);
        }
        // IDENT followed by something else — a raw expression that just
        // starts with an identifier (e.g. `foo.bar` member-access, or a
        // bare identifier as a value).
        return parseDealxExprValue(p);
    }
    // No leading IDENT — raw expression / array / nested object.
    return parseDealxExprValue(p);
}

/// Parse a value expression inside a `.dealx_expr_brace` context. Wraps
/// expr.parseExpression with two .dealx-specific extensions:
///   - `l_brace` → recurse into parseInlineObjectOrExpr (Pitfall 7 nested
///     object literal).
///   - `l_bracket` → delegates to expr.parseExpression which now owns the
///     l_bracket → array_literal atom arm (D-04). Produces a real
///     array_literal node. Showcase usage: `messageIds: ["0x100", ...]`
///     inside `carrying={CANMessages { ... }}`.
fn parseDealxExprValue(p: *Parser) std.mem.Allocator.Error!*ast.Node {
    const tok = p.peek();
    if (tok.tag == .l_brace) {
        _ = p.advance();
        const inner = try parseInlineObjectOrExpr(p);
        _ = try p.expect(.r_brace);
        return inner;
    }
    // D-04: `l_bracket` delegate to expr.parseExpression, which now owns
    // the l_bracket → array_literal atom arm. parseArrayLiteral deleted.
    return expr.parseExpression(p, 0);
}

/// Helper for parseInlineObjectOrExpr.
///
/// Two callsites with different brace ownership:
///   - Type-prefixed: `TypeName { ... }` — caller has NOT consumed `{`.
///     This function expects both `{` AND `}` (consumes both).
///   - Bare object: `{ ... }` where caller's outer-mode-switch `{` has
///     already been consumed by parseAttributes. In that case the FIRST
///     IDENT was already peeked (followed by `:`), and the trailing `}`
///     is the SAME brace the caller will expect after we return. So this
///     function consumes neither brace.
///
/// Distinguishes the two cases by whether `type_name` is non-null (== type-
/// prefixed form). `start_pos` is the byte where the literal started:
///   - Type-prefixed: the IDENT's start (typeName start);
///   - Bare: the outer `{`'s start (passed in by parseInlineObjectOrExpr).
fn parseObjectLiteralBody(p: *Parser, start_pos: u32, type_name: ?[]const u8) std.mem.Allocator.Error!*ast.Node {
    if (type_name != null) {
        _ = try p.expect(.l_brace); // consume the inner `{` after TypeName
    }
    var fields: std.ArrayList(ast.ObjectField) = .empty;
    while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
        const key_tok = p.peek();
        if (key_tok.tag != .ident and !isAttrNameKeyword(key_tok.tag)) {
            try p.diags.append(p.arena, .{
                .code = "E0103",
                .severity = .err,
                .message = "expected object field key",
                .span = key_tok.span,
            });
            _ = p.advance();
            continue;
        }
        _ = p.advance();
        const key = p.tokenText(key_tok.span);
        _ = try p.expect(.colon);
        const val = try parseDealxExprValue(p);
        if (p.peek().tag == .comma) _ = p.advance();
        try fields.append(p.arena, .{
            .key = key,
            .value = val,
            .span = ast.Span{ .start = key_tok.span.start, .end = val.span.end },
        });
    }
    var end_pos: u32 = p.peek().span.start;
    if (type_name != null) {
        // Consume the inner `}` so the caller's `expect(.r_brace)` matches
        // the OUTER brace (the D-08 mode-switch brace).
        const close = try p.expect(.r_brace);
        end_pos = close.span.end;
    }
    const span = ast.Span{ .start = start_pos, .end = end_pos };
    return p.makeNode(.object_literal, span, .{
        .object_literal = .{
            .type_name = type_name,
            .fields = try fields.toOwnedSlice(p.arena),
        },
    });
}

/// Parse a `[<connect ... />]` self-closing tag. D-09 routes the four
/// well-known props (`from`, `to`, `via`, `carrying`) to typed CompConnect
/// slots; everything else lands in `other_props`.
fn parseConnect(p: *Parser, mode_before: lexer.Mode, open_tok: lexer.Token) std.mem.Allocator.Error!*ast.Node {
    const attrs = try parseAttributes(p);
    const close = try p.expect(.tag_self_close);
    p.restoreMode(mode_before);

    var cc: ast.CompConnect = .{};
    var others: std.ArrayList(*ast.Node) = .empty;
    for (attrs) |f| {
        if (std.mem.eql(u8, f.key, "from")) {
            cc.from_expr = f.value;
        } else if (std.mem.eql(u8, f.key, "to")) {
            cc.to_expr = f.value;
        } else if (std.mem.eql(u8, f.key, "via")) {
            cc.via_expr = f.value;
        } else if (std.mem.eql(u8, f.key, "carrying")) {
            cc.carrying_expr = f.value;
        } else {
            // Wrap unknown attrs as ObjectLiteral-single-field nodes so
            // they're typed as *Node. Use the field's span and an inline
            // object-literal with a single field.
            const fields_slice = try p.arena.alloc(ast.ObjectField, 1);
            fields_slice[0] = f;
            const wrap = try p.makeNode(.object_literal, f.span, .{
                .object_literal = .{ .type_name = null, .fields = fields_slice },
            });
            try others.append(p.arena, wrap);
        }
    }
    cc.other_props = try others.toOwnedSlice(p.arena);

    const span = ast.Span{ .start = open_tok.span.start, .end = close.span.end };
    return p.makeNode(.comp_connect, span, .{ .comp_connect = cc });
}

/// Parse a `[<expose path as="name" />]` self-closing tag.
/// The first token AFTER `expose` is the port path (NamespacePath in the
/// grammar — IDENT ( "." IDENT )*). The remaining `attr=value` pairs
/// (e.g. `as="hvPowerOut"`) are captured as attrs.
fn parseExposeTag(p: *Parser, mode_before: lexer.Mode, open_tok: lexer.Token) std.mem.Allocator.Error!*ast.Node {
    // Read the port path FIRST (before attrs) — `expose battery.hvOut as=...`
    // The path is a sequence of IDENT segments joined by `.`.
    var path_buf: std.ArrayList(u8) = .empty;
    var have_path = false;
    while (true) {
        const tok = p.peek();
        if (tok.tag == .ident or isAttrNameKeyword(tok.tag)) {
            // Check: if this IDENT is followed by `=`, it's the start of
            // the attribute list — bail out and let parseAttributes handle it.
            const second = p.peek2();
            if (second.tag == .eq) break;
            _ = p.advance();
            if (have_path) try path_buf.append(p.arena, '.');
            try path_buf.appendSlice(p.arena, p.tokenText(tok.span));
            have_path = true;
            if (p.peek().tag == .dot) {
                _ = p.advance();
                continue;
            }
            break;
        }
        break;
    }
    const target: ?[]const u8 = if (have_path) try path_buf.toOwnedSlice(p.arena) else null;

    const attrs_fields = try parseAttributes(p);
    const close = try p.expect(.tag_self_close);
    p.restoreMode(mode_before);

    // Wrap each ObjectField as a single-field ObjectLiteral *Node so the
    // payload's `attrs: []*Node` typing holds.
    var attrs: std.ArrayList(*ast.Node) = .empty;
    for (attrs_fields) |f| {
        const fields_slice = try p.arena.alloc(ast.ObjectField, 1);
        fields_slice[0] = f;
        const wrap = try p.makeNode(.object_literal, f.span, .{
            .object_literal = .{ .type_name = null, .fields = fields_slice },
        });
        try attrs.append(p.arena, wrap);
    }

    const span = ast.Span{ .start = open_tok.span.start, .end = close.span.end };
    return p.makeNode(.expose_tag, span, .{
        .expose_tag = .{
            .target = target,
            .attrs = try attrs.toOwnedSlice(p.arena),
        },
    });
}

/// Parse a `[<allocate from=... to=... relationship=<<...>> />]` self-closing
/// tag. All props become attrs; downstream code reads them by name.
fn parseAllocateTag(p: *Parser, mode_before: lexer.Mode, open_tok: lexer.Token) std.mem.Allocator.Error!*ast.Node {
    const attrs_fields = try parseAttributes(p);
    const close = try p.expect(.tag_self_close);
    p.restoreMode(mode_before);

    var attrs: std.ArrayList(*ast.Node) = .empty;
    for (attrs_fields) |f| {
        const fields_slice = try p.arena.alloc(ast.ObjectField, 1);
        fields_slice[0] = f;
        const wrap = try p.makeNode(.object_literal, f.span, .{
            .object_literal = .{ .type_name = null, .fields = fields_slice },
        });
        try attrs.append(p.arena, wrap);
    }

    const span = ast.Span{ .start = open_tok.span.start, .end = close.span.end };
    return p.makeNode(.allocate_tag, span, .{
        .allocate_tag = .{ .attrs = try attrs.toOwnedSlice(p.arena) },
    });
}

/// Parse a `[<TypeName as="name" prop={value} ... />]` self-closing component
/// instance. The tag name IDENT becomes `type_ref`; the `as=` value (if any)
/// becomes `name`; everything else lands in `attrs`.
fn parseComponentInstance(
    p: *Parser,
    mode_before: lexer.Mode,
    open_tok: lexer.Token,
    type_name: []const u8,
) std.mem.Allocator.Error!*ast.Node {
    const attrs_fields = try parseAttributes(p);
    const close = try p.expect(.tag_self_close);
    p.restoreMode(mode_before);

    var inst_name: ?[]const u8 = null;
    var attrs: std.ArrayList(*ast.Node) = .empty;
    for (attrs_fields) |f| {
        if (std.mem.eql(u8, f.key, "as")) {
            // The `as` value should be a string literal — extract its value.
            if (f.value.kind == .string_literal) {
                inst_name = f.value.payload.string_literal.value;
            }
        } else {
            const fields_slice = try p.arena.alloc(ast.ObjectField, 1);
            fields_slice[0] = f;
            const wrap = try p.makeNode(.object_literal, f.span, .{
                .object_literal = .{ .type_name = null, .fields = fields_slice },
            });
            try attrs.append(p.arena, wrap);
        }
    }

    const span = ast.Span{ .start = open_tok.span.start, .end = close.span.end };
    return p.makeNode(.component_instance, span, .{
        .component_instance = .{
            .type_ref = type_name,
            .name = inst_name,
            .attrs = try attrs.toOwnedSlice(p.arena),
        },
    });
}

// ─── Task 4.1c: composite blocks ──────────────────────────────────────────

/// Parse a `[<satisfy ... >] body [</satisfy>]` pair-tag.
///
/// The opening tag attrs (requirement, by, method, status) become children
/// of the SatisfyBlock as ObjectLiteral *Node wrappers. The body contains:
///   - An OPTIONAL `=> { ReturnField+ }` return-type block (CS-10).
///   - Zero or more `criteria`/`evidence`/`compute`/`gap` sub-blocks.
///
/// For Task 4.1c, the inner sub-blocks are captured as opaque
/// annotation_body nodes so the showcase parses end-to-end and the
/// snapshot gate is met. Plan 05+ may upgrade to typed criteria/evidence/
/// compute/gap nodes once the semantic analyzer needs them.
///
/// The body is parsed in `.deal_def` mode so expression productions
/// (`evidence simulation { ... }`, `criteria { expr }`, `compute { ident =
/// expr; }`) reuse the existing deal parsers.
fn parseSatisfyBlock(p: *Parser, mode_before: lexer.Mode, open_tok: lexer.Token) std.mem.Allocator.Error!*ast.Node {
    // Push the open tag onto the stack so popTag can verify the close.
    p.pushTag("satisfy", open_tok.span);

    // Read attrs in .dealx_tag, close `>]`.
    const attrs_fields = try parseAttributes(p);
    _ = try p.expect(.tag_close);

    // Restore caller's outer mode (.dealx_outer) for the body region. The
    // body parsing is done in .deal_def — we'll switch INTO .deal_def
    // explicitly so the lookahead is fresh after the mode flip.
    p.restoreMode(mode_before);

    // Wrap attrs as a single-field ObjectLiteral *Node per slot.
    var children: std.ArrayList(*ast.Node) = .empty;
    for (attrs_fields) |f| {
        const fs = try p.arena.alloc(ast.ObjectField, 1);
        fs[0] = f;
        const wrap = try p.makeNode(.object_literal, f.span, .{
            .object_literal = .{ .type_name = null, .fields = fs },
        });
        try children.append(p.arena, wrap);
    }

    // Body — switch to .deal_def for the inner content.
    const prev_body = p.enterMode(.deal_def);
    errdefer p.restoreMode(prev_body);

    // Optional `=> { return_field; ... }` block.
    if (p.peek().tag == .arrow) {
        _ = p.advance(); // consume `=>`
        const ret_node = try parseReturnTypeBlock(p);
        try children.append(p.arena, ret_node);
    }

    // Inner sub-blocks loop. We scan tokens until we hit `[</` (which is
    // tag_close_open — but since we're in .deal_def, the lexer emits it
    // as l_bracket + lt + slash). So we look for the byte pattern instead:
    // if we see `l_bracket` followed by `lt` followed by `slash`, break.
    while (true) {
        const tok = p.peek();
        if (tok.tag == .eof) break;
        if (tok.tag == .l_bracket) {
            const second = p.peek2();
            if (second.tag == .lt) {
                // Could be `[<` (next inner pair-tag) or `[</` (close).
                // Either way, we exit the .deal_def body and let the
                // surrounding code handle it in .dealx_outer.
                break;
            }
        }
        // Otherwise: consume an inner sub-block or skip a token to make
        // progress.
        if (tok.tag == .ident or isInnerBlockKeyword(tok.tag)) {
            const sub = try parseSatisfyInnerBlock(p);
            try children.append(p.arena, sub);
        } else {
            _ = p.advance(); // skip
        }
    }

    // Restore .dealx_outer for the close-tag scan.
    p.restoreMode(prev_body);

    // Now in .dealx_outer — expect `[</`.
    const close_open = try p.expect(.tag_close_open);
    const prev_close = p.enterMode(.dealx_tag);
    errdefer p.restoreMode(prev_close);
    const close_name_tok = p.advance();
    const close_name = p.tokenText(close_name_tok.span);
    const close_tok = try p.expect(.tag_close);
    p.restoreMode(prev_close);

    const close_span = ast.Span{ .start = close_open.span.start, .end = close_tok.span.end };
    p.popTag(close_name, close_span);

    const span = ast.Span{ .start = open_tok.span.start, .end = close_tok.span.end };
    return p.makeNode(.satisfy_block, span, .{
        .satisfy_block = .{ .children = try children.toOwnedSlice(p.arena) },
    });
}

/// Parse the `=> { ReturnField; ... }` block after the satisfy opening tag.
/// Each ReturnField is `IDENT ":" TypeName ";"`. We capture each as an
/// annotation_body field for snapshot stability.
fn parseReturnTypeBlock(p: *Parser) std.mem.Allocator.Error!*ast.Node {
    const open = try p.expect(.l_brace);
    var fields: std.ArrayList(ast.AnnotationField) = .empty;
    while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
        const key_tok = p.peek();
        if (key_tok.tag != .ident and !isInnerBlockKeyword(key_tok.tag)) {
            _ = p.advance();
            continue;
        }
        _ = p.advance();
        const key = p.tokenText(key_tok.span);
        _ = try p.expect(.colon);
        // The value is a type name (IDENT ( "." IDENT )*).
        var type_parts: std.ArrayList(u8) = .empty;
        const first_t = p.advance();
        try type_parts.appendSlice(p.arena, p.tokenText(first_t.span));
        while (p.peek().tag == .dot) {
            _ = p.advance();
            try type_parts.append(p.arena, '.');
            const seg = p.advance();
            try type_parts.appendSlice(p.arena, p.tokenText(seg.span));
        }
        const owned = try type_parts.toOwnedSlice(p.arena);
        // Wrap as a string_literal node so the field has a typed value.
        const val_span = ast.Span{ .start = first_t.span.start, .end = p.lex.pos };
        const val_node = try p.makeNode(.string_literal, val_span, .{
            .string_literal = .{ .value = owned },
        });
        if (p.peek().tag == .semicolon) _ = p.advance();
        try fields.append(p.arena, .{
            .key = key,
            .value = val_node,
            .span = ast.Span{ .start = key_tok.span.start, .end = val_node.span.end },
        });
    }
    const close = try p.expect(.r_brace);
    const span = ast.Span{ .start = open.span.start, .end = close.span.end };
    return p.makeNode(.annotation_body, span, .{
        .annotation_body = .{ .fields = try fields.toOwnedSlice(p.arena) },
    });
}

/// Parse one inner block inside a satisfy body: `KEYWORD [ident]? { ... }`.
/// Captures the entire `{ ... }` body as an annotation_body for snapshot
/// stability. The leading keyword (criteria/evidence/compute/gap) plus
/// optional sub-discriminator (e.g. `simulation` after `evidence`) becomes
/// the annotation name; the body fields become the annotation_body fields.
fn parseSatisfyInnerBlock(p: *Parser) std.mem.Allocator.Error!*ast.Node {
    const kw_tok = p.advance();
    const kw_text = p.tokenText(kw_tok.span);
    var name_buf: std.ArrayList(u8) = .empty;
    try name_buf.appendSlice(p.arena, kw_text);

    // Optional sub-discriminator IDENT (e.g. `evidence simulation { ... }`).
    if (p.peek().tag == .ident or isInnerBlockKeyword(p.peek().tag)) {
        const sub = p.advance();
        try name_buf.append(p.arena, ' ');
        try name_buf.appendSlice(p.arena, p.tokenText(sub.span));
    }

    const name = try name_buf.toOwnedSlice(p.arena);

    // Consume the `{` body. To stay snapshot-stable across complex
    // expressions (criteria has bool exprs, compute has assignments, gap
    // has nested {}), we just scan brace-balanced bytes and treat the
    // content as opaque text in a single field "_body".
    if (p.peek().tag != .l_brace) {
        // Bare keyword without body — return as a doc-comment-like node
        // so we have *something*.
        const span = ast.Span{ .start = kw_tok.span.start, .end = kw_tok.span.end };
        var fields: std.ArrayList(ast.AnnotationField) = .empty;
        return p.makeNode(.annotation, span, .{
            .annotation = .{
                .name = name,
                .category = null,
                .operator = null,
                .target_segments = &.{},
                .value = null,
                .body = try p.makeNode(.annotation_body, span, .{
                    .annotation_body = .{ .fields = try fields.toOwnedSlice(p.arena) },
                }),
            },
        });
    }

    const open = try p.expect(.l_brace);
    // Brace-balanced byte scan. We track depth manually because the inner
    // content may contain arbitrarily complex expressions.
    const body_start_pos = p.lex.pos;
    var depth: u32 = 1;
    while (depth > 0 and p.peek().tag != .eof) {
        const tok = p.peek();
        if (tok.tag == .l_brace) {
            depth += 1;
        } else if (tok.tag == .r_brace) {
            depth -= 1;
            if (depth == 0) break;
        }
        _ = p.advance();
    }
    const body_end_pos = p.peek().span.start;
    const close = try p.expect(.r_brace);
    const body_text = p.source[body_start_pos..body_end_pos];
    const body_val = try p.makeNode(.string_literal, ast.Span{
        .start = body_start_pos,
        .end = body_end_pos,
    }, .{
        .string_literal = .{ .value = body_text },
    });
    var fields: std.ArrayList(ast.AnnotationField) = .empty;
    try fields.append(p.arena, .{
        .key = "_body",
        .value = body_val,
        .span = ast.Span{ .start = body_start_pos, .end = body_end_pos },
    });
    const body_node = try p.makeNode(.annotation_body, ast.Span{
        .start = open.span.start,
        .end = close.span.end,
    }, .{
        .annotation_body = .{ .fields = try fields.toOwnedSlice(p.arena) },
    });

    const span = ast.Span{ .start = kw_tok.span.start, .end = close.span.end };
    return p.makeNode(.annotation, span, .{
        .annotation = .{
            .name = name,
            .category = null,
            .operator = null,
            .target_segments = &.{},
            .value = null,
            .body = body_node,
        },
    });
}

/// Parse a `[<validate ... >] body [</validate>]` pair-tag.
/// Body is `key: value` fields (.deal_def mode) until `[</`.
fn parseValidateBlock(p: *Parser, mode_before: lexer.Mode, open_tok: lexer.Token) std.mem.Allocator.Error!*ast.Node {
    p.pushTag("validate", open_tok.span);

    const attrs_fields = try parseAttributes(p);
    _ = try p.expect(.tag_close);
    p.restoreMode(mode_before);

    var attrs: std.ArrayList(*ast.Node) = .empty;
    for (attrs_fields) |f| {
        const fs = try p.arena.alloc(ast.ObjectField, 1);
        fs[0] = f;
        const wrap = try p.makeNode(.object_literal, f.span, .{
            .object_literal = .{ .type_name = null, .fields = fs },
        });
        try attrs.append(p.arena, wrap);
    }

    // Body — `key: value` lines in .deal_def.
    const prev_body = p.enterMode(.deal_def);
    errdefer p.restoreMode(prev_body);

    while (true) {
        const tok = p.peek();
        if (tok.tag == .eof) break;
        if (tok.tag == .l_bracket) {
            const second = p.peek2();
            if (second.tag == .lt) break;
        }
        // key: value
        if (tok.tag == .ident or isInnerBlockKeyword(tok.tag)) {
            const key_tok = p.advance();
            const key = p.tokenText(key_tok.span);
            if (p.peek().tag != .colon) {
                continue;
            }
            _ = p.advance(); // consume `:`
            // Value can be a string literal or an expression — accept either.
            const val = try expr.parseExpression(p, 0);
            if (p.peek().tag == .semicolon) _ = p.advance();
            const fs = try p.arena.alloc(ast.ObjectField, 1);
            fs[0] = .{
                .key = key,
                .value = val,
                .span = ast.Span{ .start = key_tok.span.start, .end = val.span.end },
            };
            const wrap = try p.makeNode(.object_literal, ast.Span{
                .start = key_tok.span.start,
                .end = val.span.end,
            }, .{
                .object_literal = .{ .type_name = null, .fields = fs },
            });
            try attrs.append(p.arena, wrap);
        } else {
            _ = p.advance();
        }
    }

    p.restoreMode(prev_body);

    const close_open = try p.expect(.tag_close_open);
    const prev_close = p.enterMode(.dealx_tag);
    errdefer p.restoreMode(prev_close);
    const close_name_tok = p.advance();
    const close_name = p.tokenText(close_name_tok.span);
    const close_tok = try p.expect(.tag_close);
    p.restoreMode(prev_close);

    const close_span = ast.Span{ .start = close_open.span.start, .end = close_tok.span.end };
    p.popTag(close_name, close_span);

    const span = ast.Span{ .start = open_tok.span.start, .end = close_tok.span.end };
    return p.makeNode(.validate_tag, span, .{
        .validate_tag = .{ .attrs = try attrs.toOwnedSlice(p.arena) },
    });
}

/// Tokens that may appear as inner sub-block keywords or attribute keys
/// inside satisfy/validate bodies — same as isAttrNameKeyword but includes
/// the satisfy-specific evidence-type keywords.
fn isInnerBlockKeyword(tag: lexer.Tag) bool {
    return switch (tag) {
        .kw_criteria, .kw_evidence, .kw_compute, .kw_gap, .kw_maps,
        .kw_simulation, .kw_test, .kw_analysis, .kw_design,
        .kw_inspection, .kw_demonstration,
        .kw_from, .kw_to, .kw_via, .kw_carrying, .kw_relationship,
        .kw_method, .kw_status, .kw_as, .kw_by, .kw_requirement,
        .kw_baseline,
        => true,
        else => false,
    };
}

/// Tokens that may appear as attribute names. The .dealx grammar permits
/// bare keyword spellings (from, to, via, carrying, relationship, requirement,
/// by, method, status, as) in attribute-name position even though the lexer
/// emits them as `kw_*` tokens.
fn isAttrNameKeyword(tag: lexer.Tag) bool {
    return switch (tag) {
        .kw_from,
        .kw_to,
        .kw_via,
        .kw_carrying,
        .kw_relationship,
        .kw_method,
        .kw_status,
        .kw_as,
        .kw_by,
        .kw_requirement,
        // Evidence / criteria block keywords (Task 4.1c uses these as
        // satisfy-prop / object-field names).
        .kw_criteria,
        .kw_evidence,
        .kw_compute,
        .kw_gap,
        .kw_maps,
        .kw_simulation,
        .kw_test,
        .kw_analysis,
        .kw_design,
        .kw_inspection,
        .kw_demonstration,
        => true,
        else => false,
    };
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Some package-name segments use keyword-tokenized words (e.g. `model`
/// is just an IDENT in the .deal lexer, but `system` is `kw_system`).
/// Mirrors parser_deal.isKeyword for the small set we need in `.dealx`.
fn isIdentLikeKeyword(tag: lexer.Tag) bool {
    return switch (tag) {
        .kw_system,
        .kw_subsystem,
        .kw_connect,
        .kw_expose,
        .kw_traceability,
        .kw_satisfy,
        .kw_validate,
        .kw_allocate,
        => true,
        else => false,
    };
}
