use proptest::prelude::*;
use stitch::core::parse_extension_filters;

proptest! {
    #[test]
    fn extension_filters_shape(raw in ".*") {
        let (inc, exc) = parse_extension_filters(&raw);

        for e in inc.iter().chain(exc.iter()) {
            prop_assert!(e.starts_with('.'), "entry must start with a dot: {}", e);
            prop_assert!(e.len() >= 2, "entry must not be only a single dot: {}", e);
            prop_assert_eq!(e.trim(), e, "no leading/trailing spaces: {}", e);
            prop_assert_eq!(e, &e.to_lowercase(), "normalized to lowercase: {}", e);
        }
    }
}
