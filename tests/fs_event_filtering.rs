use std::collections::HashSet;
use std::path::PathBuf;
use tempfile::TempDir;

use stitch::core::is_event_path_relevant;

#[test]
fn excluded_directory_changes_are_not_relevant() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    // Simulate a path inside an excluded dir (e.g., "target")
    let p = root.join("target").join("foo.rs");

    let include_exts: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = std::iter::once(String::from("target")).collect();
    let exclude_files: HashSet<String> = HashSet::new();

    assert!(
        !is_event_path_relevant(
            &root,
            &p,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "events under excluded directory must be ignored"
    );
}

#[test]
fn excluded_filename_is_not_relevant() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    let p = root.join("LICENSE");

    let include_exts: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = HashSet::new();
    let exclude_files: HashSet<String> = std::iter::once(String::from("LICENSE")).collect();

    assert!(
        !is_event_path_relevant(
            &root,
            &p,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "excluded basename must be ignored"
    );
}

#[test]
fn include_mode_only_accepts_listed_extensions() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    let rs_file = root.join("src").join("main.rs");
    let txt_file = root.join("README.txt");
    let dir_path = root.join("src"); // directory change

    let include_exts: HashSet<String> = std::iter::once(String::from(".rs")).collect();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = HashSet::new();
    let exclude_files: HashSet<String> = HashSet::new();

    assert!(
        is_event_path_relevant(
            &root,
            &rs_file,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "matching included extension should be relevant"
    );
    assert!(
        !is_event_path_relevant(
            &root,
            &txt_file,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "non-included extension should not be relevant under include mode"
    );
    assert!(
        !is_event_path_relevant(
            &root,
            &dir_path,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "directory changes should not trip include-mode when no file extension"
    );
}

#[test]
fn project_root_self_change_is_always_relevant() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    // abs_path == project_root
    let p: PathBuf = root.clone();

    let include_exts: HashSet<String> = std::iter::once(String::from(".rs")).collect();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = std::iter::once(String::from("target")).collect();
    let exclude_files: HashSet<String> = std::iter::once(String::from("LICENSE")).collect();

    assert!(
        is_event_path_relevant(
            &root,
            &p,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "root-level metadata changes should force a refresh"
    );
}
