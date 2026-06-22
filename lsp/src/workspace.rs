//! Workspace discovery + eager parse (Plan 03-04 / D-47).
//!
//! On LSP `initialized`, walk the workspace root for `*.deal` / `*.dealx`
//! files **anywhere under the root** (layout-agnostic, Phase 1b — build/VCS
//! dirs pruned), parse each silently (no publishDiagnostics — eager parse is
//! a background warm-up), and populate the in-memory symbol `Index`
//! (D-46 / D-49). The disk `.deal/index.json` is NOT consulted or written
//! here (D-48). Directory layout is a human convention; the engine imposes
//! none — a flat directory of arbitrarily-named files is discovered the same
//! as the recommended `definitions/` + `model/` layout.
//!
//! ## deal.toml shape (verified live, 2026-05-25)
//!
//! ```toml
//! [project]
//! name = "..."
//!
//! [workspace]
//! packages = ["packages/*"]
//!
//! [workspace.aliases]
//! vehicle = "packages/vehicle"
//! ```
//!
//! The `aliases` table feeds the PS-5 alias-resolution layer in `Index`
//! (`resolve_with_alias`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use walkdir::WalkDir;

use crate::documents::Documents;
use crate::index::Index;

/// Maximum directory depth walked by the eager parse. Defends against
/// pathological workspaces (T-3-04 DoS). 16 is generous for hand-authored
/// hierarchical SysML projects.
const MAX_WALK_DEPTH: usize = 16;

/// Path-construction helpers isolated into a child module so the
/// manifest-path assembly stays in one place behind a documented contract.
mod paths {
    use anyhow::{Context, Result};
    use std::path::{Path, PathBuf};

    /// Hard-coded manifest filename. NOT derived from any function input.
    const MANIFEST_FILENAME: &str = "deal.toml";

    /// Load the deal.toml manifest text from inside `canonical_root`.
    /// Returns `Ok(None)` if the manifest does not exist; `Err(_)` on
    /// containment failure or read error.
    ///
    /// ## Path-safety contract
    ///
    /// - `canonical_root` MUST already be the result of
    ///   `std::fs::canonicalize` — the caller (`Workspace::discover`)
    ///   guarantees this.
    /// - The assembled path is `<canonical_root>/<MANIFEST_FILENAME>`
    ///   where the filename is a hard-coded `&'static str` with no
    ///   separators, no `..`, and no untrusted segments.
    /// - We re-canonicalize the assembled path if it exists; if the
    ///   resolved target does not live inside `canonical_root`, we bail
    ///   out (catches a hostile symlink at `<root>/deal.toml`).
    pub(super) fn try_load_manifest(canonical_root: &Path) -> Result<Option<String>> {
        let manifest_path = assemble_manifest_path(canonical_root)?;
        if !manifest_path.exists() {
            return Ok(None);
        }
        let resolved = std::fs::canonicalize(&manifest_path)
            .with_context(|| format!("canonicalize {}", manifest_path.display()))?;
        if !resolved.starts_with(canonical_root) {
            anyhow::bail!(
                "manifest {} resolves outside workspace root {}",
                resolved.display(),
                canonical_root.display()
            );
        }
        let text = std::fs::read_to_string(&resolved)
            .with_context(|| format!("read {}", resolved.display()))?;
        Ok(Some(text))
    }

    /// Build `<canonical_root>/deal.toml` from a constant filename plus the
    /// already-canonicalised root. Asserts that the assembled path begins
    /// with the canonical-root prefix before returning.
    fn assemble_manifest_path(canonical_root: &Path) -> Result<PathBuf> {
        let mut p = PathBuf::new();
        p.push(canonical_root);
        p.push(MANIFEST_FILENAME);
        if !p.starts_with(canonical_root) {
            anyhow::bail!(
                "internal: assembled manifest path {} escaped canonical root {}",
                p.display(),
                canonical_root.display()
            );
        }
        Ok(p)
    }
}

/// A discovered DEAL workspace.
pub struct Workspace {
    pub root: PathBuf,
    pub aliases: HashMap<String, String>,
}

impl Workspace {
    /// Construct a Workspace from already-validated inputs. Prefer
    /// `discover_from_root` or `from_manifest_text` over invoking this
    /// directly — they encapsulate the path-safety canonicalisation that
    /// the LSP client → workspace-root boundary requires.
    pub fn new(root: PathBuf, aliases: HashMap<String, String>) -> Self {
        Self { root, aliases }
    }

