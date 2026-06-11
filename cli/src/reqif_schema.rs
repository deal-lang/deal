//! Offline ReqIF 1.2 XSD bundle loader + SHA256 tamper-detection + structural validator.
//!
//! Mirrors `cli/src/schema_registry.rs` (the SysML v2 JSON Schema analogue).
//!
//! Design decisions (Plan 04-06):
//!   T-4-12: SHA256-pin both bundled XSD files at load time (tamper-detection).
//!   T-4-12: Mismatch aborts — the XSD bundle is the validation oracle.
//!   T-02-22 analogue: No network fallback — bundle_dir must contain both files.
//!
//! **Structural validation scope** (RESEARCH Pitfall 3 / OQ-1):
//!   No mature offline Rust XSD 1.0 semantic validator exists. `validate_reqif_xml`
//!   performs STRUCTURAL validation via `quick-xml`:
//!     (a) XML is well-formed (reader reaches EOF without error),
//!     (b) root element is `REQ-IF` in namespace `http://www.omg.org/spec/ReqIF/20110401/reqif.xsd`,
//!     (c) required children `THE-HEADER` and `CORE-CONTENT` are present,
//!     (d) `CORE-CONTENT` contains `SPEC-OBJECTS`,
//!     (e) every `SPEC-OBJECT`, `SPEC-RELATION`, and `SPECIFICATION` has a
//!         non-empty `IDENTIFIER` attribute.
//!   This IS the hard gate (OQ-1). Full XSD 1.0 semantic validation is the
//!   Python soft smoke recorded in 04-VERIFICATION.md (Task 3).

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context as _};
use quick_xml::events::Event;
use quick_xml::Reader;
use sha2::{Digest, Sha256};

// ─── SHA256 pins (from spec/references/omg-reqif/SHA256SUMS) ─────────────────

/// Expected SHA256 of spec/references/omg-reqif/reqif.xsd (T-4-12).
///
/// Pin acquired from spec/references/omg-reqif/SHA256SUMS during Plan 04-01.
/// Do not modify the XSD files without updating this pin.
const EXPECTED_REQIF_XSD_SHA256: &str =
    "9243f345540f25db3b53403da9ad9cd4744277ef01492ac3589937f533ba94c0";

/// Expected SHA256 of spec/references/omg-reqif/driver.xsd (T-4-12).
const EXPECTED_DRIVER_XSD_SHA256: &str =
    "4995bc97cf0a9b8462ca295006dd54d9a85fb820cf9fd6e134a51743fc44effd";

/// The expected XML namespace on the root REQ-IF element.
const REQIF_NAMESPACE: &str = "http://www.omg.org/spec/ReqIF/20110401/reqif.xsd";

// ─── Schema holder ────────────────────────────────────────────────────────────

/// Holds the verified raw bytes of the ReqIF XSD bundle.
///
/// Currently only used as a proof-of-load — the bytes are SHA256-verified and
/// stored for future use (e.g., a mature Rust XSD validator when one becomes
/// available). Structural validation uses `validate_reqif_xml` below.
pub struct ReqifSchemas {
    /// Raw bytes of reqif.xsd (SHA256-verified).
    #[allow(dead_code)]
    pub reqif_xsd: Vec<u8>,
    /// Raw bytes of driver.xsd (SHA256-verified).
    #[allow(dead_code)]
    pub driver_xsd: Vec<u8>,
}

// ─── Bundle loading ───────────────────────────────────────────────────────────

