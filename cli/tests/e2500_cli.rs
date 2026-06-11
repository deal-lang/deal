//! Integration tests for D-88: E2500 cross-file dimensional check via CLI.
//!
//! REQ: D-88 (E2500 carryover from Phase 4 SC-1, closed in Phase 5 Plan 04)
//!
//! These tests verify the 04-HUMAN-UAT Test 2 scenario:
//!   - Error path: `import deal.std.units kg V` then `attribute mass : Mass = V(800)`
//!     must make `deal check` exit non-zero with E2500 (dimensional mismatch:
//!     Voltage assigned to Mass-typed attribute).
//!   - Clean path: `import deal.std.units kg` then `attribute mass : Mass = kg(800)`
//!     must make `deal check` exit 0 (no dimensional mismatch).
//!
//! The Zig dimensional algebra fires E2500 for this pattern (proven in
//! tests/unit/sema_dimensional.zig). The CLI wires `deal_check_with_stdlib`
//! (D-88) so the stdlib dimension/unit table is seeded via analyzeWithExternalTable.
//!
//! Test setup: each test creates a tempdir with:
//!   - A `.deal` source file under `src/`
//!   - A minimal stdlib seed at `.deal/deps/deal-stdlib/packages/units/units.deal`
//! The CLI's D-49 dep-collection block finds the stdlib seed, builds stdlib_bytes,
//! and deal_check_with_stdlib uses it to resolve V as a Voltage unit_def.
//!
//! VALIDATION.md Per-Task Verification Map:
//!   cargo test -p deal --test e2500_cli

use std::path::PathBuf;
use std::process::Command;

/// Minimal stdlib DEAL source for the E2500 test fixtures.
///
/// Contains: Mass and Voltage dimensions + kg and V unit constructors.
/// This is the minimal seed required for the dimensional algebra (Check #7)
/// to resolve `V` as a Voltage unit_def and `kg` as a Mass unit_def.
/// The full deal-stdlib also includes other dimensions, but only these
/// are needed for the two E2500 test scenarios.
const MINIMAL_STDLIB: &str = r#"package deal.std.units;

/** Mass — SI base quantity (M). */
attribute def Mass {
    attribute si_M  : Integer = 1;
    attribute si_L  : Integer = 0;
    attribute si_T  : Integer = 0;
    attribute si_I  : Integer = 0;
    attribute si_TH : Integer = 0;
    attribute si_N  : Integer = 0;
    attribute si_J  : Integer = 0;
}

/** Voltage — M*L^2*T^-3*I^-1 (derived). */
attribute def Voltage {
    attribute si_M  : Integer = 1;
    attribute si_L  : Integer = 2;
    attribute si_T  : Integer = -3;
    attribute si_I  : Integer = -1;
    attribute si_TH : Integer = 0;
    attribute si_N  : Integer = 0;
    attribute si_J  : Integer = 0;
}

/** kg — kilogram, SI base unit for Mass. */
attribute def kg <<specializes>> Mass {
    attribute si_factor : Real = 1.0;
}

/** V — volt, SI derived unit for Voltage. */
attribute def V <<specializes>> Voltage {
    attribute si_factor : Real = 1.0;
}

/** g — gram, SI unit for Mass (0.001 × kg). */
attribute def g <<specializes>> Mass {
    attribute si_factor : Real = 0.001;
}

/** to_g — conversion target for Mass (identity in gram-space). */
attribute def to_g <<specializes>> Mass {
    attribute si_factor : Real = 0.001;
}
"#;

/// Path to the built deal binary.
fn deal_bin() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_deal") {
        path.into()
    } else {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("../target/debug/deal");
        p
    }
}

/// Write the minimal stdlib seed into the tempdir's `.deal/deps/` tree.
///
/// D-49 in run_check searches `.deal/deps/<dep>/packages/**/*.deal` relative
/// to the current working directory. This function creates the required layout
/// so D-49 picks up the stdlib seed when the CLI is invoked with the tempdir
/// as cwd.
fn write_stdlib_seed(tempdir: &tempfile::TempDir) {
    let units_dir = tempdir
        .path()
        .join(".deal")
        .join("deps")
        .join("deal-stdlib")
        .join("packages")
        .join("units");
    std::fs::create_dir_all(&units_dir).expect("create stdlib seed dir");
    std::fs::write(units_dir.join("units.deal"), MINIMAL_STDLIB)
        .expect("write minimal stdlib seed");
}

// ─── Test 1: Voltage assigned to Mass attribute must emit E2500 ───────────────

