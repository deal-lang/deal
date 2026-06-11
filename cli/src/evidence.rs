//! Evidence capture and baseline management for `deal evidence` (Phase 5 / D-81, D-82).
//!
//! Implements:
//!   - `deal evidence capture` — write durable evidence artifacts from the working cache
//!     into a captured set the verify engine reads (.deal/evidence/ → .deal/captured/)
//!   - `deal evidence baseline <tag>` — frozen snapshot + SHA-256 manifest (D-82)
//!     Writes to evidence/baselines/<tag>/ (tracked, repo root — NOT under .deal)
//!
//! D-81 clean split:
//!   - Working cache: `.deal/evidence/<sim>/` (gitignored, per PS-10)
//!   - Durable baseline: `evidence/baselines/<tag>/` (tracked in git, repo root)
//!
//! D-18 byte-stability: All aggregate JSON uses BTreeMap so keys are alphabetical.
//! D-82 manifest records per-sim content_hash, reproducibility_tier, tool,
//!   tool_version, verdict, plus top-level tag, timestamp, v:1.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::CliError;

// ─── EvidenceCommand (clap subcommand variants) ────────────────────────────────

/// Subcommands for `deal evidence`.
#[derive(Debug, Clone, clap::Subcommand)]
pub enum EvidenceCommand {
    /// Write durable evidence artifacts from the working simulation cache (D-74, D-81).
    Capture,
    /// Tag a frozen evidence snapshot for V&V reference (D-82).
    Baseline {
        /// Version tag for the baseline (e.g. "v2.1.0").
        tag: String,
    },
}

// ─── Constants ─────────────────────────────────────────────────────────────────

/// Provisional verdict written to metadata.json by the simulate runner until
/// Plan 06's verify engine backfills real PASS/FAIL/PARTIAL values (D-82 / Plan 05 task 2).
const DEFAULT_VERDICT: &str = "PARTIAL";

/// Valid verdict values (D-82).
const VALID_VERDICTS: &[&str] = &["PASS", "FAIL", "PARTIAL"];

// ─── Evidence entry points ─────────────────────────────────────────────────────

/// Dispatch `deal evidence` subcommand.
pub fn run_evidence(subcommand: EvidenceCommand) -> Result<(), CliError> {
    match subcommand {
        EvidenceCommand::Capture => run_evidence_capture(),
        EvidenceCommand::Baseline { tag } => run_evidence_baseline(&tag),
    }
}

// ─── Task 1: deal evidence capture ────────────────────────────────────────────

/// Capture current simulation results to the captured evidence set.
///
/// Scans `.deal/evidence/` for sim directories, reads each sim's output.json +
/// metadata.json (and skip.json if present), and snapshots them into
/// `.deal/captured/` — the path the verify engine reads (D-81 working-cache side).
///
/// Writes a BTreeMap-keyed aggregate index to `.deal/captured/index.json` (D-18).
/// Uses write_file_atomic (tmp+rename) for all writes.
pub fn run_evidence_capture() -> Result<(), CliError> {
    let cwd = std::env::current_dir().map_err(|e| {
        CliError::Internal(anyhow::anyhow!("cannot get current directory: {}", e))
    })?;
    run_evidence_capture_in(&cwd)
}

