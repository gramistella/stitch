use stitch::core::strip_lines_and_inline_comments;

#[test]
fn full_line_comment_removed_after_unmatched_apostrophe() {
    // The first line contains an apostrophe within a word ("project's"),
    // which used to leak a single-quote string state into the next line.
    // The next line is a full-line Lean comment ("-- ...") and must be removed.
    let src = "\
Intro: project's details
-- this should be stripped
x = 1
";
    let out = strip_lines_and_inline_comments(src, &["--".into()]);
    let expected = "\
Intro: project's details
x = 1
";
    assert_eq!(out, expected);
}

#[test]
fn full_line_comment_removed_after_unclosed_double_quote() {
    // Similarly, if the prior line ends with an opening double quote and no closer,
    // the next full-line comment must still be removed.
    let src = "\
title: \"Unfinished
-- lean line to strip
ok
";
    let out = strip_lines_and_inline_comments(src, &["--".into()]);
    let expected = "\
title: \"Unfinished
ok
";
    assert_eq!(out, expected);
}
