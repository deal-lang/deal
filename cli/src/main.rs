//! DEAL compiler driver — CLI entry point.
//!
//! Implements subcommands: `check` (Plan 02-02), `build` (Plan 02-04),
//! `fmt` (Plan 02-05), `parse` (Plan 02-06), `init` (Plan 04-02), and
//! `install` (Plan 04-02).
//!
//! Global flags --json / --color / --verbose are declared here as wired
//! infrastructure per D-32 (envelope) and D-34 (exit codes).
//!
//! Exit codes (D-34):
//!   0 — success (all files clean)
//!   1 — user-visible error (CliError::User — ≥1 error-severity diagnostic)
//!   2 — internal error (CliError::Internal — FFI failure, OOM, file I/O error)

use clap::{Parser, Subcommand, ValueEnum};
use std::io::Write;

// Plan 03-03 Task 1: cli/src/ffi.rs was deleted; the 9-export FFI surface is
// now owned by the shared `deal-ffi` crate (single `links = "deal"` claimant).
// This re-export keeps every existing `ffi::deal_*` call-site compiling
// without per-site edits.
use deal_ffi as ffi;
pub mod closure;
pub mod evidence;
pub mod model_values;
pub mod render;
pub mod reporter;
pub mod reqif;
pub mod reqif_schema;
pub mod resolver;
pub mod schema_registry;
pub mod sims_protocol;
pub mod simulate;
pub mod sysml_v2;
pub mod verify;

// CliError is shared with the library target for use in integration tests.
// It is defined here in main.rs (the binary root); modules compiled under
// the binary crate context use `crate::CliError` which resolves here.
// Integration tests use `deal::CliError` via the library target (see lib.rs
// which re-exports a compatible definition).

/// Single source of truth for the default deal-stdlib git tag (D-67).
/// Updated in Plan 04 to the tag chosen by Plan 03 (deal-stdlib v0.4.0).
/// Keep as a `const` so all references in run_init compile to the same literal.
const DEFAULT_STDLIB_TAG: &str = "v0.4.0";

/// Typed CLI error: User errors exit 1; Internal errors exit 2 (D-34).
#[derive(Debug)]
pub enum CliError {
    /// A user-visible error (e.g. invalid argument, blocking diagnostic).
    /// CLI prints the message to stderr and exits 1.
    User(String),
    /// An internal error (e.g. not-yet-implemented, FFI failure, OOM).
    /// CLI prints the error chain to stderr and exits 2.
    Internal(anyhow::Error),
}

impl CliError {
    /// Returns true if this is a user-visible error (exit 1).
    pub fn is_user(&self) -> bool {
        matches!(self, CliError::User(_))
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::User(msg) => write!(f, "{}", msg),
            CliError::Internal(e) => write!(f, "internal error: {:#}", e),
        }
    }
}

#[derive(Parser)]
#[command(name = "deal", version, about = "DEAL language compiler driver")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Emit machine-readable JSON instead of human-readable output.
    #[arg(long, global = true)]
    json: bool,

    /// When to use color output.
    #[arg(long, global = true, default_value = "auto")]
    color: ColorMode,

    /// Increase output verbosity.
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Parse source files and emit AST JSON to stdout.
    /// On success: emits the raw alphabetical-keyed AST JSON directly to stdout
    /// (no D-32 envelope — per WARNING-05 Option A; payloads are emitted raw).
    /// On parse error: emits diagnostics to stderr (envelope if --json), exits 1.
    Parse { paths: Vec<std::path::PathBuf> },
    /// Run semantic checks; exit 1 on any blocking diagnostic.
    Check {
        paths: Vec<std::path::PathBuf>,
        /// Evaluate verification criteria against captured evidence (SIM-5 / D-85).
        /// Uses deal_check_with_stdlib C ABI for dimension compatibility (D-85).
        #[arg(long)]
        verify: bool,
        /// Validate deal.sims.toml bindings structurally (no simulation execution).
        #[arg(long)]
        simulations: bool,
        /// When combined with --verify, re-run stale simulations before evaluating (D-84 opt-in).
        #[arg(long)]
        run_sims: bool,
    },
    /// Format source files in place (or stdin → stdout).
    Fmt {
        paths: Vec<std::path::PathBuf>,
        /// Check if files would be reformatted (exit 1 if yes); no files modified.
        #[arg(long)]
        check: bool,
        /// Write formatted output to stdout instead of editing files in-place.
        #[arg(long)]
        stdout: bool,
    },
    /// Build a target (SysML v2 JSON for Phase 2).
    /// (Plan 02-04 implements — currently exits 2)
    Build {
        #[arg(long)]
        target: BuildTarget,
        /// Run offline schema validation on the output.
        #[arg(long)]
        validate: bool,
        /// Override the output path (default: inferred from repo root, D-24).
        #[arg(long)]
        output: Option<std::path::PathBuf>,
        paths: Vec<std::path::PathBuf>,
    },
    /// Scaffold a new DEAL project in ./<name>/ (D-67/D-69).
    ///
    /// Creates the PS-8 directory layout, a deal.toml wiring deal-std as a
    /// git dependency, and a starter model (part def, port def, composition,
    /// requirement with satisfy block) that parses cleanly before `deal install`.
    Init {
        /// Project name (used as both the directory name and `[project].name`).
        name: Option<String>,
    },
    /// Resolve dependencies declared in deal.toml and write deal.lock (D-66/D-68).
    ///
    /// Git deps are cloned into `.deal/deps/<name>/` at the exact tag/rev/branch.
    /// Path deps are referenced in-place without copying.
    /// Generates a deterministic, SHA-pinned deal.lock (D-18 alphabetical order).
    Install,
    /// Run one or more simulations registered in deal.sims.toml (SIM-4 / D-71).
    ///
    /// Dispatches each simulation by its `tool` field (python/matlab/zig/generic),
    /// resolving execution order via dependency graph (Pattern 1, D-83 staleness).
    Simulate {
        /// Simulation name(s) to run. Omit to run all (requires --all).
        names: Vec<String>,
        /// Run all registered simulations in dependency order.
        #[arg(long)]
        all: bool,
        /// Re-run only simulations with stale evidence (D-83/D-84).
        #[arg(long)]
        stale: bool,
    },
    /// Evidence capture and baseline management (D-81, D-82).
    Evidence {
        #[command(subcommand)]
        subcommand: evidence::EvidenceCommand,
    },
}

#[derive(ValueEnum, Clone)]
enum BuildTarget {
    SysmlV2,
    /// ReqIF 1.2 .reqifz archive (D-61 / REQ-phase-4-2).
    Reqif,
}

#[derive(ValueEnum, Clone, Copy)]
enum ColorMode {
    Auto,
    Always,
    Never,
}

/// Compute a color choice for `anstream` from `--color` flag.
fn color_choice(mode: ColorMode) -> anstream::ColorChoice {
    match mode {
        ColorMode::Auto => anstream::ColorChoice::Auto,
        ColorMode::Always => anstream::ColorChoice::Always,
        ColorMode::Never => anstream::ColorChoice::Never,
    }
}

/// Map the CLI `--color` flag to the reporter's color preference (the reporter
/// lives in the library crate and has its own preference enum).
fn color_pref(mode: ColorMode) -> reporter::ColorPref {
    match mode {
        ColorMode::Auto => reporter::ColorPref::Auto,
        ColorMode::Always => reporter::ColorPref::Always,
        ColorMode::Never => reporter::ColorPref::Never,
    }
}