/// Capture implementation parameterised on project root (testable).
pub fn run_evidence_capture_in(project_root: &Path) -> Result<(), CliError> {
    let evidence_dir = project_root.join(".deal").join("evidence");
    let captured_dir = project_root.join(".deal").join("captured");

    // If no evidence cache exists yet, report and succeed (nothing to capture).
    if !evidence_dir.exists() {
        let _ = writeln!(
            std::io::stderr(),
            "deal evidence capture: no .deal/evidence/ directory found — nothing to capture"
        );
        return Ok(());
    }

    std::fs::create_dir_all(&captured_dir).map_err(|e| {
        CliError::Internal(anyhow::anyhow!(
            "cannot create captured dir {:?}: {}",
            captured_dir,
            e
        ))
    })?;

    // Scan evidence dir for simulation subdirectories.
    // Each immediate child of .deal/evidence/<sim-name>/ is one simulation run.
    let mut index: BTreeMap<String, serde_json::Value> = BTreeMap::new();

    let entries = std::fs::read_dir(&evidence_dir).map_err(|e| {
        CliError::Internal(anyhow::anyhow!("cannot read {:?}: {}", evidence_dir, e))
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            CliError::Internal(anyhow::anyhow!("directory entry error: {}", e))
        })?;
        let sim_path = entry.path();
        if !sim_path.is_dir() {
            continue;
        }
        let sim_name = sim_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if sim_name.is_empty() {
            continue;
        }

        // Create captured/<sim-name>/ directory.
        let cap_sim_dir = captured_dir.join(&sim_name);
        std::fs::create_dir_all(&cap_sim_dir).map_err(|e| {
            CliError::Internal(anyhow::anyhow!(
                "cannot create {:?}: {}",
                cap_sim_dir,
                e
            ))
        })?;

        // Copy output.json, metadata.json, skip.json if present.
        let mut sim_info: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        for artifact in &["output.json", "metadata.json", "skip.json"] {
            let src = sim_path.join(artifact);
            if src.exists() {
                let dst = cap_sim_dir.join(artifact);
                let bytes = std::fs::read(&src).map_err(|e| {
                    CliError::Internal(anyhow::anyhow!("read {:?}: {}", src, e))
                })?;
                write_file_atomic(&dst, &bytes)?;
                sim_info.insert(artifact.to_string(), serde_json::Value::Bool(true));
            }
        }
        index.insert(sim_name, serde_json::Value::Object(
            sim_info.into_iter().collect(),
        ));
    }

    // Write BTreeMap-keyed aggregate index (D-18).
    let index_val = serde_json::json!({
        "captured": index,
        "v": 1,
    });
    let index_bytes = serde_json::to_vec_pretty(&index_val).map_err(|e| {
        CliError::Internal(anyhow::anyhow!("serialize index: {}", e))
    })?;
    write_file_atomic(&captured_dir.join("index.json"), &index_bytes)?;

    let _ = writeln!(
        std::io::stderr(),
        "deal evidence capture: captured {} simulation(s) into .deal/captured/",
        index_val["captured"]
            .as_object()
            .map(|m| m.len())
            .unwrap_or(0)
    );

    Ok(())
}

// ─── Task 2: deal evidence baseline <tag> ─────────────────────────────────────

/// Create a tagged evidence baseline from the current evidence cache.
///
/// Copies sim output.json files from `.deal/evidence/<sim>/` into
/// `evidence/baselines/<tag>/` (tracked, repo root — NOT under .deal), and writes
/// `evidence/baselines/<tag>/manifest.json` recording, per sim:
///   - content_hash: SHA-256 over the output.json bytes (D-82/D-83)
///   - reproducibility_tier: from metadata.json (or "unknown" if absent)
///   - tool: from metadata.json
///   - tool_version: from metadata.json
///   - verdict: from metadata.json (provisional PARTIAL until Plan 06 backfills)
///
/// Plus top-level: tag, timestamp, v:1. All BTreeMap keys alphabetical (D-18).
/// Uses write_file_atomic for all writes (tmp+rename).
pub fn run_evidence_baseline(tag: &str) -> Result<(), CliError> {
    let cwd = std::env::current_dir().map_err(|e| {
        CliError::Internal(anyhow::anyhow!("cannot get current directory: {}", e))
    })?;
    run_evidence_baseline_in(&cwd, tag)
}

