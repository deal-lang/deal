//! `.deal` (definitions) parser — Plan 03 full implementation.
//!
//! Implements the 87 `deal.ebnf` productions in a single recursive-descent
//! pass. The Parser struct carries:
//!   - `arena` allocator (all *Node allocated here per D-02)
//!   - `lexer` instance
//!   - `peeked` single-token lookahead cache
//!   - `diagnostics` list (appended on parse errors)
//!   - `current_mode` (always .deal_def in Plan 03; Plan 04 mutates via
//!     enterMode/restoreMode for .dealx contexts)
//!   - `source` slice for text recovery from spans
//!
//! Plan 04 stubs (enterMode/restoreMode/pushTag/popTag) ship as no-op bodies
//! so parser_dealx.zig can compile against a stable Parser surface.
//!
//! Error recovery: on mismatch `expect` appends a diagnostic and returns the
//! mismatched token, letting the caller continue (minimal recovery, Plan 05
//! upgrades to full sync-to-boundary recovery per D-17).

const std = @import("std");
const ast = @import("ast");
const lexer = @import("lexer");
const diagnostics = @import("diagnostics");
const expr = @import("expr");

// ─── Parser struct ────────────────────────────────────────────────────────

pub const Parser = struct {
    /// Points at handle.arena.allocator() — used to allocate every *Node.
    arena: std.mem.Allocator,
    lex: lexer.Lexer,
    /// Single-token lookahead cache (RESEARCH Open Question #1, RESOLVED).
    peeked: ?lexer.Token = null,
    diags: *std.ArrayList(diagnostics.Diagnostic),
    /// Source bytes — needed to recover token text from spans.
    source: []const u8,
    /// Lexer mode. Always .deal_def in Plan 03; Plan 04 mutates this.
    current_mode: lexer.Mode = .deal_def,
    /// Open composition-tag stack (D-07). Empty for .deal files; populated
    /// by parser_dealx as `[<name>]` opens and consumed as `[</name>]` closes.
    /// The OpenTag type lives in ast.zig so both parser_deal and parser_dealx
    /// can share it without a circular module import (parser_dealx already
    /// imports parser_deal; the reverse direction would form a cycle).
    open_tags: std.ArrayList(ast.OpenTag) = .empty,
    /// Current recursion depth across parseExpression / parseDefinition / inline
    /// object literals (T-05-01 mitigation). Bumped on entry to every
    /// recursive grammar function; once `depth > MAX_PARSE_DEPTH` the
    /// recursive function emits E0403 and returns a sentinel empty
    /// Identifier node so the caller's loop can synchronize.
    depth: u16 = 0,

    /// D-28 comment attachment buffer (Plan 02-01). comment_line and
    /// comment_block tokens are silently buffered here as peek()/advance()
    /// encounter them. attachComments() drains this into a declaration's
    /// leading_comments / trailing_comments / doc_comment fields using the
    /// gofmt blank-line rule (RESEARCH §Pattern 2).
    pending_comments: std.ArrayList(ast.Comment) = .empty,

    /// T-05-01 DoS bound — defends against stack overflow from
    /// adversarially-nested expression / definition input (e.g. 10k nested
    /// `{ { { ... } } }`). Showcase nests at most ~5 levels; 256 is generous
    /// without consuming meaningful stack on any consumer platform.
    pub const MAX_PARSE_DEPTH: u16 = 256;

    // ── Peek / advance / expect ──────────────────────────────────────

    /// Advance the raw lexer and buffer any comment_line / comment_block tokens
    /// encountered. Returns the first non-comment token. doc_comment tokens are
    /// NOT buffered here — they are returned as-is so the existing
    /// doc_comment handling in parseDefinition / parseUsageMember continues
    /// to work (doc_comment tokens are converted to NodeKind.doc_comment nodes
    /// or promoted to doc_comment fields by attachComments).
    fn nextNonComment(self: *Parser) !lexer.Token {
        while (true) {
            const tok = self.lex.next(self.current_mode);
            switch (tok.tag) {
                .comment_line => {
                    const text = self.source[tok.span.start..tok.span.end];
                    try self.pending_comments.append(self.arena, .{
                        .kind = .line,
                        .text = text,
                        .span = tok.span,
                    });
                    continue;
                },
                .comment_block => {
                    const text = self.source[tok.span.start..tok.span.end];
                    try self.pending_comments.append(self.arena, .{
                        .kind = .block,
                        .text = text,
                        .span = tok.span,
                    });
                    continue;
                },
                else => return tok,
            }
        }
    }

    pub fn peek(self: *Parser) lexer.Token {
        if (self.peeked == null) {
            self.peeked = self.nextNonComment() catch self.lex.next(self.current_mode);
        }
        return self.peeked.?;
    }

    pub fn advance(self: *Parser) lexer.Token {
        if (self.peeked) |tok| {
            self.peeked = null;
            return tok;
        }
        return self.nextNonComment() catch self.lex.next(self.current_mode);
    }

    /// Consume the next token and assert it has tag `expected`. On mismatch,
    /// append a diagnostic and return the actual token (minimal recovery per
    /// Plan 03 contract — Plan 05 converts this to full sync recovery).
    pub fn expect(self: *Parser, expected: lexer.Tag) !lexer.Token {
        const tok = self.advance();
        if (tok.tag != expected) {
            try self.diags.append(self.arena, .{
                .code = "E0100",
                .severity = .err,
                .message = "unexpected token",
                .span = tok.span,
            });
        }
        return tok;
    }

    /// Peek two tokens ahead — needed for LL(2) decisions (e.g. "." + "{" in
    /// import tail). Restores lexer state after the two peeks. Skips comment
    /// tokens in both positions (consistent with peek() / advance()).
    ///
    /// Implementation note: `peek()` calls `lex.next()` internally and caches
    /// the result in `self.peeked`. After a call to `peek()`, `lex.pos` is
    /// already PAST the first upcoming token. So to get the second token we
    /// only need additional `lex.next()` calls — skipping any comments.
    pub fn peek2(self: *Parser) lexer.Token {
        // Ensure the first token is peeked (advances lex.pos past it).
        _ = self.peek();
        // Save lex state (lex.pos is already past the first token).
        const saved_pos = self.lex.pos;
        const saved_depth = self.lex.template_depth;
        const saved_stack = self.lex.template_stack;
        // Get the second non-comment token. We cannot call nextNonComment()
        // here because it has side effects on pending_comments, so we skip
        // comment tokens directly.
        var second: lexer.Token = undefined;
        while (true) {
            second = self.lex.next(self.current_mode);
            if (second.tag != .comment_line and second.tag != .comment_block) break;
        }
        // Restore lex position (self.peeked is unchanged — still the first).
        self.lex.pos = saved_pos;
        self.lex.template_depth = saved_depth;
        self.lex.template_stack = saved_stack;
        return second;
    }

    /// Allocate a Node in the arena.
    pub fn makeNode(self: *Parser, kind: ast.NodeKind, span: ast.Span, payload: ast.Payload) !*ast.Node {
        const n = try self.arena.create(ast.Node);
        n.* = .{ .kind = kind, .span = span, .payload = payload };
        return n;
    }

    /// Recover the source text for a span.
    pub fn tokenText(self: *const Parser, span: ast.Span) []const u8 {
        return self.source[span.start..span.end];
    }

    // ── Comment attachment (D-28, Plan 02-01) ────────────────────────

    /// Apply gofmt-style blank-line rule to the comments in `pending_comments`
    /// and attach them to the declaration described by `decl_span`.
    ///
    /// Gofmt rule (RESEARCH §Pattern 2, lines 401-430 verbatim):
    ///   - Comments with no blank line between them and this declaration →
    ///     `leading_comments`.
    ///   - Comments separated by a blank line from this declaration AND
    ///     adjacent to the previous declaration → `trailing_comments` on the
    ///     PREVIOUS declaration (not this one). Since this function runs at
    ///     the start of a new declaration parse, "previous declaration" means
    ///     the comments should have been attached as trailing on the prior
    ///     decl. For simplicity in Plan 02-01, we return them as "floating"
    ///     (not attached to anything) — Plan 02-05 (fmt) will handle the
    ///     full trailing-on-previous logic. For now, blanked-off comments
    ///     are dropped from attachment (safe for IR/sema which ignores cmts).
    ///   - Same-line trailing comments: these are buffered AFTER a declaration
    ///     is parsed (not before). Use `attachTrailingComment` for that.
    ///
    /// Returns arena-owned slices of Comment. The caller is responsible for
    /// setting the slices on the declaration payload.
    pub fn attachLeadingComments(
        self: *Parser,
        decl_span_start: u32,
    ) !struct { leading: []ast.Comment, doc: ?*ast.DocComment } {
        // Find the boundary: comments separated from decl_span_start by a
        // blank line are NOT attached as leading. Walk backwards from the
        // declaration's start byte through the source to count blank lines
        // between the last comment's end and the declaration's start.
        //
        // Implementation: iterate pending_comments from the end to find the
        // contiguous group that has NO blank line between them and the decl.
        const comments = self.pending_comments.items;
        if (comments.len == 0) {
            self.pending_comments.clearRetainingCapacity();
            return .{ .leading = &.{}, .doc = null };
        }

        // Find the split point: walk backwards through the comment list looking
        // for the first blank line between consecutive comments (or between the
        // last comment and the declaration). The contiguous group with NO blank
        // line immediately before the declaration is the attach group.
        //
        // attach_from starts at 0 (ALL comments attach). When a blank line is
        // found between comments[i] and comments[i+1] (or between comments[i]
        // and the declaration), set attach_from = i + 1 and stop — comments
        // before that index are "floating" and are dropped in Plan 02-01.
        var attach_from: usize = 0; // by default, all comments attach
        {
            var i: usize = comments.len;
            while (i > 0) {
                i -= 1;
                const cmt = comments[i];
                // Check for blank line between cmt.span.end and the next
                // token's start (either the next comment or decl_span_start).
                const gap_end: u32 = if (i + 1 < comments.len) comments[i + 1].span.start else decl_span_start;
                // comment_line tokens include the trailing \n in their span;
                // comment_block tokens do not — so the threshold differs.
                const consumed_nl = cmt.kind == .line;
                if (hasBlankLineBetween(self.source, cmt.span.end, gap_end, consumed_nl)) {
                    // Blank line found: this comment and everything before it
                    // does NOT attach as leading.
                    attach_from = i + 1;
                    break;
                }
            }
        }

        // Split: comments[0..attach_from] are NOT attached (floating/trailing
        // on previous decl — dropped in Plan 02-01). comments[attach_from..]
        // are leading comments on this decl.
        const leading_raw = comments[attach_from..];

        // Separate doc_comment tokens from leading_comments. The last
        // comment in leading_raw that is a doc_comment AND immediately
        // precedes the declaration (no blank line) is promoted to `doc_comment`.
        // All others go into leading_comments.
        var leading = std.ArrayList(ast.Comment).empty;
        const doc: ?*ast.DocComment = null;

        for (leading_raw) |cmt| {
            // In Plan 02-01 we only buffer comment_line and comment_block
            // (doc_comments are handled separately via the existing token path).
            // So all pending_comments here are line or block kind.
            try leading.append(self.arena, cmt);
        }

        self.pending_comments.clearRetainingCapacity();
        return .{ .leading = try leading.toOwnedSlice(self.arena), .doc = doc };
    }

    /// Returns true if there is a blank line in the gap from `start` to `end`.
    ///
    /// The `consumed_trailing_newline` parameter distinguishes two cases:
    ///   - comment_line tokens include the trailing `\n` in their span.
    ///     `consumed_trailing_newline = true` → a blank line is ONE `\n` in gap.
    ///   - comment_block tokens do NOT include trailing whitespace.
    ///     `consumed_trailing_newline = false` → a blank line is TWO `\n` in gap.
    ///
    /// Examples:
    ///   "// comment\n\npart" — line comment, gap = "\n" (1 newline) → true
    ///   "/* comment */\n\npart" — block comment, gap = "\n\n" (2 newlines) → true
    ///   "/* comment */\npart" — block comment, gap = "\n" (1 newline) → false
    fn hasBlankLineBetween(source: []const u8, start: u32, end: u32, consumed_trailing_newline: bool) bool {
        if (start >= end or end > source.len) return false;
        const gap = source[start..end];
        const threshold: u32 = if (consumed_trailing_newline) 1 else 2;
        var newline_count: u32 = 0;
        for (gap) |c| {
            if (c == '\n') {
                newline_count += 1;
                if (newline_count >= threshold) return true;
            } else if (c != ' ' and c != '\t' and c != '\r') {
                // Non-whitespace resets the blank-line count.
                newline_count = 0;
            }
        }
        return false;
    }

    // ── Mode-switching stubs (Plan 04 fills bodies) ──────────────────

    /// Switch to a new lexer mode. Returns the previous mode for restoreMode.
    /// CRITICAL (Pitfall 7): if a token has been peeked in the OLD mode, we
    /// MUST rewind `lex.pos` to that token's start before invalidating the
    /// cache — otherwise the bytes the cached token consumed under the old
    /// mode are silently skipped under the new mode. The peeked token's
    /// span.start is the byte offset where the lexer first looked at it.
    pub fn enterMode(self: *Parser, m: lexer.Mode) lexer.Mode {
        const prev = self.current_mode;
        if (self.peeked) |tok| {
            self.lex.pos = tok.span.start;
        }
        self.peeked = null;
        self.current_mode = m;
        return prev;
    }

    /// Restore a previously-saved mode. Same rewind discipline as enterMode:
    /// any token peeked under the about-to-be-discarded mode must NOT be
    /// silently swallowed under the restored mode.
    pub fn restoreMode(self: *Parser, prev: lexer.Mode) void {
        if (self.peeked) |tok| {
            self.lex.pos = tok.span.start;
        }
        self.peeked = null;
        self.current_mode = prev;
    }

    // Tag-stack methods (Plan 04 — D-07 parser-owned open-tag stack).
    //
    // `pushTag` appends an open tag at the current depth. If the stack is
    // already at MAX_TAG_DEPTH (T-04-01 DoS bound) the new tag is silently
    // dropped and E0303 is emitted at `span`. Subsequent matching `[</>]`
    // closes will hit popTag with an unmatched-close diagnostic — but that
    // is acceptable degradation; the parser cannot continue tracking depth
    // past the bound without risking stack exhaustion.
    //
    // `popTag` matches a close against the top of the stack. Per D-07
    // ("pop anyway to continue"), the pop happens BEFORE the name check so
    // we always advance the stack; if names mismatch we emit E0302 with
    // BOTH spans (primary = close, secondary = the original open).
    // If the stack is empty we emit E0301 (unmatched composition close).
    pub fn pushTag(self: *Parser, name: []const u8, span: ast.Span) void {
        if (self.open_tags.items.len >= ast.MAX_TAG_DEPTH) {
            self.diags.append(self.arena, .{
                .code = "E0303",
                .severity = .err,
                .message = "composition tag nesting too deep",
                .span = span,
            }) catch {};
            return;
        }
        self.open_tags.append(self.arena, .{ .name = name, .span = span }) catch {
            // OOM appending: emit a diagnostic but don't propagate (this
            // method is `void`); the next allocation will likely also fail
            // and surface OOM through a parse function's error return.
            self.diags.append(self.arena, .{
                .code = "E0303",
                .severity = .err,
                .message = "out of memory recording open composition tag",
                .span = span,
            }) catch {};
        };
    }

    pub fn popTag(self: *Parser, expected_name: []const u8, close_span: ast.Span) void {
        if (self.open_tags.items.len == 0) {
            self.diags.append(self.arena, .{
                .code = "E0301",
                .severity = .err,
                .message = "unmatched composition close tag",
                .span = close_span,
            }) catch {};
            return;
        }
        // D-07: pop anyway, even on mismatch, so parsing continues.
        const top = self.open_tags.pop().?;
        if (!std.mem.eql(u8, top.name, expected_name)) {
            const secondary = self.arena.alloc(diagnostics.SpanLabel, 1) catch {
                // Fall back to no-secondary diagnostic on OOM.
                self.diags.append(self.arena, .{
                    .code = "E0302",
                    .severity = .err,
                    .message = "mismatched composition close tag",
                    .span = close_span,
                }) catch {};
                return;
            };
            secondary[0] = .{ .span = top.span, .label = "opened here" };
            self.diags.append(self.arena, .{
                .code = "E0302",
                .severity = .err,
                .message = "mismatched composition close tag",
                .span = close_span,
                .secondary_spans = secondary,
            }) catch {};
        }
    }
};

