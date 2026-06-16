//! Package resolver for DEAL projects.
//!
//! Implements `deal install` dependency resolution (D-66/D-68):
//!   - Parse `deal.toml [dependencies]` into typed `Dependency` enum
//!   - Git deps: clone via git2 into `.deal/deps/<name>/` at exact ref
//!   - Path deps: reference in-place without copying
//!   - Generate a SHA-pinned, deterministic `deal.lock` (D-18 alphabetical)
//!
//! Security mitigations (threat model 04-02):
//!   T-4-02: Path traversal guard — absolute system-path deps rejected
//!   T-4-03: URL scheme validation — only https://, git@, ssh://, file:// accepted
//!   T-4-04: BTreeMap + sorted Vec<LockedPackage> ensures byte-stable deal.lock
//!
//! Accepted git URL schemes:
//!   - https://...          (TLS-secured HTTPS)
//!   - ssh://...            (SSH transport)
//!   - git@...              (SSH shorthand, e.g. git@github.com:org/repo.git)
//!   - file://...           (local filesystem — used in tests and local setups)
//!
//! Rejected: ftp://, http://, anything else that does not match the above.
//!
//! Path dependency policy:
//!   - Relative paths (../sibling, ./local) are accepted; they are resolved
//!     relative to the project root and only used as a reference (not cloned).
//!   - Absolute paths starting with a system prefix (/etc, /usr, /sys, /proc,
//!     /dev, /bin, /sbin, /lib, /root, /boot) are rejected as a traversal guard.
//!   - Absolute paths that are not system prefixes are also rejected to enforce
//!     portability. Users should use relative paths for all local dependencies.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Context as _};
use git2::Repository;
use serde::{Deserialize, Serialize};

// ─── deal.toml structures ─────────────────────────────────────────────────────

/// Top-level `deal.toml` manifest.
///
/// Only the fields relevant to the resolver are parsed here.
/// Additional sections (`[workspace]`, `[build.targets]`, etc.) are
/// deserialized into the catch-all `extra` to avoid failing on unknown keys.
#[derive(Debug, Deserialize)]
pub struct DealToml {
    pub project: ProjectSection,
    #[serde(default)]
    pub workspace: WorkspaceSection,
    /// D-18: BTreeMap ensures alphabetical key order; HashMap is forbidden for
    /// any serialized map (Pitfall 6 mitigation).
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
}

/// `[project]` section — required fields only (name, version are the minimum).
#[derive(Debug, Deserialize)]
pub struct ProjectSection {
    pub name: String,
    pub version: String,
}

/// `[workspace]` section — optional, defaults to no packages list.
#[derive(Debug, Deserialize, Default)]
pub struct WorkspaceSection {
    #[serde(default)]
    pub packages: Vec<String>,
    /// Paths (relative to the project root) excluded from `deal check` directory
    /// expansion — e.g. frontier/draft packages that intentionally do not parse
    /// on the current grammar.
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// A single dependency entry in `[dependencies]`.
///
/// Serde `untagged` deserialization: tries `Git` first, then `Path`.
/// Inline table: `dep = { git = "...", tag = "v1.0" }` → `Dependency::Git`.
/// Inline table: `dep = { path = "../sibling" }` → `Dependency::Path`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    /// A git-based dependency (D-66 vendor clone).
    Git {
        git: String,
        tag: Option<String>,
        rev: Option<String>,
        branch: Option<String>,
    },
    /// A local-path dependency (D-66 in-place reference, no clone).
    Path { path: String },
}

// ─── deal.lock structures ─────────────────────────────────────────────────────

/// The `deal.lock` file structure.
///
/// `version` is always written as `1` for this format revision.
/// `package` is sorted alphabetically by `name` for D-18 byte-stability.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockFile {
    pub version: u32,
    /// ALPHABETICAL sort by `name` before serializing — D-18 invariant.
    pub package: Vec<LockedPackage>,
}

/// A single locked package entry in `deal.lock`.
///
/// Fields are listed in ALPHABETICAL order (D-18): git, name, path, rev, tag.
/// The `Ord` derive combined with alphabetical field declaration ensures that
/// `package.sort()` produces a deterministic, alphabetically-keyed TOML output.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockedPackage {
    // ALPHABETICAL field order for D-18 — do not reorder.
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

// ─── Security validation ──────────────────────────────────────────────────────

