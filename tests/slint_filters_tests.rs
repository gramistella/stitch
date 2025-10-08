use stitch::core::{SlintFilterOptions, apply_slint_filters, is_slint_file_path};

#[test]
fn detect_slint_path() {
    use std::path::Path;
    assert!(is_slint_file_path(Path::new("ui/app.slint")));
    assert!(!is_slint_file_path(Path::new("ui/app.rs")));
}

#[test]
fn remove_only_line_comments() {
    let src = "slint\n// Amazing text! This is a comment\nText {}\n";
    let opts = SlintFilterOptions {
        remove_line_comments: true,
        remove_block_comments: false,
    };
    let got = apply_slint_filters(src, &opts);
    assert_eq!(got, "slint\nText {}\n");
}

#[test]
fn remove_only_block_comments() {
    let src = "slint\n/* multi\n line */\nText {}\n";
    let opts = SlintFilterOptions {
        remove_line_comments: false,
        remove_block_comments: true,
    };
    let got = apply_slint_filters(src, &opts);
    assert_eq!(got, "slint\nText {}\n");
}

#[test]
fn keep_markers_inside_strings() {
    let src = "Text { text: \"http://example.com // not comment\" }\nText { text: '/* not comment */' }\n";
    let opts = SlintFilterOptions {
        remove_line_comments: true,
        remove_block_comments: true,
    };
    let got = apply_slint_filters(src, &opts);
    assert_eq!(got, src);
}

#[test]
fn preserves_escaped_quotes_in_strings() {
    let src = "Text { text: \"\"\" // after escaped quote \"\"\" }\n";
    let opts = SlintFilterOptions {
        remove_line_comments: true,
        remove_block_comments: true,
    };
    let got = apply_slint_filters(src, &opts);
    assert!(got.contains("// after escaped quote"));
}
