use std::collections::HashSet;
use std::fs;
use stitch::core::*;
use tempfile::TempDir;

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
    let exclude_files: HashSet<String> = ["README".into()].into_iter().collect();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);

    // Gather file names found anywhere in the tree
    fn gather_files(n: &Node, out: &mut Vec<String>) {
        if n.is_dir {
            for c in &n.children {
                gather_files(c, out);
            }
        } else {
            out.push(n.name.clone());
        }
    }
    let mut files = Vec::new();
    gather_files(&tree, &mut files);

    assert!(!files.iter().any(|n| n == "README"));
    assert!(files.iter().any(|n| n == "keep.txt"));
}