// ─── D-17 three-tier synchronization ──────────────────────────────────────
//
// Three sync helpers (one per recovery tier — RESEARCH §Pattern 5 lines
// 428-446). Each skips forward in the token stream until it hits a token in
// its FOLLOW set OR EOF. The sync token is LEFT AT PEEK — not consumed —
// so the calling production can decide what to do with it.
//
// Each helper emits AT MOST ONE E0400 info-severity diagnostic per call
// (covering the entire skipped range, not per token — T-05-05 mitigation).

const SyncSet = std.EnumSet(lexer.Tag);

/// Statement-level FOLLOW set (D-17). Tokens that terminate a statement
/// or expression context.
const STATEMENT_SYNC: SyncSet = blk: {
    var s = SyncSet.initEmpty();
    s.insert(.semicolon);
    s.insert(.comma);
    s.insert(.r_brace);
    s.insert(.r_paren);
    s.insert(.r_bracket);
    break :blk s;
};

/// Definition-level FOLLOW set (D-17). The 14 definition keywords plus
/// `r_brace` (end of enclosing body) and `eof`.
const DEFINITION_SYNC: SyncSet = blk: {
    var s = SyncSet.initEmpty();
    s.insert(.kw_part);
    s.insert(.kw_port);
    s.insert(.kw_action);
    s.insert(.kw_state);
    s.insert(.kw_attribute);
    s.insert(.kw_item);
    s.insert(.kw_interface);
    s.insert(.kw_connection);
    s.insert(.kw_flow);
    s.insert(.kw_allocation);
    s.insert(.kw_requirement);
    s.insert(.kw_constraint);
    s.insert(.kw_need);
    s.insert(.kw_use);
    s.insert(.kw_export);
    s.insert(.kw_import);
    s.insert(.kw_package);
    s.insert(.r_brace);
    break :blk s;
};

/// Skip tokens until we hit a STATEMENT_SYNC token or EOF. Emits one E0400
/// covering the skipped range if any tokens were dropped.
pub fn syncToStatement(self: *Parser) void {
    const start = self.peek().span.start;
    var dropped: u32 = 0;
    while (true) {
        const tok = self.peek();
        if (tok.tag == .eof) break;
        if (STATEMENT_SYNC.contains(tok.tag)) break;
        _ = self.advance();
        dropped += 1;
    }
    if (dropped > 0) {
        const end = self.peek().span.start;
        self.diags.append(self.arena, .{
            .code = "E0400",
            .severity = .info,
            .message = "skipped tokens during error recovery",
            .span = .{ .start = start, .end = end },
        }) catch {};
    }
}

// ─── Phase 1.5 plan 02 (m08): drain lexer-surfaced trivia errors ─────────
//
// The Lexer cannot emit diagnostics directly (no collector at that layer);
// instead, `skipTrivia` records an unterminated-block-comment span on the
// Lexer struct (lex.unterminated_block_comment). The parser drains this
// state after the main parse loop and emits the EXISTING D-16 lexer code
// E0003 (`Codes.e_unterminated_comment`) — declared in Phase 1 but never
// wired until Phase 1.5.
//
// Drain semantics: we clear the field after emitting so a hypothetical
// repeated drain (multi-pass tooling) does not double-fire.
//
// Phase 1.5 chose to drain at parseFile-end (rather than at every peek/
// advance) because the m08 fixture shape has the unterminated `/*` as the
// LAST trivia before EOF — the parser's main loop terminates on EOF and
// THEN we drain. If a future strictness gap reveals shapes where the
// unterminated comment is mid-file with surviving trailing tokens, this
// can be promoted to a per-token-call drain.
pub fn drainLexerErrors(p: *Parser) !void {
    if (p.lex.unterminated_block_comment) |span| {
        try p.diags.append(p.arena, .{
            .code = diagnostics.Codes.e_unterminated_comment,
            .severity = .err,
            .message = "unterminated `/* */` block comment",
            .span = span,
        });
        p.lex.unterminated_block_comment = null;
    }
}

/// Skip tokens until we hit a DEFINITION_SYNC token or EOF. Emits one E0400
/// covering the skipped range if any tokens were dropped.
pub fn syncToDefinition(self: *Parser) void {
    const start = self.peek().span.start;
    var dropped: u32 = 0;
    while (true) {
        const tok = self.peek();
        if (tok.tag == .eof) break;
        if (DEFINITION_SYNC.contains(tok.tag)) break;
        _ = self.advance();
        dropped += 1;
    }
    if (dropped > 0) {
        const end = self.peek().span.start;
        self.diags.append(self.arena, .{
            .code = "E0400",
            .severity = .info,
            .message = "skipped tokens during error recovery",
            .span = .{ .start = start, .end = end },
        }) catch {};
    }
}

// ─── Public entry point ────────────────────────────────────────────────────