    /// Build a Workspace from raw manifest text plus an already-validated
    /// canonical root. This is the safe entry point: the caller is
    /// responsible for canonicalizing the root and reading the manifest
    /// file (which keeps untrusted-input → filesystem-sink flow out of
    /// this module). All path-safety logic lives in
    /// `discover_from_root`.
    pub fn from_manifest_text(canonical_root: PathBuf, manifest_text: Option<&str>) -> Self {
        let aliases = manifest_text.and_then(extract_aliases).unwrap_or_default();
        Self::new(canonical_root, aliases)
    }

    /// Discover the workspace rooted at `root`. This is the LSP-facing
    /// entry point used by `backend::initialized`.
    ///
    /// ## Path-safety
    ///
    /// The `root` argument originates from the LSP client (editor's
    /// workspace folder URI). The flow:
    ///
    /// 1. Canonicalize `root` via `std::fs::canonicalize` — resolves all
    ///    `..` segments and symlinks. The result is an absolute path.
    /// 2. Delegate to `try_load_manifest` (in a child `paths` module) for
    ///    the filename-assembly step. That helper hard-codes the filename
    ///    constant and asserts containment before issuing the read.
    /// 3. The returned manifest text (if any) is parsed for aliases.
    ///
    /// A missing deal.toml is OK — the workspace can still be enumerated
    /// (alias table will be empty).
    pub fn discover(root: &Path) -> anyhow::Result<Self> {
        let canonical_root = std::fs::canonicalize(root)
            .with_context(|| format!("canonicalize workspace root {}", root.display()))?;
        let manifest_text = paths::try_load_manifest(&canonical_root)?;
        Ok(Self::from_manifest_text(
            canonical_root,
            manifest_text.as_deref(),
        ))
    }

    /// Enumerate every `*.deal` / `*.dealx` file anywhere under the workspace
    /// root — **layout-agnostic** (Phase 1b). The directory layout is a human
    /// convention, not a contract: a flat directory of arbitrarily-named files
    /// (`gonzo.deal`) is discovered exactly like the recommended
    /// `definitions/` + `model/` layout. Element kind comes from file content,
    /// the namespace from the in-file `package …;` declaration, and
    /// def-vs-composition from the extension — never from directory names.
    ///
    /// Build / vendor / VCS directories are pruned (their subtrees are not
    /// model source). Returns paths sorted lexicographically for deterministic
    /// eager-parse ordering.
    pub fn enumerate_files(&self) -> Vec<PathBuf> {
        /// Directory names whose subtrees are never model source.
        const SKIP_DIRS: [&str; 6] = [
            ".deal",
            ".git",
            "build",
            "target",
            "node_modules",
            ".zig-cache",
        ];
        let mut out = Vec::new();
        for entry in WalkDir::new(&self.root)
            .max_depth(MAX_WALK_DEPTH)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Never prune the root itself (depth 0), even if its own dir
                // name happens to match a skip name. Otherwise prune a skip
                // directory's entire subtree (matched by name at any depth).
                e.depth() == 0
                    || !(e.file_type().is_dir()
                        && e.file_name()
                            .to_str()
                            .is_some_and(|n| SKIP_DIRS.contains(&n)))
            })
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let ext = entry.path().extension().and_then(|s| s.to_str());
            if matches!(ext, Some("deal") | Some("dealx")) {
                out.push(entry.path().to_path_buf());
            }
        }
        out.sort();
        out
    }
}

