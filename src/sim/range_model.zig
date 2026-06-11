//! Canonical Zig simulation: EV range model.
//!
//! Mirrors range_model.py's physics for the showcase EVPlatform.
//! Physics: Peukert-approximated range from battery capacity, vehicle mass,
//! and rolling resistance — same equations as the Python counterpart so
//! cross-tool evidence comparison is meaningful.
//!
//! D-73/D-80: This module proves the Zig in-process simulation path.
//!   reproducibility = .strict  →  @setFloatMode(.strict) in DealSimulation
//!
//! Phase 5 Plan 04 — canonical Zig sim for the in-process evidence path.

const std = @import("std");
const deal_sim = @import("deal_sim.zig");

/// Input fields for the EV range simulation.
/// Mirrors the `inputs` section of deal.sims.toml [simulations.range_model].
pub const RangeInput = struct {
    /// Usable battery capacity (kWh). deal.sims.toml: EnergyStorage.battery.usableCapacity
    battery_capacity_kwh: f64 = 85.0,
    /// Vehicle curb mass (kg). deal.sims.toml: EVPlatform.curbWeight
    vehicle_mass_kg: f64 = 2100.0,
    /// Rolling resistance coefficient (dimensionless). deal.sims.toml: Drivetrain.rearLeft.rollingResistance
    rolling_resistance: f64 = 0.009,
};

/// Output fields for the EV range simulation.
/// Mirrors the `outputs` section of deal.sims.toml [simulations.range_model].
pub const RangeOutput = struct {
    /// Estimated total range (km). deal.sims.toml: EVPlatform.totalRange
    total_range_km: f64,
    /// Energy consumption rate (kWh/100km).
    energy_consumption_kwh_per_100km: f64,
};

/// RangeModel — the Zig sim module consumed by DealSimulation(RangeModel).
///
/// Declares reproducibility = .strict so @setFloatMode(.strict) is applied
/// before arithmetic (D-73/D-75). The bit-reproducibility test in deal_sim.zig
/// verifies that two calls with identical inputs produce byte-identical output.
pub const RangeModel = struct {
    /// @reproducibility tier — .strict = IEEE 754 strict mode (D-73 default).
    pub const reproducibility: deal_sim.ReproducibilityTier = .strict;

    /// Input type alias (required by DealSimulation(T) comptime contract).
    pub const Input = RangeInput;
    /// Output type alias (required by DealSimulation(T) comptime contract).
    pub const Output = RangeOutput;

    /// Compute the estimated range and energy consumption.
    ///
    /// Physics (simplified Peukert model for EV range):
    ///   - Energy available: E = battery_capacity_kwh (kWh)
    ///   - Flat-road power demand: P = m * g * Crr * v  (W)
    ///     where g = 9.81 m/s², Crr = rolling_resistance, v = 100 km/h ≈ 27.78 m/s
    ///   - Energy per 100 km: E_100 = P * (100/v) / 1000  (kWh/100km)
    ///   - Total range: R = (E / E_100) * 100  (km)
    ///
    /// This is a conservative estimate (flat road, no aero drag, no regeneration)
    /// consistent with the Python range_model oracle's intent.
    pub fn run(input: Input) Output {
        @setFloatMode(.strict);

        const g: f64 = 9.81; // m/s²
        const v_ms: f64 = 100.0 / 3.6; // 100 km/h in m/s

        // Rolling resistance power at 100 km/h (W).
        const power_w: f64 = input.vehicle_mass_kg * g * input.rolling_resistance * v_ms;

        // Energy per 100 km (kWh/100km).
        // P (W) × time (h) → energy (Wh) ÷ 1000 → kWh.
        // Time for 100 km at 100 km/h = 1 h.
        const energy_per_100km: f64 = power_w * 1.0 / 1000.0;

        // Total range (km) from available capacity.
        const total_range_km: f64 = (input.battery_capacity_kwh / energy_per_100km) * 100.0;

        return .{
            .total_range_km = total_range_km,
            .energy_consumption_kwh_per_100km = energy_per_100km,
        };
    }
};

/// Convenience export: the DealSimulation(RangeModel) wrapper.
/// Exports sim_run as a C-callable entry point.
pub const RangeModelSim = deal_sim.DealSimulation(RangeModel);

test "sim.range_model.physics_sanity" {
    // Verify physics: a 2100 kg vehicle with Crr=0.009 at 100 km/h.
    // Expected power: 2100 * 9.81 * 0.009 * (100/3.6) ≈ 515.2 W → 0.5152 kWh/100km.
    // With 85 kWh: range ≈ 85 / 0.5152 * 100 ≈ 16499 km (very long — flat road only).
    const input = RangeInput{
        .battery_capacity_kwh = 85.0,
        .vehicle_mass_kg = 2100.0,
        .rolling_resistance = 0.009,
    };
    const output = RangeModel.run(input);
    try std.testing.expect(output.total_range_km > 0.0);
    try std.testing.expect(output.energy_consumption_kwh_per_100km > 0.0);
    // Energy consumption should be in a physically plausible range (0.1..50 kWh/100km).
    try std.testing.expect(output.energy_consumption_kwh_per_100km < 50.0);
    try std.testing.expect(output.energy_consumption_kwh_per_100km > 0.1);
}

test "sim.range_model.bit_reproducibility" {
    // D-73: Two calls with identical inputs must produce byte-identical output JSON.
    // This is the acceptance criterion for the .strict reproducibility tier.
    const alloc = std.testing.allocator;

    var arena1 = std.heap.ArenaAllocator.init(alloc);
    defer arena1.deinit();
    var arena2 = std.heap.ArenaAllocator.init(alloc);
    defer arena2.deinit();

    const input_json =
        \\{"deal_sim_protocol":"v0","inputs":{"battery_capacity_kwh":85.0,"rolling_resistance":0.009,"vehicle_mass_kg":2100.0}}
    ;

    var out1_ptr: [*]const u8 = undefined;
    var out1_len: usize = 0;
    const ok1 = RangeModelSim.run_with_arena(arena1.allocator(), input_json, &out1_ptr, &out1_len);
    try std.testing.expect(ok1);
    const bytes1 = try alloc.dupe(u8, out1_ptr[0..out1_len]);
    defer alloc.free(bytes1);

    var out2_ptr: [*]const u8 = undefined;
    var out2_len: usize = 0;
    const ok2 = RangeModelSim.run_with_arena(arena2.allocator(), input_json, &out2_ptr, &out2_len);
    try std.testing.expect(ok2);
    const bytes2 = try alloc.dupe(u8, out2_ptr[0..out2_len]);
    defer alloc.free(bytes2);

    // Bit-reproducibility assertion (D-73 strict).
    try std.testing.expectEqualSlices(u8, bytes1, bytes2);
}
