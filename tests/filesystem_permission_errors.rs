#[cfg(unix)]
mod unix_permissions {
    use std::collections::HashSet;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
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
    fn scan_handles_permission_denied_directories() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create a directory with no permissions
        let restricted_dir = root.join("restricted");
        fs::create_dir(&restricted_dir).unwrap();

        // Set permissions to 0o000 (no read, write, execute)
        let mut perms = fs::metadata(&restricted_dir).unwrap().permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&restricted_dir, perms).unwrap();

        // Create a normal file in the root
        fs::write(root.join("normal.txt"), "content").unwrap();

        let include = HashSet::new();
        let exclude_exts = HashSet::new();
        let exclude_dirs = HashSet::new();
        let exclude_files = HashSet::new();

        // This should not panic or hang
        let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);

        // Should include the normal file but skip the restricted directory
        let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();
        println!("Found names: {names:?}");
        assert!(names.contains(&"normal.txt"));
        // Note: The restricted directory might still appear but be empty or inaccessible
        // The key is that scanning doesn't panic or hang

        // Clean up permissions for tempdir cleanup
        let mut perms = fs::metadata(&restricted_dir).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&restricted_dir, perms).unwrap();
    }

    #[test]
    fn scan_handles_partially_accessible_directory() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create a directory structure with mixed permissions
        let parent_dir = root.join("parent");
        let accessible_dir = parent_dir.join("accessible");
        let restricted_dir = parent_dir.join("restricted");

        fs::create_dir_all(&accessible_dir).unwrap();
        fs::create_dir(&restricted_dir).unwrap();

        // Set restricted directory to no permissions
        let mut perms = fs::metadata(&restricted_dir).unwrap().permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&restricted_dir, perms).unwrap();

        // Create files in accessible directory
        fs::write(accessible_dir.join("file.txt"), "content").unwrap();

        let include = HashSet::new();
        let exclude_exts = HashSet::new();
        let exclude_dirs = HashSet::new();
        let exclude_files = HashSet::new();

        let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);

        // Should find the accessible file but skip the restricted directory
        let mut found_paths = Vec::new();
        collect_paths(&tree, "", &mut found_paths);

        assert!(
            found_paths
                .iter()
                .any(|p| p.ends_with("parent/accessible/file.txt"))
        );
        // Note: The restricted directory might still appear but be empty or inaccessible
        // The key is that scanning doesn't panic or hang

        // Clean up permissions for tempdir cleanup
        let mut perms = fs::metadata(&restricted_dir).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&restricted_dir, perms).unwrap();
    }
}
