use stitch::core::strip_lines_and_inline_comments;

#[test]
fn unicode_spacing_characters_handling() {
    // Test various Unicode spacing characters
    let test_cases = vec![
        ("\u{00A0}", "Non-breaking space (NBSP)"), // Already tested in existing tests
        ("\u{2000}", "En quad"),
        ("\u{2001}", "Em quad"),
        ("\u{2002}", "En space"),
        ("\u{2003}", "Em space"),
        ("\u{2004}", "Three-per-em space"),
        ("\u{2005}", "Four-per-em space"),
        ("\u{2006}", "Six-per-em space"),
        ("\u{2007}", "Figure space"),
        ("\u{2008}", "Punctuation space"),
        ("\u{2009}", "Thin space"),
        ("\u{200A}", "Hair space"),
        ("\u{200B}", "Zero width space"),
        ("\u{200C}", "Zero width non-joiner"),
        ("\u{200D}", "Zero width joiner"),
        ("\u{202F}", "Narrow no-break space"),
        ("\u{205F}", "Medium mathematical space"),
        ("\u{3000}", "Ideographic space"),
    ];

    for (spacing_char, description) in test_cases {
        let test_line = format!("code {spacing_char}  // comment");
        let result = strip_lines_and_inline_comments(&test_line, &["//".to_string()]);

        // The spacing character should be preserved, but the comment should be removed
        assert!(
            result.contains(spacing_char),
            "{description} should be preserved in output: '{result}'"
        );
        assert!(
            !result.contains("// comment"),
            "Comment should be removed when {description} is present: '{result}'"
        );
    }
}

#[test]
fn bom_at_start_of_file() {
    let content_with_bom = "\u{FEFF}fn main() {\n    // comment\n    println!(\"hello\");\n}";
    let result = strip_lines_and_inline_comments(content_with_bom, &["//".to_string()]);

    // BOM should be preserved
    assert!(
        result.starts_with('\u{FEFF}'),
        "BOM should be preserved at start of file"
    );
    assert!(!result.contains("// comment"), "Comment should be removed");
    assert!(
        result.contains("println!(\"hello\")"),
        "Code should be preserved"
    );
}

#[test]
fn mixed_unicode_spacing_before_prefixes() {
    let test_cases = vec![
        (
            "code \u{00A0}\u{2009}\u{3000}// comment",
            "Mixed spacing before comment",
        ),
        (
            "code \u{2000}\u{2001}\u{2002}# comment",
            "Multiple em spaces before hash comment",
        ),
        ("code \u{200A} -- comment", "Thin space before dash comment"),
    ];

    for (test_line, description) in test_cases {
        let result = strip_lines_and_inline_comments(
            test_line,
            &["//".to_string(), "#".to_string(), "--".to_string()],
        );

        // All Unicode spacing should be preserved
        for ch in test_line.chars() {
            if ch.is_whitespace() && ch != ' ' && ch != '\t' && ch != '\n' {
                assert!(
                    result.contains(ch),
                    "Unicode spacing character {ch} should be preserved in: {description} - Input: {test_line:?}, Output: {result:?}"
                );
            }
        }

        // Comments should be removed
        assert!(
            !result.contains("// comment")
                && !result.contains("# comment")
                && !result.contains("-- comment"),
            "Comments should be removed in: {description} - Input: {test_line:?}, Output: {result:?}"
        );
    }
}

#[test]
fn unicode_spacing_whitespace_detection() {
    // Test that Unicode spacing characters are properly detected as whitespace
    let test_cases = vec![
        ("\u{00A0}\u{2009}code", "NBSP + thin space"),
        ("\u{3000}\u{2000}code", "Ideographic space + en quad"),
        (
            "\u{200B}\u{200C}code",
            "Zero width space + zero width non-joiner",
        ),
        ("\u{FEFF}\u{00A0}code", "BOM + NBSP"),
    ];

    for (test_line, description) in test_cases {
        // Test that comment stripping works correctly with Unicode spacing
        let result = strip_lines_and_inline_comments(
            &format!("{test_line} // comment"),
            &["//".to_string()],
        );

        // The Unicode spacing should be preserved
        assert!(
            result.contains("\u{00A0}")
                || result.contains("\u{2009}")
                || result.contains("\u{3000}")
                || result.contains("\u{2000}")
                || result.contains("\u{200B}")
                || result.contains("\u{200C}")
                || result.contains("\u{FEFF}"),
            "Unicode spacing should be preserved in: {description}"
        );

        // Comment should be removed
        assert!(
            !result.contains("// comment"),
            "Comment should be removed in: {description}"
        );
    }
}

#[test]
fn unicode_spacing_in_string_literals() {
    let test_cases = vec![
        (
            "let s = \"\u{00A0}\u{2009}text\u{3000}\"; // comment",
            "Unicode spacing in string",
        ),
        (
            "let s = '\u{2000}\u{2001}text\u{2002}'; # comment",
            "Multiple Unicode spaces in char",
        ),
        (
            "let s = r#\"\u{200A}\u{200B}raw\u{200C}\"#; -- comment",
            "Unicode spacing in raw string",
        ),
    ];

    for (test_line, description) in test_cases {
        let result = strip_lines_and_inline_comments(
            test_line,
            &["//".to_string(), "#".to_string(), "--".to_string()],
        );

        // String content should be preserved exactly
        let expected_content = if description.contains("raw") {
            "raw"
        } else {
            "text"
        };
        assert!(
            result.contains(expected_content),
            "String content should be preserved in: {description} - Input: {test_line:?}, Output: {result:?}"
        );

        // Comments should be removed
        assert!(
            !result.contains("// comment")
                && !result.contains("# comment")
                && !result.contains("-- comment"),
            "Comments should be removed in: {description}"
        );
    }
}
