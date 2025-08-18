use pretty_assertions::assert_eq;
use stitch::core::strip_lines_and_inline_comments;

#[test]
fn keeps_prefixes_inside_double_quotes() {
    let src = "let a = \" #, // , -- inside\"  // comment\n";
    let out = strip_lines_and_inline_comments(src, &["#".into(), "//".into(), "--".into()]);
    assert_eq!(out, "let a = \" #, // , -- inside\"\n");
}

#[test]
fn keeps_prefixes_inside_single_quotes() {
    // Language-agnostic single-quoted string
    let src = "name = 'path // keep'  # cut\n";
    let out = strip_lines_and_inline_comments(src, &["#".into(), "//".into()]);
    assert_eq!(out, "name = 'path // keep'\n");
}

#[test]
fn keeps_prefixes_inside_raw_string_hash0() {
    let src = r#"let s = r"keep // inside"  # trim
"#;
    let out = strip_lines_and_inline_comments(src, &["#".into(), "//".into()]);
    assert_eq!(out, "let s = r\"keep // inside\"\n");
}

#[test]
fn keeps_prefixes_inside_raw_string_hash3() {
    let src = r####"let s = r###"keep // inside"###    -- cut
"####;

    let out = strip_lines_and_inline_comments(src, &["--".into(), "//".into()]);
    assert_eq!(out, "let s = r###\"keep // inside\"###\n");
}

#[test]
fn keeps_prefixes_inside_triple_quotes_same_line() {
    let src = r#"x = """ keep // inside """   // outer
"#;
    let out = strip_lines_and_inline_comments(src, &["//".into()]);
    assert_eq!(out, "x = \"\"\" keep // inside \"\"\"\n");
}

#[test]
fn removes_full_line_comments_with_leading_ws() {
    let src = "   // full line\ncode\n\t# also full line\n";
    let out = strip_lines_and_inline_comments(src, &["//".into(), "#".into()]);
    assert_eq!(out, "code\n");
}

#[test]
fn inline_requires_immediate_whitespace_before_prefix() {
    let src = "value=1//not comment\n";
    let out = strip_lines_and_inline_comments(src, &["//".into()]);
    assert_eq!(out, "value=1//not comment\n");
}

#[test]
fn whitespace_can_be_tab_before_prefix() {
    let src = "v\t// cut here\n";
    let out = strip_lines_and_inline_comments(src, &["//".into()]);
    assert_eq!(out, "v\n");
}

#[test]
fn escaped_quotes_inside_double_quotes_do_not_break() {
    let src = "let s = \"He said \\\"// not\\\" ok\"  // remove\n";
    let out = strip_lines_and_inline_comments(src, &["//".into()]);
    assert_eq!(out, "let s = \"He said \\\"// not\\\" ok\"\n");
}

#[test]
fn raw_string_unclosed_on_line_keeps_rest_of_line() {
    // We simulate a line where a raw string start appears but no closing quote+hashes on the same line.
    let src = r#"let s = r#"unterminated // still inside
"#;
    let out = strip_lines_and_inline_comments(src, &["//".into()]);
    assert_eq!(out, src);
}

#[test]
fn earliest_prefix_wins_when_multiple_present() {
    let src = "let x = 1   // first  # second\n";
    let out = strip_lines_and_inline_comments(src, &["#".into(), "//".into()]);
    assert_eq!(out, "let x = 1\n");
}
