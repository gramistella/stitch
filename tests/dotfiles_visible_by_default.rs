use std::collections::HashSet;
use std::fs;
use stitch::core::scan_dir_to_node;
use tempfile::TempDir;

#[test]
fn dotfiles_are_included_by_default() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(root.join(".env"), "x").unwrap();
    fs::write(root.join("normal.txt"), "x").unwrap();

    let include = HashSet::new();
    let exclude_exts = HashSet::new();
    let exclude_dirs = HashSet::new();
    let exclude_files = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);
    let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();

    assert!(names.contains(&".env"));
    assert!(names.contains(&"normal.txt"));
}
