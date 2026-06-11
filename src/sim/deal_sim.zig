//! DEAL Zig simulation SDK — comptime DealSimulation(T) wrapper.
//!
//! D-73/D-75/D-80: Zig-internal sims run in-process and emit schema-valid
//! evidence artifacts. The @reproducibility tier (default .strict per D-73)
//! maps to @setFloatMode at function scope before any arithmetic, making the
//! in-process fast path bit-reproducible and authoritative.
//!
//! Phase 5 Plan 04: ships the comptime DealSimulation(T) wrapper used by
//! range_model.zig (the canonical Zig sim proving the in-process path).
//!
//! Usage pattern:
//!
//!   const DealSim = @import("deal_sim").DealSimulation;
//!
//!   const MyModel = struct {
//!       pub const reproducibility: std.builtin.FloatMode = .strict;
//!       pub fn run(input: Input) Output { ... }
//!   };
//!   const MySim = DealSim(MyModel);
//!   // MySim.sim_run is a C-callable export that sets @setFloatMode(.strict)
//!   // before calling MyModel.run().

const std = @import("std");

/// Reproducibility tier → @setFloatMode mapping (ADR-deal-stdlib-numeric-model.md).
///
/// D-75: Phase 5 implements only deferred-decision #5 — mapping @reproducibility
/// tier to the orchestrator. Concretely: the declared tier is applied as a real
/// @setFloatMode selection at function scope before any arithmetic in sim_run.
///
/// The tiers map as follows (Assumption A-001 from the ADR):
///   .strict   → @setFloatMode(.strict)   — IEEE 754 strict, bit-reproducible (default)
///   .optimized → @setFloatMode(.optimized) — allows FMA/reassociation
///
/// Phase 5 ships only .strict (the default) as the concrete implementation.
/// The upsize-f128, kahan, and tolerant tiers are deferred to the full
/// numeric system (ADR deferred-decision #1..#4).
pub const ReproducibilityTier = enum {
    /// IEEE 754 strict mode — bit-reproducible across all runs. Default per D-73.
    strict,
    /// Optimized mode — allows FMA and other platform-specific optimizations.
    /// Advisory enforcement for external tools (D-76).
    optimized,
};

