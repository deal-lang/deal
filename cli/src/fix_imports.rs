//! `deal fix-imports` (ADR-0004 P6 WS-A): deterministic, compiler-driven import
//! insertion.
//!
//! For each `.deal` file, the set of type names it REFERENCES (from the AST)
//! minus the set already IN SCOPE (declared in its own package, brought by an
//! import, or a prelude primitive) is resolved against a global name→package
//! index built from every discoverable file's `elements`. The missing
//! `import pkg.{Name};` lines are merged in (named imports only, never widened
//! to wildcards). Resolution is exact — no message-text heuristics — and a name
//! declared in more than one package is REPORTED, never guessed.
//!
//! This module is the pure core (no FFI/IO); `main.rs::run_fix_imports` supplies
//! the AST + envelope data via the FFI and the global index via the closure.

use std::collections::{BTreeMap, BTreeSet};

use deal_closure::ImportEdge;

/// Prelude primitives (mirrors `sema.zig::isBuiltinType`) — never imported.
pub const PRIMITIVES: &[&str] = &[
    "Real", "Integer", "String", "Boolean", "Rational", "Number", "Complex", "Natural",
    "ScalarValue",
];

fn is_primitive(name: &str) -> bool {
    PRIMITIVES.contains(&name)
}

/// Recursively collect BARE type-reference names from an AST JSON value:
/// `type_annotation.name_segments`, `specialization.target_segments`, and
/// `redefinition.target_segments`. Only single-segment (unqualified) references
/// are import candidates — a multi-segment reference is already package-qualified.
pub fn collect_referenced_types(ast: &serde_json::Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    walk(ast, &mut out);
    out
}

fn walk(v: &serde_json::Value, out: &mut BTreeSet<String>) {
    match v {
        serde_json::Value::Object(map) => {
            // AST nodes carry their kind as "k" (json.zig writeNode); the index
            // `elements` use "kind" — this walk is over the AST, so "k".
            if let Some(kind) = map.get("k").and_then(|k| k.as_str()) {
                let segs_key = match kind {
                    "type_annotation" => Some("name_segments"),
                    "specialization" | "redefinition" => Some("target_segments"),
                    _ => None,
                };
                if let Some(key) = segs_key {
                    if let Some(arr) = map.get(key).and_then(|s| s.as_array()) {
                        if arr.len() == 1 {
                            if let Some(name) = arr[0].as_str() {
                                out.insert(name.to_string());
                            }
                        }
                    }
                }
            }
            for child in map.values() {
                walk(child, out);
            }
        }
        serde_json::Value::Array(arr) => {
            for child in arr {
                walk(child, out);
            }
        }
        _ => {}
    }
}

/// Build the global name→declaring-packages index from each file's `elements`
/// envelope object. Keys are FQ (`pkg.Name`); `kind=="package"` entries are
/// skipped (a package is not an importable type).
pub fn build_global_index(element_maps: &[serde_json::Value]) -> BTreeMap<String, BTreeSet<String>> {
    let mut index: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for elements in element_maps {
        let Some(obj) = elements.as_object() else {
            continue;
        };
        for (fq, meta) in obj {
            if meta.get("kind").and_then(|k| k.as_str()) == Some("package") {
                continue;
            }
            let Some((pkg, name)) = fq.rsplit_once('.') else {
                continue;
            };
            if pkg.is_empty() || name.is_empty() {
                continue;
            }
            index
                .entry(name.to_string())
                .or_default()
                .insert(pkg.to_string());
        }
    }
    index
}

/// The planned import fixes for one file.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FileFix {
    /// Rewritten source (equals the input when nothing changed).
    pub new_text: String,
    /// (package, sorted names) added or merged — for reporting.
    pub added: Vec<(String, Vec<String>)>,
    /// Referenced names with no declaring package anywhere (likely typos /
    /// genuinely undefined) — reported, not fixed.
    pub unresolved: Vec<String>,
    /// Referenced names declared in >1 package — the human must choose; reported.
    pub ambiguous: Vec<(String, Vec<String>)>,
}

impl FileFix {
    pub fn changed(&self, original: &str) -> bool {
        self.new_text != original
    }
}