/// Parse a .deal file from the given source buffer. Returns the root Node or
/// null on OOM (a fatal diagnostic E0004 is appended in that case). Other
/// parse errors produce partial ASTs with diagnostics attached.
///
/// Called by parser.zig / lib.zig — takes explicit fields to avoid a circular
/// import (lib.zig → parser.zig → parser_deal.zig would create a cycle if
/// parser_deal.zig imported lib.zig).
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
    return parseDealFile(&p) catch |err| {
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

// ─── §1: File structure ────────────────────────────────────────────────────

fn parseDealFile(p: *Parser) !*ast.Node {
    const start: u32 = 0;

    // Consume leading doc-comment or header block.
    const header = try parseOptionalHeaderBlock(p);
    const pkg = try parsePackageDecl(p);

    // Parse import declarations (appear before definitions in grammar).
    var imports: std.ArrayList(*ast.Node) = .empty;
    while (p.peek().tag == .kw_import) {
        const imp = try parseImportDecl(p);
        try imports.append(p.arena, imp);
    }

    var exports_list: std.ArrayList(*ast.Node) = .empty;
    var defs: std.ArrayList(*ast.Node) = .empty;

    while (p.peek().tag != .eof) {
        const tok = p.peek();
        const tok_start = tok.span.start;
        if (tok.tag == .kw_export) {
            const ex = try parseExportDecl(p);
            try exports_list.append(p.arena, ex);
        } else if (tok.tag == .kw_import) {
            // Stray imports after the leading import block — still parse.
            const imp = try parseImportDecl(p);
            try imports.append(p.arena, imp);
        } else if (canStartDefinition(tok.tag)) {
            const def = try parseDefinition(p);
            try defs.append(p.arena, def);
        } else {
            // Unknown token at top level — sync to next definition keyword.
            // D-17 definition-tier recovery. We emit a single E0101 at the
            // bad token and then syncToDefinition picks up the rest.
            try p.diags.append(p.arena, .{
                .code = "E0101",
                .severity = .err,
                .message = "expected definition or import declaration",
                .span = tok.span,
            });
            syncToDefinition(p);
            // Guarantee forward progress — if sync didn't advance past
            // tok_start (e.g. tok was already in DEFINITION_SYNC like
            // r_brace), advance one token so we don't loop.
            if (p.peek().span.start == tok_start) _ = p.advance();
        }
    }

    // Phase 1.5 plan 02 (m08): drain any lexer-surfaced trivia errors
    // recorded during the main parse loop (currently: unterminated `/* */`
    // block comment → Codes.e_unterminated_comment E0003).
    try drainLexerErrors(p);

    const end = p.peek().span.start;
    const span = ast.Span{ .start = start, .end = end };

    return p.makeNode(.deal_file, span, .{
        .deal_file = .{
            .header = header,
            .package_decl = pkg,
            .imports = try imports.toOwnedSlice(p.arena),
            .exports = try exports_list.toOwnedSlice(p.arena),
            .definitions = try defs.toOwnedSlice(p.arena),
        },
    });
}

fn parseOptionalHeaderBlock(p: *Parser) !?*ast.Node {
    const tok = p.peek();
    // @header — lexer emits the `@` prefix as annotation token
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
    const key_tok = p.advance(); // path, schema, created, etc.
    const key_text = p.tokenText(key_tok.span);
    // WR-03: capture the colon token's span directly rather than rescanning
    // p.source for the ':' byte. The lexer already produced this span; the
    // previous code threw it away then walked the source bytes to recover it,
    // which is fragile (any future grammar change that lets keys contain a
    // colon-like byte would misalign the rescan).
    const colon_tok = try p.expect(.colon);

    // Consume everything until newline or closing brace as raw value text.
    // The grammar says values are "free-form text up to newline" (FS-2).
    var value_buf: std.ArrayList(u8) = .empty;

    // Since the lexer skips whitespace, we read tokens until we hit a
    // token that starts on a new line. We detect this by comparing byte
    // positions vs the source.
    var end_pos = key_tok.span.end;

    // Collect raw value up to newline. Clamp every cursor to source.len so
    // truncated headers (Plan 05 malformed corpus) don't panic.
    var i: u32 = colon_tok.span.end;
    if (i > p.source.len) i = @intCast(p.source.len);
    // Skip leading spaces.
    while (i < p.source.len and (p.source[i] == ' ' or p.source[i] == '\t')) i += 1;
    const val_start = i;
    while (i < p.source.len and p.source[i] != '\n' and p.source[i] != '\r') i += 1;
    // Trim trailing spaces.
    var val_end = i;
    while (val_end > val_start and (p.source[val_end - 1] == ' ' or p.source[val_end - 1] == '\t')) val_end -= 1;

    const value_text = if (val_start <= val_end and val_end <= p.source.len)
        p.source[val_start..val_end]
    else
        "";
    try value_buf.appendSlice(p.arena, value_text);
    end_pos = @intCast(i);

    // Skip past the newline in the lexer by peeking past it.
    // The lexer already skips whitespace including newlines, so any
    // subsequent peek()/advance() will start after the newline.
    // We just need to advance past any tokens that happen to be on
    // the same "line" as the value. Since header values are free-form
    // text tokens, and the lexer tokenizes them as separate tokens,
    // we scan forward until we hit a token on the NEXT line.
    //
    // Implementation: consume tokens until the next token's start position
    // is >= end_pos (the newline position). IN-07: `i` is already u32 so the
    // previous `@as(u32, @intCast(i))` was dead casting noise.
    while (true) {
        const next = p.peek();
        if (next.tag == .r_brace or next.tag == .eof) break;
        if (next.span.start >= i) break;
        _ = p.advance();
    }

    const span = ast.Span{ .start = key_tok.span.start, .end = end_pos };
    return ast.HeaderField{
        .key = key_text,
        .value = try value_buf.toOwnedSlice(p.arena),
        .span = span,
    };
}

fn parsePackageDecl(p: *Parser) !?*ast.Node {
    if (p.peek().tag != .kw_package) return null;
    const start = p.advance(); // consume "package"
    const segments = try parseQualifiedNameSegments(p);
    const semi = try p.expect(.semicolon);
    const span = ast.Span{ .start = start.span.start, .end = semi.span.end };
    return p.makeNode(.package_decl, span, .{
        .package_decl = .{ .segments = segments },
    });
}

fn parseImportDecl(p: *Parser) !*ast.Node {
    const start = p.advance(); // consume "import"

    // Check for relative prefix
    var segments: std.ArrayList([]const u8) = .empty;
    const tok = p.peek();

    // Handle relative prefix "." or ".."
    if (tok.tag == .dot) {
        _ = p.advance(); // consume "."
        const next = p.peek();
        if (next.tag == .dot) {
            _ = p.advance(); // consume second "."
            try segments.append(p.arena, "..");
        } else {
            try segments.append(p.arena, ".");
        }
    }

    // Parse the main path: IDENT ("." IDENT)*
    // Keyword tokens are also valid as path segments (e.g. "analysis" is
    // kw_analysis but is used as a package name).
    const first = p.peek();
    if (first.tag == .ident or isKeyword(first.tag)) {
        _ = p.advance();
        try segments.append(p.arena, p.tokenText(first.span));
    } else {
        try p.diags.append(p.arena, .{
            .code = "E0100",
            .severity = .err,
            .message = "unexpected token",
            .span = first.span,
        });
    }

    while (p.peek().tag == .dot) {
        // LL(2): peek 2 to see if next after "." is IDENT or keyword
        // (path continue) vs "{" (destructured import) vs "*" (glob).
        const second = p.peek2();
        if (second.tag == .ident or isKeyword(second.tag)) {
            _ = p.advance(); // consume "."
            const seg_tok = p.advance();
            try segments.append(p.arena, p.tokenText(seg_tok.span));
        } else {
            break; // "." is start of import tail
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
            // _NameList with optional "as" aliasing
            while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
                const name_tok = try p.expect(.ident);
                const name = p.tokenText(name_tok.span);
                var alias: ?[]const u8 = null;
                if (p.peek().tag == .kw_as) {
                    _ = p.advance(); // consume "as"
                    const alias_tok = try p.expect(.ident);
                    alias = p.tokenText(alias_tok.span);
                }
                try items.append(p.arena, .{ .name = name, .alias = alias });
                if (p.peek().tag == .comma) _ = p.advance();
            }
            _ = try p.expect(.r_brace);
        } else if (after_dot.tag == .star) {
            _ = p.advance(); // consume "*"
            kind = .wildcard;
        }
    } else if (next.tag == .kw_as) {
        _ = p.advance(); // consume "as"
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

fn parseExportDecl(p: *Parser) !*ast.Node {
    const start = p.advance(); // consume "export"
    // Module name may be an ident or a keyword used as a name (e.g. "system").
    const mod_tok = p.advance();
    if (mod_tok.tag != .ident and !isKeyword(mod_tok.tag)) {
        try p.diags.append(p.arena, .{
            .code = "E0100",
            .severity = .err,
            .message = "expected module name",
            .span = mod_tok.span,
        });
        // WR-06: do NOT leak the offending token's text (e.g. ";" or "}") into
        // the AST as a module name — downstream semantic passes have no way to
        // distinguish that from a legitimate keyword-named module. Sync to the
        // next definition boundary and produce a stub export_decl with an
        // empty module + no items so the AST shape is still valid.
        syncToDefinition(p);
        return p.makeNode(.export_decl, mod_tok.span, .{
            .export_decl = .{
                .module = "",
                .items = &.{},
            },
        });
    }
    const mod_name = p.tokenText(mod_tok.span);
    _ = try p.expect(.dot);
    _ = try p.expect(.l_brace);

    var names: std.ArrayList([]const u8) = .empty;
    while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
        const n_tok = try p.expect(.ident);
        try names.append(p.arena, p.tokenText(n_tok.span));
        if (p.peek().tag == .comma) _ = p.advance();
    }
    _ = try p.expect(.r_brace);
    const semi = try p.expect(.semicolon);
    const span = ast.Span{ .start = start.span.start, .end = semi.span.end };

    return p.makeNode(.export_decl, span, .{
        .export_decl = .{
            .module = mod_name,
            .items = try names.toOwnedSlice(p.arena),
        },
    });
}

// ─── §7-§10: Definitions ──────────────────────────────────────────────────

fn parseDefinition(p: *Parser) !*ast.Node {
    // Collect pending line/block comments as leading_comments using the gofmt
    // blank-line rule. We peek first so pending_comments is fully populated
    // before calling attachLeadingComments.
    const decl_peek = p.peek();
    const attach = try p.attachLeadingComments(decl_peek.span.start);

    // Leading doc comment (existing behavior: promoted to doc_comment field).
    var doc_node: ?*ast.Node = null;
    if (p.peek().tag == .doc_comment) {
        const dc_tok = p.advance();
        const text = p.tokenText(dc_tok.span);
        doc_node = try p.makeNode(.doc_comment, dc_tok.span, .{
            .doc_comment = .{ .text = text },
        });
    }

    // Collect modifiers and direction.
    var modifiers: std.ArrayList(ast.Modifier) = .empty;
    var direction: ast.Direction = .none;

    while (true) {
        const tok = p.peek();
        const mod: ?ast.Modifier = switch (tok.tag) {
            .kw_abstract => .abstract,
            .kw_derived => .derived,
            .kw_readonly => .readonly,
            .kw_ordered => .ordered,
            .kw_nonunique => .nonunique,
            .kw_individual => .individual,
            .kw_variation => .variation,
            .kw_portion => .portion,
            .kw_end => .end_kw,
            .kw_ref => .ref_kw,
            else => null,
        };
        if (mod) |m| {
            _ = p.advance();
            try modifiers.append(p.arena, m);
        } else break;
    }

    // Direction prefix.
    {
        const tok = p.peek();
        switch (tok.tag) {
            .kw_in => { _ = p.advance(); direction = .in; },
            .kw_out => { _ = p.advance(); direction = .out; },
            .kw_inout => { _ = p.advance(); direction = .inout; },
            else => {},
        }
    }

    const mods_slice = try modifiers.toOwnedSlice(p.arena);

    // Dispatch on element keyword.
    const kw = p.peek();
    return switch (kw.tag) {
        .kw_part => parseElementDef(p, .part_def, .kw_part, doc_node, mods_slice, direction, attach.leading, attach.doc),
        .kw_port => parseElementDef(p, .port_def, .kw_port, doc_node, mods_slice, direction, attach.leading, attach.doc),
        .kw_action => parseElementDef(p, .action_def, .kw_action, doc_node, mods_slice, direction, attach.leading, attach.doc),
        .kw_state => parseElementDef(p, .state_def, .kw_state, doc_node, mods_slice, direction, attach.leading, attach.doc),
        .kw_attribute => parseElementDef(p, .attribute_def, .kw_attribute, doc_node, mods_slice, direction, attach.leading, attach.doc),
        .kw_item => parseElementDef(p, .item_def, .kw_item, doc_node, mods_slice, direction, attach.leading, attach.doc),
        .kw_interface => parseElementDef(p, .interface_def, .kw_interface, doc_node, mods_slice, direction, attach.leading, attach.doc),
        .kw_connection => parseElementDef(p, .connection_def, .kw_connection, doc_node, mods_slice, direction, attach.leading, attach.doc),
        .kw_flow => parseElementDef(p, .flow_def, .kw_flow, doc_node, mods_slice, direction, attach.leading, attach.doc),
        .kw_allocation => parseElementDef(p, .allocation_def, .kw_allocation, doc_node, mods_slice, direction, attach.leading, attach.doc),
        .kw_requirement => parseRequirementDef(p, doc_node, mods_slice, direction, attach.leading, attach.doc),
        .kw_constraint => parseConstraintDef(p, doc_node, mods_slice, direction, attach.leading, attach.doc),
        .kw_calc => parseCalcDef(p, doc_node, mods_slice, direction, attach.leading, attach.doc),
        .kw_need => parseNeedDef(p, doc_node, mods_slice, direction, attach.leading, attach.doc),
        .kw_use => parseUseCaseDef(p, doc_node, mods_slice, direction, attach.leading, attach.doc),
        // Import declarations can appear here too.
        .kw_import => parseImportDecl(p),
        else => {
            // D-18 invariant: preserve the Plan 03 behavior here for the
            // 19-file showcase. The old else-arm did:
            //   advance one token, wrap it in an identifier node with
            //   name = tokenText, return.
            // This was hit by orphan doc-comments between top-level
            // definitions (showcase has many) and produced snapshot-
            // stable AST entries. Plan 05 keeps the same wrapping but
            // adds:
            //   (a) if the wrapped token is NOT a doc-comment and is NOT
            //       a known prefix (modifier/direction), emit E0101 so
            //       malformed-corpus tests see a diagnostic;
            //   (b) on true syntax-error cases, syncToDefinition picks
            //       up the rest of the bad input.
            const tok = p.advance();
            if (tok.tag != .doc_comment) {
                try p.diags.append(p.arena, .{
                    .code = "E0101",
                    .severity = .err,
                    .message = "expected definition keyword",
                    .span = tok.span,
                });
                syncToDefinition(p);
            }
            return p.makeNode(.identifier, tok.span, .{
                .identifier = .{ .name = p.tokenText(tok.span) },
            });
        },
    };
}

/// Parse a standard element definition: KEYWORD "def" IDENT [StructuralRel] Body
fn parseElementDef(
    p: *Parser,
    kind: ast.NodeKind,
    kw_tag: lexer.Tag,
    doc_node: ?*ast.Node,
    modifiers: []ast.Modifier,
    direction: ast.Direction,
    leading_comments: []ast.Comment,
    attached_doc_comment: ?*ast.DocComment,
) !*ast.Node {
    const start_tok = p.advance(); // consume element keyword
    std.debug.assert(start_tok.tag == kw_tag);
    _ = try p.expect(.kw_def);

    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);

    const specializes = try parseOptionalStructuralRelationship(p);
    const body_result = try parseDefinitionBody(p, modifiers, direction);

    const end_pos = if (body_result.end > 0) body_result.end else name_tok.span.end;
    const span = ast.Span{
        .start = if (doc_node) |dn| dn.span.start else start_tok.span.start,
        .end = end_pos,
    };

    const def = ast.ElementDef{
        .name = name,
        .modifiers = modifiers,
        .direction = direction,
        .specializes = specializes,
        .annotations = body_result.annotations,
        .members = body_result.members,
        .doc = doc_node,
        .leading_comments = leading_comments,
        .doc_comment = attached_doc_comment,
    };

    return p.makeNode(kind, span, makeElementDefPayload(kind, def));
}