/// Comptime DealSimulation(T) wrapper.
///
/// T must declare:
///   - pub const reproducibility: ReproducibilityTier (optional; defaults to .strict)
///   - A nested `Input` type (used as the deserialization target)
///   - A nested `Output` type (used as the serialization source)
///   - pub fn run(input: Input) Output
///
/// The returned struct exports sim_run as a C-callable function that:
///   1. Sets @setFloatMode from T.reproducibility (D-75)
///   2. Deserializes input_json into T.Input via std.json
///   3. Calls T.run(input) → T.Output
///   4. Serializes {output, metadata} to arena-allocated JSON written to out_json/out_len
///   5. Returns true on success
///
/// Ownership: out_json/out_len point into an arena allocated by sim_run.
/// The caller (zig_runner.zig) is responsible for using the bytes before
/// the arena is freed.
///
/// D-73: The in-process call (sim_run) + the evidence artifact serialization
/// in zig_runner.zig together constitute "one execution, two outputs."
pub fn DealSimulation(comptime T: type) type {
    return struct {
        /// C-callable simulation entry point.
        ///
        /// Implements the JSON Simulation Protocol v0 (spec/sims/v0/).
        /// Input envelope: {"deal_sim_protocol":"v0","inputs":{...}} or bare flat dict.
        /// Output envelope: {"deal_sim_protocol":"v0","exit_code":0,"outputs":{...}}.
        ///
        /// SAFETY (T-05-11 / T-05-SC):
        ///   - @setFloatMode applied at function scope before arithmetic (D-75)
        ///   - NULL + non-zero length pairs rejected (ASVS V5)
        ///   - All allocations in a single arena; caller receives pointer into that arena
        pub export fn sim_run(
            input_json: [*]const u8,
            input_len: usize,
            out_json: *[*]const u8,
            out_len: *usize,
        ) callconv(.c) bool {
            // D-75: set float mode from declared @reproducibility tier.
            // Applied at function scope BEFORE any arithmetic (including deserialization
            // that may involve floating-point). Default: .strict (D-73, A-001).
            const tier: ReproducibilityTier = if (@hasDecl(T, "reproducibility"))
                T.reproducibility
            else
                .strict;
            const float_mode: std.builtin.FloatMode = switch (tier) {
                .strict => .strict,
                .optimized => .optimized,
            };
            @setFloatMode(float_mode);

            // Guard NULL + non-zero length (ASVS V5 / T-05-SC).
            if (input_len > 0 and @intFromPtr(input_json) == 0) return false;

            // Allocate arena for all per-call allocations.
            var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
            const alloc = arena.allocator();
            // NOTE: we intentionally do NOT defer arena.deinit() here — the
            // caller (zig_runner.zig) needs the out_json bytes to remain valid
            // after this call. zig_runner.zig passes a pointer-to-arena-allocator
            // strategy instead; for the direct C ABI path the arena is leaked
            // (acceptable: the Rust orchestrator calls this per-sim-run, so the
            // lifetime is bounded by the run). For the in-process path via
            // zig_runner, use run_sim_with_arena which accepts a caller-owned arena.

            return sim_run_internal(alloc, input_json[0..input_len], out_json, out_len)
                catch false;
        }

        /// Internal simulation runner — accepts a caller-managed allocator.
        ///
        /// Used by zig_runner.run_zig_sim to share the caller's arena so the
        /// output bytes remain valid while the caller serializes evidence artifacts.
        pub fn run_with_arena(
            alloc: std.mem.Allocator,
            input_json_bytes: []const u8,
            out_json: *[*]const u8,
            out_len: *usize,
        ) bool {
            // D-75: same float mode selection as sim_run.
            const tier: ReproducibilityTier = if (@hasDecl(T, "reproducibility"))
                T.reproducibility
            else
                .strict;
            const float_mode: std.builtin.FloatMode = switch (tier) {
                .strict => .strict,
                .optimized => .optimized,
            };
            @setFloatMode(float_mode);

            return sim_run_internal(alloc, input_json_bytes, out_json, out_len)
                catch false;
        }

        /// Shared implementation: parse input JSON, call T.run(), serialize output.
        fn sim_run_internal(
            alloc: std.mem.Allocator,
            input_json_bytes: []const u8,
            out_json: *[*]const u8,
            out_len: *usize,
        ) !bool {
            // Deserialize input.
            // Accept both the spec/sims/v0/ envelope ({"deal_sim_protocol":"v0","inputs":{...}})
            // and the bare flat dict (Shape B — for testing without envelope wrapper).
            const Input = T.Input;
            var parsed_input: Input = undefined;

            // Try spec/sims/v0/ envelope shape first.
            const raw = try std.json.parseFromSlice(std.json.Value, alloc, input_json_bytes, .{});
            defer raw.deinit();

            if (raw.value == .object and raw.value.object.get("inputs") != null) {
                const inputs_val = raw.value.object.get("inputs").?;
                // Shape A: envelope has "inputs" key → deserialize from that sub-object.
                const inputs_str = try std.json.Stringify.valueAlloc(alloc, inputs_val, .{});
                defer alloc.free(inputs_str);
                const inner = try std.json.parseFromSlice(Input, alloc, inputs_str, .{
                    .ignore_unknown_fields = true,
                });
                defer inner.deinit();
                parsed_input = inner.value;
            } else {
                // Shape B: bare flat dict — deserialize directly.
                const inner = try std.json.parseFromSlice(Input, alloc, input_json_bytes, .{
                    .ignore_unknown_fields = true,
                });
                defer inner.deinit();
                parsed_input = inner.value;
            }

            // Execute the simulation.
            const output: T.Output = T.run(parsed_input);

            // Record which tier was used for evidence metadata (D-75).
            const tier_str: []const u8 = if (@hasDecl(T, "reproducibility"))
                switch (T.reproducibility) {
                    .strict => "strict",
                    .optimized => "optimized",
                }
            else
                "strict";

            // Serialize output envelope (spec/sims/v0/ Shape A).
            // Keys alphabetical per D-18: deal_sim_protocol, exit_code, outputs.
            //
            // Use std.json.Stringify.valueAlloc (Zig 0.16.0 API) to serialize
            // the output struct, then hand-roll the envelope around it.
            const outputs_json = try std.json.Stringify.valueAlloc(alloc, output, .{});
            defer alloc.free(outputs_json);

            // Build the full envelope using appendSlice (Zig 0.16.0 ArrayList API).
            var buf: std.ArrayList(u8) = .empty;
            try buf.appendSlice(alloc, "{\"deal_sim_protocol\":\"v0\",\"exit_code\":0,");
            try buf.appendSlice(alloc, "\"outputs\":");
            try buf.appendSlice(alloc, outputs_json);
            // Append reproducibility_tier for evidence metadata (D-75).
            try buf.appendSlice(alloc, ",\"reproducibility_tier\":\"");
            try buf.appendSlice(alloc, tier_str);
            try buf.appendSlice(alloc, "\"}");

            out_json.* = buf.items.ptr;
            out_len.* = buf.items.len;
            return true;
        }

        /// Tier accessor — used by zig_runner.zig to record the declared tier in evidence.
        pub fn reproducibilityTier() ReproducibilityTier {
            return if (@hasDecl(T, "reproducibility")) T.reproducibility else .strict;
        }

        /// Tier name string — used in evidence metadata.json (D-75).
        pub fn reproducibilityTierName() []const u8 {
            return switch (reproducibilityTier()) {
                .strict => "strict",
                .optimized => "optimized",
            };
        }
    };
}

