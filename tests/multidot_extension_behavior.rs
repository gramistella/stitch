use std::collections::HashSet;
use std::fs;
use stitch::core::*;
use tempfile::TempDir;

/// This test verifies that multi-dot extensions now work correctly:
/// - `parse_extension_filters` accepts ".tar.gz"
/// - `scan_dir_to_node` now matches multi-dot extensions like ".tar.gz"
#[test]
fn include_multidot_extension_now_works() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let file = root.join("archive.tar.gz");
    fs::write(&file, "x").unwrap();

    // User asks for ".tar.gz"
    let (inc, _exc) = parse_extension_filters(".tar.gz");
    assert!(inc.contains(".tar.gz"));

    let include: HashSet<String> = inc; // contains ".tar.gz"
    let exclude_exts: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &ex_dirs, &ex_files);

    // Now ".tar.gz" files should be included when filtering for ".tar.gz"
    let has_archive = tree.children.iter().any(|n| n.name == "archive.tar.gz");
    assert!(
        has_archive,
        "Multi-dot extension '.tar.gz' should now match files with that extension"
    );
}
