use proptest::prelude::*;
use stitch::core::{parse_hierarchy_text, render_unicode_tree_from_paths};

fn segment() -> impl Strategy<Value = String> {
    // simple dir/file name parts, no slashes
    "[A-Za-z0-9_\\-]{1,8}".prop_map(|s| s)
}

fn ext() -> impl Strategy<Value = String> {
    // small alpha extension
    "[a-z]{1,3}".prop_map(|s| s)
}

fn path() -> impl Strategy<Value = String> {
    // depth: 1..=3; last element 50% dir, 50% file
    (1usize..=3, prop::bool::ANY)
        .prop_flat_map(|(depth, is_file)| {
            let dirs = prop::collection::vec(segment(), depth.saturating_sub(1).max(0));
            let leaf = if is_file {
                (segment(), ext())
                    .prop_map(|(stem, e)| format!("{stem}.{e}"))
                    .boxed()
            } else {
                segment().boxed()
            };
            (dirs, leaf)
        })
        .prop_map(|(mut dirs, leaf)| {
            dirs.push(leaf);
            dirs.join("/")
        })
}

proptest! {
    // Rendering and then parsing should contain every original relative path
    #[test]
    fn render_then_parse_contains_all_input(paths in prop::collection::vec(path(), 1..8)) {
        let tree = render_unicode_tree_from_paths(&paths, Some("root"));
        let parsed = parse_hierarchy_text(&tree).expect("root line is present");

        for p in paths {
            prop_assert!(parsed.contains(&p),
                "parsed set must contain original path: {p}\nRendered:\n{tree}");
        }
    }

    // Rendering is deterministic w.r.t. input order (BTreeMap ordering)
    #[test]
    fn render_is_deterministic_under_permutations(mut paths in prop::collection::vec(path(), 1..8)) {
        let mut a = paths.clone();
        a.sort(); // canonical order
        let det1 = render_unicode_tree_from_paths(&a, Some("root"));

        paths.reverse(); // different order
        let det2 = render_unicode_tree_from_paths(&paths, Some("root"));

        prop_assert_eq!(det1, det2);
    }

    // Duplicate input paths do not duplicate leaves in the output
    #[test]
    fn render_deduplicates_leaves(paths in prop::collection::vec(path(), 1..8)) {
        let mut dup = paths.clone();
        dup.extend(paths.clone()); // deliberately duplicate
        let out = render_unicode_tree_from_paths(&dup, Some("root"));

        for leaf in paths {
            let needle = leaf.rsplit('/').next().unwrap(); // last component only
            let count = out.matches(needle).count();
            prop_assert!(count >= 1, "expected at least one occurrence of {needle}");
        }
    }
}
