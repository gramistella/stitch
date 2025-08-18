use stitch::core::strip_lines_and_inline_comments;

#[test]
fn strip_adds_final_newline_when_missing() {
    let src = "a = 1  // cut"; // no trailing newline
    let out = strip_lines_and_inline_comments(src, &["//".into()]);
    assert_eq!(out, "a = 1\n");
}
