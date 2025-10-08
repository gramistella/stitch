use std::collections::HashSet;
use stitch::core::{gather_paths_set, render_unicode_tree_from_paths, scan_dir_to_node};
use tempfile::TempDir;

#[test]
#[ignore = "Not for CI, but useful locally"]
fn stress_test_large_directory_structure() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create a large directory structure with ~1000 files
    let start_time = std::time::Instant::now();

    // Create nested directories
    for i in 0..10 {
        for j in 0..10 {
            for k in 0..10 {
                let dir = root
                    .join(format!("dir_{i}"))
                    .join(format!("subdir_{j}"))
                    .join(format!("nested_{k}"));
                std::fs::create_dir_all(&dir).unwrap();

                // Create files in each directory
                for file_num in 0..10 {
                    let file_path = dir.join(format!("file_{file_num}.rs"));
                    std::fs::write(
                        &file_path,
                        format!("// File {i}.{j}.{k}.{file_num}\nfn main() {{}}"),
                    )
                    .unwrap();
                }
            }
        }
    }

    let creation_time = start_time.elapsed();
    println!("Created 1000 files in {creation_time:?}");

    // Test scanning performance
    let scan_start = std::time::Instant::now();
    let h = HashSet::new();
    let tree = scan_dir_to_node(root, &h, &h, &h, &h);
    let scan_time = scan_start.elapsed();

    println!("Scanned 1000 files in {scan_time:?}");

    // Verify we found all files
    let paths = gather_paths_set(&tree);
    let file_count = paths.iter().filter(|p| p.is_file()).count();
    assert!(
        file_count >= 1000,
        "Should find at least 1000 files, found {file_count}"
    );

    // Test rendering performance
    let render_start = std::time::Instant::now();
    let path_strings: Vec<String> = paths
        .iter()
        .filter(|p| p.is_file())
        .map(|p| p.strip_prefix(root).unwrap().to_string_lossy().to_string())
        .collect();
    let rendered = render_unicode_tree_from_paths(&path_strings, Some("root"));
    let render_time = render_start.elapsed();

    println!("Rendered tree in {render_time:?}");

    // Verify rendering contains expected structure
    assert!(
        rendered.contains("dir_0"),
        "Rendered tree should contain directory structure"
    );
    assert!(
        rendered.contains("file_0.rs"),
        "Rendered tree should contain files"
    );

    // Performance assertions (adjust thresholds as needed)
    assert!(
        scan_time.as_millis() < 5000,
        "Scanning should complete within 5 seconds"
    );
    assert!(
        render_time.as_millis() < 2000,
        "Rendering should complete within 2 seconds"
    );
}

#[test]
#[ignore = "Not for CI, but useful locally"]
fn stress_test_include_mode_directory_elision() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create a structure with many empty directories and some with files
    for i in 0..50 {
        let dir = root.join(format!("empty_dir_{i}"));
        std::fs::create_dir_all(&dir).unwrap();

        // Create some directories with files
        if i % 10 == 0 {
            let file_dir = dir.join("has_files");
            std::fs::create_dir_all(&file_dir).unwrap();
            std::fs::write(file_dir.join("important.rs"), "fn main() {}").unwrap();
        }
    }

    // Test with include mode (only .rs files)
    let mut include_exts = HashSet::new();
    include_exts.insert(".rs".to_string());
    let exclude_exts = HashSet::new();
    let exclude_dirs = HashSet::new();
    let exclude_files = HashSet::new();

    let start_time = std::time::Instant::now();
    let tree = scan_dir_to_node(
        root,
        &include_exts,
        &exclude_exts,
        &exclude_dirs,
        &exclude_files,
    );
    let scan_time = start_time.elapsed();

    println!("Include mode scan completed in {scan_time:?}");

    // Verify that empty directories are elided
    let paths = gather_paths_set(&tree);
    let dir_count = paths.iter().filter(|p| p.is_dir()).count();
    let file_count = paths.iter().filter(|p| p.is_file()).count();

    // Should have fewer directories than created (empty ones elided)
    assert!(
        dir_count < 50,
        "Empty directories should be elided in include mode"
    );
    assert!(file_count > 0, "Should find some .rs files");

    // Performance assertion
    assert!(
        scan_time.as_millis() < 1000,
        "Include mode scan should be fast"
    );
}

#[test]
#[ignore = "Not for CI, but useful locally"]
fn stress_test_ordering_consistency() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create files with various names to test ordering
    let file_names = vec![
        "a.rs",
        "z.rs",
        "1.rs",
        "9.rs",
        "A.rs",
        "Z.rs",
        "file_1.rs",
        "file_10.rs",
        "file_2.rs",
        "file_20.rs",
        "test_a.rs",
        "test_z.rs",
        "test_1.rs",
        "test_9.rs",
    ];

    for name in &file_names {
        std::fs::write(root.join(name), format!("// {name}\nfn main() {{}}")).unwrap();
    }

    let h = HashSet::new();
    let tree = scan_dir_to_node(root, &h, &h, &h, &h);

    // Verify ordering is consistent
    let mut prev_name = String::new();
    for child in &tree.children {
        if child.is_dir {
            continue;
        }
        let current_name = &child.name;
        assert!(
            prev_name <= *current_name,
            "Files should be in sorted order: '{prev_name}' should come before '{current_name}'"
        );
        prev_name = current_name.clone();
    }

    // Verify we found all files
    let found_names: HashSet<_> = tree
        .children
        .iter()
        .filter(|c| !c.is_dir)
        .map(|c| c.name.clone())
        .collect();

    for name in &file_names {
        assert!(found_names.contains(*name), "Should find file: {name}");
    }
}