/// Run the `deal check` subcommand.
///
/// For each path: read source, call FFI parse, collect diagnostics.
/// If `--json`: emit D-32 envelope on stdout.
/// If not `--json`: render human-readable diagnostics on stderr.
///
/// Exit codes per D-34:
///   0 — zero error-severity diagnostics across all files.
///   1 — ≥1 error-severity diagnostic (User error).
///   2 — I/O failure, FFI failure (Internal error).
fn run_check(
    paths: &[std::path::PathBuf],
    json_mode: bool,
    color: ColorMode,
) -> Result<(), CliError> {
    // E2402: check for declared dependencies that have not been installed (Pitfall 4).
    // If the caller supplied a directory arg, look for deal.toml in that directory;
    // otherwise look in the current working directory.
    // Full cross-file import resolution through vendored sources lands in Plan 04 —
    // here only the not-installed guard is added.
    {
        // WR-02: locate the project root by walking up from each input path
        // looking for `deal.toml`, rather than relying on a directory argument
        // being present. The previous logic used the first directory arg (or `.`
        // if none), so `deal check packages/foo.deal` from outside the project
        // root silently skipped the E2402 not-installed gate.
        let check_root: std::path::PathBuf =
            find_deal_toml_root(paths).unwrap_or_else(|| std::path::PathBuf::from("."));
        let toml_path = check_root.join("deal.toml");
        if toml_path.exists() {
            if let Ok(toml_bytes) = std::fs::read(&toml_path) {
                if let Ok(toml_str) = std::str::from_utf8(&toml_bytes) {
                    if let Ok(manifest) = toml::from_str::<resolver::DealToml>(toml_str) {
                        let deps_base = check_root.join(".deal").join("deps");
                        let stderr_choice = color_choice(color);
                        let mut stderr =
                            anstream::AutoStream::new(std::io::stderr(), stderr_choice);
                        let mut missing = false;
                        for (name, dep) in &manifest.dependencies {
                            if matches!(dep, resolver::Dependency::Git { .. }) {
                                let dep_dir = deps_base.join(name);
                                if !dep_dir.exists() {
                                    let _ = writeln!(
                                        stderr,
                                        "error[E2402]: dependency '{name}' not resolved — run 'deal install'"
                                    );
                                    missing = true;
                                }
                            }
                        }
                        if missing {
                            return Err(CliError::User(String::new()));
                        }
                    }
                }
            }
        }
    }

    // ── ADR-0004 P4 (WS-D): closure-driven loading ──────────────────────────
    //
    // Replaces the former "flat-merge the whole tree + all deps into one blob"
    // model with entry-point + package-complete import-closure loading: discover
    // the project (workspace roots) and dependency roots, build a module map,
    // pick entry points (Decision 4), compute the reachable-package closure, and
    // analyze ONLY the project files in that closure (each with externals =
    // closure ∖ self). Unreachable, cleanly-parsed files are not analyzed.
    // Discover + plan the entry-point import closure (shared with build/emit).
    let plan = plan_load_from_paths(paths)?;

    // Pre-read every closure file's source once for per-file external assembly.
    let mut source_cache: std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> =
        std::collections::BTreeMap::new();
    for p in &plan.closure {
        if let Ok(b) = std::fs::read(p) {
            source_cache.insert(p.clone(), b);
        }
    }

    // (Dependency sources — git deps under .deal/deps and path deps — are now
    // discovered as `dep_files` above and enter the analysis only through the
    // package-complete closure, replacing the former flat `stdlib_bytes` blob.)

    // Workspace root for `.deal/index.json`. Only set when the caller passed
    // an explicit directory arg (signaling workspace-mode intent). Single-file
    // invocations (`deal check foo.deal`) skip index emission entirely —
    // writing per-file index dirs scattered across the tree would be noise,
    // not a workspace index. Phase 3 LSP / explicit `--workspace` flag will
    // generalize this; for Phase 2 closeout the directory-arg convention
    // exactly matches SPEC §criterion 1's example (`deal check tests/showcase/`).
    let workspace_root: Option<std::path::PathBuf> = paths.iter().find(|p| p.is_dir()).cloned();

    // Collect per-file results.
    let mut all_diagnostics: Vec<serde_json::Value> = Vec::new();
    let mut per_file_indexes: Vec<Vec<u8>> = Vec::new();
    let mut any_errors = false;

    // Stderr stream for human-mode output.
    let stderr_choice = color_choice(color);
    let mut stderr = anstream::AutoStream::new(std::io::stderr(), stderr_choice);

    for path in &plan.analyze {
        let source_bytes = match source_cache.get(path) {
            Some(b) => b.clone(),
            None => std::fs::read(path).map_err(|e| {
                CliError::Internal(anyhow::anyhow!("cannot read {:?}: {}", path, e))
            })?,
        };
        let filename = path.to_string_lossy();

        // External table = every OTHER closure file's source, NUL-separated.
        // Calling deal_check_with_stdlib — even with an empty external blob —
        // puts sema in STRICT workspace mode: import-scoped resolution, E2000
        // on un-imported cross-file references (ADR-0004 P3/P4).
        let externals = externals_for(path, &plan.closure, &source_cache);

        // SAFETY: all slices live for the duration of the call; null on OOM.
        let handle = unsafe {
            let mut _out_diag_ptr: *const u8 = std::ptr::null();
            let mut _out_diag_len: usize = 0;
            ffi::deal_check_with_stdlib(
                source_bytes.as_ptr(),
                source_bytes.len(),
                filename.as_bytes().as_ptr(),
                filename.len(),
                externals.as_ptr(),
                externals.len(),
                &mut _out_diag_ptr,
                &mut _out_diag_len,
            )
        };
        if handle.is_null() {
            return Err(CliError::Internal(anyhow::anyhow!(
                "deal_check_with_stdlib returned null for {:?} (OOM)",
                path
            )));
        }

        // T-02-13 / T-05-10 (Clone-Before-Free): clone diagnostic JSON BEFORE
        // deal_free — the arena-owned pointer is invalid afterward.
        let diag_json_owned: Vec<u8>;
        let has_errors: bool;
        unsafe {
            has_errors = ffi::deal_has_errors(handle);
            let mut out_ptr: *const u8 = std::ptr::null();
            let mut out_len: usize = 0;
            let ok = ffi::deal_diagnostics_json(handle, &mut out_ptr, &mut out_len);
            if !ok {
                ffi::deal_free(handle);
                return Err(CliError::Internal(anyhow::anyhow!(
                    "deal_diagnostics_json OOM for {:?}",
                    path
                )));
            }
            diag_json_owned = std::slice::from_raw_parts(out_ptr, out_len).to_vec();
            ffi::deal_free(handle);
        }

        // Index: reuse the map's already-parsed envelope (produced by deal_parse,
        // which owns its arena) instead of calling deal_index_json on the check
        // handle (whose entries alias the now-freed external arena — D-88 hazard).
        // Files that produced no symbol table contribute nothing.
        if let Some(m) = plan.map.modules.get(path) {
            if !m.index_json.is_empty() {
                per_file_indexes.push(m.index_json.clone());
            }
        }

        if has_errors {
            any_errors = true;
        }

        let diag_array: serde_json::Value =
            serde_json::from_slice(&diag_json_owned).map_err(|e| {
                CliError::Internal(anyhow::anyhow!("diagnostic JSON parse error: {}", e))
            })?;
        let diags = match diag_array.as_array() {
            Some(a) => a,
            None => {
                return Err(CliError::Internal(anyhow::anyhow!(
                    "deal_diagnostics_json did not return a JSON array"
                )));
            }
        };

        if json_mode {
            all_diagnostics.extend(diags.iter().cloned());
        } else {
            let source_str = std::str::from_utf8(&source_bytes).unwrap_or("");
            for diag in diags {
                render::render_diagnostic(&mut stderr, source_str, diag)
                    .map_err(|e| CliError::Internal(anyhow::anyhow!("render error: {}", e)))?;
            }
        }
    }

    if json_mode {
        // Assemble the D-32 envelope.
        // DO NOT re-serialize diagnostics through serde_json::Value (Pitfall 1
        // risk + RESEARCH §Anti-Patterns). Instead, emit raw JSON.
        // Envelope: {"command":"check","deal_version":"...","diagnostics":[...],
        //            "summary":{...},"v":1}
        // Alphabetical keys: command, deal_version, diagnostics, summary, v.

        let deal_version = env!("CARGO_PKG_VERSION");

        // Count errors/warnings/hints from collected diagnostics.
        let mut err_count: u64 = 0;
        let mut warn_count: u64 = 0;
        let mut hint_count: u64 = 0;
        for d in &all_diagnostics {
            match d["severity"].as_str().unwrap_or("") {
                "err" => err_count += 1,
                "warn" => warn_count += 1,
                "hint" | "info" => hint_count += 1,
                _ => {}
            }
        }

        // Emit diagnostics array as raw JSON (pass-through from Zig).
        let diags_json = serde_json::to_string(&all_diagnostics)
            .map_err(|e| CliError::Internal(anyhow::anyhow!("JSON serialization error: {}", e)))?;

        // Build summary JSON with alphabetical keys (errors, hints, warnings).
        let summary_json = format!(
            r#"{{"errors":{},"hints":{},"warnings":{}}}"#,
            err_count, hint_count, warn_count
        );

        // Assemble envelope with alphabetical top-level keys (D-32).
        // command, deal_version, diagnostics, summary, v
        // Use write! through stdout so no println! escapes (RESEARCH §Anti-Patterns).
        let stdout_choice = color_choice(ColorMode::Never); // JSON mode: no color
        let mut stdout = anstream::AutoStream::new(std::io::stdout(), stdout_choice);
        write!(
            stdout,
            r#"{{"command":"check","deal_version":"{}","diagnostics":{},"summary":{},"v":1}}"#,
            deal_version, diags_json, summary_json
        )
        .map_err(|e| CliError::Internal(anyhow::anyhow!("stdout write error: {}", e)))?;
        writeln!(stdout)
            .map_err(|e| CliError::Internal(anyhow::anyhow!("stdout write error: {}", e)))?;
    }

    // Workspace symbol index — SPEC §criterion 1: `deal check tests/showcase/`
    // exits 0 AND writes a valid `.deal/index.json`. Each per-file index is
    // already alphabetical-keyed by `writeIndexJson` in src/json.zig; the
    // merge unions the `elements` maps and concatenates `imports_graph`
    // edges. BTreeMap preserves D-18 alphabetical-key invariant.
    // Phase 3 LSP can consume this as the workspace symbol index without a
    // separate import step.
    if let Some(workspace_root) =
        workspace_root.filter(|_| !any_errors && !per_file_indexes.is_empty())
    {
        let mut merged_elements = std::collections::BTreeMap::<String, serde_json::Value>::new();
        let mut merged_imports: Vec<serde_json::Value> = Vec::new();
        let mut deal_version: Option<String> = None;
        for bytes in &per_file_indexes {
            let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| {
                CliError::Internal(anyhow::anyhow!("per-file index JSON parse error: {}", e))
            })?;
            if deal_version.is_none() {
                if let Some(s) = v.get("deal_version").and_then(|x| x.as_str()) {
                    deal_version = Some(s.to_string());
                }
            }
            if let Some(elems) = v.get("elements").and_then(|x| x.as_object()) {
                for (k, val) in elems {
                    merged_elements.insert(k.clone(), val.clone());
                }
            }
            if let Some(imports) = v.get("imports_graph").and_then(|x| x.as_array()) {
                merged_imports.extend(imports.iter().cloned());
            }
        }
        let workspace_index = serde_json::json!({
            "deal_version": deal_version.unwrap_or_else(|| "0.1.0-phase2".to_string()),
            "elements": merged_elements,
            "imports_graph": merged_imports,
            "v": 1,
        });
        let index_dir = workspace_root.join(".deal");
        std::fs::create_dir_all(&index_dir).map_err(|e| {
            CliError::Internal(anyhow::anyhow!("cannot create {:?}: {}", index_dir, e))
        })?;
        let index_path = index_dir.join("index.json");
        let serialized = serde_json::to_vec(&workspace_index)
            .map_err(|e| CliError::Internal(anyhow::anyhow!("workspace index serialize: {}", e)))?;
        std::fs::write(&index_path, &serialized)
            .map_err(|e| CliError::Internal(anyhow::anyhow!("write {:?}: {}", index_path, e)))?;
    }

    // Phase 6 CLI richness: on a clean human-mode run, emit the staged summary
    // header instead of a silent exit 0. Skipped in --json mode (the envelope
    // was already written to stdout) and whenever blocking diagnostics were
    // rendered to stderr above.
    if !json_mode && !any_errors {
        let rep = reporter::Reporter::new(color_pref(color));
        let project_file_count = plan.analyze.len();
        // Count resolved symbols across per-file indexes (display only).
        let mut symbol_count = 0usize;
        for bytes in &per_file_indexes {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) {
                if let Some(elems) = v.get("elements").and_then(|x| x.as_object()) {
                    symbol_count += elems.len();
                }
            }
        }
        let _ = rep.banner(
            &mut stderr,
            &format!(
                "checking {} file{}",
                project_file_count,
                if project_file_count == 1 { "" } else { "s" }
            ),
        );
        let rows = vec![
            ("parse", format!("{} files", project_file_count), Some("ok")),
            ("resolve", format!("{} symbols", symbol_count), Some("ok")),
            ("units", "dimensional algebra".to_string(), Some("ok")),
        ];
        let _ = rep.phases(&mut stderr, &rows);
    }

    if any_errors {
        Err(CliError::User(String::new()))
    } else {
        Ok(())
    }
}