/// `deal check` exits non-zero with E2500 for cross-file dimensional mismatch.
///
/// D-88: Wire deal_check_with_stdlib into deal check so analyzeWithExternalTable
/// seeds the symbol table with the stdlib dimension/unit entries.
///
/// Scenario (04-HUMAN-UAT Test 2):
///   import deal.std.units kg V;
///   attribute mass : Mass = V(800);   ← Voltage assigned to Mass → E2500
#[test]
fn test_e2500_voltage_assigned_to_mass_attribute() {
    let tempdir = tempfile::TempDir::new().expect("create tempdir");

    // Write the minimal stdlib seed for D-49 dep collection.
    write_stdlib_seed(&tempdir);

    // Write the test source file — Voltage assigned to Mass-typed attribute.
    let src_dir = tempdir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");
    let source_path = src_dir.join("mass_voltage_mismatch.deal");
    std::fs::write(
        &source_path,
        r#"package test.e2500;

import deal.std.units.{kg, V};

part def VehiclePart {
    public (
        // E2500: Mass attribute assigned a Voltage value.
        attribute mass : Mass = V(800);
    )
}
"#,
    )
    .expect("write test source");

    // Run `deal check <file>` with cwd = tempdir so D-49 finds the stdlib dep.
    let output = Command::new(deal_bin())
        .args(["check", "--json"])
        .arg(&source_path)
        .current_dir(tempdir.path())
        .output()
        .expect("failed to run deal check");

    let exit_code = output.status.code().unwrap_or(99);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must exit non-zero (D-34: user error → exit 1).
    assert_ne!(
        exit_code, 0,
        "deal check on Mass=V(800) expected non-zero exit but got 0\n\
         stdout: {stdout}\nstderr: {stderr}"
    );

    // Diagnostics JSON must contain at least one E2500 entry.
    assert!(
        stdout.contains("E2500"),
        "expected E2500 in JSON output for Mass=V(800) scenario\n\
         stdout: {stdout}\nstderr: {stderr}"
    );
}

// ─── Test 2: Correct mass unit must produce no E2500 ─────────────────────────

/// `deal check` exits 0 with no E2500 for correct dimensional usage.
///
/// D-88 clean path: when the unit constructor matches the declared type's
/// dimension (kg is Mass, mass is Mass-typed), no E2500 is emitted.
///
/// Scenario:
///   import deal.std.units kg;
///   attribute mass : Mass = kg(800);  ← correct: Mass = Mass → no E2500
#[test]
fn test_e2500_correct_mass_unit_no_error() {
    let tempdir = tempfile::TempDir::new().expect("create tempdir");

    // Write the minimal stdlib seed for D-49 dep collection.
    write_stdlib_seed(&tempdir);

    // Write the test source file — correct Mass unit usage.
    let src_dir = tempdir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");
    let source_path = src_dir.join("mass_correct.deal");
    std::fs::write(
        &source_path,
        r#"package test.e2500_clean;

import deal.std.units.{kg};

part def VehiclePart {
    public (
        // Correct: Mass attribute with Mass-dimension unit constructor.
        attribute mass : Mass = kg(800);
    )
}
"#,
    )
    .expect("write test source");

    // Run `deal check <file>` with cwd = tempdir so D-49 finds the stdlib dep.
    let output = Command::new(deal_bin())
        .args(["check", "--json"])
        .arg(&source_path)
        .current_dir(tempdir.path())
        .output()
        .expect("failed to run deal check");

    let exit_code = output.status.code().unwrap_or(99);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must exit 0 — correct usage produces no errors (D-34).
    assert_eq!(
        exit_code, 0,
        "deal check on Mass=kg(800) expected exit 0 but got {exit_code}\n\
         stdout: {stdout}\nstderr: {stderr}"
    );

    // Diagnostics JSON must NOT contain E2500.
    assert!(
        !stdout.contains("E2500"),
        "unexpected E2500 in JSON output for Mass=kg(800) scenario\n\
         stdout: {stdout}\nstderr: {stderr}"
    );
}

// ─── Test 3: Mixed-unit same-dimension must emit E2501 ────────────────────────

/// `deal check` exits non-zero with E2501 for mixed-unit same-dimension expression.
///
/// D-09: CLI coverage for E2501 (mixed-unit same-dimension, no explicit conversion).
///
/// Scenario:
///   import deal.std.units {kg, g};
///   attribute total : Mass = kg(1) + g(500);  ← two distinct Mass units mixed → E2501
#[test]
fn test_e2501_mixed_unit_same_dimension() {
    let tempdir = tempfile::TempDir::new().expect("create tempdir");

    // Write the minimal stdlib seed — must include `g` for the two-unit scenario.
    write_stdlib_seed(&tempdir);

    let src_dir = tempdir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");
    let source_path = src_dir.join("mixed_mass_units.deal");
    std::fs::write(
        &source_path,
        r#"package test.e2501;

import deal.std.units.{kg, g};

part def WeightCalc {
    public (
        // E2501: two distinct Mass units (kg and g) combined without conversion.
        attribute total : Mass = kg(1) + g(500);
    )
}
"#,
    )
    .expect("write test source");

    let output = Command::new(deal_bin())
        .args(["check", "--json"])
        .arg(&source_path)
        .current_dir(tempdir.path())
        .output()
        .expect("failed to run deal check");

    let exit_code = output.status.code().unwrap_or(99);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must exit non-zero (D-34: user error → exit 1).
    assert_ne!(
        exit_code, 0,
        "deal check on mixed Mass units expected non-zero exit but got 0\n\
         stdout: {stdout}\nstderr: {stderr}"
    );

    // Diagnostics JSON must contain at least one E2501 entry.
    assert!(
        stdout.contains("E2501"),
        "expected E2501 in JSON output for mixed-unit same-dimension scenario\n\
         stdout: {stdout}\nstderr: {stderr}"
    );
}

