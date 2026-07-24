//! Regression: project-root resolution must never fall back to the process CWD
//! when explicit paths were given.
//!
//! `find_deal_toml_root` used to append the CWD as a search origin unconditionally.
//! For a file outside any project that meant an unrelated project's `deal.toml`
//! got bound to it — dragging in that project's `[workspace].exclude`, its
//! `[dependencies]` E2402 gate, and its vendored `.deal/deps` — so results
//! depended on which directory the command happened to be run from. ADR-0004
//! requires the opposite (determinism / single source of truth).
//!
//! Three `"."` fallbacks had to go, not one: the E2402 gate, apply_workspace_excludes
//! (which is called from *inside* plan_load_from_paths), and the dependency scan.

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

/// Repo root (…/deal), used to find a real project to run *from*.
fn repo_root() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // pop "cli"
    p
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create dirs");
    }
    std::fs::write(path, contents).expect("write file");
}

/// A self-contained file outside any project must check identically regardless of
/// the CWD. Run it from inside the showcase (a real project with a manifest and a
/// vendored stdlib) — none of that may leak in.
#[test]
fn standalone_file_does_not_inherit_cwd_project() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let file = tmp.path().join("solo.deal");
    // Self-contained: declares its own package and uses only a primitive type,
    // so it is clean on its own and needs no imports.
    write(
        &file,
        "package solo;\n\
         \n\
         part def Widget {\n\
         \x20   public (\n\
         \x20       attribute w : Real [1] = 1.0;\n\
         \x20   )\n\
         }\n",
    );

    let showcase = repo_root().join("spec/examples/showcase");
    if !showcase.join("deal.toml").exists() {
        eprintln!("showcase project missing — skipping cwd-isolation test");
        return;
    }

    let out = Command::new(deal_bin())
        .args(["check", "--color=never"])
        .arg(&file)
        // Deliberately run from *inside another project*.
        .current_dir(&showcase)
        .output()
        .expect("run deal check");

    let stderr = String::from_utf8_lossy(&out.stderr);
    let code = out.status.code().unwrap_or(99);

    assert_eq!(
        code, 0,
        "a self-contained file outside any project must check clean regardless of \
         the CWD project\nstderr: {stderr}"
    );
    // The showcase declares a git dependency; if its manifest leaked in, the
    // not-installed gate (or its deps) would surface here.
    assert!(
        !stderr.contains("E2402"),
        "the CWD project's [dependencies] gate must not apply to an outside file\
         \nstderr: {stderr}"
    );
}

/// Control: the bare invocation (no path args) must STILL resolve the enclosing
/// project from the CWD — that is the one legitimate use of the CWD origin.
#[test]
fn bare_invocation_still_resolves_enclosing_project() {
    let showcase = repo_root().join("spec/examples/showcase");
    if !showcase.join("deal.toml").exists() {
        eprintln!("showcase project missing — skipping bare-invocation test");
        return;
    }

    let out = Command::new(deal_bin())
        .args(["check", "--color=never"])
        .current_dir(&showcase)
        .output()
        .expect("run deal check");

    // It must behave as a project check (resolving the enclosing project), not
    // error out for lack of a root. Either it is clean, or it reports real
    // diagnostics — but never "no .deal or .dealx files found".
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("no .deal or .dealx files found"),
        "bare `deal check` must still resolve the enclosing project from the CWD\
         \nstderr: {stderr}"
    );
}