/// Run the `deal fmt` subcommand.
///
/// Behavior matrix:
///   `deal fmt foo.deal`          — format in-place atomically (temp+rename)
///   `deal fmt --stdout foo.deal` — write formatted output to stdout
///   `deal fmt -` (or no paths)   — read stdin, write to stdout
///   `deal fmt --check foo.deal`  — check-only; exit 1 if would change
///   `deal fmt --json foo.deal`   — emit D-32 envelope for any diagnostics
///
/// FS-3 identity gate: requires `git config user.name` to be set.
///
/// Exit codes (D-34):
///   0 — success (all files already canonical, or formatted in-place)
///   1 — user error (parse/sema error, or --check found a file that needs formatting)
///   2 — internal error (FFI failure, I/O error, missing git config)
fn run_fmt(
    paths: &[std::path::PathBuf],
    check: bool,
    stdout_mode: bool,
    json_mode: bool,
    color: ColorMode,
) -> Result<(), CliError> {
    // FS-3: identity gate — require git config user.name/email.
    check_fs3_identity()?;

    let stderr_choice = color_choice(color);
    let mut stderr = anstream::AutoStream::new(std::io::stderr(), stderr_choice);
    let stdout_choice = color_choice(ColorMode::Never); // stdout: no color codes in source bytes

    // Determine the list of (source_bytes, filename, destination) tuples.
    // stdin mode: paths is empty OR contains a single "-".
    let stdin_mode = paths.is_empty() || (paths.len() == 1 && paths[0].to_str() == Some("-"));

    if stdin_mode {
        // Read all of stdin.
        let mut source_bytes = Vec::new();
        use std::io::Read;
        std::io::stdin()
            .read_to_end(&mut source_bytes)
            .map_err(|e| CliError::Internal(anyhow::anyhow!("stdin read error: {}", e)))?;
        let filename = "<stdin>";
        let formatted = fmt_source(&source_bytes, filename, json_mode, &mut stderr)?;
        // Always write to stdout in stdin mode.
        let mut out = anstream::AutoStream::new(std::io::stdout(), stdout_choice);
        out.write_all(&formatted)
            .map_err(|e| CliError::Internal(anyhow::anyhow!("stdout write error: {}", e)))?;
        return Ok(());
    }

    let mut would_change = false;

    for path in paths {
        let source_bytes = std::fs::read(path)
            .map_err(|e| CliError::Internal(anyhow::anyhow!("cannot read {:?}: {}", path, e)))?;
        let filename = path.to_string_lossy().into_owned();
        let formatted = fmt_source(&source_bytes, &filename, json_mode, &mut stderr)?;

        if stdout_mode {
            // Write formatted bytes to stdout; leave file untouched.
            let mut out = anstream::AutoStream::new(std::io::stdout(), stdout_choice);
            out.write_all(&formatted)
                .map_err(|e| CliError::Internal(anyhow::anyhow!("stdout write error: {}", e)))?;
        } else if check {
            // Check mode: compare formatted vs source; report mismatch but don't write.
            if formatted != source_bytes {
                would_change = true;
                let _ = writeln!(stderr, "would reformat: {}", filename);
            }
        } else {
            // In-place mode: write atomically (temp file + rename).
            if formatted != source_bytes {
                write_file_atomic(path, &formatted)?;
            }
        }
    }

    if check && would_change {
        Err(CliError::User(String::new()))
    } else {
        Ok(())
    }
}

/// FS-3 identity gate: check that git config user.name is set to a real value.
///
/// WR-07: this is a provenance gate, so it must fail closed:
///   - The stdout is trimmed and must be non-empty after trimming. The previous
///     `!output.stdout.is_empty()` accepted a lone newline / whitespace as a
///     valid identity.
///   - A missing `git` binary (or git invocation failure) is treated as a
///     FAILURE rather than success. The old "missing git = OK" branch made the
///     gate trivially bypassable by simply running outside a repo.
fn check_fs3_identity() -> Result<(), CliError> {
    let result = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output();
    match result {
        Ok(output) if output.status.success() => {
            let name = String::from_utf8_lossy(&output.stdout);
            if name.trim().is_empty() {
                Err(CliError::Internal(anyhow::anyhow!(
                    "deal fmt requires git config user.name to be set to a non-empty value per FS-3"
                )))
            } else {
                Ok(())
            }
        }
        Ok(_) => Err(CliError::Internal(anyhow::anyhow!(
            "deal fmt requires git config user.name/email to be set per FS-3"
        ))),
        Err(_) => {
            // WR-07: fail closed. A provenance gate must not pass when the
            // committer identity cannot be established (git missing or failing).
            Err(CliError::Internal(anyhow::anyhow!(
                "deal fmt requires git to verify committer identity (FS-3); \
                 git is not available or failed to run"
            )))
        }
    }
}

