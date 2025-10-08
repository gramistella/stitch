use std::collections::HashSet;
use std::fs;
use stitch::core::scan_dir_to_node;
use tempfile::TempDir;

fn fs_case_insensitive(root: &std::path::Path) -> bool {
    let probe = root.join("CiProbe");
    let _ = std::fs::create_dir(&probe);
    let exists = root.join("ciprobe").exists();
    let _ = std::fs::remove_dir_all(&probe);
    exists
}

#[test]
fn exclude_file_basename_case_semantics_are_clear() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("ReadMe"), "x").unwrap();

    let include = HashSet::new();
    let exclude_exts = HashSet::new();
    let exclude_dirs = HashSet::new();
    let mut exclude_files = HashSet::new();
    exclude_files.insert("README".to_string());

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);
    let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();

    // The current implementation does exact string matching for exclude_files
    // The test verifies that the behavior is consistent and documented
    if fs_case_insensitive(root) {
        // On case-insensitive filesystems, the exact string match "README" != "ReadMe"
        // so the file should remain (current implementation is case-sensitive)
        assert!(
            names.contains(&"ReadMe"),
            "on CI filesystems, exact string match is case-sensitive"
        );
    } else {
        // On case-sensitive filesystems, "ReadMe" != "README" so file should remain
        assert!(
            names.contains(&"ReadMe"),
            "on CS filesystems, case mismatch should not exclude file"
        );
    }
}
