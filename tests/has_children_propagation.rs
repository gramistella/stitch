use std::collections::HashSet;
use std::fs;
use stitch::core::*;
use tempfile::TempDir;

#[test]
fn has_children_propagates_through_nested_dirs() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::create_dir_all(root.join("a/b/c")).unwrap();
    fs::write(root.join("a/b/c/leaf.txt"), "x").unwrap();

    let inc: HashSet<String> = HashSet::new();
    let exc: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &inc, &exc, &ex_dirs, &ex_files);

    let a = tree.children.iter().find(|n| n.name == "a").unwrap();
    let b = a.children.iter().find(|n| n.name == "b").unwrap();
    let c = b.children.iter().find(|n| n.name == "c").unwrap();

    assert!(c.has_children, "c has a file");
    assert!(b.has_children, "b should reflect descendant file");
    assert!(a.has_children, "a should reflect descendant file");
    assert!(dir_contains_file(a));
}
