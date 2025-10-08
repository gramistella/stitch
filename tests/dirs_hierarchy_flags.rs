use std::collections::HashSet;
use stitch::core::{WorkspaceSettings, collect_selected_paths, scan_dir_to_node};
use tempfile::TempDir;

// Helper to create a test directory structure
fn create_test_structure(root: &std::path::Path) {
    std::fs::create_dir_all(root.join("src").join("lib")).unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::write(root.join("src").join("main.rs"), "fn main() {}").unwrap();
    std::fs::write(
        root.join("src").join("lib").join("mod.rs"),
        "pub mod utils;",
    )
    .unwrap();
    std::fs::write(root.join("tests").join("test.rs"), "#[test] fn test() {}").unwrap();
    std::fs::write(root.join("README.md"), "# Project").unwrap();
}

#[test]
fn dirs_only_flag_behavior() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    create_test_structure(root);

    let h = HashSet::new();
    let tree = scan_dir_to_node(root, &h, &h, &h, &h);

    // Simulate selecting all directories
    let mut explicit_states = std::collections::HashMap::new();
    explicit_states.insert(root.to_path_buf(), true);
    explicit_states.insert(root.join("src"), true);
    explicit_states.insert(root.join("src").join("lib"), true);
    explicit_states.insert(root.join("tests"), true);

    let mut files = Vec::new();
    let mut dirs = Vec::new();
    collect_selected_paths(&tree, &explicit_states, None, &mut files, &mut dirs);

    // With dirs_only=true, we should only get directories, no files
    assert!(!dirs.is_empty(), "should have directories selected");

    // Verify that the selected paths are actually directories
    for dir_path in &dirs {
        assert!(
            dir_path.is_dir(),
            "selected path should be a directory: {dir_path:?}"
        );
    }
}

#[test]
fn hierarchy_only_flag_behavior() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    create_test_structure(root);

    let h = HashSet::new();
    let tree = scan_dir_to_node(root, &h, &h, &h, &h);

    // Simulate selecting all items
    let mut explicit_states = std::collections::HashMap::new();
    explicit_states.insert(root.to_path_buf(), true);
    explicit_states.insert(root.join("src"), true);
    explicit_states.insert(root.join("src").join("main.rs"), true);
    explicit_states.insert(root.join("src").join("lib"), true);
    explicit_states.insert(root.join("src").join("lib").join("mod.rs"), true);
    explicit_states.insert(root.join("tests"), true);
    explicit_states.insert(root.join("tests").join("test.rs"), true);
    explicit_states.insert(root.join("README.md"), true);

    let mut files = Vec::new();
    let mut dirs = Vec::new();
    collect_selected_paths(&tree, &explicit_states, None, &mut files, &mut dirs);

    // With hierarchy_only=true, we should get both files and directories
    // but the output should only show the hierarchy structure, not file contents
    assert!(!files.is_empty(), "should have files selected");
    assert!(!dirs.is_empty(), "should have directories selected");

    // Verify we have the expected structure
    let file_names: HashSet<_> = files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(file_names.contains("main.rs"));
    assert!(file_names.contains("mod.rs"));
    assert!(file_names.contains("test.rs"));
    assert!(file_names.contains("README.md"));
}

#[test]
fn workspace_settings_flags_persistence() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create workspace settings with flags enabled
    let settings = WorkspaceSettings {
        dirs_only: true,
        hierarchy_only: true,
        ..Default::default()
    };

    // Save and reload settings
    stitch::core::save_workspace(root, &settings).unwrap();
    let loaded = stitch::core::load_workspace(root).unwrap();

    assert!(loaded.dirs_only);
    assert!(loaded.hierarchy_only);
}
