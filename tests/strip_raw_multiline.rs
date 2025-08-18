// tests/strip_raw_multiline.rs
use stitch::core::strip_lines_and_inline_comments;

#[test]
fn raw_string_with_hashes_spans_lines_and_blocks_inline() {
    // Use 3 hashes on the OUTER raw string to safely contain an inner r##"..."
    let src = r###"let s = r##"line 1 // keep
line 2 # keep
"##   // cut
"###;
    let out = strip_lines_and_inline_comments(src, &["//".into(), "#".into()]);
    let expected = "let s = r##\"line 1 // keep\nline 2 # keep\n\"##\n";
    assert_eq!(out, expected);
}
