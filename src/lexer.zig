//! Stateless four-mode lexer (D-06).
//!
//! The lexer is stateless with respect to mode; the parser passes the
//! expected mode on every `next(mode)` / `peek(mode)` call. Four modes:
//!   - `.deal_def`          — definition-file body (`.deal`); `[<`, `>]`,
//!                            `/>]`, `[</` are NEVER recognized as
//!                            composition-tag delimiters — they tokenize
//!                            as plain `l_bracket` + `lt` etc.
//!   - `.dealx_outer`       — text between composition tags; `[<` is
//!                            `tag_open`, `[</` is `tag_close_open`. Other
//!                            structural tokens behave like `.deal_def`.
//!   - `.dealx_tag`         — inside `[< ... >]` or `[</ ... >]`. Recognizes
//!                            `>]`, `/>]` to close, `=` for attribute
//!                            assignment, and IDENT for prop names. `{`
//!                            is emitted as `l_brace`; the parser drives
//!                            the next call with `.dealx_expr_brace`.
//!   - `.dealx_expr_brace`  — inside `{ ... }` attribute expression body;
//!                            behaves identically to `.deal_def` per D-08
//!                            (full deal expression grammar).
//!
//! Anti-patterns avoided (RESEARCH lines 519-526):
//!   A1 — UTF-8 validation via `std.unicode.utf8ValidateSlice` (no
//!         hand-rolled byte-walker). Resolved on first use in Plan 02:
//!         signature in 0.16.0 is exactly
//!         `pub fn utf8ValidateSlice(input: []const u8) bool` per
//!         /opt/homebrew/Cellar/zig/0.16.0_1/lib/zig/std/unicode.zig L231.
//!   A2 — `ArrayList(T) = .empty` allocator-explicit only (no
//!         stored-allocator `.init(allocator)`). This module holds no
//!         growing buffers; the source slice is borrowed and per-token
//!         spans are returned by value.
//!   A3 — No pre-tokenization slab; the parser drives `next` on demand.
//!
//! Threat mitigations (PLAN <threat_model>):
//!   T-02-01 — Template `${}` interpolation depth bounded at
//!             `template_depth_limit = 64`. Past the bound the lexer emits
//!             `.unknown` and continues without panic.
//!   T-02-02 — Unterminated `/**` doc-comment scanning produces a single
//!             `.unknown` token spanning `/**` to EOF. One pass, no retry.
//!   T-02-04 — Keyword lookup is byte-exact via
//!             `std.StaticStringMap.get` (no case-folding).

const std = @import("std");
const ast = @import("ast");
const keywords = @import("keywords");

/// Parser-driven lexer mode (D-06). Each `next(mode)` call is independent;
/// the lexer never stores the mode between calls.
pub const Mode = enum {
    deal_def,
    dealx_outer,
    dealx_tag,
    dealx_expr_brace,
};

