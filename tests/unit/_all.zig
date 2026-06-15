//! Unit-test umbrella — single entry point for `zig build test`'s
//! lexer/parser/etc. unit suites. The Zig 0.16.0 idiom for collecting
//! tests across multiple files into a single addTest invocation is to
//! `@import` each file from a comptime block; the test runner then
//! discovers every `test "..."` declared in those imports.
//!
//! Wired here so build.zig can spawn a SECOND addTest target rooted at
//! `tests/unit/_all.zig` (alongside the existing src/lib.zig target).
//! `-Dtest-filter=lexer.mode_flip` matches by test-name substring across
//! both targets.

comptime {
    _ = @import("comment_attachment.zig");
    _ = @import("lexer_mode_flip.zig");
    _ = @import("lexer_keywords.zig");
    _ = @import("lexer_calc.zig");
    _ = @import("lexer_behavioral.zig");
    _ = @import("lexer_comments.zig");
    _ = @import("lexer_templates.zig");
    _ = @import("lexer_snapshot.zig");
    _ = @import("expr_precedence.zig");
    _ = @import("parser_deal_snapshot.zig");
    _ = @import("parser_deal_coverage.zig");
    _ = @import("parser_behavioral.zig");
    _ = @import("parser_dealx_smoke.zig");
    _ = @import("parser_dealx_connect_via.zig");
    _ = @import("parser_dealx_snapshot.zig");
    _ = @import("parser_dealx_tag_balance.zig");
    _ = @import("parser_dealx_coverage.zig");
    _ = @import("recovery_statement.zig");
    _ = @import("recovery_definition.zig");
    _ = @import("recovery_dealx_tag.zig");
    _ = @import("recovery_corpus.zig");
    _ = @import("strictness_m08_m18_m19.zig");
    _ = @import("parser_deal_strictness.zig");
    _ = @import("property_span_containment.zig");
    _ = @import("determinism_parse_twice.zig");
    _ = @import("diag_json_roundtrip.zig");
    _ = @import("c_abi_invalid_utf8.zig");
    _ = @import("c_abi_source_too_large.zig");
    _ = @import("c_abi_no_leaks.zig");
    _ = @import("sema_corpus.zig");
    _ = @import("sema_dimensional.zig");
    _ = @import("sema_calc.zig");
    _ = @import("sema_behavioral.zig");
    _ = @import("determinism_lower_twice.zig");
    _ = @import("property_ir_id_uniqueness.zig");
    _ = @import("fmt_roundtrip.zig");
}
