use stitch::core::render_unicode_tree_from_paths;

#[test]
fn render_without_root_has_no_top_line() {
    let paths = vec!["a/b.rs".into(), "a/c.rs".into(), "d.txt".into()];
    let out = render_unicode_tree_from_paths(&paths, None);
    // Just ensure it starts with the first top-level dir/file and ends with a newline.
    assert!(out.starts_with("├── a\n") || out.starts_with("└── d.txt\n"));
    assert!(out.ends_with('\n'));
}
