use stitch::core::render_unicode_tree_from_paths;

#[test]
fn connectors_correct_on_mixed_depths() {
    let paths = vec!["a/b/c.txt".into(), "a/d.txt".into(), "e.txt".into()];
    let out = render_unicode_tree_from_paths(&paths, Some("root"));
    let expected = "\
root
├── a
│   ├── b
│   │   └── c.txt
│   └── d.txt
└── e.txt
";
    assert_eq!(out, expected);
}
