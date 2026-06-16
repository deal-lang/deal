//! Build script for the DEAL Rust FFI test harness.
//!
//! Responsibility:
//!   1. Invoke `zig build` in `deal/` to ensure `libdeal.a` exists.
//!   2. Print `cargo::rustc-link-search=native=<deal>/zig-out/lib` so the
//!      linker can find `libdeal.a`.
//!   3. Print `cargo::rustc-link-lib=static=deal` to link statically.
//!   4. Emit `cargo::rerun-if-changed=...` directives for the Zig sources
//!      so cargo rebuilds whenever the Zig side changes.
//!
//! Uses the Cargo 1.77+ double-colon (`cargo::...`) namespace.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // tests/ffi/ is two levels below deal/.
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR unset");
    let deal_dir = PathBuf::from(&manifest_dir)
        .join("..")
        .join("..")
        .canonicalize()
        .expect("failed to canonicalize deal/ directory");

    // 1. Build libdeal.a via the Zig build system.
    let status = Command::new("zig")
        .arg("build")
        .current_dir(&deal_dir)
        .status()
        .expect("failed to invoke `zig build` — is zig 0.16.0 on PATH?");
    assert!(
        status.success(),
        "`zig build` failed in {}",
        deal_dir.display()
    );

    // 2. Tell rustc / lld where to find the static library.
    let lib_dir = deal_dir.join("zig-out").join("lib");
    println!("cargo::rustc-link-search=native={}", lib_dir.display());

    // 3. Link libdeal.a statically.
    println!("cargo::rustc-link-lib=static=deal");

    // 4. Re-run if any Zig-side source changes.
    let src_dir = deal_dir.join("src");
    let include_dir = deal_dir.join("include");
    let build_zig = deal_dir.join("build.zig");
    let build_zon = deal_dir.join("build.zig.zon");
    println!("cargo::rerun-if-changed={}", src_dir.display());
    println!("cargo::rerun-if-changed={}", include_dir.display());
    println!("cargo::rerun-if-changed={}", build_zig.display());
    println!("cargo::rerun-if-changed={}", build_zon.display());
}