/// Parse `source_bytes` with the given `filename`, call deal_format, and
/// return the formatted bytes (cloned from the arena before deal_free).
///
/// On parse errors: if `json_mode` is true, emits the D-32 envelope to stdout
/// and returns a User error with an empty message (exit 1). Otherwise renders
/// human-readable diagnostics to `stderr`.
fn fmt_source(
    source_bytes: &[u8],
    filename: &str,
    json_mode: bool,
    stderr: &mut anstream::AutoStream<std::io::Stderr>,
) -> Result<Vec<u8>, CliError> {
    let handle = unsafe {
        ffi::deal_parse(
            source_bytes.as_ptr(),
            source_bytes.len(),
            filename.as_bytes().as_ptr(),
            filename.len(),
        )
    };

    if handle.is_null() {
        return Err(CliError::Internal(anyhow::anyhow!(
            "deal_parse returned null for {:?} (OOM)",
            filename
        )));
    }

    // Check for parse errors before formatting.
    let has_errors = unsafe { ffi::deal_has_errors(handle) };
    if has_errors {
        // Clone diagnostics BEFORE deal_free (Pitfall 3).
        let diag_json_owned: Vec<u8> = unsafe {
            let mut out_ptr: *const u8 = std::ptr::null();
            let mut out_len: usize = 0;
            let ok = ffi::deal_diagnostics_json(handle, &mut out_ptr, &mut out_len);
            if !ok {
                ffi::deal_free(handle);
                return Err(CliError::Internal(anyhow::anyhow!(
                    "deal_diagnostics_json OOM for {:?}",
                    filename
                )));
            }
            let bytes = std::slice::from_raw_parts(out_ptr, out_len).to_vec();
            ffi::deal_free(handle);
            bytes
        };

        if json_mode {
            // Emit D-32 envelope to stdout.
            let all_diagnostics: serde_json::Value = serde_json::from_slice(&diag_json_owned)
                .unwrap_or(serde_json::Value::Array(vec![]));
            let deal_version = env!("CARGO_PKG_VERSION");
            let diags_json =
                serde_json::to_string(&all_diagnostics).unwrap_or_else(|_| "[]".to_string());
            let stdout_choice = anstream::ColorChoice::Never;
            let mut stdout = anstream::AutoStream::new(std::io::stdout(), stdout_choice);
            let _ = write!(
                stdout,
                r#"{{"command":"fmt","deal_version":"{}","diagnostics":{},"summary":{{"errors":1,"hints":0,"warnings":0}},"v":1}}"#,
                deal_version, diags_json
            );
            let _ = writeln!(stdout);
        } else {
            let source_str = std::str::from_utf8(source_bytes).unwrap_or("");
            let all_diagnostics: serde_json::Value = serde_json::from_slice(&diag_json_owned)
                .unwrap_or(serde_json::Value::Array(vec![]));
            if let Some(diags) = all_diagnostics.as_array() {
                for diag in diags {
                    let _ = render::render_diagnostic(stderr, source_str, diag);
                }
            }
        }

        return Err(CliError::User(String::new()));
    }

    // No parse errors — format the source.
    let formatted_owned: Vec<u8> = unsafe {
        let mut out_ptr: *const u8 = std::ptr::null();
        let mut out_len: usize = 0;
        let ok = ffi::deal_format(handle, &mut out_ptr, &mut out_len);
        if !ok {
            ffi::deal_free(handle);
            return Err(CliError::Internal(anyhow::anyhow!(
                "deal_format failed (OOM) for {:?}",
                filename
            )));
        }
        // Clone bytes BEFORE deal_free (Pitfall 3 / T-02-29).
        let bytes = std::slice::from_raw_parts(out_ptr, out_len).to_vec();
        ffi::deal_free(handle);
        bytes
    };

    Ok(formatted_owned)
}

/// Write `data` to `path` atomically using a temp file + rename.
/// On failure (e.g. cross-device rename), falls back to plain write.
fn write_file_atomic(path: &std::path::Path, data: &[u8]) -> Result<(), CliError> {
    // Create temp file in the same directory to ensure same filesystem for rename.
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    let tmp_path = parent.join(format!(
        ".deal_fmt_tmp_{}_{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        std::process::id()
    ));

    // Write to temp file.
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

    // Rename to destination (atomic on POSIX).
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

/// Run the `deal parse` subcommand.
///
/// Per WARNING-05 Option A decision (locked in Plan 02-06):
///   - On SUCCESS: emit the raw alphabetical-keyed AST JSON returned from
///     `deal_ast_json()` DIRECTLY to stdout. NO envelope wrapping.
///     The bytes are already D-18 alphabetical-keyed by the Zig emitter;
///     the CLI passes them through unchanged.
///   - On PARSE ERROR (error-severity diagnostics):
///     * `--json` set: emit D-32 envelope with "command":"parse" to STDERR;
///       stdout is EMPTY (no partial AST mixed with diagnostics).
///     * `--json` NOT set: render human-readable diagnostics to stderr via
///       render_diagnostic; stdout is EMPTY.
///   - Exit 0 on success, 1 on parse error (D-34 User error), 2 on I/O / FFI
///     failure (D-34 Internal error).
///
/// WARNING-05 rationale (why no envelope on success):
///   D-32 defines the envelope as diagnostic-bearing: {v, command, deal_version,
///   diagnostics, summary}. It is designed to multiplex diagnostics with a
///   summary. The AST is a PAYLOAD, not a diagnostic. Wrapping the AST in a new
///   `ast` field would silently expand the public contract surface. The minimum-
///   surface choice keeps D-32 crisp: when --json produces an envelope, the
///   envelope is about diagnostics only.
fn run_parse(
    paths: &[std::path::PathBuf],
    json_mode: bool,
    color: ColorMode,
) -> Result<(), CliError> {
    let stderr_choice = color_choice(color);
    let mut stderr = anstream::AutoStream::new(std::io::stderr(), stderr_choice);

    for path in paths {
        // I/O failures are Internal errors (exit 2).
        let source_bytes = std::fs::read(path)
            .map_err(|e| CliError::Internal(anyhow::anyhow!("cannot read {:?}: {}", path, e)))?;
        let filename = path.to_string_lossy();

        // Call FFI parse. SAFETY: source_bytes is alive for the duration.
        let handle = unsafe {
            ffi::deal_parse(
                source_bytes.as_ptr(),
                source_bytes.len(),
                filename.as_bytes().as_ptr(),
                filename.len(),
            )
        };

        if handle.is_null() {
            return Err(CliError::Internal(anyhow::anyhow!(
                "deal_parse returned null for {:?} (OOM)",
                path
            )));
        }

        // Pitfall 3: clone all needed bytes BEFORE deal_free.
        let has_errors: bool;
        let ast_json_owned: Vec<u8>;
        let diag_json_owned: Vec<u8>;

        unsafe {
            has_errors = ffi::deal_has_errors(handle);

            // Clone AST JSON bytes.
            let mut ast_ptr: *const u8 = std::ptr::null();
            let mut ast_len: usize = 0;
            let ast_ok = ffi::deal_ast_json(handle, &mut ast_ptr, &mut ast_len);
            if !ast_ok {
                ffi::deal_free(handle);
                return Err(CliError::Internal(anyhow::anyhow!(
                    "deal_ast_json OOM for {:?}",
                    path
                )));
            }
            ast_json_owned = std::slice::from_raw_parts(ast_ptr, ast_len).to_vec();

            // Clone diagnostics JSON bytes (needed if has_errors).
            let mut diag_ptr: *const u8 = std::ptr::null();
            let mut diag_len: usize = 0;
            let diag_ok = ffi::deal_diagnostics_json(handle, &mut diag_ptr, &mut diag_len);
            if !diag_ok {
                ffi::deal_free(handle);
                return Err(CliError::Internal(anyhow::anyhow!(
                    "deal_diagnostics_json OOM for {:?}",
                    path
                )));
            }
            diag_json_owned = std::slice::from_raw_parts(diag_ptr, diag_len).to_vec();

            // Free arena — all pointers are now invalid.
            ffi::deal_free(handle);
        }

        if has_errors {
            // Parse error path — emit diagnostics, DO NOT emit AST to stdout.
            if json_mode {
                // Emit D-32 envelope to STDERR (not stdout) — diagnostics path.
                let all_diagnostics: serde_json::Value = serde_json::from_slice(&diag_json_owned)
                    .unwrap_or(serde_json::Value::Array(vec![]));
                let deal_version = env!("CARGO_PKG_VERSION");
                let diags_json =
                    serde_json::to_string(&all_diagnostics).unwrap_or_else(|_| "[]".to_string());

                // Count errors/warnings/hints.
                let mut err_count: u64 = 0;
                let mut warn_count: u64 = 0;
                let mut hint_count: u64 = 0;
                if let Some(diags) = all_diagnostics.as_array() {
                    for d in diags {
                        match d["severity"].as_str().unwrap_or("") {
                            "err" => err_count += 1,
                            "warn" => warn_count += 1,
                            "hint" | "info" => hint_count += 1,
                            _ => {}
                        }
                    }
                }

                let _ = write!(
                    stderr,
                    r#"{{"command":"parse","deal_version":"{}","diagnostics":{},"summary":{{"errors":{},"hints":{},"warnings":{}}},"v":1}}"#,
                    deal_version, diags_json, err_count, hint_count, warn_count,
                );
                let _ = writeln!(stderr);
            } else {
                // Human mode: render diagnostics to stderr.
                let source_str = std::str::from_utf8(&source_bytes).unwrap_or("");
                let all_diagnostics: serde_json::Value = serde_json::from_slice(&diag_json_owned)
                    .unwrap_or(serde_json::Value::Array(vec![]));
                if let Some(diags) = all_diagnostics.as_array() {
                    for diag in diags {
                        let _ = render::render_diagnostic(&mut stderr, source_str, diag);
                    }
                }
            }

            return Err(CliError::User(String::new()));
        }

        // Success path — emit raw AST JSON to stdout.
        // Per WARNING-05 Option A: NO envelope wrapping. The bytes returned by
        // deal_ast_json() are already D-18 alphabetical-keyed; pass through unchanged.
        // Both `deal parse FILE` and `deal parse --json FILE` emit the same raw bytes
        // to stdout on success (--json is a no-op for the success path in Phase 2).
        let stdout_choice = color_choice(ColorMode::Never); // JSON output: no color codes
        let mut stdout = anstream::AutoStream::new(std::io::stdout(), stdout_choice);
        stdout
            .write_all(&ast_json_owned)
            .map_err(|e| CliError::Internal(anyhow::anyhow!("stdout write error: {}", e)))?;
        writeln!(stdout)
            .map_err(|e| CliError::Internal(anyhow::anyhow!("stdout write error: {}", e)))?;
    }

    Ok(())
}

/// Run the `deal build --target sysml-v2` subcommand.
///
/// For each path: parse + lower (via FFI), clone IR JSON bytes (Pitfall 3),
/// emit SysML v2 JSON (sysml_v2::emit_from_bytes), accumulate all elements
/// into one workspace-wide Package (D-24), write to `build/sysml-v2/showcase.sysml-v2.json`.
///
/// If `--validate` is set: validate the output JSON against the bundled SysML.json;
/// exit 1 on validation failure (D-34 user error).
///
/// Exit codes per D-34:
///   0 — success
///   1 — validation failure (user error — schema-invalid output)
///   2 — FFI failure, schema-load failure, I/O error (internal error)
/// Expand `paths` so directory entries are replaced by every `.deal` / `.dealx`
/// file underneath them (recursive). Plain-file entries pass through unchanged.
/// Output order is deterministic per directory (alphabetical) for reproducible
/// builds — D-24 emits one consolidated workspace JSON, and the IR JSON is
/// already alphabetical-keyed, but lower-pass arena allocation order can leak
/// into element order without this sort.
fn expand_path_args(paths: &[std::path::PathBuf]) -> Result<Vec<std::path::PathBuf>, CliError> {
    let mut out = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = paths.iter().rev().cloned().collect();
    while let Some(p) = stack.pop() {
        let meta = std::fs::metadata(&p)
            .map_err(|e| CliError::Internal(anyhow::anyhow!("cannot stat {:?}: {}", p, e)))?;
        if meta.is_dir() {
            let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(&p)
                .map_err(|e| CliError::Internal(anyhow::anyhow!("cannot read dir {:?}: {}", p, e)))?
                .filter_map(|r| r.ok().map(|e| e.path()))
                .collect();
            entries.sort();
            for entry in entries.into_iter().rev() {
                stack.push(entry);
            }
        } else if matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("deal" | "dealx")
        ) {
            out.push(p);
        }
    }
    Ok(out)
}

