//! c_abi.source_too_large — Tests that deal_parse handles source_len overflow
//! correctly (Plan 06 Task 6.2).
//!
//! Security reference: T-06-02 / V12 ASVS — source_len > maxInt(u32) check at
//! the top of deal_parse / deal_parse_internal, BEFORE any buffer access. This
//! guard prevents integer overflow on u32 span fields (D-15).
//!
//! WARNING: Passing a fake source_len risks reading past the actual buffer if the
//! bound check is absent. This test IS the gate that proves the bound check fires
//! BEFORE any buffer read — the tiny real buffer is never scanned because the
//! length-overflow check triggers first.

const std = @import("std");
const lib = @import("lib");
const diagnostics_mod = @import("diagnostics");

test "c_abi.source_too_large" {
    // A real (small) buffer — the content doesn't matter because the length
    // check fires before the buffer is read. We pass a usize value larger than
    // u32 max as source.len to trigger E0004.
    const small_buf = "deal part Foo {}";

    // Use an empty slice — only the length matters for the overflow check.
    const fake_source = small_buf[0..0]; // empty slice; only the length matters

    // Construct a source slice that has the same base pointer as small_buf but
    // a length of maxInt(u32)+1. The bound check must fire before any access
    // to bytes[maxInt(u32)] (which would be out of bounds).
    //
    // In Zig 0.16.0 we can do this safely through the internal API by passing
    // a length parameter rather than a real slice. We use deal_parse (C ABI)
    // with a tiny real buffer but a fake length, mirroring how the C caller
    // would trigger the bug if the check were absent.
    //
    // deal_parse_internal takes a []const u8 whose .len is the real length.
    // To test the overflow path we need to simulate passing source_len >
    // maxInt(u32). The C ABI route is the correct path here.
    const filename = "test.deal";

    // Use the raw C ABI entry point (deal_parse) with a real (small) buffer
    // but a source_len value that exceeds u32 max. The plan guarantees the
    // guard fires BEFORE the buffer is dereferenced.
    const opaque_h = lib.deal_parse(
        fake_source.ptr,
        @as(usize, std.math.maxInt(u32)) + 1,
        filename.ptr,
        filename.len,
    );
    defer lib.deal_free(opaque_h);

    // D-10: handle must be non-null.
    try std.testing.expect(opaque_h != null);

    // Exactly one diagnostic with code E0004.
    try std.testing.expectEqual(@as(u32, 1), lib.deal_diagnostics_count(opaque_h));
    try std.testing.expect(lib.deal_has_errors(opaque_h));

    // Read diagnostics JSON via the ABI and confirm E0004 is present.
    var ptr: [*]const u8 = undefined;
    var len: usize = 0;
    try std.testing.expect(lib.deal_diagnostics_json(opaque_h, &ptr, &len));
    const json_slice = ptr[0..len];
    try std.testing.expect(std.mem.indexOf(u8, json_slice, diagnostics_mod.Codes.e_source_too_large) != null);

    // AST JSON should contain "root":null (parser not invoked).
    var ast_ptr: [*]const u8 = undefined;
    var ast_len: usize = 0;
    try std.testing.expect(lib.deal_ast_json(opaque_h, &ast_ptr, &ast_len));
    const ast_slice = ast_ptr[0..ast_len];
    try std.testing.expect(std.mem.indexOf(u8, ast_slice, "\"root\":null") != null);
}
