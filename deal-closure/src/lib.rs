//! ADR-0004 shared import-closure walker: module discovery, the workspace
//! module map, and the package-complete closure walker.
//!
//! Extracted from the CLI (`cli/src/closure.rs`, P4) into its own crate (P5
//! WS-A) so both the CLI checker and the LSP load the IDENTICAL reachable set —
//! one walker, two consumers, no drift.
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
#[derive(Debug, Default, Deserialize)]
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
    /// The full index_json envelope bytes (owned; arena already freed). Reused
    /// by `deal check` for workspace-index emission so the analysis loop does
    /// not re-`deal_parse` every file (WS-D). Empty when the file parsed but
    /// produced no symbol table.
    pub index_json: Vec<u8>,
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
    parse_module_from(path, &source)
}

/// Like `parse_module` but parses the GIVEN bytes rather than reading from disk.
/// Used by the LSP to build the closure from live editor buffers (unsaved
/// content) instead of stale disk (ADR-0004 P5 WS-C.2).
pub fn parse_module_from(path: &Path, source: &[u8]) -> Option<ParsedModule> {
    let filename = path.to_str()?;
    let handle = deal_ffi::safe::parse(source, filename)?;
    // index_json is None when the file parsed but produced no symbol table
    // (e.g. parse errors). Such a file still belongs in the map under package
    // "" so the analysis loop reports its diagnostics rather than silently
    // dropping it; it simply contributes no index/imports/exports.
    let env_bytes = deal_ffi::safe::index_json(&handle).unwrap_or_default();
    let env: Envelope = serde_json::from_slice(&env_bytes).unwrap_or_default();
    Some(ParsedModule {
        path: path.to_path_buf(),
        package: env.package,
        imports: env.imports_graph,
        exports: env.exports,
        index_json: env_bytes,
    })
}

/// The sorted, de-duplicated import paths declared by a module, parsed from its
/// index_json envelope. The LSP compares this across an edit to detect when a
/// file's reachability changed and the closure must be rebuilt (WS-C.2).
pub fn imports_from_index_json(index_json: &[u8]) -> Vec<String> {
    let env: Envelope = serde_json::from_slice(index_json).unwrap_or_default();
    let mut imports: Vec<String> = env
        .imports_graph
        .into_iter()
        .map(|e| e.import_path)
        .collect();
    imports.sort();
    imports.dedup();
    imports
}

/// A workspace module map: package FQ → files declaring it, + the parsed cache.
#[derive(Debug, Default)]
pub struct ModuleMap {
    pub by_package: BTreeMap<String, Vec<PathBuf>>,
    pub modules: BTreeMap<PathBuf, ParsedModule>,
}

impl ModuleMap {
    /// Build the map by parsing every file in `files` from disk. A file with an
    /// empty package (e.g. a parse failure) is recorded under "" — it never
    /// silently vanishes from the map.
    pub fn build(files: &[PathBuf]) -> Self {
        let mut map = ModuleMap::default();
        for path in files {
            if let Some(parsed) = parse_module(path) {
                map.insert(path.clone(), parsed);
            }
        }
        map
    }

    /// Build the map from `(path, source_bytes)` pairs rather than from disk, so
    /// the LSP can include unsaved editor-buffer content (WS-C.2).
    pub fn build_from(sources: &[(PathBuf, Vec<u8>)]) -> Self {
        let mut map = ModuleMap::default();
        for (path, bytes) in sources {
            if let Some(parsed) = parse_module_from(path, bytes) {
                map.insert(path.clone(), parsed);
            }
        }
        map
    }

