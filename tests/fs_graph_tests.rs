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
fn test_scan_dir_include_exclude() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Layout:
    // root/
    //   a.txt
    //   b.rs
    //   c.lock
    //   node_modules/x.js
    //   sub/deep/d.rs
    //   sub/keep.md
    mkfile(&root.join("a.txt"));
    mkfile(&root.join("b.rs"));
    mkfile(&root.join("c.lock"));
    mkfile(&root.join("node_modules/x.js"));
    mkfile(&root.join("sub/deep/d.rs"));
    mkfile(&root.join("sub/keep.md"));

    let include: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = std::iter::once(String::from(".lock")).collect();
    let exclude_dirs: HashSet<String> = std::iter::once(String::from("node_modules")).collect();
    let exclude_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);

    // root should contain a.txt, b.rs and sub (with children), but not c.lock or node_modules
    let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"a.txt"));
    assert!(names.contains(&"b.rs"));
    assert!(names.contains(&"sub"));
    assert!(!names.contains(&"c.lock"));
    assert!(!names.contains(&"node_modules"));

    // sub should contain keep.md and deep (with d.rs)
    let sub = tree.children.iter().find(|n| n.name == "sub").unwrap();
    let sub_names: Vec<_> = sub.children.iter().map(|n| n.name.as_str()).collect();
    assert!(sub_names.contains(&"keep.md"));
    assert!(sub_names.contains(&"deep"));
}

#[test]
fn test_scan_dir_with_include_exts_hides_empty_dirs() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    mkfile(&root.join("sub1/one.txt"));
    mkfile(&root.join("sub2/two.md"));

    // Only include .rs files -> both sub1 and sub2 should be hidden (no .rs underneath)
    let include: HashSet<String> = std::iter::once(String::from(".rs")).collect();
    let exclude: HashSet<String> = HashSet::new();
    let nodirs: HashSet<String> = HashSet::new();
    let nofiles: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude, &nodirs, &nofiles);
    // children should be empty because no matching files
    assert!(
        tree.children.is_empty(),
        "Expected no visible children when include set filters everything out"
    );
}

#[test]
fn test_gather_paths_set() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    mkfile(&root.join("a.txt"));
    mkfile(&root.join("sub/b.rs"));

    let inc: HashSet<String> = HashSet::new();
    let exc: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &inc, &exc, &ex_dirs, &ex_files);
    let set = gather_paths_set(&tree);
    // should include root + all children paths
    assert!(set.contains(&PathBuf::from(root)));
    assert_eq!(
        set.len(),
        1 + tree.children.len()
            + tree
                .children
                .iter()
                .map(|n| n.children.len())
                .sum::<usize>()
    );
}

#[test]
fn test_collect_selected_paths_inheritance() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    mkfile(&root.join("a.txt"));
    mkfile(&root.join("b.rs"));
    mkfile(&root.join("sub1/c.txt"));
    mkfile(&root.join("sub1/deep/d.rs"));

    let include: HashSet<String> = HashSet::new();
    let exclude: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude, &ex_dirs, &ex_files);

    // Explicitly select the dir sub1 and the file b.rs
    let mut explicit: HashMap<PathBuf, bool> = HashMap::new();
    let sub1_path = root.join("sub1");
    let b_rs_path = root.join("b.rs");
    explicit.insert(sub1_path, true);
    explicit.insert(b_rs_path, true);

    let mut files = Vec::new();
    let mut dirs = Vec::new();
    collect_selected_paths(&tree, &explicit, None, &mut files, &mut dirs);

    let files_unix: Vec<_> = files.iter().map(|p| path_to_unix(p)).collect();
    assert!(
        files_unix.iter().any(|s| s.ends_with("b.rs")),
        "b.rs should be selected explicitly"
    );
    assert!(
        files_unix.iter().any(|s| s.ends_with("sub1/c.txt")),
        "c.txt should be selected via inheritance from dir"
    );
    assert!(
        files_unix.iter().any(|s| s.ends_with("sub1/deep/d.rs")),
        "d.rs should be selected via inheritance from dir"
    );

    let dirs_unix: Vec<_> = dirs.iter().map(|p| path_to_unix(p)).collect();
    assert!(
        dirs_unix.iter().any(|s| s.ends_with("sub1")),
        "selected directory should appear in dirs_out"
    );
}

#[test]
fn test_dir_contains_file_logic() {
    // Build a small artificial tree
    let leaf = Node {
        name: "x.txt".into(),
        path: PathBuf::from("x.txt"),
        is_dir: false,
        children: vec![],
        expanded: false,
        has_children: false,
    };
    let empty_dir = Node {
        name: "empty".into(),
        path: PathBuf::from("empty"),
        is_dir: true,
        children: vec![],
        expanded: true,
        has_children: false,
    };
    let nested_dir = Node {
        name: "nested".into(),
        path: PathBuf::from("nested"),
        is_dir: true,
        children: vec![leaf.clone()],
        expanded: true,
        has_children: true,
    };

    assert!(dir_contains_file(&leaf));
    assert!(!dir_contains_file(&empty_dir));
    assert!(dir_contains_file(&nested_dir));
}
