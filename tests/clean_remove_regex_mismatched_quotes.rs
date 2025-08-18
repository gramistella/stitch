use stitch::core::clean_remove_regex;

#[test]
fn mismatched_or_unquoted_is_identity() {
    assert_eq!(clean_remove_regex("'abc\""), "'abc\"");
    assert_eq!(clean_remove_regex("abc"), "abc");
}
