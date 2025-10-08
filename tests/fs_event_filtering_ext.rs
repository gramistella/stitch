use std::collections::HashSet;
use stitch::core::is_event_path_relevant;
use tempfile::TempDir;

#[test]
fn excluded_extension_is_not_relevant() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    let p = root.join("note.LOG");

    let include_exts: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = std::iter::once(String::from(".log")).collect();
    let exclude_dirs: HashSet<String> = HashSet::new();
    let exclude_files: HashSet<String> = HashSet::new();

    assert!(
        !is_event_path_relevant(
            &root,
            &p,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "excluded extension should not be relevant (case-insensitive)"
    );
}
