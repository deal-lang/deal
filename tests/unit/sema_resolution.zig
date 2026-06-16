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
