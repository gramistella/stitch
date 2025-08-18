#[cfg(unix)]
#[test]
fn path_to_unix_non_utf8_replaces_invalid() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;
    use stitch::core::path_to_unix;

    let os = OsString::from_vec(vec![b'a', 0xFF, b'b']); // invalid UTF-8 byte
    let p = PathBuf::from(os);
    let s = path_to_unix(&p);
    assert!(
        s.contains('\u{FFFD}'),
        "expected replacement char in '{}'",
        s
    );
}
