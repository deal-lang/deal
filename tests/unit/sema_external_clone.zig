//! ADR-0004 P0 — external-table seed entries must be deep-copied into the
//! analysis arena, so a handle's symbol table survives the external (stdlib)
//! arena's teardown. This is the regression lock for the use-after-free that
//! `deal_check_with_stdlib` → `deal_index_json` hit once the stdlib (which
//! declares dimensions/units) was fed in: the seed loop shared borrowed
//! `*SymbolEntry` pointers, and `writeIndexJson` later read their freed
//! `id`/`source_file` slices.
//!
//! The test reproduces the exact crash surface: a QUALIFIED `unit_def` (so its
//! map key equals `entry.id` and `writeIndexJson` emits it) is seeded from an
//! external table in its own arena; that arena is freed and its pages scribbled;
//! then `writeIndexJson` must still produce the correct id/source_file, and the
//! cloned `dim_meta.unit.dim_name` must still read correctly. A bare-name seed
//! or a bindings-only check would NOT exercise this path and must not be
//! substituted.

const std = @import("std");
const sema = @import("sema");
const json = @import("json");
const diagnostics = @import("diagnostics");

test "P0: seeded external dim/unit survives external-arena teardown (no UAF)" {
    const gpa = std.testing.allocator;

    // Analysis arena — owns the clones. Testing-allocator-backed so a missed
    // clone (dangling into the freed external arena) or a leak is caught.
    var analysis_arena = std.heap.ArenaAllocator.init(gpa);
    defer analysis_arena.deinit();
    const aalloc = analysis_arena.allocator();

    // External (stdlib-like) table in its OWN, separately-freed arena.
    var ext_arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    const ealloc = ext_arena.allocator();

    const ext = try ealloc.create(sema.SymbolTable);
    ext.* = .{
        .entries = std.StringHashMap(*sema.SymbolEntry).init(ealloc),
        .imported_packages = std.StringHashMap(void).init(ealloc),
        .imports_graph = &.{},
        .package_segments = &.{},
        .bindings = .empty,
    };

    // A package-QUALIFIED unit_def (id has a dot), seeded under that same key so
    // writeIndexJson's `key == e.id` filter emits it; with a .unit dim_meta whose
    // dim_name also lives in the external arena.
    const unit = try ealloc.create(sema.SymbolEntry);
    unit.* = .{
        .id = try ealloc.dupe(u8, "u.Volt"),
        .kind = .unit_def,
        .span = .{ .start = 1, .end = 5 },
        .name_span = .{ .start = 1, .end = 5 },
        .source_file = try ealloc.dupe(u8, "u.deal"),
        .dim_meta = .{ .unit = .{
            .dim_name = try ealloc.dupe(u8, "Voltage"),
            .is_conversion = false,
        } },
    };
    try ext.entries.put(try ealloc.dupe(u8, "u.Volt"), unit);

    // Analyze with a null root: the seed loop runs before the `if (root)` guard.
    var diags: std.ArrayList(diagnostics.Diagnostic) = .empty;
    const table = try sema.analyzeWithExternalTable(aalloc, null, &diags, "t.deal", ext);

    // Free the external arena, then scribble over the freed region so a surviving
    // borrow would read corrupted bytes (deterministic, not allocator-dependent).
    ext_arena.deinit();
    {
        var scribble = std.heap.ArenaAllocator.init(std.heap.page_allocator);
        defer scribble.deinit();
        const salloc = scribble.allocator();
        var i: usize = 0;
        while (i < 64) : (i += 1) {
            const buf = try salloc.alloc(u8, 4096);
            @memset(buf, 0xAA);
        }
    }

    // index_json must read only analysis-arena memory.
    const out = try json.writeIndexJson(aalloc, table, "test");
    try std.testing.expect(std.mem.indexOf(u8, out, "u.Volt") != null);
    try std.testing.expect(std.mem.indexOf(u8, out, "u.deal") != null);

    // The dim_meta clone survived the teardown too.
    const seeded = table.entries.get("u.Volt").?;
    try std.testing.expectEqualStrings("Voltage", seeded.dim_meta.?.unit.dim_name);
}