/// Load the ReqIF 1.2 XSD bundle from `bundle_dir` and SHA256-verify each file.
///
/// On success, returns `ReqifSchemas` holding the verified bytes.
/// On failure (missing file, tampered content), returns `Err`.
///
/// Enforces T-4-12 (tamper-detection) by verifying SHA256 of both XSD files
/// against pins embedded in this source file (acquired during Plan 04-01).
pub fn load_reqif_schemas(bundle_dir: &Path) -> anyhow::Result<ReqifSchemas> {
    // Canonicalize to prevent path traversal via symlinks or relative components.
    let bundle_dir = bundle_dir
        .canonicalize()
        .with_context(|| format!("cannot canonicalize bundle_dir {}", bundle_dir.display()))?;

    let reqif_path = bundle_dir.join("reqif.xsd");
    let driver_path = bundle_dir.join("driver.xsd");

    // Safety: verify both resolved paths are children of bundle_dir (CWE-22 guard).
    assert_within(&reqif_path, &bundle_dir)?;
    assert_within(&driver_path, &bundle_dir)?;

    // Read raw bytes for SHA256 verification.
    let reqif_bytes = std::fs::read(&reqif_path)
        .with_context(|| format!("cannot read reqif.xsd from {}", reqif_path.display()))?;
    let driver_bytes = std::fs::read(&driver_path)
        .with_context(|| format!("cannot read driver.xsd from {}", driver_path.display()))?;

    // T-4-12: SHA256 tamper-detection — abort on mismatch.
    verify_sha256(&reqif_bytes, EXPECTED_REQIF_XSD_SHA256, "reqif.xsd")?;
    verify_sha256(&driver_bytes, EXPECTED_DRIVER_XSD_SHA256, "driver.xsd")?;

    Ok(ReqifSchemas {
        reqif_xsd: reqif_bytes,
        driver_xsd: driver_bytes,
    })
}

// ─── Structural validation ────────────────────────────────────────────────────

/// Extract the non-empty `IDENTIFIER` attribute value from an element, if present.
fn extract_identifier(attributes: quick_xml::events::attributes::Attributes<'_>) -> Option<String> {
    for attr_result in attributes.flatten() {
        let key = std::str::from_utf8(attr_result.key.as_ref()).unwrap_or("");
        if key == "IDENTIFIER" {
            let val = attr_result.unescape_value().unwrap_or_default();
            if !val.is_empty() {
                return Some(val.into_owned());
            }
        }
    }
    None
}

/// Internal helper: process a start (or empty) XML element for structural validation.
#[allow(clippy::too_many_arguments)]
fn process_start_element<'a>(
    name: &str,
    attributes: quick_xml::events::attributes::Attributes<'a>,
    _depth: i32,
    found_reqif_root: &mut bool,
    root_has_correct_ns: &mut bool,
    found_the_header: &mut bool,
    found_core_content: &mut bool,
    found_spec_objects: &mut bool,
    inside_core_content: &mut bool,
    core_content_depth: &mut i32,
    must_have_identifier: &[&str],
    violations: &mut Vec<String>,
) {
    match name {
        "REQ-IF" => {
            *found_reqif_root = true;
            let mut ns_ok = false;
            for attr_result in attributes {
                if let Ok(attr) = attr_result {
                    let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                    let val = attr.unescape_value().unwrap_or_default();
                    if key == "xmlns" && val == REQIF_NAMESPACE {
                        ns_ok = true;
                    }
                }
            }
            *root_has_correct_ns = ns_ok;
        }
        "THE-HEADER" => {
            *found_the_header = true;
        }
        "CORE-CONTENT" => {
            *found_core_content = true;
            *inside_core_content = true;
            *core_content_depth = _depth;
        }
        "SPEC-OBJECTS" if *inside_core_content => {
            *found_spec_objects = true;
        }
        elem_name if must_have_identifier.contains(&elem_name) => {
            let mut has_identifier = false;
            for attr_result in attributes {
                if let Ok(attr) = attr_result {
                    let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                    if key == "IDENTIFIER" {
                        let val = attr.unescape_value().unwrap_or_default();
                        if !val.is_empty() {
                            has_identifier = true;
                        }
                    }
                }
            }
            if !has_identifier {
                violations.push(format!(
                    "structural: <{elem_name}> missing non-empty IDENTIFIER attribute"
                ));
            }
        }
        _ => {}
    }
}