fn parseRequirementDef(
    p: *Parser,
    doc_node: ?*ast.Node,
    modifiers: []ast.Modifier,
    direction: ast.Direction,
    leading_comments: []ast.Comment,
    attached_doc_comment: ?*ast.DocComment,
) !*ast.Node {
    const start_tok = p.advance(); // consume "requirement"
    _ = try p.expect(.kw_def);
    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);
    const body_result = try parseDefinitionBody(p, modifiers, direction);

    const span = ast.Span{
        .start = if (doc_node) |dn| dn.span.start else start_tok.span.start,
        .end = body_result.end,
    };
    const def = ast.ElementDef{
        .name = name,
        .modifiers = modifiers,
        .direction = direction,
        .specializes = null,
        .annotations = body_result.annotations,
        .members = body_result.members,
        .doc = doc_node,
        .leading_comments = leading_comments,
        .doc_comment = attached_doc_comment,
    };
    return p.makeNode(.requirement_def, span, .{ .requirement_def = def });
}

fn parseNeedDef(
    p: *Parser,
    doc_node: ?*ast.Node,
    modifiers: []ast.Modifier,
    direction: ast.Direction,
    leading_comments: []ast.Comment,
    attached_doc_comment: ?*ast.DocComment,
) !*ast.Node {
    const start_tok = p.advance(); // consume "need"
    _ = try p.expect(.kw_def);
    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);
    const body_result = try parseDefinitionBody(p, modifiers, direction);

    const span = ast.Span{
        .start = if (doc_node) |dn| dn.span.start else start_tok.span.start,
        .end = body_result.end,
    };
    const def = ast.ElementDef{
        .name = name,
        .modifiers = modifiers,
        .direction = direction,
        .specializes = null,
        .annotations = body_result.annotations,
        .members = body_result.members,
        .doc = doc_node,
        .leading_comments = leading_comments,
        .doc_comment = attached_doc_comment,
    };
    return p.makeNode(.need_def, span, .{ .need_def = def });
}

fn parseUseCaseDef(
    p: *Parser,
    doc_node: ?*ast.Node,
    modifiers: []ast.Modifier,
    direction: ast.Direction,
    leading_comments: []ast.Comment,
    attached_doc_comment: ?*ast.DocComment,
) !*ast.Node {
    const start_tok = p.advance(); // consume "use"
    _ = try p.expect(.kw_case);
    _ = try p.expect(.kw_def);
    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);
    const body_result = try parseDefinitionBody(p, modifiers, direction);

    const span = ast.Span{
        .start = if (doc_node) |dn| dn.span.start else start_tok.span.start,
        .end = body_result.end,
    };
    const def = ast.ElementDef{
        .name = name,
        .modifiers = modifiers,
        .direction = direction,
        .specializes = null,
        .annotations = body_result.annotations,
        .members = body_result.members,
        .doc = doc_node,
        .leading_comments = leading_comments,
        .doc_comment = attached_doc_comment,
    };
    return p.makeNode(.use_case_def, span, .{ .use_case_def = def });
}

// ─── §10.5: Calc/constraint definition parse functions (Phase 05.2 Wave 2) ──

fn parseConstraintDef(
    p: *Parser,
    doc_node: ?*ast.Node,
    modifiers: []ast.Modifier,
    direction: ast.Direction,
    leading_comments: []ast.Comment,
    attached_doc_comment: ?*ast.DocComment,
) !*ast.Node {
    const start_tok = p.advance(); // consume "constraint"
    std.debug.assert(start_tok.tag == .kw_constraint);
    _ = try p.expect(.kw_def);
    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);

    // Optional param list if peek == '('
    const params: ?*ast.Node =
        if (p.peek().tag == .l_paren) try parseParamList(p) else null;

    // Optional specializes relationship
    const specializes = try parseOptionalStructuralRelationship(p);

    // Body
    const body = try parseConstraintBody(p);

    const span = ast.Span{
        .start = if (doc_node) |dn| dn.span.start else start_tok.span.start,
        .end = body.span.end,
    };
    return p.makeNode(.constraint_def, span, .{
        .constraint_def = .{
            .name = name,
            .modifiers = modifiers,
            .direction = direction,
            .params = params,
            .specializes = specializes,
            .body = body,
            .annotations = &.{},
            .doc = doc_node,
            .leading_comments = leading_comments,
            .doc_comment = attached_doc_comment,
        },
    });
}

fn parseCalcDef(
    p: *Parser,
    doc_node: ?*ast.Node,
    modifiers: []ast.Modifier,
    direction: ast.Direction,
    leading_comments: []ast.Comment,
    attached_doc_comment: ?*ast.DocComment,
) !*ast.Node {
    const start_tok = p.advance(); // consume "calc"
    std.debug.assert(start_tok.tag == .kw_calc);
    _ = try p.expect(.kw_def);
    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);

    // Required param list: "(" ParamDecl* ")"
    const params = try parseParamList(p);

    // Required return type annotation: ":" TypeRef
    const type_node = try parseTypeAnnotation(p);

    // Optional return contract: "=>" ContractItem+
    const return_contract: ?*ast.Node =
        if (p.peek().tag == .arrow) try parseReturnContract(p) else null;

    // Calc body: "{" LocalBinding* ReturnStatement "}"
    const body = try parseCalcBody(p);

    const span = ast.Span{
        .start = if (doc_node) |dn| dn.span.start else start_tok.span.start,
        .end = body.span.end,
    };
    return p.makeNode(.calc_def, span, .{
        .calc_def = .{
            .name = name,
            .modifiers = modifiers,
            .direction = direction,
            .params = params,
            .type_node = type_node,
            .return_contract = return_contract,
            .body = body,
            .annotations = &.{},
            .doc = doc_node,
            .leading_comments = leading_comments,
            .doc_comment = attached_doc_comment,
        },
    });
}

/// Parse ":" TypeRef (required type annotation — not optional).
fn parseTypeAnnotation(p: *Parser) !*ast.Node {
    const colon_tok = try p.expect(.colon);
    const segments = try parseQualifiedNameSegments(p);
    const last_end: u32 = if (segments.len > 0) p.lex.pos else colon_tok.span.end;
    const span = ast.Span{ .start = colon_tok.span.start, .end = last_end };
    return p.makeNode(.type_annotation, span, .{
        .type_annotation = .{ .name_segments = segments },
    });
}

fn parseParamList(p: *Parser) !*ast.Node {
    const open = try p.expect(.l_paren);
    var params: std.ArrayList(*ast.Node) = .empty;

    while (p.peek().tag != .r_paren and p.peek().tag != .eof) {
        const loop_start = p.peek().span.start;
        const param = try parseParamDecl(p);
        try params.append(p.arena, param);
        if (p.peek().tag == .comma) {
            _ = p.advance();
        } else if (p.peek().tag != .r_paren and p.peek().tag != .eof) {
            // No comma and no close paren — we may be stuck on an error token.
            // Only advance if position hasn't changed since loop start (last-resort guard).
            if (p.peek().span.start == loop_start) _ = p.advance();
        }
    }
    const close = try p.expect(.r_paren);
    const span = ast.Span{ .start = open.span.start, .end = close.span.end };
    return p.makeNode(.param_list, span, .{
        .param_list = .{ .params = try params.toOwnedSlice(p.arena) },
    });
}

fn parseParamDecl(p: *Parser) !*ast.Node {
    // Optional direction prefix (D-11)
    var dir: ast.Direction = .in;
    switch (p.peek().tag) {
        .kw_in    => { _ = p.advance(); dir = .in; },
        .kw_out   => { _ = p.advance(); dir = .out; },
        .kw_inout => { _ = p.advance(); dir = .inout; },
        else => {},
    }
    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);
    _ = try p.expect(.colon);
    const type_node = try parseTypeAnnotationInner(p, name_tok.span.start);
    const mult = try parseOptionalMultiplicity(p);

    const end_pos = if (mult) |m| m.span.end else type_node.span.end;
    const span = ast.Span{ .start = name_tok.span.start, .end = end_pos };
    return p.makeNode(.param_decl, span, .{
        .param_decl = .{
            .name = name,
            .direction = dir,
            .type_node = type_node,
            .multiplicity = mult,
        },
    });
}

/// Parse type segments after the colon was already consumed by the caller.
fn parseTypeAnnotationInner(p: *Parser, colon_start: u32) !*ast.Node {
    const segments = try parseQualifiedNameSegments(p);
    const end: u32 = if (segments.len > 0) p.lex.pos else colon_start;
    const span = ast.Span{ .start = colon_start, .end = end };
    return p.makeNode(.type_annotation, span, .{
        .type_annotation = .{ .name_segments = segments },
    });
}

/// Parse "=>" ContractItem+ (contextual sig D-07; ± D-09; ConstraintRef)
fn parseReturnContract(p: *Parser) !*ast.Node {
    const arrow_tok = p.advance(); // consume .arrow ("=>")
    var items: std.ArrayList(*ast.Node) = .empty;

    while (true) {
        const tok = p.peek();
        const item: *ast.Node = blk: {
            if (tok.tag == .ident and std.mem.eql(u8, p.tokenText(tok.span), "sig")) {
                // D-07: contextual sig — only recognized after "=>"
                break :blk try parsePrecisionSpecSig(p);
            } else if (tok.tag == .plus_minus) {
                break :blk try parsePrecisionSpecPlusMinus(p);
            } else if (tok.tag == .plus and p.peek2().tag == .slash) {
                // +/- ASCII alias — consume "+", expect "/", then "-"
                break :blk try parsePrecisionSpecPlusSlashMinus(p);
            } else if (tok.tag == .ident) {
                // ConstraintRef: QualifiedName + optional args
                break :blk try parseConstraintRef(p);
            } else {
                break; // end of contract items
            }
        };
        try items.append(p.arena, item);
        if (p.peek().tag == .comma) {
            _ = p.advance(); // consume ","
        } else {
            break;
        }
    }
    const span = ast.Span{ .start = arrow_tok.span.start, .end = p.peek().span.start };
    return p.makeNode(.return_contract, span, .{
        .return_contract = .{ .items = try items.toOwnedSlice(p.arena) },
    });
}

fn parsePrecisionSpecSig(p: *Parser) !*ast.Node {
    const sig_tok = p.advance(); // consume "sig" (as ident)
    // Expect an integer literal
    const val_tok = try p.expect(.int_literal);
    const val_node = try p.makeNode(.int_literal, val_tok.span, .{
        .int_literal = .{ .text = p.tokenText(val_tok.span) },
    });
    const span = ast.Span{ .start = sig_tok.span.start, .end = val_tok.span.end };
    return p.makeNode(.precision_spec, span, .{
        .precision_spec = .{ .kind = .sig_figures, .value = val_node },
    });
}

fn parsePrecisionSpecPlusMinus(p: *Parser) !*ast.Node {
    const pm_tok = p.advance(); // consume ± token
    const val = try expr.parseExpression(p, 0);
    const span = ast.Span{ .start = pm_tok.span.start, .end = val.span.end };
    return p.makeNode(.precision_spec, span, .{
        .precision_spec = .{ .kind = .tolerance_relative, .value = val },
    });
}