/// Baseline implementation parameterised on project root (testable).
pub fn run_evidence_baseline_in(project_root: &Path, tag: &str) -> Result<(), CliError> {
    let evidence_dir = project_root.join(".deal").join("evidence");

    // evidence/baselines/<tag>/ is at repo root — NOT under .deal (D-81).
    let baseline_dir = project_root.join("evidence").join("baselines").join(tag);

    if !evidence_dir.exists() {
        return Err(CliError::User(format!(
            "deal evidence baseline: no .deal/evidence/ directory found — run `deal simulate` first"
        )));
    }

    std::fs::create_dir_all(&baseline_dir).map_err(|e| {
        CliError::Internal(anyhow::anyhow!(
            "cannot create baseline dir {:?}: {}",
            baseline_dir,
            e
        ))
    })?;

    // Scan evidence dir for simulation subdirectories.
    let entries = std::fs::read_dir(&evidence_dir).map_err(|e| {
        CliError::Internal(anyhow::anyhow!("cannot read {:?}: {}", evidence_dir, e))
    })?;

    // BTreeMap ensures alphabetical key order in manifest (D-18 byte-stability).
    let mut sim_manifest: BTreeMap<String, serde_json::Value> = BTreeMap::new();

    for entry in entries {
        let entry = entry.map_err(|e| {
            CliError::Internal(anyhow::anyhow!("directory entry error: {}", e))
        })?;
        let sim_path = entry.path();
        if !sim_path.is_dir() {
            continue;
        }
        let sim_name = sim_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if sim_name.is_empty() {
            continue;
        }

        let output_src = sim_path.join("output.json");
        let metadata_src = sim_path.join("metadata.json");

        // Read output.json for content hash (skip sims that haven't run yet).
        if !output_src.exists() {
            continue;
        }
        let output_bytes = std::fs::read(&output_src).map_err(|e| {
            CliError::Internal(anyhow::anyhow!("read {:?}: {}", output_src, e))
        })?;

        // SHA-256 content hash (D-82 / T-05-12 tamper detection).
        let content_hash = {
            let mut hasher = Sha256::new();
            hasher.update(&output_bytes);
            hex::encode(hasher.finalize())
        };

        // Read metadata.json for tool, tool_version, reproducibility_tier, verdict.
        let (tool, tool_version, reproducibility_tier, verdict) =
            read_metadata_fields(&metadata_src);

        // Copy output.json into baseline/<sim-name>/ (frozen snapshot).
        let baseline_sim_dir = baseline_dir.join(&sim_name);
        std::fs::create_dir_all(&baseline_sim_dir).map_err(|e| {
            CliError::Internal(anyhow::anyhow!("create {:?}: {}", baseline_sim_dir, e))
        })?;
        write_file_atomic(&baseline_sim_dir.join("output.json"), &output_bytes)?;

        // BTreeMap per-sim entry (alphabetical keys: D-18).
        let mut sim_entry: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        sim_entry.insert("content_hash".to_string(), serde_json::Value::String(content_hash));
        sim_entry.insert("reproducibility_tier".to_string(), serde_json::Value::String(reproducibility_tier));
        sim_entry.insert("tool".to_string(), serde_json::Value::String(tool));
        sim_entry.insert("tool_version".to_string(), serde_json::Value::String(tool_version));
        sim_entry.insert("verdict".to_string(), serde_json::Value::String(verdict));

        sim_manifest.insert(sim_name, serde_json::Value::Object(
            sim_entry.into_iter().collect(),
        ));
    }

    if sim_manifest.is_empty() {
        return Err(CliError::User(format!(
            "deal evidence baseline: no simulation output.json files found in .deal/evidence/ — run `deal simulate` first"
        )));
    }

    // Build manifest (BTreeMap top-level keys for D-18 byte-stability).
    let timestamp = chrono_like_timestamp();
    let mut manifest: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    manifest.insert("sims".to_string(), serde_json::Value::Object(
        sim_manifest.into_iter().collect(),
    ));
    manifest.insert("tag".to_string(), serde_json::Value::String(tag.to_string()));
    manifest.insert("timestamp".to_string(), serde_json::Value::String(timestamp));
    manifest.insert("v".to_string(), serde_json::Value::Number(1.into()));

    let manifest_val = serde_json::Value::Object(manifest.into_iter().collect());
    let manifest_bytes = serde_json::to_vec_pretty(&manifest_val).map_err(|e| {
        CliError::Internal(anyhow::anyhow!("serialize manifest: {}", e))
    })?;
    write_file_atomic(&baseline_dir.join("manifest.json"), &manifest_bytes)?;

    let _ = writeln!(
        std::io::stderr(),
        "deal evidence baseline: wrote baseline '{}' to evidence/baselines/{}/",
        tag, tag
    );

    Ok(())
}

