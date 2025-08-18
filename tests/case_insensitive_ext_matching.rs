use std::collections::HashSet;
use std::fs;
use stitch::core::*;
use tempfile::TempDir;

#[test]
fn uppercase_extension_matches_lowercase_include() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(root.join("A.TXT"), "x").unwrap();
    fs::write(root.join("B.RS"), "x").unwrap();

    // Lowercase filters must match uppercase files on disk.
    let include: HashSet<String> = [".txt".into(), ".rs".into()].into_iter().collect();
    let exclude: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude, &ex_dirs, &ex_files);
    let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();

    assert!(names.contains(&"A.TXT"));
    assert!(names.contains(&"B.RS"));
}
