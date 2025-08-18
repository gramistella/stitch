use pretty_assertions::assert_eq;
use stitch::core::parse_extension_filters;

#[test]
fn extension_filters_ignore_empty_and_weird_tokens_and_normalize_case() {
    // Intentionally messy: empty tokens, dots alone, stray "-".
    let (inc, exc) = parse_extension_filters(" , ., . , - ,     -. , Rs, -TOML, .JPEG, - .PNG ");

    // Normalized to lowercase and dotted
    assert!(inc.contains(".rs"));
    assert!(inc.contains(".jpeg"));
    assert!(exc.contains(".toml"));
    assert!(exc.contains(".png"));

    // None of the meaningless tokens should have produced entries
    // (exact counts just sanity-check we didn't accidentally add empties).
    assert_eq!(inc.len(), 2);
    assert_eq!(exc.len(), 2);
}

#[test]
fn extension_filters_allow_same_ext_in_both_sets_without_crashing() {
    // Parsing itself allows conflicts; resolution is up to caller logic.
    let (inc, exc) = parse_extension_filters(" md , -md ");
    assert!(inc.contains(".md"));
    assert!(exc.contains(".md"));
}
