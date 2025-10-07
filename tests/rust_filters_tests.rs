use stitch::core::{RustFilterOptions, apply_rust_filters, is_rust_file_path};

#[test]
fn detect_rust_path() {
    use std::path::Path;
    assert!(is_rust_file_path(Path::new("src/lib.rs")));
    assert!(!is_rust_file_path(Path::new("src/lib.ts")));
}

#[test]
fn remove_regular_line_and_block_comments_but_keep_strings() {
    let src = r##"
let x = "http://example.com"; // trailing comment
/* block
   comment */
let y = 1; /* inline block */ let z = 2;  // another
let s = r#"// not a comment inside raw"#;
"##;
    let opts = RustFilterOptions {
        remove_inline_regular_comments: true,
        remove_doc_comments: false,
        function_signatures_only: false,
    };
    let got = apply_rust_filters(src, &opts);
    let expected = "let x = \"http://example.com\";\nlet y = 1;  let z = 2;\nlet s = r#\"// not a comment inside raw\"#;\n";
    assert_eq!(got, expected);
}

#[test]
fn remove_doc_comments_line_and_block() {
    let src = r"
/// doc line
//! crate doc
/** block doc */
/*! inner block doc */
fn f() {}
";
    let opts = RustFilterOptions {
        remove_inline_regular_comments: false,
        remove_doc_comments: true,
        function_signatures_only: false,
    };
    let got = apply_rust_filters(src, &opts);
    let expected = "fn f() {}\n";
    assert_eq!(got, expected);
}

#[test]
fn function_signatures_only_extracts_free_impl_trait_methods() {
    let src = r"
pub fn free<T>(x: T) -> Result<(), ()> { Ok(()) }

impl Foo {
    pub async fn m(&self, x: i32) {}
}

trait T {
    fn t(&self);
}
";
    let opts = RustFilterOptions {
        remove_inline_regular_comments: false,
        remove_doc_comments: false,
        function_signatures_only: true,
    };
    let got = apply_rust_filters(src, &opts);
    // We render full signatures; allow flexible whitespace from token printing
    assert!(got.contains("fn free"));
    assert!(got.contains("(x:"));
    assert!(got.contains("-> Result"));
    assert!(got.contains("async fn m"));
    assert!(got.contains("x: i32"));
    assert!(got.contains("fn t"));
    assert!(got.contains("&self"));
}

#[test]
fn nested_block_comments_removed_when_regular_comments_enabled() {
    let src = "let a = 1; /* level1 /* level2 */ */ let b = 2;";
    let opts = RustFilterOptions {
        remove_inline_regular_comments: true,
        remove_doc_comments: false,
        function_signatures_only: false,
    };
    let got = apply_rust_filters(src, &opts);
    assert_eq!(got, "let a = 1;  let b = 2;");
}

#[test]
fn signatures_filter_matches_variants() {
    use stitch::core::signatures_filter_matches;
    assert!(signatures_filter_matches("src/main.rs", "src/*"));
    assert!(signatures_filter_matches("tests/foo.rs", "tests/*"));
    assert!(signatures_filter_matches("src/lib.rs", "lib.rs"));
    assert!(signatures_filter_matches(
        "pkg/src/lib.rs",
        "lib.rs,tests/*"
    ));
    assert!(!signatures_filter_matches("src/lib.rs", "tests/*,main.rs"));
    assert!(!signatures_filter_matches("src/lib.rs", "   "));
}

#[test]
fn per_file_signatures_only_respects_filter() {
    use stitch::core::signatures_filter_matches;
    // Simulate selection code's decision process for two files
    let opts_on = RustFilterOptions {
        remove_inline_regular_comments: false,
        remove_doc_comments: false,
        function_signatures_only: true,
    };
    let src = "pub fn a(x: i32) {}\nfn b() {}\n";

    // Filter: only src/* files should be signature-only
    let filter = "src/*";

    // src/lib.rs -> signature-only
    let eff_src = if !filter.trim().is_empty() && !signatures_filter_matches("src/lib.rs", filter) {
        let mut e = opts_on.clone();
        e.function_signatures_only = false;
        e
    } else {
        opts_on.clone()
    };
    let got_src = apply_rust_filters(src, &eff_src);
    assert!(got_src.contains("fn a"));
    assert!(got_src.contains("x: i32"));
    assert!(got_src.contains("fn b"));

    // tests/mod.rs -> not signature-only
    let eff_tests =
        if !filter.trim().is_empty() && !signatures_filter_matches("tests/mod.rs", filter) {
            let mut e = opts_on;
            e.function_signatures_only = false;
            e
        } else {
            opts_on
        };
    let got_tests = apply_rust_filters(src, &eff_tests);
    assert_eq!(got_tests, src);
}
