use std::collections::HashSet;
use std::fs;

use stitch::core::{
    ensure_workspace_dir, is_event_path_relevant, scan_dir_to_node, workspace_file,
};
use tempfile::TempDir;

#[test]
fn scanner_ignores_stitchworkspace_when_excluded_by_name() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create a real project file and a workspace dir with files in it.
    fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();

    let ws_dir = ensure_workspace_dir(root).expect("create workspace dir");
    let ws_json = workspace_file(root);
    fs::write(&ws_json, r#"{ "version": 1 }"#).unwrap();
    fs::write(ws_dir.join("scratch.txt"), "should be ignored").unwrap();

    // Exclude `.stitchworkspace` by directory name.
    let include_exts: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = std::iter::once(String::from(".stitchworkspace")).collect();
    let exclude_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(
        root,
        &include_exts,
        &exclude_exts,
        &exclude_dirs,
        &exclude_files,
    );
    let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();

    assert!(
        names.contains(&"main.rs"),
        "regular project file should be present"
    );
    assert!(
        !names.contains(&".stitchworkspace"),
        "workspace folder should be excluded from the tree"
    );
}

#[test]
fn fs_event_relevance_ignores_workspace_dir_changes_when_excluded() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    // Prepare workspace dir and a change path inside it.
    let _ = ensure_workspace_dir(&root).unwrap();
    let p_in_ws = root.join(".stitchworkspace").join("workspace.json");

    // Filters: explicitly exclude `.stitchworkspace`
    let include_exts: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = std::iter::once(String::from(".stitchworkspace")).collect();
    let exclude_files: HashSet<String> = HashSet::new();

    // Changes inside the excluded workspace dir should be ignored
    assert!(
        !is_event_path_relevant(
            &root,
            &p_in_ws,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "workspace dir changes must be ignored when excluded"
    );

    // Root self-change is always relevant (forces a refresh)
    assert!(is_event_path_relevant(
        &root,
        &root,
        &include_exts,
        &exclude_exts,
        &exclude_dirs,
        &exclude_files
    ));

    // A normal file should still be relevant under default (no include/exclude ext rules)
    let p_file = root.join("lib.rs");
    fs::write(&p_file, "pub fn x() {}\n").unwrap();
    assert!(is_event_path_relevant(
        &root,
        &p_file,
        &include_exts,
        &exclude_exts,
        &exclude_dirs,
        &exclude_files
    ));
}