// ─── Helpers ───────────────────────────────────────────────────────────────────

/// Read tool, tool_version, reproducibility_tier, and verdict from a metadata.json file.
///
/// Returns provisional defaults if the file is absent or fields are missing.
/// Verdict defaults to "PARTIAL" per D-82 (Plan 06 backfills real PASS/FAIL/PARTIAL).
fn read_metadata_fields(metadata_path: &Path) -> (String, String, String, String) {
    let defaults = (
        "unknown".to_string(),
        "unknown".to_string(),
        "unknown".to_string(),
        DEFAULT_VERDICT.to_string(),
    );

    if !metadata_path.exists() {
        return defaults;
    }

    let bytes = match std::fs::read(metadata_path) {
        Ok(b) => b,
        Err(_) => return defaults,
    };
    let val: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => return defaults,
    };

    let tool = val["tool"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let tool_version = val["tool_version"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let reproducibility_tier = val["reproducibility_tier"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    // Verdict: read from metadata if present and valid, otherwise default to PARTIAL.
    let verdict = {
        let v = val["verdict"].as_str().unwrap_or(DEFAULT_VERDICT);
        if VALID_VERDICTS.contains(&v) {
            v.to_string()
        } else {
            DEFAULT_VERDICT.to_string()
        }
    };

    (tool, tool_version, reproducibility_tier, verdict)
}

/// Write `data` to `path` atomically using a temp file + rename.
///
/// Creates temp file in the same directory (same filesystem, enabling POSIX atomic rename).
fn write_file_atomic(path: &Path, data: &[u8]) -> Result<(), CliError> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let fname = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let tmp_path = parent.join(format!(
        ".deal_ev_tmp_{}_{}",
        fname,
        std::process::id()
    ));

    {
        let mut tmp_file = std::fs::File::create(&tmp_path).map_err(|e| {
            CliError::Internal(anyhow::anyhow!(
                "cannot create temp file {:?}: {}",
                tmp_path,
                e
            ))
        })?;
        tmp_file.write_all(data).map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            CliError::Internal(anyhow::anyhow!(
                "cannot write temp file {:?}: {}",
                tmp_path,
                e
            ))
        })?;
    }

    std::fs::rename(&tmp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        CliError::Internal(anyhow::anyhow!(
            "cannot rename {:?} to {:?}: {}",
            tmp_path,
            path,
            e
        ))
    })?;

    Ok(())
}

/// Simple RFC-3339 timestamp without pulling in chrono.
fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let (year, month, day) = days_to_ymd(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, h, m, s
    )
}

/// Gregorian calendar decomposition (good until year 2100).
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // 400-year cycle = 146097 days
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

