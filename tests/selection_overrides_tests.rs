use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use stitch::core::*;
use tempfile::TempDir;

fn mkfile(p: &std::path::Path) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, "x").unwrap();
}

#[test]
fn explicit_false_on_dir_blocks_descendants_even_if_parent_selected() {
    // root/
    //   keep/
    //     a.txt
    //   drop/
    //     b.txt
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    mkfile(&root.join("keep/a.txt"));
    mkfile(&root.join("drop/b.txt"));

    let inc: HashSet<String> = HashSet::new();
    let exc: HashSet<String> = HashSet::new();
    let ex_dirs: HashSet<String> = HashSet::new();
    let ex_files: HashSet<String> = HashSet::new();

    let tree = scan_dir_to_node(root, &inc, &exc, &ex_dirs, &ex_files);

    // Select the root dir (inherit everything), but explicitly uncheck `drop/`.
    let mut explicit: HashMap<PathBuf, bool> = HashMap::new();
    explicit.insert(root.to_path_buf(), true);
    explicit.insert(root.join("drop"), false);

    let mut files = Vec::new();
    let mut dirs = Vec::new();
    collect_selected_paths(&tree, &explicit, None, &mut files, &mut dirs);

    let files_unix: Vec<_> = files.iter().map(|p| path_to_unix(p)).collect();
    assert!(
        files_unix.iter().any(|s| s.ends_with("keep/a.txt")),
        "file under kept dir should be included"
    );
    assert!(
        !files_unix.iter().any(|s| s.ends_with("drop/b.txt")),
        "descendants of explicitly-false dir must be excluded"
    );

    let dirs_unix: Vec<_> = dirs.iter().map(|p| path_to_unix(p)).collect();
    assert!(
        dirs_unix.iter().any(|s| s.ends_with("keep")),
        "non-empty kept dir should appear in dirs list when dir selection is used"
    );
    assert!(
        !dirs_unix.iter().any(|s| s.ends_with("drop")),
        "explicitly-false dir should not appear"
    );
}