    fn insert(&mut self, path: PathBuf, parsed: ParsedModule) {
        self.by_package
            .entry(parsed.package.clone())
            .or_default()
            .push(path.clone());
        self.modules.insert(path, parsed);
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

/// Compute the package-complete import closure from entry files.
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

/// The resolved workspace-loading context for one invocation: the parsed module
/// map, the package-complete import closure, and the subset of files to actually
/// analyze + report.
#[derive(Debug)]
pub struct LoadPlan {
    pub map: ModuleMap,
    /// Every reachable file (project + dep), package-complete.
    pub closure: Vec<PathBuf>,
    /// Project files to analyze/report/emit: the reachable closure restricted
    /// to project files, PLUS any project file that failed to parse or yielded
    /// no symbol table (so its diagnostics still surface rather than being
    /// silently dropped). Unreachable, cleanly-parsed project files are
    /// excluded — their errors must not surface (entry-point semantics).
    pub analyze: Vec<PathBuf>,
}

/// Build the load plan: discovery is done by the caller (so workspace excludes
/// stay in the caller); this picks entry points, computes the closure, and
/// restricts the analyze set.
///
/// Entry points (ADR-0004 P4 Decision 4): explicit file args are entries (any
/// extension); otherwise the project's `.dealx` files; failing that, all
/// project files (`.deal`-only fallback). Empty entries → `Err` (never a
/// silent exit-0).
pub fn plan_load(
    project_files: &[PathBuf],
    dep_files: &[PathBuf],
    explicit_files: &[PathBuf],
) -> Result<LoadPlan, String> {
    if project_files.is_empty() {
        return Err("no .deal or .dealx files found".to_string());
    }

    // Build the map over the full discovered set (project + deps), once.
    let mut all_files: Vec<PathBuf> = Vec::with_capacity(project_files.len() + dep_files.len());
    all_files.extend_from_slice(project_files);
    all_files.extend_from_slice(dep_files);
    all_files.sort();
    all_files.dedup();
    let map = ModuleMap::build(&all_files);

    // Entry points (Decision 4).
    let entries: Vec<PathBuf> = if !explicit_files.is_empty() {
        explicit_files.to_vec()
    } else {
        let dealx: Vec<PathBuf> = project_files
            .iter()
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("dealx"))
            .cloned()
            .collect();
        if dealx.is_empty() {
            project_files.to_vec()
        } else {
            dealx
        }
    };
    if entries.is_empty() {
        return Err("no entry points (.dealx or .deal) found".to_string());
    }

    let closure = closure_files(&map, &entries);
    let closure_set: BTreeSet<PathBuf> = closure.iter().cloned().collect();
    let project_set: BTreeSet<PathBuf> = project_files.iter().cloned().collect();

    // analyze = reachable project files ∪ project files whose parse yielded no
    // symbol table (parse errors / hard failures), so their diagnostics surface.
    let mut analyze: Vec<PathBuf> = project_files
        .iter()
        .filter(|p| {
            closure_set.contains(*p)
                || match map.modules.get(*p) {
                    None => true,                          // parse returned None
                    Some(m) => m.index_json.is_empty(),    // parsed to no table
                }
        })
        .cloned()
        .collect();
    // Explicit file-arg entries are always analyzed even if they live outside
    // the discovered project roots.
    for e in explicit_files {
        if !analyze.contains(e) && project_set.contains(e) {
            analyze.push(e.clone());
        }
    }
    analyze.sort();
    analyze.dedup();

    Ok(LoadPlan {
        map,
        closure,
        analyze,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic `ParsedModule` (no FFI) for walker tests.
    /// `exports` is a list of `(package, mod, item)` barrel edges.
    fn pm(
        path: &str,
        package: &str,
        imports: &[&str],
        exports: &[(&str, &str, &str)],
    ) -> ParsedModule {
        ParsedModule {
            path: PathBuf::from(path),
            package: package.to_string(),
            imports: imports
                .iter()
                .map(|p| ImportEdge {
                    import_path: p.to_string(),
                    is_wildcard: false,
                    items: vec![],
                })
                .collect(),
            exports: exports
                .iter()
                .map(|(p, m, i)| ExportEdge {
                    package: p.to_string(),
                    module: m.to_string(),
                    item: i.to_string(),
                })
                .collect(),
            // Non-empty so the analyze-set filter treats it as cleanly parsed.
            index_json: b"{}".to_vec(),
        }
    }

    fn map_of(mods: Vec<ParsedModule>) -> ModuleMap {
        let mut map = ModuleMap::default();
        for m in mods {
            map.by_package
                .entry(m.package.clone())
                .or_default()
                .push(m.path.clone());
            map.modules.insert(m.path.clone(), m);
        }
        map
    }

    fn names(files: &[PathBuf]) -> BTreeSet<String> {
        files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect()
    }

    /// Risk F: a single `import app.geo` must pull in EVERY file of `app.geo`
    /// (same-package siblings), and an unreachable package must be excluded.
    #[test]
    fn closure_is_package_complete_and_excludes_unreachable() {
        let map = map_of(vec![
            pm("main.deal", "app", &["app.geo"], &[]),
            pm("geo/p1.deal", "app.geo", &[], &[]),
            pm("geo/p2.deal", "app.geo", &[], &[]), // sibling — not separately imported
            pm("dead/d.deal", "app.dead", &[], &[]), // unreachable
        ]);
        let entries = vec![PathBuf::from("main.deal")];
        let got = names(&closure_files(&map, &entries));
        assert!(got.contains("main.deal"));
        assert!(got.contains("geo/p1.deal"));
        assert!(got.contains("geo/p2.deal"), "same-package sibling must load");
        assert!(!got.contains("dead/d.deal"), "unreachable must not load");
    }

    /// Risk B: a wildcard import of a barrel must transitively pull in the
    /// declaring package of everything the barrel re-exports.
    #[test]
    fn closure_follows_barrel_export_targets_transitively() {
        let map = map_of(vec![
            pm("main.deal", "app", &["app.lib"], &[]),
            // app.lib is a barrel re-exporting Thing from sub-package app.lib.impl.
            pm("lib/index.deal", "app.lib", &[], &[("app.lib", "impl", "Thing")]),
            pm("lib/impl.deal", "app.lib.impl", &[], &[]),
        ]);
        let entries = vec![PathBuf::from("main.deal")];
        let got = names(&closure_files(&map, &entries));
        assert!(got.contains("lib/index.deal"));
        assert!(
            got.contains("lib/impl.deal"),
            "transitive barrel target package must load"
        );
    }

    /// Flat single-package barrel (stdlib style): `export` edge whose `mod` is
    /// empty resolves to the package itself.
    #[test]
    fn export_target_package_falls_back_to_package_for_empty_mod() {
        let map = map_of(vec![pm("u/index.deal", "deal.std.units", &[], &[])]);
        let edge = ExportEdge {
            package: "deal.std.units".to_string(),
            module: String::new(),
            item: "Mass".to_string(),
        };
        assert_eq!(map.export_target_package(&edge), "deal.std.units");
    }
}
