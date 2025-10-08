#[cfg(unix)]
#[test]
fn scanner_does_not_follow_symlink_loops() {
    use std::collections::HashSet;
    use std::fs;
    use std::os::unix::fs::symlink;
    use stitch::core::scan_dir_to_node;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::create_dir_all(root.join("real")).unwrap();
    fs::write(root.join("real/file.txt"), "x").unwrap();
    // Create loop: real/loop -> real
    symlink(root.join("real"), root.join("real/loop")).unwrap();

    let h = HashSet::new();
    let tree = scan_dir_to_node(root, &h, &h, &h, &h);

    // Expect: "real" present, "file.txt" present, but not an endlessly nested "loop" chain.
    let real = tree.children.iter().find(|n| n.name == "real").unwrap();
    assert!(real.children.iter().any(|n| n.name == "file.txt"));
    // Either "loop" is omitted, or included once but not infinitely expanded.
    let loop_count = real.children.iter().filter(|n| n.name == "loop").count();
    assert!(
        loop_count <= 1,
        "symlink loop must not cause unbounded descent"
    );
}