/// Drop files under `[workspace].exclude` paths (relative to the project root)
/// from an expanded path list. Frontier/draft packages that intentionally don't
/// parse on the current grammar are kept on disk but skipped by check/build.
fn apply_workspace_excludes(
    paths: &[std::path::PathBuf],
    mut resolved: Vec<std::path::PathBuf>,
) -> Vec<std::path::PathBuf> {
    let root = find_deal_toml_root(paths).unwrap_or_else(|| std::path::PathBuf::from("."));
    if let Ok(bytes) = std::fs::read(root.join("deal.toml")) {
        if let Ok(s) = std::str::from_utf8(&bytes) {
            if let Ok(manifest) = toml::from_str::<resolver::DealToml>(s) {
                let excluded: Vec<std::path::PathBuf> = manifest
                    .workspace
                    .exclude
                    .iter()
                    .map(|e| root.join(e))
                    .filter_map(|p| std::fs::canonicalize(&p).ok())
                    .collect();
                if !excluded.is_empty() {
                    resolved.retain(|p| match std::fs::canonicalize(p) {
                        Ok(cp) => !excluded.iter().any(|ex| cp.starts_with(ex)),
                        Err(_) => true,
                    });
                }
            }
        }
    }
    resolved
}

/// Shared workspace loader (ADR-0004 P4 WS-D): discover project + dependency
/// files from `paths`, then compute the entry-point import closure. Used by
/// `check`, `build`, and the emitters so they all load the identical reachable
/// package set (entry points: explicit file args → `.dealx` → `.deal`).
/// Per-command concerns (E2402 not-installed gating, output inference) stay in
/// the individual commands.
fn plan_load_from_paths(paths: &[std::path::PathBuf]) -> Result<closure::LoadPlan, CliError> {
    let project_root =
        find_deal_toml_root(paths).unwrap_or_else(|| std::path::PathBuf::from("."));
    let manifest: Option<resolver::DealToml> = std::fs::read(project_root.join("deal.toml"))
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|s| toml::from_str::<resolver::DealToml>(&s).ok());

    // Explicit file arguments are entry points and always part of the set.
    let explicit_files: Vec<std::path::PathBuf> =
        paths.iter().filter(|p| p.is_file()).cloned().collect();

    // Project discovery roots: [workspace].roots (+ deprecated `packages`)
    // when a manifest is present; otherwise argument-scoped expansion.
    let project_files: Vec<std::path::PathBuf> = {
        let roots: Vec<std::path::PathBuf> = match &manifest {
            Some(m) => {
                let mut rs: Vec<String> = m.workspace.roots.clone();
                rs.extend(m.workspace.packages.iter().cloned());
                rs.into_iter().map(|r| project_root.join(r)).collect()
            }
            None => Vec::new(),
        };
        let discovered = if roots.is_empty() {
            expand_path_args(paths)?
        } else {
            closure::discover_files(&roots)
        };
        let mut pf = apply_workspace_excludes(paths, discovered);
        pf.extend(explicit_files.iter().cloned());
        pf.sort();
        pf.dedup();
        pf
    };
    if project_files.is_empty() {
        return Err(CliError::User(format!(
            "no .deal or .dealx files found under {paths:?}"
        )));
    }

    // Dependency roots (git deps under .deal/deps/<name>/packages, path deps
    // under <path>/packages) → external sources entering only via the closure.
    let mut dep_roots: Vec<std::path::PathBuf> = Vec::new();
    {
        let deps_base = project_root.join(".deal").join("deps");
        if let Ok(entries) = std::fs::read_dir(&deps_base) {
            for e in entries.flatten() {
                let pkgs = e.path().join("packages");
                if pkgs.is_dir() {
                    dep_roots.push(pkgs);
                }
            }
        }
        if let Some(m) = &manifest {
            for dep in m.dependencies.values() {
                if let resolver::Dependency::Path { path } = dep {
                    let pkgs = project_root.join(path).join("packages");
                    if pkgs.is_dir() {
                        dep_roots.push(pkgs);
                    }
                }
            }
        }
    }
    let dep_files = closure::discover_files(&dep_roots);

    closure::plan_load(&project_files, &dep_files, &explicit_files).map_err(CliError::User)
}

/// Assemble the NUL-separated external-source blob for `file` = every OTHER
/// closure file's source (ADR-0004 P4 strict mode: external table present →
/// import-scoped resolution). `source_cache` must already hold the closure.
fn externals_for<'a>(
    file: &std::path::Path,
    closure_files: &[std::path::PathBuf],
    source_cache: &std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
) -> Vec<u8> {
    let mut externals: Vec<u8> = Vec::new();
    for other in closure_files {
        if other.as_path() == file {
            continue;
        }
        if let Some(src) = source_cache.get(other) {
            if !externals.is_empty() {
                externals.push(0u8);
            }
            externals.extend_from_slice(src);
        }
    }
    externals
}

