//! ADR-0003 cross-file name resolution — end-to-end binding capture.
//!
//! Proves the workspace-merged resolver (P2 WS-A): a `<<specializes>>` target
//! imported from a package PREFIX (`import interfaces.{ThermallyManaged}`) binds
//! to the declaration that actually lives in a sub-package
//! (`interfaces.thermal.ThermallyManaged`), and is recorded as a reference
//! binding carrying that canonical fully-qualified id — exactly the key the LSP
//! reverse index uses. Mirrors the showcase's loose-import shape.

const std = @import("std");
const lib = @import("lib");

test "ADR-0003: cross-file specializes binds to declaring FQ id (prefix-subtree)" {
    const gpa = std.testing.allocator;

    // Declares ThermallyManaged in package `interfaces.thermal`.
    const interfaces_src =
        \\package interfaces.thermal;
        \\interface def ThermallyManaged { }
    ;
    // Imports it via the PREFIX package `interfaces` (not `interfaces.thermal`)
    // and specializes it — the showcase's exact pattern.
    const battery_src =
        \\package vehicle.battery;
        \\import interfaces.{ThermallyManaged};
        \\part def BatteryPack <<specializes>> ThermallyManaged { }
    ;

    const ws = [_]lib.StdlibSource{
        .{ .source = interfaces_src, .filename = "interfaces/thermal.deal" },
    };
    const handle = try lib.deal_parse_internal_with_stdlib(
        gpa,
        battery_src,
        "vehicle/battery.deal",
        &ws,
        true, // ADR-0003: all declarations in the external table for resolution
    );
    defer lib.deal_free_internal(handle);

    const table = handle.index_root orelse return error.TestUnexpectedResult;

    var found = false;
    for (table.bindings.items) |b| {
        if (std.mem.eql(u8, b.ref_kind, "specializes") and
            std.mem.eql(u8, b.resolved_path, "interfaces.thermal.ThermallyManaged"))
        {
            found = true;
            // P2 WS-C0: the recorded span must be the bare terminal name token
            // (NOT including the `<<specializes>>` operator) so rename is precise.
            const slice = battery_src[@as(usize, b.from_span.start)..@as(usize, b.from_span.end)];
            try std.testing.expectEqualStrings("ThermallyManaged", slice);
        }
    }
    try std.testing.expect(found);
}

test "ADR-0003: cross-file type reference binds (type_ref)" {
    const gpa = std.testing.allocator;

    const lib_src =
        \\package lib.parts;
        \\part def Cell { }
    ;
    // Consumer imports Cell via the prefix package `lib` and uses it as a type.
    const consumer_src =
        \\package vehicle.battery;
        \\import lib.{Cell};
        \\part def Pack {
        \\  part cell : Cell;
        \\}
    ;

    const ws = [_]lib.StdlibSource{
        .{ .source = lib_src, .filename = "lib/parts.deal" },
    };
    const handle = try lib.deal_parse_internal_with_stdlib(
        gpa,
        consumer_src,
        "vehicle/battery.deal",
        &ws,
        true,
    );
    defer lib.deal_free_internal(handle);

    const table = handle.index_root orelse return error.TestUnexpectedResult;
    var found = false;
    for (table.bindings.items) |b| {
        if (std.mem.eql(u8, b.ref_kind, "type_ref") and
            std.mem.eql(u8, b.resolved_path, "lib.parts.Cell"))
        {
            found = true;
            // Precise span: the type-name token only (not `part cell : ` etc.).
            const slice = consumer_src[@as(usize, b.from_span.start)..@as(usize, b.from_span.end)];
            try std.testing.expectEqualStrings("Cell", slice);
        }
    }
    try std.testing.expect(found);
}

test "ADR-0003: built-in type reference records no binding" {
    const gpa = std.testing.allocator;
    const src =
        \\package p;
        \\part def Thing {
        \\  attribute mass : Real;
        \\}
    ;
    const handle = try lib.deal_parse_internal_with_stdlib(
        gpa,
        src,
        "p.deal",
        &[_]lib.StdlibSource{},
        true,
    );
    defer lib.deal_free_internal(handle);

    const table = handle.index_root orelse return error.TestUnexpectedResult;
    // `Real` is a built-in — no in-workspace declaration, so no type_ref binding.
    for (table.bindings.items) |b| {
        try std.testing.expect(!std.mem.eql(u8, b.resolved_path, "Real"));
    }
}

test "P2 WS-C: named import item binds to declaring FQ id (import ref-kind)" {
    const gpa = std.testing.allocator;

    const interfaces_src =
        \\package interfaces.thermal;
        \\interface def ThermallyManaged { }
    ;
    const battery_src =
        \\package vehicle.battery;
        \\import interfaces.{ThermallyManaged};
        \\part def BatteryPack <<specializes>> ThermallyManaged { }
    ;

    const ws = [_]lib.StdlibSource{
        .{ .source = interfaces_src, .filename = "interfaces/thermal.deal" },
    };
    const handle = try lib.deal_parse_internal_with_stdlib(
        gpa,
        battery_src,
        "vehicle/battery.deal",
        &ws,
        true,
    );
    defer lib.deal_free_internal(handle);

    const table = handle.index_root orelse return error.TestUnexpectedResult;
    var found = false;
    for (table.bindings.items) |b| {
        if (std.mem.eql(u8, b.ref_kind, "import") and
            std.mem.eql(u8, b.resolved_path, "interfaces.thermal.ThermallyManaged"))
        {
            found = true;
            // The bound span is the bare name token inside `{ … }`, so rename
            // edits the import item without touching the `interfaces.` prefix.
            const slice = battery_src[@as(usize, b.from_span.start)..@as(usize, b.from_span.end)];
            try std.testing.expectEqualStrings("ThermallyManaged", slice);
        }
    }
    try std.testing.expect(found);
}

test "ADR-0003: same-file specializes binds to local FQ id" {
    const gpa = std.testing.allocator;
    const src =
        \\package p;
        \\part def Base { }
        \\part def Derived <<specializes>> Base { }
    ;
    const handle = try lib.deal_parse_internal_with_stdlib(
        gpa,
        src,
        "p.deal",
        &[_]lib.StdlibSource{},
        true,
    );
    defer lib.deal_free_internal(handle);

    const table = handle.index_root orelse return error.TestUnexpectedResult;
    var found = false;
    for (table.bindings.items) |b| {
        if (std.mem.eql(u8, b.ref_kind, "specializes") and
            std.mem.eql(u8, b.resolved_path, "p.Base"))
        {
            found = true;
        }
    }
    try std.testing.expect(found);
}
