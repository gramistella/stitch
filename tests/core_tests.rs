use pretty_assertions::assert_eq;
use stitch::core::*;

#[test]
fn test_parse_extension_filters() {
    let (inc, exc) = parse_extension_filters(".rs, .toml, -.lock, -md, js");
    assert!(inc.contains(".rs"));
    assert!(inc.contains(".toml"));
    assert!(inc.contains(".js"));
    assert!(exc.contains(".lock"));
    assert!(exc.contains(".md"));
}

#[test]
fn test_split_prefix_list() {
    assert_eq!(split_prefix_list(" #, // ,--,"), vec!["#", "//", "--"]);
    assert_eq!(split_prefix_list("   ,  ,  "), Vec::<String>::new());
}

#[test]
fn test_clean_and_compile_regex() {
    let raw = "  \"\"\"foo.*?bar\"\"\"  ";
    let cleaned = clean_remove_regex(raw);
    assert_eq!(cleaned, "foo.*?bar");
    let re = compile_remove_regex_opt(Some(&cleaned)).unwrap();
    assert!(re.is_match("zzzfoo\n\nBARbar"));
}

#[test]
fn test_strip_lines_and_inline_comments() {
    let src = "\
    # shebang-like\n\
    let a = 1;  // comment\n\
       // full line comment\n\
    let b = 2;    # another\n\
    let c = 3; #tail
";
    let out = strip_lines_and_inline_comments(src, &["#".into(), "//".into()]);
    // Full-line comment removal does NOT leave a blank spacer line.
    let expected = "\
    let a = 1;\n\
    let b = 2;\n\
    let c = 3;\n";
    assert_eq!(out, expected);
}

#[test]
fn test_collapse_consecutive_blank_lines() {
    let s = "a\n\n\nb\n\nc\n";
    let got = collapse_consecutive_blank_lines(s);
    assert_eq!(got, "a\n\nb\n\nc\n");
}

#[test]
fn test_render_then_parse_hierarchy_roundtrip() {
    let mut paths = vec![
        "src/main.rs".to_string(),
        "src/lib.rs".to_string(),
        "README.md".to_string(),
        "tests/core_tests.rs".to_string(),
    ];
    paths.sort();

    let tree = render_unicode_tree_from_paths(&paths, Some("stitch"));
    let parsed = parse_hierarchy_text(&tree).unwrap();

    // The parser includes intermediate directories as nodes ("src", "tests").
    let mut expected: std::collections::HashSet<String> = paths.into_iter().collect();
    expected.insert("src".into());
    expected.insert("tests".into());
    assert_eq!(parsed, expected);
}

#[test]
fn test_path_helpers() {
    use std::path::PathBuf;
    let p = PathBuf::from("a/b/c");
    assert_eq!(path_to_unix(&p), "a/b/c");

    let p2 = PathBuf::from("this/path/does/not/exist");
    let n = normalize_path(&p2);
    assert!(n.ends_with("this/path/does/not/exist"));

    let anc = PathBuf::from("a/b");
    let leaf = PathBuf::from("a/b/c/d");
    assert!(is_ancestor_of(&anc, &leaf));
    assert!(!is_ancestor_of(&leaf, &anc));
}

#[test]
fn parse_hierarchy_text_minimal() {
    let tree = "\
root
├── src
│   └── main.rs
└── README.md
";
    let got = parse_hierarchy_text(tree).unwrap();
    let mut expected = std::collections::HashSet::new();
    expected.insert("src".into());
    expected.insert("src/main.rs".into());
    expected.insert("README.md".into());
    assert_eq!(got, expected);
}

#[test]
fn parse_extension_filters_normalization_and_conflicts() {
    // Upper-case, missing dot, spaces, and conflicting include/exclude on .md
    let (inc, exc) = parse_extension_filters(" .RS , md ,  -.TMP , - .Md ");
    assert!(inc.contains(".rs"));
    assert!(inc.contains(".md"));
    assert!(exc.contains(".tmp"));
    // parse step doesn’t resolve conflicts; both sets can contain the same ext:
    assert!(exc.contains(".md"));
}

#[test]
fn compile_remove_regex_opt_invalid_returns_none() {
    // Unbalanced paren is invalid
    let bad = compile_remove_regex_opt(Some("("));
    assert!(bad.is_none());
}

#[test]
fn strip_inline_comments_requires_whitespace_before_prefix() {
    let src = "\
let url = \"http://example.com\"; // ok strip
path=http://x#y  # not-a-comment here
x=1#no-space-prefix
y = 2 # strip me
";
    let out = strip_lines_and_inline_comments(src, &["#".into(), "//".into()]);
    // Should strip the // trail and the '# strip me', but not 'http://x#y' or 'x=1#no-space-prefix'
    let expected = "\
let url = \"http://example.com\";
path=http://x#y
x=1#no-space-prefix
y = 2
";
    assert_eq!(out, expected);
}

#[test]
fn collapse_blank_lines_preserves_no_trailing_newline() {
    let s = "a\n\n\nb";
    let got = collapse_consecutive_blank_lines(s);
    assert_eq!(got, "a\n\nb");
}

#[test]
fn render_unicode_tree_deterministic_ordering() {
    // Unsorted input should render alphabetically thanks to BTreeMap
    let paths = vec![
        "b/b2.txt".to_string(),
        "a/a2.txt".to_string(),
        "a/a1.txt".to_string(),
        "b/b1.txt".to_string(),
    ];
    let rendered = render_unicode_tree_from_paths(&paths, Some("root"));
    let expected = "\
root
├── a
│   ├── a1.txt
│   └── a2.txt
└── b
    ├── b1.txt
    └── b2.txt
";
    assert_eq!(rendered, expected);
}

#[test]
fn is_ancestor_of_normalizes_dotdot_segments() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let a = root.join("a");
    let b = a.join("b");
    fs::create_dir_all(&b).unwrap();

    let weird = root.join("a/../a/b"); // should normalize to root/a/b
    assert!(is_ancestor_of(&weird, &b.join("leaf")));
    assert!(is_ancestor_of(&a, &b));
    assert!(!is_ancestor_of(&b, &a));
}