/// Token tag — every variant the lexer is allowed to emit.
///
/// Variants are grouped by `lexical.ebnf` section. `.unknown` is the
/// graceful-degradation fallback for malformed bytes and resource-bound
/// overflows; on the 19-file showcase corpus the count MUST be zero
/// (Plan 02 success criterion #1).
pub const Tag = enum {
    // §1 Foundation
    eof,
    unknown,

    // §3 Identifiers/keywords
    ident,
    /// Single-quoted unrestricted name — `'use case'`. Distinct from
    /// `string_literal` because the parser interprets it differently in
    /// name-position vs value-position contexts.
    unrestricted_name,

    // §5 Keywords (88 total — see keywords.zig). Grouped by category for
    // visual mapping back to lexical.ebnf §5.

    // Element (17)
    kw_part,
    kw_port,
    kw_action,
    kw_state,
    kw_attribute,
    kw_item,
    kw_interface,
    kw_connection,
    kw_flow,
    kw_allocation,
    kw_requirement,
    kw_constraint,
    kw_calc,  // NEW — SD-21 calc def keyword
    kw_need,
    kw_def,
    kw_use,
    kw_case,

    // Calc / constraint body keywords (SD-21/22) — 2 new entries
    // NOTE: kw_sig is NOT added — D-07 forbids global reservation of `sig`
    kw_return,   // NEW — calc result statement
    kw_require,  // NEW — constraint invariant

    // Behavioral surface keywords (BH-1..BH-7) — 20 new entries.
    // Admissible only inside ActionBody/StateBody, but globally reserved
    // (flat-reserved lexer, consistent with the rest of this table).
    kw_decide,
    kw_par,
    kw_loop,
    kw_while,
    kw_until,
    kw_for,
    kw_send,
    kw_accept,     // accept-action (distinct from kw_accepts, the verification method list)
    kw_assign,     // assign-action keyword (operator ":=" is .colon_eq)
    kw_bind,
    kw_node,
    kw_succession,
    kw_on,
    kw_entry,
    kw_do,
    kw_exit,
    kw_else,       // guard default in `[else]`
    kw_start,
    kw_done,
    kw_terminate,

    // Modifier (10)
    kw_abstract,
    kw_derived,
    kw_readonly,
    kw_ordered,
    kw_nonunique,
    kw_individual,
    kw_variation,
    kw_portion,
    kw_end,
    kw_ref,

    // Direction (3)
    kw_in,
    kw_out,
    kw_inout,

    // Visibility (3)
    kw_public,
    kw_protected,
    kw_private,

    // Module (4)
    kw_package,
    kw_import,
    kw_export,
    kw_as,

    // Requirement / use-case body (10)
    kw_actor,
    kw_subject,
    kw_precondition,
    kw_postcondition,
    kw_verification,
    kw_accepts,
    kw_rejects,
    kw_threshold,
    kw_operator,
    kw_conditions,

    // Composition tag names (8)
    kw_system,
    kw_subsystem,
    kw_connect,
    kw_expose,
    kw_traceability,
    kw_satisfy,
    kw_validate,
    kw_allocate,

    // Composition attribute names (7)
    kw_from,
    kw_to,
    kw_via,
    kw_carrying,
    kw_method,
    kw_status,
    kw_relationship,

    // Evidence / criteria (11)
    kw_criteria,
    kw_evidence,
    kw_compute,
    kw_maps,
    kw_gap,
    kw_simulation,
    kw_test,
    kw_analysis,
    kw_design,
    kw_inspection,
    kw_demonstration,

    // Logical operators (4)
    kw_AND,
    kw_OR,
    kw_NOT,
    kw_WHERE,

    // @header fields (9)
    kw_path,
    kw_schema,
    kw_created,
    kw_modified,
    kw_reviewed,
    kw_hash,
    kw_baseline,
    kw_marking,
    kw_by,

    // §4 Literals
    int_literal,
    real_literal,
    string_literal,
    boolean_literal,
    template_head,
    template_middle,
    template_tail,
    /// Template literal with NO interpolation (`` `hello` `` style).
    template_literal,

    // §6 Operators (DELIMITED_OPERATOR, ANNOTATION_PREFIX, ANNOTATION,
    // SysML aliases). DELIMITED_OPERATOR carries the inner name in its
    // span ("<<allocated to>>" — text retrieved via source slice).
    delimited_operator,
    annotation_prefix,
    annotation,
    sysml_specializes, // ":>"
    sysml_redefines, // ":>>"
    sysml_references, // "::>"
    sysml_conjugates, // "~"
    item_flow, // "~>" — BH-6 object/item flow operator (longest-match over "~")

    // §7 Delimiters & Punctuation
    l_brace,
    r_brace,
    l_paren,
    r_paren,
    l_bracket,
    r_bracket,
    semicolon,
    comma,
    dot,
    dotdot,
    coloncolon,
    colon,
    colon_eq, // ":=" — BH assign-action operator (longest-match over ":")
    eq,
    arrow, // "=>"
    thin_arrow, // "->"
    gt,
    lt,
    gt_eq,
    lt_eq,
    eq_eq,
    bang_eq,
    plus,
    plus_minus,  // NEW — ± U+00B1 (UTF-8: 0xC2 0xB1); SD-21 precision operator
    minus,
    star,
    slash,

    // §11 Composition tag delimiters (emitted ONLY in .dealx_outer / .dealx_tag)
    tag_open, // "[<"
    tag_close_open, // "[</"
    tag_close, // ">]"
    tag_self_close, // "/>]"

    // §2 Comments — all three comment forms are emitted as tokens (D-28) so
    // the parser can attach them to declaration nodes (comment_attachment.zig).
    // The parser's advance() helper silently buffers comment_line and
    // comment_block tokens and exposes them via attachComments() at
    // declaration boundaries.
    comment_line, // `//...\n` (or `//...EOF`)
    comment_block, // `/*...*/` (not doc — no double-star opening)
    doc_comment, // `/**...*/` — promoted to ?*DocComment on declarations (D-29)
};

