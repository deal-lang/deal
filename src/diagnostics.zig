//! Structured diagnostics (D-14, D-15, D-16).
//!
//! The data model is rich on purpose so Phase 3's LSP can consume the JSON
//! shape without re-instrumenting the parser. Phase 1 emits the same rich
//! structure and renders a single line per diagnostic at the CLI.
//!
//! Error-code namespaces (D-16) — never renumber once shipped:
//!   E0001..E0099  lexer
//!   E0100..E0299  parser-deal
//!   E0300..E0399  parser-dealx
//!   E0400..E0499  recovery / structural
//!   W0500..W0599  warnings
//!   H0600..H0699  hints
//!   E2000..E2099  semantic — name resolution (Phase 2 D-33)
//!   E2100..E2199  semantic — type checking (Phase 2 D-33)
//!   E2200..E2299  semantic — multiplicity enforcement (Phase 2 D-33)
//!   E2300..E2349  semantic — <<specializes>> compatibility (Phase 2 D-33)
//!   E2350..E2399  semantic — @trace reference validation (Phase 2 D-33)
//!   E2400..E2499  semantic — import resolution (Phase 2 D-33)
//!   E2500..E2599  semantic — dimensional algebra / unit checks (Phase 4 D-33 extension)
//!                            regression fixtures: tests/regressions/sema/07-dimensional*.deal
//!   E2600..E2699  semantic — calc/constraint/precision (Phase 05.2 D-10)

const std = @import("std");
const ast = @import("ast");

pub const Severity = enum(u8) {
    err = 0,
    warn = 1,
    info = 2,
    hint = 3,
};

/// A secondary span with an attached label, e.g. "opened here" for the
/// matched `[<system>]` open tag on a mismatched-close-tag diagnostic (D-07).
pub const SpanLabel = struct {
    span: ast.Span,
    label: []const u8,
};

/// Editor-actionable replacement suggestion (D-14). Plan 05's recovery
/// machinery may attach one of these where the synchronization point makes
/// the fix unambiguous.
pub const FixIt = struct {
    replacement: []const u8,
    replace_span: ast.Span,
};

/// Rich diagnostic record (D-14). Field order matches RESEARCH §Example E.
/// The JSON emitter writes fields in alphabetical order per D-18; the Zig
/// field order here is documentation-friendly (severity right after code).
pub const Diagnostic = struct {
    /// Letter-prefixed numeric code per D-16 (e.g. "E0302").
    code: []const u8,
    severity: Severity,
    message: []const u8,
    /// Primary byte-offset span (D-15).
    span: ast.Span,
    secondary_spans: []const SpanLabel = &.{},
    fix_it: ?FixIt = null,
    notes: []const u8 = "",
};

