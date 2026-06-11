//! Byte-offset → 1-based (line, column) translation (D-15).
//!
//! Lazy line-start table: built on the first `lineCol` call and cached on
//! the struct. Subsequent calls binary-search the cached table.
//!
//! Why lazy: Phase 1 only needs line/col when emitting diagnostics. Files
//! that parse cleanly never pay the scan cost. The hot path of the lexer
//! never touches this module.
//!
//! Anti-patterns avoided (RESEARCH lines 522-524):
//!   A2 — `ArrayList(u32) = .empty` allocator-explicit; pass the allocator
//!         on every call.

const std = @import("std");

pub const SourceMap = struct {
    source: []const u8,
    /// `null` until `lineCol` is called at least once; then a slice of
    /// byte offsets where each line begins. The first entry is always 0
    /// (line 1 begins at byte 0).
    line_starts: ?[]const u32 = null,

    pub fn init(source: []const u8) SourceMap {
        return .{ .source = source };
    }

    /// Translate `offset` (a byte index into `source`) into 1-based
    /// `(line, col)`. Builds the line-start table on first call using
    /// `allocator`. Subsequent calls use the cached table without
    /// allocating.
    ///
    /// `offset` clamped to `source.len` (an offset at EOF is at the
    /// 1-based line/col of the byte just past the end).
    pub fn lineCol(
        self: *SourceMap,
        allocator: std.mem.Allocator,
        offset: u32,
    ) struct { line: u32, col: u32 } {
        if (self.line_starts == null) {
            self.buildLineStarts(allocator) catch {
                // Allocator failure → degrade to 1:1 rather than panic.
                // The caller still gets a defined result; renderers
                // proceed without crashing.
                //
                // WR-04: clamp before incrementing so a u32::MAX offset
                // (theoretically reachable from a span.end constructed at
                // source.len = maxInt(u32) in the V12 / E0001 paths) does
                // not overflow the `+ 1`. The function is on a degraded-OOM
                // path so producing col = maxInt(u32) is acceptable
                // degradation; the goal is to avoid the runtime overflow
                // panic in debug builds.
                const safe_col: u32 = if (offset == std.math.maxInt(u32))
                    std.math.maxInt(u32)
                else
                    offset + 1;
                return .{ .line = 1, .col = safe_col };
            };
        }
        const starts = self.line_starts.?;
        const clamped: u32 = @min(offset, @as(u32, @intCast(self.source.len)));

        // Binary search for the largest entry ≤ clamped. Hand-rolled
        // (std.sort.upperBound returns the first entry > target, which
        // we'd then decrement; we keep the loop visible since we need
        // `index` for the line number anyway).
        var lo: usize = 0;
        var hi: usize = starts.len;
        while (lo + 1 < hi) {
            const mid = (lo + hi) / 2;
            if (starts[mid] <= clamped) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        return .{
            .line = @intCast(lo + 1),
            .col = clamped - starts[lo] + 1,
        };
    }

    fn buildLineStarts(self: *SourceMap, allocator: std.mem.Allocator) !void {
        var list: std.ArrayList(u32) = .empty;
        errdefer list.deinit(allocator);

        // Line 1 begins at byte 0.
        try list.append(allocator, 0);

        var i: u32 = 0;
        while (i < self.source.len) : (i += 1) {
            if (self.source[i] == '\n') {
                // The NEXT line begins at the byte after the `\n`.
                try list.append(allocator, i + 1);
            }
        }

        self.line_starts = try list.toOwnedSlice(allocator);
    }
};

test "source_map.line_col single line" {
    const gpa = std.testing.allocator;
    var sm = SourceMap.init("hello");
    defer if (sm.line_starts) |s| gpa.free(s);

    const lc = sm.lineCol(gpa, 3);
    try std.testing.expectEqual(@as(u32, 1), lc.line);
    try std.testing.expectEqual(@as(u32, 4), lc.col);
}

test "source_map.line_col multiline" {
    const gpa = std.testing.allocator;
    //                0    5     11
    //                ↓    ↓      ↓
    //               "ab\ncde\nfghij"
    var sm = SourceMap.init("ab\ncde\nfghij");
    defer if (sm.line_starts) |s| gpa.free(s);

    // Offset 0 → line 1, col 1
    {
        const lc = sm.lineCol(gpa, 0);
        try std.testing.expectEqual(@as(u32, 1), lc.line);
        try std.testing.expectEqual(@as(u32, 1), lc.col);
    }
    // Offset 3 → first byte after first \n → line 2, col 1
    {
        const lc = sm.lineCol(gpa, 3);
        try std.testing.expectEqual(@as(u32, 2), lc.line);
        try std.testing.expectEqual(@as(u32, 1), lc.col);
    }
    // Offset 7 → "f" → line 3, col 1
    {
        const lc = sm.lineCol(gpa, 7);
        try std.testing.expectEqual(@as(u32, 3), lc.line);
        try std.testing.expectEqual(@as(u32, 1), lc.col);
    }
    // Offset 10 → "i" → line 3, col 4
    {
        const lc = sm.lineCol(gpa, 10);
        try std.testing.expectEqual(@as(u32, 3), lc.line);
        try std.testing.expectEqual(@as(u32, 4), lc.col);
    }
}
