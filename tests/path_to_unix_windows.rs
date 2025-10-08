#[cfg(windows)]
#[test]
fn path_to_unix_converts_backslashes_and_keeps_drive() {
    use std::path::PathBuf;
    use stitch::core::path_to_unix;

    let p = PathBuf::from(r"C:\projects\stitch\src\lib.rs");
    let s = path_to_unix(&p);
    assert!(
        s.contains("C:"),
        "drive letter should be preserved in lossy path"
    );
    assert!(
        s.ends_with("projects/stitch/src/lib.rs"),
        "backslashes should become slashes"
    );
}
