use std::collections::HashSet;
use std::fs;
use stitch::core::scan_dir_to_node;
use tempfile::TempDir;

#[test]
fn exclude_multidot_now_works_correctly() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("archive.tar.gz"), "x").unwrap();

    let include: HashSet<String> = HashSet::new();
    let exclude_tar_gz: HashSet<String> = std::iter::once(String::from(".tar.gz")).collect();
    let exclude_gz: HashSet<String> = std::iter::once(String::from(".gz")).collect();
    let nodirs = HashSet::new();
    let nofiles = HashSet::new();

    // Excluding ".tar.gz" should now hide the file.
    let tree1 = scan_dir_to_node(root, &include, &exclude_tar_gz, &nodirs, &nofiles);
    assert!(
        !tree1.children.iter().any(|n| n.name == "archive.tar.gz"),
        "Excluding '.tar.gz' should hide files with that extension"
    );

    // Excluding ".gz" should also hide it (backward compatibility).
    let tree2 = scan_dir_to_node(root, &include, &exclude_gz, &nodirs, &nofiles);
    assert!(
        !tree2.children.iter().any(|n| n.name == "archive.tar.gz"),
        "Excluding '.gz' should still hide .tar.gz files"
    );
}