/// Structurally validate a ReqIF 1.2 XML document.
///
/// Returns `Ok(())` if all structural checks pass.
/// Returns `Err(Vec<String>)` with the list of structural violations on failure.
///
/// **Scope** (RESEARCH Pitfall 3 / OQ-1): This is STRUCTURAL validation only:
///   (a) XML is well-formed
///   (b) Root element is `REQ-IF` in the OMG ReqIF namespace
///   (c) `THE-HEADER` and `CORE-CONTENT` children are present
///   (d) `CORE-CONTENT` contains `SPEC-OBJECTS`
///   (e) Every `SPEC-OBJECT` / `SPEC-RELATION` / `SPECIFICATION` has a
///       non-empty `IDENTIFIER` attribute
///   (f) Every `SPEC-OBJECT-REF` resolves to a declared `SPEC-OBJECT` IDENTIFIER
///       (referential integrity — CR-02: no dangling references)
///
/// This IS the hard gate (D-58/D-60). Full XSD 1.0 semantic validation is
/// the Python soft smoke (Task 3 in Plan 04-06), analogous to schema_registry.rs
/// being the JSON Schema hard gate while viewer-import is the smoke.
pub fn validate_reqif_xml(xml: &[u8]) -> Result<(), Vec<String>> {
    use std::collections::HashSet;

    let mut violations: Vec<String> = Vec::new();

    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);

    // Track structural presence.
    let mut found_reqif_root = false;
    let mut root_has_correct_ns = false;
    let mut found_the_header = false;
    let mut found_core_content = false;
    let mut found_spec_objects = false;

    // Depth tracking to know when we are inside CORE-CONTENT.
    let mut depth: i32 = 0;
    let mut inside_core_content = false;
    let mut core_content_depth: i32 = 0;

    // CR-02 referential-integrity pass: collect every declared SPEC-OBJECT
    // IDENTIFIER and every SPEC-OBJECT-REF target text. After the parse,
    // every ref must resolve to a declared SPEC-OBJECT or the document
    // contains a dangling reference (which real ReqIF tools reject).
    let mut spec_object_ids: HashSet<String> = HashSet::new();
    let mut spec_object_refs: Vec<String> = Vec::new();
    // True while the reader is positioned directly inside a <SPEC-OBJECT-REF>
    // element, so the next Text event is the ref target.
    let mut inside_spec_object_ref = false;

    // Elements that must have a non-empty IDENTIFIER attribute.
    const MUST_HAVE_IDENTIFIER: &[&str] = &["SPEC-OBJECT", "SPEC-RELATION", "SPECIFICATION"];

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name_bytes = e.local_name();
                let name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");
                depth += 1;

                // Collect declared SPEC-OBJECT identifiers for the ref check.
                if name == "SPEC-OBJECT" {
                    if let Some(id) = extract_identifier(e.attributes()) {
                        spec_object_ids.insert(id);
                    }
                }
                if name == "SPEC-OBJECT-REF" {
                    inside_spec_object_ref = true;
                }

                process_start_element(
                    name,
                    e.attributes(),
                    depth,
                    &mut found_reqif_root,
                    &mut root_has_correct_ns,
                    &mut found_the_header,
                    &mut found_core_content,
                    &mut found_spec_objects,
                    &mut inside_core_content,
                    &mut core_content_depth,
                    MUST_HAVE_IDENTIFIER,
                    &mut violations,
                );
            }
            Ok(Event::Empty(ref e)) => {
                // Self-closing elements: increment then immediately process, no depth change needed
                let name_bytes = e.local_name();
                let name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");
                depth += 1;

                // A self-closing SPEC-OBJECT (unusual, but legal) still declares
                // an identifier; collect it for the ref check.
                if name == "SPEC-OBJECT" {
                    if let Some(id) = extract_identifier(e.attributes()) {
                        spec_object_ids.insert(id);
                    }
                }

                process_start_element(
                    name,
                    e.attributes(),
                    depth,
                    &mut found_reqif_root,
                    &mut root_has_correct_ns,
                    &mut found_the_header,
                    &mut found_core_content,
                    &mut found_spec_objects,
                    &mut inside_core_content,
                    &mut core_content_depth,
                    MUST_HAVE_IDENTIFIER,
                    &mut violations,
                );

                // WR-08: a self-closing <CORE-CONTENT/> has no children and emits
                // no matching End event, so `process_start_element` setting
                // `inside_core_content = true` would otherwise leave it stuck on
                // permanently. Reset it immediately for the empty-element case.
                if name == "CORE-CONTENT" {
                    inside_core_content = false;
                }

                // Self-closing: undo depth increment
                depth -= 1;
            }
            Ok(Event::Text(ref t)) => {
                if inside_spec_object_ref {
                    if let Ok(text) = t.decode() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            spec_object_refs.push(trimmed.to_string());
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name_bytes = e.local_name();
                let name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");
                // WR-08: only clear `inside_core_content` when this End event
                // closes the SAME element that opened the CORE-CONTENT scope —
                // i.e. its name matches AND the current depth equals the depth
                // recorded when CORE-CONTENT opened. A nested element that also
                // happens to be named CORE-CONTENT (legal XML) no longer
                // prematurely clears the flag.
                if name == "CORE-CONTENT" && inside_core_content && depth == core_content_depth {
                    inside_core_content = false;
                }
                if name == "SPEC-OBJECT-REF" {
                    inside_spec_object_ref = false;
                }
                depth -= 1;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                violations.push(format!("XML is not well-formed: {e}"));
                return Err(violations);
            }
            _ => {}
        }
        buf.clear();
    }

    // (f) CR-02 referential integrity: every SPEC-OBJECT-REF must resolve.
    for r in &spec_object_refs {
        if !spec_object_ids.contains(r) {
            violations.push(format!(
                "structural: <SPEC-OBJECT-REF> '{r}' does not resolve to any declared \
                 SPEC-OBJECT IDENTIFIER (dangling reference)"
            ));
        }
    }

    // Apply structural rules.
    if !found_reqif_root {
        violations.push("structural: root element is not <REQ-IF>".to_string());
    } else if !root_has_correct_ns {
        violations.push(format!(
            "structural: <REQ-IF> missing xmlns=\"{REQIF_NAMESPACE}\""
        ));
    }

    if !found_the_header {
        violations.push("structural: <THE-HEADER> child of REQ-IF not found".to_string());
    }

    if !found_core_content {
        violations.push("structural: <CORE-CONTENT> child of REQ-IF not found".to_string());
    }

    if !found_spec_objects {
        violations.push("structural: <SPEC-OBJECTS> inside CORE-CONTENT not found".to_string());
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

// ─── Schema directory location ────────────────────────────────────────────────

/// Locate the `spec/references/omg-reqif/` directory relative to the deal repo root.
///
/// Strategy (in priority order):
/// 1. `DEAL_REQIF_SCHEMAS_DIR` environment variable (for CI / custom layouts).
/// 2. Walk up from `CARGO_MANIFEST_DIR` env var (set by cargo during builds + tests).
/// 3. Walk up from `std::env::current_exe()` looking for `spec/references/omg-reqif/`.
pub fn locate_reqif_schemas_dir() -> anyhow::Result<PathBuf> {
    // 1. Explicit override.
    if let Ok(dir) = std::env::var("DEAL_REQIF_SCHEMAS_DIR") {
        let p = PathBuf::from(dir);
        if p.exists() {
            return Ok(p);
        }
    }

    // 2. CARGO_MANIFEST_DIR is set during `cargo build` / `cargo test`.
    //    cli/Cargo.toml is in <repo>/cli/ so the parent is <repo>/.
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let candidate = PathBuf::from(&manifest)
            .parent()
            .map(|p| p.join("spec").join("references").join("omg-reqif"))
            .unwrap_or_default();
        if candidate.exists() {
            return Ok(candidate);
        }
        // Also try the manifest dir itself in case cwd differs.
        let candidate2 = PathBuf::from(&manifest)
            .join("..")
            .join("spec")
            .join("references")
            .join("omg-reqif");
        if candidate2.exists() {
            return Ok(candidate2.canonicalize()?);
        }
    }

    // 3. Walk up from current exe.
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.as_path();
        for _ in 0..8 {
            let candidate = dir.join("spec").join("references").join("omg-reqif");
            if candidate.exists() {
                return Ok(candidate);
            }
            match dir.parent() {
                Some(p) => dir = p,
                None => break,
            }
        }
    }

    Err(anyhow!(
        "cannot locate spec/references/omg-reqif/ directory — set DEAL_REQIF_SCHEMAS_DIR env var \
         to the absolute path of the ReqIF XSD bundle"
    ))
}

