use std::collections::HashSet;
use std::fs;
use stitch::core::*;
use tempfile::TempDir;

fn gather_files(node: &Node, out: &mut Vec<String>) {
    if node.is_dir {
        for child in &node.children {
            gather_files(child, out);
        }
    } else {
        out.push(node.name.clone());
    }
}

#[test]
fn exclude_files_applies_by_basename_recursively() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::create_dir_all(root.join("a/b")).unwrap();
    fs::write(root.join("a/README"), "x").unwrap();
    fs::write(root.join("a/b/README"), "x").unwrap();
    fs::write(root.join("keep.txt"), "x").unwrap();

    let include: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = HashSet::new();
    let exclude_files: HashSet<String> = std::iter::once(String::from("README")).collect();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);

    let mut files = Vec::new();
    gather_files(&tree, &mut files);

    assert!(!files.iter().any(|n| n == "README"));
    assert!(files.iter().any(|n| n == "keep.txt"));
}
