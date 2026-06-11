//! gen-malformed — corpus generator for Plan 05's recovery.corpus test.
//!
//! Walks `tests/showcase/` for `.deal` / `.dealx` files and applies five
//! mutation strategies to each (drop-token, swap-token, truncate,
//! inject-garbage, unmatched-bracket). Writes one mutated file per
//! strategy to `tests/malformed/gen_<strategy>_<base>_<seq>.deal[x]`
//! until at least 30 files have been produced.
//!
//! Uses a fixed PRNG seed (0xDEA1_2026 — "DEAL 2026" with L→1) so
//! identical mutations are produced on every run (T-05-06 mitigation).
//!
//! Also writes a manifest `tests/malformed/_manifest.txt` recording the
//! base file and mutation type for each generated file.

const std = @import("std");

const SEED: u64 = 0xDEA1_2026;
const SHOWCASE_DIR = "tests/showcase";
const MALFORMED_DIR = "tests/malformed";
const TARGET_COUNT: usize = 30;

const Strategy = enum {
    drop_byte,
    swap_pair,
    truncate,
    inject_garbage,
    unmatched_bracket,

    fn name(self: Strategy) []const u8 {
        return switch (self) {
            .drop_byte => "drop_byte",
            .swap_pair => "swap_pair",
            .truncate => "truncate",
            .inject_garbage => "inject_garbage",
            .unmatched_bracket => "unmatched_bracket",
        };
    }
};

pub fn main() !void {
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena.deinit();
    const alloc = arena.allocator();

    // Zig 0.16: file I/O routes through std.Io.Dir + an Io instance.
    // Threaded(.{}) is the standard impl; deinit'd at program exit.
    var io_instance: std.Io.Threaded = .init(alloc, .{});
    defer io_instance.deinit();
    const io = io_instance.io();
    const cwd = std.Io.Dir.cwd();

    var prng = std.Random.DefaultPrng.init(SEED);
    const rng = prng.random();

    // Collect showcase files.
    var showcase_files: std.ArrayList([]const u8) = .empty;
    try collectShowcaseFiles(alloc, io, cwd, SHOWCASE_DIR, &showcase_files);

    if (showcase_files.items.len == 0) {
        std.debug.print("gen-malformed: no .deal / .dealx files found under {s}\n", .{SHOWCASE_DIR});
        return;
    }

    std.debug.print("gen-malformed: found {d} showcase files\n", .{showcase_files.items.len});

    var manifest: std.ArrayList(u8) = .empty;
    var produced: usize = 0;

    // Per-strategy round-robin until TARGET_COUNT reached.
    const strategies = [_]Strategy{
        .drop_byte, .swap_pair, .truncate, .inject_garbage, .unmatched_bracket,
    };

    var strategy_idx: usize = 0;
    var file_idx: usize = 0;
    while (produced < TARGET_COUNT) {
        const strat = strategies[strategy_idx % strategies.len];
        const src_path = showcase_files.items[file_idx % showcase_files.items.len];

        // WR-09: treat file-read errors as hard errors so the produced corpus
        // is deterministic. The previous `continue` path silently shrank the
        // corpus when any read failed, breaking the T-05-06 reproducibility
        // guarantee (same seed must produce same files) — a transient I/O
        // error would silently change the malformed corpus on the next run.
        const src_bytes = cwd.readFileAlloc(io, src_path, alloc, .unlimited) catch |err| {
            std.debug.print("gen-malformed: read {s} failed ({s})\n", .{ src_path, @errorName(err) });
            return err;
        };

        const mutated = try mutate(alloc, src_bytes, strat, rng);

        // Build output filename: gen_<strategy>_<basename>_<seq>.deal[x]
        const ext = if (std.mem.endsWith(u8, src_path, ".dealx")) ".dealx" else ".deal";
        const base_with_ext = std.fs.path.basename(src_path);
        const base = base_with_ext[0 .. base_with_ext.len - ext.len];
        const out_name = try std.fmt.allocPrint(
            alloc,
            MALFORMED_DIR ++ "/gen_{s}_{s}_{d:0>2}{s}",
            .{ strat.name(), base, produced, ext },
        );
        try cwd.writeFile(io, .{ .sub_path = out_name, .data = mutated });

        try manifest.appendSlice(alloc, out_name);
        try manifest.appendSlice(alloc, " <- ");
        try manifest.appendSlice(alloc, src_path);
        try manifest.appendSlice(alloc, " (");
        try manifest.appendSlice(alloc, strat.name());
        try manifest.appendSlice(alloc, ")\n");

        produced += 1;
        strategy_idx += 1;
        file_idx += 1;
    }

    try cwd.writeFile(io, .{
        .sub_path = MALFORMED_DIR ++ "/_manifest.txt",
        .data = manifest.items,
    });

    std.debug.print("gen-malformed: produced {d} files; manifest at {s}/_manifest.txt\n", .{
        produced, MALFORMED_DIR,
    });
}