// ─── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_no_evidence_dir_succeeds() {
        // When .deal/evidence/ doesn't exist, capture should succeed (nothing to do).
        let tmpdir = tempfile::tempdir().expect("tempdir");
        let result = run_evidence_capture_in(tmpdir.path());
        assert!(result.is_ok(), "capture with no evidence dir should succeed");
    }

    #[test]
    fn capture_copies_artifacts() {
        let tmpdir = tempfile::tempdir().expect("tempdir");
        let root = tmpdir.path();

        // Create .deal/evidence/sim_a/ with output.json + metadata.json
        let sim_dir = root.join(".deal").join("evidence").join("sim_a");
        std::fs::create_dir_all(&sim_dir).unwrap();
        std::fs::write(sim_dir.join("output.json"), b"{\"x\": 1}").unwrap();
        std::fs::write(sim_dir.join("metadata.json"), b"{\"tool\": \"python\"}").unwrap();

        run_evidence_capture_in(root).expect("capture should succeed");

        // Verify captured artifacts exist
        assert!(
            root.join(".deal").join("captured").join("sim_a").join("output.json").exists(),
            "captured output.json must exist"
        );
        assert!(
            root.join(".deal").join("captured").join("index.json").exists(),
            "captured index.json must exist"
        );
    }

    #[test]
    fn capture_index_uses_btreemap() {
        let tmpdir = tempfile::tempdir().expect("tempdir");
        let root = tmpdir.path();

        // Create two sim dirs
        for name in &["zebra_sim", "alpha_sim"] {
            let sim_dir = root.join(".deal").join("evidence").join(name);
            std::fs::create_dir_all(&sim_dir).unwrap();
            std::fs::write(sim_dir.join("output.json"), b"{\"v\": 1}").unwrap();
        }

        run_evidence_capture_in(root).expect("capture should succeed");

        let index_bytes =
            std::fs::read(root.join(".deal").join("captured").join("index.json")).unwrap();
        let index_str = std::str::from_utf8(&index_bytes).unwrap();

        // alpha_sim should appear before zebra_sim in the JSON string (D-18 alphabetical)
        let alpha_pos = index_str.find("alpha_sim").unwrap_or(usize::MAX);
        let zebra_pos = index_str.find("zebra_sim").unwrap_or(usize::MAX);
        assert!(
            alpha_pos < zebra_pos,
            "index.json keys must be alphabetical (D-18): alpha_sim before zebra_sim"
        );
    }

    #[test]
    fn read_metadata_defaults_when_absent() {
        let tmpdir = tempfile::tempdir().expect("tempdir");
        let absent = tmpdir.path().join("metadata.json");
        let (tool, tool_version, tier, verdict) = read_metadata_fields(&absent);
        assert_eq!(tool, "unknown");
        assert_eq!(tool_version, "unknown");
        assert_eq!(tier, "unknown");
        assert_eq!(verdict, DEFAULT_VERDICT);
    }

    #[test]
    fn read_metadata_parses_fields() {
        let tmpdir = tempfile::tempdir().expect("tempdir");
        let meta_path = tmpdir.path().join("metadata.json");
        let meta = serde_json::json!({
            "tool": "python",
            "tool_version": "3.11.0",
            "reproducibility_tier": "strict",
            "verdict": "PASS",
        });
        std::fs::write(&meta_path, serde_json::to_vec(&meta).unwrap()).unwrap();
        let (tool, tool_version, tier, verdict) = read_metadata_fields(&meta_path);
        assert_eq!(tool, "python");
        assert_eq!(tool_version, "3.11.0");
        assert_eq!(tier, "strict");
        assert_eq!(verdict, "PASS");
    }

    #[test]
    fn read_metadata_invalid_verdict_defaults_to_partial() {
        let tmpdir = tempfile::tempdir().expect("tempdir");
        let meta_path = tmpdir.path().join("metadata.json");
        let meta = serde_json::json!({
            "tool": "python",
            "tool_version": "3.11.0",
            "reproducibility_tier": "tolerant",
            "verdict": "INVALID_VALUE",
        });
        std::fs::write(&meta_path, serde_json::to_vec(&meta).unwrap()).unwrap();
        let (_, _, _, verdict) = read_metadata_fields(&meta_path);
        assert_eq!(verdict, DEFAULT_VERDICT, "invalid verdict must default to PARTIAL");
    }
}
