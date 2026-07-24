//! ADR-0004 P6 (WS-C): the `deal simulate` strict model gate.
//!
//! A simulation reads values out of the model, so running one against a model that
//! fails import-scoped analysis is garbage-in/garbage-out. Worse, the IR/value
//! loaders silently skip files that fail to parse, so before the gate a broken model
//! quietly produced `null` sim inputs instead of an error.
//!
//! What these tests pin:
//!   1. A model with errors is REFUSED — exit 1, the message points at `deal check`,
//!      and no sim output is produced.
//!   2. The gate is VACUOUS for a project with no model files, so the documented
//!      sim-only / pre-seeded-`input.json` workflow (D-72) keeps working.
//!   3. The gate fires on MODEL validity even when inputs are fully pre-seeded —
//!      pre-seeding satisfies inputs, it does not make a broken model simulable.

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

/// A minimal project with a `simulations/deal.sims.toml` naming one python sim.
/// The entry script does not need to exist: the gate runs before dispatch.
fn write_sim_registry(root: &Path) {
    write(
        &root.join("simulations/deal.sims.toml"),
        "[simulations.demo]\n\
         tool = \"python\"\n\
         entry = \"simulations/demo.py\"\n\
         class = \"Demo\"\n\
         inputs = []\n\
         outputs = []\n",
    );
}

fn write_manifest(root: &Path) {
    write(
        &root.join("deal.toml"),
        "[project]\n\
         name = \"gate-demo\"\n\
         version = \"0.1.0\"\n\
         schema = \"deal/0.1\"\n\
         marking = \"Unclassified\"\n",
    );
}

fn run_simulate(root: &Path) -> (i32, String) {
    let out = Command::new(deal_bin())
        .args(["simulate", "demo", "--color=never"])
        .current_dir(root)
        .output()
        .expect("run deal simulate");
    let mut combined = String::from_utf8_lossy(&out.stderr).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stdout));
    (out.status.code().unwrap_or(99), combined)
}

/// (1) A model file with an unresolved type must block simulation.
#[test]
fn gate_refuses_model_with_errors() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let root = tmp.path();
    write_manifest(root);
    write_sim_registry(root);

    // `Nope` is neither declared locally nor imported → strict analysis errors.
    write(
        &root.join("packages/app/parts.deal"),
        "package app.parts;\n\
         \n\
         part def Widget {\n\
         \x20   public (\n\
         \x20       attribute w : Nope [1];\n\
         \x20   )\n\
         }\n",
    );

    let (code, output) = run_simulate(root);

    assert_eq!(
        code, 1,
        "a model with errors must be refused with exit 1 (D-34 user error)\noutput: {output}"
    );
    assert!(
        output.contains("refusing to simulate"),
        "refusal should say it is refusing to simulate\noutput: {output}"
    );
    assert!(
        output.contains("deal check"),
        "refusal should point the user at `deal check`\noutput: {output}"
    );
    assert!(
        !root.join(".deal/evidence/demo/output.json").exists(),
        "no sim output may be produced when the gate refuses"
    );
}

/// (2) A project with no model files must NOT be gated — the sim-only /
/// pre-seeded workflow (D-72) predates and outlives the gate.
#[test]
fn gate_is_vacuous_for_model_less_project() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let root = tmp.path();
    write_sim_registry(root);

    let (_code, output) = run_simulate(root);

    // It may still fail for unrelated reasons (missing python entry script), but it
    // must not fail *because of the gate*.
    assert!(
        !output.contains("refusing to simulate"),
        "the gate must stay vacuous when the project has no model files\noutput: {output}"
    );
}

/// (3) Pre-seeding inputs satisfies *inputs*; it does not make a broken *model*
/// simulable. The gate is about model validity and still fires.
#[test]
fn gate_refuses_even_when_inputs_pre_seeded() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let root = tmp.path();
    write_manifest(root);
    write_sim_registry(root);
    write(
        &root.join("packages/app/parts.deal"),
        "package app.parts;\n\
         \n\
         part def Widget {\n\
         \x20   public (\n\
         \x20       attribute w : Nope [1];\n\
         \x20   )\n\
         }\n",
    );
    // Fully pre-seeded input for the sim.
    write(
        &root.join(".deal/evidence/demo/input.json"),
        "{\"v\":1,\"inputs\":{}}\n",
    );

    let (code, output) = run_simulate(root);

    assert_eq!(
        code, 1,
        "pre-seeded inputs must not bypass the model gate\noutput: {output}"
    );
    assert!(
        output.contains("refusing to simulate"),
        "pre-seeded inputs must not bypass the model gate\noutput: {output}"
    );
}
