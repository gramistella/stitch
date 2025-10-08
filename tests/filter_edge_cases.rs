use std::path::Path;
use stitch::core::{
    RustFilterOptions, SlintFilterOptions, apply_rust_filters, apply_slint_filters,
    is_rust_file_path,
};

#[test]
fn rust_path_detection_is_case_sensitive() {
    // Test that path detection is case-sensitive (current implementation)
    assert!(!is_rust_file_path(Path::new("src/lib.RS"))); // Uppercase extension
    assert!(is_rust_file_path(Path::new("src/lib.rs"))); // Lowercase extension
    assert!(!is_rust_file_path(Path::new("src/lib.Rs"))); // Mixed case extension
}

#[test]
fn rust_long_where_clauses_across_lines() {
    let src = r#"
fn complex_function<T, U, V>()
where
    T: Clone + Send + Sync,
    U: Iterator<Item = T>,
    V: FnOnce(T) -> Result<U, String>,
{
    // function body
    println!("hello");
}
"#;
    let opts = RustFilterOptions {
        remove_inline_regular_comments: false,
        remove_doc_comments: false,
        function_signatures_only: true,
    };
    let got = apply_rust_filters(src, &opts);
    // Should preserve the entire function signature including the where clause
    assert!(got.contains("where"));
    assert!(got.contains("T: Clone + Send + Sync"));
    assert!(!got.contains("println!"));
}

#[test]
fn rust_attributes_on_functions() {
    let src = r"
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub fn attributed_function() -> i32 {
    // function body
    42
}
";
    let opts = RustFilterOptions {
        remove_inline_regular_comments: false,
        remove_doc_comments: false,
        function_signatures_only: true,
    };
    let got = apply_rust_filters(src, &opts);
    // Should preserve attributes
    assert!(got.contains("#[derive(Debug, Clone)]"));
    assert!(got.contains("#[allow(dead_code)]"));
    assert!(got.contains("pub fn attributed_function() -> i32"));
    assert!(!got.contains("42"));
}

#[test]
fn rust_generics_with_lifetimes_and_bounds() {
    let src = r"
fn lifetime_generic<'a, 'b, T>(x: &'a T, y: &'b T) -> &'a T
where
    T: Clone + 'static,
    'b: 'a,
{
    // function body
    x.clone()
}
";
    let opts = RustFilterOptions {
        remove_inline_regular_comments: false,
        remove_doc_comments: false,
        function_signatures_only: true,
    };
    let got = apply_rust_filters(src, &opts);
    // Should preserve complex generics and lifetime bounds
    assert!(got.contains("fn lifetime_generic<'a, 'b, T>"));
    assert!(got.contains("where"));
    assert!(got.contains("T: Clone + 'static"));
    assert!(got.contains("'b: 'a"));
    assert!(!got.contains("x.clone()"));
}

#[test]
fn rust_macros_in_bodies_signatures_only() {
    let src = r#"
fn macro_function() {
    println!("Hello, world!");
    dbg!(42);
    vec![1, 2, 3];
}
"#;
    let opts = RustFilterOptions {
        remove_inline_regular_comments: false,
        remove_doc_comments: false,
        function_signatures_only: true,
    };
    let got = apply_rust_filters(src, &opts);
    // Should remove macro calls in function body
    assert!(got.contains("fn macro_function()"));
    assert!(!got.contains("println!"));
    assert!(!got.contains("dbg!"));
    assert!(!got.contains("vec!"));
}

#[test]
fn slint_block_comments_no_nesting() {
    let src = r"
slint
/* outer block
   /* inner block */
   more outer */
Text {}
";
    let opts = SlintFilterOptions {
        remove_line_comments: false,
        remove_block_comments: true,
    };
    let got = apply_slint_filters(src, &opts);
    // Current implementation doesn't handle nesting - stops at first */
    assert_eq!(got, "\nslint\n   more outer */\nText {}\n");
}

#[test]
fn slint_multiline_block_and_inline_mix() {
    let src = r#"
slint
/* multi
   line
   block */ Text { text: "hello" } // inline comment
Text {}
"#;
    let opts = SlintFilterOptions {
        remove_line_comments: true,
        remove_block_comments: true,
    };
    let got = apply_slint_filters(src, &opts);
    // Should remove both block and inline comments
    assert_eq!(got, "\nslint\n Text { text: \"hello\" } \nText {}\n");
}

#[test]
fn slint_complex_string_escaping() {
    let src = r#"
Text { text: "quoted \"string\" with // not comment" }
Text { text: 'single \'quote\' with /* not comment */' }
"#;
    let opts = SlintFilterOptions {
        remove_line_comments: true,
        remove_block_comments: true,
    };
    let got = apply_slint_filters(src, &opts);
    // Should preserve strings with escaped quotes and not treat comment markers inside as comments
    assert_eq!(got, src);
}