/// Plan the import fixes for one file (pure — no FFI/IO).
pub fn plan_file_fix(
    source: &str,
    referenced: &BTreeSet<String>,
    own_package: &str,
    edges: &[ImportEdge],
    global: &BTreeMap<String, BTreeSet<String>>,
) -> FileFix {
    let imported_items: BTreeSet<&str> = edges
        .iter()
        .filter(|e| !e.is_wildcard)
        .flat_map(|e| e.items.iter().map(|s| s.as_str()))
        .collect();
    let wildcard_pkgs: BTreeSet<&str> = edges
        .iter()
        .filter(|e| e.is_wildcard)
        .map(|e| e.import_path.as_str())
        .collect();

    let mut needed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut unresolved: Vec<String> = Vec::new();
    let mut ambiguous: Vec<(String, Vec<String>)> = Vec::new();

    for name in referenced {
        if is_primitive(name) {
            continue;
        }
        let pkgs = global.get(name);
        // In scope locally: declared in this file's own package.
        if pkgs.map(|p| p.contains(own_package)).unwrap_or(false) {
            continue;
        }
        // In scope via import: a selective item, or a wildcard of a declaring package.
        if imported_items.contains(name.as_str()) {
            continue;
        }
        if let Some(pkgs) = pkgs {
            if pkgs.iter().any(|p| wildcard_pkgs.contains(p.as_str())) {
                continue;
            }
        }
        // Needs an import: resolve to a single declaring package (excluding own).
        match pkgs {
            None => unresolved.push(name.clone()),
            Some(pkgs) => {
                let candidates: Vec<&String> =
                    pkgs.iter().filter(|p| p.as_str() != own_package).collect();
                match candidates.len() {
                    0 => unresolved.push(name.clone()),
                    1 => {
                        needed
                            .entry(candidates[0].clone())
                            .or_default()
                            .insert(name.clone());
                    }
                    _ => ambiguous
                        .push((name.clone(), candidates.into_iter().cloned().collect())),
                }
            }
        }
    }

    let added: Vec<(String, Vec<String>)> = needed
        .iter()
        .map(|(p, names)| (p.clone(), names.iter().cloned().collect()))
        .collect();
    let new_text = if needed.is_empty() {
        source.to_string()
    } else {
        apply_imports(source, &needed)
    };

    FileFix {
        new_text,
        added,
        unresolved,
        ambiguous,
    }
}

