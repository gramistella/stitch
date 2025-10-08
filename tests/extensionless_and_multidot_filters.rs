use std::collections::HashSet;
use std::fs;
use stitch::core::*;
use tempfile::TempDir;

#[test]
fn extensionless_filenames_work() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create various extensionless files
    fs::write(root.join("justfile"), "x").unwrap();
    fs::write(root.join("Makefile"), "x").unwrap();
    fs::write(root.join("Dockerfile"), "x").unwrap();
    fs::write(root.join("README"), "x").unwrap();
    fs::write(root.join("LICENSE"), "x").unwrap();

    // Test filtering for extensionless files
    let (inc, _exc) = parse_extension_filters("justfile,Makefile,Dockerfile");
    assert!(inc.contains(".justfile"));
    assert!(inc.contains(".makefile"));
    assert!(inc.contains(".dockerfile"));

    let include: HashSet<String> = inc;
    let exclude_exts: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &ex_dirs, &ex_files);

    // Should include the extensionless files
    assert!(tree.children.iter().any(|n| n.name == "justfile"));
    assert!(tree.children.iter().any(|n| n.name == "Makefile"));
    assert!(tree.children.iter().any(|n| n.name == "Dockerfile"));

    // Should not include other extensionless files
    assert!(!tree.children.iter().any(|n| n.name == "README"));
    assert!(!tree.children.iter().any(|n| n.name == "LICENSE"));
}

#[test]
fn multidot_extensions_work() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create various multi-dot files
    fs::write(root.join("archive.tar.gz"), "x").unwrap();
    fs::write(root.join("bundle.tar.bz2"), "x").unwrap();
    fs::write(root.join("app.test.js"), "x").unwrap();
    fs::write(root.join("types.d.ts"), "x").unwrap();
    fs::write(root.join("config.test.yml"), "x").unwrap();

    // Test filtering for multi-dot extensions
    let (inc, _exc) = parse_extension_filters(".tar.gz,.test.js,.d.ts");
    assert!(inc.contains(".tar.gz"));
    assert!(inc.contains(".test.js"));
    assert!(inc.contains(".d.ts"));

    let include: HashSet<String> = inc;
    let exclude_exts: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &ex_dirs, &ex_files);

    // Should include the multi-dot files
    assert!(tree.children.iter().any(|n| n.name == "archive.tar.gz"));
    assert!(tree.children.iter().any(|n| n.name == "app.test.js"));
    assert!(tree.children.iter().any(|n| n.name == "types.d.ts"));

    // Should not include other multi-dot files
    assert!(!tree.children.iter().any(|n| n.name == "bundle.tar.bz2"));
    assert!(!tree.children.iter().any(|n| n.name == "config.test.yml"));
}

#[test]
fn mixed_filters_work() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create mixed files
    fs::write(root.join("main.rs"), "x").unwrap();
    fs::write(root.join("justfile"), "x").unwrap();
    fs::write(root.join("archive.tar.gz"), "x").unwrap();
    fs::write(root.join("README.md"), "x").unwrap();
    fs::write(root.join("Makefile"), "x").unwrap();

    // Test mixed filter: single extension, extensionless, multi-dot
    let (inc, _exc) = parse_extension_filters(".rs,justfile,.tar.gz");
    assert!(inc.contains(".rs"));
    assert!(inc.contains(".justfile"));
    assert!(inc.contains(".tar.gz"));

    let include: HashSet<String> = inc;
    let exclude_exts: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &ex_dirs, &ex_files);

    // Should include files matching any of the filters
    assert!(tree.children.iter().any(|n| n.name == "main.rs"));
    assert!(tree.children.iter().any(|n| n.name == "justfile"));
    assert!(tree.children.iter().any(|n| n.name == "archive.tar.gz"));

    // Should not include other files
    assert!(!tree.children.iter().any(|n| n.name == "README.md"));
    assert!(!tree.children.iter().any(|n| n.name == "Makefile"));
}

