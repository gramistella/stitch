use stitch::core::strip_lines_and_inline_comments;

#[test]
fn strip_normalizes_crlf_to_lf() {
    let src = "a\r\nb // cut\r\nc\r\n";
    let out = strip_lines_and_inline_comments(src, &["//".into()]);
    assert_eq!(out, "a\nb\nc\n");
}
