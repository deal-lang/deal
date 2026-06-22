//! ADR-0004 P4 (WS-B/C): module discovery, the workspace module map, and the
//! import-closure walker.
//!
//! Everything is driven by the **index_json envelope** (P3 emits `package`,
//! `imports_graph`, and `exports`) extracted via the existing FFI — there is no
//! Rust-side DEAL parsing. The closure unit is a **package** (a package spans
//! files): resolving an import to its files, pulling in same-package siblings,
//! and following barrel `export` edges to their declaring packages are all
//! package-keyed (see `closure_files`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// One `imports_graph` edge from the index envelope.
#[derive(Debug, Clone, Deserialize)]
pub struct ImportEdge {
    pub import_path: String,
    #[serde(default)]
    pub is_wildcard: bool,
    #[serde(default)]
    pub items: Vec<String>,
}

/// One `exports` edge from the index envelope (`export mod.{item}` in `package`).
#[derive(Debug, Clone, Deserialize)]
pub struct ExportEdge {
    pub package: String,
    #[serde(rename = "mod")]
    pub module: String,
    pub item: String,
}

/// The subset of the index_json envelope the closure walker consumes.
#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(default)]
    package: String,
    #[serde(default)]
    imports_graph: Vec<ImportEdge>,
    #[serde(default)]
    exports: Vec<ExportEdge>,
}

/// A discovered + parsed module (one file).
#[derive(Debug, Clone)]
pub struct ParsedModule {
    pub path: PathBuf,
    pub package: String,
    pub imports: Vec<ImportEdge>,
    pub exports: Vec<ExportEdge>,
}

/// Discover all `.deal`/`.dealx` files under the given roots (recursive).
pub fn discover_files(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in roots {
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("deal" | "dealx")
            ) {
                out.push(p.to_path_buf());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Parse one file via the FFI and extract its package + import/export edges.
/// Returns `None` on read/parse failure (the caller decides how to surface it).
pub fn parse_module(path: &Path) -> Option<ParsedModule> {
    let source = std::fs::read(path).ok()?;
    let filename = path.to_str()?;
    let handle = deal_ffi::safe::parse(&source, filename)?;
    let env_bytes = deal_ffi::safe::index_json(&handle)?;
    let env: Envelope = serde_json::from_slice(&env_bytes).ok()?;
    Some(ParsedModule {
        path: path.to_path_buf(),
        package: env.package,
        imports: env.imports_graph,
        exports: env.exports,
    })
}

/// A workspace module map: package FQ → files declaring it, + the parsed cache.
#[derive(Debug, Default)]
pub struct ModuleMap {
    pub by_package: BTreeMap<String, Vec<PathBuf>>,
    pub modules: BTreeMap<PathBuf, ParsedModule>,
}

impl ModuleMap {
    /// Build the map by parsing every file in `files`. A file with an empty
    /// package (e.g. a parse failure) is recorded under "" — it never silently
    /// vanishes from the map.
    pub fn build(files: &[PathBuf]) -> Self {
        let mut map = ModuleMap::default();
        for path in files {
            if let Some(parsed) = parse_module(path) {
                map.by_package
                    .entry(parsed.package.clone())
                    .or_default()
                    .push(path.clone());
                map.modules.insert(path.clone(), parsed);
            }
        }
        map
    }

    /// All files declaring package `pkg` (empty slice if unknown).
    pub fn files_for_package(&self, pkg: &str) -> &[PathBuf] {
        self.by_package
            .get(pkg)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Package that declares the symbol re-exported by an `export` edge — i.e.
    /// the package containing `edge.package.module` (a sub-package) or, failing
    /// that, `edge.package` itself (flat single-package, stdlib-style). Used to
    /// follow barrels transitively (WS-C).
    pub fn export_target_package(&self, edge: &ExportEdge) -> String {
        let sub = format!("{}.{}", edge.package, edge.module);
        if self.by_package.contains_key(&sub) {
            sub
        } else {
            edge.package.clone()
        }
    }
}

/// WS-C: compute the package-complete import closure from entry files.
///
/// Reachable packages = transitive closure, from the entry files' own packages,
/// over (a) each module's imports and (b) each reachable package's `export`
/// targets (barrels, transitively). The returned set is EVERY file of EVERY
/// reachable package (package-complete — this is what includes same-package
/// siblings + barrel-target files, which P3 sema's Tier-2 + computeSurfaces
/// require in the external table). Cycle-safe via a visited set over packages.
pub fn closure_files(map: &ModuleMap, entries: &[PathBuf]) -> Vec<PathBuf> {
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut queue: Vec<String> = Vec::new();

    // Seed with the entry files' packages.
    for e in entries {
        if let Some(m) = map.modules.get(e) {
            if visited.insert(m.package.clone()) {
                queue.push(m.package.clone());
            }
        }
    }

    while let Some(pkg) = queue.pop() {
        // Visit every file of this package; enqueue packages reached by its
        // imports and by its barrel export targets.
        for path in map.files_for_package(&pkg) {
            let Some(m) = map.modules.get(path) else {
                continue;
            };
            for imp in &m.imports {
                if visited.insert(imp.import_path.clone()) {
                    queue.push(imp.import_path.clone());
                }
            }
            for exp in &m.exports {
                let target = map.export_target_package(exp);
                if visited.insert(target.clone()) {
                    queue.push(target);
                }
            }
        }
    }

    // Loaded set = every file of every reachable package, plus the entries
    // themselves (an entry whose package is "" would otherwise be omitted).
    let mut files: BTreeSet<PathBuf> = BTreeSet::new();
    for pkg in &visited {
        for path in map.files_for_package(pkg) {
            files.insert(path.clone());
        }
    }
    for e in entries {
        files.insert(e.clone());
    }
    files.into_iter().collect()
}
