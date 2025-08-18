#[cfg(unix)]
mod unix_symlink {
    use stitch::core::*;
    use tempfile::TempDir;

    #[test]
    fn is_ancestor_of_handles_symlinks_both_directions() {
        use std::fs;
        use std::os::unix::fs::symlink;

        let tmp = TempDir::new().unwrap();
        let real = tmp.path().join("real");
        let link = tmp.path().join("link");

        fs::create_dir_all(&real).unwrap();
        symlink(&real, &link).unwrap();

        // real/child exists; link/child refers to the same path via the symlink.
        let real_child = real.join("child");
        fs::create_dir_all(&real_child).unwrap();

        // link should be ancestor of real/child through normalization
        assert!(is_ancestor_of(&link, &real_child));

        // Create a file via the symlinked path; should still resolve to the real dir.
        let file_via_link = link.join("child/through_link.txt");
        fs::write(&file_via_link, "x").unwrap();

        // real should also be ancestor of the path accessed via the symlink.
        assert!(is_ancestor_of(&real, &file_via_link));
        // sanity: the file really landed in real/child
        assert!(real_child.join("through_link.txt").exists());
    }
}
