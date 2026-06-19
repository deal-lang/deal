//! parser_import_placement — ADR-0004 P2 (R5) import placement.
//!
//! WS-B: a `.deal` definition body may contain a nested `import`, which the
//! parser places as a body member (scoped binding is deferred to P3).
//! WS-C: a `.dealx` import after composition content is a placement error
//! (`E0305`); top-of-file imports are accepted.
//!
//! These assert the *syntax-layer* behavior only — P2 changes no name
//! resolution. The nested import is parsed and placed; it is not yet bound.

const std = @import("std");
const ast = @import("ast");
const parser = @import("parser");
const diagnostics = @import("diagnostics");

fn hasCode(diags: *const std.ArrayList(diagnostics.Diagnostic), code: []const u8) bool {
    for (diags.items) |d| {
        if (std.mem.eql(u8, d.code, code)) return true;
    }
    return false;
}

test "P2 R5: a nested import in a .deal definition body is parsed as a body member" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const al = arena.allocator();
    const src =
        \\package test;
        \\part def Rover {
        \\    import internal.telemetry.{Beacon};
        \\    attribute mass : Real;
        \\}
    ;
    var diags: std.ArrayList(diagnostics.Diagnostic) = .empty;
    const root = (try parser.parseFile(al, src, .deal, &diags)).?;
    const df = root.payload.deal_file;

    // The only import is nested in the body — none at file level.
    try std.testing.expectEqual(@as(usize, 0), df.imports.len);
    // Placement parses cleanly: no error diagnostics.
    for (diags.items) |d| try std.testing.expect(d.severity != .err);

    // The part def's body contains the import_decl as a member.
    var found_nested_import = false;
    for (df.definitions) |def| {
        if (def.payload == .part_def) {
            for (def.payload.part_def.members) |m| {
                if (m.payload == .import_decl) found_nested_import = true;
            }
        }
    }
    try std.testing.expect(found_nested_import);
}

test "P2 R5: a .dealx import after composition content emits E0305" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const al = arena.allocator();
    const src =
        \\package model;
        \\import spacecraft.*;
        \\[<system Vehicle>]
        \\  [<subsystem Body>][</subsystem>]
        \\[</system>]
        \\import reqs.{MassBudget};
    ;
    var diags: std.ArrayList(diagnostics.Diagnostic) = .empty;
    _ = try parser.parseFile(al, src, .dealx, &diags);
    try std.testing.expect(hasCode(&diags, "E0305"));
}

test "P2 R5: top-of-file .dealx imports do not emit E0305" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const al = arena.allocator();
    const src =
        \\package model;
        \\import spacecraft.*;
        \\import reqs.{MassBudget};
        \\[<system Vehicle>]
        \\  [<subsystem Body>][</subsystem>]
        \\[</system>]
    ;
    var diags: std.ArrayList(diagnostics.Diagnostic) = .empty;
    _ = try parser.parseFile(al, src, .dealx, &diags);
    try std.testing.expect(!hasCode(&diags, "E0305"));
}