/// D-16 error-code namespace (LOCKED — codes never change meaning across
/// versions). Plans 02/03/04 emit some of these codes as inline string
/// literals; Plan 05 introduces the named constants here so new callsites
/// (sync helpers, expect, recovery emissions) use them. Old callsites keep
/// their string literals — the on-the-wire JSON output is identical.
pub const Codes = struct {
    // Lexer (E0001..E0099)
    pub const e_invalid_utf8 = "E0001";
    pub const e_unterminated_string = "E0002";
    pub const e_unterminated_comment = "E0003";
    pub const e_source_too_large = "E0004";
    pub const e_template_too_deep = "E0005";

    // Parser-deal (E0100..E0299)
    pub const e_expected_token = "E0100";
    pub const e_expected_definition = "E0101";
    pub const e_unexpected_eof = "E0102";
    pub const e_expected_attribute_value = "E0103";
    pub const e_expected_expression = "E0110";
    pub const e_expected_identifier = "E0111";
    pub const e_invalid_modifier = "E0120";
    // E0121 — `.` not followed by an identifier (member-access without member name)
    pub const e_dangling_dot = "E0121";
    // E0122 — function-call arg list contains a comma with no preceding/following expression
    pub const e_empty_arg_comma = "E0122";
    // E0123 — annotation body field separator is `,` (rejected; only `;` is canonical)
    pub const e_annotation_comma_separator = "E0123";
    // E0124 — unrecognized key inside a `verification { ... }` block
    pub const e_unknown_verification_key = "E0124";

    // Parser-dealx (E0300..E0399)
    pub const e_unmatched_close_tag = "E0301";
    pub const e_mismatched_close_tag = "E0302";
    pub const e_nesting_too_deep_tag = "E0303";
    pub const e_unclosed_tag_at_eof = "E0304";

    // Recovery / structural (E0400..E0499)
    pub const e_sync_dropped_tokens = "E0400";
    pub const e_unclosed_brace = "E0401";
    pub const e_unclosed_bracket = "E0402";
    pub const e_nesting_too_deep_expr = "E0403";

    // Warnings (W0500..W0599)
    pub const w_unused_import = "W0500";

    // Hints (H0600..H0699)
    pub const h_did_you_mean = "H0600";

    // Semantic — E2000..E2499 (Phase 2 D-33; new band, never renumbered)
    // Each constant cites its semantic check + the SPEC.md acceptance fixture.

    // E2000..E2099 — Check #1 name resolution
    // tests/regressions/sema/01-name-resolution.deal
    /// Check #1 — tests/regressions/sema/01-name-resolution.deal
    pub const e_name_not_found = "E2000";
    /// Check #1 — ambiguous identifier resolves to multiple declarations
    pub const e_ambiguous_name = "E2001";
    /// Check #1 — same name declared more than once in the same scope
    pub const e_duplicate_declaration = "E2002";

    // E2100..E2199 — Check #2 type checking
    // tests/regressions/sema/02-type-check.deal
    /// Check #2 — tests/regressions/sema/02-type-check.deal
    pub const e_type_mismatch = "E2100";
    /// Check #2 — attribute or usage requires an explicit type annotation
    pub const e_type_annotation_required = "E2101";

    // E2200..E2299 — Check #3 multiplicity enforcement
    // tests/regressions/sema/03-multiplicity.deal
    /// Check #3 — tests/regressions/sema/03-multiplicity.deal
    pub const e_multiplicity_violation = "E2200";
    /// Check #3 — required property (multiplicity [1]) is missing in a composition
    pub const e_multiplicity_required_missing = "E2201";

    // E2300..E2349 — Check #4 <<specializes>> compatibility
    // tests/regressions/sema/04-specializes.deal
    /// Check #4 — tests/regressions/sema/04-specializes.deal (cycle)
    pub const e_specialization_cycle = "E2300";
    /// Check #4 — specializes target has incompatible kind
    pub const e_specialization_type_mismatch = "E2301";

    // E2350..E2399 — Check #5 @trace reference validation
    // tests/regressions/sema/05-trace.deal
    /// Check #5 — tests/regressions/sema/05-trace.deal
    pub const e_trace_target_not_found = "E2350";

    // E2400..E2499 — Check #6 import resolution
    // tests/regressions/sema/06-import.deal
    /// Check #6 — tests/regressions/sema/06-import.deal
    pub const e_unresolved_import = "E2400";
    /// Check #6 — import forms a cycle between packages
    pub const e_circular_import = "E2401";
    /// Check #6 — [dependencies] entry present in deal.toml but .deal/deps/<name>/ absent;
    ///             emitted by the dependency checker, suggests running `deal install`
    ///             (per RESEARCH Pitfall 4, import-resolution band E2400..E2499)
    pub const e_dependency_not_resolved = "E2402";

    // E2500..E2599 — Check #7 dimensional algebra / unit checks (Phase 4 D-33 extension)
    // tests/regressions/sema/07-dimensional-mismatch.deal
    // tests/regressions/sema/07-mixed-unit.deal
    // tests/regressions/sema/07-unknown-unit.deal
    /// Check #7 — expected dimension != actual (e.g. Mass attribute assigned a Voltage value);
    ///             per D-55 dimensional type safety
    pub const e_dimension_mismatch = "E2500";
    /// Check #7 — mixed-unit same-dimension expression without explicit conversion
    ///             (e.g. comparing a kg value to an lb value directly); per D-57
    pub const e_mixed_unit_comparison = "E2501";
    /// Check #7 — unit literal not resolvable to any declared dimension
    pub const e_unknown_unit = "E2502";
    /// Check #7 — wrong source/target dimension in a unit conversion call
    pub const e_conversion_type_mismatch = "E2503";

    // E2600..E2699 — Check #8 calc/constraint/precision (Phase 05.2 D-10)
    // tests/malformed/m25_calc_out_param_expr.deal
    // tests/malformed/m26_calc_no_return.deal
    // tests/malformed/m27_require_not_boolean.deal
    /// Check #8a — out-param calc used in expression position (purity rule D7/D-10)
    pub const e_calc_purity_violation = "E2600";
    /// Check #8b — calc body has no ReturnStatement (sema authoritative; parser also emits)
    pub const e_calc_missing_return = "E2601";
    /// Check #8c — in-language out-param assignment body (parse error per D-11)
    pub const e_calc_out_param_assignment_body = "E2602";
    /// Check #8d — require expression result is not Boolean type
    pub const e_require_not_boolean = "E2610";
    /// Check #8e — ConstraintRef in return contract not resolved to a constraint_def, or cycle
    pub const e_constraint_ref_not_found = "E2611";

    // E2700..E2799 — Stage-2 behavioral surface resolution (BH-1..BH-7, S2.4)
    /// A succession / decide / par / transition target references a step that is
    /// not declared in the enclosing action/state body.
    pub const e_behavioral_step_not_found = "E2700";
    /// An item-flow `: FlowType` does not resolve to a known type.
    pub const e_behavioral_flow_type_not_found = "E2701";
};

