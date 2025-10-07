use std::collections::HashSet;
use std::fs;
use stitch::core::*;
use tempfile::TempDir;

/// This test documents current behavior:
/// - `parse_extension_filters` will happily accept ".tar.gz"
/// - `scan_dir_to_node` uses only the LAST extension (via `Path::extension`), so ".tar.gz" files
///   are considered ".gz" for matching.
#[test]
fn include_multidot_extension_documents_last_segment_behavior() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let file = root.join("archive.tar.gz");
    fs::write(&file, "x").unwrap();

    // User asks for ".tar.gz"
    let (inc, _exc) = parse_extension_filters(".tar.gz");
    assert!(inc.contains(".tar.gz"));

    // But the scanner compares only the last extension ("gz").
    let include: HashSet<String> = inc; // contains ".tar.gz"
    let exclude_exts: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &ex_dirs, &ex_files);

    // Under current behavior, "archive.tar.gz" will NOT be included because ".tar.gz" != ".gz".
    let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();
    assert!(
        !names.contains(&"archive.tar.gz"),
        "Currently, include set is matched against the last extension only (\".gz\"). \
Consider normalizing multi-dot semantics if you want a different behavior."
    );
}
