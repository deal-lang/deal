//! deal-config — the `deal.toml` + `deal.lock` schema (ADR-0004 P1), shared by
//! the CLI (P4 closure loader) and the LSP (P5).
//!
//! SCOPE: schema types only. No git2, no clone/network resolution — that stays
//! in the CLI (or a future `deal-resolve`) so the LSP can depend on this crate
//! for the schema/closure without pulling libgit2 into the language server.
//!
//! Strict-everywhere (ADR-0004 P1 decision): every modeled section uses
//! `#[serde(deny_unknown_fields)]`, so a mistyped key is a loud parse error.
//! The whole manifest is modeled — `[project]` metadata, `[workspace]`,
//! `[aliases]`, `[dependencies]`, `[simulations]`, `[views]`, `[build.targets]` —
//! so real manifests (e.g. the cubesat) round-trip under strict parsing.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ─── deal.toml ────────────────────────────────────────────────────────────────

/// Top-level `deal.toml` manifest.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DealToml {
    pub project: ProjectSection,
    #[serde(default)]
    pub workspace: WorkspaceSection,
    /// ADR-0004 R1 / P1: top-level `[aliases]` — import namespace → location
    /// (a directory, relative to the manifest). Distinct from the legacy
    /// `[workspace.aliases]` (migrated to top-level under the P1 schema).
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    /// D-18: `BTreeMap` keeps alphabetical key order (byte-stable output).
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
    #[serde(default)]
    pub simulations: Option<SimulationsSection>,
    #[serde(default)]
    pub views: Option<ViewsSection>,
    #[serde(default)]
    pub build: Option<BuildSection>,
}

/// `[project]` — `name`/`version` required; metadata fields modeled (strict).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSection {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub marking: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// `[workspace]` — module-discovery configuration.
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSection {
    /// ADR-0004 R1 / P1: directories scanned for `*.deal`/`*.dealx`. New honored
    /// field (discovery roots).
    #[serde(default)]
    pub roots: Vec<String>,
    /// Subtrees pruned from discovery (frontier/draft packages).
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Deprecated alias for `roots`, tolerated for one release (callers warn and
    /// treat as `roots`). Modeled explicitly so strict parsing accepts it.
    #[serde(default)]
    pub packages: Vec<String>,
}

/// A `[dependencies]` entry: git, path, or a registry version string.
///
/// Untagged: tried in order Git → Path → Version. A bare string (`"0.1"`)
/// matches only `Version`; an inline table matches `Git` or `Path` by its keys.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Dependency {
    Git {
        git: String,
        #[serde(default)]
        tag: Option<String>,
        #[serde(default)]
        rev: Option<String>,
        #[serde(default)]
        branch: Option<String>,
    },
    Path {
        path: String,
    },
    /// Registry version (e.g. `deal-std = "0.1"`). Reserves a future registry;
    /// resolution is not yet implemented (the CLI reports it as unsupported).
    Version(String),
}

/// `[simulations]` — modeled for strict parsing; contents owned by the sim subsystem.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationsSection {
    #[serde(default)]
    pub registry: Option<String>,
    #[serde(default)]
    pub cache_dir: Option<String>,
}

/// `[build]` — currently just `[build.targets]`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildSection {
    #[serde(default)]
    pub targets: BTreeMap<String, BuildTarget>,
}

/// One `[build.targets.<name>]` entry.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildTarget {
    pub format: String,
    pub output: String,
    #[serde(default)]
    pub template: Option<String>,
}

/// One `[views]` entry (a named cross-cutting view).
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ViewEntry {
    pub source: String,
    pub view: String,
}

/// `[views]` — a reserved `theme` key plus arbitrary named view entries.
///
/// Hand-written `Deserialize` because the reserved-key-plus-arbitrary-entries
/// shape is incompatible with `#[serde(flatten)]` + `deny_unknown_fields`
/// (ADR-0004 P1 note). Entry NAMES are free; each entry's FIELDS are strict.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ViewsSection {
    pub theme: Option<String>,
    pub entries: BTreeMap<String, ViewEntry>,
}

