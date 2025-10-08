use stitch::core::{
    LocalSettings, Profile, ProfileScope, RustOptions, SlintOptions, WorkspaceSettings,
    clear_stale_current_profile, load_local_settings, save_local_settings, save_profile,
};
use tempfile::TempDir;

#[test]
fn clears_stale_current_profile_when_profile_missing() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let local_settings = LocalSettings {
        current_profile: Some("ghost".into()),
    };
    save_local_settings(root, &local_settings).expect("save local settings");

    assert_eq!(
        load_local_settings(root)
            .unwrap()
            .current_profile
            .as_deref(),
        Some("ghost")
    );

    let cleared = clear_stale_current_profile(root).expect("clear stale");
    assert!(cleared, "should report that it cleared a stale profile");

    let settings2 = load_local_settings(root).expect("re-load local settings");
    assert_eq!(settings2.current_profile, None);

    let cleared_again = clear_stale_current_profile(root).expect("second clear");
    assert!(!cleared_again, "second run should do nothing");
}

#[test]
fn no_clear_when_shared_profile_exists() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let prof = Profile {
        name: "alpha".into(),
        settings: WorkspaceSettings {
            version: 1,
            ext_filter: ".rs".into(),
            exclude_dirs: String::new(),
            exclude_files: String::new(),
            remove_prefix: String::new(),
            remove_regex: String::new(),
            hierarchy_only: false,
            dirs_only: false,
            rust: RustOptions {
                rust_remove_inline_comments: false,
                rust_remove_doc_comments: false,
                rust_function_signatures_only: false,
                rust_signatures_only_filter: String::new(),
            },
            slint: SlintOptions {
                slint_remove_line_comments: false,
                slint_remove_block_comments: false,
            },
        },
        explicit: vec![],
    };
    save_profile(root, &prof, ProfileScope::Shared).expect("save shared profile");

    let local_settings = LocalSettings {
        current_profile: Some("alpha".into()),
    };
    save_local_settings(root, &local_settings).expect("save local settings");

    let cleared = clear_stale_current_profile(root).expect("clear call");
    assert!(!cleared, "profile exists; should not clear");

    let settings2 = load_local_settings(root).expect("load");
    assert_eq!(settings2.current_profile.as_deref(), Some("alpha"));
}

#[test]
fn no_clear_when_local_profile_exists() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let prof = Profile {
        name: "beta".into(),
        settings: WorkspaceSettings {
            version: 1,
            ext_filter: String::new(),
            exclude_dirs: String::new(),
            exclude_files: String::new(),
            remove_prefix: String::new(),
            remove_regex: String::new(),
            hierarchy_only: false,
            dirs_only: false,
            rust: RustOptions {
                rust_remove_inline_comments: false,
                rust_remove_doc_comments: false,
                rust_function_signatures_only: false,
                rust_signatures_only_filter: String::new(),
            },
            slint: SlintOptions {
                slint_remove_line_comments: false,
                slint_remove_block_comments: false,
            },
        },
        explicit: vec![],
    };
    save_profile(root, &prof, ProfileScope::Local).expect("save local profile");

    let local_settings = LocalSettings {
        current_profile: Some("beta".into()),
    };
    save_local_settings(root, &local_settings).expect("save local settings");

    let cleared = clear_stale_current_profile(root).expect("clear call");
    assert!(!cleared, "local profile exists; should not clear");

    let settings2 = load_local_settings(root).expect("load");
    assert_eq!(settings2.current_profile.as_deref(), Some("beta"));
}

#[test]
fn noop_when_local_settings_are_missing() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let res = clear_stale_current_profile(root).expect("call should succeed");
    assert!(!res, "nothing to clear when local settings file is absent");
}
