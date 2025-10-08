use std::collections::HashSet;
use stitch::core::is_event_path_relevant;
use tempfile::TempDir;

#[test]
fn include_vs_exclude_precedence_events() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    let rs_file = root.join("src").join("main.rs");
    let txt_file = root.join("src").join("readme.txt");

    // Include .rs but exclude .txt - include should win for .rs files
    let include_exts: HashSet<String> = std::iter::once(String::from(".rs")).collect();
    let exclude_exts: HashSet<String> = std::iter::once(String::from(".txt")).collect();
    let exclude_dirs: HashSet<String> = HashSet::new();
    let exclude_files: HashSet<String> = HashSet::new();

    // .rs file should be relevant (include wins)
    assert!(
        is_event_path_relevant(
            &root,
            &rs_file,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "include extension should win over exclude extension"
    );

    // .txt file should not be relevant (exclude wins)
    assert!(
        !is_event_path_relevant(
            &root,
            &txt_file,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "exclude extension should win for non-included files"
    );
}

#[test]
fn exclude_file_overrides_include_ext_events() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    let main_rs = root.join("src").join("main.rs");
    let temp_rs = root.join("src").join("temp.rs");

    // Include .rs but exclude specific file "temp.rs"
    let include_exts: HashSet<String> = std::iter::once(String::from(".rs")).collect();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = HashSet::new();
    let exclude_files: HashSet<String> = std::iter::once(String::from("temp.rs")).collect();

    // main.rs should be relevant (included extension)
    assert!(
        is_event_path_relevant(
            &root,
            &main_rs,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "included extension should make file relevant"
    );

    // temp.rs should not be relevant (excluded by basename)
    assert!(
        !is_event_path_relevant(
            &root,
            &temp_rs,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "excluded file should override included extension"
    );
}

#[test]
fn directory_changes_without_include_mode() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    let src_dir = root.join("src");
    let tests_dir = root.join("tests");
    let target_dir = root.join("target");

    // No include mode (empty include set)
    let include_exts: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = HashSet::new();
    let exclude_files: HashSet<String> = HashSet::new();

    // All directory changes should be relevant when not in include mode
    assert!(
        is_event_path_relevant(
            &root,
            &src_dir,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "directory changes should be relevant without include mode"
    );

    assert!(
        is_event_path_relevant(
            &root,
            &tests_dir,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "directory changes should be relevant without include mode"
    );

    assert!(
        is_event_path_relevant(
            &root,
            &target_dir,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "directory changes should be relevant without include mode"
    );
}

#[test]
fn directory_changes_with_excluded_dirs() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    let src_dir = root.join("src");
    let target_dir = root.join("target");
    let build_dir = root.join("build");

    // No include mode, but exclude target directory
    let include_exts: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = std::iter::once(String::from("target")).collect();
    let exclude_files: HashSet<String> = HashSet::new();

    // src and build should be relevant
    assert!(
        is_event_path_relevant(
            &root,
            &src_dir,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "non-excluded directory should be relevant"
    );

    assert!(
        is_event_path_relevant(
            &root,
            &build_dir,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "non-excluded directory should be relevant"
    );

    // target should not be relevant
    assert!(
        !is_event_path_relevant(
            &root,
            &target_dir,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "excluded directory should not be relevant"
    );
}

#[test]
fn nested_directory_changes_with_excludes() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    let src_dir = root.join("src");
    let src_utils_dir = root.join("src").join("utils");
    let target_dir = root.join("target");
    let target_debug_dir = root.join("target").join("debug");

    // Exclude target directory
    let include_exts: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = std::iter::once(String::from("target")).collect();
    let exclude_files: HashSet<String> = HashSet::new();

    // src directories should be relevant
    assert!(
        is_event_path_relevant(
            &root,
            &src_dir,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "src directory should be relevant"
    );

    assert!(
        is_event_path_relevant(
            &root,
            &src_utils_dir,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "nested src directory should be relevant"
    );

    // target directories should not be relevant
    assert!(
        !is_event_path_relevant(
            &root,
            &target_dir,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "target directory should not be relevant"
    );

    assert!(
        !is_event_path_relevant(
            &root,
            &target_debug_dir,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "nested target directory should not be relevant"
    );
}

#[test]
fn file_vs_directory_exclusion_precedence() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    let temp_file = root.join("temp");
    let temp_dir = root.join("temp");

    // Exclude "temp" as both file and directory
    let include_exts: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = std::iter::once(String::from("temp")).collect();
    let exclude_files: HashSet<String> = std::iter::once(String::from("temp")).collect();

    // Both should be excluded
    assert!(
        !is_event_path_relevant(
            &root,
            &temp_file,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "excluded file should not be relevant"
    );

    assert!(
        !is_event_path_relevant(
            &root,
            &temp_dir,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "excluded directory should not be relevant"
    );
}