// ─── Test 4: Unknown unit literal must emit E2502 ─────────────────────────────

/// `deal check` exits non-zero with E2502 for an undeclared unit literal.
///
/// D-09: CLI coverage for E2502 (unknown unit — unit constructor not in any imported package).
///
/// Scenario:
///   import deal.std.units.{kg};
///   attribute weight : Mass = lb(500);  ← lb not declared in MINIMAL_STDLIB → E2502
#[test]
fn test_e2502_unknown_unit_literal() {
    let tempdir = tempfile::TempDir::new().expect("create tempdir");

    write_stdlib_seed(&tempdir);

    let src_dir = tempdir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");
    let source_path = src_dir.join("unknown_unit.deal");
    std::fs::write(
        &source_path,
        r#"package test.e2502;

import deal.std.units.{kg};

part def Measurement {
    public (
        // E2502: lb is not declared in any imported stdlib package.
        attribute weight : Mass = lb(500);
    )
}
"#,
    )
    .expect("write test source");

    let output = Command::new(deal_bin())
        .args(["check", "--json"])
        .arg(&source_path)
        .current_dir(tempdir.path())
        .output()
        .expect("failed to run deal check");

    let exit_code = output.status.code().unwrap_or(99);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must exit non-zero.
    assert_ne!(
        exit_code, 0,
        "deal check on unknown unit expected non-zero exit but got 0\n\
         stdout: {stdout}\nstderr: {stderr}"
    );

    // Diagnostics JSON must contain at least one E2502 entry.
    assert!(
        stdout.contains("E2502"),
        "expected E2502 in JSON output for unknown-unit scenario\n\
         stdout: {stdout}\nstderr: {stderr}"
    );
}

// ─── Test 5: Conversion type mismatch must emit E2503 ─────────────────────────

/// `deal check` exits non-zero with E2503 for a conversion call with wrong dimension.
///
/// D-09: CLI coverage for E2503 (conversion type mismatch — wrong dimension in
/// conversion call). Fixture is distinct from the Phase-4 regression pin
/// (`07-conversion-mismatch.deal` which uses `to_kg(V(800))`).
///
/// Scenario (fresh minimal fixture — NOT 07-conversion-mismatch.deal):
///   import deal.std.units.{to_g, V};
///   attribute mass : Mass = to_g(V(800));
///       ↑ to_g converts to Mass, but argument V(800) has Voltage dimension → E2503
#[test]
fn test_e2503_conversion_type_mismatch() {
    let tempdir = tempfile::TempDir::new().expect("create tempdir");

    write_stdlib_seed(&tempdir);

    let src_dir = tempdir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");
    let source_path = src_dir.join("mass_for_voltage.deal");
    // Fixture is distinct from the Phase-4 regression pin (07-conversion-mismatch.deal
    // which uses `to_kg(V(800))`). This test uses `to_g(V(800))` — a conversion call
    // targeting Mass dimension but receiving a Voltage argument → E2503.
    // The `to_g` conversion is defined in MINIMAL_STDLIB (starts with `to_` prefix,
    // specializes Mass) so sema identifies it as a conversion call per D-57.
    std::fs::write(
        &source_path,
        r#"package test.e2503;

import deal.std.units.{to_g, V};

part def BadConversion {
    public (
        // E2503: to_g expects a Mass-dimension source, but V(800) is Voltage.
        attribute mass : Mass = to_g(V(800));
    )
}
"#,
    )
    .expect("write test source");

    let output = Command::new(deal_bin())
        .args(["check", "--json"])
        .arg(&source_path)
        .current_dir(tempdir.path())
        .output()
        .expect("failed to run deal check");

    let exit_code = output.status.code().unwrap_or(99);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must exit non-zero.
    assert_ne!(
        exit_code, 0,
        "deal check on mass-unit for Voltage expected non-zero exit but got 0\n\
         stdout: {stdout}\nstderr: {stderr}"
    );

    // Diagnostics JSON must contain at least one E2503 entry.
    assert!(
        stdout.contains("E2503"),
        "expected E2503 in JSON output for conversion-type-mismatch scenario\n\
         stdout: {stdout}\nstderr: {stderr}"
    );
}
