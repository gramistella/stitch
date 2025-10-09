use stitch::core::{RustFilterOptions, apply_rust_filters};

#[test]
fn signatures_only_does_not_mangle_struct_and_impl_named_raw_fn() {
    // This is the exact code that breaks: a type named `r#fn` and an impl for it.
    // In signatures-only mode, the buggy reducer mistakes the `fn` inside `r#fn`
    // for the `fn` keyword and converts the surrounding `{ ... }` blocks into `;`.
    let src = r"
#![allow(dead_code)]

pub struct r#fn {
    pub a: i32,
}

impl r#fn {
    pub fn new(a: i32) -> Self { Self { a } }
}

pub fn real_function(x: i32) -> i32 { x + r#fn::new(2).a }
";

    let opts = RustFilterOptions {
        remove_inline_regular_comments: false,
        remove_doc_comments: false,
        function_signatures_only: true,
    };

    let got = apply_rust_filters(src, &opts);

    // EXPECTED (correct) behavior:
    //  - The struct stays braced with its fields.
    //  - The impl stays braced; the method body becomes a signature.
    //  - The free function becomes a signature.
    //
    // CURRENT (buggy) behavior yields:
    //  - `pub struct r#fn ;`
    //  - `impl r#fn ;`
    // which these assertions will catch.

    // struct must remain a braced item with its field
    assert!(
        got.contains("pub struct r#fn {\n    pub a: i32,\n}"),
        "struct should remain braced with fields; got:\n{got}"
    );

    // impl must remain braced; method body reduced to a signature
    assert!(
        got.contains("impl r#fn {\n    pub fn new(a: i32) -> Self;\n}"),
        "impl block should remain (method becomes signature); got:\n{got}"
    );

    // free function reduced to signature is expected
    assert!(
        got.contains("pub fn real_function(x: i32) -> i32;"),
        "free function should be converted to a signature; got:\n{got}"
    );

    // explicitly assert the buggy forms are NOT present
    assert!(
        !got.contains("pub struct r#fn ;"),
        "buggy semicolon struct detected:\n{got}"
    );
    assert!(
        !got.contains("impl r#fn ;"),
        "buggy semicolon impl detected:\n{got}"
    );
}

#[test]
fn generic_struct_and_impl_named_raw_fn_are_not_mangled() {
    let src = r"
pub struct r#fn<T> {
    pub a: T,
}
impl<T> r#fn<T> {
    pub fn new(a: T) -> Self { Self { a } }
}
";
    let got = apply_rust_filters(
        src,
        &RustFilterOptions {
            remove_inline_regular_comments: false,
            remove_doc_comments: false,
            function_signatures_only: true,
        },
    );
    assert!(got.contains("pub struct r#fn<T> {\n    pub a: T,\n}"));
    assert!(got.contains("impl<T> r#fn<T> {\n    pub fn new(a: T) -> Self;\n}"));
    assert!(!got.contains("pub struct r#fn<T> ;"));
    assert!(!got.contains("impl<T> r#fn<T> ;"));
}

#[test]
fn function_named_raw_fn_still_reduces_body() {
    let src = r"
pub fn r#fn() -> i32 { 42 }
";
    let got = apply_rust_filters(
        src,
        &RustFilterOptions {
            remove_inline_regular_comments: false,
            remove_doc_comments: false,
            function_signatures_only: true,
        },
    );
    assert!(got.contains("pub fn r#fn() -> i32;"));
}
