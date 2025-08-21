use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use stitch::core::{ensure_workspace_dir, workspace_dir};

fn read(p: &PathBuf) -> String {
    fs::read_to_string(p).unwrap_or_default()
}

#[test]
fn appends_gitignore_rule_once_on_first_workspace_creation() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Seed a root .gitignore without the Stitch rule (no trailing newline on purpose)
    let gi = root.join(".gitignore");
    fs::write(&gi, "target\nnode_modules").unwrap();

    // Sanity: workspace dir does not exist yet
    let ws = workspace_dir(root);
    assert!(!ws.exists());

    // First creation should append the rule
    ensure_workspace_dir(root).expect("create workspace");

    let content = read(&gi);
    assert!(
        content.contains("# Stitch workspace (per-user)"),
        "expected friendly comment block to be appended"
    );
    assert!(
        content.contains(".stitchworkspace/local/"),
        "expected canonical ignore rule to be appended"
    );

    // Should not duplicate on subsequent calls
    ensure_workspace_dir(root).expect("idempotent");
    let content2 = read(&gi);
    assert_eq!(
        content, content2,
        "ensure_workspace_dir must not duplicate the ignore rule"
    );
}

#[test]
fn does_not_append_if_variant_rule_already_present_without_trailing_slash() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Pre-seed with a common variant: no trailing slash
    let gi = root.join(".gitignore");
    fs::write(&gi, ".stitchworkspace/local\n").unwrap();

    let before = read(&gi);
    ensure_workspace_dir(root).expect("create workspace");
    let after = read(&gi);

    assert_eq!(
        before, after,
        "variant entry without trailing slash should be treated as present; no extra rule appended"
    );
}

#[test]
fn does_not_append_if_double_star_variant_is_present() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Another variant: glob with **
    let gi = root.join(".gitignore");
    fs::write(&gi, "**/.stitchworkspace/local\n").unwrap();

    let before = read(&gi);
    ensure_workspace_dir(root).expect("create workspace");
    let after = read(&gi);

    assert_eq!(
        before, after,
        "double-star variant should be treated as present; no extra rule appended"
    );
}

#[test]
fn does_nothing_when_no_gitignore_exists() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // No .gitignore at repo root
    let gi = root.join(".gitignore");
    assert!(!gi.exists());

    ensure_workspace_dir(root).expect("create workspace");

    // Still should not exist (we don't create .gitignore proactively)
    assert!(
        !gi.exists(),
        "ensure_workspace_dir should not create .gitignore when absent"
    );
}

#[test]
fn does_not_append_if_workspace_dir_preexisted() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let gi = root.join(".gitignore");

    // Pre-create both .gitignore and the workspace dir (simulates a repo that already had it)
    fs::write(&gi, "target/\n").unwrap();
    fs::create_dir_all(workspace_dir(root)).unwrap();

    let before = read(&gi);
    ensure_workspace_dir(root).expect("ensure");
    let after = read(&gi);

    assert_eq!(
        before, after,
        "rule should only be appended on first creation of .stitchworkspace"
    );
}
