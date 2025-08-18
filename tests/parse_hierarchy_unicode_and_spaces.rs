use stitch::core::parse_hierarchy_text;

#[test]
fn parses_unicode_and_spaces_in_names() {
    let tree = "\
root
└── src
    ├── naïve file.rs
    └── emoji 😀.txt
";
    let got = parse_hierarchy_text(tree).unwrap();
    assert!(got.contains("src/naïve file.rs"));
    assert!(got.contains("src/emoji 😀.txt"));
    assert!(got.contains("src")); // intermediate dir captured
}
