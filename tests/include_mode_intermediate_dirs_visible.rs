use std::collections::HashSet;
use std::fs;
use stitch::core::*;
use tempfile::TempDir;

#[test]
fn include_mode_shows_intermediate_dirs_with_deeper_matches() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // a/b/c/file.rs
    fs::create_dir_all(root.join("a/b/c")).unwrap();
    fs::write(root.join("a/b/c/file.rs"), "x").unwrap();

    let include_exts: HashSet<String> = [".rs".into()].into_iter().collect();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = HashSet::new();
    let exclude_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(
        root,
        &include_exts,
        &exclude_exts,
        &exclude_dirs,
        &exclude_files,
    );

    let a = tree.children.iter().find(|n| n.name == "a").expect("a");
    let b = a.children.iter().find(|n| n.name == "b").expect("b");
    let c = b.children.iter().find(|n| n.name == "c").expect("c");
    let file = c
        .children
        .iter()
        .find(|n| n.name == "file.rs")
        .expect("file");

    assert!(a.is_dir && b.is_dir && c.is_dir);
    assert!(!file.is_dir);
}
