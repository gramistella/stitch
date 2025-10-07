use std::collections::HashSet;
use std::fs;
use stitch::core::*;
use tempfile::TempDir;

#[test]
fn exclude_file_by_basename_overrides_include_ext() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(root.join("show.rs"), "x").unwrap();
    fs::write(root.join("hide.rs"), "x").unwrap();

    let include_exts: HashSet<String> = std::iter::once(String::from(".rs")).collect();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = HashSet::new();
    let exclude_files: HashSet<String> = std::iter::once(String::from("hide.rs")).collect();

    let tree = scan_dir_to_node(
        root,
        &include_exts,
        &exclude_exts,
        &exclude_dirs,
        &exclude_files,
    );
    let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();

    assert!(names.contains(&"show.rs"));
    assert!(!names.contains(&"hide.rs"));
}
