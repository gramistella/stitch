use stitch::core::strip_lines_and_inline_comments;

#[test]
fn triple_quoted_multiline_preserves_inside_and_cuts_after_closing() {
    let src = "x = \"\"\"\nline # keep\n\"\"\"    // cut here\n";
    let out = strip_lines_and_inline_comments(src, &["//".into(), "#".into()]);
    let expected = "x = \"\"\"\nline # keep\n\"\"\"\n";
    assert_eq!(out, expected);
}

#[test]
fn triple_single_quoted_multiline_also_protected() {
    let src = "y = '''\n// keep inside\n'''\t# after close, trim\n";
    let out = strip_lines_and_inline_comments(src, &["//".into(), "#".into()]);
    let expected = "y = '''\n// keep inside\n'''\n";
    assert_eq!(out, expected);
}
