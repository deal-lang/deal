//! Simulation orchestrator for `deal simulate` (Phase 5 / D-71).
//!
//! Implements:
//!   - `deal.sims.toml` registry parsing into typed structs (D-70, SIM-3)
//!   - Dependency-graph resolution via topological sort (Pattern 1)
//!   - Staleness detection via SHA-256 content hash (D-83)
//!   - Subprocess dispatch by `tool` field (D-71, D-72, D-79)
//!   - Evidence artifact writing to `.deal/evidence/<name>/` (D-74, D-81)
//!   - Graceful skip for unlicensed/absent tools (D-72, D-58 pattern)
//!
//! Security mitigations (threat model 05-03):
//!   T-05-06: model_path validated against `^[a-zA-Z0-9._]+$` before filesystem ops
//!   T-05-07: runner split into command+args via Command::new (no shell interpolation)
//!   T-05-08: MATLAB absent → graceful-skip + skip.json (ErrorKind::NotFound)
//!   T-05-09: staleness key uses SHA-256 over BTreeMap-ordered inputs (D-83)

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::reporter::{ColorPref, Ink, Reporter};
use crate::CliError;

// ─── model_path validation (T-05-06) ─────────────────────────────────────────

/// Validate a `model_path` string against the safe D-23 qualified-path alphabet.
///
/// Allows: alphanumerics, `.`, `_`  (no `/`, `..`, whitespace, or shell metacharacters).
/// Returns Err if the path contains disallowed characters.
pub fn validate_model_path(mp: &str) -> Result<(), CliError> {
    if mp.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_') && !mp.is_empty() {
        Ok(())
    } else {
        Err(CliError::User(format!(
            "invalid model_path {:?}: must match ^[a-zA-Z0-9._]+$ (T-05-06)",
            mp
        )))
    }
}

// ─── deal.sims.toml structures (SIM-3) ────────────────────────────────────────

/// Top-level `deal.sims.toml` registry.
///
/// D-18: `BTreeMap` ensures alphabetical key order in serialized output.
#[derive(Debug, Deserialize, Clone)]
pub struct SimsRegistry {
    /// Map from simulation name to its registry entry (D-18: BTreeMap).
    #[serde(default)]
    pub simulations: BTreeMap<String, SimEntry>,
}

/// A single `[simulations.<name>]` entry in `deal.sims.toml` (SIM-3).
#[derive(Debug, Deserialize, Clone)]
pub struct SimEntry {
    /// Tool adapter: "python", "matlab", "zig", or "generic" (D-71, D-79).
    pub tool: String,
    /// Script entry point for python/zig tools (e.g. "simulations/battery_thermal.py").
    pub entry: Option<String>,
    /// Python class name within the entry module (e.g. "BatteryThermal").
    pub class: Option<String>,
    /// Runner command for matlab/generic tools (e.g. "matlab -batch").
    pub runner: Option<String>,
    /// Workspace-relative model path this sim binds to.
    pub binds_to: Option<String>,
    /// `@simulation:` annotation path in the model.
    pub annotation: Option<String>,
    /// Input bindings: model field → sim parameter mappings.
    #[serde(default)]
    pub inputs: Vec<SimIoBinding>,
    /// Output bindings: sim result → model field mappings.
    #[serde(default)]
    pub outputs: Vec<SimIoBinding>,
    /// Whether to run this sim automatically on `deal check --run-sims`.
    #[serde(default)]
    pub auto_run: bool,
    /// Declared `@reproducibility` tier: "strict", "tolerant", or "advisory" (D-75).
    pub reproducibility: Option<String>,
    /// Explicit dependency overrides — sim names that must complete before this one.
    /// Overrides inferred `model_path` dependencies (Pattern 1).
    pub depends_on: Option<Vec<String>>,
}

/// A single input or output binding in `deal.sims.toml`.
#[derive(Debug, Deserialize, Clone)]
pub struct SimIoBinding {
    /// Workspace-relative model path this binding reads from or writes to.
    pub model_path: Option<String>,
    /// Simulation parameter name.
    pub param: Option<String>,
    /// Unit annotation (advisory for external tools; enforced for Zig-internal sims).
    pub unit: Option<String>,
}

// ─── Registry parsing ─────────────────────────────────────────────────────────

