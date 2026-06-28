//! Regression test (ADR-0004 P6): vendored dependency sources under `.deal/deps/`
//! must enter analysis ONLY through dependency discovery — never ALSO as project
//! files. Before the fix, `expand_path_args` walked the project tree without
//! pruning `.deal/`, so `deal check <project-dir>` discovered each vendored unit
//! twice (once as a project file, once as a dependency) and emitted a spurious
//! `E2002 duplicate declaration` for every symbol the dependency declared. The
//! EV-platform showcase surfaced this as ~65 duplicate `deal.std.units.*` errors.
//!
//! The real `.deal/` directory is gitignored, so this builds a hermetic project
//! in a TempDir at runtime: one vendored package declaring `vendorlib.shapes.Widget`
//! exactly once, and one project file importing it. Checking the project must
//! exit 0 (Widget resolves → dep IS loaded) with no E2002 (dep loaded only ONCE).

use std::process::Command;

fn deal_bin() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_deal") {
        path.into()
    } else {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("../target/debug/deal");
        p
    }
}

fn write(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixture dirs");
    }
    std::fs::write(path, contents).expect("write fixture file");
}

#[test]
fn vendored_dep_not_double_discovered_as_project_file() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let root = tmp.path();

    write(
        &root.join("deal.toml"),
        "[project]\n\
         name = \"dep-dedup\"\n\
         version = \"0.1.0\"\n\
         schema = \"deal/0.1\"\n\
         marking = \"Unclassified\"\n",
    );

    // Project source: imports a type that ONLY the vendored dependency declares.
    write(
        &root.join("packages/app/main.deal"),
        "package app.main;\n\
         \n\
         import vendorlib.shapes.{Widget};\n\
         \n\
         part def Assembly {\n\
         \x20   public (\n\
         \x20       attribute w : Widget [1];\n\
         \x20   )\n\
         }\n",
    );

    // Vendored dependency under `.deal/deps/<name>/packages/` — the shape
    // `deal install` produces. Declares `vendorlib.shapes.Widget` exactly once.
    write(
        &root.join(".deal/deps/vendorlib/packages/shapes/index.deal"),
        "package vendorlib.shapes;\n\
         \n\
         part def Widget {\n\
         \x20   public (\n\
         \x20       attribute size : Real [1] = 1.0;\n\
         \x20   )\n\
         }\n",
    );

    let out = Command::new(deal_bin())
        .args(["check", "--color=never"])
        .arg(root)
        .output()
        .expect("failed to run deal check");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let code = out.status.code().unwrap_or(99);

    assert_eq!(
        code, 0,
        "checking a project whose only Widget declaration is vendored must exit 0 \
         (the vendored unit must not be discovered as a project file too)\n\
         stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        !stderr.contains("E2002") && !stderr.contains("duplicate declaration"),
        "vendored dependency was double-discovered (spurious E2002)\nstderr: {stderr}"
    );
}