fn parsePrecisionSpecPlusSlashMinus(p: *Parser) !*ast.Node {
    const plus_tok = p.advance();  // consume "+"
    _ = try p.expect(.slash);      // consume "/"
    _ = try p.expect(.minus);      // consume "-"
    const val = try expr.parseExpression(p, 0);
    const span = ast.Span{ .start = plus_tok.span.start, .end = val.span.end };
    return p.makeNode(.precision_spec, span, .{
        .precision_spec = .{ .kind = .tolerance_relative, .value = val },
    });
}

fn parseConstraintRef(p: *Parser) !*ast.Node {
    const start_pos = p.peek().span.start;
    // QualifiedName: IDENT ("." IDENT)*
    const segments = try parseQualifiedNameSegments(p);
    var args: ?[]*ast.Node = null;
    if (p.peek().tag == .l_paren) {
        // Optional argument list — reuse existing parseArgList from expr
        const call_args = try parseConstraintRefArgs(p);
        args = call_args;
    }
    const end_pos = if (args) |a| blk: {
        if (a.len > 0) break :blk a[a.len - 1].span.end;
        break :blk p.peek().span.start;
    } else if (segments.len > 0) p.lex.pos else start_pos;
    const span = ast.Span{ .start = start_pos, .end = end_pos };
    return p.makeNode(.constraint_ref, span, .{
        .constraint_ref = .{ .name_segments = segments, .args = args },
    });
}

fn parseConstraintRefArgs(p: *Parser) ![]*ast.Node {
    _ = p.advance(); // consume "("
    var args: std.ArrayList(*ast.Node) = .empty;
    while (p.peek().tag != .r_paren and p.peek().tag != .eof) {
        const arg = try expr.parseExpression(p, 0);
        try args.append(p.arena, arg);
        if (p.peek().tag == .comma) _ = p.advance();
    }
    _ = try p.expect(.r_paren);
    return args.toOwnedSlice(p.arena);
}

fn parseCalcBody(p: *Parser) !*ast.Node {
    const open = try p.expect(.l_brace);
    var bindings: std.ArrayList(*ast.Node) = .empty;
    var annots: std.ArrayList(*ast.Node) = .empty;
    var return_stmt: ?*ast.Node = null;

    while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
        const loop_tok = p.peek();
        const loop_tok_start = loop_tok.span.start;
        switch (loop_tok.tag) {
            .kw_return => {
                return_stmt = try parseReturnStatement(p);
                // ReturnStatement must be last — break after consuming it.
                break;
            },
            .kw_derived, .kw_attribute => {
                const binding = try parseMemberDeclaration(p);
                if (binding) |b| try bindings.append(p.arena, b);
            },
            .annotation, .annotation_prefix => {
                const ann = try parseAnnotationStatement(p);
                try annots.append(p.arena, ann);
            },
            .doc_comment => {
                // Doc comments are valid in calc bodies (e.g. before a binding).
                const dc_tok = p.advance();
                const dc_node = try p.makeNode(.doc_comment, dc_tok.span, .{
                    .doc_comment = .{ .text = p.tokenText(dc_tok.span) },
                });
                try annots.append(p.arena, dc_node);
            },
            else => {
                // Unexpected token — emit E2602 if this looks like out-param assignment (D-11)
                // otherwise E0100 for generic unexpected, then sync
                const next_tok = loop_tok;
                const msg: []const u8 = if (next_tok.tag == .ident) blk: {
                    const tok2 = p.peek2();
                    if (tok2.tag == .eq) {
                        break :blk "out-param assignment not valid in calc body (D-11); use a return statement";
                    }
                    break :blk "unexpected token in calc body";
                } else "unexpected token in calc body";
                const code: []const u8 = if (next_tok.tag == .ident and p.peek2().tag == .eq)
                    "E2602" else "E0100";
                try p.diags.append(p.arena, .{
                    .code = code,
                    .severity = .err,
                    .message = msg,
                    .span = next_tok.span,
                });
                syncToStatement(p);
                // Forward progress guard: consume STATEMENT_SYNC tokens that
                // aren't body-terminators to prevent infinite loops.
                const after = p.peek();
                if (after.tag == .semicolon or after.tag == .comma) {
                    _ = p.advance();
                } else if (after.tag == .r_brace or after.tag == .eof) {
                    break;
                } else if (after.span.start == loop_tok_start) {
                    _ = p.advance(); // last-resort: force progress
                }
            },
        }
    }

    // E2601 if no ReturnStatement found
    if (return_stmt == null) {
        try p.diags.append(p.arena, .{
            .code = "E2601",
            .severity = .err,
            .message = "calc body must end with a `return` statement",
            .span = open.span,
        });
        // Synthesize a recovery return statement
        const dummy_ident = try p.makeNode(.identifier, open.span, .{
            .identifier = .{ .name = "<missing>" },
        });
        return_stmt = try p.makeNode(.require_statement, open.span, .{
            .require_statement = .{ .condition = dummy_ident, .precision = null },
        });
    }

    const close = try p.expect(.r_brace);
    const span = ast.Span{ .start = open.span.start, .end = close.span.end };
    return p.makeNode(.calc_body, span, .{
        .calc_body = .{
            .bindings = try bindings.toOwnedSlice(p.arena),
            .return_stmt = return_stmt.?,
            .annotations = try annots.toOwnedSlice(p.arena),
        },
    });
}

fn parseReturnStatement(p: *Parser) !*ast.Node {
    const kw_tok = p.advance(); // consume "return"
    std.debug.assert(kw_tok.tag == .kw_return);
    const val = try expr.parseExpression(p, 0);
    var end_pos = val.span.end;
    if (p.peek().tag == .semicolon) {
        const semi = p.advance();
        end_pos = semi.span.end;
    }
    const span = ast.Span{ .start = kw_tok.span.start, .end = end_pos };
    // ReturnStatement reuses require_statement node kind (same payload: condition field).
    return p.makeNode(.require_statement, span, .{
        .require_statement = .{ .condition = val, .precision = null },
    });
}

fn parseConstraintBody(p: *Parser) !*ast.Node {
    const open = try p.expect(.l_brace);
    var members: std.ArrayList(*ast.Node) = .empty;

    while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
        const tok = p.peek();
        const tok_start = tok.span.start;
        switch (tok.tag) {
            .kw_require => {
                const req = try parseRequireStatement(p);
                try members.append(p.arena, req);
            },
            .kw_derived, .kw_attribute => {
                const binding = try parseMemberDeclaration(p);
                if (binding) |b| try members.append(p.arena, b);
            },
            .annotation, .annotation_prefix => {
                const ann = try parseAnnotationStatement(p);
                try members.append(p.arena, ann);
            },
            .doc_comment => {
                // Doc comments are valid in constraint bodies (attached to require stmts).
                // Consume and attach as a member (future: attach to following statement).
                const dc_tok = p.advance();
                const dc_node = try p.makeNode(.doc_comment, dc_tok.span, .{
                    .doc_comment = .{ .text = p.tokenText(dc_tok.span) },
                });
                try members.append(p.arena, dc_node);
            },
            else => {
                try p.diags.append(p.arena, .{
                    .code = "E0100",
                    .severity = .err,
                    .message = "unexpected token in constraint body",
                    .span = tok.span,
                });
                syncToStatement(p);
                // Forward progress guard: if sync stopped on a STATEMENT_SYNC
                // token that isn't a body-terminator, consume it so we don't
                // loop forever.
                const after = p.peek();
                if (after.tag == .semicolon or after.tag == .comma) {
                    _ = p.advance();
                } else if (after.tag == .r_brace or after.tag == .eof) {
                    break;
                } else if (after.span.start == tok_start) {
                    _ = p.advance(); // last-resort: force progress
                }
            },
        }
    }
    const close = try p.expect(.r_brace);
    const span = ast.Span{ .start = open.span.start, .end = close.span.end };
    return p.makeNode(.constraint_body, span, .{
        .constraint_body = .{ .members = try members.toOwnedSlice(p.arena) },
    });
}

fn parseRequireStatement(p: *Parser) !*ast.Node {
    const kw_tok = p.advance(); // consume "require"
    std.debug.assert(kw_tok.tag == .kw_require);
    const condition = try expr.parseExpression(p, 0);
    // Optional precision: "=>" ReturnContract (D-06 generalized precision)
    var precision: ?*ast.Node = null;
    if (p.peek().tag == .arrow) {
        precision = try parseReturnContract(p);
    }
    var end_pos = condition.span.end;
    if (p.peek().tag == .semicolon) {
        const semi = p.advance();
        end_pos = semi.span.end;
    }
    const span = ast.Span{ .start = kw_tok.span.start, .end = end_pos };
    return p.makeNode(.require_statement, span, .{
        .require_statement = .{ .condition = condition, .precision = precision },
    });
}

/// Convert a NodeKind to the appropriate Payload variant for ElementDef.
fn makeElementDefPayload(kind: ast.NodeKind, def: ast.ElementDef) ast.Payload {
    return switch (kind) {
        .part_def => .{ .part_def = def },
        .port_def => .{ .port_def = def },
        .action_def => .{ .action_def = def },
        .state_def => .{ .state_def = def },
        .attribute_def => .{ .attribute_def = def },
        .item_def => .{ .item_def = def },
        .interface_def => .{ .interface_def = def },
        .connection_def => .{ .connection_def = def },
        .flow_def => .{ .flow_def = def },
        .allocation_def => .{ .allocation_def = def },
        .requirement_def => .{ .requirement_def = def },
        // NOTE: constraint_def removed from makeElementDefPayload (Phase 05.2)
        // — it now uses ConstraintDefinition, not ElementDef. Handled by parseConstraintDef.
        .need_def => .{ .need_def = def },
        .use_case_def => .{ .use_case_def = def },
        else => unreachable,
    };
}

// ─── §11: Definition body ─────────────────────────────────────────────────

const BodyResult = struct {
    members: []*ast.Node,
    annotations: []*ast.Node,
    end: u32,
};

fn parseDefinitionBody(
    p: *Parser,
    _modifiers: []ast.Modifier,
    _direction: ast.Direction,
) !BodyResult {
    _ = _modifiers;
    _ = _direction;
    _ = try p.expect(.l_brace);

    var members: std.ArrayList(*ast.Node) = .empty;
    var annots: std.ArrayList(*ast.Node) = .empty;

    while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
        const tok = p.peek();
        const tok_start = tok.span.start;
        switch (tok.tag) {
            // Visibility wrappers
            .kw_public, .kw_protected, .kw_private => {
                const vw = try parseVisibilityWrapper(p);
                try members.append(p.arena, vw);
            },
            // Annotations
            .annotation, .annotation_prefix => {
                const ann = try parseAnnotationStatement(p);
                try annots.append(p.arena, ann);
            },
            // Verification block (requirement bodies)
            .kw_verification => {
                const vb = try parseVerificationBlock(p);
                try members.append(p.arena, vb);
            },
            // Precondition / postcondition (use-case bodies)
            .kw_precondition => {
                const pc = try parsePreconditionBlock(p);
                try members.append(p.arena, pc);
            },
            .kw_postcondition => {
                const pc = try parsePostconditionBlock(p);
                try members.append(p.arena, pc);
            },
            // Operator statements (<<redefines>> etc.)
            .delimited_operator => {
                const os = try parseOperatorStatement(p);
                try members.append(p.arena, os);
            },
            // Doc comment attached to next member
            .doc_comment => {
                const dc_tok = p.advance();
                const text = p.tokenText(dc_tok.span);
                const dc_node = try p.makeNode(.doc_comment, dc_tok.span, .{
                    .doc_comment = .{ .text = text },
                });
                try members.append(p.arena, dc_node);
            },
            else => {
                // Member declaration. If it returns null AND no forward
                // progress was made, sync to next statement (D-17). This
                // avoids the silent break-out-of-loop that Plan 03 used.
                const member = try parseMemberDeclaration(p);
                if (member) |m| {
                    try members.append(p.arena, m);
                } else {
                    // No member started — emit and sync. If sync lands on
                    // a STATEMENT_SYNC token like `;`, consume it; if on
                    // `r_brace`/eof, fall out of the loop.
                    if (p.peek().span.start == tok_start) {
                        try p.diags.append(p.arena, .{
                            .code = "E0100",
                            .severity = .err,
                            .message = "unexpected token in definition body",
                            .span = tok.span,
                        });
                        syncToStatement(p);
                    }
                    // Consume the sync token if it's a semicolon/comma so
                    // the next iteration starts fresh.
                    const after_sync = p.peek();
                    if (after_sync.tag == .semicolon or after_sync.tag == .comma) {
                        _ = p.advance();
                    } else if (after_sync.tag == .r_brace or after_sync.tag == .eof) {
                        break;
                    } else if (after_sync.span.start == tok_start) {
                        // Still stuck — force a single advance to guarantee
                        // termination.
                        _ = p.advance();
                    }
                }
            },
        }
    }

    const close = try p.expect(.r_brace);

    return BodyResult{
        .members = try members.toOwnedSlice(p.arena),
        .annotations = try annots.toOwnedSlice(p.arena),
        .end = close.span.end,
    };
}