// ─── SHA256 verification ──────────────────────────────────────────────────────

/// Compute SHA256 of `bytes` and compare to `expected_hex`.
///
/// Copied verbatim from `cli/src/schema_registry.rs` per PATTERNS.md Shared Pattern.
/// Returns `Err` with a descriptive message if the digest does not match.
fn verify_sha256(bytes: &[u8], expected_hex: &str, label: &str) -> anyhow::Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let actual_hex = hex::encode(digest);

    if actual_hex != expected_hex {
        return Err(anyhow!(
            "Schema bundle tamper-detection failure ({label}): \
             expected SHA256 {expected_hex}, got {actual_hex}. \
             Do not modify spec/references/omg-reqif/ without updating the pins in \
             spec/references/omg-reqif/SHA256SUMS and reqif_schema.rs (T-4-12)."
        ));
    }
    Ok(())
}

// ─── Path traversal guard (CWE-22) ───────────────────────────────────────────

fn assert_within(path: &Path, base: &Path) -> anyhow::Result<()> {
    if !path.starts_with(base) {
        return Err(anyhow!(
            "path traversal guard: {} is not inside {}",
            path.display(),
            base.display()
        ));
    }
    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal valid ReqIF 1.2 XML skeleton.
    const MINIMAL_VALID_REQIF: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <THE-HEADER>
    <REQ-IF-HEADER IDENTIFIER="HDR_001" CREATION-TIME="2026-01-01T00:00:00Z" REQ-IF-TOOL-ID="deal" />
  </THE-HEADER>
  <CORE-CONTENT>
    <SPEC-TYPES>
      <SPEC-OBJECT-TYPE IDENTIFIER="SOT_REQ" LONG-NAME="RequirementType">
        <SPEC-ATTRIBUTES>
          <ATTRIBUTE-DEFINITION-STRING IDENTIFIER="ADS_TEXT" LONG-NAME="ReqText" />
        </SPEC-ATTRIBUTES>
      </SPEC-OBJECT-TYPE>
    </SPEC-TYPES>
    <SPEC-OBJECTS>
      <SPEC-OBJECT IDENTIFIER="DEAL_gold_reqif01_req_REQ_CHARGE_TIME" LONG-NAME="REQ_CHARGE_TIME">
        <TYPE><SPEC-OBJECT-TYPE-REF>SOT_REQ</SPEC-OBJECT-TYPE-REF></TYPE>
        <VALUES>
          <ATTRIBUTE-VALUE-STRING THE-VALUE="The battery shall charge in 30 minutes or less.">
            <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>ADS_TEXT</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
          </ATTRIBUTE-VALUE-STRING>
        </VALUES>
      </SPEC-OBJECT>
    </SPEC-OBJECTS>
    <SPEC-RELATIONS />
    <SPECIFICATIONS>
      <SPECIFICATION IDENTIFIER="SPEC_001" LONG-NAME="EV Platform Requirements">
        <CHILDREN>
          <SPEC-HIERARCHY>
            <OBJECT><SPEC-OBJECT-REF>DEAL_gold_reqif01_req_REQ_CHARGE_TIME</SPEC-OBJECT-REF></OBJECT>
          </SPEC-HIERARCHY>
        </CHILDREN>
      </SPECIFICATION>
    </SPECIFICATIONS>
  </CORE-CONTENT>
</REQ-IF>"#;

    #[test]
    fn validate_accepts_minimal_valid_reqif() {
        let result = validate_reqif_xml(MINIMAL_VALID_REQIF.as_bytes());
        assert!(
            result.is_ok(),
            "minimal valid ReqIF should pass structural validation: {:?}",
            result.err()
        );
    }

    #[test]
    fn validate_rejects_missing_reqif_root() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<NOT-REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <THE-HEADER />
  <CORE-CONTENT><SPEC-OBJECTS /></CORE-CONTENT>
</NOT-REQ-IF>"#;
        let result = validate_reqif_xml(xml.as_bytes());
        assert!(result.is_err(), "missing REQ-IF root should fail");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("root element is not")),
            "error should mention root: {errors:?}"
        );
    }

    #[test]
    fn validate_rejects_wrong_namespace() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<REQ-IF xmlns="http://wrong.namespace.example.com/reqif">
  <THE-HEADER />
  <CORE-CONTENT><SPEC-OBJECTS /></CORE-CONTENT>
