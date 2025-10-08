use stitch::core::{clean_remove_regex, compile_remove_regex_opt, strip_lines_and_inline_comments};

#[test]
fn comment_prefix_chains_earliest_match_wins() {
    let src = "\
let x = 1;  ## double hash comment
let y = 2;  # single hash comment
let z = 3;  --- triple dash comment
let w = 4;  -- double dash comment
";
    let out =
        strip_lines_and_inline_comments(src, &["##".into(), "#".into(), "---".into(), "--".into()]);

    // Should strip all comments since all prefixes are configured
    let expected = "\
let x = 1;
let y = 2;
let z = 3;
let w = 4;
";
    assert_eq!(out, expected);
}

#[test]
fn comment_prefix_chains_longest_prefix_does_not_win() {
    let src = "\
let x = 1;  ## this should match ##
let y = 2;  # this should match #
let z = 3;  --- this should match ---
let w = 4;  -- this should match --
";
    let out =
        strip_lines_and_inline_comments(src, &["##".into(), "#".into(), "---".into(), "--".into()]);

    // All should be stripped regardless of length
    let expected = "\
let x = 1;
let y = 2;
let z = 3;
let w = 4;
";
    assert_eq!(out, expected);
}

#[test]
fn mixed_whitespace_before_inline_prefixes() {
    let src = "\
let x = 1;\t# tab before hash
let y = 2; \u{00A0}# NBSP before hash
let z = 3;\t\u{00A0}# tab and NBSP before hash
let w = 4;  \t# space and tab before hash
";
    let out = strip_lines_and_inline_comments(src, &["#".into()]);

    // Should strip all comments with mixed whitespace
    // NBSP characters remain because only ' ' and '\t' are trimmed after cutting
    let expected = "\
let x = 1;
let y = 2; \u{00A0}
let z = 3;\t\u{00A0}
let w = 4;
";
    assert_eq!(out, expected);
}