// ─── §11: Visibility wrapper ──────────────────────────────────────────────

fn parseVisibilityWrapper(p: *Parser) !*ast.Node {
    const vis_tok = p.advance(); // consume visibility keyword
    const vis: ast.Visibility = switch (vis_tok.tag) {
        .kw_public => .public,
        .kw_protected => .protected,
        .kw_private => .private,
        else => .public,
    };
    _ = try p.expect(.l_paren);

    var members: std.ArrayList(*ast.Node) = .empty;

    while (p.peek().tag != .r_paren and p.peek().tag != .eof) {
        const tok = p.peek();
        if (tok.tag == .delimited_operator) {
            const os = try parseOperatorStatement(p);
            try members.append(p.arena, os);
        } else if (tok.tag == .annotation or tok.tag == .annotation_prefix) {
            const ann = try parseAnnotationStatement(p);
            try members.append(p.arena, ann);
        } else if (tok.tag == .doc_comment) {
            const dc_tok = p.advance();
            const dc_node = try p.makeNode(.doc_comment, dc_tok.span, .{
                .doc_comment = .{ .text = p.tokenText(dc_tok.span) },
            });
            try members.append(p.arena, dc_node);
        } else {
            const member = try parseMemberDeclaration(p) orelse break;
            try members.append(p.arena, member);
        }
    }

    const close = try p.expect(.r_paren);
    const span = ast.Span{ .start = vis_tok.span.start, .end = close.span.end };

    return p.makeNode(.visibility_wrapper, span, .{
        .visibility_wrapper = .{
            .visibility = vis,
            .members = try members.toOwnedSlice(p.arena),
        },
    });
}

// ─── §12: Member declarations ─────────────────────────────────────────────

/// Parse a single member. Returns null if the next token cannot start a member.
fn parseMemberDeclaration(p: *Parser) std.mem.Allocator.Error!?*ast.Node {
    // Leading doc comment
    var doc_node: ?*ast.Node = null;
    if (p.peek().tag == .doc_comment) {
        const dc_tok = p.advance();
        doc_node = try p.makeNode(.doc_comment, dc_tok.span, .{
            .doc_comment = .{ .text = p.tokenText(dc_tok.span) },
        });
    }

    // Collect modifiers
    var modifiers: std.ArrayList(ast.Modifier) = .empty;
    while (true) {
        const tok = p.peek();
        const mod: ?ast.Modifier = switch (tok.tag) {
            .kw_abstract => .abstract,
            .kw_derived => .derived,
            .kw_readonly => .readonly,
            .kw_ordered => .ordered,
            .kw_nonunique => .nonunique,
            .kw_individual => .individual,
            .kw_variation => .variation,
            .kw_portion => .portion,
            .kw_end => .end_kw,
            .kw_ref => .ref_kw,
            else => null,
        };
        if (mod) |m| {
            _ = p.advance();
            try modifiers.append(p.arena, m);
        } else break;
    }

    // Direction prefix
    var direction: ast.Direction = .none;
    {
        const tok = p.peek();
        switch (tok.tag) {
            .kw_in => { _ = p.advance(); direction = .in; },
            .kw_out => { _ = p.advance(); direction = .out; },
            .kw_inout => { _ = p.advance(); direction = .inout; },
            else => {},
        }
    }

    const mods_slice = try modifiers.toOwnedSlice(p.arena);

    const kw = p.peek();
    return switch (kw.tag) {
        .kw_part => try parseUsage(p, .part_usage, .kw_part, doc_node, mods_slice, direction),
        .kw_port => try parseUsage(p, .port_usage, .kw_port, doc_node, mods_slice, direction),
        .kw_attribute => try parseAttributeUsage(p, doc_node, mods_slice, direction),
        .kw_action => try parseActionUsage(p, doc_node, mods_slice, direction),
        .kw_actor => try parseActorUsage(p, doc_node, mods_slice, direction),
        .kw_subject => try parseSubjectUsage(p, doc_node, mods_slice, direction),
        .kw_state => try parseUsage(p, .state_usage, .kw_state, doc_node, mods_slice, direction),
        .kw_item => try parseUsage(p, .item_usage, .kw_item, doc_node, mods_slice, direction),
        .kw_interface => try parseUsage(p, .interface_usage, .kw_interface, doc_node, mods_slice, direction),
        .kw_connection => try parseUsage(p, .connection_usage, .kw_connection, doc_node, mods_slice, direction),
        .kw_flow => try parseUsage(p, .flow_usage, .kw_flow, doc_node, mods_slice, direction),
        .kw_allocation => try parseUsage(p, .allocation_usage, .kw_allocation, doc_node, mods_slice, direction),
        .kw_requirement => try parseUsage(p, .requirement_usage, .kw_requirement, doc_node, mods_slice, direction),
        .kw_constraint => try parseUsage(p, .constraint_usage, .kw_constraint, doc_node, mods_slice, direction),
        .kw_need => try parseUsage(p, .need_usage, .kw_need, doc_node, mods_slice, direction),
        .kw_use => try parseUseCaseUsage(p, doc_node, mods_slice, direction),
        else => {
            // WR-01: if modifiers were consumed but no element keyword follows,
            // surface E0120 so callers learn the user's modifier tokens were
            // dropped on the floor. Without this, abstract/derived/etc. tokens
            // get silently advanced past with no diagnostic and the resulting
            // AST is missing the modifiers a user wrote.
            if (mods_slice.len > 0 or direction != .none) {
                try p.diags.append(p.arena, .{
                    .code = "E0120",
                    .severity = .err,
                    .message = "modifiers must precede an element keyword",
                    .span = kw.span,
                });
            }
            if (doc_node != null) return doc_node; // return doc alone if no member follows
            return null;
        },
    };
}

/// Generic usage parser for part/port/state/item/interface/connection/flow/allocation/requirement/constraint/need usages.
fn parseUsage(
    p: *Parser,
    kind: ast.NodeKind,
    kw_tag: lexer.Tag,
    doc_node: ?*ast.Node,
    modifiers: []ast.Modifier,
    direction: ast.Direction,
) std.mem.Allocator.Error!*ast.Node {
    const start_tok = p.advance(); // consume element keyword
    std.debug.assert(start_tok.tag == kw_tag);

    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);

    const type_node = try parseOptionalTypeAnnotation(p);
    const mult = try parseOptionalMultiplicity(p);

    // Default value or inline body.
    var default_value: ?*ast.Node = null;
    var inline_body: ?*ast.Node = null;
    if (p.peek().tag == .eq) {
        _ = p.advance(); // consume "="
        default_value = try expr.parseExpression(p, 0);
    } else if (p.peek().tag == .l_brace) {
        inline_body = try parseInlineBody(p);
    }

    var end_pos = name_tok.span.end;
    var semi: ?lexer.Token = null;
    if (p.peek().tag == .semicolon) {
        semi = p.advance();
        end_pos = semi.?.span.end;
    } else if (default_value) |dv| {
        end_pos = dv.span.end;
    } else if (inline_body) |ib| {
        end_pos = ib.span.end;
    }

    const span = ast.Span{
        .start = if (doc_node) |dn| dn.span.start else start_tok.span.start,
        .end = end_pos,
    };

    const usage = ast.ElementUsage{
        .name = name,
        .modifiers = modifiers,
        .direction = direction,
        .type_node = type_node,
        .multiplicity = mult,
        .default_value = default_value,
        .inline_body = inline_body,
        .annotations = &.{},
        .doc = doc_node,
    };

    return p.makeNode(kind, span, makeUsagePayload(kind, usage));
}

fn makeUsagePayload(kind: ast.NodeKind, u: ast.ElementUsage) ast.Payload {
    return switch (kind) {
        .part_usage => .{ .part_usage = u },
        .port_usage => .{ .port_usage = u },
        .attribute_usage => .{ .attribute_usage = u },
        .action_usage => .{ .action_usage = u },
        .actor_usage => .{ .actor_usage = u },
        .subject_usage => .{ .subject_usage = u },
        .state_usage => .{ .state_usage = u },
        .item_usage => .{ .item_usage = u },
        .interface_usage => .{ .interface_usage = u },
        .connection_usage => .{ .connection_usage = u },
        .flow_usage => .{ .flow_usage = u },
        .allocation_usage => .{ .allocation_usage = u },
        .requirement_usage => .{ .requirement_usage = u },
        .constraint_usage => .{ .constraint_usage = u },
        .need_usage => .{ .need_usage = u },
        .use_case_usage => .{ .use_case_usage = u },
        else => unreachable,
    };
}

fn parseAttributeUsage(
    p: *Parser,
    doc_node: ?*ast.Node,
    modifiers: []ast.Modifier,
    direction: ast.Direction,
) !*ast.Node {
    const start_tok = p.advance(); // consume "attribute"
    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);
    const type_node = try parseOptionalTypeAnnotation(p);
    const mult = try parseOptionalMultiplicity(p);

    var default_value: ?*ast.Node = null;
    if (p.peek().tag == .eq) {
        _ = p.advance(); // consume "="
        default_value = try expr.parseExpression(p, 0);
    }

    // D-06 generalized precision: `=> ReturnContract` after default value, before ";"
    // FIRST-set disjointness: "=>" cannot start an expression; only appears here.
    var precision: ?*ast.Node = null;
    if (p.peek().tag == .arrow) {
        precision = try parseReturnContract(p);
    }

    var end_pos = name_tok.span.end;
    if (p.peek().tag == .semicolon) {
        const semi = p.advance();
        end_pos = semi.span.end;
    } else if (precision) |prec| {
        end_pos = prec.span.end;
    } else if (default_value) |dv| {
        end_pos = dv.span.end;
    }

    const span = ast.Span{
        .start = if (doc_node) |dn| dn.span.start else start_tok.span.start,
        .end = end_pos,
    };

    const usage = ast.ElementUsage{
        .name = name,
        .modifiers = modifiers,
        .direction = direction,
        .type_node = type_node,
        .multiplicity = mult,
        .default_value = default_value,
        .inline_body = null,
        .annotations = &.{},
        .doc = doc_node,
        .precision = precision,
    };
    return p.makeNode(.attribute_usage, span, .{ .attribute_usage = usage });
}

fn parseActionUsage(
    p: *Parser,
    doc_node: ?*ast.Node,
    modifiers: []ast.Modifier,
    direction: ast.Direction,
) !*ast.Node {
    const start_tok = p.advance(); // consume "action"
    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);
    const type_node = try parseOptionalTypeAnnotation(p);

    var end_pos = name_tok.span.end;
    if (p.peek().tag == .semicolon) {
        const semi = p.advance();
        end_pos = semi.span.end;
    }

    const span = ast.Span{
        .start = if (doc_node) |dn| dn.span.start else start_tok.span.start,
        .end = end_pos,
    };

    const usage = ast.ElementUsage{
        .name = name,
        .modifiers = modifiers,
        .direction = direction,
        .type_node = type_node,
        .multiplicity = null,
        .default_value = null,
        .inline_body = null,
        .annotations = &.{},
        .doc = doc_node,
    };
    return p.makeNode(.action_usage, span, .{ .action_usage = usage });
}

