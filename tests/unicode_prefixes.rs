use stitch::core::strip_lines_and_inline_comments;

#[test]
fn supports_fullwidth_hash_prefix() {
    // U+FF03 FULLWIDTH NUMBER SIGN
    let src = "keep\n  ＃ full line\nx  ＃ tail\n";
    let out = strip_lines_and_inline_comments(src, &["＃".into()]);
    assert_eq!(out, "keep\nx\n");
}
