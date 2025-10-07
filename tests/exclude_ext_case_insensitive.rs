use std::collections::HashSet;
use std::fs;
use stitch::core::scan_dir_to_node;
use tempfile::TempDir;

#[test]
fn exclude_extension_is_case_insensitive() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(root.join("A.TXT"), "x").unwrap();
    fs::write(root.join("B.txt"), "x").unwrap();
    fs::write(root.join("C.md"), "x").unwrap();

    let include = HashSet::new();
    let exclude_exts: HashSet<String> = std::iter::once(String::from(".txt")).collect();
    let exclude_dirs = HashSet::new();
    let exclude_files = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);
    let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();

    assert!(!names.contains(&"A.TXT"));
    assert!(!names.contains(&"B.txt"));
    assert!(names.contains(&"C.md"));
}
