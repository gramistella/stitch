use stitch::core::parse_hierarchy_text;

#[test]
fn parse_hierarchy_handles_crlf_endings() {
    let tree = "root\r\n├── src\r\n│   └── main.rs\r\n└── README.md\r\n";
    let got = parse_hierarchy_text(tree).unwrap();

    let mut expected = std::collections::HashSet::new();
    expected.insert("src".into());
    expected.insert("src/main.rs".into());
    expected.insert("README.md".into());
    assert_eq!(got, expected);
}