fn run_build(
    paths: &[std::path::PathBuf],
    validate: bool,
    _json_mode: bool,
    color: ColorMode,
    output_override: Option<&std::path::Path>,
) -> Result<(), CliError> {
    let stderr_choice = color_choice(color);
    let mut stderr = anstream::AutoStream::new(std::io::stderr(), stderr_choice);

    // Discover + plan the entry-point import closure (ADR-0004 P4 WS-D): build
    // emits the reachable-package model and enforces import visibility like
    // `check` (strict). Unreachable packages are not emitted.
    let plan = plan_load_from_paths(paths)?;

    // Pre-read every closure file's source for per-file external assembly.
    let mut source_cache: std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> =
        std::collections::BTreeMap::new();
    for p in &plan.closure {
        if let Ok(b) = std::fs::read(p) {
            source_cache.insert(p.clone(), b);
        }
    }

    // Infer output dir from the analyzed set's repo root, or use --output override.
    let output_path = match output_override {
        Some(p) => p.to_path_buf(),
        None => infer_output_path(&plan.analyze)?,
    };

    // Process each reachable source file via FFI.
    let mut all_sysml_elements: Vec<serde_json::Value> = Vec::new();
    let mut any_sema_errors = false;

    for path in &plan.analyze {
        let source_bytes = match source_cache.get(path) {
            Some(b) => b.clone(),
            None => std::fs::read(path).map_err(|e| {
                CliError::Internal(anyhow::anyhow!("cannot read {:?}: {}", path, e))
            })?,
        };
        let filename = path.to_string_lossy();

        // ── Enforce (strict): deal_check_with_stdlib with the closure as the
        // external table. Import-scoping violations block codegen. This handle
        // is used ONLY for diagnostics — its symbol-table entries alias the
        // freed external arena, so IR must come from the deal_parse pass below,
        // never this handle (D-88).
        let externals = externals_for(path, &plan.closure, &source_cache);
        let check_handle = unsafe {
            let mut _o: *const u8 = std::ptr::null();
            let mut _ol: usize = 0;
            ffi::deal_check_with_stdlib(
                source_bytes.as_ptr(),
                source_bytes.len(),
                filename.as_bytes().as_ptr(),
                filename.len(),
                externals.as_ptr(),
                externals.len(),
                &mut _o,
                &mut _ol,
            )
        };
        if check_handle.is_null() {
            return Err(CliError::Internal(anyhow::anyhow!(
                "deal_check_with_stdlib returned null for {:?} (OOM)",
                path
            )));
        }
        let has_errors = unsafe { ffi::deal_has_errors(check_handle) };
        if has_errors {
            any_sema_errors = true;
            let mut out_ptr: *const u8 = std::ptr::null();
            let mut out_len: usize = 0;
            let diag_ok =
                unsafe { ffi::deal_diagnostics_json(check_handle, &mut out_ptr, &mut out_len) };
            if diag_ok && !out_ptr.is_null() {
                let diag_bytes = unsafe { std::slice::from_raw_parts(out_ptr, out_len).to_vec() };
                unsafe { ffi::deal_free(check_handle) };
                if let Ok(diag_val) = serde_json::from_slice::<serde_json::Value>(&diag_bytes) {
                    if let Some(diags) = diag_val.as_array() {
                        let source_str = std::str::from_utf8(&source_bytes).unwrap_or("");
                        for diag in diags {
                            let _ = render::render_diagnostic(&mut stderr, source_str, diag);
                        }
                    }
                }
            } else {
                unsafe { ffi::deal_free(check_handle) };
            }
            continue;
        }
        unsafe { ffi::deal_free(check_handle) };

        // ── Emit: a fresh deal_parse pass owns its arena, so deal_ir_json is
        // safe (the check handle's IR would alias the freed external arena).
        let parse_handle = unsafe {
            ffi::deal_parse(
                source_bytes.as_ptr(),
                source_bytes.len(),
                filename.as_bytes().as_ptr(),
                filename.len(),
            )
        };
        if parse_handle.is_null() {
            return Err(CliError::Internal(anyhow::anyhow!(
                "deal_parse returned null for {:?} (OOM)",
                path
            )));
        }
        // Pitfall 3: clone IR JSON bytes BEFORE calling deal_free.
        let ir_json_owned: Vec<u8>;
        unsafe {
            let mut out_ptr: *const u8 = std::ptr::null();
            let mut out_len: usize = 0;
            let ok = ffi::deal_ir_json(parse_handle, &mut out_ptr, &mut out_len);
            if !ok {
                ffi::deal_free(parse_handle);
                return Err(CliError::Internal(anyhow::anyhow!(
                    "deal_ir_json failed for {:?}",
                    path
                )));
            }
            ir_json_owned = std::slice::from_raw_parts(out_ptr, out_len).to_vec();
            ffi::deal_free(parse_handle);
        }

        // Parse IR JSON and emit SysML v2.
        let sysml_val = sysml_v2::emit_from_bytes(&ir_json_owned).map_err(|e| {
            CliError::Internal(anyhow::anyhow!(
                "SysML v2 emit failed for {:?}: {}",
                path,
                e
            ))
        })?;

        // Collect all ownedRelationship elements from this file's Package.
        if let Some(owned) = sysml_val["ownedRelationship"].as_array() {
            all_sysml_elements.extend(owned.iter().cloned());
        }
    }

    // If any file had sema errors, exit 1 — sema errors block codegen (user error).
    if any_sema_errors {
        return Err(CliError::User(String::new()));
    }

    // Build the consolidated workspace-wide Package (D-24).
    let workspace_uuid = sysml_v2::deal_id_to_uuid("workspace");
    let consolidated = serde_json::json!({
        "@id": workspace_uuid,
        "@type": "Package",
        "declaredName": "Workspace",
        "elementId": workspace_uuid,
        "ownedRelationship": all_sysml_elements,
        "qualifiedName": "Workspace"
    });

    // Validate if --validate flag is set (D-34: validation failure = user error exit 1).
    if validate {
        match schema_registry::validate(&consolidated) {
            Ok(()) => {}
            Err(errors) => {
                for err in &errors {
                    let _ = writeln!(stderr, "validation error: {}", err);
                }
                return Err(CliError::User(format!(
                    "SysML v2 validation failed ({} error(s))",
                    errors.len()
                )));
            }
        }
    }

    // Write output file (D-24: single workspace-wide file).
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            CliError::Internal(anyhow::anyhow!(
                "cannot create output directory {}: {}",
                parent.display(),
                e
            ))
        })?;
    }

    let output_json = serde_json::to_string_pretty(&consolidated)
        .map_err(|e| CliError::Internal(anyhow::anyhow!("JSON serialization error: {}", e)))?;

    std::fs::write(&output_path, output_json).map_err(|e| {
        CliError::Internal(anyhow::anyhow!(
            "cannot write output to {}: {}",
            output_path.display(),
            e
        ))
    })?;

    Ok(())
}

// ─── run_build_reqif ──────────────────────────────────────────────────────────

/// Run the `deal build --target reqif` subcommand.
///
/// For each path: parse + lower (via FFI), clone IR JSON bytes (Pitfall 3 / T-02-13),
/// accumulate elements across all source files into a single IR graph, call
/// `reqif::emit_from_bytes` to produce the .reqifz archive.
///
/// Default output path: `build/reqif/model.reqifz` (mirrors sysml-v2 path inference).
/// If `--validate` is set, the structural XSD gate already runs inside
/// `reqif::emit_from_bytes`; surface violations as `CliError::User`.
///
/// Success message (CLI Copywriting, 04-UI-SPEC):
///   `deal build: wrote build/reqif/model.reqifz ({N} requirements, {M} relations)`
fn run_build_reqif(
    paths: &[std::path::PathBuf],
    _validate: bool,
    _json_mode: bool,
    color: ColorMode,
    output_override: Option<&std::path::Path>,
) -> Result<(), CliError> {
    let stderr_choice = color_choice(color);
    let mut stderr = anstream::AutoStream::new(std::io::stderr(), stderr_choice);

    // Discover + plan the entry-point import closure (ADR-0004 P4 WS-D): emit
    // the reachable-package model, enforcing import visibility like `check`.
    let plan = plan_load_from_paths(paths)?;

    // Pre-read every closure file's source for per-file external assembly.
    let mut source_cache: std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> =
        std::collections::BTreeMap::new();
    for p in &plan.closure {
        if let Ok(b) = std::fs::read(p) {
            source_cache.insert(p.clone(), b);
        }
    }

    // Infer output path (default: build/reqif/model.reqifz).
    let output_path = match output_override {
        Some(p) => p.to_path_buf(),
        None => infer_reqif_output_path(&plan.analyze)?,
    };

    // Accumulate IR elements across all reachable source files.
    let mut merged_elements = serde_json::Map::new();
    let mut merged_edges: Vec<serde_json::Value> = Vec::new();
    let mut any_sema_errors = false;

    for path in &plan.analyze {
        let source_bytes = match source_cache.get(path) {
            Some(b) => b.clone(),
            None => std::fs::read(path).map_err(|e| {
                CliError::Internal(anyhow::anyhow!("cannot read {:?}: {}", path, e))
            })?,
        };
        let filename = path.to_string_lossy();

        // ── Enforce (strict): deal_check_with_stdlib with the closure external
        // table. Violations block codegen. Diagnostics-only handle (D-88).
        let externals = externals_for(path, &plan.closure, &source_cache);
        let check_handle = unsafe {
            let mut _o: *const u8 = std::ptr::null();
            let mut _ol: usize = 0;
            ffi::deal_check_with_stdlib(
                source_bytes.as_ptr(),
                source_bytes.len(),
                filename.as_bytes().as_ptr(),
                filename.len(),
                externals.as_ptr(),
                externals.len(),
                &mut _o,
                &mut _ol,
            )
        };
        if check_handle.is_null() {
            return Err(CliError::Internal(anyhow::anyhow!(
                "deal_check_with_stdlib returned null for {:?} (OOM)",
                path
            )));
        }
        let has_errors = unsafe { ffi::deal_has_errors(check_handle) };
        if has_errors {
            any_sema_errors = true;
            let mut out_ptr: *const u8 = std::ptr::null();
            let mut out_len: usize = 0;
            let diag_ok =
                unsafe { ffi::deal_diagnostics_json(check_handle, &mut out_ptr, &mut out_len) };
            if diag_ok && !out_ptr.is_null() {
                let diag_bytes = unsafe { std::slice::from_raw_parts(out_ptr, out_len).to_vec() };
                unsafe { ffi::deal_free(check_handle) };
                if let Ok(diag_val) = serde_json::from_slice::<serde_json::Value>(&diag_bytes) {
                    if let Some(diags) = diag_val.as_array() {
                        let source_str = std::str::from_utf8(&source_bytes).unwrap_or("");
                        for diag in diags {
                            let _ = render::render_diagnostic(&mut stderr, source_str, diag);
                        }
                    }
                }
            } else {
                unsafe { ffi::deal_free(check_handle) };
            }
            continue;
        }
        unsafe { ffi::deal_free(check_handle) };

        // ── Emit: fresh deal_parse pass owns its arena → safe deal_ir_json.
        let parse_handle = unsafe {
            ffi::deal_parse(
                source_bytes.as_ptr(),
                source_bytes.len(),
                filename.as_bytes().as_ptr(),
                filename.len(),
            )
        };
        if parse_handle.is_null() {
            return Err(CliError::Internal(anyhow::anyhow!(
                "deal_parse returned null for {:?} (OOM)",
                path
            )));
        }
        // T-02-13 / Pitfall 3: clone IR JSON bytes BEFORE calling deal_free.
        let ir_json_owned: Vec<u8>;
        unsafe {
            let mut out_ptr: *const u8 = std::ptr::null();
            let mut out_len: usize = 0;
            let ok = ffi::deal_ir_json(parse_handle, &mut out_ptr, &mut out_len);
            if !ok {
                ffi::deal_free(parse_handle);
                return Err(CliError::Internal(anyhow::anyhow!(
                    "deal_ir_json failed for {:?}",
                    path
                )));
            }
            ir_json_owned = std::slice::from_raw_parts(out_ptr, out_len).to_vec();
            ffi::deal_free(parse_handle); // pointer now invalid — ir_json_owned is the live copy
        }

        // Merge elements and edges from this file into the accumulator.
        if let Ok(ir_val) = serde_json::from_slice::<serde_json::Value>(&ir_json_owned) {
            if let Some(elems) = ir_val["elements"].as_object() {
                for (k, v) in elems {
                    merged_elements.insert(k.clone(), v.clone());
                }
            }
            if let Some(edges) = ir_val["edges"].as_array() {
                merged_edges.extend(edges.iter().cloned());
            }
        }
    }

    if any_sema_errors {
        return Err(CliError::User(String::new()));
    }

    // Build the merged IR JSON.
    let merged_ir = serde_json::json!({
        "edges": merged_edges,
        "elements": merged_elements,
        "ir_version": "v0",
        "v": 1
    });
    let merged_bytes = serde_json::to_vec(&merged_ir).map_err(|e| {
        CliError::Internal(anyhow::anyhow!("IR JSON re-serialization error: {}", e))
    })?;

    // Emit and write .reqifz (structural validation runs inside emit_from_bytes).
    let (req_count, rel_count) = reqif::emit_from_bytes(&merged_bytes, &output_path)
        .map_err(|e| CliError::User(format!("ReqIF build failed: {}", e)))?;

    // Success message (UI-SPEC §CLI Copywriting).
    use owo_colors::OwoColorize;
    let stdout_choice = color_choice(color);
    let mut stdout = anstream::AutoStream::new(std::io::stdout(), stdout_choice);
    let _ = writeln!(
        stdout,
        "{}",
        format!(
            "deal build: wrote {} ({} requirements, {} relations)",
            output_path.display(),
            req_count,
            rel_count
        )
        .green()
    );

    Ok(())
}

