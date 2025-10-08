#[cfg(target_os = "windows")]
#[test]
fn windows_junctions_behavior() {
    use std::os::windows::fs::symlink_dir;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create a directory structure
    let target_dir = root.join("target");
    std::fs::create_dir(&target_dir).unwrap();
    std::fs::write(target_dir.join("file.txt"), "content").unwrap();

    // Create a junction pointing to the target directory
    let junction = root.join("junction");
    symlink_dir(&target_dir, &junction).unwrap();

    // Test that scanning handles junctions correctly
    use std::collections::HashSet;
    use stitch::core::{is_ancestor_of, scan_dir_to_node};

    let h = HashSet::new();
    let tree = scan_dir_to_node(root, &h, &h, &h, &h);

    // The junction should be treated as a directory and its contents should be accessible
    let junction_node = tree.children.iter().find(|n| n.name == "junction");
    assert!(
        junction_node.is_some(),
        "junction should be found in scan results"
    );

    // Test is_ancestor_of with junctions
    assert!(
        is_ancestor_of(root, &junction),
        "junction should be considered under root"
    );
    assert!(
        is_ancestor_of(&junction, &target_dir.join("file.txt")),
        "files in junction target should be considered under junction"
    );
}

#[cfg(not(target_os = "windows"))]
#[test]
fn windows_junctions_behavior() {
    // Skip on non-Windows platforms
    println!("Skipping Windows junctions test on non-Windows platform");
}
