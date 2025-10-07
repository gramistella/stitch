use proptest::prelude::*;
use stitch::core::collapse_consecutive_blank_lines;
use stitch::core::strip_lines_and_inline_comments;

proptest! {
    // Idempotence: applying collapse twice == once
    #[test]
    fn collapse_idempotent(s in ".*") {
        let collapsed_once = collapse_consecutive_blank_lines(&s);
        let collapsed_twice = collapse_consecutive_blank_lines(&collapsed_once);
        let has_triple_blank = collapsed_twice.contains("\n\n\n");
        prop_assert_eq!(collapsed_once, collapsed_twice);

        // No triple blank lines remain
        prop_assert!(!has_triple_blank);
    }

    // If no prefixes are given, stripping must be identity
    #[test]
    fn strip_with_no_prefixes_is_identity(s in ".*") {
        let out = strip_lines_and_inline_comments(&s, &Vec::<String>::new());
        prop_assert_eq!(out, s);
    }
}
