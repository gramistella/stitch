use stitch::core::strip_lines_and_inline_comments;

#[test]
fn inline_comment_treats_nbsp_and_emspace_as_whitespace() {
    // Both NBSP (U+00A0) and EM SPACE (U+2003) are considered whitespace by char::is_whitespace(),
    // so the inline comment after them will be stripped. Only ASCII space/tab are trimmed
    // from the left side after cutting, so those wide spaces remain in the kept text.
    let src = concat!(
        "a\u{00A0}// NBSP before slashes -> strip\n",
        "b\u{2003}// EM space before slashes -> strip\n",
        "c // ASCII space before slashes -> strip\n",
        "d\t// TAB before slashes -> strip\n",
    );

    let out = strip_lines_and_inline_comments(src, &["//".into()]);
    // NBSP and EM SPACE remain because the function only trims ' ' and '\t' after cutting.
    let expected = concat!("a\u{00A0}\n", "b\u{2003}\n", "c\n", "d\n",);

    assert_eq!(out, expected);
}