#[test]
fn mixed_whitespace_with_multiple_prefixes() {
    let src = "\
let x = 1;\t// tab before double slash
let y = 2; \u{00A0}-- NBSP before double dash
let z = 3;\t\u{00A0}# tab and NBSP before hash
let w = 4;  \t/* space and tab before block start
";
    let out =
        strip_lines_and_inline_comments(src, &["//".into(), "--".into(), "#".into(), "/*".into()]);

    // Should strip all comments with mixed whitespace
    // NBSP characters remain because only ' ' and '\t' are trimmed after cutting
    let expected = "\
let x = 1;
let y = 2; \u{00A0}
let z = 3;\t\u{00A0}
let w = 4;
";
    assert_eq!(out, expected);
}

#[test]
fn regex_quoting_variants_with_escaped_quotes() {
    // Test regex literals with escaped quotes inside quotes
    let raw1 = r#"  """foo\"bar.*?baz\"qux"""  "#;
    let cleaned1 = clean_remove_regex(raw1);
    assert_eq!(cleaned1, r#"foo\"bar.*?baz\"qux"#);

    let raw2 = r"  '''foo\'bar.*?baz\'qux'''  ";
    let cleaned2 = clean_remove_regex(raw2);
    assert_eq!(cleaned2, r"foo\'bar.*?baz\'qux");

    let raw3 = r#"  """foo\\"bar.*?baz\\"qux"""  "#;
    let cleaned3 = clean_remove_regex(raw3);
    assert_eq!(cleaned3, r#"foo\\"bar.*?baz\\"qux"#);
}

#[test]
fn regex_quoting_variants_with_nested_quotes() {
    // Test regex literals with nested quotes
    let raw1 = r#"  """foo"bar.*?baz"qux"""  "#;
    let cleaned1 = clean_remove_regex(raw1);
    assert_eq!(cleaned1, r#"foo"bar.*?baz"qux"#);

    let raw2 = r"  '''foo'bar.*?baz'qux'''  ";
    let cleaned2 = clean_remove_regex(raw2);
    assert_eq!(cleaned2, r"foo'bar.*?baz'qux");
}

#[test]
fn regex_quoting_variants_with_mixed_quotes() {
    // Test regex literals with mixed quote types
    let raw1 = r#"  """foo'bar.*?baz"qux"""  "#;
    let cleaned1 = clean_remove_regex(raw1);
    assert_eq!(cleaned1, r#"foo'bar.*?baz"qux"#);

    let raw2 = r#"  '''foo"bar.*?baz'qux'''  "#;
    let cleaned2 = clean_remove_regex(raw2);
    assert_eq!(cleaned2, r#"foo"bar.*?baz'qux"#);
}

#[test]
fn regex_quoting_variants_compilation() {
    // Test that cleaned regex patterns compile correctly
    let patterns = vec![
        r#"foo\"bar.*?baz\"qux"#,
        r"foo'bar.*?baz'qux",
        r#"foo"bar.*?baz"qux"#,
        r#"foo\\"bar.*?baz\\"qux"#,
    ];

    for pattern in patterns {
        let compiled = compile_remove_regex_opt(Some(pattern));
        assert!(compiled.is_some(), "Pattern should compile: {pattern}");
    }
}

#[test]
fn regex_quoting_variants_matching() {
    // Test that compiled regex patterns match correctly
    let raw = r#"  """foo\"bar.*?baz\"qux"""  "#;
    let cleaned = clean_remove_regex(raw);
    let re = compile_remove_regex_opt(Some(&cleaned)).unwrap();

    // Should match strings containing the pattern
    assert!(re.is_match("prefix foo\"bar\n\nbaz\"qux suffix"));
    assert!(re.is_match("foo\"bar baz\"qux"));
    assert!(!re.is_match("foo bar baz qux")); // No quotes
}

#[test]
fn regex_quoting_variants_with_special_chars() {
    // Test regex literals with special regex characters
    let raw1 = r#"  """foo\(bar\).*?baz\[qux\]"""  "#;
    let cleaned1 = clean_remove_regex(raw1);
    assert_eq!(cleaned1, r"foo\(bar\).*?baz\[qux\]");

    let raw2 = r"  '''foo\{bar\}.*?baz\?qux'''  ";
    let cleaned2 = clean_remove_regex(raw2);
    assert_eq!(cleaned2, r"foo\{bar\}.*?baz\?qux");
}

#[test]
fn regex_quoting_variants_with_unicode() {
    // Test regex literals with Unicode characters
    let raw1 = r#"  """foo\u{1234}bar.*?baz\u{5678}qux"""  "#;
    let cleaned1 = clean_remove_regex(raw1);
    assert_eq!(cleaned1, r"foo\u{1234}bar.*?baz\u{5678}qux");

    let raw2 = r"  '''foo测试bar.*?baz测试qux'''  ";
    let cleaned2 = clean_remove_regex(raw2);
    assert_eq!(cleaned2, r"foo测试bar.*?baz测试qux");
}

#[test]
fn regex_quoting_variants_with_newlines() {
    // Test regex literals with newlines
    let raw1 = r#"  """foo
bar.*?baz
qux"""  "#;
    let cleaned1 = clean_remove_regex(raw1);
    assert_eq!(cleaned1, "foo\nbar.*?baz\nqux");

    let raw2 = r"  '''foo\r\nbar.*?baz\r\nqux'''  ";
    let cleaned2 = clean_remove_regex(raw2);
    assert_eq!(cleaned2, "foo\\r\\nbar.*?baz\\r\\nqux");
}

#[test]
fn regex_quoting_variants_with_tabs() {
    // Test regex literals with tabs
    let raw1 = r#"  """foo\tbar.*?baz\tqux"""  "#;
    let cleaned1 = clean_remove_regex(raw1);
    assert_eq!(cleaned1, "foo\\tbar.*?baz\\tqux");

    let raw2 = r"  '''foo	bar.*?baz	qux'''  "; // Contains actual tabs
    let cleaned2 = clean_remove_regex(raw2);
    assert_eq!(cleaned2, "foo\tbar.*?baz\tqux");
}

#[test]
fn regex_quoting_variants_with_backslashes() {
    // Test regex literals with various backslash sequences
    let raw1 = r#"  """foo\\bar.*?baz\\qux"""  "#;
    let cleaned1 = clean_remove_regex(raw1);
    assert_eq!(cleaned1, r"foo\\bar.*?baz\\qux");

    let raw2 = r"  '''foo\abar.*?baz\bqux'''  ";
    let cleaned2 = clean_remove_regex(raw2);
    assert_eq!(cleaned2, r"foo\abar.*?baz\bqux");
}