/// Token = (tag, span). No text slot per D-15: text is recovered from
/// `source[span.start..span.end]` on demand by the parser / JSON emitter.
pub const Token = struct {
    tag: Tag,
    span: ast.Span,
};

/// Bound on `${ ... }` interpolation nesting depth (T-02-01).
pub const template_depth_limit: usize = 64;

/// The lexer's only mutable state. `pos` advances as bytes are consumed.
/// `template_depth` and `template_stack` track open `${ ... }` segments
/// inside template strings (bounded scan, T-02-01).
pub const Lexer = struct {
    source: []const u8,
    pos: u32 = 0,

    /// Per-frame interpolation depth counter for the topmost open `${...}`.
    /// We store one `u32` per open template — each entry counts the number
    /// of unmatched `{` opens within that `${...}` segment. The lexer
    /// pushes on `${` and pops on the matching `}`; the parser sees a
    /// `template_open` / `template_close` pair around the inner expression
    /// tokens.
    template_stack: [template_depth_limit]u32 = [_]u32{0} ** template_depth_limit,
    template_depth: u8 = 0,

    /// Phase 1.5 plan 02 (m08): captures the span of an unterminated `/* */`
    /// block comment discovered in `skipTrivia`. The lexer cannot append
    /// diagnostics directly (no DiagnosticCollector at this layer), so it
    /// surfaces the failure via this field. The parser drains it in
    /// `parseFile` (parser_deal.zig + parser_dealx.zig) and emits the
    /// EXISTING Codes.e_unterminated_comment (E0003) — declared in
    /// src/diagnostics.zig since Phase 1 but never wired until now.
    ///
    /// Drain semantics: parser sets this back to null after emitting so a
    /// hypothetical second-pass tokenization (e.g. tooling re-using the
    /// lexer struct) does not double-fire (T-15-02-04 bounded-state
    /// mitigation).
    unterminated_block_comment: ?ast.Span = null,

    pub fn init(source: []const u8) Lexer {
        return .{ .source = source };
    }

    /// Compute the next token in `mode`. Advances `pos` past whatever
    /// bytes the token consumes. Whitespace, line comments, and block
    /// comments are silently skipped between tokens (doc comments emit
    /// their own `.doc_comment` token).
    pub fn next(self: *Lexer, mode: Mode) Token {
        self.skipTrivia();

        const start = self.pos;
        if (start >= self.source.len) {
            return tokenAt(.eof, start, start);
        }

        const c = self.source[start];

        // Mode-dependent composition-tag dispatch (D-06).
        switch (mode) {
            .dealx_outer => {
                // `[</` → tag_close_open; `[<` → tag_open. Longest match
                // ordered so `[</` is checked first.
                if (c == '[' and self.peekByte(1) == '<') {
                    if (self.peekByte(2) == '/') {
                        self.pos += 3;
                        return tokenAt(.tag_close_open, start, self.pos);
                    }
                    self.pos += 2;
                    return tokenAt(.tag_open, start, self.pos);
                }
            },
            .dealx_tag => {
                // `/>]` → tag_self_close; `>]` → tag_close. Longest match.
                if (c == '/' and self.peekByte(1) == '>' and self.peekByte(2) == ']') {
                    self.pos += 3;
                    return tokenAt(.tag_self_close, start, self.pos);
                }
                if (c == '>' and self.peekByte(1) == ']') {
                    self.pos += 2;
                    return tokenAt(.tag_close, start, self.pos);
                }
            },
            .deal_def, .dealx_expr_brace => {
                // Composition-tag delimiters are NEVER recognized here;
                // `[<` lexes as `l_bracket` then `lt`, etc. Fall through
                // to the shared dispatch below.
            },
        }

        // Identifiers / keywords (and boolean literal which is just
        // `true` / `false` recognized after the ident scan).
        if (isIdentStart(c)) {
            return self.scanIdent(start);
        }

        // Numeric literals.
        if (isDigit(c)) {
            return self.scanNumber(start);
        }

        // Unrestricted name OR single-quoted string — both start with `'`.
        // The grammar (lexical.ebnf §3 + §4) defines `UNRESTRICTED_NAME`
        // and `SINGLE_QUOTED_STRING` with the same body shape; parser
        // context decides interpretation. We emit `.unrestricted_name` —
        // Plan 03's parser converts to a string literal in value position
        // and to an identifier in name position.
        if (c == '\'') {
            return self.scanQuotedSlice(start, '\'', .unrestricted_name);
        }

        // String literal (double-quoted).
        if (c == '"') {
            return self.scanQuotedSlice(start, '"', .string_literal);
        }

        // Template literal — backtick.
        if (c == '`') {
            return self.scanTemplateStart(start);
        }

        // Template middle/tail closing `}` — only fires when we are
        // inside a `${...}` (template_depth > 0) AND the current top-of-
        // stack depth counter is 0. Otherwise `}` is a plain `r_brace`.
        if (c == '}' and self.template_depth > 0 and
            self.template_stack[self.template_depth - 1] == 0)
        {
            self.template_depth -= 1;
            return self.scanTemplateContinuation(start);
        }

        // DELIMITED_OPERATOR `<< ... >>` — checked BEFORE the comparison
        // operators so `<<specializes>>` is one token, not two `lt`s.
        if (c == '<' and self.peekByte(1) == '<') {
            return self.scanDelimitedOperator(start);
        }

        // Doc comment `/**` vs block comment `/*` vs line comment `//`.
        // D-28: all three comment forms are emitted as tokens.
        if (c == '/') {
            const c1 = self.peekByte(1);
            if (c1 == '*') {
                if (self.peekByte(2) == '*') {
                    return self.scanDocComment(start);
                } else {
                    return self.scanBlockComment(start);
                }
            } else if (c1 == '/') {
                return self.scanLineComment(start);
            }
            // else: fall through to scanPunctuation (the `/` operator)
        }

        // Annotation prefix `@IDENT:` or bare annotation `@IDENT`.
        if (c == '@') {
            return self.scanAnnotation(start);
        }

        // Multi-byte punctuation — longest match first (RESEARCH §Pitfall 1
        // analogue for delimiters).
        return self.scanPunctuation(start);
    }

    /// Pure lookahead — restores `pos` (and template-stack state) before
    /// returning. Plan 02 uses the simplest implementation; Plan 03 may
    /// optimize with a one-token cache on the parser's frame.
    pub fn peek(self: *Lexer, mode: Mode) Token {
        const saved_pos = self.pos;
        const saved_depth = self.template_depth;
        const saved_stack = self.template_stack;
        const tok = self.next(mode);
        self.pos = saved_pos;
        self.template_depth = saved_depth;
        self.template_stack = saved_stack;
        return tok;
    }

    // ─── Trivia ────────────────────────────────────────────────────

    fn skipTrivia(self: *Lexer) void {
        // D-28 (Plan 02-01): only skip whitespace here. Comment tokens
        // (comment_line, comment_block, doc_comment) are emitted from next()
        // so the parser can attach them to declarations via attachComments().
        while (self.pos < self.source.len) {
            const c = self.source[self.pos];
            switch (c) {
                ' ', '\t', '\r', '\n' => self.pos += 1,
                else => return,
            }
        }
    }

    // ─── Comment scanners (D-28) ───────────────────────────────────

    fn scanLineComment(self: *Lexer, start: u32) Token {
        // Consume `//` and scan to end-of-line or EOF.
        // The token span includes the `//` and the newline if present.
        self.pos = start + 2;
        while (self.pos < self.source.len and self.source[self.pos] != '\n') {
            self.pos += 1;
        }
        // Consume the trailing newline if present (span includes it).
        if (self.pos < self.source.len and self.source[self.pos] == '\n') {
            self.pos += 1;
        }
        return tokenAt(.comment_line, start, self.pos);
    }

    fn scanBlockComment(self: *Lexer, start: u32) Token {
        // Consume `/*` (NOT `/**` — that's scanDocComment). Scan to `*/`.
        //
        // Phase 1.5 plan 02 (m08): capture the opening byte position so the
        // EOF-recovery branch can record an accurate span for the existing
        // Codes.e_unterminated_comment (E0003) diagnostic the parser emits on
        // drain. Unterminated block comment: record span + advance to EOF.
        self.pos = start + 2;
        while (self.pos + 1 < self.source.len) {
            if (self.source[self.pos] == '*' and self.source[self.pos + 1] == '/') {
                self.pos += 2;
                return tokenAt(.comment_block, start, self.pos);
            }
            self.pos += 1;
        } else {
            // Unterminated — record the span so the parser can emit E0003
            // on drain. Advance to EOF; next next() call still emits EOF.
            self.unterminated_block_comment = ast.Span{
                .start = start,
                .end = @intCast(self.source.len),
            };
            self.pos = @intCast(self.source.len);
            return tokenAt(.comment_block, start, self.pos);
        }
    }

    // ─── Identifier / keyword scanner ──────────────────────────────

    fn scanIdent(self: *Lexer, start: u32) Token {
        // We've already checked source[start] is an IDENT_START. Walk
        // forward over IDENT_PART characters.
        self.pos = start + 1;
        while (self.pos < self.source.len and isIdentPart(self.source[self.pos])) {
            self.pos += 1;
        }
        const slice = self.source[start..self.pos];

        // Boolean literal — `true` / `false` short-circuit the keyword
        // table per lexical.ebnf §4 BOOLEAN_LITERAL note ("the lexer
        // emits BOOLEAN_LITERAL for them rather than keyword tokens").
        if (std.mem.eql(u8, slice, "true") or std.mem.eql(u8, slice, "false")) {
            return tokenAt(.boolean_literal, start, self.pos);
        }

        // Keyword table lookup (byte-exact, T-02-04 mitigation).
        if (keywords.global_keywords.get(slice)) |kw| {
            return tokenAt(kw, start, self.pos);
        }
        return tokenAt(.ident, start, self.pos);
    }

    // ─── Number scanner (INT vs REAL) ──────────────────────────────

    fn scanNumber(self: *Lexer, start: u32) Token {
        // Eat the integer head.
        self.pos = start;
        while (self.pos < self.source.len and isDigit(self.source[self.pos])) {
            self.pos += 1;
        }

        // REAL? Requires `.` followed by AT LEAST ONE digit — and the
        // `.` must NOT be the first of a `..` token (multiplicity range).
        // Per lexical.ebnf §4: REAL_LITERAL ::= [0-9]+ "." [0-9]+ ...
        if (self.pos < self.source.len and self.source[self.pos] == '.' and
            self.peekByte(1) != '.' and
            self.pos + 1 < self.source.len and isDigit(self.source[self.pos + 1]))
        {
            self.pos += 1; // consume `.`
            while (self.pos < self.source.len and isDigit(self.source[self.pos])) {
                self.pos += 1;
            }
            // Optional exponent: ( "e" | "E" ) ( "+" | "-" )? [0-9]+
            if (self.pos < self.source.len and
                (self.source[self.pos] == 'e' or self.source[self.pos] == 'E'))
            {
                const exp_start = self.pos;
                self.pos += 1;
                if (self.pos < self.source.len and
                    (self.source[self.pos] == '+' or self.source[self.pos] == '-'))
                {
                    self.pos += 1;
                }
                if (self.pos < self.source.len and isDigit(self.source[self.pos])) {
                    while (self.pos < self.source.len and isDigit(self.source[self.pos])) {
                        self.pos += 1;
                    }
                } else {
                    // No digits after `e` — rewind; the `e` belongs to a
                    // following identifier or is itself invalid. The
                    // identifier scanner will pick it up next pass.
                    self.pos = exp_start;
                }
            }
            return tokenAt(.real_literal, start, self.pos);
        }

        return tokenAt(.int_literal, start, self.pos);
    }

    // ─── String / unrestricted-name scanner ────────────────────────

    fn scanQuotedSlice(self: *Lexer, start: u32, quote: u8, tag: Tag) Token {
        // Consume opening quote.
        self.pos = start + 1;
        while (self.pos < self.source.len) {
            const c = self.source[self.pos];
            if (c == quote) {
                self.pos += 1;
                return tokenAt(tag, start, self.pos);
            }
            if (c == '\\' and self.pos + 1 < self.source.len) {
                // Skip escape sequence (lexical.ebnf §4 ESCAPE_SEQUENCE).
                // We don't validate the escape body in Plan 02 — the
                // parser / semantic pass can re-validate when it needs
                // the literal value.
                self.pos += 2;
                continue;
            }
            if (c == '\n') {
                // Unterminated string. The grammar disallows raw newline
                // in a string body — bail out as `.unknown` spanning from
                // the opening quote to the newline so the parser can
                // recover cleanly.
                return tokenAt(.unknown, start, self.pos);
            }
            self.pos += 1;
        }
        // EOF before closing quote — same recovery shape.
        return tokenAt(.unknown, start, self.pos);
    }

    // ─── Template literal scanner ──────────────────────────────────

    fn scanTemplateStart(self: *Lexer, start: u32) Token {
        // Consume opening backtick. We then scan body characters looking
        // for either `${` (push depth, emit template_head) or the
        // closing backtick (emit template_literal — no interpolation).
        self.pos = start + 1;
        while (self.pos < self.source.len) {
            const c = self.source[self.pos];
            if (c == '`') {
                self.pos += 1;
                return tokenAt(.template_literal, start, self.pos);
            }
            if (c == '\\' and self.pos + 1 < self.source.len) {
                self.pos += 2;
                continue;
            }
            if (c == '$' and self.peekByte(1) == '{') {
                // Push a new template-depth frame.
                if (self.template_depth >= template_depth_limit) {
                    // T-02-01 DoS bound: don't recurse further. Emit
                    // `.unknown` at the offending `${`; the lexer
                    // continues but the parser will recover. Set pos
                    // past the `${` so we don't loop on the same byte.
                    const fail_pos = self.pos;
                    self.pos += 2;
                    return tokenAt(.unknown, fail_pos, self.pos);
                }
                self.pos += 2; // consume `${`
                self.template_stack[self.template_depth] = 0;
                self.template_depth += 1;
                return tokenAt(.template_head, start, self.pos);
            }
            self.pos += 1;
        }
        // EOF before closing — `.unknown` spanning the partial template.
        return tokenAt(.unknown, start, self.pos);
    }

    fn scanTemplateContinuation(self: *Lexer, start: u32) Token {
        // Called when we just popped a `${...}` frame on encountering `}`.
        // Scan body characters until either the next `${` (template_middle)
        // or the closing backtick (template_tail).
        self.pos = start + 1; // consume the `}`
        while (self.pos < self.source.len) {
            const c = self.source[self.pos];
            if (c == '`') {
                self.pos += 1;
                return tokenAt(.template_tail, start, self.pos);
            }
            if (c == '\\' and self.pos + 1 < self.source.len) {
                self.pos += 2;
                continue;
            }
            if (c == '$' and self.peekByte(1) == '{') {
                if (self.template_depth >= template_depth_limit) {
                    const fail_pos = self.pos;
                    self.pos += 2;
                    return tokenAt(.unknown, fail_pos, self.pos);
                }
                self.pos += 2;
                self.template_stack[self.template_depth] = 0;
                self.template_depth += 1;
                return tokenAt(.template_middle, start, self.pos);
            }
            self.pos += 1;
        }
        return tokenAt(.unknown, start, self.pos);
    }

    // ─── Delimited operator `<< ... >>` ────────────────────────────

    fn scanDelimitedOperator(self: *Lexer, start: u32) Token {
        // Consume opening `<<`.
        self.pos = start + 2;
        // OPERATOR_NAME ::= ( [a-zA-Z] | " " )+
        while (self.pos < self.source.len) {
            const c = self.source[self.pos];
            if (c == '>' and self.peekByte(1) == '>') {
                self.pos += 2;
                return tokenAt(.delimited_operator, start, self.pos);
            }
            if (isAlpha(c) or c == ' ') {
                self.pos += 1;
                continue;
            }
            // Anything else aborts — recover as `.unknown` spanning from
            // `<<` to the bad byte.
            return tokenAt(.unknown, start, self.pos);
        }
        return tokenAt(.unknown, start, self.pos);
    }

    // ─── Doc comment `/** ... */` ──────────────────────────────────

    fn scanDocComment(self: *Lexer, start: u32) Token {
        // Consume `/**`. T-02-02: scan to either `*/` or EOF.
        self.pos = start + 3;
        while (self.pos + 1 < self.source.len) {
            if (self.source[self.pos] == '*' and self.source[self.pos + 1] == '/') {
                self.pos += 2;
                return tokenAt(.doc_comment, start, self.pos);
            }
            self.pos += 1;
        }
        // Unterminated — span from `/**` to EOF, emit `.unknown` (per
        // T-02-02 and lexical.ebnf §13). Plan 05's recovery will translate
        // this `.unknown` into diagnostic E0001 "unterminated doc-comment".
        self.pos = @intCast(self.source.len);
        return tokenAt(.unknown, start, self.pos);
    }

    // ─── Annotations `@IDENT(:?)` ──────────────────────────────────

    fn scanAnnotation(self: *Lexer, start: u32) Token {
        // Consume `@`, then IDENT, then optionally `:` (longest match).
        self.pos = start + 1;
        if (self.pos >= self.source.len or !isIdentStart(self.source[self.pos])) {
            // Bare `@` is not valid in any production.
            return tokenAt(.unknown, start, self.pos);
        }
        self.pos += 1;
        while (self.pos < self.source.len and isIdentPart(self.source[self.pos])) {
            self.pos += 1;
        }
        if (self.pos < self.source.len and self.source[self.pos] == ':') {
            // Lookahead: `:` followed by `:` would be a `::` after a
            // plain `@IDENT`. The grammar's ANNOTATION_PREFIX consumes a
            // single `:` only. We're safe to consume one `:` here because
            // `@trace::` doesn't appear in the grammar.
            self.pos += 1;
            return tokenAt(.annotation_prefix, start, self.pos);
        }
        return tokenAt(.annotation, start, self.pos);
    }

    // ─── Punctuation (longest-match) ───────────────────────────────

    fn scanPunctuation(self: *Lexer, start: u32) Token {
        const c0 = self.source[start];
        const c1 = self.peekByte(1);
        const c2 = self.peekByte(2);

        // 3-byte sequences first.
        if (c0 == ':' and c1 == ':' and c2 == '>') {
            self.pos = start + 3;
            return tokenAt(.sysml_references, start, self.pos);
        }
        if (c0 == ':' and c1 == '>' and c2 == '>') {
            self.pos = start + 3;
            return tokenAt(.sysml_redefines, start, self.pos);
        }

        // 2-byte sequences.
        switch (c0) {
            ':' => {
                if (c1 == ':') {
                    self.pos = start + 2;
                    return tokenAt(.coloncolon, start, self.pos);
                }
                if (c1 == '>') {
                    self.pos = start + 2;
                    return tokenAt(.sysml_specializes, start, self.pos);
                }
                if (c1 == '=') {
                    // ":=" assign-action operator (BH). Distinct from "=" (eq),
                    // ":" (colon), ":>" (specializes), "::"/"::>"/":>>" above.
                    self.pos = start + 2;
                    return tokenAt(.colon_eq, start, self.pos);
                }
                self.pos = start + 1;
                return tokenAt(.colon, start, self.pos);
            },
            '=' => {
                if (c1 == '=') {
                    self.pos = start + 2;
                    return tokenAt(.eq_eq, start, self.pos);
                }
                if (c1 == '>') {
                    self.pos = start + 2;
                    return tokenAt(.arrow, start, self.pos);
                }
                self.pos = start + 1;
                return tokenAt(.eq, start, self.pos);
            },
            '!' => {
                if (c1 == '=') {
                    self.pos = start + 2;
                    return tokenAt(.bang_eq, start, self.pos);
                }
                // bare `!` not in the grammar; treat as unknown.
                self.pos = start + 1;
                return tokenAt(.unknown, start, self.pos);
            },
            '<' => {
                if (c1 == '=') {
                    self.pos = start + 2;
                    return tokenAt(.lt_eq, start, self.pos);
                }
                self.pos = start + 1;
                return tokenAt(.lt, start, self.pos);
            },
            '>' => {
                if (c1 == '=') {
                    self.pos = start + 2;
                    return tokenAt(.gt_eq, start, self.pos);
                }
                self.pos = start + 1;
                return tokenAt(.gt, start, self.pos);
            },
            '.' => {
                if (c1 == '.') {
                    self.pos = start + 2;
                    return tokenAt(.dotdot, start, self.pos);
                }
                self.pos = start + 1;
                return tokenAt(.dot, start, self.pos);
            },
            '-' => {
                if (c1 == '>') {
                    self.pos = start + 2;
                    return tokenAt(.thin_arrow, start, self.pos);
                }
                self.pos = start + 1;
                return tokenAt(.minus, start, self.pos);
            },
            // 1-byte punctuation.
            '{' => {
                // Track brace-depth for the topmost open template.
                if (self.template_depth > 0) {
                    self.template_stack[self.template_depth - 1] += 1;
                }
                self.pos = start + 1;
                return tokenAt(.l_brace, start, self.pos);
            },
            '}' => {
                // template_depth>0 with stack[top]==0 is handled in `next`
                // before we reach scanPunctuation. Here we know either
                // template_depth == 0 OR stack[top] > 0 — decrement and
                // emit plain r_brace.
                if (self.template_depth > 0) {
                    self.template_stack[self.template_depth - 1] -= 1;
                }
                self.pos = start + 1;
                return tokenAt(.r_brace, start, self.pos);
            },
            '(' => {
                self.pos = start + 1;
                return tokenAt(.l_paren, start, self.pos);
            },
            ')' => {
                self.pos = start + 1;
                return tokenAt(.r_paren, start, self.pos);
            },
            '[' => {
                self.pos = start + 1;
                return tokenAt(.l_bracket, start, self.pos);
            },
            ']' => {
                self.pos = start + 1;
                return tokenAt(.r_bracket, start, self.pos);
            },
            ';' => {
                self.pos = start + 1;
                return tokenAt(.semicolon, start, self.pos);
            },
            ',' => {
                self.pos = start + 1;
                return tokenAt(.comma, start, self.pos);
            },
            '+' => {
                self.pos = start + 1;
                return tokenAt(.plus, start, self.pos);
            },
            '*' => {
                self.pos = start + 1;
                return tokenAt(.star, start, self.pos);
            },
            '/' => {
                self.pos = start + 1;
                return tokenAt(.slash, start, self.pos);
            },
            '~' => {
                if (c1 == '>') {
                    // "~>" item/object flow operator (BH-6). Longest-match over
                    // "~" (sysml_conjugates).
                    self.pos = start + 2;
                    return tokenAt(.item_flow, start, self.pos);
                }
                self.pos = start + 1;
                return tokenAt(.sysml_conjugates, start, self.pos);
            },
            else => {
                // ± U+00B1 is 0xC2 0xB1 in UTF-8 (2 bytes). Dispatch
                // BEFORE the generic high-bit handler so we emit one
                // .plus_minus token rather than two .unknown tokens.
                // peekByte(1) is bounds-safe: returns 0 past end-of-buffer
                // (see peekByte helper below), so a truncated 0xC2 at EOF
                // falls through to the generic path. (T-05.2-W1-01)
                if (c0 == 0xC2 and self.peekByte(1) == 0xB1) {
                    self.pos = start + 2;
                    return tokenAt(.plus_minus, start, self.pos);
                }
                // High-bit byte? Try to skip a full UTF-8 scalar without
                // crashing; emit `.unknown` for it. Multi-byte UTF-8
                // bytes outside string literals are not legal at this
                // point in the grammar but we must degrade gracefully
                // (A1 / RESEARCH line 519). std.unicode.utf8ValidateSlice
                // gives us a fast-path for the common "valid byte but no
                // grammar slot" case.
                if (c0 >= 0x80) {
                    // Compute UTF-8 byte length from the leading byte.
                    const len: u32 =
                        if (c0 < 0xC0) 1 // continuation byte mid-stream
                        else if (c0 < 0xE0) 2
                        else if (c0 < 0xF0) 3
                        else 4;
                    const end_pos: u32 = @min(start + len, @as(u32, @intCast(self.source.len)));
                    if (std.unicode.utf8ValidateSlice(self.source[start..end_pos])) {
                        self.pos = end_pos;
                    } else {
                        self.pos = start + 1;
                    }
                    return tokenAt(.unknown, start, self.pos);
                }
                // Plain ASCII byte with no grammar slot.
                self.pos = start + 1;
                return tokenAt(.unknown, start, self.pos);
            },
        }
    }

    // ─── Helpers ───────────────────────────────────────────────────

    inline fn peekByte(self: *const Lexer, offset: u32) u8 {
        const idx = @as(usize, self.pos) + offset;
        if (idx >= self.source.len) return 0;
        return self.source[idx];
    }
};

inline fn tokenAt(tag: Tag, start: u32, end: u32) Token {
    return .{ .tag = tag, .span = .{ .start = start, .end = end } };
}

inline fn isAlpha(c: u8) bool {
    return (c >= 'a' and c <= 'z') or (c >= 'A' and c <= 'Z');
}

inline fn isDigit(c: u8) bool {
    return c >= '0' and c <= '9';
}

inline fn isIdentStart(c: u8) bool {
    return isAlpha(c) or c == '_';
}

inline fn isIdentPart(c: u8) bool {
    return isAlpha(c) or isDigit(c) or c == '_';
}
