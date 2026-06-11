//! Keyword table — Plan 02 implementation.
//!
//! Two comptime perfect-hash tables (D-05, RESEARCH §Pattern 2):
//!
//!   1. `global_keywords` — every spelling that the LEXER promotes from
//!      `.ident` to a `kw_*` tag. 85 entries, locked at planning time
//!      against `spec/grammar/lexical.ebnf` §5 `_Keyword` production
//!      (lines 459-481). The line-749 summary comment that says "76" is a
//!      stale doc-string carried over from an earlier revision of the
//!      grammar; the 85-count from the production body is authoritative
//!      and is being corrected in a separate spec PR.
//!
//!   2. `dealx_tag_names` — every composition-tag spelling that the
//!      `.dealx` parser recognizes as a structural tag head (Plan 04
//!      consumer; the lexer does not use this map). Exported here so the
//!      parser can validate tag names with a comptime hash lookup instead
//!      of an `if/else` chain.
//!
//! Anti-patterns avoided (RESEARCH lines 522-526):
//!   A2 — `std.StaticStringMap(V).initComptime(.{})` only; no
//!         `std.ComptimeStringMap` (removed in 0.16.0).

const std = @import("std");
const lexer = @import("lexer");

/// 88 reserved/contextual keyword spellings from `lexical.ebnf` §5.
/// Order mirrors the production for readability — the per-category sums
/// total exactly 88 (17+10+3+3+4+10+8+7+11+4+9+2). Each entry is
/// byte-equality matched (RESEARCH §T-locale): the lexer scans an IDENT
/// slice, then `global_keywords.get(slice)` returns the keyword tag if
/// present, else the slice stays an `.ident`.
/// Phase 05.2: added calc (Element, 16→17), return+require (new Calc/constraint
/// body section, +2). `sig` is intentionally absent — D-07 forbids global
/// reservation; `sig` stays `.ident` and is recognised contextually by the
/// parser after `=>` in return contracts.
pub const global_keywords = std.StaticStringMap(lexer.Tag).initComptime(.{
    // Element keywords (17) — SD-1, SD-18, SD-21
    .{ "part", .kw_part },
    .{ "port", .kw_port },
    .{ "action", .kw_action },
    .{ "state", .kw_state },
    .{ "attribute", .kw_attribute },
    .{ "item", .kw_item },
    .{ "interface", .kw_interface },
    .{ "connection", .kw_connection },
    .{ "flow", .kw_flow },
    .{ "allocation", .kw_allocation },
    .{ "requirement", .kw_requirement },
    .{ "constraint", .kw_constraint },
    .{ "calc", .kw_calc },  // NEW — SD-21
    .{ "need", .kw_need },
    .{ "def", .kw_def },
    .{ "use", .kw_use },
    .{ "case", .kw_case },

    // Modifier keywords (10) — SD-7
    .{ "abstract", .kw_abstract },
    .{ "derived", .kw_derived },
    .{ "readonly", .kw_readonly },
    .{ "ordered", .kw_ordered },
    .{ "nonunique", .kw_nonunique },
    .{ "individual", .kw_individual },
    .{ "variation", .kw_variation },
    .{ "portion", .kw_portion },
    .{ "end", .kw_end },
    .{ "ref", .kw_ref },

    // Direction keywords (3) — SD-8
    .{ "in", .kw_in },
    .{ "out", .kw_out },
    .{ "inout", .kw_inout },

    // Visibility keywords (3) — SD-9
    .{ "public", .kw_public },
    .{ "protected", .kw_protected },
    .{ "private", .kw_private },

    // Module keywords (4) — PS-2, PS-3, PS-4
    .{ "package", .kw_package },
    .{ "import", .kw_import },
    .{ "export", .kw_export },
    .{ "as", .kw_as },

    // Requirement / use-case body keywords (10) — SD-18, SD-20
    .{ "actor", .kw_actor },
    .{ "subject", .kw_subject },
    .{ "precondition", .kw_precondition },
    .{ "postcondition", .kw_postcondition },
    .{ "verification", .kw_verification },
    .{ "accepts", .kw_accepts },
    .{ "rejects", .kw_rejects },
    .{ "threshold", .kw_threshold },
    .{ "operator", .kw_operator },
    .{ "conditions", .kw_conditions },

    // Composition tag names (8) — contextual in `.dealx` `[< >]` tags
    .{ "system", .kw_system },
    .{ "subsystem", .kw_subsystem },
    .{ "connect", .kw_connect },
    .{ "expose", .kw_expose },
    .{ "traceability", .kw_traceability },
    .{ "satisfy", .kw_satisfy },
    .{ "validate", .kw_validate },
    .{ "allocate", .kw_allocate },

    // Composition attribute names (7) — contextual in tag attribute heads
    .{ "from", .kw_from },
    .{ "to", .kw_to },
    .{ "via", .kw_via },
    .{ "carrying", .kw_carrying },
    .{ "method", .kw_method },
    .{ "status", .kw_status },
    .{ "relationship", .kw_relationship },

    // Evidence / criteria names (11) — contextual in `[<satisfy>]` bodies
    .{ "criteria", .kw_criteria },
    .{ "evidence", .kw_evidence },
    .{ "compute", .kw_compute },
    .{ "maps", .kw_maps },
    .{ "gap", .kw_gap },
    .{ "simulation", .kw_simulation },
    .{ "test", .kw_test },
    .{ "analysis", .kw_analysis },
    .{ "design", .kw_design },
    .{ "inspection", .kw_inspection },
    .{ "demonstration", .kw_demonstration },

    // Logical operators (4) — in criteria expressions
    .{ "AND", .kw_AND },
    .{ "OR", .kw_OR },
    .{ "NOT", .kw_NOT },
    .{ "WHERE", .kw_WHERE },

    // @header field keywords (9) — FS-2, contextual in `@header { ... }`
    .{ "path", .kw_path },
    .{ "schema", .kw_schema },
    .{ "created", .kw_created },
    .{ "modified", .kw_modified },
    .{ "reviewed", .kw_reviewed },
    .{ "hash", .kw_hash },
    .{ "baseline", .kw_baseline },
    .{ "marking", .kw_marking },
    .{ "by", .kw_by },

    // Calc / constraint body keywords (2) — SD-21/22 (Phase 05.2)
    // NOTE: `sig` is intentionally absent — D-07 forbids global reservation.
    .{ "return", .kw_return },   // NEW — calc result statement
    .{ "require", .kw_require }, // NEW — constraint invariant
});

