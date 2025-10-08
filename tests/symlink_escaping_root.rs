#[cfg(unix)]
mod unix_symlink_escaping {
    use std::collections::HashSet;
    use std::fs;
    use std::os::unix::fs::symlink;
    use stitch::core::scan_dir_to_node;
    use tempfile::TempDir;

    #[test]
    fn scan_does_not_follow_symlinks_outside_root() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create a directory outside the root
        let outside_dir = tmp.path().join("outside");
        fs::create_dir_all(&outside_dir).unwrap();
        fs::write(outside_dir.join("secret.txt"), "secret content").unwrap();
        fs::write(outside_dir.join("another.txt"), "another file").unwrap();

        // Create a symlink inside root pointing outside
        let symlink_path = root.join("link_to_outside");
        symlink(&outside_dir, &symlink_path).unwrap();

        // Create a normal file in root
        fs::write(root.join("normal.txt"), "normal content").unwrap();

        let include = HashSet::new();
        let exclude_exts = HashSet::new();
        let exclude_dirs = HashSet::new();
        let exclude_files = HashSet::new();

        let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);

        // Should include the symlink itself and normal file, but not the contents outside
        let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"link_to_outside"));
        assert!(names.contains(&"normal.txt"));

        // The symlink should be treated as a file, not followed
        let symlink_node = tree
            .children
            .iter()
            .find(|n| n.name == "link_to_outside")
            .unwrap();
        assert!(!symlink_node.is_dir);
        assert!(symlink_node.children.is_empty());
    }

    #[test]
    fn scan_handles_nested_symlink_escaping() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create structure: root/nested/link -> ../../outside
        let nested_dir = root.join("nested");
        fs::create_dir(&nested_dir).unwrap();

        let outside_dir = tmp.path().join("outside");
        fs::create_dir_all(&outside_dir).unwrap();
        fs::write(outside_dir.join("secret.txt"), "secret").unwrap();

        let symlink_path = nested_dir.join("escape_link");
        symlink("../../outside", &symlink_path).unwrap();

        // Create a normal file in nested
        fs::write(nested_dir.join("normal.txt"), "normal").unwrap();

        let include = HashSet::new();
        let exclude_exts = HashSet::new();
        let exclude_dirs = HashSet::new();
        let exclude_files = HashSet::new();

        let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);

        // Should find the nested directory with its contents, but not follow the escaping symlink
        let nested_node = tree.children.iter().find(|n| n.name == "nested").unwrap();
        assert!(nested_node.is_dir);

        let nested_names: Vec<_> = nested_node
            .children
            .iter()
            .map(|n| n.name.as_str())
            .collect();
        assert!(nested_names.contains(&"escape_link"));
        assert!(nested_names.contains(&"normal.txt"));

        // The escaping symlink should be treated as a file
        let escape_node = nested_node
            .children
            .iter()
            .find(|n| n.name == "escape_link")
            .unwrap();
        assert!(!escape_node.is_dir);
        assert!(escape_node.children.is_empty());
    }

    #[test]
    fn scan_handles_absolute_symlink_escaping() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create a directory outside with absolute path
        let outside_dir = tmp.path().join("absolute_outside");
        fs::create_dir_all(&outside_dir).unwrap();
        fs::write(outside_dir.join("absolute_secret.txt"), "absolute secret").unwrap();

        // Create symlink with absolute path
        let symlink_path = root.join("absolute_link");
        symlink(&outside_dir, &symlink_path).unwrap();

        let include = HashSet::new();
        let exclude_exts = HashSet::new();
        let exclude_dirs = HashSet::new();
        let exclude_files = HashSet::new();

        let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);

        // Should include the symlink but not follow it
        assert!(tree.children.iter().map(|n| n.name.as_str()).any(|x| x == "absolute_link"));

        let link_node = tree
            .children
            .iter()
            .find(|n| n.name == "absolute_link")
            .unwrap();
        assert!(!link_node.is_dir);
        assert!(link_node.children.is_empty());
    }
}