/// Infer the output path for the ReqIF .reqifz archive.
///
/// Default: `<repo_root>/build/reqif/model.reqifz`
fn infer_reqif_output_path(paths: &[std::path::PathBuf]) -> Result<std::path::PathBuf, CliError> {
    let base_dir: std::path::PathBuf = if paths.is_empty() {
        std::env::current_dir().map_err(|e| {
            CliError::Internal(anyhow::anyhow!("cannot get current directory: {}", e))
        })?
    } else {
        let first = paths[0].canonicalize().unwrap_or_else(|_| paths[0].clone());
        let mut found: Option<std::path::PathBuf> = None;
        let mut dir = first.as_path();
        loop {
            if dir.join("Cargo.toml").exists() {
                found = Some(dir.to_path_buf());
                break;
            }
            match dir.parent() {
                Some(p) => dir = p,
                None => break,
            }
        }
        found.unwrap_or_else(|| {
            first
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf()
        })
    };

    Ok(base_dir.join("build").join("reqif").join("model.reqifz"))
}

// ─── Infer SysML v2 output path ───────────────────────────────────────────────

/// Infer the output path for the consolidated SysML v2 JSON.
///
/// Per D-24: `<repo_root>/tests/showcase/build/sysml-v2/showcase.sysml-v2.json`
/// if `tests/showcase/` is a real (non-broken) directory; otherwise
/// `<repo_root>/build/sysml-v2/showcase.sysml-v2.json`.
///
/// WR-02: find the project root containing `deal.toml` by walking up from each
/// input path (file or directory). Returns the first directory found that
/// contains a `deal.toml`, or `None` if no input path has one in its ancestry.
///
/// This mirrors how `infer_output_path` walks up looking for `Cargo.toml`, so the
/// E2402 not-installed gate fires regardless of whether the user passed a
/// directory argument or a bare file path, and independent of the cwd.
fn find_deal_toml_root(paths: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    // Always consider the current working directory as a fallback search origin
    // so `deal check` with no path args still resolves the enclosing project.
    let mut origins: Vec<std::path::PathBuf> = paths.to_vec();
    if let Ok(cwd) = std::env::current_dir() {
        origins.push(cwd);
    }

    for origin in &origins {
        let canonical = origin.canonicalize().unwrap_or_else(|_| origin.clone());
        // Start the walk at the path itself if it is a directory, otherwise at
        // its parent (a file's deal.toml lives in an ancestor directory).
        let start: &std::path::Path = if canonical.is_dir() {
            canonical.as_path()
        } else {
            canonical
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
        };
        let mut dir = start;
        loop {
            if dir.join("deal.toml").exists() {
                return Some(dir.to_path_buf());
            }
            match dir.parent() {
                Some(p) => dir = p,
                None => break,
            }
        }
    }
    None
}

/// We find the repo root by walking up from the first path looking for Cargo.toml.
fn infer_output_path(paths: &[std::path::PathBuf]) -> Result<std::path::PathBuf, CliError> {
    let base_dir: std::path::PathBuf = if paths.is_empty() {
        std::env::current_dir().map_err(|e| {
            CliError::Internal(anyhow::anyhow!("cannot get current directory: {}", e))
        })?
    } else {
        // Walk up from first path to find repo root (contains Cargo.toml).
        let first = paths[0].canonicalize().unwrap_or_else(|_| paths[0].clone());
        let mut found: Option<std::path::PathBuf> = None;
        let mut dir = first.as_path();
        loop {
            if dir.join("Cargo.toml").exists() {
                found = Some(dir.to_path_buf());
                break;
            }
            match dir.parent() {
                Some(p) => dir = p,
                None => break,
            }
        }
        found.unwrap_or_else(|| {
            first
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf()
        })
    };

    // Prefer tests/showcase/build/ if showcase is a real directory (not a broken symlink).
    let showcase_dir = base_dir.join("tests").join("showcase");
    let output_base = if showcase_dir.exists() && showcase_dir.is_dir() {
        showcase_dir.join("build")
    } else {
        base_dir.join("build")
    };

    Ok(output_base.join("sysml-v2").join("showcase.sysml-v2.json"))
}

// ─── Starter model sources (D-69) ────────────────────────────────────────────
//
// These embedded string constants define a minimal but real example that:
//   - Parses cleanly under the locked 0.1.0-draft grammar
//   - Contains one part def, one port def, one requirement def (in .deal)
//   - Contains one composition + one traceability/satisfy block (in .dealx)
//   - Does NOT import deal.std (so `deal check` passes before `deal install`)
//
// The satisfy block intentionally uses method="analysis" to keep body empty.

const STARTER_DEAL: &str = r#"package starter;

part def StarterPart {}
port def StarterPort {}

requirement def REQ_001 {
    verification {
        accepts: [analysis];
    }
}
"#;

const STARTER_DEALX: &str = r#"package model;

[<system StarterSystem>]
    [<StarterPart as="part" />]
[</system>]

[<traceability>]
    [<satisfy requirement="REQ_001" by="StarterSystem" method="analysis">]
    [</satisfy>]
[</traceability>]
"#;

// ─── run_init ─────────────────────────────────────────────────────────────────

