//! Offline JSON Schema validator for SysML v2 output.
//!
//! Implements `jsonschema::Retrieve` against the bundled `tests/schemas/`
//! directory — offline-by-construction (no network access, no reqwest).
//!
//! Design decisions (Plan 02-04):
//!   D-19: Rust owns the offline JSON-Schema validator.
//!   T-02-21: SHA256-pin both bundled schemas at build_validator() time.
//!   T-02-22: No network fallback — LocalBundleRetriever rejects unknown URIs.
//!   Pitfall 4: Validator is OnceLock-cached — expensive SysML.json compilation
//!              happens ONCE per process, not once per test or once per file.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use anyhow::{anyhow, Context as _};
use jsonschema::{Retrieve, Uri, Validator};
use serde_json::Value;
use sha2::{Digest, Sha256};

// ─── SHA256 pins from tests/schemas/README.md ────────────────────────────────

/// Expected SHA256 of tests/schemas/SysML.json (from tests/schemas/README.md).
const EXPECTED_SYSML_SHA256: &str =
    "bb0d8af159cf2cbe4a0df4ed6b903505a57e33d047068e5a50a7008a18d546c5";

/// Expected SHA256 of tests/schemas/KerML.json (from tests/schemas/README.md).
const EXPECTED_KERML_SHA256: &str =
    "e454fe4b7c04f3d95874b6c1a4e6ef056ea5874c71fd3d17e3180319c2f58ab2";

// ─── LocalBundleRetriever ─────────────────────────────────────────────────────

/// A `jsonschema::Retrieve` implementation that resolves all OMG $ref / $id
/// URIs against the local `tests/schemas/` bundle.
///
/// Offline-by-construction: `retrieve()` returns `Err` for any URI not
/// registered in the bundle — there is no HTTP fallback.
pub struct LocalBundleRetriever {
    /// Key: absolute URI string (from schema `$id` field).
    /// Value: the JSON sub-schema (or root schema) at that URI.
    bundle: HashMap<String, Value>,
}

impl LocalBundleRetriever {
    /// Construct a retriever from the given `bundle_dir` (typically
    /// `<repo_root>/tests/schemas/`).
    ///
    /// Reads SysML.json and KerML.json, verifies SHA256 against the pins
    /// declared in tests/schemas/README.md (T-02-21), then indexes every
    /// `$id` field found in each schema.
    pub fn new(bundle_dir: &Path) -> anyhow::Result<Self> {
        // Canonicalize bundle_dir first to prevent path traversal via symlinks
        // or relative components. All subsequent joins are against this canonical
        // absolute base — never against caller-supplied subpaths (CWE-22 guard).
        let bundle_dir = bundle_dir
            .canonicalize()
            .with_context(|| format!("cannot canonicalize bundle_dir {}", bundle_dir.display()))?;

        // Build absolute paths for the two fixed schema file names.
        // These names are hardcoded (not caller-supplied), so there is no
        // additional path component that could escape the bundle directory.
        let sysml_path = bundle_dir.join("SysML.json");
        let kerml_path = bundle_dir.join("KerML.json");

        // Safety: verify both resolved paths are children of bundle_dir.
        fn assert_within(path: &std::path::Path, base: &std::path::Path) -> anyhow::Result<()> {
            if !path.starts_with(base) {
                return Err(anyhow!(
                    "path traversal guard: {} is not inside {}",
                    path.display(),
                    base.display()
                ));
            }
            Ok(())
        }
        assert_within(&sysml_path, &bundle_dir)?;
        assert_within(&kerml_path, &bundle_dir)?;

        // Read raw bytes for SHA256 verification.
        let sysml_bytes = std::fs::read(&sysml_path)
            .with_context(|| format!("cannot read SysML.json from {}", sysml_path.display()))?;
        let kerml_bytes = std::fs::read(&kerml_path)
            .with_context(|| format!("cannot read KerML.json from {}", kerml_path.display()))?;

        // T-02-21: SHA256 tamper-detection.
        verify_sha256(&sysml_bytes, EXPECTED_SYSML_SHA256, "SysML.json")?;
        verify_sha256(&kerml_bytes, EXPECTED_KERML_SHA256, "KerML.json")?;

        // Parse JSON.
        let sysml: Value = serde_json::from_slice(&sysml_bytes)
            .with_context(|| "failed to parse SysML.json")?;
        let kerml: Value = serde_json::from_slice(&kerml_bytes)
            .with_context(|| "failed to parse KerML.json")?;

        // Index all $id → sub-schema mappings.
        let mut bundle = HashMap::new();
        index_schema_ids(&sysml, &mut bundle);
        index_schema_ids(&kerml, &mut bundle);

        Ok(Self { bundle })
    }
}

