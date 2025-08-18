use proptest::prelude::*;
use stitch::core::collapse_consecutive_blank_lines;
use stitch::core::strip_lines_and_inline_comments;

proptest! {
    // Idempotence: applying collapse twice == once
    #[test]
    fn collapse_idempotent(s in ".*") {
        let once = collapse_consecutive_blank_lines(&s);
        let twice = collapse_consecutive_blank_lines(&once);
        prop_assert_eq!(once.clone(), twice);

        // No triple blank lines remain
        prop_assert!(!once.contains("\n\n\n"));
    }

    // If no prefixes are given, stripping must be identity
    #[test]
    fn strip_with_no_prefixes_is_identity(s in ".*") {
        let out = strip_lines_and_inline_comments(&s, &Vec::<String>::new());
        prop_assert_eq!(out, s);
    }
}
