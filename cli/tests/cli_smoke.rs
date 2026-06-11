//! CLI smoke tests — verify binary interface contract (D-34).
//!
//! These tests enforce the exit-code contract from D-34:
//!   0 — success (--version)
//!   2 — internal error / not-yet-implemented (all subcommand stubs)
//!
//! Per Pitfall 8 (RESEARCH.md): "A panic or FFI shape mismatch is reported as
//! exit 1 (user-visible error) instead of exit 2 (internal error). CI sees
//! 'expected error' and continues." These tests explicitly assert exit 2 for
//! missing files so the distinction is enforced from day one.
//!
//! Updated by Plan 02-06: parse/fmt/build are now fully implemented;
//! the stub test now only covers `parse` on a nonexistent file (exit 2 = I/O error).

use std::process::Command;

/// Path to the built deal binary.
fn deal_bin() -> std::path::PathBuf {
    // `CARGO_BIN_EXE_deal` is set by cargo test for [[bin]] targets.
    // Fallback: find in target/debug for manual runs.
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_deal") {
        path.into()
    } else {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("../target/debug/deal");
        p
    }
}

/// --version exits 0 and prints "deal <version>".
#[test]
fn version_flag_exits_zero() {
    let output = Command::new(deal_bin())
        .arg("--version")
        .output()
        .expect("failed to run deal --version");

    assert!(
        output.status.success(),
        "deal --version expected exit 0 but got {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("deal "),
        "deal --version stdout should start with 'deal ' but got: {:?}",
        stdout,
    );
}

/// All four subcommands are now fully implemented (Plan 02-06 final stub removal).
/// This test verifies that parse/check/fmt/build all exit 2 on nonexistent file
/// (Internal I/O error per D-34) — not 0 or 1 — confirming the binary is wired.
#[test]
fn subcommand_stubs_exit_two() {
    // Sentinel path — file does not exist; each subcommand should exit 2 (I/O error).
    let sentinel = "nonexistent-fixture.deal";

    let cases: &[(&str, &[&str])] = &[
        ("parse", &["parse", sentinel]),
        ("fmt", &["fmt", sentinel]),
        ("build", &["build", "--target", "sysml-v2", sentinel]),
    ];

    for (name, args) in cases {
        let output = Command::new(deal_bin())
            .args(*args)
            .output()
            .unwrap_or_else(|e| panic!("failed to run deal {} : {}", name, e));

        let exit_code = output.status.code().unwrap_or(0);
        assert_eq!(
            exit_code,
            2,
            "deal {} on nonexistent file expected exit code 2 (D-34 internal I/O error) but got {}\nstdout: {}\nstderr: {}",
            name,
            exit_code,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

/// `deal check` on a nonexistent file exits 2 (Internal I/O error), not 1.
#[test]
fn check_nonexistent_file_exits_two() {
    let output = Command::new(deal_bin())
        .args(["check", "/tmp/__deal_nonexistent_fixture__.deal"])
        .output()
        .expect("failed to run deal check");

    let exit_code = output.status.code().unwrap_or(0);
    assert_eq!(
        exit_code,
        2,
        "deal check <nonexistent> expected exit 2 (D-34 internal) but got {}",
        exit_code,
    );
}