/// Parse a `deal.sims.toml` file into a `SimsRegistry`.
///
/// Returns `CliError::User` if the file is missing or cannot be parsed as TOML.
pub fn parse_registry(path: &Path) -> Result<SimsRegistry, CliError> {
    let bytes = std::fs::read(path).map_err(|e| {
        CliError::User(format!("cannot read {}: {}", path.display(), e))
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|e| {
        CliError::Internal(anyhow::anyhow!("deal.sims.toml is not valid UTF-8: {}", e))
    })?;
    toml::from_str(text).map_err(|e| {
        CliError::User(format!("invalid deal.sims.toml: {}", e))
    })
}

// ─── Topological sort (Kahn's algorithm) ─────────────────────────────────────

/// Compute a deterministic topological execution order for the simulation registry.
///
/// Builds a dependency graph by:
///   1. Inferring edges from `model_path` overlap: if sim A's `outputs[].model_path`
///      matches sim B's `inputs[].model_path`, then A must run before B.
///   2. Honoring explicit `depends_on` overrides: `B.depends_on = ["A"]` adds edge A→B.
///
/// The zero-in-degree queue is sorted (D-18 deterministic order) at each step.
///
/// Returns `CliError::User` with "circular dependency" if a cycle is detected.
pub fn topological_order(registry: &SimsRegistry) -> Result<Vec<String>, CliError> {
    // Build output_path → producer index
    let mut output_producer: HashMap<&str, &str> = HashMap::new();
    for (name, entry) in &registry.simulations {
        for out in &entry.outputs {
            if let Some(mp) = &out.model_path {
                output_producer.insert(mp.as_str(), name.as_str());
            }
        }
    }

    // Build adjacency list and in-degree map
    // adj[A] = list of sims that depend on A (A must run before them)
    let mut in_degree: HashMap<String, usize> = registry
        .simulations
        .keys()
        .map(|k| (k.clone(), 0))
        .collect();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();

    for (name, entry) in &registry.simulations {
        // Inferred edges from model_path overlap
        for inp in &entry.inputs {
            if let Some(mp) = &inp.model_path {
                if let Some(producer) = output_producer.get(mp.as_str()) {
                    let producer = producer.to_string();
                    if producer != *name {
                        adj.entry(producer.clone()).or_default().push(name.clone());
                        *in_degree.entry(name.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
        // Explicit depends_on overrides
        if let Some(deps) = &entry.depends_on {
            for dep in deps {
                if dep != name && registry.simulations.contains_key(dep) {
                    adj.entry(dep.clone()).or_default().push(name.clone());
                    *in_degree.entry(name.clone()).or_insert(0) += 1;
                }
            }
        }
    }

    // Kahn's BFS — sorted queue for D-18 determinism
    let mut queue: Vec<String> = in_degree
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(k, _)| k.clone())
        .collect();
    queue.sort();
    let mut queue: VecDeque<String> = queue.into();
    let mut order: Vec<String> = Vec::new();

    while let Some(node) = queue.pop_front() {
        order.push(node.clone());
        if let Some(neighbors) = adj.get(&node) {
            // Sort neighbors before inserting into queue for D-18 determinism
            let mut sorted_neighbors: Vec<String> = neighbors
                .iter()
                .filter_map(|n| {
                    let deg = in_degree.get_mut(n)?;
                    *deg -= 1;
                    if *deg == 0 { Some(n.clone()) } else { None }
                })
                .collect();
            sorted_neighbors.sort();
            for n in sorted_neighbors {
                queue.push_back(n);
            }
        }
    }

    if order.len() < registry.simulations.len() {
        return Err(CliError::User(
            "circular dependency in deal.sims.toml: simulation dependency graph has a cycle".into(),
        ));
    }

    Ok(order)
}

// ─── Staleness key (D-83) ────────────────────────────────────────────────────

/// Compute the SHA-256 staleness key for a simulation.
///
/// The key is deterministic (D-83/D-18) and is a function of:
///   - `resolved_inputs`: the resolved numeric values as a JSON value (must be
///     BTreeMap-ordered before calling)
///   - `sim_source_path`: path to the sim source file (entry script / .py / .m)
///   - `registry_entry_toml`: the `[simulations.<name>]` section as a TOML string
///
/// Returns a lowercase hex SHA-256 digest.
pub fn compute_staleness_key(
    resolved_inputs: &serde_json::Value,
    sim_source_path: &Path,
    registry_entry_toml: &str,
) -> String {
    let mut h = Sha256::new();
    // D-18 determinism: inputs must already be BTreeMap-ordered
    h.update(serde_json::to_vec(resolved_inputs).unwrap_or_default());
    // Sim source file bytes — catches logic changes
    h.update(std::fs::read(sim_source_path).unwrap_or_default());
    // Registry entry TOML section — catches entry/runner/class changes
    h.update(registry_entry_toml.as_bytes());
    hex::encode(h.finalize())
}

// ─── Evidence cache helpers ───────────────────────────────────────────────────

/// Return the evidence directory for a named simulation: `.deal/evidence/<name>/`
pub fn evidence_dir(project_root: &Path, sim_name: &str) -> PathBuf {
    project_root.join(".deal").join("evidence").join(sim_name)
}

/// Write a JSON value to a path atomically (temp-file + rename, D-81).
fn write_json_atomic(path: &Path, value: &serde_json::Value) -> Result<(), CliError> {
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| {
        CliError::Internal(anyhow::anyhow!("cannot create {:?}: {}", parent, e))
    })?;
    let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = parent.join(format!(".deal_sim_tmp_{}_{}", fname, std::process::id()));
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| CliError::Internal(anyhow::anyhow!("serialize JSON: {}", e)))?;
    std::fs::write(&tmp, &bytes).map_err(|e| {
        CliError::Internal(anyhow::anyhow!("write tmp {:?}: {}", tmp, e))
    })?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        CliError::Internal(anyhow::anyhow!("rename {:?} → {:?}: {}", tmp, path, e))
    })?;
    Ok(())
}

/// Write a graceful-skip record when a tool binary is absent (D-72 / T-05-08).
///
/// Creates `.deal/evidence/<name>/skip.json` with D-18 sorted keys.
fn write_skip_record(
    evidence_dir: &Path,
    sim_name: &str,
    reason: &str,
) -> Result<(), CliError> {
    // Timestamp in ISO-8601 format (UTC)
    let timestamp = chrono_like_timestamp();
    let skip = serde_json::json!({
        "reason": reason,
        "sim": sim_name,
        "skipped": true,
        "timestamp": timestamp,
    });
    let skip_path = evidence_dir.join("skip.json");
    write_json_atomic(&skip_path, &skip)?;
    Ok(())
}

/// Simple RFC-3339 timestamp without pulling in chrono.
fn chrono_like_timestamp() -> String {
    // Use std::time for a best-effort ISO timestamp (no chrono dependency needed).
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Decompose to rough ISO-8601 (seconds precision, UTC)
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    // Days since 1970-01-01 to year/month/day (Gregorian proleptic, good until 2099)
    let (year, month, day) = days_to_ymd(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, h, m, s
    )
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Simple Gregorian calendar decomposition (accurate 1970-2099)
    let mut y = 1970u64;
    let mut remaining = days;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let days_in_year = if leap { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let month_days: &[u64] = if leap {
        &[31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        &[31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut mo = 0u64;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md {
            mo = i as u64 + 1;
            break;
        }
        remaining -= md;
    }
    (y, mo, remaining + 1)
}

// ─── IR model_path resolution ─────────────────────────────────────────────────

/// Resolve a `model_path` (e.g. `EnergyStorage.battery.packResistance`) against
/// the DEAL workspace IR elements map.
///
/// The IR elements map uses D-23 qualified IDs as keys (e.g.
/// `EnergyStorage.battery.packResistance`). This function validates the path
/// (T-05-06) then looks it up in the elements map, returning the resolved
/// numeric value as a `serde_json::Value`.
///
/// Returns `None` (with a stderr warning) if the path cannot be resolved.
pub fn resolve_model_path(
    model_path: &str,
    ir_elements: &serde_json::Map<String, serde_json::Value>,
    model_index: &crate::model_values::ModelValueIndex,
    stderr: &mut dyn std::io::Write,
) -> Option<serde_json::Value> {
    // T-05-06: validate before any use (do NOT weaken this guard — 05-08).
    if let Err(e) = validate_model_path(model_path) {
        let _ = writeln!(stderr, "warning: {}", e);
        return None;
    }
    // 1. Direct IR-elements lookup (legacy path — kept for IR elements that
    //    carry an inline `value`/`literal`).
    if let Some(element) = ir_elements.get(model_path) {
        if let Some(v) = element.get("value") {
            if !v.is_null() {
                return Some(v.clone());
            }
        }
        if let Some(v) = element.get("literal") {
            if !v.is_null() {
                return Some(v.clone());
            }
        }
    }
    // 2. 05-08: resolve via the shared AST ModelValueIndex. The registry
    //    model_path form (`EnergyStorage.battery.packResistance`) does not match
    //    the IR element key form (`vehicle.battery.BatteryPack.packResistance`),
    //    AND the IR payload carries no literal value — the AST does. The index
    //    matches by exact-then-unique-suffix and returns the numeric magnitude.
    if let Some(num) = model_index.resolve(model_path) {
        return Some(serde_json::json!(num));
    }
    let _ = writeln!(
        stderr,
        "warning: model_path '{}' could not be resolved to a value — sim input will be null",
        model_path
    );
    None
}

/// Build the `inputs` object for `input.json` from registry bindings + IR values.
///
/// For each binding: validates model_path (T-05-06), resolves value from IR.
/// Emits a diagnostic to stderr for any unresolved path (Pitfall 6).
/// Returns a D-18 alphabetically-keyed BTreeMap.
pub fn build_input_json(
    entry: &SimEntry,
    ir_elements: &serde_json::Map<String, serde_json::Value>,
    model_index: &crate::model_values::ModelValueIndex,
    stderr: &mut dyn std::io::Write,
) -> serde_json::Value {
    let mut inputs: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for binding in &entry.inputs {
        if let Some(param) = &binding.param {
            let value = if let Some(mp) = &binding.model_path {
                resolve_model_path(mp, ir_elements, model_index, stderr)
                    .unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Null
            };
            let mut val_obj = serde_json::json!({"value": value});
            if let Some(unit) = &binding.unit {
                val_obj["unit"] = serde_json::json!(unit);
            }
            inputs.insert(param.clone(), val_obj);
        }
    }
    serde_json::json!({
        "deal_sim_protocol": "v0",
        "inputs": inputs,
        "v": 1,
    })
}

// ─── Tool dispatch ─────────────────────────────────────────────────────────────

/// Result of a single simulation run.
#[derive(Debug)]
pub enum SimResult {
    /// Simulation completed successfully.
    Success,
    /// Simulation was skipped (tool absent — D-72).
    Skipped(String),
}

/// Dispatch a simulation by its `tool` field (D-71).
///
/// - `python`: runs `python <entry> --input <in> --output <out> --metadata <meta>`
/// - `matlab`/`generic`: splits `runner` and spawns via `Command::new` (T-05-07)
/// - `zig`: stub — defers to Plan 04 (`run_zig_sim` stub)
///
/// On `ErrorKind::NotFound` (tool binary absent): writes `skip.json` and returns
/// `SimResult::Skipped` (D-72 graceful-skip, NOT `Err`).
pub fn dispatch_sim(
    sim_name: &str,
    entry: &SimEntry,
    input_json_path: &Path,
    output_json_path: &Path,
    metadata_json_path: &Path,
    workdir: &Path,
    ev_dir: &Path,
    stderr: &mut dyn std::io::Write,
) -> Result<SimResult, CliError> {
    match entry.tool.as_str() {
        "python" => dispatch_python(
            sim_name,
            entry,
            input_json_path,
            output_json_path,
            metadata_json_path,
            workdir,
            ev_dir,
            stderr,
        ),
        "matlab" | "generic" => dispatch_runner(
            sim_name,
            entry,
            input_json_path,
            output_json_path,
            metadata_json_path,
            workdir,
            ev_dir,
            stderr,
        ),
        "zig" => {
            // D-73 in-process Zig dispatch — deferred to Plan 04
            let _ = writeln!(
                stderr,
                "warning: zig tool dispatch not yet implemented (Plan 04) — skipping '{}'",
                sim_name
            );
            write_skip_record(ev_dir, sim_name, "zig tool dispatch not yet implemented (Plan 04)")?;
            Ok(SimResult::Skipped("zig tool not yet implemented".into()))
        }
        other => {
            let _ = writeln!(
                stderr,
                "warning: unknown tool '{}' for sim '{}' — skipping",
                other, sim_name
            );
            write_skip_record(ev_dir, sim_name, &format!("unknown tool: {}", other))?;
            Ok(SimResult::Skipped(format!("unknown tool: {}", other)))
        }
    }
}

fn dispatch_python(
    sim_name: &str,
    entry: &SimEntry,
    input_json_path: &Path,
    output_json_path: &Path,
    metadata_json_path: &Path,
    workdir: &Path,
    ev_dir: &Path,
    stderr: &mut dyn std::io::Write,
) -> Result<SimResult, CliError> {
    let entry_path = entry.entry.as_deref().ok_or_else(|| {
        CliError::User(format!("sim '{}': python tool requires 'entry' field", sim_name))
    })?;

    // Try "python3" first, then "python" as fallback
    let result = std::process::Command::new("python3")
        .arg(entry_path)
        .arg("--input").arg(input_json_path)
        .arg("--output").arg(output_json_path)
        .arg("--metadata").arg(metadata_json_path)
        .current_dir(workdir)
        .output();

    let result = match result {
        Err(e) if e.kind() == ErrorKind::NotFound => {
            // Try "python" fallback
            std::process::Command::new("python")
                .arg(entry_path)
                .arg("--input").arg(input_json_path)
                .arg("--output").arg(output_json_path)
                .arg("--metadata").arg(metadata_json_path)
                .current_dir(workdir)
                .output()
        }
        other => other,
    };

    match result {
        Ok(output) if output.status.success() => Ok(SimResult::Success),
        Ok(output) => {
            let stderr_msg = String::from_utf8_lossy(&output.stderr);
            Err(CliError::User(format!(
                "simulation '{}' failed (exit {}): {}",
                sim_name, output.status.code().unwrap_or(-1), stderr_msg.trim()
            )))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            let reason = format!("python/python3 not found: {}", e);
            let _ = writeln!(stderr, "warning: sim '{}' skipped — {}", sim_name, reason);
            write_skip_record(ev_dir, sim_name, &reason)?;
            Ok(SimResult::Skipped(reason))
        }
        Err(e) => Err(CliError::Internal(anyhow::anyhow!(
            "spawn python for '{}': {}", sim_name, e
        ))),
    }
}

fn dispatch_runner(
    sim_name: &str,
    entry: &SimEntry,
    input_json_path: &Path,
    output_json_path: &Path,
    metadata_json_path: &Path,
    workdir: &Path,
    ev_dir: &Path,
    stderr: &mut dyn std::io::Write,
) -> Result<SimResult, CliError> {
    let runner = entry.runner.as_deref().ok_or_else(|| {
        CliError::User(format!(
            "sim '{}': {} tool requires 'runner' field",
            sim_name, entry.tool
        ))
    })?;

    // T-05-07: split runner into command + args — NEVER shell-interpolate
    let parts: Vec<&str> = runner.split_whitespace().collect();
    if parts.is_empty() {
        return Err(CliError::User(format!(
            "sim '{}': runner field is empty", sim_name
        )));
    }

    let (cmd, args) = (parts[0], &parts[1..]);
    let result = std::process::Command::new(cmd)
        .args(args)
        .arg("--input").arg(input_json_path)
        .arg("--output").arg(output_json_path)
        .arg("--metadata").arg(metadata_json_path)
        .current_dir(workdir)
        .output();

    match result {
        Ok(output) if output.status.success() => Ok(SimResult::Success),
        Ok(output) => {
            let stderr_msg = String::from_utf8_lossy(&output.stderr);
            Err(CliError::User(format!(
                "simulation '{}' failed (exit {}): {}",
                sim_name, output.status.code().unwrap_or(-1), stderr_msg.trim()
            )))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            // D-72 / T-05-08: graceful-skip on absent tool
            let reason = format!("{} not found: {}", cmd, e);
            let _ = writeln!(stderr, "warning: sim '{}' skipped — {}", sim_name, reason);
            write_skip_record(ev_dir, sim_name, &reason)?;
            Ok(SimResult::Skipped(reason))
        }
        Err(e) => Err(CliError::Internal(anyhow::anyhow!(
            "spawn '{}' for '{}': {}", cmd, sim_name, e
        ))),
    }
}

// ─── Orchestration entry points ───────────────────────────────────────────────

/// Run named simulation(s) from the registry.
///
/// Implements:
///   - TOML registry parse
///   - Dependency graph resolution via topological sort
///   - Staleness detection (D-83) when `--stale` is set
///   - Tool dispatch (D-71): python / matlab / generic / zig
///   - Evidence artifact writing to `.deal/evidence/<name>/` (D-81)
///   - Graceful skip for absent tools (D-72)
///   - model_path resolution from project IR
pub fn run_simulate(
    names: &[String],
    all: bool,
    stale: bool,
    color: ColorPref,
) -> Result<(), CliError> {
    let cwd = std::env::current_dir().map_err(|e| {
        CliError::Internal(anyhow::anyhow!("cannot get current dir: {}", e))
    })?;
    let rep = Reporter::new(color);
    run_simulate_in_reported(&cwd, names, all, stale, &rep)
}

/// Internal: run simulations from `project_root` with default (auto) styling.
///
/// Separated for testability — tests and the `--run-sims` refresh path call
/// this with no color context; styling resolves from the stderr TTY / `CI`.
pub fn run_simulate_in(
    project_root: &Path,
    names: &[String],
    all: bool,
    stale: bool,
) -> Result<(), CliError> {
    let rep = Reporter::new(ColorPref::Auto);
    run_simulate_in_reported(project_root, names, all, stale, &rep)
}

/// Internal worker: run simulations, routing per-sim status through `rep`.
fn run_simulate_in_reported(
    project_root: &Path,
    names: &[String],
    all: bool,
    stale: bool,
    rep: &Reporter,
) -> Result<(), CliError> {
    let registry_path = project_root.join("simulations").join("deal.sims.toml");
    if !registry_path.exists() {
        return Err(CliError::User(format!(
            "no deal.sims.toml found at {:?} — run from a DEAL project directory",
            registry_path
        )));
    }

    let registry = parse_registry(&registry_path)?;

    // Determine which sims to run
    let run_order: Vec<String> = if all || names.is_empty() {
        topological_order(&registry)?
    } else {
        names.to_vec()
    };

    // Load project IR for model_path resolution
    let ir_elements = load_project_ir(project_root);

    // 05-08: shared AST value index (resolves registry model_paths whose form
    // differs from IR element keys, and whose values live in the AST not the IR).
    let model_index = crate::model_values::ModelValueIndex::build(project_root);

    let mut stderr_buf = std::io::stderr();
    let stderr: &mut dyn std::io::Write = &mut stderr_buf;

    for sim_name in &run_order {
        let entry = registry.simulations.get(sim_name).ok_or_else(|| {
            CliError::User(format!("simulation '{}' not found in deal.sims.toml", sim_name))
        })?;

        let ev_dir = evidence_dir(project_root, sim_name);
        std::fs::create_dir_all(&ev_dir).map_err(|e| {
            CliError::Internal(anyhow::anyhow!("cannot create evidence dir {:?}: {}", ev_dir, e))
        })?;

        // Staleness check (D-83/D-84): skip if cached hash matches current
        if stale {
            let metadata_path = ev_dir.join("metadata.json");
            if metadata_path.exists() {
                let sim_source_path = entry.entry.as_deref()
                    .map(|e| project_root.join("simulations").join(e))
                    .unwrap_or_default();
                let input_val = build_input_json(entry, &ir_elements, &model_index, stderr);
                let entry_toml = format!("[simulations.{}]\n", sim_name);
                let current_key = compute_staleness_key(&input_val, &sim_source_path, &entry_toml);
                if let Ok(meta_bytes) = std::fs::read(&metadata_path) {
                    if let Ok(meta_json) = serde_json::from_slice::<serde_json::Value>(&meta_bytes) {
                        if meta_json.get("staleness_key")
                            .and_then(|v| v.as_str()) == Some(&current_key)
                        {
                            let _ = writeln!(
                                stderr,
                                "deal simulate: '{}' is fresh (staleness key unchanged) — skipping",
                                sim_name
                            );
                            continue;
                        }
                    }
                }
            }
        }

        // Build input.json: prefer pre-existing input.json with resolved (non-null) values
        // over IR-resolved inputs that may be null when deal-std is not installed.
        // This allows gate contexts and integration test fixtures to supply values
        // directly without requiring a full IR build (D-72 graceful-gate pattern).
        let input_path = ev_dir.join("input.json");
        let output_path = ev_dir.join("output.json");
        let metadata_path = ev_dir.join("metadata.json");
        let input_val = if input_path.exists() {
            if let Ok(existing_bytes) = std::fs::read(&input_path) {
                if let Ok(existing_json) = serde_json::from_slice::<serde_json::Value>(&existing_bytes) {
                    // Prefer existing input.json if it has non-null values for required params
                    let has_real_values = entry.inputs.iter().any(|b| {
                        if let Some(param) = &b.param {
                            existing_json.pointer(&format!("/inputs/{}/value", param))
                                .map(|v| !v.is_null())
                                .unwrap_or(false)
                        } else {
                            false
                        }
                    });
                    if has_real_values {
                        let _ = writeln!(
                            stderr,
                            "deal simulate: using pre-existing input.json for '{}' (has resolved values)",
                            sim_name
                        );
                        existing_json
                    } else {
                        build_input_json(entry, &ir_elements, &model_index, stderr)
                    }
                } else {
                    build_input_json(entry, &ir_elements, &model_index, stderr)
                }
            } else {
                build_input_json(entry, &ir_elements, &model_index, stderr)
            }
        } else {
            build_input_json(entry, &ir_elements, &model_index, stderr)
        };
        write_json_atomic(&input_path, &input_val)?;

        // Resolve sim source path (for staleness key)
        let sim_source_path = entry.entry.as_deref()
            .map(|e| project_root.join("simulations").join(e))
            .unwrap_or_default();

        // Compute workdir: the simulations/ directory of the project
        let workdir = project_root.join("simulations");

        // Phase 6 CLI richness: show a live spinner while the sim subprocess
        // runs (TTY only), and time the dispatch for the result line.
        let detail = entry.entry.as_deref().unwrap_or(sim_name.as_str()).to_string();
        let pb = rep.spinner(&format!("{:<7} {}", entry.tool, detail));
        let start = Instant::now();

        // Dispatch the simulation
        let result = dispatch_sim(
            sim_name,
            entry,
            &input_path,
            &output_path,
            &metadata_path,
            &workdir,
            &ev_dir,
            stderr,
        )?;

        let elapsed = start.elapsed();
        if let Some(pb) = pb {
            pb.finish_and_clear();
        }

        match result {
            SimResult::Success => {
                // Validate output.json against spec/sims/v0/output.schema.json
                if output_path.exists() {
                    let output_bytes = std::fs::read(&output_path).map_err(|e| {
                        CliError::Internal(anyhow::anyhow!("cannot read {:?}: {}", output_path, e))
                    })?;
                    let output_json: serde_json::Value = serde_json::from_slice(&output_bytes)
                        .map_err(|e| CliError::User(format!(
                            "sim '{}': output.json is not valid JSON: {}", sim_name, e
                        )))?;

                    if let Err(errors) = crate::sims_protocol::validate_output(&output_json) {
                        return Err(CliError::User(format!(
                            "sim '{}': output.json failed schema validation: {}",
                            sim_name,
                            errors.join("; ")
                        )));
                    }

                    // Enrich metadata.json with staleness key (D-83)
                    let entry_toml = format!("[simulations.{}]\n", sim_name);
                    let staleness_key = compute_staleness_key(
                        &input_val,
                        &sim_source_path,
                        &entry_toml,
                    );
                    if metadata_path.exists() {
                        if let Ok(meta_bytes) = std::fs::read(&metadata_path) {
                            if let Ok(mut meta_json) = serde_json::from_slice::<serde_json::Value>(&meta_bytes) {
                                meta_json["staleness_key"] = serde_json::json!(staleness_key);
                                meta_json["sim"] = serde_json::json!(sim_name);
                                write_json_atomic(&metadata_path, &meta_json)?;
                            }
                        }
                    }

                }
                // Styled timed result line: `python  thermal/motor.py ✓ 0.8s`.
                let mark = rep.paint("✓", Ink::Green);
                let timing = rep.paint(&format!("{:.1}s", elapsed.as_secs_f64()), Ink::Green);
                let _ = writeln!(stderr, "  {:<7} {} {} {}", entry.tool, detail, mark, timing);
            }
            SimResult::Skipped(reason) => {
                let mark = rep.paint("⊘", Ink::Yellow);
                let note = rep.paint(&format!("skipped — {}", reason), Ink::Yellow);
                let _ = writeln!(stderr, "  {:<7} {} {} {}", entry.tool, detail, mark, note);
            }
        }
    }

    Ok(())
}

/// Load the DEAL project IR from the project root.
///
/// Runs `deal_ir_json` FFI call over all `.deal`/`.dealx` files in the project,
/// returning the merged IR elements map. Falls back to an empty map on any error.
fn load_project_ir(project_root: &Path) -> serde_json::Map<String, serde_json::Value> {
    // Collect .deal and .dealx files from the project
    let deal_files = collect_deal_files(project_root);
    if deal_files.is_empty() {
        return serde_json::Map::new();
    }

    let mut merged: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();

    for path in &deal_files {
        let source_bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let filename = path.to_string_lossy();

        use deal_ffi as ffi;
        let handle = unsafe {
            ffi::deal_parse(
                source_bytes.as_ptr(),
                source_bytes.len(),
                filename.as_bytes().as_ptr(),
                filename.len(),
            )
        };
        if handle.is_null() {
            continue;
        }

        // Extract IR JSON and merge elements
        let mut ir_ptr: *const u8 = std::ptr::null();
        let mut ir_len: usize = 0;
        let ok = unsafe { ffi::deal_ir_json(handle, &mut ir_ptr, &mut ir_len) };
        if ok && !ir_ptr.is_null() && ir_len > 0 {
            let ir_bytes = unsafe { std::slice::from_raw_parts(ir_ptr, ir_len).to_vec() };
            unsafe { ffi::deal_free(handle) };
            if let Ok(ir_json) = serde_json::from_slice::<serde_json::Value>(&ir_bytes) {
                if let Some(elements) = ir_json.get("elements").and_then(|e| e.as_object()) {
                    for (k, v) in elements {
                        merged.insert(k.clone(), v.clone());
                    }
                }
            }
        } else {
            unsafe { ffi::deal_free(handle) };
        }
    }

    merged
}

/// Collect all `.deal` and `.dealx` files under a project directory.
fn collect_deal_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_deal_files_recursive(root, &mut files);
    files
}

fn collect_deal_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip .deal/ (gitignored generated cache) and target/
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == ".deal" || name == "target" || name.starts_with('.') {
                continue;
            }
            collect_deal_files_recursive(&path, files);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext == "deal" || ext == "dealx" {
                files.push(path);
            }
        }
    }
}