fn parseActorUsage(
    p: *Parser,
    doc_node: ?*ast.Node,
    modifiers: []ast.Modifier,
    direction: ast.Direction,
) !*ast.Node {
    const start_tok = p.advance(); // consume "actor"
    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);
    const type_node = try parseOptionalTypeAnnotation(p);
    const mult = try parseOptionalMultiplicity(p);

    var end_pos = name_tok.span.end;
    if (p.peek().tag == .semicolon) {
        const semi = p.advance();
        end_pos = semi.span.end;
    }

    const span = ast.Span{
        .start = if (doc_node) |dn| dn.span.start else start_tok.span.start,
        .end = end_pos,
    };
    const usage = ast.ElementUsage{
        .name = name,
        .modifiers = modifiers,
        .direction = direction,
        .type_node = type_node,
        .multiplicity = mult,
        .default_value = null,
        .inline_body = null,
        .annotations = &.{},
        .doc = doc_node,
    };
    return p.makeNode(.actor_usage, span, .{ .actor_usage = usage });
}

fn parseSubjectUsage(
    p: *Parser,
    doc_node: ?*ast.Node,
    modifiers: []ast.Modifier,
    direction: ast.Direction,
) !*ast.Node {
    const start_tok = p.advance(); // consume "subject"
    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);
    const type_node = try parseOptionalTypeAnnotation(p);
    const mult = try parseOptionalMultiplicity(p);

    var end_pos = name_tok.span.end;
    if (p.peek().tag == .semicolon) {
        const semi = p.advance();
        end_pos = semi.span.end;
    }

    const span = ast.Span{
        .start = if (doc_node) |dn| dn.span.start else start_tok.span.start,
        .end = end_pos,
    };
    const usage = ast.ElementUsage{
        .name = name,
        .modifiers = modifiers,
        .direction = direction,
        .type_node = type_node,
        .multiplicity = mult,
        .default_value = null,
        .inline_body = null,
        .annotations = &.{},
        .doc = doc_node,
    };
    return p.makeNode(.subject_usage, span, .{ .subject_usage = usage });
}

fn parseUseCaseUsage(
    p: *Parser,
    doc_node: ?*ast.Node,
    modifiers: []ast.Modifier,
    direction: ast.Direction,
) !*ast.Node {
    const start_tok = p.advance(); // consume "use"
    _ = try p.expect(.kw_case);
    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);
    const type_node = try parseOptionalTypeAnnotation(p);
    const mult = try parseOptionalMultiplicity(p);

    var end_pos = name_tok.span.end;
    if (p.peek().tag == .semicolon) {
        const semi = p.advance();
        end_pos = semi.span.end;
    }

    const span = ast.Span{
        .start = if (doc_node) |dn| dn.span.start else start_tok.span.start,
        .end = end_pos,
    };
    const usage = ast.ElementUsage{
        .name = name,
        .modifiers = modifiers,
        .direction = direction,
        .type_node = type_node,
        .multiplicity = mult,
        .default_value = null,
        .inline_body = null,
        .annotations = &.{},
        .doc = doc_node,
    };
    return p.makeNode(.use_case_usage, span, .{ .use_case_usage = usage });
}

// ─── §13: Type annotation, multiplicity, inline body ─────────────────────

fn parseOptionalTypeAnnotation(p: *Parser) !?*ast.Node {
    if (p.peek().tag != .colon) return null;
    const colon_tok = p.advance(); // consume ":"
    const segments = try parseQualifiedNameSegments(p);
    if (segments.len == 0) return null;
    const last_seg_end = if (segments.len > 0) blk: {
        // Find end of last segment in source.
        // We can use the lexer position which is now past the name.
        break :blk p.lex.pos;
    } else colon_tok.span.end;
    const span = ast.Span{ .start = colon_tok.span.start, .end = last_seg_end };
    return p.makeNode(.type_annotation, span, .{
        .type_annotation = .{ .name_segments = segments },
    });
}

fn parseOptionalMultiplicity(p: *Parser) !?*ast.Node {
    if (p.peek().tag != .l_bracket) return null;
    return parseMultiplicity(p);
}

fn parseMultiplicity(p: *Parser) !*ast.Node {
    const open = p.advance(); // consume "["
    var lower: u32 = 0;
    var upper: ?u32 = null;
    var unbounded = false;

    const tok = p.peek();
    if (tok.tag == .star) {
        _ = p.advance();
        unbounded = true;
        lower = 0;
    } else if (tok.tag == .int_literal) {
        const n_tok = p.advance();
        const n_str = p.tokenText(n_tok.span);
        // CR-02: emit a diagnostic on parseInt failure (overflow / non-digit)
        // rather than silently coercing to 0. Downstream consumers (LSP,
        // codegen) must be able to tell that the parsed multiplicity is the
        // result of an error so they don't act on a fabricated [0..0] bound.
        lower = std.fmt.parseInt(u32, n_str, 10) catch blk: {
            try p.diags.append(p.arena, .{
                .code = "E0100",
                .severity = .err,
                .message = "multiplicity lower bound not a valid u32",
                .span = n_tok.span,
            });
            break :blk 0;
        };

        if (p.peek().tag == .dotdot) {
            _ = p.advance(); // consume ".."
            const upper_tok = p.peek();
            if (upper_tok.tag == .star) {
                _ = p.advance();
                unbounded = true;
            } else if (upper_tok.tag == .int_literal) {
                const u_tok = p.advance();
                const u_str = p.tokenText(u_tok.span);
                // CR-02: same pattern for the upper bound — surface the error
                // instead of silently dropping back to null.
                upper = std.fmt.parseInt(u32, u_str, 10) catch blk: {
                    try p.diags.append(p.arena, .{
                        .code = "E0100",
                        .severity = .err,
                        .message = "multiplicity upper bound not a valid u32",
                        .span = u_tok.span,
                    });
                    break :blk null;
                };
            }
        } else {
            upper = lower; // [N] = [N..N]
        }
    }

    const close = try p.expect(.r_bracket);
    const span = ast.Span{ .start = open.span.start, .end = close.span.end };
    return p.makeNode(.multiplicity, span, .{
        .multiplicity = .{
            .lower = lower,
            .upper = upper,
            .unbounded = unbounded,
        },
    });
}

fn parseInlineBody(p: *Parser) std.mem.Allocator.Error!*ast.Node {
    const open = p.advance(); // consume "{"
    var members: std.ArrayList(*ast.Node) = .empty;

    while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
        const tok = p.peek();
        if (tok.tag == .delimited_operator) {
            const os = try parseOperatorStatement(p);
            try members.append(p.arena, os);
        } else if (tok.tag == .annotation or tok.tag == .annotation_prefix) {
            const ann = try parseAnnotationStatement(p);
            try members.append(p.arena, ann);
        } else {
            const member = try parseMemberDeclaration(p) orelse break;
            try members.append(p.arena, member);
        }
    }

    const close = try p.expect(.r_brace);
    const span = ast.Span{ .start = open.span.start, .end = close.span.end };
    return p.makeNode(.inline_body, span, .{
        .inline_body = .{ .members = try members.toOwnedSlice(p.arena) },
    });
}

// ─── §15: Structural relationships ───────────────────────────────────────

fn parseOptionalStructuralRelationship(p: *Parser) !?*ast.Node {
    if (p.peek().tag != .delimited_operator) return null;
    return parseStructuralRelationship(p);
}

fn parseStructuralRelationship(p: *Parser) !*ast.Node {
    const op_tok = p.advance(); // consume delimited_operator
    const op_text = p.tokenText(op_tok.span);
    // Strip << and >> to get inner text
    const inner = if (op_text.len >= 4) op_text[2 .. op_text.len - 2] else op_text;
    const segments = try parseQualifiedNameSegments(p);

    const end_pos = p.lex.pos;
    const span = ast.Span{ .start = op_tok.span.start, .end = end_pos };
    return p.makeNode(.structural_relationship, span, .{
        .structural_relationship = .{
            .op_text = inner,
            .target_segments = segments,
        },
    });
}

fn parseOperatorStatement(p: *Parser) !*ast.Node {
    const op_tok = p.advance(); // consume delimited_operator
    const op_text = p.tokenText(op_tok.span);
    const inner = if (op_text.len >= 4) op_text[2 .. op_text.len - 2] else op_text;

    const segments = try parseNamespacePathSegments(p);

    var default_value: ?*ast.Node = null;
    if (p.peek().tag == .eq) {
        _ = p.advance(); // consume "="
        default_value = try expr.parseExpression(p, 0);
    }

    var end_pos = op_tok.span.end;
    if (p.peek().tag == .semicolon) {
        const semi = p.advance();
        end_pos = semi.span.end;
    } else if (default_value) |dv| {
        end_pos = dv.span.end;
    }

    const span = ast.Span{ .start = op_tok.span.start, .end = end_pos };
    return p.makeNode(.operator_statement, span, .{
        .operator_statement = .{
            .op_text = inner,
            .target_segments = segments,
            .value = default_value,
        },
    });
}

// ─── §16: Annotations ─────────────────────────────────────────────────────

fn parseAnnotationStatement(p: *Parser) !*ast.Node {
    const tok = p.peek();
    if (tok.tag == .annotation_prefix) {
        return parseCategoryAnnotation(p);
    } else {
        return parseStandaloneAnnotation(p);
    }
}

fn parseCategoryAnnotation(p: *Parser) !*ast.Node {
    const ann_tok = p.advance(); // consume ANNOTATION_PREFIX (@category:)
    const raw = p.tokenText(ann_tok.span);
    // raw is like "@simulation:" — strip @ and trailing :
    const category = if (raw.len >= 2) raw[1 .. raw.len - 1] else raw;

    // operator (DELIMITED_OPERATOR)
    var op_text: ?[]const u8 = null;
    var target_segments: [][]const u8 = &.{};

    if (p.peek().tag == .delimited_operator) {
        const op_tok = p.advance();
        const op_raw = p.tokenText(op_tok.span);
        op_text = if (op_raw.len >= 4) op_raw[2 .. op_raw.len - 2] else op_raw;

        // Optional target name
        if (p.peek().tag == .ident) {
            target_segments = try parseQualifiedNameSegments(p);
        }
    }

    // Optional annotation body { ... } or plain expression value.
    // Examples:
    //   @confidence: 0.95              → value = real_literal
    //   @rationale: "text"             → value = string_literal
    //   @trace:<<satisfies>> REQ_01 {} → body = annotation_body
    var body: ?*ast.Node = null;
    var value: ?*ast.Node = null;
    if (p.peek().tag == .l_brace) {
        body = try parseAnnotationBody(p);
    } else if (op_text == null) {
        // No operator: if the next token can start an expression (literal,
        // ident, string, real, int, bool, template), parse it as the value.
        const nxt = p.peek();
        const can_be_value = switch (nxt.tag) {
            .int_literal, .real_literal, .string_literal, .template_literal,
            .template_head, .boolean_literal, .ident => true,
            else => false,
        };
        if (can_be_value) {
            value = try expr.parseExpression(p, 0);
        }
    }

    const end_pos = if (body) |b| b.span.end else if (value) |v| v.span.end else ann_tok.span.end;
    const span = ast.Span{ .start = ann_tok.span.start, .end = end_pos };

    return p.makeNode(.annotation, span, .{
        .annotation = .{
            .name = category,
            .category = category,
            .operator = op_text,
            .target_segments = target_segments,
            .value = value,
            .body = body,
        },
    });
}

fn parseStandaloneAnnotation(p: *Parser) !*ast.Node {
    const ann_tok = p.advance(); // consume ANNOTATION (@key)
    const raw = p.tokenText(ann_tok.span);
    // strip leading @
    const name = if (raw.len >= 2) raw[1..] else raw;

    var value: ?*ast.Node = null;
    if (p.peek().tag == .colon) {
        _ = p.advance(); // consume ":"
        // Parse value — expression
        value = try expr.parseExpression(p, 0);
    }

    const end_pos = if (value) |v| v.span.end else ann_tok.span.end;
    const span = ast.Span{ .start = ann_tok.span.start, .end = end_pos };

    return p.makeNode(.annotation, span, .{
        .annotation = .{
            .name = name,
            .category = null,
            .operator = null,
            .target_segments = &.{},
            .value = value,
            .body = null,
        },
    });
}

