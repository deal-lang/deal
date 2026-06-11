//! Parser driver — dispatches on file mode (D-12).
//!
//! Mode selection is filename-driven: `.dealx` suffix → composition parser
//! (Plan 04), otherwise → definition parser (Plan 03). `lib.zig` stores the
//! resolved mode on the handle before calling `parseFile`.
//!
//! Plan 03: Only `.deal` mode is fully implemented. `.dealx` returns null
//! (stub) until Plan 04 ships.

const std = @import("std");
const ast = @import("ast");
const diagnostics = @import("diagnostics");
const parser_deal = @import("parser_deal");
const parser_dealx = @import("parser_dealx");

/// Parse a file given its arena allocator, source bytes, mode, and diagnostics
/// list. Returns the root Node (or null on OOM after appending a diagnostic).
pub fn parseFile(
    arena: std.mem.Allocator,
    source: []const u8,
    mode: ast.Mode,
    diag_list: *std.ArrayList(diagnostics.Diagnostic),
) !?*ast.Node {
    return switch (mode) {
        .deal => parser_deal.parseFile(arena, source, diag_list),
        .dealx => parser_dealx.parseFile(arena, source, diag_list),
    };
}