/// Validate that a git URL uses an accepted transport scheme.
///
/// Accepted: `https://`, `ssh://`, `git@` (SSH shorthand), `file://`.
/// Rejected: everything else (ftp://, http://, etc.).
/// This is T-4-03 mitigation.
fn validate_git_url_scheme(url: &str) -> anyhow::Result<()> {
    let accepted = url.starts_with("https://")
        || url.starts_with("ssh://")
        || url.starts_with("git@")
        || url.starts_with("file://");
    if !accepted {
        return Err(anyhow!(
            "rejected git URL with unsupported scheme: '{}' — \
             accepted schemes are https://, ssh://, git@, file:// (T-4-03)",
            url
        ));
    }
    Ok(())
}

/// System-path prefixes that are always rejected as path dependency roots.
///
/// This is not an exhaustive security boundary — it is a sanity guard to
/// catch obvious accidents like `path = "/etc/passwd"`. The real protection
/// is the "no absolute paths" rule: any absolute path is rejected.
const SYSTEM_PATH_PREFIXES: &[&str] = &[
    "/etc", "/usr", "/sys", "/proc", "/dev", "/bin", "/sbin", "/lib", "/root", "/boot", "/var",
    "/tmp",
];

/// Validate a path dependency string using string-only checks (no filesystem access).
///
/// Policy (T-4-02 mitigation — first gate):
///   - Absolute paths (starting with `/` on Unix, or a drive letter `X:\` on Windows)
///     are rejected unconditionally for portability and security.
///   - System prefixes (even if the string is not fully absolute) are rejected.
///   - Relative paths are accepted at this phase; a second canonicalization gate in
///     `resolve_all` asserts that the fully-resolved path is not a system path.
///
/// Deliberately avoids constructing `Path`/`PathBuf` from `dep_path` before the
/// validation is complete — string comparisons only so the taint does not
/// propagate to filesystem-access primitives (CWE-22).
fn validate_dep_path_lexical(dep_path: &str) -> anyhow::Result<()> {
    // Check for absolute path indicators using string ops only (not Path::new).
    // On Unix an absolute path starts with '/'. On Windows it starts with 'X:\' or '\\'.
    let is_abs = dep_path.starts_with('/')
        || dep_path.starts_with('\\')
        || (dep_path.len() >= 3
            && dep_path.as_bytes()[1] == b':'
            && (dep_path.as_bytes()[2] == b'\\' || dep_path.as_bytes()[2] == b'/'));

    if is_abs {
        // Extra-specific error for well-known system prefixes.
        for prefix in SYSTEM_PATH_PREFIXES {
            if dep_path.starts_with(prefix) {
                return Err(anyhow!(
                    "path traversal guard: dependency path '{}' resolves to a system path \
                     (prefix '{}') — absolute paths are not allowed (T-4-02)",
                    dep_path,
                    prefix
                ));
            }
        }
        return Err(anyhow!(
            "path traversal guard: dependency path '{}' is absolute — \
             use a relative path (e.g. '../sibling') instead (T-4-02)",
            dep_path
        ));
    }
    Ok(())
}

/// Assert that a resolved (canonicalized) path does not land on a system prefix.
///
/// This is the second T-4-02 gate, applied after `canonicalize()` turns a
/// relative path into an absolute one.  It catches symlink chains that resolve
/// to `/etc/…` even when the lexical form looks benign (e.g. `path = "../etc"`
/// with a symlink farm).  It also prevents accidental system-path access when
/// the project root is unusually placed.
fn assert_not_system_path(resolved: &Path) -> anyhow::Result<()> {
    let resolved_str = resolved.to_string_lossy();
    for prefix in SYSTEM_PATH_PREFIXES {
        // WR-09: anchor the match on a path boundary. A bare `starts_with(prefix)`
        // both (a) false-positively rejects legitimate siblings such as
        // `/etcetera-project` or `/usrlocal`, and (b) is a poor security primitive.
        // Reject only when the resolved path is EXACTLY a system prefix or sits
        // strictly underneath it (`/etc/...`), never when it merely shares a
        // textual prefix without a separator boundary.
        let is_system =
            resolved_str == *prefix || resolved_str.starts_with(&format!("{}/", prefix));
        if is_system {
            return Err(anyhow!(
                "path traversal guard: resolved path '{}' is a system path (prefix '{}') — \
                 dependency paths must not reference system directories (T-4-02)",
                resolved.display(),
                prefix
            ));
        }
    }
    Ok(())
}

// ─── Git resolution ───────────────────────────────────────────────────────────

