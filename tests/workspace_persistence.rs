use stitch::core::{
    WorkspaceSettings, ensure_workspace_dir, load_workspace, save_workspace, workspace_dir,
    workspace_file,
};
use tempfile::TempDir;

#[test]
fn load_missing_workspace_returns_none_and_ensure_creates_dir() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Nothing there yet
    assert!(load_workspace(root).is_none());

    // ensure_workspace_dir should create the folder idempotently
    let wd1 = ensure_workspace_dir(root).expect("create .stitchworkspace");
    assert!(wd1.exists());
    assert_eq!(wd1, workspace_dir(root));

    let wd2 = ensure_workspace_dir(root).expect("idempotent create");
    assert_eq!(wd1, wd2);
}

#[test]
fn save_then_load_roundtrip_and_overwrite_is_atomic() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // First save
    let s1 = WorkspaceSettings {
        version: 1,
        ext_filter: ".rs,.toml".into(),
        exclude_dirs: "target,node_modules".into(),
        exclude_files: "LICENSE".into(),
        remove_prefix: "#,//".into(),
        remove_regex: "\"\"\"(?m)^\\s*TODO:.*$\"\"\"".into(),
        hierarchy_only: false,
        dirs_only: false,
        current_profile: None,
    };
    save_workspace(root, &s1).expect("save v1");

    // File exists and loads back
    let wf = workspace_file(root);
    assert!(wf.exists(), "workspace.json must exist after save");

    let loaded1 = load_workspace(root).expect("load s1");
    assert_eq!(loaded1.version, 1);
    assert_eq!(loaded1.ext_filter, s1.ext_filter);
    assert_eq!(loaded1.exclude_dirs, s1.exclude_dirs);
    assert_eq!(loaded1.exclude_files, s1.exclude_files);
    assert_eq!(loaded1.remove_prefix, s1.remove_prefix);
    assert_eq!(loaded1.remove_regex, s1.remove_regex);
    assert_eq!(loaded1.hierarchy_only, s1.hierarchy_only);
    assert_eq!(loaded1.dirs_only, s1.dirs_only);
    assert_eq!(loaded1.current_profile, None);

    // Overwrite with different settings
    let mut s2 = loaded1.clone();
    s2.ext_filter = ".rs".into();
    s2.hierarchy_only = true;
    save_workspace(root, &s2).expect("save v2 overwrite");

    let loaded2 = load_workspace(root).expect("load s2");
    assert_eq!(loaded2.ext_filter, ".rs");
    assert!(loaded2.hierarchy_only);

    // The temporary file used for atomic write should not remain.
    let tmp_path = wf.with_extension("json.tmp");
    assert!(
        !tmp_path.exists(),
        "temporary write file should be cleaned up by rename()"
    );
}
