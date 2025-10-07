use pretty_assertions::assert_eq;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use stitch::core::*;
use tempfile::TempDir;

fn mkfile(p: &Path) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, "x").unwrap();
}

#[test]
fn scan_dir_children_order_files_then_dirs_sorted() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    mkfile(&root.join("b.rs"));
    mkfile(&root.join("a.rs"));
    // dirs with content so they remain visible
    mkfile(&root.join("bbb/x.rs"));
    mkfile(&root.join("aaa/y.rs"));

    let include: HashSet<String> = HashSet::new();
    let exclude: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude, &ex_dirs, &ex_files);
    let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();

    // Files (sorted) first, then directories (sorted)
    assert_eq!(names, vec!["a.rs", "b.rs", "aaa", "bbb"]);
}

#[test]
fn scan_dir_include_takes_precedence_over_exclude() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    mkfile(&root.join("show.rs"));
    mkfile(&root.join("hide.rs"));

    let include: HashSet<String> = [".rs".into()].into_iter().collect();
    // Even if .rs is in the exclude set, include takes precedence because include_exts is non-empty
    let exclude_exts: HashSet<String> = [".rs".into()].into_iter().collect();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &ex_dirs, &ex_files);
    let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"show.rs"));
    assert!(names.contains(&"hide.rs"));
}

#[test]
fn scan_dir_respects_exclude_files_and_exclude_dirs() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    mkfile(&root.join("LICENSE"));
    mkfile(&root.join("keep.rs"));
    mkfile(&root.join("node_modules/skip.js"));

    let include: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = ["node_modules".into()].into_iter().collect();
    let exclude_files: HashSet<String> = ["LICENSE".into()].into_iter().collect();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);
    let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();

    assert!(names.contains(&"keep.rs"));
    assert!(!names.contains(&"LICENSE"));
    assert!(!names.contains(&"node_modules"));
}

#[test]
fn collect_selected_paths_false_overrides_inheritance() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    mkfile(&root.join("sub/a.txt"));
    mkfile(&root.join("sub/b.txt"));

    let inc: HashSet<String> = HashSet::new();
    let exc: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &inc, &exc, &ex_dirs, &ex_files);

    let mut explicit: HashMap<PathBuf, bool> = HashMap::new();
    let sub = root.join("sub");
    let a = sub.join("a.txt");
    explicit.insert(sub, true); // select the directory
    explicit.insert(a, false); // but explicitly de-select a.txt

    let mut files = Vec::new();
    let mut dirs = Vec::new();
    collect_selected_paths(&tree, &explicit, None, &mut files, &mut dirs);

    let files_unix: Vec<_> = files.iter().map(|p| path_to_unix(p)).collect();
    assert!(
        !files_unix.iter().any(|s| s.ends_with("sub/a.txt")),
        "explicit false should override inherited true"
    );
    assert!(
        files_unix.iter().any(|s| s.ends_with("sub/b.txt")),
        "other files still inherited from selected dir"
    );

    // selected dir appears only if it (recursively) contains a file
    let dirs_unix: Vec<_> = dirs.iter().map(|p| path_to_unix(p)).collect();
    assert!(dirs_unix.iter().any(|s| s.ends_with("sub")));
}

#[test]
fn selected_empty_directory_is_not_emitted_in_dirs_out() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create an empty dir and a non-empty dir; select both.
    fs::create_dir_all(root.join("empty")).unwrap();
    mkfile(&root.join("full/x.rs"));

    let inc: HashSet<String> = HashSet::new();
    let exc: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &inc, &exc, &ex_dirs, &ex_files);

    let mut explicit: HashMap<PathBuf, bool> = HashMap::new();
    explicit.insert(root.join("empty"), true);
    explicit.insert(root.join("full"), true);

    let mut files = Vec::new();
    let mut dirs = Vec::new();
    collect_selected_paths(&tree, &explicit, None, &mut files, &mut dirs);

    let dirs_unix: Vec<_> = dirs.iter().map(|p| path_to_unix(p)).collect();
    assert!(
        !dirs_unix.iter().any(|s| s.ends_with("empty")),
        "empty dir should not be included in dirs_out"
    );
    assert!(
        dirs_unix.iter().any(|s| s.ends_with("full")),
        "non-empty dir should be included in dirs_out"
    );
}