/// Clone or re-use a git dependency and return the exact commit SHA.
///
/// Algorithm (mirrors RESEARCH §Code Examples):
///   1. If `dest` exists, open the repository; otherwise clone it.
///   2. Resolve the ref: `refs/tags/<tag>` → `refs/heads/<branch>` → `rev` → `HEAD`.
///   3. `peel_to_commit()` to obtain the concrete commit object.
///   4. `set_head_detached(commit_id)` + `checkout_head(None)` to materialise the tree.
///   5. Return the commit SHA as a 40-hex string.
pub fn resolve_git_dep(
    git: &str,
    tag: Option<&str>,
    rev: Option<&str>,
    branch: Option<&str>,
    dest: &Path,
) -> anyhow::Result<String> {
    // T-4-03: scheme validation before any network/filesystem access.
    validate_git_url_scheme(git)?;

    // Open or clone.
    // A non-bare clone stores HEAD at <dest>/.git/HEAD; a bare clone stores it
    // at <dest>/HEAD. Check both so that previously-cloned deps are reused.
    let is_existing_repo =
        dest.exists() && (dest.join(".git").join("HEAD").exists() || dest.join("HEAD").exists());
    let repo = if is_existing_repo {
        // Looks like a git repo already exists — open it.
        Repository::open(dest)
            .with_context(|| format!("failed to open existing repo at {}", dest.display()))?
    } else {
        // Clone fresh.
        std::fs::create_dir_all(dest)
            .with_context(|| format!("cannot create dep dir {}", dest.display()))?;
        Repository::clone(git, dest)
            .with_context(|| format!("failed to clone '{}' into '{}'", git, dest.display()))?
    };

    // Resolve the reference to an object.
    let reference = if let Some(t) = tag {
        format!("refs/tags/{}", t)
    } else if let Some(b) = branch {
        format!("refs/heads/{}", b)
    } else if let Some(r) = rev {
        r.to_string()
    } else {
        "HEAD".to_string()
    };

    let obj = repo
        .revparse_single(&reference)
        .with_context(|| format!("cannot resolve ref '{}' in '{}'", reference, git))?;

    let commit = obj
        .peel_to_commit()
        .with_context(|| format!("ref '{}' does not point to a commit", reference))?;

    let sha = commit.id().to_string();

    // Detach HEAD at the exact commit and materialise the working tree.
    repo.set_head_detached(commit.id())
        .with_context(|| format!("set_head_detached failed for {sha}"))?;
    repo.checkout_head(None)
        .with_context(|| format!("checkout_head failed for {sha}"))?;

    Ok(sha)
}

// ─── Top-level resolver ───────────────────────────────────────────────────────

