//! ADR-0004 R1 end-to-end: `deal check` resolves imports through `[aliases]`.
//!
//! An alias maps an import namespace to a directory whose files declare a
//! DIFFERENTLY-named package (`gadgets = "packages/lib"`, files under it declare
//! `package widgets…`). Before this landed the alias was inert at every layer —
//! the closure walker ignored it and sema matched the literal written path — so an
//! aliased import silently failed to resolve. These tests pin both the positive
//! case and the control (same files, no alias → the reference is unresolved).

use std::path::Path;
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

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create dirs");
    }
    std::fs::write(path, contents).expect("write file");
}

/// Lay down a self-contained project (no stdlib) where `app.deal` imports `Thing`
/// via the alias namespace `gadgets`, while `Thing` actually lives in package
/// `widgets.core`. `declare_alias` toggles the `[aliases]` entry.
fn write_project(root: &Path, declare_alias: bool) {
    let aliases = if declare_alias {
        "\n[aliases]\ngadgets = \"packages/lib\"\n"
    } else {
        "\n"
    };
    write(
        &root.join("deal.toml"),
        &format!(
            "[project]\n\
             name = \"alias-fixture\"\n\
             version = \"0.1.0\"\n\
             schema = \"deal/0.1\"\n\
             marking = \"Unclassified\"\n{aliases}"
        ),
    );
    // packages/lib/ declares `widgets` (bare) + `widgets.core` → LCSP namespace
    // `widgets`, so `gadgets` resolves to `widgets`.
    write(&root.join("packages/lib/index.deal"), "package widgets;\n");
    write(
        &root.join("packages/lib/core.deal"),
        "package widgets.core;\n\
         \n\
         part def Thing {\n\
         \x20   public (\n\
         \x20       attribute n : Real [1] = 1.0;\n\
         \x20   )\n\
         }\n",
    );
    // Entry: imports Thing through the ALIAS namespace `gadgets`.
    write(
        &root.join("model/app.deal"),
        "package app;\n\
         \n\
         import gadgets.core.{Thing};\n\
         \n\
         part def App {\n\
         \x20   public (\n\
         \x20       part t : Thing [1];\n\
         \x20   )\n\
         }\n",
    );
}

fn check(root: &Path) -> (i32, String) {
    let out = Command::new(deal_bin())
        .args(["check", "--color=never"])
        .arg(root)
        .output()
        .expect("run deal check");
    let mut s = String::from_utf8_lossy(&out.stderr).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stdout));
    (out.status.code().unwrap_or(99), s)
}

#[test]
fn aliased_import_resolves_under_check() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    write_project(tmp.path(), true);

    let (code, output) = check(tmp.path());
    assert_eq!(
        code, 0,
        "an import through a declared [aliases] namespace must resolve\noutput: {output}"
    );
    assert!(
        !output.contains("E2100") && !output.contains("E2000") && !output.contains("E2400"),
        "no unresolved-name/import diagnostics expected\noutput: {output}"
    );
}

/// Control: identical sources with NO `[aliases]` entry — the `gadgets.core`
/// import names a package that does not exist, so `Thing` is unresolved. This is
/// what the positive case looked like before alias resolution existed.
#[test]
fn without_alias_declaration_the_reference_is_unresolved() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    write_project(tmp.path(), false);

    let (code, output) = check(tmp.path());
    assert_eq!(
        code, 1,
        "without the alias, the aliased import must not resolve\noutput: {output}"
    );
}