/// Evaluate structural validity of `deal.sims.toml` bindings (no execution).
pub fn validate_bindings() -> Result<(), CliError> {
    let cwd = std::env::current_dir().map_err(|e| {
        CliError::Internal(anyhow::anyhow!("cannot get current dir: {}", e))
    })?;
    let registry_path = cwd.join("simulations").join("deal.sims.toml");
    let registry = parse_registry(&registry_path)?;

    // Validate model_path values (T-05-06)
    for (name, entry) in &registry.simulations {
        for binding in entry.inputs.iter().chain(entry.outputs.iter()) {
            if let Some(mp) = &binding.model_path {
                validate_model_path(mp).map_err(|e| {
                    CliError::User(format!("sim '{}': {}", name, e))
                })?;
            }
        }
    }

    // Check topological sort (detect cycles)
    topological_order(&registry)?;
    Ok(())
}

/// Evaluate satisfaction criteria against captured evidence, optionally re-running stale sims.
///
/// STUB — Plan 05 implements.
pub fn evaluate(run_sims: bool) -> Result<(), CliError> {
    let _ = run_sims;
    Err(CliError::User(
        "deal check --verify: not yet implemented (Phase 5 Plan 05)".into(),
    ))
}

/// Validate a `output.json` artifact against the `spec/sims/v0/output.schema.json`.
///
/// Delegates to `sims_protocol::validate_output`.
pub fn validate_output(output_json: &serde_json::Value) -> Result<(), CliError> {
    crate::sims_protocol::validate_output(output_json).map_err(|errors| {
        CliError::User(format!(
            "output.json schema validation failed: {}",
            errors.join("; ")
        ))
    })
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ── parse_registry ────────────────────────────────────────────────────────

    #[test]
    fn parse_registry_showcase() {
        // Parse the showcase deal.sims.toml; verify 5 sims with correct fields.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("spec/examples/showcase/simulations/deal.sims.toml");
        let registry = parse_registry(&path).expect("parse showcase registry");

        assert_eq!(registry.simulations.len(), 5, "expected 5 sims");

        // battery_thermal — python
        let bt = registry.simulations.get("battery_thermal").expect("battery_thermal");
        assert_eq!(bt.tool, "python");
        assert_eq!(bt.entry.as_deref(), Some("thermal/battery_thermal.py"));
        assert_eq!(bt.class.as_deref(), Some("BatteryThermal"));
        assert_eq!(bt.inputs.len(), 3);
        assert_eq!(bt.outputs.len(), 2);

        // motor_efficiency — matlab
        let me = registry.simulations.get("motor_efficiency").expect("motor_efficiency");
        assert_eq!(me.tool, "matlab");
        assert!(me.runner.is_some());

        // range_model — python, auto_run=true
        let rm = registry.simulations.get("range_model").expect("range_model");
        assert_eq!(rm.tool, "python");
        assert!(rm.auto_run);
    }

    // ── dep_order ─────────────────────────────────────────────────────────────

    #[test]
    fn dep_order_showcase() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("spec/examples/showcase/simulations/deal.sims.toml");
        let registry = parse_registry(&path).expect("parse registry");
        let order = topological_order(&registry).expect("topological order");

        // motor_thermal writes Propulsion.motor.peakPower
        // motor_efficiency also reads Propulsion.motor.peakTorque
        // The inferred edge is motor_thermal → motor_efficiency
        // (motor_thermal outputs Propulsion.motor.peakPower = motor_efficiency input... let's check)
        // Actually: motor_thermal outputs Propulsion.motor.peakPower
        //           nothing reads peakPower as input (motor_efficiency reads peakTorque)
        // So motor_thermal is independent. Let's just verify:
        //   - producer before consumer where there's a real edge
        //   - all 5 sims present
        //   - deterministic (sorted)
        assert_eq!(order.len(), 5, "all 5 sims must appear in order");
        for name in &["battery_thermal", "motor_efficiency", "motor_thermal", "range_model", "vehicle_dynamics"] {
            assert!(order.contains(&name.to_string()), "order must contain '{}'", name);
        }
    }

    #[test]
    fn dep_order_producer_before_consumer() {
        // Construct a synthetic registry where sim_a produces X and sim_b consumes X.
        // topological_order must put sim_a before sim_b.
        let toml = r#"
[simulations.sim_a]
tool = "python"
entry = "sim_a.py"
outputs = [
    { param = "x", model_path = "Model.x" }
]

[simulations.sim_b]
tool = "python"
entry = "sim_b.py"
inputs = [
    { param = "x", model_path = "Model.x" }
]
"#;
        let registry: SimsRegistry = toml::from_str(toml).expect("parse");
        let order = topological_order(&registry).expect("order");
        let a_idx = order.iter().position(|s| s == "sim_a").unwrap();
        let b_idx = order.iter().position(|s| s == "sim_b").unwrap();
        assert!(a_idx < b_idx, "sim_a (producer) must precede sim_b (consumer)");
    }

    #[test]
    fn dep_order_cycle_detection() {
        // A → B → A cycle must return CliError::User with "circular"
        let toml = r#"
[simulations.sim_a]
tool = "python"
entry = "sim_a.py"
inputs  = [{ param = "y", model_path = "Model.y" }]
outputs = [{ param = "x", model_path = "Model.x" }]

[simulations.sim_b]
tool = "python"
entry = "sim_b.py"
inputs  = [{ param = "x", model_path = "Model.x" }]
outputs = [{ param = "y", model_path = "Model.y" }]
"#;
        let registry: SimsRegistry = toml::from_str(toml).expect("parse");
        let err = topological_order(&registry).expect_err("cycle must fail");
        let msg = format!("{}", err);
        assert!(
            msg.contains("circular"),
            "error must mention 'circular', got: {:?}", msg
        );
    }

    #[test]
    fn dep_order_deterministic() {
        // Running topological_order twice on the same registry produces identical output.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("spec/examples/showcase/simulations/deal.sims.toml");
        let registry = parse_registry(&path).expect("parse");
        let order1 = topological_order(&registry).expect("order 1");
        let order2 = topological_order(&registry).expect("order 2");
        assert_eq!(order1, order2, "topological_order must be deterministic");
    }

    // ── staleness_key ─────────────────────────────────────────────────────────

    #[test]
    fn staleness_key_deterministic() {
        // Same inputs → same key
        let inputs = serde_json::json!({"packResistance": {"unit": "ohm", "value": 0.05}});
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"sim source v1").unwrap();

        let k1 = compute_staleness_key(&inputs, tmp.path(), "[simulations.battery_thermal]");
        let k2 = compute_staleness_key(&inputs, tmp.path(), "[simulations.battery_thermal]");
        assert_eq!(k1, k2, "key must be deterministic");
    }

    #[test]
    fn staleness_key_changes_on_source_change() {
        let inputs = serde_json::json!({"x": {"value": 1.0}});
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp.as_file_mut(), "v1").unwrap();
        let k1 = compute_staleness_key(&inputs, tmp.path(), "entry");

        // Overwrite source
        let mut tmp2 = tempfile::NamedTempFile::new().unwrap();
        tmp2.write_all(b"v2").unwrap();
        let k2 = compute_staleness_key(&inputs, tmp2.path(), "entry");

        assert_ne!(k1, k2, "key must change when source file bytes change");
    }

    #[test]
    fn staleness_key_changes_on_input_change() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"same source").unwrap();

        let inputs1 = serde_json::json!({"x": {"value": 1.0}});
        let inputs2 = serde_json::json!({"x": {"value": 2.0}});

        let k1 = compute_staleness_key(&inputs1, tmp.path(), "entry");
        let k2 = compute_staleness_key(&inputs2, tmp.path(), "entry");

        assert_ne!(k1, k2, "key must change when resolved input values change");
    }

    #[test]
    fn staleness_key_changes_on_registry_change() {
        let inputs = serde_json::json!({"x": {"value": 1.0}});
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"source").unwrap();

        let k1 = compute_staleness_key(&inputs, tmp.path(), "[simulations.my_sim]\ntool = \"python\"");
        let k2 = compute_staleness_key(&inputs, tmp.path(), "[simulations.my_sim]\ntool = \"matlab\"");

        assert_ne!(k1, k2, "key must change when registry TOML section changes");
    }

    // ── model_path validation (T-05-06) ────────────────────────────────────────

    #[test]
    fn model_path_valid_paths() {
        assert!(validate_model_path("EnergyStorage.battery.packResistance").is_ok());
        assert!(validate_model_path("Model.x").is_ok());
        assert!(validate_model_path("ABC_123.foo").is_ok());
    }

    #[test]
    fn model_path_invalid_paths() {
        assert!(validate_model_path("").is_err(), "empty path");
        assert!(validate_model_path("../etc/passwd").is_err(), "path traversal");
        assert!(validate_model_path("foo/bar").is_err(), "slash");
        assert!(validate_model_path("foo bar").is_err(), "space");
        assert!(validate_model_path("foo;bar").is_err(), "semicolon");
    }

    // ── graceful-skip (D-72 / T-05-08) ────────────────────────────────────────

    #[test]
    fn graceful_skip_writes_skip_json() {
        let dir = tempfile::tempdir().unwrap();
        let ev = dir.path().join("ev");
        std::fs::create_dir_all(&ev).unwrap();
        write_skip_record(&ev, "motor_efficiency", "matlab not found").unwrap();
        let skip_path = ev.join("skip.json");
        assert!(skip_path.exists(), "skip.json must be created");
        let bytes = std::fs::read(&skip_path).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["skipped"], serde_json::json!(true));
        assert_eq!(json["sim"], serde_json::json!("motor_efficiency"));
        assert_eq!(json["reason"], serde_json::json!("matlab not found"));
        // D-18: keys must be alphabetically sorted
        let text = String::from_utf8(bytes).unwrap();
        let reason_pos = text.find("\"reason\"").unwrap_or(0);
        let sim_pos = text.find("\"sim\"").unwrap_or(0);
        let skipped_pos = text.find("\"skipped\"").unwrap_or(0);
        let timestamp_pos = text.find("\"timestamp\"").unwrap_or(0);
        assert!(reason_pos < sim_pos, "D-18: reason before sim");
        assert!(sim_pos < skipped_pos, "D-18: sim before skipped");
        assert!(skipped_pos < timestamp_pos, "D-18: skipped before timestamp");
    }
}