/// Resolve all dependencies in `project_dir/deal.toml` and write `deal.lock`.
///
/// For each `[dependencies]` entry:
///   - `Dependency::Git`: vendor clone into `<project_dir>/.deal/deps/<name>/`
///     at the specified tag/rev/branch, record SHA in lock.
///   - `Dependency::Path`: validate path (reject absolute), record in lock with
///     `path` set and `rev = None` (D-66 in-place, no clone).
///
/// Returns the constructed `LockFile` on success.
///
/// The `deal.lock` is always written (even if content is unchanged); callers
/// can compare old vs new bytes to detect changes.
pub fn resolve_all(project_dir: &Path) -> anyhow::Result<LockFile> {
    // Canonicalize project_dir before any joins so that symlinks and `..` components
    // in the caller-supplied path are resolved to a real, trusted absolute path.
    // This is the CWE-22 guard for the project_dir taint source — all subsequent
    // joins use the canonicalized root, not the raw caller-supplied value.
    let project_dir = project_dir.canonicalize().with_context(|| {
        format!(
            "cannot canonicalize project dir '{}'",
            project_dir.display()
        )
    })?;
    let project_dir = project_dir.as_path();

    let toml_path = project_dir.join("deal.toml");
    let toml_bytes = std::fs::read(&toml_path)
        .with_context(|| format!("cannot read {}", toml_path.display()))?;
    let toml_str = std::str::from_utf8(&toml_bytes)
        .with_context(|| format!("deal.toml is not valid UTF-8: {}", toml_path.display()))?;

    let manifest: DealToml = toml::from_str(toml_str)
        .with_context(|| format!("failed to parse {}", toml_path.display()))?;

    let deps_base = project_dir.join(".deal").join("deps");
    std::fs::create_dir_all(&deps_base)
        .with_context(|| format!("cannot create {}", deps_base.display()))?;

    let mut packages: Vec<LockedPackage> = Vec::new();

    // BTreeMap iteration is already alphabetical by key (D-18).
    for (name, dep) in &manifest.dependencies {
        match dep {
            Dependency::Git {
                git,
                tag,
                rev,
                branch,
            } => {
                let dest = deps_base.join(name);
                let sha = resolve_git_dep(
                    git,
                    tag.as_deref(),
                    rev.as_deref(),
                    branch.as_deref(),
                    &dest,
                )?;
                packages.push(LockedPackage {
                    git: Some(git.clone()),
                    name: name.clone(),
                    path: None,
                    rev: Some(sha),
                    tag: tag.clone(),
                });
            }
            Dependency::Path { path } => {
                // T-4-02 first gate: lexical validation before any filesystem operation.
                validate_dep_path_lexical(path)?;
                // Resolve the canonical absolute path for the lock entry.
                //
                // CR-01: do NOT fall back to the un-normalized joined path when
                // canonicalize() fails. The previous fallback meant a non-existent
                // or symlink-escaping target (e.g. `../../../../etc/shadow`) was
                // never normalized before the system-path check, so the `..`
                // components survived and the prefix-denylist was trivially
                // bypassed (CWE-22). Fail closed: a path dependency MUST resolve
                // to a real on-disk path so the `..` segments are collapsed before
                // any security assertion runs.
                let resolved = project_dir.join(path);
                let canonical = resolved.canonicalize().with_context(|| {
                    format!(
                        "path dependency '{}' does not resolve to a real path — \
                         path dependencies must point at an existing directory (T-4-02)",
                        path
                    )
                })?;
                // T-4-02 second gate: assert the canonicalized path is not a system path.
                // After canonicalize() succeeds the path is fully normalized (no `..`
                // components, symlinks resolved), so the boundary-anchored prefix check
                // is now operating on the true target.
                assert_not_system_path(&canonical)?;
                packages.push(LockedPackage {
                    git: None,
                    name: name.clone(),
                    path: Some(canonical.to_string_lossy().into_owned()),
                    rev: None,
                    tag: None,
                });
            }
        }
    }

    // D-18: sort by name for byte-stable deal.lock output (Pitfall 6 / T-4-04).
    packages.sort_by(|a, b| a.name.cmp(&b.name));

    let lock = LockFile {
        version: 1,
        package: packages,
    };

    // Serialize to TOML and write deal.lock.
    let lock_toml = toml::to_string(&lock).with_context(|| "failed to serialize deal.lock")?;
    let lock_path = project_dir.join("deal.lock");
    std::fs::write(&lock_path, &lock_toml)
        .with_context(|| format!("cannot write {}", lock_path.display()))?;

    Ok(lock)
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_git_url_scheme_accepted() {
        assert!(validate_git_url_scheme("https://github.com/org/repo.git").is_ok());
        assert!(validate_git_url_scheme("ssh://git@github.com/org/repo.git").is_ok());
        assert!(validate_git_url_scheme("git@github.com:org/repo.git").is_ok());
        assert!(validate_git_url_scheme("file:///home/user/myrepo.git").is_ok());
    }

    #[test]
    fn test_validate_git_url_scheme_rejected() {
        assert!(validate_git_url_scheme("ftp://example.com/repo.git").is_err());
        assert!(validate_git_url_scheme("http://example.com/repo.git").is_err());
        assert!(validate_git_url_scheme("rsync://example.com/repo.git").is_err());
    }

    #[test]
    fn test_validate_dep_path_accepted() {
        assert!(validate_dep_path_lexical("../sibling").is_ok());
        assert!(validate_dep_path_lexical("./local").is_ok());
        assert!(validate_dep_path_lexical("relative/path").is_ok());
    }

    #[test]
    fn test_validate_dep_path_rejected() {
        assert!(validate_dep_path_lexical("/etc/passwd").is_err());
        assert!(validate_dep_path_lexical("/usr/local/share").is_err());
        assert!(validate_dep_path_lexical("/home/user/project").is_err());
    }

    #[test]
    fn btreemap_in_dealtom_is_alphabetical() {
        let toml_str = r#"
[project]
name = "test"
version = "0.1.0"

[dependencies]
zzz-dep = { git = "https://github.com/org/zzz.git", tag = "v1.0" }
aaa-dep = { path = "../aaa" }
mmm-dep = { git = "https://github.com/org/mmm.git", rev = "abc123" }
"#;
        let manifest: DealToml = toml::from_str(toml_str).unwrap();
        let keys: Vec<&String> = manifest.dependencies.keys().collect();
        assert_eq!(
            keys,
            vec!["aaa-dep", "mmm-dep", "zzz-dep"],
            "BTreeMap must iterate alphabetically"
        );
    }
}