impl Retrieve for LocalBundleRetriever {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.bundle
            .get(uri.as_str())
            .cloned()
            .ok_or_else(|| format!("Schema not in offline bundle: {uri}").into())
    }
}

// ─── SHA256 verification ──────────────────────────────────────────────────────

/// Compute SHA256 of `bytes` and compare to `expected_hex`.
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
             Do not modify tests/schemas/ without updating the pins in tests/schemas/README.md \
             and schema_registry.rs (T-02-21)."
        ));
    }
    Ok(())
}

// ─── Schema $id indexer ───────────────────────────────────────────────────────

/// Walk the given schema Value and register every `$id` field (absolute URI)
/// → the containing sub-object into `bundle`.
///
/// The OMG SysML.json and KerML.json schemas have no top-level `$id`, but
/// every entry in `$defs` has a `$id` of the form
/// `https://www.omg.org/spec/SysML/20250201/<TypeName>`.
/// We register both the schema root under those $id keys and each $defs entry.
fn index_schema_ids(schema: &Value, bundle: &mut HashMap<String, Value>) {
    match schema {
        Value::Object(obj) => {
            // If this object has a $id field with a string value, register it.
            if let Some(Value::String(id)) = obj.get("$id") {
                if id.starts_with("http") {
                    bundle.insert(id.clone(), schema.clone());
                }
            }
            // Recurse into all values (handles $defs, etc.).
            for value in obj.values() {
                index_schema_ids(value, bundle);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                index_schema_ids(item, bundle);
            }
        }
        _ => {}
    }
}

// ─── Cached validator ─────────────────────────────────────────────────────────

/// Process-global cached validator. Compiled once on first call (Pitfall 4).
/// The schema compilation for 126K-line SysML.json is expensive; caching it
/// amortizes the cost across all files processed in a single `deal build` run.
static CACHED_VALIDATOR: OnceLock<Arc<Validator>> = OnceLock::new();

/// Build (or return the cached) JSON Schema validator for SysML v2 JSON output.
///
/// On first call, loads `tests/schemas/SysML.json` (relative to the deal repo
/// root), verifies SHA256, registers all $id → sub-schema mappings (including
/// KerML.json which is referenced by SysML.json), and compiles the validator.
/// The compiled validator is cached in a process-global `OnceLock<Arc<Validator>>`.
///
/// Exit code implication: if this function returns `Err`, the caller must
/// propagate as CliError::Internal (exit code 2 per D-34) — it is an
/// infrastructure failure, not a user input error.
pub fn build_validator() -> anyhow::Result<Arc<Validator>> {
    if let Some(v) = CACHED_VALIDATOR.get() {
        return Ok(Arc::clone(v));
    }

    let bundle_dir = locate_schemas_dir()?;
    let retriever = LocalBundleRetriever::new(&bundle_dir)?;

    // Load the root schema (SysML.json) for the validator.
    let sysml_path = bundle_dir.join("SysML.json");
    let sysml_bytes = std::fs::read(&sysml_path)
        .with_context(|| format!("cannot read {}", sysml_path.display()))?;
    let sysml_schema: Value = serde_json::from_slice(&sysml_bytes)
        .with_context(|| "cannot parse SysML.json")?;

    // Build the validator using draft202012 options with our offline retriever.
    // default-features = false in Cargo.toml prevents reqwest from being pulled.
    let validator = jsonschema::options()
        .with_retriever(retriever)
        .build(&sysml_schema)
        .map_err(|e| anyhow!("failed to compile SysML.json schema: {e}"))?;

    let arc = Arc::new(validator);

    // Store in OnceLock; if another thread raced us, discard ours and use theirs.
    let _ = CACHED_VALIDATOR.set(Arc::clone(&arc));
    Ok(Arc::clone(CACHED_VALIDATOR.get().unwrap()))
}

// ─── Validate helper ─────────────────────────────────────────────────────────

