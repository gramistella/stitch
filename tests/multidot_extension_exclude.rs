use std::collections::HashSet;
use std::fs;
use stitch::core::scan_dir_to_node;
use tempfile::TempDir;

#[test]
fn exclude_multidot_matches_last_segment_only() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("archive.tar.gz"), "x").unwrap();

    let include: HashSet<String> = HashSet::new();
    let exclude_tar_gz: HashSet<String> = std::iter::once(String::from(".tar.gz")).collect();
    let exclude_gz: HashSet<String> = std::iter::once(String::from(".gz")).collect();
    let nodirs = HashSet::new();
    let nofiles = HashSet::new();

    // Excluding ".tar.gz" should NOT hide it if only the last segment is considered.
    let tree1 = scan_dir_to_node(root, &include, &exclude_tar_gz, &nodirs, &nofiles);
    assert!(tree1.children.iter().any(|n| n.name == "archive.tar.gz"));

    // Excluding ".gz" should hide it.
    let tree2 = scan_dir_to_node(root, &include, &exclude_gz, &nodirs, &nofiles);
    assert!(!tree2.children.iter().any(|n| n.name == "archive.tar.gz"));
}
