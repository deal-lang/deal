//! DEAL Zig simulation runner — in-process call + evidence artifact serialization.
//!
//! D-73: One execution, two outputs. The Rust orchestrator calls Zig sims
//! in-process (via the C ABI sim_run export) for immediate CLI results, then
//! this module serializes the same run's input/output/metadata to schema-valid
//! evidence artifacts under .deal/evidence/<name>/.
//!
//! Phase 5 Plan 04.

const std = @import("std");
const deal_sim = @import("deal_sim.zig");

/// Result of a Zig sim run — carries the output JSON and the evidence artifact bytes.
pub const SimRunResult = struct {
    /// Output JSON bytes (spec/sims/v0/ output.json format).
    output_json: []const u8,
    /// Metadata JSON bytes (spec/sims/v0/ metadata.json format).
    metadata_json: []const u8,
};

/// Run a Zig simulation in-process and serialize evidence artifacts.
///
/// D-73: "One execution, two outputs." This function:
///   1. Calls DealSimulation(T).run_with_arena (in-process).
///   2. Captures the output JSON.
///   3. Serializes a metadata envelope recording the declared @reproducibility
///      tier, sim name, duration, and timestamp (D-75).
///   4. Optionally writes the artifacts to .deal/evidence/<sim_name>/ (D-73
///      second output) when evidence_dir is non-null.
///
/// Parameters:
///   T             — The model type (same T as DealSimulation(T)).
///   alloc         — Arena allocator; output bytes live in this allocator.
///   sim_name      — Name used for the evidence artifact subdirectory.
///   input_json    — Input JSON bytes (spec/sims/v0/ or bare flat dict).
///   evidence_dir  — If non-null, write output.json + metadata.json here.
///
/// Returns SimRunResult with arena-allocated bytes; lifetime matches alloc.
pub fn run_zig_sim(
    comptime T: type,
    alloc: std.mem.Allocator,
    sim_name: []const u8,
    input_json: []const u8,
    evidence_dir: ?[]const u8,
) !SimRunResult {
    const Sim = deal_sim.DealSimulation(T);

    // ── Step 1: In-process execution ─────────────────────────────────────────
    const t_start = std.time.milliTimestamp();

    var out_ptr: [*]const u8 = undefined;
    var out_len: usize = 0;
    const ok = Sim.run_with_arena(alloc, input_json, &out_ptr, &out_len);
    if (!ok) return error.SimRunFailed;

    const t_end = std.time.milliTimestamp();
    const duration_ms: i64 = t_end - t_start;

    const output_json_bytes = out_ptr[0..out_len];

    // ── Step 2: Build metadata JSON ───────────────────────────────────────────
    // Fields alphabetical per D-18: deal_sim_protocol, deal_sim_version,
    // duration_s, reproducibility_tier, sim_name, timestamp, tool, v.
    const tier_name = Sim.reproducibilityTierName();

    var meta_buf = std.ArrayList(u8).init(alloc);
    const mw = meta_buf.writer();

    // Minimal ISO 8601 timestamp (UTC) — compile-time constant matches D-18 determinism.
    // In production the Rust orchestrator overwrites this with the real timestamp;
    // the Zig layer records a placeholder that is replaced by the evidence capture step.
    const timestamp_placeholder = "1970-01-01T00:00:00Z";

    try mw.print(
        "{{\"deal_sim_protocol\":\"v0\",\"deal_sim_version\":\"0.1.0\"," ++
            "\"duration_s\":{d:.6},\"reproducibility_tier\":\"{s}\"," ++
            "\"sim_name\":\"{s}\",\"timestamp\":\"{s}\",\"tool\":\"zig\",\"v\":1}}",
        .{
            @as(f64, @floatFromInt(duration_ms)) / 1000.0,
            tier_name,
            sim_name,
            timestamp_placeholder,
        },
    );

    const metadata_json_bytes = meta_buf.items;

    // ── Step 3: Write evidence artifacts (D-73 second output) ────────────────
    if (evidence_dir) |dir_path| {
        // Create .deal/evidence/<sim_name>/ if it does not exist.
        var dir = try std.fs.cwd().makeOpenPath(dir_path, .{});
        defer dir.close();

        // Write output.json.
        const out_file = try dir.createFile("output.json", .{});
        defer out_file.close();
        try out_file.writeAll(output_json_bytes);

        // Write metadata.json.
        const meta_file = try dir.createFile("metadata.json", .{});
        defer meta_file.close();
        try meta_file.writeAll(metadata_json_bytes);
    }

    return .{
        .output_json = output_json_bytes,
        .metadata_json = metadata_json_bytes,
    };
}

test "sim.zig_runner.run_produces_output_and_metadata" {
    // Verify that run_zig_sim returns output_json and metadata_json with
    // the expected protocol fields. Does NOT write to disk (evidence_dir = null).
    const RangeModel = @import("range_model.zig").RangeModel;
    const alloc = std.testing.allocator;

    var arena = std.heap.ArenaAllocator.init(alloc);
    defer arena.deinit();

    const input_json =
        \\{"deal_sim_protocol":"v0","inputs":{"battery_capacity_kwh":85.0,"rolling_resistance":0.01,"vehicle_mass_kg":2100.0}}
    ;

    const result = try run_zig_sim(
        RangeModel,
        arena.allocator(),
        "range_model",
        input_json,
        null, // no disk write
    );

    // Output JSON must contain protocol field and outputs.
    try std.testing.expect(std.mem.indexOf(u8, result.output_json, "deal_sim_protocol") != null);
    try std.testing.expect(std.mem.indexOf(u8, result.output_json, "exit_code") != null);

    // Metadata JSON must contain required fields.
    try std.testing.expect(std.mem.indexOf(u8, result.metadata_json, "deal_sim_protocol") != null);
    try std.testing.expect(std.mem.indexOf(u8, result.metadata_json, "reproducibility_tier") != null);
    try std.testing.expect(std.mem.indexOf(u8, result.metadata_json, "zig") != null);
}