fn collectShowcaseFiles(
    alloc: std.mem.Allocator,
    io: std.Io,
    cwd: std.Io.Dir,
    dir_path: []const u8,
    out: *std.ArrayList([]const u8),
) !void {
    var dir = cwd.openDir(io, dir_path, .{ .iterate = true }) catch return;
    defer dir.close(io);
    var walker = try dir.walk(alloc);
    defer walker.deinit();
    while (try walker.next(io)) |entry| {
        if (entry.kind != .file) continue;
        const name = entry.basename;
        if (std.mem.endsWith(u8, name, ".deal") or std.mem.endsWith(u8, name, ".dealx")) {
            const full = try std.fmt.allocPrint(alloc, "{s}/{s}", .{ dir_path, entry.path });
            try out.append(alloc, full);
        }
    }
}

fn mutate(
    alloc: std.mem.Allocator,
    src: []const u8,
    strat: Strategy,
    rng: std.Random,
) ![]u8 {
    if (src.len < 4) {
        var buf = try alloc.alloc(u8, src.len + 4);
        @memcpy(buf[0..src.len], src);
        @memcpy(buf[src.len..], "@@@@");
        return buf;
    }
    return switch (strat) {
        .drop_byte => mutDropByte(alloc, src, rng),
        .swap_pair => mutSwapPair(alloc, src, rng),
        .truncate => mutTruncate(alloc, src, rng),
        .inject_garbage => mutInjectGarbage(alloc, src, rng),
        .unmatched_bracket => mutUnmatchedBracket(alloc, src, rng),
    };
}

fn mutDropByte(alloc: std.mem.Allocator, src: []const u8, rng: std.Random) ![]u8 {
    var targets: std.ArrayList(usize) = .empty;
    defer targets.deinit(alloc);
    for (src, 0..) |b, i| {
        if (b == ';' or b == '}' or b == ')') try targets.append(alloc, i);
    }
    const drop_pos: usize = if (targets.items.len > 0)
        targets.items[rng.uintLessThan(usize, targets.items.len)]
    else
        rng.uintLessThan(usize, src.len);

    var buf = try alloc.alloc(u8, src.len - 1);
    @memcpy(buf[0..drop_pos], src[0..drop_pos]);
    @memcpy(buf[drop_pos..], src[drop_pos + 1 ..]);
    return buf;
}

fn mutSwapPair(alloc: std.mem.Allocator, src: []const u8, rng: std.Random) ![]u8 {
    var buf = try alloc.dupe(u8, src);
    if (buf.len < 2) return buf;
    const i = rng.uintLessThan(usize, buf.len - 1);
    const tmp = buf[i];
    buf[i] = buf[i + 1];
    buf[i + 1] = tmp;
    return buf;
}

fn mutTruncate(alloc: std.mem.Allocator, src: []const u8, rng: std.Random) ![]u8 {
    const min_cut = (src.len * 60) / 100;
    const max_cut = (src.len * 95) / 100;
    const range = max_cut - min_cut;
    const cut: usize = if (range == 0) min_cut else min_cut + rng.uintLessThan(usize, range);
    return alloc.dupe(u8, src[0..cut]);
}

fn mutInjectGarbage(alloc: std.mem.Allocator, src: []const u8, rng: std.Random) ![]u8 {
    const garbage = "@@@&&&!!!";
    var insert_pos: usize = src.len / 2;
    for (src, 0..) |b, i| {
        if (b == '\n' and i > src.len / 4 and i < (src.len * 3) / 4) {
            if (rng.boolean()) {
                insert_pos = i + 1;
                break;
            }
        }
    }
    if (insert_pos > src.len) insert_pos = src.len;
    var buf = try alloc.alloc(u8, src.len + garbage.len);
    @memcpy(buf[0..insert_pos], src[0..insert_pos]);
    @memcpy(buf[insert_pos .. insert_pos + garbage.len], garbage);
    @memcpy(buf[insert_pos + garbage.len ..], src[insert_pos..]);
    return buf;
}

fn mutUnmatchedBracket(alloc: std.mem.Allocator, src: []const u8, rng: std.Random) ![]u8 {
    _ = rng;
    var last_brace: ?usize = null;
    for (src, 0..) |b, i| {
        if (b == '}') last_brace = i;
    }
    if (last_brace) |pos| {
        var buf = try alloc.alloc(u8, src.len - 1);
        @memcpy(buf[0..pos], src[0..pos]);
        @memcpy(buf[pos..], src[pos + 1 ..]);
        return buf;
    }
    return alloc.dupe(u8, src);
}
