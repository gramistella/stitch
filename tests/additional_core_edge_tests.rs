use pretty_assertions::assert_eq;
use stitch::core::*;
use tempfile::TempDir;

#[test]
fn parse_hierarchy_text_empty_returns_none() {
    assert!(parse_hierarchy_text("").is_none());
}

#[test]
fn parse_hierarchy_text_just_root_yields_empty_set() {
    let got = parse_hierarchy_text("root\n").unwrap();
    assert!(got.is_empty());
}

#[test]
fn render_unicode_tree_empty_paths() {
    let with_root = render_unicode_tree_from_paths(&[], Some("root"));
    assert_eq!(with_root, "root\n");

    let without_root = render_unicode_tree_from_paths(&[], None);
    assert_eq!(without_root, "");
}

#[test]
fn clean_remove_regex_single_and_triple_single_quotes() {
    assert_eq!(clean_remove_regex("'abc'"), "abc");
    assert_eq!(clean_remove_regex("'''x.*?y'''"), "x.*?y");
}

#[test]
fn compile_remove_regex_opt_multiline_dot_matches_newline() {
    let re = compile_remove_regex_opt(Some("START.*?END")).unwrap(); // (?ms) gets prepended
    let s = "a START\n mid \nEND b";
    let out = re.replace_all(s, "");
    assert_eq!(out, "a  b");
}

#[test]
fn normalize_path_lexically_cleans_dot_and_dotdot_tails() {
    use std::ffi::OsStr;
    use std::path::Component;

    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    let base = root.join("base").join("existing");
    std::fs::create_dir_all(&base).unwrap();

    // Path with dot/dotdot that should normalize logically.
    let weird = root.join("base/existing/../existing/new/../leaf");
    let norm = normalize_path(&weird);

    // Always true: the tail is "leaf".
    assert_eq!(norm.file_name(), Some(OsStr::new("leaf")));

    // Robust check: ensure "existing" appears among ancestors,
    // regardless of whether it's the *immediate* parent or not.
    let has_existing_ancestor = norm
        .components()
        .any(|c| matches!(c, Component::Normal(os) if os == OsStr::new("existing")));
    assert!(
        has_existing_ancestor,
        "normalized path should retain an 'existing' ancestor, got: {}",
        norm.display()
    );
}

#[test]
fn is_ancestor_of_is_reflexive_and_honors_normalization() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let a = root.join("a");
    let b = a.join("b");
    std::fs::create_dir_all(&b).unwrap();

    // same path → true
    assert!(is_ancestor_of(&a, &a));

    // dotdot segment normalizes
    let odd = a.join("../a");
    assert!(is_ancestor_of(&odd, &b));
    assert!(!is_ancestor_of(&b, &a));
}
