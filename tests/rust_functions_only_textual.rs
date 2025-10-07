use stitch::core::RustFilterOptions;
use stitch::core::apply_rust_filters;

const fn opts() -> RustFilterOptions {
    RustFilterOptions {
        remove_inline_regular_comments: false,
        remove_doc_comments: false,
        function_signatures_only: true,
    }
}

#[test]
fn preserves_non_function_items_and_attributes() {
    let src = r"
#![allow(dead_code)]
use std::path::Path;
#[cfg(test)]
mod tests {}
#[test]
fn t() { assert_eq!(1,1); }
";
    let got = apply_rust_filters(src, &opts());
    assert!(got.contains("#![allow(dead_code)]"));
    assert!(got.contains("use std::path::Path;"));
    assert!(got.contains("#[cfg(test)]"));
    assert!(got.contains("mod tests {}"));
    assert!(got.contains("#[test]"));
    assert!(got.contains("fn t"));
    assert!(got.contains(';'));
}

#[test]
fn keeps_raw_strings_and_comments_outside_fn() {
    let src = r##"
// a line
let s = r#"not a { brace }"#;
fn f(x: i32) { let y = { 1 + 2 }; }
// trailing
"##;
    let got = apply_rust_filters(src, &opts());
    assert!(got.contains("// a line"));
    assert!(got.contains("r#\"not a { brace }\"#"));
    assert!(got.contains("fn f"));
    assert!(got.contains(';'));
    assert!(got.contains("// trailing"));
}

#[test]
fn handles_nested_braces_in_fn_body() {
    let src = r"
fn g() { if true { { { 1; } } } }
";
    let got = apply_rust_filters(src, &opts());
    assert!(got.contains("fn g"));
    assert!(got.ends_with(";\n"));
}
