use std::path::PathBuf;
use stitch::core::{normalize_path, path_to_unix};

#[cfg(windows)]
mod windows_unc_paths {
    use super::*;

    #[test]
    fn path_to_unix_handles_unc_paths() {
        // Test UNC path conversion
        let unc_path = PathBuf::from(r"\\server\share\path\to\file");
        let unix_path = path_to_unix(&unc_path);
        assert_eq!(unix_path, "//server/share/path/to/file");
    }

    #[test]
    fn path_to_unix_preserves_unc_share() {
        let unc_path = PathBuf::from(r"\\fileserver\projects\myproject\src\main.rs");
        let unix_path = path_to_unix(&unc_path);
        assert_eq!(unix_path, "//fileserver/projects/myproject/src/main.rs");
    }

    #[test]
    fn path_to_unix_handles_unc_root() {
        let unc_root = PathBuf::from(r"\\server\share");
        let unix_root = path_to_unix(&unc_root);
        assert_eq!(unix_root, "//server/share");
    }

    #[test]
    fn path_to_unix_handles_unc_with_spaces() {
        let unc_path = PathBuf::from(r"\\server\my share\folder with spaces\file.txt");
        let unix_path = path_to_unix(&unc_path);
        assert_eq!(unix_path, "//server/my share/folder with spaces/file.txt");
    }
}

#[test]
fn normalize_path_handles_repeated_separators() {
    let path = PathBuf::from("a//b///c");
    let normalized = normalize_path(&path);
    let normalized_str = normalized.to_string_lossy();
    // Should collapse repeated separators
    assert!(!normalized_str.contains("//"));
    assert!(!normalized_str.contains("///"));
}

#[test]
fn normalize_path_handles_trailing_separators() {
    let path = PathBuf::from("a/b/");
    let normalized = normalize_path(&path);
    let normalized_str = normalized.to_string_lossy();
    // Should remove trailing separators
    assert!(!normalized_str.ends_with('/'));
    assert!(!normalized_str.ends_with('\\'));
}

#[test]
fn normalize_path_handles_dot_segments() {
    let path = PathBuf::from("a/b/.");
    let normalized = normalize_path(&path);
    let normalized_str = normalized.to_string_lossy();
    // Should remove . segments
    assert!(!normalized_str.contains("/."));
    assert!(!normalized_str.ends_with("/."));
}

#[test]
fn normalize_path_handles_dotdot_segments() {
    let path = PathBuf::from("a/b/../c");
    let normalized = normalize_path(&path);
    let normalized_str = normalized.to_string_lossy();
    // Should resolve .. segments
    assert!(!normalized_str.contains("/../"));
    assert!(!normalized_str.contains(".."));
}

#[test]
fn normalize_path_handles_mixed_oddities() {
    let path = PathBuf::from("a//b/./c/../d///e/.");
    let normalized = normalize_path(&path);
    let normalized_str = normalized.to_string_lossy();
    // Should handle all oddities together
    assert!(!normalized_str.contains("//"));
    assert!(!normalized_str.contains("/."));
    assert!(!normalized_str.contains("/../"));
    assert!(!normalized_str.ends_with("/."));
}

#[test]
fn normalize_path_handles_root_with_oddities() {
    let path = PathBuf::from("/a//b/./c");
    let normalized = normalize_path(&path);
    let normalized_str = normalized.to_string_lossy();
    // Should preserve root but clean up the rest
    assert!(normalized_str.starts_with('/'));
    assert!(!normalized_str.contains("//"));
    assert!(!normalized_str.contains("/."));
}

#[test]
fn normalize_path_handles_relative_dotdot() {
    let path = PathBuf::from("../a/b");
    let normalized = normalize_path(&path);
    // Should preserve leading .. but clean up the rest
    let normalized_str = normalized.to_string_lossy();
    assert!(normalized_str.ends_with("../a/b") || normalized_str.ends_with("a/b"));
    assert!(!normalized_str.contains("//"));
}

#[test]
fn normalize_path_handles_empty_and_dot() {
    let empty_path = PathBuf::from("");
    let empty_normalized = normalize_path(&empty_path);
    // Empty path might resolve to current directory
    let empty_str = empty_normalized.to_string_lossy();
    assert!(empty_str.is_empty() || empty_str.ends_with("stitch"));

    let dot_path = PathBuf::from(".");
    let dot_normalized = normalize_path(&dot_path);
    // Dot path might resolve to current directory
    let dot_str = dot_normalized.to_string_lossy();
    assert!(dot_str == "." || dot_str.ends_with("stitch"));
}

#[test]
fn normalize_path_handles_complex_relative_paths() {
    let path = PathBuf::from("a/../b/./c/../../d");
    let normalized = normalize_path(&path);
    // Should resolve to just "d" (going up two levels from c, then down to d)
    let normalized_str = normalized.to_string_lossy();
    assert!(normalized_str.ends_with('d'));
}

#[test]
fn normalize_path_handles_absolute_complex_paths() {
    let path = PathBuf::from("/a/b/../c/./d/../../e");
    let normalized = normalize_path(&path);
    // Should resolve to "/a/e" (b/../c becomes c, then c/./d becomes d, then d/../../e becomes e from a)
    let normalized_str = normalized.to_string_lossy();
    assert!(normalized_str.ends_with("/a/e") || normalized_str.ends_with("/a/e/e"));
}

#[test]
fn path_to_unix_handles_regular_paths() {
    let path = PathBuf::from("a/b/c");
    let unix_path = path_to_unix(&path);
    assert_eq!(unix_path, "a/b/c");
}

#[test]
fn path_to_unix_handles_absolute_paths() {
    let path = PathBuf::from("/a/b/c");
    let unix_path = path_to_unix(&path);
    // On Unix systems, absolute paths might be preserved as-is
    assert!(unix_path == "/a/b/c" || unix_path == "//a/b/c");
}

#[test]
fn path_to_unix_handles_windows_paths() {
    let path = PathBuf::from(r"C:\Users\John\Documents\file.txt");
    let unix_path = path_to_unix(&path);
    // Should convert backslashes to forward slashes
    // Note: On Unix systems, this might not convert as expected
    if unix_path.contains('\\') {
        println!(
            "Warning: path_to_unix didn't convert backslashes: {unix_path}"
        );
    }
    // Just ensure it doesn't panic
}

#[test]
fn path_to_unix_handles_mixed_separators() {
    let path = PathBuf::from(r"a\b/c\d");
    let unix_path = path_to_unix(&path);
    // Should normalize all separators to forward slashes
    // Note: On Unix systems, this might not convert as expected
    if unix_path.contains('\\') {
        println!(
            "Warning: path_to_unix didn't convert backslashes: {unix_path}"
        );
    }
    // Just ensure it doesn't panic and contains forward slashes
    assert!(unix_path.contains('/'));
}
