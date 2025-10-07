// tests/prop_fs_scanner_random_tree.rs
use proptest::prelude::*;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

use stitch::core::{Node, path_to_unix, scan_dir_to_node};

/// ===== Generators =====
fn seg() -> impl Strategy<Value = String> {
    // directory / file name part (no slash)
    // small to keep FS work cheap
    "[A-Za-z0-9_\\-]{1,8}".prop_map(|s| s)
}

fn extseg() -> impl Strategy<Value = String> {
    // lowercase extension segment
    "[a-z]{1,3}".prop_map(|s| s)
}

#[derive(Clone, Debug)]
struct FileSpec {
    dirs: Vec<String>, // relative directory components
    fname: String,     // file name (may have one or two dots)
    last_ext: String,  // last extension normalized like ".rs" (lower) or "" if none
}

fn filename_with_last_ext() -> impl Strategy<Value = (String, String)> {
    // 70% single extension:  name.ext   (last_ext=".ext")
    // 30% double extension: name.ext1.ext2 (last_ext=".ext2")
    prop_oneof![
        (seg(), extseg()).prop_map(|(base, e)| { (format!("{base}.{e}"), format!(".{e}")) }),
        (seg(), extseg(), extseg())
            .prop_map(|(base, e1, e2)| { (format!("{base}.{e1}.{e2}"), format!(".{e2}")) }),
    ]
}

fn file_spec() -> impl Strategy<Value = FileSpec> {
    (
        prop::collection::vec(seg(), 0..=2),
        filename_with_last_ext(),
    )
        .prop_map(|(dirs, (fname, last_ext))| FileSpec {
            dirs,
            fname,
            last_ext,
        })
}

/// ===== Helpers =====
fn make_on_disk(root: &Path, files: &[FileSpec]) {
    for f in files {
        let mut p = root.to_path_buf();
        for d in &f.dirs {
            p.push(d);
        }
        fs::create_dir_all(&p).unwrap();
        p.push(&f.fname);
        fs::write(p, "x").unwrap();
    }
}

fn last_ext_of_filename(fname: &str) -> String {
    use std::path::Path;
    let p = Path::new(fname);
    match p.extension() {
        Some(e) => format!(".{}", e.to_string_lossy().to_lowercase()),
        None => String::new(),
    }
}

fn collect_tree_paths(
    root: &Path,
    node: &Node,
) -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
    // returns (file_rel_paths, dir_rel_paths, dir_basenames)
    fn walk(
        root: &Path,
        n: &Node,
        files: &mut BTreeSet<String>,
        dirs: &mut BTreeSet<String>,
        dir_basenames: &mut BTreeSet<String>,
    ) {
        if n.is_dir {
            if n.path != root {
                // record every directory except the root itself (for some checks)
                let rel = n.path.strip_prefix(root).unwrap_or(&n.path);
                dirs.insert(path_to_unix(rel));
                dir_basenames.insert(n.name.clone());
            }
            for c in &n.children {
                walk(root, c, files, dirs, dir_basenames);
            }
        } else {
            let rel = n.path.strip_prefix(root).unwrap_or(&n.path);
            files.insert(path_to_unix(rel));
        }
    }
    let mut f = BTreeSet::new();
    let mut d = BTreeSet::new();
    let mut db = BTreeSet::new();
    walk(root, node, &mut f, &mut d, &mut db);
    (f, d, db)
}

fn order_ok_everywhere(node: &Node) -> bool {
    // In each directory: files first (sorted by name), then dirs (sorted by name)
    fn check(n: &Node) -> bool {
        if !n.is_dir {
            return true;
        }
        // partition children
        let mut file_names = Vec::new();
        let mut dir_names = Vec::new();
        let mut saw_dir = false;
        for c in &n.children {
            if c.is_dir {
                saw_dir = true;
                dir_names.push(c.name.clone());
            } else {
                if saw_dir {
                    return false; // found a file after a directory
                }
                file_names.push(c.name.clone());
            }
        }
        let mut f_sorted = file_names.clone();
        f_sorted.sort();
        let mut d_sorted = dir_names.clone();
        d_sorted.sort();
        if file_names != f_sorted || dir_names != d_sorted {
            return false;
        }
        n.children.iter().filter(|c| c.is_dir).all(check)
    }
    check(node)
}

fn any_component_in<'a>(
    mut comps: impl Iterator<Item = &'a str>,
    banned: &HashSet<String>,
) -> bool {
    comps.any(|c| banned.contains(c))
}

/// Deterministic, data-derived subsets so tests are reproducible without extra RNG:
/// - include/ext/exclude sets are subsets of *present* extensions (excluding empty "")
/// - `exclude_dirs` are subset of present directory names
/// - `exclude_files` are subset of present basenames
///   The exact selection uses simple predicates on the string to vary results across inputs.
fn derive_filter_sets(
    files: &[FileSpec],
) -> (
    HashSet<String>, // include_exts
    HashSet<String>, // exclude_exts
    HashSet<String>, // exclude_dirs (by base name)
    HashSet<String>, // exclude_files (basenames)
    bool,            // include_mode (include_exts non-empty)
) {
    let mut present_exts: BTreeSet<String> = BTreeSet::new();
    let mut dir_names: BTreeSet<String> = BTreeSet::new();
    let mut file_basenames: BTreeSet<String> = BTreeSet::new();

    for f in files {
        if !f.last_ext.is_empty() {
            present_exts.insert(f.last_ext.clone());
        }
        for d in &f.dirs {
            dir_names.insert(d.clone());
        }
        file_basenames.insert(f.fname.clone());
    }

    // Build include/exclude ext sets from present_exts deterministically:
    let exts_vec: Vec<_> = present_exts.into_iter().collect();
    let include_mode = !exts_vec.is_empty() && (exts_vec.len() % 2 == 0);
    let mut include_exts = HashSet::new();
    let mut exclude_exts = HashSet::new();
    if include_mode {
        // take "first half" by lexicographic order — guaranteed non-empty
        let half = exts_vec.len().div_ceil(2);
        include_exts.extend(exts_vec.iter().take(half).cloned());
    } else {
        // put all but first into exclude set (may be empty)
        exclude_exts.extend(exts_vec.iter().skip(1).cloned());
    }

    // Exclude dir names with even length — stable and varied enough
    let exclude_dirs: HashSet<String> =
        dir_names.into_iter().filter(|d| d.len() % 2 == 0).collect();

    // Exclude file basenames whose length is divisible by 3
    let exclude_files: HashSet<String> = file_basenames
        .into_iter()
        .filter(|b| b.len() % 3 == 0)
        .collect();

    (
        include_exts,
        exclude_exts,
        exclude_dirs,
        exclude_files,
        include_mode,
    )
}

