use stitch::core::{
    Profile, ProfileScope, RustOptions, SlintOptions, WorkspaceSettings, load_profile, save_profile,
};
use tempfile::TempDir;

const fn ws() -> WorkspaceSettings {
    WorkspaceSettings {
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
    }
}

#[test]
fn load_profile_prefers_local_over_shared() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let mut shared = Profile {
        name: "same".into(),
        settings: ws(),
        explicit: vec![],
    };
    shared.settings.ext_filter = ".rs".into();
    save_profile(root, &shared, ProfileScope::Shared).unwrap();

    let mut local = Profile {
        name: "same".into(),
        settings: ws(),
        explicit: vec![],
    };
    local.settings.ext_filter = ".md".into();
    save_profile(root, &local, ProfileScope::Local).unwrap();

    let (loaded, scope) = load_profile(root, "same").expect("load");
    assert_eq!(scope, ProfileScope::Local);
    assert_eq!(loaded.settings.ext_filter, ".md");
}
