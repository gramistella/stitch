#[cfg(all(unix, target_os = "linux"))]
mod non_utf8 {
    use std::collections::HashSet;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::ffi::OsStringExt;
    use stitch::core::*;
    use tempfile::TempDir;

    #[test]
    fn scan_dir_tolerates_non_utf8_filenames() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // "fo\x80" (invalid UTF-8)
        let os = OsString::from_vec(vec![b'f', b'o', 0x80]);
        let path = root.join(&os);
        fs::write(&path, "x").unwrap();

        let include: HashSet<String> = HashSet::new();
        let exclude: HashSet<String> = HashSet::new();
        let ex_dirs: HashSet<String> = HashSet::new();
        let ex_files: HashSet<String> = HashSet::new();

        let tree = scan_dir_to_node(root, &include, &exclude, &ex_dirs, &ex_files);

        // We can't reliably match the lossy name text, so assert we see exactly one file child.
        assert_eq!(tree.children.len(), 1);
        assert!(!tree.children[0].is_dir);
    }
}