/// Composition tag-name semantic classifier (Plan 04 consumer).
///
/// Plan 04's `parser_dealx.zig` will validate that a tag head identifier
/// names a structural composition kind. Using a comptime perfect-hash here
/// avoids a long `if/else` chain at the parser-frame entry to each
/// `parseSystemBlock` / `parseSubsystemBlock` / `parseConnect` etc.
///
/// Note: tag-name keywords are ALSO in `global_keywords` (so the lexer
/// emits e.g. `kw_system` instead of `ident`); this map covers the
/// semantic-tag dimension only. Plan 02 declares it but does not exercise
/// it — the body of every `parser_dealx` call site lands in Plan 04.
pub const TagSemantic = enum {
    system,
    subsystem,
    connect,
    expose,
    traceability,
    satisfy,
    validate,
    allocate,
};

pub const dealx_tag_names = std.StaticStringMap(TagSemantic).initComptime(.{
    .{ "system", .system },
    .{ "subsystem", .subsystem },
    .{ "connect", .connect },
    .{ "expose", .expose },
    .{ "traceability", .traceability },
    .{ "satisfy", .satisfy },
    .{ "validate", .validate },
    .{ "allocate", .allocate },
});

comptime {
    // Force the maps to be evaluated at comptime so any malformed entry
    // (e.g. duplicate key, missing Tag variant) is a build-time error
    // rather than a silent runtime fallback.
    _ = global_keywords;
    _ = dealx_tag_names;
}