#[test]
fn exclude_extensionless_filenames_work() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create extensionless files
    fs::write(root.join("justfile"), "x").unwrap();
    fs::write(root.join("Makefile"), "x").unwrap();
    fs::write(root.join("Dockerfile"), "x").unwrap();
    fs::write(root.join("README"), "x").unwrap();

    // Test excluding extensionless files
    let (_inc, exc) = parse_extension_filters("-justfile,-Makefile");
    assert!(exc.contains(".justfile"));
    assert!(exc.contains(".makefile"));

    let include: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = exc;
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &ex_dirs, &ex_files);

    // Should exclude the specified extensionless files
    assert!(!tree.children.iter().any(|n| n.name == "justfile"));
    assert!(!tree.children.iter().any(|n| n.name == "Makefile"));

    // Should include other extensionless files
    assert!(tree.children.iter().any(|n| n.name == "Dockerfile"));
    assert!(tree.children.iter().any(|n| n.name == "README"));
}

#[test]
fn exclude_multidot_extensions_work() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create multi-dot files
    fs::write(root.join("archive.tar.gz"), "x").unwrap();
    fs::write(root.join("bundle.tar.bz2"), "x").unwrap();
    fs::write(root.join("app.test.js"), "x").unwrap();
    fs::write(root.join("types.d.ts"), "x").unwrap();

    // Test excluding multi-dot extensions
    let (_inc, exc) = parse_extension_filters("-.tar.gz,-.test.js");
    assert!(exc.contains(".tar.gz"));
    assert!(exc.contains(".test.js"));

    let include: HashSet<String> = HashSet::new();
    let exclude_exts: HashSet<String> = exc;
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &ex_dirs, &ex_files);

    // Should exclude the specified multi-dot files
    assert!(!tree.children.iter().any(|n| n.name == "archive.tar.gz"));
    assert!(!tree.children.iter().any(|n| n.name == "app.test.js"));

    // Should include other multi-dot files
    assert!(tree.children.iter().any(|n| n.name == "bundle.tar.bz2"));
    assert!(tree.children.iter().any(|n| n.name == "types.d.ts"));
}

#[test]
fn case_insensitive_matching_works() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create files with different cases
    fs::write(root.join("Justfile"), "x").unwrap();
    fs::write(root.join("MAKEFILE"), "x").unwrap();
    fs::write(root.join("Archive.TAR.GZ"), "x").unwrap();

    // Test case-insensitive matching
    let (inc, _exc) = parse_extension_filters("justfile,Makefile,.tar.gz");
    assert!(inc.contains(".justfile"));
    assert!(inc.contains(".makefile"));
    assert!(inc.contains(".tar.gz"));

    let include: HashSet<String> = inc;
    let exclude_exts: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &ex_dirs, &ex_files);

    // Should match despite case differences
    assert!(tree.children.iter().any(|n| n.name == "Justfile"));
    assert!(tree.children.iter().any(|n| n.name == "MAKEFILE"));
    assert!(tree.children.iter().any(|n| n.name == "Archive.TAR.GZ"));
}

#[test]
fn backward_compatibility_single_extensions_still_work() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create single extension files
    fs::write(root.join("main.rs"), "x").unwrap();
    fs::write(root.join("script.py"), "x").unwrap();
    fs::write(root.join("readme.txt"), "x").unwrap();

    // Test that single extensions still work as before
    let (inc, _exc) = parse_extension_filters(".rs,.py");
    assert!(inc.contains(".rs"));
    assert!(inc.contains(".py"));

    let include: HashSet<String> = inc;
    let exclude_exts: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &include, &exclude_exts, &ex_dirs, &ex_files);

    // Should include single extension files
    assert!(tree.children.iter().any(|n| n.name == "main.rs"));
    assert!(tree.children.iter().any(|n| n.name == "script.py"));

    // Should not include other single extension files
    assert!(!tree.children.iter().any(|n| n.name == "readme.txt"));
}
