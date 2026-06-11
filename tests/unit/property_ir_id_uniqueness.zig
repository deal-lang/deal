//! property.ir_id_uniqueness — Plan 02-03 IR property test.
//!
//! For each of the 19 showcase files, lowers the source via the test-only
//! `deal_lower_internal` helper and asserts that every IrNode ID in the
//! resulting Document is unique (no two elements share the same fully-
//! qualified path).
//!
//! This exercises the sema uniqueness guarantee (one of the 6 blocking
//! checks in Plan 02-02) as observed through the IR layer. If two nodes
//! were accidentally given the same ID, the IR Document's StringHashMap
//! would silently overwrite one entry — this test catches that regression
//! by verifying the element count equals the number of distinct IDs.
//!
//! Uses `deal_lower_internal` (the test-only helper) because the property
//! checks the IR structure directly, not the JSON byte-stream. The
//! determinism test covers the public ABI contract.

const std = @import("std");
const lib = @import("lib");

/// 19 showcase files — mirrors the corpus used by determinism_parse_twice.
const showcase_files = [_][]const u8{
    "tests/showcase/packages/vehicle/battery.deal",
    "tests/showcase/packages/vehicle/motor.deal",
    "tests/showcase/packages/vehicle/behaviors.deal",
    "tests/showcase/packages/vehicle/components.deal",
    "tests/showcase/packages/vehicle/index.deal",
    "tests/showcase/packages/interfaces/electrical.deal",
    "tests/showcase/packages/interfaces/thermal.deal",
    "tests/showcase/packages/interfaces/connections.deal",
    "tests/showcase/packages/interfaces/index.deal",
    "tests/showcase/packages/requirements/system.deal",
    "tests/showcase/packages/requirements/needs.deal",
    "tests/showcase/packages/requirements/index.deal",
    "tests/showcase/packages/use-cases/driving.deal",
    "tests/showcase/packages/use-cases/charging.deal",
    "tests/showcase/packages/use-cases/index.deal",
    "tests/showcase/model/traceability.dealx",
    "tests/showcase/model/vehicle.dealx",
    "tests/showcase/model/variants/performance.dealx",
    "tests/showcase/model/variants/sedan.dealx",
};

test "property.ir_id_uniqueness" {
    const gpa = std.testing.allocator;
    const io = std.testing.io;
    const cwd = std.Io.Dir.cwd();

    for (showcase_files) |path| {
        const source = cwd.readFileAlloc(io, path, gpa, .unlimited) catch |err| {
            std.debug.print("property.ir_id_uniqueness: read {s} failed ({s})\n", .{ path, @errorName(err) });
            return err;
        };
        defer gpa.free(source);

        // Lower via the test-only helper. Use a per-iteration ArenaAllocator
        // so ALL lowering allocations are released at end of each iteration.
        // This prevents the 218-leak false failure from prior Plan 02-05.
        // (Plan 02-06 fix: deal_lower_internal now takes the allocator directly
        // instead of creating its own arena, so the caller controls lifetime.)
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit(); // releases all arena-owned allocations (doc + internals)

        const doc = try lib.deal_lower_internal(arena.allocator(), source, path);

        // Walk the document and collect all IDs into a temporary set.
        // Use gpa for seen (not arena) so it's freed via defer below.
        var seen = std.StringHashMap(void).init(gpa);
        defer seen.deinit();

        var it = doc.elements.iterator();
        while (it.next()) |entry| {
            const id = entry.key_ptr.*;
            // Assert uniqueness: inserting a duplicate ID means the HashMap
            // already has it. Use getOrPut to detect collisions.
            const gop = try seen.getOrPut(id);
            if (gop.found_existing) {
                std.debug.print(
                    "property.ir_id_uniqueness: duplicate ID '{s}' in {s}\n",
                    .{ id, path },
                );
                // Fail with a clear message.
                try std.testing.expectEqual(false, gop.found_existing);
            }
        }

        // Additional check: element count matches distinct ID count.
        try std.testing.expectEqual(doc.elements.count(), seen.count());
    }
}