impl<'de> Deserialize<'de> for ViewsSection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        // Deserialize the raw table, then split the reserved `theme` from the
        // arbitrary view entries.
        let mut raw: BTreeMap<String, toml::Value> = BTreeMap::deserialize(deserializer)?;
        let theme = match raw.remove("theme") {
            None => None,
            Some(toml::Value::String(s)) => Some(s),
            Some(_) => return Err(D::Error::custom("`[views].theme` must be a string")),
        };
        let mut entries = BTreeMap::new();
        for (name, value) in raw {
            let entry: ViewEntry = value
                .try_into()
                .map_err(|e| D::Error::custom(format!("invalid view `{name}`: {e}")))?;
            entries.insert(name, entry);
        }
        Ok(ViewsSection { theme, entries })
    }
}

// ─── deal.lock ──────────────────────────────────────────────────────────────

/// The `deal.lock` file. `package` is sorted alphabetically by `name` (D-18).
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockFile {
    pub version: u32,
    pub package: Vec<LockedPackage>,
}

/// One locked package. ALPHABETICAL field order (D-18): git, name, path, rev, tag.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockedPackage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"
[project]
name = "halcyon"
version = "0.1.0"
schema = "deal/0.1"
marking = "Unclassified"
description = "demo"

[workspace]
roots = ["packages", "model"]
exclude = ["packages/behaviors"]

[aliases]
reqs = "packages/requirements"

[dependencies]
deal-stdlib = { path = "../../../deal-stdlib" }
some-git = { git = "https://github.com/x/y", tag = "v1.0" }
deal-std = "0.1"

[simulations]
registry = "simulations/deal.sims.toml"
cache_dir = ".deal/simulations"

[views]
theme = "model/global.dealview"
structure-bdd = { source = "packages/spacecraft/index.deal", view = "model/views/structure-bdd.dealview" }

[build.targets]
sysml-v2 = { format = "json", output = "build/sysml-v2/" }
docs = { format = "html", output = "build/docs/", template = "program-review" }
"#;

    #[test]
    fn parses_full_manifest() {
        let m: DealToml = toml::from_str(FULL).unwrap();
        assert_eq!(m.project.name, "halcyon");
        assert_eq!(m.project.schema.as_deref(), Some("deal/0.1"));
        assert_eq!(m.workspace.roots, vec!["packages", "model"]);
        assert_eq!(m.aliases.get("reqs").map(String::as_str), Some("packages/requirements"));
        assert_eq!(m.dependencies.len(), 3);
        match m.dependencies.get("deal-std").unwrap() {
            Dependency::Version(v) => assert_eq!(v, "0.1"),
            other => panic!("expected Version, got {other:?}"),
        }
        match m.dependencies.get("deal-stdlib").unwrap() {
            Dependency::Path { path } => assert_eq!(path, "../../../deal-stdlib"),
            other => panic!("expected Path, got {other:?}"),
        }
        let views = m.views.unwrap();
        assert_eq!(views.theme.as_deref(), Some("model/global.dealview"));
        assert_eq!(
            views.entries.get("structure-bdd").unwrap().view,
            "model/views/structure-bdd.dealview"
        );
        let build = m.build.unwrap();
        assert_eq!(build.targets.get("docs").unwrap().template.as_deref(), Some("program-review"));
    }

    #[test]
    fn rejects_unknown_workspace_key() {
        let bad = r#"
[project]
name = "x"
version = "0.1.0"

[workspace]
rootz = ["packages"]
"#;
        assert!(toml::from_str::<DealToml>(bad).is_err(), "deny_unknown_fields must reject `rootz`");
    }

    #[test]
    fn dependencies_btreemap_alphabetical() {
        let m: DealToml = toml::from_str(FULL).unwrap();
        let keys: Vec<&String> = m.dependencies.keys().collect();
        assert_eq!(keys, vec!["deal-std", "deal-stdlib", "some-git"]);
    }
}