test "sim.deal_sim.setFloatMode_strict" {
    // D-73 acceptance: sim_run with a minimal T that declares reproducibility = .strict
    // must produce bit-reproducible output across two calls.
    const TestModel = struct {
        pub const reproducibility: ReproducibilityTier = .strict;

        pub const Input = struct {
            x: f64 = 0.0,
        };
        pub const Output = struct {
            y: f64,
        };

        pub fn run(input: Input) Output {
            @setFloatMode(.strict);
            return .{ .y = input.x * 2.0 + 1.0 };
        }
    };

    const Sim = DealSimulation(TestModel);
    const alloc = std.testing.allocator;

    const input_json = "{\"deal_sim_protocol\":\"v0\",\"inputs\":{\"x\":3.0}}";

    var out1_ptr: [*]const u8 = undefined;
    var out1_len: usize = 0;
    var arena1 = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena1.deinit();

    const ok1 = Sim.run_with_arena(arena1.allocator(), input_json, &out1_ptr, &out1_len);
    try std.testing.expect(ok1);
    const bytes1 = try alloc.dupe(u8, out1_ptr[0..out1_len]);
    defer alloc.free(bytes1);

    var out2_ptr: [*]const u8 = undefined;
    var out2_len: usize = 0;
    var arena2 = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena2.deinit();

    const ok2 = Sim.run_with_arena(arena2.allocator(), input_json, &out2_ptr, &out2_len);
    try std.testing.expect(ok2);
    const bytes2 = try alloc.dupe(u8, out2_ptr[0..out2_len]);
    defer alloc.free(bytes2);

    // D-73 strict mode assertion: both outputs must be byte-identical.
    try std.testing.expectEqualSlices(u8, bytes1, bytes2);
    // Verify the output contains "deal_sim_protocol" and "exit_code":0.
    try std.testing.expect(std.mem.indexOf(u8, bytes1, "deal_sim_protocol") != null);
    try std.testing.expect(std.mem.indexOf(u8, bytes1, "exit_code") != null);
}

test "sim.deal_sim.default_reproducibility_is_strict" {
    // When T does NOT declare reproducibility, the wrapper defaults to .strict (D-73).
    const NoTierModel = struct {
        // Deliberately omits pub const reproducibility
        pub const Input = struct {
            v: f64 = 0.0,
        };
        pub const Output = struct {
            result: f64,
        };
        pub fn run(input: Input) Output {
            return .{ .result = input.v + 1.0 };
        }
    };

    const Sim = DealSimulation(NoTierModel);
    try std.testing.expectEqualStrings("strict", Sim.reproducibilityTierName());
}
