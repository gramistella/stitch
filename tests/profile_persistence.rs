use stitch::core::{
    Profile, ProfileScope, ProfileSelection, RustOptions, WorkspaceSettings, delete_profile,
    ensure_profiles_dirs, ensure_workspace_dir, list_profiles, load_profile, save_profile,
};
use tempfile::TempDir;

#[test]
fn ensure_profiles_dirs_creates_both_locations() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let ws_dir = stitch::core::workspace_dir(root);
    assert!(!ws_dir.exists());
    ensure_workspace_dir(root).unwrap();
    ensure_profiles_dirs(root).unwrap();

    let shared = ws_dir.join("profiles");
    let local = ws_dir.join("local").join("profiles");
    assert!(shared.exists(), "shared profiles dir should exist");
    assert!(local.exists(), "local profiles dir should exist");
}

fn sample_ws() -> WorkspaceSettings {
    WorkspaceSettings {
        version: 1,
        ext_filter: ".rs".into(),
        exclude_dirs: "target".into(),
        exclude_files: "LICENSE".into(),
        remove_prefix: "#,//".into(),
        remove_regex: "TODO:.*$".into(),
        hierarchy_only: false,
        dirs_only: false,
        rust: RustOptions {
            rust_remove_inline_comments: false,
            rust_remove_doc_comments: false,
            rust_function_signatures_only: false,
            rust_signatures_only_filter: String::new(),
        },
    }
}

#[test]
fn save_then_load_profile_roundtrip_shared_scope() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let prof = Profile {
        name: "alpha".into(),
        settings: sample_ws(),
        explicit: vec![ProfileSelection {
            path: "src/lib.rs".into(),
            state: true,
        }],
    };

    save_profile(root, &prof, ProfileScope::Shared).expect("save profile");
    let (loaded, scope) = load_profile(root, "alpha").expect("load profile");

    assert_eq!(scope, ProfileScope::Shared);
    assert_eq!(loaded.name, "alpha");
    assert_eq!(loaded.settings.ext_filter, ".rs");
    assert_eq!(loaded.explicit.len(), 1);
    assert_eq!(loaded.explicit[0].path, "src/lib.rs");
    assert!(loaded.explicit[0].state);
}

#[test]
fn list_profiles_local_overrides_shared_same_name() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let p_shared = Profile {
        name: "same".into(),
        settings: sample_ws(),
        explicit: vec![],
    };
    let p_local = Profile {
        name: "same".into(),
        settings: sample_ws(),
        explicit: vec![],
    };
    let p_beta = Profile {
        name: "beta".into(),
        settings: sample_ws(),
        explicit: vec![],
    };

    save_profile(root, &p_shared, ProfileScope::Shared).unwrap();
    save_profile(root, &p_local, ProfileScope::Local).unwrap();
    save_profile(root, &p_beta, ProfileScope::Shared).unwrap();

    let metas = list_profiles(root);

    // We expect just two names: "beta" (shared) and "same" (local overrides shared)
    assert_eq!(metas.len(), 2);

    let beta = metas
        .iter()
        .find(|m| m.name == "beta")
        .expect("beta present");
    assert_eq!(beta.scope, ProfileScope::Shared);

    let same = metas
        .iter()
        .find(|m| m.name == "same")
        .expect("same present");
    assert_eq!(same.scope, ProfileScope::Local);
}

#[test]
fn delete_profile_removes_entry() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let p = Profile {
        name: "to_delete".into(),
        settings: sample_ws(),
        explicit: vec![],
    };
    save_profile(root, &p, ProfileScope::Shared).unwrap();

    assert!(list_profiles(root).iter().any(|m| m.name == "to_delete"));
    delete_profile(root, ProfileScope::Shared, "to_delete").unwrap();
    assert!(!list_profiles(root).iter().any(|m| m.name == "to_delete"));
}
