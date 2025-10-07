use pretty_assertions::assert_eq;
use stitch::core::*;

#[test]
fn render_tree_deduplicates_duplicate_paths() {
    let paths = vec![
        "src/lib.rs".to_string(),
        "src/lib.rs".to_string(),
        "src/main.rs".to_string(),
    ];
    let out = render_unicode_tree_from_paths(&paths, Some("root"));
    // "lib.rs" should appear exactly once
    let count_lib = out.matches("lib.rs").count();
    let count_main = out.matches("main.rs").count();
    assert_eq!(count_lib, 1);
    assert_eq!(count_main, 1);
}

#[test]
fn strip_ignores_empty_prefix_entries() {
    let src = "a = 1 // keep\n# full\nb = 2\n";
    // The empty string in the list must be ignored (otherwise it would nuke everything).
    let out = strip_lines_and_inline_comments(src, &[String::new(), "//".into(), "#".into()]);
    assert_eq!(out, "a = 1\nb = 2\n");
}

#[test]
fn compile_remove_regex_opt_none_is_none() {
    assert!(compile_remove_regex_opt(None).is_none());
}