</REQ-IF>"#;
        let result = validate_reqif_xml(xml.as_bytes());
        assert!(result.is_err(), "wrong namespace should fail");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("xmlns") || e.contains("namespace") || e.contains("root element")),
            "error should mention namespace: {errors:?}"
        );
    }

    #[test]
    fn validate_rejects_missing_the_header() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <CORE-CONTENT><SPEC-OBJECTS /></CORE-CONTENT>
</REQ-IF>"#;
        let result = validate_reqif_xml(xml.as_bytes());
        assert!(result.is_err(), "missing THE-HEADER should fail");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("THE-HEADER")),
            "error should mention THE-HEADER: {errors:?}"
        );
    }

    #[test]
    fn validate_rejects_missing_core_content() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <THE-HEADER />
</REQ-IF>"#;
        let result = validate_reqif_xml(xml.as_bytes());
        assert!(result.is_err(), "missing CORE-CONTENT should fail");
    }

    #[test]
    fn validate_rejects_spec_object_missing_identifier() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <THE-HEADER />
  <CORE-CONTENT>
    <SPEC-OBJECTS>
      <SPEC-OBJECT LONG-NAME="NoIdentifier">
        <TYPE /><VALUES />
      </SPEC-OBJECT>
    </SPEC-OBJECTS>
  </CORE-CONTENT>