/// Merge `needed` (package → names) into the source: extend an existing
/// `import pkg.{…};` line for the same package, else insert a new line after the
/// last import (or after the `package …;` line). Preserves a trailing newline.
fn apply_imports(source: &str, needed: &BTreeMap<String, BTreeSet<String>>) -> String {
    let mut lines: Vec<String> = source.lines().map(|s| s.to_string()).collect();
    let mut merged: BTreeSet<String> = BTreeSet::new();

    for (pkg, new_names) in needed {
        if let Some(idx) = lines.iter().position(|l| {
            parse_import_line(l)
                .map(|(p, wc, _)| p == *pkg && !wc)
                .unwrap_or(false)
        }) {
            if let Some((_, _, items)) = parse_import_line(&lines[idx]) {
                let mut all: BTreeSet<String> = items.into_iter().collect();
                all.extend(new_names.iter().cloned());
                let indent: String = lines[idx]
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect();
                lines[idx] = format!("{indent}import {pkg}.{{{}}};", join(&all));
                merged.insert(pkg.clone());
            }
        }
    }

    let mut new_lines: Vec<String> = needed
        .iter()
        .filter(|(p, _)| !merged.contains(*p))
        .map(|(p, names)| format!("import {p}.{{{}}};", join(names)))
        .collect();
    new_lines.sort();

    if !new_lines.is_empty() {
        let at = insert_index(&lines);
        for (i, nl) in new_lines.into_iter().enumerate() {
            lines.insert(at + i, nl);
        }
    }

    let mut out = lines.join("\n");
    if source.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn join(names: &BTreeSet<String>) -> String {
    names.iter().cloned().collect::<Vec<_>>().join(", ")
}

/// Parse `import <path>.{a, b};` or `import <path>.*;` → (package, is_wildcard, items).
fn parse_import_line(line: &str) -> Option<(String, bool, Vec<String>)> {
    let rest = line.trim().strip_prefix("import ")?.trim();
    if let Some(pkg) = rest.strip_suffix(".*;") {
        return Some((pkg.trim().to_string(), true, vec![]));
    }
    let rest = rest.strip_suffix(';')?.trim();
    let brace = rest.find(".{")?;
    let pkg = rest[..brace].trim().to_string();
    let items_str = rest[brace + 2..].strip_suffix('}')?;
    let items = items_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Some((pkg, false, items))
}

/// Line index for new imports: after the last `import`, else after `package …;`,
/// else the top of the file.
fn insert_index(lines: &[String]) -> usize {
    let mut last_import = None;
    let mut package_line = None;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim_start();
        if t.starts_with("import ") {
            last_import = Some(i);
        } else if package_line.is_none() && t.starts_with("package ") {
            package_line = Some(i);
        }
    }
    last_import
        .map(|i| i + 1)
        .or(package_line.map(|i| i + 1))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(path: &str, wildcard: bool, items: &[&str]) -> ImportEdge {
        ImportEdge {
            import_path: path.to_string(),
            is_wildcard: wildcard,
            items: items.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn global(pairs: &[(&str, &[&str])]) -> BTreeMap<String, BTreeSet<String>> {
        let mut g: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (name, pkgs) in pairs {
            g.insert(
                name.to_string(),
                pkgs.iter().map(|s| s.to_string()).collect(),
            );
        }
        g
    }

    fn refs(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn collects_bare_type_refs_from_ast() {
        // AST nodes use "k" for kind (json.zig writeNode), matching reality.
        let ast = serde_json::json!({
            "k": "part_def",
            "members": [
                {"k": "attribute_usage", "type": {"k": "type_annotation", "name_segments": ["Mass"]}},
                {"k": "specialization", "target_segments": ["Vehicle"]},
                {"k": "type_annotation", "name_segments": ["deal", "std", "units", "Power"]}
            ]
        });
        let got = collect_referenced_types(&ast);
        assert!(got.contains("Mass"));
        assert!(got.contains("Vehicle"));
        // Multi-segment (qualified) ref is NOT collected as a bare candidate.
        assert!(!got.contains("Power"));
    }

    #[test]
    fn global_index_skips_packages_and_keys_by_name() {
        let elements = serde_json::json!({
            "deal.std.units.Mass": {"kind": "dimension_def"},
            "deal.std.units": {"kind": "package"},
            "app.geo.Point": {"kind": "part_def"}
        });
        let idx = build_global_index(&[elements]);
        assert_eq!(idx.get("Mass").unwrap().iter().next().unwrap(), "deal.std.units");
        assert_eq!(idx.get("Point").unwrap().iter().next().unwrap(), "app.geo");
        assert!(!idx.contains_key("units")); // the package entry was skipped
    }

    #[test]
    fn inserts_missing_import_after_package_line() {
        let src = "package app;\npart def T {\n    public (\n        attribute m : Mass [1];\n    )\n}\n";
        let g = global(&[("Mass", &["deal.std.units"])]);
        let fix = plan_file_fix(src, &refs(&["Mass"]), "app", &[], &g);
        assert!(fix.new_text.contains("import deal.std.units.{Mass};"));
        // Inserted on line 1 (right after `package app;`).
        assert_eq!(fix.new_text.lines().nth(1).unwrap(), "import deal.std.units.{Mass};");
        assert_eq!(fix.added, vec![("deal.std.units".to_string(), vec!["Mass".to_string()])]);
    }

    #[test]
    fn merges_into_existing_same_package_import() {
        let src = "package app;\nimport deal.std.units.{kg, V};\nattribute def a : Mass;\n";
        let g = global(&[("Mass", &["deal.std.units"])]);
        let fix = plan_file_fix(src, &refs(&["Mass"]), "app", &[edge("deal.std.units", false, &["kg", "V"])], &g);
        // Merged into the existing line, alphabetically, no duplicate import line.
        assert!(fix.new_text.contains("import deal.std.units.{Mass, V, kg};"));
        assert_eq!(fix.new_text.matches("import deal.std.units").count(), 1);
    }

    #[test]
    fn skips_in_scope_local_imported_and_primitive() {
        let g = global(&[
            ("Mass", &["deal.std.units"]),
            ("Local", &["app"]),
            ("Wild", &["other.pkg"]),
        ]);
        let edges = [
            edge("deal.std.units", false, &["Mass"]), // selective import
            edge("other.pkg", true, &[]),             // wildcard
        ];
        let fix = plan_file_fix(
            "package app;\n",
            &refs(&["Mass", "Local", "Wild", "Real"]),
            "app",
            &edges,
            &g,
        );
        assert!(fix.added.is_empty(), "nothing to add: {:?}", fix.added);
        assert!(fix.unresolved.is_empty());
        assert!(!fix.changed("package app;\n"));
    }

    #[test]
    fn reports_unresolved_and_ambiguous() {
        let g = global(&[("Dup", &["pkg.a", "pkg.b"])]);
        let fix = plan_file_fix("package app;\n", &refs(&["Dup", "Nope"]), "app", &[], &g);
        assert!(fix.unresolved.contains(&"Nope".to_string()));
        assert_eq!(fix.ambiguous.len(), 1);
        assert_eq!(fix.ambiguous[0].0, "Dup");
        // Ambiguous/unresolved names are NOT auto-imported.
        assert!(fix.added.is_empty());
    }

    #[test]
    fn idempotent_when_already_clean() {
        let src = "package app;\nimport deal.std.units.{Mass};\nattribute def a : Mass;\n";
        let g = global(&[("Mass", &["deal.std.units"])]);
        let fix = plan_file_fix(src, &refs(&["Mass"]), "app", &[edge("deal.std.units", false, &["Mass"])], &g);
        assert!(!fix.changed(src), "no change expected; got:\n{}", fix.new_text);
    }
}