/// Validate `json` against the bundled SysML v2 schema.
///
/// Returns `Ok(())` if the document is schema-valid.
/// Returns `Err(Vec<String>)` with all validation error messages on failure.
///
/// This function is `pub` so that `golden_fixture_schema_validity.rs` can call
/// it directly on hand-authored `.expected.json` files, independent of the
/// emitter's byte-match (WARNING-03 mitigation).
pub fn validate(json: &Value) -> Result<(), Vec<String>> {
    let validator = build_validator().map_err(|e| vec![format!("schema load error: {e}")])?;

    let errors: Vec<String> = validator
        .iter_errors(json)
        .map(|e| format!("{}: {}", e.instance_path(), e))
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// ─── Schema directory location ────────────────────────────────────────────────

/// Locate the `tests/schemas/` directory relative to the deal repo root.
///
/// Strategy (in priority order):
/// 1. `DEAL_SCHEMAS_DIR` environment variable (for CI / custom layouts).
/// 2. Walk up from the current binary's manifest dir using `CARGO_MANIFEST_DIR`
///    env var (set by cargo during builds and tests).
/// 3. Walk up from `std::env::current_exe()` looking for `tests/schemas/`.
pub fn locate_schemas_dir() -> anyhow::Result<std::path::PathBuf> {
    // 1. Explicit override.
    if let Ok(dir) = std::env::var("DEAL_SCHEMAS_DIR") {
        let p = std::path::PathBuf::from(dir);
        if p.exists() {
            return Ok(p);
        }
    }

    // 2. CARGO_MANIFEST_DIR is set during `cargo build` / `cargo test`.
    //    cli/Cargo.toml is in <repo>/cli/ so the parent is <repo>/.
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let candidate = std::path::PathBuf::from(&manifest)
            .parent()
            .map(|p| p.join("tests").join("schemas"))
            .unwrap_or_default();
        if candidate.exists() {
            return Ok(candidate);
        }
        // Also try the manifest dir itself in case cwd is different.
        let candidate2 = std::path::PathBuf::from(&manifest)
            .join("..").join("tests").join("schemas");
        if candidate2.exists() {
            return Ok(candidate2.canonicalize()?);
        }
    }

    // 3. Walk up from current exe.
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.as_path();
        for _ in 0..8 {
            let candidate = dir.join("tests").join("schemas");
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
        "cannot locate tests/schemas/ directory — set DEAL_SCHEMAS_DIR env var \
         to the absolute path of the schemas bundle"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemas_dir_locatable() {
        let dir = locate_schemas_dir().expect("should locate tests/schemas/");
        assert!(dir.join("SysML.json").exists(), "SysML.json missing in {}", dir.display());
        assert!(dir.join("KerML.json").exists(), "KerML.json missing in {}", dir.display());
    }

    #[test]
    fn sha256_pins_match() {
        // Verifies T-02-21: bundled schemas match expected digests.
        let dir = locate_schemas_dir().expect("locate schemas");
        let sysml_bytes = std::fs::read(dir.join("SysML.json")).expect("read SysML.json");
        let kerml_bytes = std::fs::read(dir.join("KerML.json")).expect("read KerML.json");
        verify_sha256(&sysml_bytes, EXPECTED_SYSML_SHA256, "SysML.json")
            .expect("SysML.json SHA256 mismatch");
        verify_sha256(&kerml_bytes, EXPECTED_KERML_SHA256, "KerML.json")
            .expect("KerML.json SHA256 mismatch");
    }

    #[test]
    fn retriever_returns_known_id() {
        let dir = locate_schemas_dir().expect("locate schemas");
        let retriever = LocalBundleRetriever::new(&dir).expect("build retriever");
        // PartDefinition is a known SysML $id.
        let uri_str = "https://www.omg.org/spec/SysML/20250201/PartDefinition".to_string();
        let uri = Uri::parse(uri_str).expect("parse URI");
        let result = retriever.retrieve(&uri);
        assert!(result.is_ok(), "PartDefinition should be in bundle");
    }

    #[test]
    fn retriever_rejects_unknown_id() {
        let dir = locate_schemas_dir().expect("locate schemas");
        let retriever = LocalBundleRetriever::new(&dir).expect("build retriever");
        let uri_str = "https://example.com/not-in-bundle".to_string();
        let uri = Uri::parse(uri_str).expect("parse URI");
        assert!(retriever.retrieve(&uri).is_err(), "unknown URI should fail offline");
    }

    #[test]
    fn validator_is_cached() {
        // Second call should return quickly from OnceLock.
        let _v1 = build_validator().expect("first build");
        let start = std::time::Instant::now();
        let _v2 = build_validator().expect("second build");
        let elapsed = start.elapsed();
        // Second call must be essentially instant (< 100ms) — it hits the OnceLock.
        assert!(
            elapsed.as_millis() < 100,
            "second build_validator call took {}ms — OnceLock not working?",
            elapsed.as_millis()
        );
    }
}
