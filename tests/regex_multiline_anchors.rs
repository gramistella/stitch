// tests/regex_multiline_anchors.rs
use stitch::core::*;

#[test]
fn remove_regex_respects_multiline_line_anchors_single_line_lazy() {
    // With (?ms), use .*? to avoid spanning multiple lines.
    let re = compile_remove_regex_opt(Some("^X.*?$")).unwrap();
    let s = "A\nX mid\nB\n";
    let out = re.replace_all(s, "").to_string();
    assert_eq!(out, "A\n\nB\n");
}

#[test]
fn remove_regex_dot_matches_newlines_spanning_blocks() {
    // Still verifies the intended (?s) behavior (dot matches newline).
    let re = compile_remove_regex_opt(Some("START.*END")).unwrap();
    let s = "pre\nSTART\nin between\nEND\npost";
    let out = re.replace_all(s, "").to_string();
    assert_eq!(out, "pre\n\npost");
}