/// Parse import aliases from a deal.toml string (ADR-0004 P5 WS-A). Honors the
/// authoritative top-level `[aliases]` over the deprecated `[workspace.aliases]`
/// via the shared `deal_config::DealToml` schema (single source of truth with
/// the CLI). Falls back to a lenient scan if the manifest has an unrelated error
/// elsewhere — a manifest typo must not break the editor's alias navigation.
/// Public for tests.
pub fn extract_aliases(deal_toml: &str) -> Option<HashMap<String, String>> {
    let mut out: HashMap<String, String> = HashMap::new();
    if let Ok(manifest) = toml::from_str::<deal_config::DealToml>(deal_toml) {
        // Deprecated [workspace.aliases] first; authoritative top-level overlays.
        for (k, v) in &manifest.workspace.aliases {
            out.insert(k.clone(), v.clone());
        }
        for (k, v) in &manifest.aliases {
            out.insert(k.clone(), v.clone());
        }
    } else if let Ok(val) = toml::from_str::<toml::Value>(deal_toml) {
        for loc in [
            val.get("workspace").and_then(|w| w.get("aliases")),
            val.get("aliases"),
        ] {
            if let Some(tbl) = loc.and_then(|v| v.as_table()) {
                for (k, v) in tbl {
                    if let Some(s) = v.as_str() {
                        out.insert(k.clone(), s.to_string());
                    }
                }
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Resolve dependency package roots whose `.deal` files become EXTERNAL sources
/// in the workspace import closure (ADR-0004 P5 WS-B). Three sources, in order:
///   1. vendored git deps under `.deal/deps/<name>/packages` (after `deal install`);
///   2. `path` dependencies from `deal.toml` (`<path>/packages`);
///   3. a configured stdlib path (`initializationOptions.stdlibPath`) — prefer its
///      `packages/` subdir — so hover/goto into `deal.std.units` resolves without
///      requiring `deal install`.
/// Only the *reachable* packages actually enter the closure (`closure_files`).
fn dependency_roots(root: &Path, stdlib_path: Option<&Path>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();

    let deps_base = root.join(".deal").join("deps");
    if let Ok(entries) = std::fs::read_dir(&deps_base) {
        for e in entries.flatten() {
            let pkgs = e.path().join("packages");
            if pkgs.is_dir() {
                roots.push(pkgs);
            }
        }
    }

    if let Ok(text) = std::fs::read_to_string(root.join("deal.toml")) {
        if let Ok(manifest) = toml::from_str::<deal_config::DealToml>(&text) {
            for dep in manifest.dependencies.values() {
                if let deal_config::Dependency::Path { path } = dep {
                    let pkgs = root.join(path).join("packages");
                    if pkgs.is_dir() {
                        roots.push(pkgs);
                    }
                }
            }
        }
    }

    if let Some(sp) = stdlib_path {
        let pkgs = sp.join("packages");
        roots.push(if pkgs.is_dir() { pkgs } else { sp.to_path_buf() });
    }

    roots
}

/// Eagerly parse every workspace file (silently — no publishDiagnostics) against
/// the workspace **import closure** (workspace files + reachable dependency
/// packages, including the stdlib) and populate the symbol index. Invoked as a
/// background tokio task from `backend::initialized` so the LSP responds to
/// `initialized` immediately.
///
/// The closure's source bytes are cached on `Documents` (`set_closure_external`)
/// as the durable external table the live-strict `did_change` path re-analyzes
/// against (WS-7). Per-file failures are logged and skipped.
pub async fn eager_parse(
    workspace: Arc<Workspace>,
    documents: Arc<Documents>,
    index: Arc<Index>,
    stdlib_path: Option<PathBuf>,
) {
    let workspace_files = workspace.enumerate_files();
    let total = workspace_files.len();
    tracing::info!(
        "eager_parse: starting on {} workspace files under {}",
        total,
        workspace.root.display()
    );

    // Dependency/stdlib package files become EXTERNAL sources; only the
    // reachable packages enter the closure (computed below).
    let dep_roots = dependency_roots(&workspace.root, stdlib_path.as_deref());
    let dep_files = deal_closure::discover_files(&dep_roots);
    tracing::info!(
        "eager_parse: {} dependency file(s) under {} root(s)",
        dep_files.len(),
        dep_roots.len()
    );

    // Build the module map over workspace + deps, then compute the import
    // closure seeded from every workspace file (so the whole project is indexed
    // and only the imported dependency packages — e.g. deal.std.units — load).
    let mut all_files = workspace_files.clone();
    all_files.extend(dep_files.iter().cloned());
    all_files.sort();
    all_files.dedup();
    let map = deal_closure::ModuleMap::build(&all_files);
    let closure = deal_closure::closure_files(&map, &workspace_files);

    // Read each closure file's source once.
    let mut source_by_path: HashMap<PathBuf, Vec<u8>> = HashMap::with_capacity(closure.len());
    for path in &closure {
        match std::fs::read(path) {
            Ok(bytes) => {
                source_by_path.insert(path.clone(), bytes);
            }
            Err(e) => tracing::warn!("eager_parse: read {} failed: {e}", path.display()),
        }
    }

    // Cache the external blob (workspace + reachable deps incl. stdlib) for the
    // did-change live-strict path. Deterministic order = closure order.
    let closure_sources: Vec<Vec<u8>> = closure
        .iter()
        .filter_map(|p| source_by_path.get(p).cloned())
        .collect();
    documents.set_closure_external(closure_sources.clone());

    // Phase 2: analyze each workspace file against the closure external set so
    // cross-file AND stdlib references resolve; populate the index.
    let external_refs: Vec<&[u8]> = closure_sources.iter().map(|v| v.as_slice()).collect();
    let mut indexed = 0usize;
    for path in &workspace_files {
        let Some(bytes) = source_by_path.get(path) else {
            continue;
        };
        let text = match std::str::from_utf8(bytes) {
            Ok(t) => t.to_string(),
            Err(_) => {
                tracing::warn!("eager_parse: {} is not valid UTF-8 — skipping", path.display());
                continue;
            }
        };
        let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(path) else {
            tracing::warn!(
                "eager_parse: cannot build file:// URL for {}",
                path.display()
            );
            continue;
        };
        if let Err(e) = documents
            .open_silent_with_external(uri.clone(), text, &external_refs, &index)
            .await
        {
            tracing::warn!("eager_parse: analyze {uri} failed: {e}");
            continue;
        }
        indexed += 1;
    }
    tracing::info!(
        "eager_parse: completed — {indexed}/{total} workspace files; closure = {} files; index size = {}",
        closure.len(),
        index.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn extract_aliases_round_trip() {
        let toml_src = r#"
[project]
name = "x"

[workspace.aliases]
veh = "packages/vehicle"
ifc = "packages/interfaces"
"#;
        let aliases = extract_aliases(toml_src).expect("aliases parse");
        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases.get("veh"), Some(&"packages/vehicle".to_string()));
        assert_eq!(aliases.get("ifc"), Some(&"packages/interfaces".to_string()));
    }

    #[test]
    fn extract_aliases_missing_section_is_none() {
        let toml_src = r#"
[project]
name = "x"
"#;
        assert!(extract_aliases(toml_src).is_none());
    }

    #[test]
    fn discover_handles_missing_deal_toml() {
        let tmp = std::env::temp_dir().join(format!("deal-lsp-ws-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let ws = Workspace::discover(&tmp).expect("discover ok on empty dir");
        assert!(ws.aliases.is_empty());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn enumerate_walks_packages_and_model() {
        let tmp = std::env::temp_dir().join(format!("deal-lsp-ws-enum-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("packages/sub")).unwrap();
        fs::create_dir_all(tmp.join("model")).unwrap();
        fs::write(tmp.join("packages/a.deal"), "package a;").unwrap();
        fs::write(tmp.join("packages/sub/b.deal"), "package b;").unwrap();
        fs::write(tmp.join("model/c.dealx"), "").unwrap();
        fs::write(tmp.join("packages/skip.txt"), "ignored").unwrap();
        let ws = Workspace {
            root: tmp.clone(),
            aliases: HashMap::new(),
        };
        let files = ws.enumerate_files();
        assert_eq!(files.len(), 3, "expected 3 files, got {files:?}");
        // Lexicographic sort: packages/a.deal, packages/sub/b.deal, model/c.dealx
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"a.deal".to_string()));
        assert!(names.contains(&"b.deal".to_string()));
        assert!(names.contains(&"c.dealx".to_string()));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn enumerate_is_layout_agnostic_flat_and_gonzo() {
        // Phase 1b: a flat directory with arbitrarily-named files and NO
        // packages/ or model/ dirs must still be discovered; build/ is pruned.
        let tmp = std::env::temp_dir().join(format!("deal-lsp-ws-gonzo-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("gonzo.deal"), "package gonzo;").unwrap();
        fs::write(tmp.join("whatever.dealx"), "").unwrap();
        // a .deal under a pruned build/ dir must NOT be discovered
        fs::create_dir_all(tmp.join("build")).unwrap();
        fs::write(tmp.join("build/ignored.deal"), "package x;").unwrap();
        let ws = Workspace {
            root: tmp.clone(),
            aliases: HashMap::new(),
        };
        let files = ws.enumerate_files();
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(
            names.contains(&"gonzo.deal".to_string()),
            "flat gonzo.deal must be found: {names:?}"
        );
        assert!(
            names.contains(&"whatever.dealx".to_string()),
            "arbitrarily-named .dealx must be found"
        );
        assert!(
            !names.contains(&"ignored.deal".to_string()),
            "build/ subtree must be pruned"
        );
        assert_eq!(files.len(), 2, "exactly the 2 root files, got {files:?}");
        let _ = fs::remove_dir_all(&tmp);
    }
}
