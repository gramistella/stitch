use std::collections::HashSet;
use stitch::core::{gather_paths_set, scan_dir_to_node};
use tempfile::TempDir;

#[test]
fn scan_handles_deep_nesting() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let mut p = root.to_path_buf();
    for i in 0..64 {
        p.push(format!("d{i}"));
    }
    std::fs::create_dir_all(&p).unwrap();
    std::fs::write(p.join("leaf.rs"), "x").unwrap();

    let h = HashSet::new();
    let tree = scan_dir_to_node(root, &h, &h, &h, &h);
    let set = gather_paths_set(&tree);
    assert!(set.iter().any(|q| q.ends_with("leaf.rs")));
}
