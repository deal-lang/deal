//! Integration tests for `cli/src/resolver.rs`.
//!
//! Test coverage:
//!   1. Git dep: clone a local bare repo at tag v0.4.0 → .deal/deps/<name>/ exists
//!   2. Lockfile SHA: deal.lock records the tag's commit SHA
//!   3. Determinism: two consecutive resolve_all calls produce byte-identical deal.lock
//!   4. Path dep: Dependency::Path produces LockedPackage.path set, rev = None (D-66)
//!   5. Path traversal rejection: path = "/etc/passwd" is rejected (T-4-02)
//!   6. Bad URL scheme rejection: url with file:// is allowed; ftp:// is rejected (T-4-03)

use std::path::Path;
use std::fs;

/// Build a minimal bare git repo in `bare_dir`, commit a stub file, and tag it `v0.4.0`.
/// Returns the commit SHA for the tag.
fn create_bare_git_repo(bare_dir: &Path) -> String {
    use std::process::Command;

    // Init a normal (non-bare) repo in a temp working dir, then clone --bare
    let work_dir = bare_dir.parent().unwrap().join("work_tmp");
    fs::create_dir_all(&work_dir).unwrap();

    // Init
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&work_dir)
        .output()
        .expect("git init");

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&work_dir)
        .output()
        .expect("git config email");

    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&work_dir)
        .output()
        .expect("git config name");

    // Create a stub deal.toml
    let stub_toml = work_dir.join("deal.toml");
    fs::write(&stub_toml, "[project]\nname = \"deal-std\"\nversion = \"0.4.0\"\n").unwrap();

    // Create stub si.deal in packages/units/
    let pkg_dir = work_dir.join("packages").join("units");
    fs::create_dir_all(&pkg_dir).unwrap();
    fs::write(pkg_dir.join("si.deal"), "package deal.std.units;\n\nattribute def kg;\n").unwrap();

    // Stage + commit
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&work_dir)
        .output()
        .expect("git add");

    Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&work_dir)
        .output()
        .expect("git commit");

    // Tag
    Command::new("git")
        .args(["tag", "v0.4.0"])
        .current_dir(&work_dir)
        .output()
        .expect("git tag");

    // Get commit SHA for the tag
    let sha_out = Command::new("git")
        .args(["rev-parse", "v0.4.0^{}"])
        .current_dir(&work_dir)
        .output()
        .expect("git rev-parse");
    let sha = String::from_utf8(sha_out.stdout).unwrap().trim().to_string();

    // Clone bare into bare_dir
    Command::new("git")
        .args(["clone", "--bare", work_dir.to_str().unwrap(), bare_dir.to_str().unwrap()])
        .output()
        .expect("git clone --bare");

    // Cleanup work dir
    let _ = fs::remove_dir_all(&work_dir);

    sha
}

/// Create a consumer project deal.toml that depends on the bare repo via file:// URL.
fn create_consumer_project(project_dir: &Path, dep_name: &str, bare_url: &str) {
    fs::create_dir_all(project_dir).unwrap();
    let toml_content = format!(
        "[project]\nname = \"test-consumer\"\nversion = \"0.1.0\"\n\n[dependencies]\n{} = {{ git = \"{}\", tag = \"v0.4.0\" }}\n",
        dep_name, bare_url
    );
    fs::write(project_dir.join("deal.toml"), toml_content).unwrap();
}

#[test]
fn test_git_dep_clone_and_lockfile() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bare_dir = tmp.path().join("deal-std.git");
    let project_dir = tmp.path().join("my-project");

    // Build a bare git repo
    let expected_sha = create_bare_git_repo(&bare_dir);

    // file:// URL to the bare repo
    let bare_url = format!("file://{}", bare_dir.display());
    create_consumer_project(&project_dir, "deal-std", &bare_url);

    // Run resolve_all
    let lock = deal::resolver::resolve_all(&project_dir)
        .expect("resolve_all should succeed");

    // Assertion A: .deal/deps/deal-std/ exists with the stub file
    let dep_dir = project_dir.join(".deal").join("deps").join("deal-std");
    assert!(dep_dir.exists(), ".deal/deps/deal-std/ should exist after install");
    assert!(
        dep_dir.join("packages").join("units").join("si.deal").exists(),
        "cloned repo should contain packages/units/si.deal"
    );

    // Assertion B: deal.lock records the tag's commit SHA
    let lock_path = project_dir.join("deal.lock");
    assert!(lock_path.exists(), "deal.lock should be written");
    let lock_content = fs::read_to_string(&lock_path).unwrap();
    assert!(
        lock_content.contains(&expected_sha),
        "deal.lock should contain commit SHA {expected_sha}, got:\n{lock_content}"
    );

    // Confirm lock struct has the package
    assert_eq!(lock.package.len(), 1, "LockFile should have one package");
    assert_eq!(lock.package[0].name, "deal-std");
    assert_eq!(lock.package[0].rev.as_deref(), Some(expected_sha.as_str()));
}

