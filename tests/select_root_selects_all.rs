use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use stitch::core::*;
use tempfile::TempDir;

#[test]
fn selecting_root_selects_everything_and_emits_root_dir() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(root.join("a.txt"), "x").unwrap();
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("sub/b.rs"), "x").unwrap();

    let include: HashSet<String> = HashSet::new();
    let exclude: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude, &ex_dirs, &ex_files);

    // Explicitly select the root path
    let mut explicit: HashMap<PathBuf, bool> = HashMap::new();
    explicit.insert(root.to_path_buf(), true);

    let mut files = Vec::new();
    let mut dirs = Vec::new();
    collect_selected_paths(&tree, &explicit, None, &mut files, &mut dirs);

    // Both files should be present
    let files_unix: Vec<_> = files.iter().map(|p| path_to_unix(p)).collect();
    assert!(files_unix.iter().any(|s| s.ends_with("a.txt")));
    assert!(files_unix.iter().any(|s| s.ends_with("sub/b.rs")));

    // Root dir (non-empty) should be emitted in dirs_out
    assert!(dirs.iter().any(|p| p == root));
}
