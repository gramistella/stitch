use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::{fs, path::Path};
use stitch::core::*;
use tempfile::TempDir;

fn mkfile(p: &Path) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, "x").unwrap();
}

#[test]
fn dir_contains_file_false_when_only_subdirs_and_no_files_anywhere() {
    // Build a purely-directory tree with no files.
    let leaf_dir = Node {
        name: "c".into(),
        path: PathBuf::from("c"),
        is_dir: true,
        children: vec![], // no files
        expanded: true,
        has_children: false,
    };
    let mid_dir = Node {
        name: "b".into(),
        path: PathBuf::from("b"),
        is_dir: true,
        children: vec![leaf_dir], // still no files under b
        expanded: true,
        has_children: false,
    };
    let top_dir = Node {
        name: "a".into(),
        path: PathBuf::from("a"),
        is_dir: true,
        children: vec![mid_dir], // still no files anywhere
        expanded: true,
        has_children: false,
    };

    assert!(!dir_contains_file(&top_dir));
}

#[test]
fn child_true_overrides_ancestor_false() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // root/
    //   off/
    //     keep.txt
    mkfile(&root.join("off/keep.txt"));

    let inc: HashSet<String> = HashSet::new();
    let exc: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &inc, &exc, &ex_dirs, &ex_files);

    // Ancestor explicitly false, but child explicitly true should still select child.
    let mut explicit: HashMap<PathBuf, bool> = HashMap::new();
    explicit.insert(root.join("off"), false);
    explicit.insert(root.join("off/keep.txt"), true);

    let mut files = Vec::new();
    let mut dirs = Vec::new();
    collect_selected_paths(&tree, &explicit, None, &mut files, &mut dirs);

    let files_unix: Vec<_> = files.iter().map(|p| path_to_unix(p)).collect();
    assert!(files_unix.iter().any(|s| s.ends_with("off/keep.txt")));
}
