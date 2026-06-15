//! Layout-agnostic discovery (Phase 1b).
//!
//! The recommended `definitions/` + `model/` layout is a human convention for
//! readability — NOT a requirement. The engine discovers `*.deal`/`*.dealx`
//! anywhere: a flat directory with an arbitrarily-named file (`gonzo.deal`)
//! mixing element kinds must `deal check` clean, with no `[workspace] packages`
//! glob and no special directory names. This is the test that would have caught
//! a hardcoded-directory assumption in discovery.

use std::path::PathBuf;
use std::process::Command;

/// Resolve the `deal` binary built by `cargo test`.
fn deal_bin() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_deal") {
        PathBuf::from(p)
    } else {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("../target/debug/deal");
        p
    }
}

/// A flat project — no `packages/`, `model/`, or `definitions/` dirs, no
/// `[workspace] packages` glob, an arbitrarily-named file mixing a part def and
/// a port def — must `deal check .` with exit 0.
#[test]
fn flat_gonzo_layout_checks_clean() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    // Minimal manifest WITHOUT a [workspace] packages glob — discovery is
    // path-driven, so the glob is not required.
    std::fs::write(
        root.join("deal.toml"),
        "[project]\nname = \"gonzo\"\nversion = \"0.1.0\"\nschema = \"deal/0.1\"\nmarking = \"Unclassified\"\n",
    )
    .unwrap();

    // Arbitrarily-named file mixing element kinds, using only built-in types
    // (no imports / stdlib needed).
    std::fs::write(
        root.join("gonzo.deal"),
        "package gonzo;\n\n\
         part def Widget {\n    attribute size : Real;\n}\n\n\
         port def Signal {\n    attribute level : Real;\n}\n",
    )
    .unwrap();

    let out = Command::new(deal_bin())
        .args(["check", "."])
        .current_dir(root)
        .output()
        .expect("run deal check");

    assert!(
        out.status.success(),
        "deal check on a flat gonzo.deal layout must exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
