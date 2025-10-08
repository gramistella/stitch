use std::collections::HashSet;
use std::fs;
use stitch::core::{is_event_path_relevant, scan_dir_to_node};
use tempfile::TempDir;

#[test]
fn exclude_dotfile_by_basename() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create various dotfiles and normal files
    fs::write(root.join(".env"), "secret").unwrap();
    fs::write(root.join(".gitignore"), "ignore").unwrap();
    fs::write(root.join(".config"), "config").unwrap();
    fs::write(root.join("normal.txt"), "normal").unwrap();

    let include = HashSet::new();
    let exclude_exts = HashSet::new();
    let exclude_dirs = HashSet::new();
    let exclude_files = std::iter::once(String::from(".env")).collect();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);
    let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();

    // .env should be excluded, but other dotfiles should remain
    assert!(!names.contains(&".env"));
    assert!(names.contains(&".gitignore"));
    assert!(names.contains(&".config"));
    assert!(names.contains(&"normal.txt"));
}

#[test]
fn exclude_multiple_dotfiles_by_basename() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(root.join(".env"), "secret").unwrap();
    fs::write(root.join(".env.local"), "local secret").unwrap();
    fs::write(root.join(".gitignore"), "ignore").unwrap();
    fs::write(root.join(".config"), "config").unwrap();

    let include = HashSet::new();
    let exclude_exts = HashSet::new();
    let exclude_dirs = HashSet::new();
    let exclude_files = std::iter::once(String::from(".env")).collect();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);
    let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();

    // Only .env should be excluded (exact basename match)
    assert!(!names.contains(&".env"));
    assert!(names.contains(&".env.local")); // Different basename
    assert!(names.contains(&".gitignore"));
    assert!(names.contains(&".config"));
}

#[test]
fn event_relevance_for_excluded_dotfile() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    let env_file = root.join(".env");
    let gitignore_file = root.join(".gitignore");

    let include_exts: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = HashSet::new();
    let exclude_dirs: HashSet<String> = HashSet::new();
    let exclude_files: HashSet<String> = std::iter::once(String::from(".env")).collect();

    // .env should not be relevant due to exclusion
    assert!(
        !is_event_path_relevant(
            &root,
            &env_file,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "excluded dotfile should not be relevant for events"
    );

    // .gitignore should still be relevant
    assert!(
        is_event_path_relevant(
            &root,
            &gitignore_file,
            &include_exts,
            &exclude_exts,
            &exclude_dirs,
            &exclude_files
        ),
        "non-excluded dotfile should be relevant for events"
    );
}

#[test]
fn dotfiles_in_nested_directories_with_excludes() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create nested structure with dotfiles
    let nested_dir = root.join("nested");
    fs::create_dir(&nested_dir).unwrap();

    fs::write(nested_dir.join(".env"), "nested secret").unwrap();
    fs::write(nested_dir.join(".gitignore"), "nested ignore").unwrap();
    fs::write(nested_dir.join("normal.txt"), "nested normal").unwrap();

    // Also create at root level
    fs::write(root.join(".env"), "root secret").unwrap();
    fs::write(root.join("root.txt"), "root normal").unwrap();

    let include = HashSet::new();
    let exclude_exts = HashSet::new();
    let exclude_dirs = HashSet::new();
    let exclude_files = std::iter::once(String::from(".env")).collect();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);

    // Check root level
    let root_names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();
    assert!(!root_names.contains(&".env"));
    assert!(root_names.contains(&"root.txt"));
    assert!(root_names.contains(&"nested"));

    // Check nested level
    let nested_node = tree.children.iter().find(|n| n.name == "nested").unwrap();
    let nested_names: Vec<_> = nested_node
        .children
        .iter()
        .map(|n| n.name.as_str())
        .collect();
    assert!(!nested_names.contains(&".env"));
    assert!(nested_names.contains(&".gitignore"));
    assert!(nested_names.contains(&"normal.txt"));
}

#[test]
fn dotfiles_with_extension_excludes() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(root.join(".env"), "secret").unwrap();
    fs::write(root.join(".env.backup"), "backup").unwrap();
    fs::write(root.join(".config.json"), "config").unwrap();
    fs::write(root.join(".gitignore"), "ignore").unwrap();

    let include = HashSet::new();
    let exclude_exts = std::iter::once(String::from(".backup")).collect();
    let exclude_dirs = HashSet::new();
    let exclude_files = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &exclude_dirs, &exclude_files);
    let names: Vec<_> = tree.children.iter().map(|n| n.name.as_str()).collect();

    // .env.backup should be excluded by extension, others should remain
    assert!(names.contains(&".env"));
    assert!(!names.contains(&".env.backup"));
    assert!(names.contains(&".config.json"));
    assert!(names.contains(&".gitignore"));
}