/// Predicate defining which files SHOULD be included by `scan_dir_to_node` with the given filters.
fn should_include_file(
    rel_components: &[&str],
    basename: &str,
    last_ext: &str,
    include_exts: &HashSet<String>,
    exclude_exts: &HashSet<String>,
    exclude_dirs: &HashSet<String>,
    exclude_files: &HashSet<String>,
) -> bool {
    if any_component_in(rel_components.iter().copied(), exclude_dirs) {
        return false;
    }
    if exclude_files.contains(basename) {
        return false;
    }
    if !include_exts.is_empty() {
        include_exts.contains(last_ext)
    } else if !exclude_exts.is_empty() {
        !exclude_exts.contains(last_ext)
    } else {
        true
    }
}

proptest! {
    // keep the generated tree small and fast
    #![proptest_config(ProptestConfig {
        cases: 64, .. ProptestConfig::default()
    })]

    #[test]
    fn scanner_respects_filters_and_order(files in prop::collection::vec(file_spec(), 1..20)) {
        use std::collections::BTreeSet;

        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // --- helpers (scoped to this test) ---
        fn fs_case_insensitive(root: &std::path::Path) -> bool {
            let probe = root.join("CiProbe");
            let _ = std::fs::create_dir(&probe);
            let insensitive_alias = root.join("ciprobe");
            let is_ci = insensitive_alias.exists();
            let _ = std::fs::remove_dir_all(&probe);
            is_ci
        }
        fn lower_set(set: &BTreeSet<String>) -> BTreeSet<String> {
            set.iter().map(|s| s.to_lowercase()).collect()
        }

        // Materialize the random tree.
        make_on_disk(root, &files);

        // Derive deterministic filter sets from what's present.
        let (include_exts, exclude_exts, exclude_dirs, exclude_files, include_mode) =
            derive_filter_sets(&files);

        // Run the scanner.
        let tree = scan_dir_to_node(
            root,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files,
        );

        // Collect actual model outputs.
        let (actual_files, actual_dirs, actual_dir_basenames) = collect_tree_paths(root, &tree);

        // ==== Expected file set: compute from the generated files and the filters ====
        let mut expected_files: BTreeSet<String> = BTreeSet::new();
        for f in &files {
            // rel components for dir exclusion check
            let mut comps : Vec<&str> = f.dirs.iter().map(std::string::String::as_str).collect();

            // Basename & last ext
            let basename = f.fname.as_str();
            let last_ext = last_ext_of_filename(basename);

            if should_include_file(
                &comps,
                basename,
                &last_ext,
                &include_exts,
                &exclude_exts,
                &exclude_dirs,
                &exclude_files
            ) {
                // Push full rel path
                comps.push(basename);
                let rel_path = comps.join("/");
                expected_files.insert(rel_path);
            }
        }

        // ==== Assertions (case-aware) ====
        let case_insensitive = fs_case_insensitive(root);

        // 1) Files: the model must exactly match the expected set (normalized if FS is CI).
        if case_insensitive {
            prop_assert_eq!(lower_set(&actual_files), lower_set(&expected_files));
        } else {
            prop_assert_eq!(actual_files, expected_files.clone());
        }

        // 2) Per-directory ordering (files then dirs; each block sorted).
        prop_assert!(order_ok_everywhere(&tree), "directory children ordering violated somewhere");

        // 3) No excluded directory basename appears anywhere in the model.
        if case_insensitive {
            for banned in &exclude_dirs {
                let banned_l = banned.to_lowercase();
                prop_assert!(
                    !actual_dir_basenames.iter().any(|n| n.to_lowercase() == banned_l),
                    "excluded dir name '{}' (case-insensitive) must not be present", banned
                );
            }
        } else {
            for banned in &exclude_dirs {
                prop_assert!(
                    !actual_dir_basenames.contains(banned),
                    "excluded dir name '{}' must not be present", banned
                );
            }
        }

        // 4) In include-mode, every non-root directory that appears must have
        //    at least one included file descendant (normalized if FS is CI).
        let expected_files_norm =
            if case_insensitive { lower_set(&expected_files) } else { expected_files.clone() };

        let actual_dirs_norm: BTreeSet<String> = if case_insensitive {
            actual_dirs.iter().map(|d| d.to_lowercase()).collect()
        } else {
            actual_dirs
        };

        if include_mode {
            for d in &actual_dirs_norm {
                // Does this directory prefix at least one expected file?
                let prefix = format!("{d}/");
                let has_desc = expected_files_norm.iter().any(|f| f == d || f.starts_with(&prefix));
                prop_assert!(has_desc,
                    "include-mode: directory '{}' should be hidden if it has no included file descendant",
                    d);
            }
        }
    }
}