/// Run the `deal init <name>` subcommand.
///
/// Scaffolds the RECOMMENDED project layout in `./<name>/`. The layout is a
/// human convention for readability/findability, NOT a requirement — the
/// toolchain discovers `*.deal`/`*.dealx` anywhere (a flat directory of
/// arbitrarily-named files works too). Includes:
///   - Directories: definitions/, model/, simulations/, test/data/, docs/, .deal/
///   - deal.toml: [project], [workspace], [dependencies] with deal-std git dep (D-67)
///   - definitions/starter.deal: part def, port def, requirement def (D-69)
///   - model/starter.dealx: composition + satisfy block (D-69)
///   - .gitignore: contains `.deal/`
///
/// Overwrite guard: exits non-zero if ./<name>/ already exists and is non-empty.
///
/// Exit codes (D-34):
///   0 — success
///   1 — user error (overwrite guard, missing name)
///   2 — I/O failure
fn run_init(name_opt: Option<String>, _json: bool, color: ColorMode) -> Result<(), CliError> {
    // Resolve the project name from argument or prompt.
    let name = match name_opt {
        Some(n) if !n.is_empty() => n,
        _ => {
            return Err(CliError::User(
                "deal init: project name is required — usage: deal init <name>".to_string(),
            ));
        }
    };

    let project_dir = std::path::Path::new(&name);

    // Overwrite guard (UI-SPEC): if directory exists and is non-empty, reject.
    if project_dir.exists() {
        let is_empty = std::fs::read_dir(project_dir)
            .map(|mut d| d.next().is_none())
            .unwrap_or(false);
        if !is_empty {
            return Err(CliError::User(format!(
                "directory './{name}' already exists and is not empty\n\
                 note: remove or rename the directory, then re-run `deal init {name}`"
            )));
        }
    }

    // ── Create the recommended directory tree ──
    // Convention only — discovery is layout-agnostic (Phase 1b). `definitions/`
    // holds *.deal defs (group by kind as the project grows); `model/` holds
    // *.dealx usages + co-located *.dealview sidecars.
    let dirs = [
        "definitions",
        "model",
        "simulations",
        "test/data",
        "docs",
        ".deal",
    ];
    for dir in &dirs {
        std::fs::create_dir_all(project_dir.join(dir)).map_err(|e| {
            CliError::Internal(anyhow::anyhow!(
                "cannot create directory {:?}: {}",
                project_dir.join(dir),
                e
            ))
        })?;
    }

    // ── .gitignore ──
    let gitignore_content = ".deal/\n";
    std::fs::write(project_dir.join(".gitignore"), gitignore_content)
        .map_err(|e| CliError::Internal(anyhow::anyhow!("cannot write .gitignore: {}", e)))?;

    // ── deal.toml ──
    // [project] + [workspace] + [dependencies] with deal-std git dep (D-67).
    // DEFAULT_STDLIB_TAG is the single source of truth for the stdlib tag.
    let deal_toml_content = format!(
        r#"[project]
name = "{name}"
version = "0.1.0"
schema = "deal/0.1"
marking = "Unclassified"
description = "A DEAL project"

[workspace]
# Recommended source roots. Discovery is layout-agnostic, so this is
# documentary — `deal check .` finds *.deal/*.dealx anywhere.
packages = ["definitions/*", "model"]

[dependencies]
deal-std = {{ git = "https://github.com/deal-lang/deal-stdlib", tag = "{DEFAULT_STDLIB_TAG}" }}
"#
    );
    std::fs::write(project_dir.join("deal.toml"), &deal_toml_content)
        .map_err(|e| CliError::Internal(anyhow::anyhow!("cannot write deal.toml: {}", e)))?;

    // ── Starter model files (D-69) ──
    std::fs::write(
        project_dir.join("definitions").join("starter.deal"),
        STARTER_DEAL,
    )
    .map_err(|e| {
        CliError::Internal(anyhow::anyhow!(
            "cannot write definitions/starter.deal: {}",
            e
        ))
    })?;

    std::fs::write(
        project_dir.join("model").join("starter.dealx"),
        STARTER_DEALX,
    )
    .map_err(|e| CliError::Internal(anyhow::anyhow!("cannot write model/starter.dealx: {}", e)))?;

    // ── Print success messages (UI-SPEC §CLI Copywriting) ──
    // Success green: "deal init: project '{name}' created in ./{name}/"
    // Hint plain:    "Next: cd {name} && deal install"
    let color_choice = color_choice(color);
    let mut stdout = anstream::AutoStream::new(std::io::stdout(), color_choice);
    use owo_colors::OwoColorize;
    let success_msg = format!("deal init: project '{name}' created in ./{name}/");
    if matches!(color, ColorMode::Never) {
        let _ = writeln!(stdout, "{success_msg}");
    } else {
        let _ = writeln!(stdout, "{}", success_msg.green());
    }
    let _ = writeln!(stdout, "Next: cd {name} && deal install");

    Ok(())
}

// ─── run_install ──────────────────────────────────────────────────────────────

/// Run the `deal install` subcommand.
///
/// Resolves all dependencies in `./deal.toml` by calling `resolver::resolve_all(".")`.
/// For each git dep, prints `Downloading {name} v{tag} from {url}...` before cloning.
/// On success, prints the count of resolved packages and a commit-deal.lock reminder
/// if the lockfile content changed.
///
/// Exit codes (D-34):
///   0 — success
///   1 — user error (resolver error, missing deal.toml)
///   2 — internal I/O error
fn run_install(_json: bool, color: ColorMode) -> Result<(), CliError> {
    // Pre-scan deal.toml for git deps so we can print the Downloading messages
    // before calling resolve_all (which does the actual work).
    let current_dir = std::env::current_dir()
        .map_err(|e| CliError::Internal(anyhow::anyhow!("cannot get current dir: {}", e)))?;

    let toml_path = current_dir.join("deal.toml");
    let toml_bytes = std::fs::read(&toml_path).map_err(|e| {
        CliError::User(format!(
            "cannot read deal.toml: {} — run `deal init` first",
            e
        ))
    })?;
    let toml_str = std::str::from_utf8(&toml_bytes)
        .map_err(|e| CliError::Internal(anyhow::anyhow!("deal.toml is not valid UTF-8: {}", e)))?;
    let manifest: resolver::DealToml = toml::from_str(toml_str)
        .map_err(|e| CliError::User(format!("invalid deal.toml: {}", e)))?;

    let color_choice = color_choice(color);
    let mut stdout = anstream::AutoStream::new(std::io::stdout(), color_choice);

    // Print per-dep Downloading messages (UI-SPEC §CLI Copywriting).
    for (name, dep) in &manifest.dependencies {
        if let resolver::Dependency::Git { git, tag, .. } = dep {
            let version_str = tag.as_deref().unwrap_or("HEAD");
            let _ = writeln!(stdout, "Downloading {name} v{version_str} from {git}...");
        }
    }

    let dep_count = manifest.dependencies.len();

    // Read old lockfile content for change detection.
    let lock_path = current_dir.join("deal.lock");
    let old_lock = std::fs::read_to_string(&lock_path).ok();

    // Run the resolver.
    resolver::resolve_all(&current_dir).map_err(|e| CliError::User(format!("{:#}", e)))?;

    // Read new lockfile content.
    let new_lock = std::fs::read_to_string(&lock_path).ok();

    // Success message (UI-SPEC §CLI Copywriting — green).
    use owo_colors::OwoColorize;
    let success_msg = format!(
        "deal install: {dep_count} {} resolved, deal.lock updated",
        if dep_count == 1 {
            "dependency"
        } else {
            "dependencies"
        }
    );
    if matches!(color, ColorMode::Never) {
        let _ = writeln!(stdout, "{success_msg}");
    } else {
        let _ = writeln!(stdout, "{}", success_msg.green());
    }

    // Lock-change hint (UI-SPEC).
    if old_lock != new_lock {
        let _ = writeln!(
            stdout,
            "deal install: deal.lock updated — commit deal.lock to reproduce this build"
        );
    }

    Ok(())
}

fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Parse { paths } => run_parse(&paths, cli.json, cli.color),
        Command::Check {
            paths,
            verify,
            simulations,
            run_sims,
        } => {
            // --verify and --simulations are Phase 5 flags; run_check ignores them
            // in Wave 0 (stub dispatch). Plan 05 wires --verify; Plan 03 wires --simulations.
            if verify {
                return verify::run_verify(&paths, run_sims, cli.json, color_pref(cli.color));
            }
            if simulations {
                return simulate::validate_bindings();
            }
            run_check(&paths, cli.json, cli.color)
        }
        Command::Fmt {
            paths,
            check,
            stdout,
        } => run_fmt(&paths, check, stdout, cli.json, cli.color),
        Command::Build {
            target: BuildTarget::SysmlV2,
            validate,
            output,
            paths,
        } => run_build(&paths, validate, cli.json, cli.color, output.as_deref()),
        Command::Build {
            target: BuildTarget::Reqif,
            validate,
            output,
            paths,
        } => run_build_reqif(&paths, validate, cli.json, cli.color, output.as_deref()),
        Command::Init { name } => run_init(name, cli.json, cli.color),
        Command::Install => run_install(cli.json, cli.color),
        Command::Simulate { names, all, stale } => {
            simulate::run_simulate(&names, all, stale, color_pref(cli.color))
        }
        Command::Evidence { subcommand } => evidence::run_evidence(subcommand),
    }
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(ref e) if e.is_user() => {
            // User error: print only if there's a message (empty string = silent,
            // the diagnostics were already rendered above).
            let msg = format!("{}", e);
            if !msg.is_empty() {
                let mut stderr =
                    anstream::AutoStream::new(std::io::stderr(), anstream::ColorChoice::Auto);
                let _ = writeln!(stderr, "error: {}", msg);
            }
            std::process::ExitCode::from(1)
        }
        Err(e) => {
            let mut stderr =
                anstream::AutoStream::new(std::io::stderr(), anstream::ColorChoice::Auto);
            let _ = writeln!(stderr, "error: {}", e);
            std::process::ExitCode::from(2)
        }
    }
}