fn parseAnnotationBody(p: *Parser) !*ast.Node {
    const open = p.advance(); // consume "{"
    var fields: std.ArrayList(ast.AnnotationField) = .empty;

    while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
        const key_tok = p.peek();
        // Key can be ident or keyword
        const key_text: []const u8 = blk: {
            if (key_tok.tag == .ident or isKeyword(key_tok.tag)) {
                _ = p.advance();
                break :blk p.tokenText(key_tok.span);
            }
            break :blk "";
        };
        // D-01: `;` is the canonical annotation-body field terminator — consume
        // it explicitly rather than falling through to the unknown-token path.
        if (key_tok.tag == .semicolon) {
            _ = p.advance();
            continue;
        }
        if (key_text.len == 0) {
            // D-01: `,` as a field separator is rejected with a hard diagnostic
            // (E0123). Other unknown tokens get a generic expected-token error.
            if (key_tok.tag == .comma) {
                try p.diags.append(p.arena, .{
                    .code = diagnostics.Codes.e_annotation_comma_separator,
                    .severity = .err,
                    .message = "`,` is not a valid annotation field separator; use `;`",
                    .span = key_tok.span,
                });
            } else {
                try p.diags.append(p.arena, .{
                    .code = diagnostics.Codes.e_expected_token,
                    .severity = .err,
                    .message = "expected field key or `;` in annotation body",
                    .span = key_tok.span,
                });
            }
            _ = p.advance();
            continue;
        }

        _ = try p.expect(.colon);
        const val = try expr.parseExpression(p, 0);

        const field_span = ast.Span{ .start = key_tok.span.start, .end = val.span.end };
        try fields.append(p.arena, .{
            .key = key_text,
            .value = val,
            .span = field_span,
        });
    }

    const close = try p.expect(.r_brace);
    const span = ast.Span{ .start = open.span.start, .end = close.span.end };
    return p.makeNode(.annotation_body, span, .{
        .annotation_body = .{ .fields = try fields.toOwnedSlice(p.arena) },
    });
}

// ─── §17: Verification block ──────────────────────────────────────────────

fn parseVerificationBlock(p: *Parser) !*ast.Node {
    const start_tok = p.advance(); // consume "verification"
    _ = try p.expect(.l_brace);

    var fields: std.ArrayList(ast.VerifField) = .empty;

    while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
        const key_tok = p.peek();
        const fkind: ast.VerifFieldKind = switch (key_tok.tag) {
            .kw_accepts => .accepts,
            .kw_rejects => .rejects,
            .kw_threshold => .threshold,
            .kw_operator => .operator,
            .kw_conditions => .conditions,
            else => {
                // D-02: unknown key in verification block — emit a hard diagnostic
                // rather than silently dropping the token.
                try p.diags.append(p.arena, .{
                    .code = diagnostics.Codes.e_unknown_verification_key,
                    .severity = .err,
                    .message = "unknown key in verification block",
                    .span = key_tok.span,
                });
                _ = p.advance();
                continue;
            },
        };
        const fstart = p.advance(); // consume the key keyword
        _ = try p.expect(.colon);

        const val = try parseVerificationValue(p);

        if (p.peek().tag == .semicolon) _ = p.advance();

        const field_span = ast.Span{ .start = fstart.span.start, .end = val.span.end };
        try fields.append(p.arena, .{
            .kind = fkind,
            .value = val,
            .span = field_span,
        });
    }

    const close = try p.expect(.r_brace);
    const span = ast.Span{ .start = start_tok.span.start, .end = close.span.end };
    return p.makeNode(.verification_block, span, .{
        .verification_block = .{ .fields = try fields.toOwnedSlice(p.arena) },
    });
}

fn parseVerificationValue(p: *Parser) std.mem.Allocator.Error!*ast.Node {
    const tok = p.peek();
    if (tok.tag == .l_brace) {
        return parseVerificationObject(p);
    } else {
        // D-04: `l_bracket` (and all other expressions) delegate to
        // expr.parseExpression, which now owns the l_bracket → array_literal
        // atom arm. parseVerificationArray deleted.
        return expr.parseExpression(p, 0);
    }
}

fn parseVerificationObject(p: *Parser) std.mem.Allocator.Error!*ast.Node {
    const open = p.advance(); // consume "{"
    var fields: std.ArrayList(ast.AnnotationField) = .empty;

    while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
        const key_tok = p.peek();
        if (key_tok.tag != .ident and !isKeyword(key_tok.tag)) {
            _ = p.advance();
            continue;
        }
        _ = p.advance();
        const key_text = p.tokenText(key_tok.span);
        _ = try p.expect(.colon);
        const val = try parseVerificationValue(p);
        if (p.peek().tag == .comma) _ = p.advance();
        try fields.append(p.arena, .{
            .key = key_text,
            .value = val,
            .span = ast.Span{ .start = key_tok.span.start, .end = val.span.end },
        });
    }

    const close = try p.expect(.r_brace);
    const span = ast.Span{ .start = open.span.start, .end = close.span.end };
    return p.makeNode(.annotation_body, span, .{
        .annotation_body = .{ .fields = try fields.toOwnedSlice(p.arena) },
    });
}

// ─── §18: Precondition / postcondition blocks ────────────────────────────

fn parsePreconditionBlock(p: *Parser) !*ast.Node {
    const start_tok = p.advance(); // consume "precondition"
    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);
    _ = try p.expect(.l_brace);

    var conditions: std.ArrayList(*ast.Node) = .empty;
    while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
        const cond = try expr.parseExpression(p, 0);
        try conditions.append(p.arena, cond);
        // AND/OR chains are handled inside parseExpression's Pratt loop
        // so we just loop until the brace closes.
    }

    const close = try p.expect(.r_brace);
    const span = ast.Span{ .start = start_tok.span.start, .end = close.span.end };
    return p.makeNode(.precondition_block, span, .{
        .precondition_block = .{
            .name = name,
            .conditions = try conditions.toOwnedSlice(p.arena),
        },
    });
}

fn parsePostconditionBlock(p: *Parser) !*ast.Node {
    const start_tok = p.advance(); // consume "postcondition"
    const name_tok = try p.expect(.ident);
    const name = p.tokenText(name_tok.span);
    _ = try p.expect(.l_brace);

    var conditions: std.ArrayList(*ast.Node) = .empty;
    while (p.peek().tag != .r_brace and p.peek().tag != .eof) {
        const cond = try expr.parseExpression(p, 0);
        try conditions.append(p.arena, cond);
    }

    const close = try p.expect(.r_brace);
    const span = ast.Span{ .start = start_tok.span.start, .end = close.span.end };
    return p.makeNode(.postcondition_block, span, .{
        .postcondition_block = .{
            .name = name,
            .conditions = try conditions.toOwnedSlice(p.arena),
        },
    });
}

// ─── Qualified/namespace name helpers ────────────────────────────────────

fn parseQualifiedNameSegments(p: *Parser) ![][]const u8 {
    var segs: std.ArrayList([]const u8) = .empty;
    // First segment: may be ident or keyword used as identifier
    const first = p.peek();
    if (first.tag == .ident or isKeyword(first.tag)) {
        _ = p.advance();
        try segs.append(p.arena, p.tokenText(first.span));
    } else {
        return try segs.toOwnedSlice(p.arena);
    }

    while (p.peek().tag == .dot) {
        const second = p.peek2();
        if (second.tag == .ident or isKeyword(second.tag)) {
            _ = p.advance(); // consume "."
            const seg_tok = p.advance();
            try segs.append(p.arena, p.tokenText(seg_tok.span));
        } else {
            break;
        }
    }

    return try segs.toOwnedSlice(p.arena);
}

fn parseNamespacePathSegments(p: *Parser) ![][]const u8 {
    var segs: std.ArrayList([]const u8) = .empty;
    const first = p.peek();
    if (first.tag == .ident or isKeyword(first.tag)) {
        _ = p.advance();
        try segs.append(p.arena, p.tokenText(first.span));
    } else {
        return try segs.toOwnedSlice(p.arena);
    }

    while (true) {
        const next = p.peek();
        if (next.tag == .dot) {
            const second = p.peek2();
            if (second.tag == .ident or isKeyword(second.tag)) {
                _ = p.advance(); // "."
                const seg = p.advance();
                try segs.append(p.arena, p.tokenText(seg.span));
                continue;
            }
        } else if (next.tag == .coloncolon) {
            _ = p.advance(); // "::"
            const seg = p.advance();
            try segs.append(p.arena, p.tokenText(seg.span));
            continue;
        }
        break;
    }

    return try segs.toOwnedSlice(p.arena);
}

/// Does this tag start a definition (or a doc-comment / modifier that can
/// PREFIX a definition)? Used by parseDealFile's top-level loop to decide
/// whether to descend into parseDefinition or syncToDefinition first.
fn canStartDefinition(tag: lexer.Tag) bool {
    return switch (tag) {
        .kw_part, .kw_port, .kw_action, .kw_state, .kw_attribute,
        .kw_item, .kw_interface, .kw_connection, .kw_flow,
        .kw_allocation, .kw_requirement, .kw_constraint, .kw_calc, .kw_need,
        .kw_use, .kw_import,
        .kw_abstract, .kw_derived, .kw_readonly, .kw_ordered,
        .kw_nonunique, .kw_individual, .kw_variation, .kw_portion,
        .kw_end, .kw_ref,
        .doc_comment, .annotation, .annotation_prefix,
        => true,
        else => false,
    };
}

// ─── Helper: is this token a keyword? ────────────────────────────────────

fn isKeyword(tag: lexer.Tag) bool {
    return switch (tag) {
        .kw_part, .kw_port, .kw_action, .kw_state, .kw_attribute,
        .kw_item, .kw_interface, .kw_connection, .kw_flow,
        .kw_allocation, .kw_requirement, .kw_constraint, .kw_calc, .kw_need,
        .kw_def, .kw_use, .kw_case, .kw_abstract, .kw_derived,
        .kw_readonly, .kw_ordered, .kw_nonunique, .kw_individual,
        .kw_variation, .kw_portion, .kw_end, .kw_ref,
        .kw_in, .kw_out, .kw_inout, .kw_public, .kw_protected,
        .kw_private, .kw_package, .kw_import, .kw_export, .kw_as,
        .kw_actor, .kw_subject, .kw_precondition, .kw_postcondition,
        .kw_verification, .kw_accepts, .kw_rejects, .kw_threshold,
        .kw_operator, .kw_conditions, .kw_system, .kw_subsystem,
        .kw_connect, .kw_expose, .kw_traceability, .kw_satisfy,
        .kw_validate, .kw_allocate, .kw_from, .kw_to, .kw_via,
        .kw_carrying, .kw_method, .kw_status, .kw_relationship,
        .kw_criteria, .kw_evidence, .kw_compute, .kw_maps,
        .kw_gap, .kw_simulation, .kw_test, .kw_analysis, .kw_design,
        .kw_inspection, .kw_demonstration, .kw_AND, .kw_OR, .kw_NOT,
        .kw_WHERE, .kw_path, .kw_schema, .kw_created, .kw_modified,
        .kw_reviewed, .kw_hash, .kw_baseline, .kw_marking, .kw_by,
        .kw_return, .kw_require,  // Phase 05.2 Wave 2
        => true,
        else => false,
    };
}

// ─── Smoke test ───────────────────────────────────────────────────────────

test "parser_deal.smoke" {
    const std_lib = std.testing;
    const source = "package foo;\n\npart def Bar {}";
    const gpa = std_lib.allocator;

    var arena = std.heap.ArenaAllocator.init(gpa);
    defer arena.deinit();

    var diag_list: std.ArrayList(diagnostics.Diagnostic) = .empty;
    defer diag_list.deinit(gpa);

    const root = (try parseFile(arena.allocator(), source, &diag_list)) orelse {
        try std_lib.expect(false); // should not be null
        return;
    };

    try std_lib.expectEqual(ast.NodeKind.deal_file, root.kind);

    const df = root.payload.deal_file;
    try std_lib.expect(df.package_decl != null);
    try std_lib.expectEqual(@as(usize, 1), df.definitions.len);
    try std_lib.expectEqual(ast.NodeKind.part_def, df.definitions[0].kind);
    try std_lib.expectEqualStrings("Bar", df.definitions[0].payload.part_def.name);
}