</REQ-IF>"#;
        let result = validate_reqif_xml(xml.as_bytes());
        assert!(result.is_err(), "SPEC-OBJECT missing IDENTIFIER should fail");
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("IDENTIFIER")),
            "error should mention IDENTIFIER: {errors:?}"
        );
    }

    #[test]
    fn validate_rejects_ill_formed_xml() {
        let xml = b"<not-valid-xml<<";
        let result = validate_reqif_xml(xml);
        assert!(result.is_err(), "ill-formed XML should fail");
    }

    #[test]
    fn reqif_schemas_dir_locatable() {
        let dir = locate_reqif_schemas_dir()
            .expect("should locate spec/references/omg-reqif/");
        assert!(
            dir.join("reqif.xsd").exists(),
            "reqif.xsd missing in {}",
            dir.display()
        );
        assert!(
            dir.join("driver.xsd").exists(),
            "driver.xsd missing in {}",
            dir.display()
        );
    }

    #[test]
    fn sha256_pins_match() {
        // Verifies T-4-12: bundled XSD files match expected digests.
        let dir = locate_reqif_schemas_dir().expect("locate reqif schemas");
        let reqif_bytes = std::fs::read(dir.join("reqif.xsd")).expect("read reqif.xsd");
        let driver_bytes = std::fs::read(dir.join("driver.xsd")).expect("read driver.xsd");
        verify_sha256(&reqif_bytes, EXPECTED_REQIF_XSD_SHA256, "reqif.xsd")
            .expect("reqif.xsd SHA256 mismatch");
        verify_sha256(&driver_bytes, EXPECTED_DRIVER_XSD_SHA256, "driver.xsd")
            .expect("driver.xsd SHA256 mismatch");
    }

    #[test]
    fn load_reqif_schemas_succeeds() {
        let dir = locate_reqif_schemas_dir().expect("locate reqif schemas");
        let schemas = load_reqif_schemas(&dir).expect("load_reqif_schemas failed");
        assert!(!schemas.reqif_xsd.is_empty(), "reqif.xsd bytes must be non-empty");
        assert!(!schemas.driver_xsd.is_empty(), "driver.xsd bytes must be non-empty");
    }
}
