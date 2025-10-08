use stitch::core::{
    ensure_profiles_dirs, ensure_workspace_dir, load_profile, load_workspace, workspace_file,
};
use tempfile::TempDir;

#[test]
fn load_workspace_handles_corrupt_json() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let _ = ensure_workspace_dir(root).unwrap();
    let wf = workspace_file(root);
    std::fs::write(&wf, "{ not json ").unwrap();
    assert!(
        load_workspace(root).is_none(),
        "should not panic or succeed on corrupt JSON"
    );
}

#[test]
fn load_profile_handles_corrupt_json() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    ensure_profiles_dirs(root).unwrap();

    // Create a corrupt profile file
    let profile_path = root
        .join(".stitchworkspace")
        .join("profiles")
        .join("test.json");
    std::fs::write(&profile_path, "{ invalid json content ").unwrap();

    assert!(
        load_profile(root, "test").is_none(),
        "should not panic or succeed on corrupt profile JSON"
    );
}