#[test]
fn test_lockfile_determinism() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bare_dir = tmp.path().join("deal-std-det.git");
    let project_dir = tmp.path().join("consumer-det");

    let _sha = create_bare_git_repo(&bare_dir);
    let bare_url = format!("file://{}", bare_dir.display());
    create_consumer_project(&project_dir, "deal-std", &bare_url);

    // First resolve
    deal::resolver::resolve_all(&project_dir).expect("first resolve_all");
    let lock1 = fs::read_to_string(project_dir.join("deal.lock")).unwrap();

    // Second resolve (repo already cloned)
    deal::resolver::resolve_all(&project_dir).expect("second resolve_all");
    let lock2 = fs::read_to_string(project_dir.join("deal.lock")).unwrap();

    // Assertion C: byte-identical
    assert_eq!(
        lock1, lock2,
        "deal.lock must be byte-identical across two consecutive resolve_all calls (D-18)"
    );
}

#[test]
fn test_path_dep_no_clone() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project_dir = tmp.path().join("path-consumer");
    let sibling_dir = tmp.path().join("my-lib");

    fs::create_dir_all(&project_dir).unwrap();
    fs::create_dir_all(&sibling_dir).unwrap();
    // Write a stub deal.toml in the sibling so it looks like a valid dep
    fs::write(sibling_dir.join("deal.toml"), "[project]\nname=\"my-lib\"\nversion=\"0.1.0\"\n").unwrap();

    // Relative path dep
    let toml_content = "[project]\nname = \"path-consumer\"\nversion = \"0.1.0\"\n\n[dependencies]\nmy-lib = { path = \"../my-lib\" }\n";
    fs::write(project_dir.join("deal.toml"), toml_content).unwrap();

    let lock = deal::resolver::resolve_all(&project_dir).expect("path dep resolve");

    // Assertion D: path dep has path set, rev = None
    assert_eq!(lock.package.len(), 1);
    assert_eq!(lock.package[0].name, "my-lib");
    assert!(lock.package[0].path.is_some(), "path dep should set path");
    assert!(lock.package[0].rev.is_none(), "path dep should have rev = None (D-66 in-place)");

    // .deal/deps/my-lib/ should NOT exist (path deps are referenced in-place)
    let vendored = project_dir.join(".deal").join("deps").join("my-lib");
    assert!(
        !vendored.exists(),
        ".deal/deps/my-lib/ must NOT be created for path deps (D-66)"
    );
}

#[test]
fn test_path_traversal_rejection() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project_dir = tmp.path().join("traversal-consumer");
    fs::create_dir_all(&project_dir).unwrap();

    // Absolute path escape — should be rejected (T-4-02)
    let toml_content = "[project]\nname = \"evil\"\nversion = \"0.1.0\"\n\n[dependencies]\nbad = { path = \"/etc/passwd\" }\n";
    fs::write(project_dir.join("deal.toml"), toml_content).unwrap();

    let result = deal::resolver::resolve_all(&project_dir);
    assert!(
        result.is_err(),
        "path = '/etc/passwd' must be rejected (T-4-02 path traversal guard)"
    );
    let err_msg = format!("{:#}", result.unwrap_err());
    assert!(
        err_msg.contains("path") || err_msg.contains("traversal") || err_msg.contains("absolute"),
        "error message should mention path/traversal issue: {err_msg}"
    );
}

#[test]
fn test_bad_git_scheme_rejected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project_dir = tmp.path().join("scheme-consumer");
    fs::create_dir_all(&project_dir).unwrap();

    // ftp:// scheme — should be rejected (T-4-03)
    let toml_content = "[project]\nname = \"scheme-test\"\nversion = \"0.1.0\"\n\n[dependencies]\nbad = { git = \"ftp://example.com/repo.git\", tag = \"v1.0\" }\n";
    fs::write(project_dir.join("deal.toml"), toml_content).unwrap();

    let result = deal::resolver::resolve_all(&project_dir);
    assert!(
        result.is_err(),
        "ftp:// scheme must be rejected (T-4-03)"
    );
    let err_msg = format!("{:#}", result.unwrap_err());
    assert!(
        err_msg.contains("scheme") || err_msg.contains("url") || err_msg.contains("URL"),
        "error message should mention scheme/url: {err_msg}"
    );
}