/// Ergonomic helper that wraps the parser's diagnostic ArrayList + arena
/// allocator. Provides three emit methods:
///   - emit:              for a literal message slice (caller-owned bytes
///                        living for at least as long as the arena).
///   - emitWithSecondary: same shape plus a secondary-span slice; the slice
///                        is duped into the arena to guarantee lifetime.
///   - emitFmt:           comptime-fmt + runtime args; the result is allocated
///                        in the arena via std.fmt.allocPrint.
///
/// SECURITY (T-DiagInjection / RESEARCH line 1012, V7 line 993):
///   `emitFmt` REQUIRES `comptime fmt: []const u8`. This is a build-time
///   guarantee: source bytes (which are runtime data) can NEVER reach the
///   format-string machinery — Zig 0.16.0 rejects any non-comptime fmt
///   string at compile time. Any call like `emitFmt(... , source_slice, .{})`
///   fails to compile. Token tag names interpolated via `{s}` are safe
///   because @tagName(tok.tag) returns a comptime-known string from a
///   bounded enum.
///
/// ANTI-PATTERN MITIGATION (RESEARCH line 521 — "Allocating diagnostic
///   strings outside the arena"): the collector ALWAYS uses `self.arena`
///   for any new allocation it performs. Caller-supplied slices that are
///   not arena-owned must be duped via emitFmt's allocPrint path. Helpers
///   here document the lifetime contract at each entry point.
pub const DiagnosticCollector = struct {
    list: *std.ArrayList(Diagnostic),
    arena: std.mem.Allocator,

    pub fn init(list: *std.ArrayList(Diagnostic), arena: std.mem.Allocator) DiagnosticCollector {
        return .{ .list = list, .arena = arena };
    }

    /// Append a Diagnostic with default secondary_spans/fix_it/notes.
    /// `code` and `message` must outlive the collector (callers typically
    /// pass `Codes.e_*` constants and arena-owned message strings).
    pub fn emit(
        self: *DiagnosticCollector,
        code: []const u8,
        severity: Severity,
        message: []const u8,
        span: ast.Span,
    ) !void {
        try self.list.append(self.arena, .{
            .code = code,
            .severity = severity,
            .message = message,
            .span = span,
        });
    }

    /// Append a Diagnostic carrying a secondary-span slice. The slice is
    /// duped into the arena so the caller's stack-allocated buffer can
    /// safely go out of scope.
    pub fn emitWithSecondary(
        self: *DiagnosticCollector,
        code: []const u8,
        severity: Severity,
        message: []const u8,
        span: ast.Span,
        secondary: []const SpanLabel,
    ) !void {
        const owned = try self.arena.dupe(SpanLabel, secondary);
        try self.list.append(self.arena, .{
            .code = code,
            .severity = severity,
            .message = message,
            .span = span,
            .secondary_spans = owned,
        });
    }

    /// Append a Diagnostic whose message is formatted with `std.fmt`.
    /// `fmt` is comptime — T-DiagInjection mitigation. The resulting message
    /// is allocated in the arena and owns its bytes for the arena's lifetime.
    pub fn emitFmt(
        self: *DiagnosticCollector,
        code: []const u8,
        severity: Severity,
        span: ast.Span,
        comptime fmt: []const u8,
        args: anytype,
    ) !void {
        const msg = try std.fmt.allocPrint(self.arena, fmt, args);
        try self.list.append(self.arena, .{
            .code = code,
            .severity = severity,
            .message = msg,
            .span = span,
        });
    }
};
