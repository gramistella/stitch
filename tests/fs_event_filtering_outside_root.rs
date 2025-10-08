use std::collections::HashSet;
use stitch::core::is_event_path_relevant;
use tempfile::TempDir;

#[test]
fn changes_outside_root_are_not_relevant() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let outside = root.parent().unwrap().join("outside.txt");
    let h = HashSet::new();
    assert!(
        !is_event_path_relevant(&root, &outside, &h, &h, &h, &h),
        "events outside root should be ignored"
    );
}
