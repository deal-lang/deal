//! Golden fixture tests for the ReqIF 1.2 emitter (Plan 04-06).
//!
//! Two gates per fixture:
//!   Gate 1: Byte-exact match — emitter output (XML extracted from .reqifz)
//!           equals the hand-authored `.expected.reqif` file byte-for-byte.
//!   Gate 2: Valid zip — the emitted `.reqifz` opens as a valid zip archive
//!           containing a `.reqif` XML entry.
//!
//! The `.expected.reqif` files contain RAW unwrapped ReqIF XML (not the .reqifz
//! archive) for readability in diffs. The test runner extracts the XML from the
//! .reqifz before comparing.

use std::io::Read;
use std::process::Command;

/// Path to the built deal binary.
fn deal_bin() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_deal") {
        path.into()
    } else {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("../target/debug/deal");
        p
    }
}

/// Repo root — all fixture paths are relative to this.
fn repo_root() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // pop "cli"
    p
}

/// Extract the first `.reqif` XML entry from a `.reqifz` archive.
///
/// Returns the raw XML bytes. Panics if the archive is invalid or has no .reqif entry.
fn extract_reqif_from_reqifz(reqifz_bytes: &[u8]) -> Vec<u8> {
    let cursor = std::io::Cursor::new(reqifz_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .expect("reqifz must be a valid zip archive");

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .unwrap_or_else(|e| panic!("cannot read zip entry {i}: {e}"));
        let name = entry.name().to_string();
        if name.ends_with(".reqif") {
            let mut xml_bytes = Vec::new();
            entry.read_to_end(&mut xml_bytes)
                .unwrap_or_else(|e| panic!("cannot read zip entry {name}: {e}"));
            return xml_bytes;
        }
    }

    panic!("reqifz archive contains no .reqif entry");
}

/// Run `deal build --target reqif --validate` on a fixture, extract the XML from
/// the .reqifz output, and compare byte-for-byte against the `.expected.reqif`.
///
/// Returns the extracted XML string for subsequent structural validation.
fn run_fixture_and_compare(source_filename: &str) -> String {
    let root = repo_root();
    let source_path = root.join("tests/golden/reqif").join(source_filename);

    let stem = source_filename
        .strip_suffix(".deal")
        .unwrap_or_else(|| panic!("fixture must end in .deal: {source_filename}"));

    let expected_path = root
        .join("tests/golden/reqif")
        .join(format!("{stem}.expected.reqif"));
    let expected_xml = std::fs::read_to_string(&expected_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", expected_path.display(), e));

    // Use a unique temp directory per test invocation to avoid parallel races.
    let tmp_dir = std::env::temp_dir().join(format!(
        "deal-golden-reqif-{}-{}",
        stem,
        std::process::id()
    ));
    std::fs::create_dir_all(&tmp_dir)
        .unwrap_or_else(|e| panic!("cannot create temp dir {}: {}", tmp_dir.display(), e));
    let output_path = tmp_dir.join(format!("{stem}.reqifz"));

    // Run `deal build --target reqif --validate --output <tmp>/<stem>.reqifz`
    let result = Command::new(deal_bin())
        .args(["build", "--color=never", "--target", "reqif", "--validate"])
        .arg("--output")
        .arg(&output_path)
        .arg(&source_path)
        .output()
        .unwrap_or_else(|e| panic!("failed to run deal build: {e}"));

    let exit_code = result.status.code().unwrap_or(99);
    assert_eq!(
        exit_code,
        0,
        "deal build --target reqif on {source_filename} expected exit 0 but got {exit_code}\n\
         stderr: {}",
        String::from_utf8_lossy(&result.stderr),
    );

    // Read the .reqifz output.
    let reqifz_bytes = std::fs::read(&output_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", output_path.display(), e));

    // Gate 2: valid zip.
    assert!(
        zip::ZipArchive::new(std::io::Cursor::new(&reqifz_bytes)).is_ok(),
        "reqifz output is not a valid zip archive"
    );

    // Extract XML from the .reqifz.
    let actual_xml_bytes = extract_reqif_from_reqifz(&reqifz_bytes);
    let actual_xml = String::from_utf8(actual_xml_bytes)
        .unwrap_or_else(|e| panic!("reqif XML is not valid UTF-8: {e}"));

    // Clean up temp dir.
    let _ = std::fs::remove_dir_all(&tmp_dir);

    // Gate 1: Byte-exact match against the .expected.reqif fixture.
    assert_eq!(
        actual_xml, expected_xml,
        "Byte-exact match failed for {source_filename}.\n\
         If the emitter changed intentionally:\n\
           1. Update the .expected.reqif file with: cargo run -p deal -- build \\\n\
              --target reqif --output /tmp/out.reqifz tests/golden/reqif/{source_filename}\n\
              then extract the XML with unzip -p /tmp/out.reqifz '*.reqif'\n\
           2. Verify the XML is structurally valid ReqIF 1.2.\n\
         See tests/golden/reqif/README.md for the two-gate workflow.",
    );

    actual_xml
}

// ─── Fixture 01: single requirement definition ──────────────────────────────

#[test]
fn golden_01_requirement_def() {
    let xml = run_fixture_and_compare("01-requirement-def.deal");
    // Structural gate: must pass validate_reqif_xml
    let result = deal::reqif_schema::validate_reqif_xml(xml.as_bytes());
    assert!(
        result.is_ok(),
        "emitted XML for 01-requirement-def failed structural validation: {:?}",
        result.err()
    );
}

// ─── Fixture 02: trace relation ──────────────────────────────────────────────

#[test]
fn golden_02_trace_relation() {
    let xml = run_fixture_and_compare("02-trace-relation.deal");
    // Structural gate.
    let result = deal::reqif_schema::validate_reqif_xml(xml.as_bytes());
    assert!(
        result.is_ok(),
        "emitted XML for 02-trace-relation failed structural validation: {:?}",
        result.err()
    );
}

// ─── Valid zip test ───────────────────────────────────────────────────────────

#[test]
fn reqifz_is_valid_zip() {
    let root = repo_root();
    let source_path = root.join("tests/golden/reqif/01-requirement-def.deal");

    let tmp_dir = std::env::temp_dir().join(format!(
        "deal-reqifz-zip-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&tmp_dir).expect("create temp dir");
    let output_path = tmp_dir.join("out.reqifz");

    let result = Command::new(deal_bin())
        .args(["build", "--color=never", "--target", "reqif"])
        .arg("--output")
        .arg(&output_path)
        .arg(&source_path)
        .output()
        .expect("run deal build");

    assert_eq!(result.status.code().unwrap_or(99), 0, "build failed: {}", String::from_utf8_lossy(&result.stderr));

    let reqifz_bytes = std::fs::read(&output_path).expect("read reqifz");
    let archive = zip::ZipArchive::new(std::io::Cursor::new(&reqifz_bytes));
    assert!(archive.is_ok(), "reqifz must be valid zip");

    let mut archive = archive.unwrap();
    let mut found_reqif = false;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).unwrap();
        if entry.name().ends_with(".reqif") {
            found_reqif = true;
        }
    }
    assert!(found_reqif, "reqifz must contain a .reqif entry");

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
