use pretty_assertions::assert_eq;
use stitch::core::parse_hierarchy_text;

#[test]
fn parse_hierarchy_text_ignores_trailing_ws_and_blank_lines() {
    // Mixed whitespace and a blank line at the end.
    let tree = "\
myroot   
├── src  
│   ├── lib.rs  
│   └── bin
│       └── main.rs
     
";
    let got = parse_hierarchy_text(tree).unwrap();

    let mut expected = std::collections::HashSet::new();
    expected.insert("src".into());
    expected.insert("src/lib.rs".into());
    expected.insert("src/bin".into());
    expected.insert("src/bin/main.rs".into());

    assert_eq!(got, expected);
}

#[test]
fn parse_hierarchy_text_skips_lines_without_names() {
    // Lines that are only tree runes / whitespace should be skipped.
    let tree = "\
root
│   
└── a
    │
    └── b.txt
";
    let got = parse_hierarchy_text(tree).unwrap();

    let mut expected = std::collections::HashSet::new();
    expected.insert("a".into());
    expected.insert("a/b.txt".into());

    assert_eq!(got, expected);
}

#[test]
fn parse_hierarchy_text_handles_single_level_files() {
    let tree = "\
root
├── README.md
└── LICENSE
";
    let got = parse_hierarchy_text(tree).unwrap();

    let mut expected = std::collections::HashSet::new();
    expected.insert("README.md".into());
    expected.insert("LICENSE".into());

    assert_eq!(got, expected);
}
