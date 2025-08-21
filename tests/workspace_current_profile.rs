use stitch::core::{
    Profile, ProfileScope, WorkspaceSettings, clear_stale_current_profile, load_workspace,
    save_profile, save_workspace, workspace_dir, workspace_file,
};
use tempfile::TempDir;

#[test]
fn clears_stale_current_profile_when_profile_missing() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Seed a workspace that points to a non-existent profile.
    let ws = WorkspaceSettings {
        version: 1,
        ext_filter: "".into(),
        exclude_dirs: "".into(),
        exclude_files: "".into(),
        remove_prefix: "".into(),
        remove_regex: "".into(),
        hierarchy_only: false,
        dirs_only: false,
        current_profile: Some("ghost".into()),
    };
    save_workspace(root, &ws).expect("save workspace");

    // Sanity: workspace exists and points to ghost
    assert!(workspace_dir(root).exists());
    assert!(workspace_file(root).exists());
    assert_eq!(
        load_workspace(root).unwrap().current_profile.as_deref(),
        Some("ghost")
    );

    // Clear stale ref
    let cleared = clear_stale_current_profile(root).expect("clear stale");
    assert!(cleared, "should report that it cleared a stale profile");

    // Verify it was removed
    let ws2 = load_workspace(root).expect("re-load workspace");
    assert_eq!(ws2.current_profile, None);

    // Idempotency: running again should be a no-op
    let cleared_again = clear_stale_current_profile(root).expect("second clear");
    assert!(!cleared_again, "second run should do nothing");
}

#[test]
fn no_clear_when_shared_profile_exists() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create a shared profile "alpha"
    let prof = Profile {
        name: "alpha".into(),
        settings: WorkspaceSettings {
            version: 1,
            ext_filter: ".rs".into(),
            exclude_dirs: "".into(),
            exclude_files: "".into(),
            remove_prefix: "".into(),
            remove_regex: "".into(),
            hierarchy_only: false,
            dirs_only: false,
            current_profile: Some("alpha".into()),
        },
        explicit: vec![],
    };
    save_profile(root, &prof, ProfileScope::Shared).expect("save shared profile");

    // Workspace points to existing shared profile
    let ws = WorkspaceSettings {
        version: 1,
        ext_filter: "".into(),
        exclude_dirs: "".into(),
        exclude_files: "".into(),
        remove_prefix: "".into(),
        remove_regex: "".into(),
        hierarchy_only: false,
        dirs_only: false,
        current_profile: Some("alpha".into()),
    };
    save_workspace(root, &ws).expect("save workspace");

    let cleared = clear_stale_current_profile(root).expect("clear call");
    assert!(!cleared, "profile exists; should not clear");

    let ws2 = load_workspace(root).expect("load");
    assert_eq!(ws2.current_profile.as_deref(), Some("alpha"));
}

#[test]
fn no_clear_when_local_profile_exists() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create a local/private profile "beta"
    let prof = Profile {
        name: "beta".into(),
        settings: WorkspaceSettings {
            version: 1,
            ext_filter: "".into(),
            exclude_dirs: "".into(),
            exclude_files: "".into(),
            remove_prefix: "".into(),
            remove_regex: "".into(),
            hierarchy_only: false,
            dirs_only: false,
            current_profile: Some("beta".into()),
        },
        explicit: vec![],
    };
    save_profile(root, &prof, ProfileScope::Local).expect("save local profile");

    // Workspace points to existing local profile
    let ws = WorkspaceSettings {
        version: 1,
        ext_filter: "".into(),
        exclude_dirs: "".into(),
        exclude_files: "".into(),
        remove_prefix: "".into(),
        remove_regex: "".into(),
        hierarchy_only: false,
        dirs_only: false,
        current_profile: Some("beta".into()),
    };
    save_workspace(root, &ws).expect("save workspace");

    let cleared = clear_stale_current_profile(root).expect("clear call");
    assert!(!cleared, "local profile exists; should not clear");

    let ws2 = load_workspace(root).expect("load");
    assert_eq!(ws2.current_profile.as_deref(), Some("beta"));
}

#[test]
fn noop_when_workspace_missing() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // No workspace.json present
    let res = clear_stale_current_profile(root).expect("call should succeed");
    assert!(!res, "nothing to clear when workspace.json is absent");
}
