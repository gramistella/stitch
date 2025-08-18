use stitch::core::strip_lines_and_inline_comments;

#[test]
fn overlapping_prefixes_earliest_wins() {
    let src = "v   ## cut here (long)\n w    # and here\nx # tail\n";
    let out = strip_lines_and_inline_comments(src, &["##".into(), "#".into()]);

    // First line trimmed at "##" after spaces; second line trimmed at "#"; third line trimmed at "#".
    // Only ASCII spaces/tabs before the prefix are removed from the kept slice's end.
    assert_eq!(out, "v\n w\nx\n");
}
