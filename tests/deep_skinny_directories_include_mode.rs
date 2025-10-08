use std::collections::HashSet;
use std::fs;
use stitch::core::scan_dir_to_node;
use tempfile::TempDir;

fn collect_paths(node: &stitch::core::Node, prefix: &str, paths: &mut Vec<String>) {
    let current_path = if prefix.is_empty() {
        node.name.clone()
    } else {
        format!("{}/{}", prefix, node.name)
    };
    if !node.is_dir {
        paths.push(current_path.clone());
    }
    for child in &node.children {
        collect_paths(child, &current_path, paths);
    }
}


#[test]
fn include_mode_hides_intermediate_dirs_with_no_matching_descendants() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create deep structure: a/b/c/d/e/f.rs
    let deep_path = root.join("a").join("b").join("c").join("d").join("e");
    fs::create_dir_all(&deep_path).unwrap();
    fs::write(deep_path.join("f.rs"), "rust code").unwrap();

    // Create another deep path with non-matching file: a/b/c/d/g.txt
    let other_path = root.join("a").join("b").join("c").join("d");
    fs::write(other_path.join("g.txt"), "text file").unwrap();

    // Create a shallow matching file: a/h.rs
    fs::write(root.join("a").join("h.rs"), "another rust file").unwrap();

    let include = std::iter::once(String::from(".rs")).collect();
    let exclude_exts = HashSet::new();
    let exclude_dirs = HashSet::new();
    let exclude_files = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);

    // Helper to collect all paths in the tree
    let mut found_paths = Vec::new();
    collect_paths(&tree, "", &mut found_paths);

    // Should find both .rs files (paths include temp dir prefix)
    assert!(found_paths.iter().any(|p| p.ends_with("a/b/c/d/e/f.rs")));
    assert!(found_paths.iter().any(|p| p.ends_with("a/h.rs")));

    // Should not find the .txt file
    assert!(!found_paths.iter().any(|p| p.ends_with("a/b/c/d/g.txt")));
}

#[test]
fn include_mode_shows_intermediate_dirs_only_when_they_have_matching_descendants() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create structure where intermediate dirs should be visible
    // a/b/c/d/e/f.rs (all intermediate dirs should be visible)
    let deep_path = root.join("a").join("b").join("c").join("d").join("e");
    fs::create_dir_all(&deep_path).unwrap();
    fs::write(deep_path.join("f.rs"), "rust code").unwrap();

    // Create a branch that doesn't lead to matching files: a/b/x/y/z.txt
    let non_matching_path = root.join("a").join("b").join("x").join("y");
    fs::create_dir_all(&non_matching_path).unwrap();
    fs::write(non_matching_path.join("z.txt"), "text file").unwrap();

    let include = std::iter::once(String::from(".rs")).collect();
    let exclude_exts = HashSet::new();
    let exclude_dirs = HashSet::new();
    let exclude_files = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);

    // Helper to check if a path exists in the tree

    // The path to the .rs file should exist (including all intermediate dirs)
    assert!(path_exists_in_tree(&tree, "a/b/c/d/e/f.rs"));

    // The non-matching branch should not exist in the tree
    assert!(!path_exists_in_tree(&tree, "a/b/x"));
    assert!(!path_exists_in_tree(&tree, "a/b/x/y"));
    assert!(!path_exists_in_tree(&tree, "a/b/x/y/z.txt"));
}

#[test]
fn include_mode_hides_intermediate_dirs_when_only_descendant_is_excluded() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create deep structure with a file that matches include but is excluded by basename
    let deep_path = root.join("a").join("b").join("c").join("d").join("e");
    fs::create_dir_all(&deep_path).unwrap();
    fs::write(deep_path.join("temp.rs"), "temporary rust file").unwrap();

    // Create another file in the same deep path that should be included
    fs::write(deep_path.join("main.rs"), "main rust file").unwrap();

    let include = std::iter::once(String::from(".rs")).collect();
    let exclude_exts = HashSet::new();
    let exclude_dirs = HashSet::new();
    let exclude_files = std::iter::once(String::from("temp.rs")).collect();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);

    // Helper to collect all paths in the tree
    let mut found_paths = Vec::new();
    collect_paths(&tree, "", &mut found_paths);

    // Should find main.rs but not temp.rs
    assert!(found_paths.iter().any(|p| p.ends_with("a/b/c/d/e/main.rs")));
    assert!(!found_paths.iter().any(|p| p.ends_with("a/b/c/d/e/temp.rs")));

    // The intermediate directories should still be visible because main.rs is included
    assert!(path_exists_in_tree(&tree, "a/b/c/d/e"));
}

fn path_exists_in_tree(node: &stitch::core::Node, target_path: &str) -> bool {
    if target_path.is_empty() {
        return true;
    }

    let parts: Vec<&str> = target_path.split('/').collect();
    let mut current = node;

    for (i, part) in parts.iter().enumerate() {
        if let Some(child) = current.children.iter().find(|c| c.name == *part) {
            current = child;
            if i == parts.len() - 1 {
                return true;
            }
        } else {
            return false;
        }
    }
    false
}

#[test]
fn include_mode_with_mixed_exclusions() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create a complex structure with mixed include/exclude patterns
    let dir1 = root.join("src").join("module1");
    let dir2 = root.join("src").join("module2");
    let dir3 = root.join("tests");

    fs::create_dir_all(&dir1).unwrap();
    fs::create_dir_all(&dir2).unwrap();
    fs::create_dir_all(&dir3).unwrap();

    // Files that should be included
    fs::write(dir1.join("lib.rs"), "module1 lib").unwrap();
    fs::write(dir2.join("mod.rs"), "module2 mod").unwrap();
    fs::write(dir3.join("test.rs"), "test file").unwrap();

    // Files that should be excluded
    fs::write(dir1.join("temp.rs"), "temp file").unwrap();
    fs::write(dir2.join("backup.rs"), "backup file").unwrap();
    fs::write(dir3.join("old_test.rs"), "old test").unwrap();

    // Non-matching files
    fs::write(dir1.join("readme.txt"), "readme").unwrap();
    fs::write(dir2.join("config.json"), "config").unwrap();

    let include = std::iter::once(String::from(".rs")).collect();
    let exclude_exts = HashSet::new();
    let exclude_dirs = HashSet::new();
    let exclude_files = std::iter::once(String::from("temp.rs")).collect();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);

    // Helper to collect all paths in the tree
    let mut found_paths = Vec::new();
    collect_paths(&tree, "", &mut found_paths);

    // Should find the included .rs files
    assert!(
        found_paths
            .iter()
            .any(|p| p.ends_with("src/module1/lib.rs"))
    );
    assert!(
        found_paths
            .iter()
            .any(|p| p.ends_with("src/module2/mod.rs"))
    );
    assert!(found_paths.iter().any(|p| p.ends_with("tests/test.rs")));

    // Should not find excluded files
    assert!(
        !found_paths
            .iter()
            .any(|p| p.ends_with("src/module1/temp.rs"))
    );

    // Should not find non-matching files
    assert!(
        !found_paths
            .iter()
            .any(|p| p.ends_with("src/module1/readme.txt"))
    );
    assert!(
        !found_paths
            .iter()
            .any(|p| p.ends_with("src/module2/config.json"))
    );
}
