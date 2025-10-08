use crate::core::Node;
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs,
    path::{Component, Path, PathBuf},
};

type NamePath = (String, PathBuf);

/* =========================== Filesystem & paths ============================ */

#[must_use]
pub fn path_to_unix(p: &Path) -> String {
    let mut s = String::new();
    let mut first = true;

    for comp in p {
        if !first {
            s.push('/');
        }
        first = false;

        let comp_str = comp.to_string_lossy();

        // Handle UNC paths on Windows
        #[cfg(windows)]
        if comp_str == "\\" && s.is_empty() {
            // This is the root of a UNC path, skip it as we'll handle it specially
            continue;
        }

        s.push_str(&comp_str);
    }

    // Handle UNC paths on Windows - convert \\server\share to //server/share
    #[cfg(windows)]
    if let Some(path_str) = p.to_str() {
        if path_str.starts_with(r"\\") && !path_str.starts_with(r"\\?") {
            // This is a UNC path, convert it properly
            let unix_unc = path_str.replace('\\', "/");
            return unix_unc;
        }
    }

    s
}

#[must_use]
pub fn is_ancestor_of(ancestor: &Path, p: &Path) -> bool {
    // Try to canonicalize both paths first
    let anc_canon = dunce::canonicalize(ancestor).unwrap_or_else(|_| normalize_path(ancestor));
    let pp_canon = dunce::canonicalize(p).unwrap_or_else(|_| {
        // If we can't canonicalize the full path, try to canonicalize the parent
        if let Some(parent) = p.parent()
            && let Ok(parent_canon) = dunce::canonicalize(parent)
            && let Some(file_name) = p.file_name()
        {
            return parent_canon.join(file_name);
        }
        normalize_path(p)
    });

    pp_canon.starts_with(&anc_canon)
}

#[must_use]
pub fn normalize_path(p: &Path) -> PathBuf {
    if p.as_os_str().is_empty() {
        return PathBuf::new();
    }

    if let Ok(c) = dunce::canonicalize(p) {
        return c;
    }

    let mut prefix: Option<OsString> = None;
    let mut has_root = false;
    let mut parts: Vec<OsString> = Vec::new();

    for comp in p.components() {
        match comp {
            Component::Prefix(pref) => {
                prefix = Some(pref.as_os_str().to_os_string());
            }
            Component::RootDir => {
                has_root = true;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if let Some(last) = parts.last() {
                    if last == ".." {
                        parts.push(OsString::from(".."));
                    } else {
                        let _ = parts.pop();
                    }
                } else if !has_root && prefix.is_none() {
                    parts.push(OsString::from(".."));
                }
            }
            Component::Normal(name) => {
                parts.push(name.to_os_string());
            }
        }
    }

    let mut result = String::new();

    if let Some(pref) = prefix {
        let pref_str = pref.to_string_lossy().replace('\\', "/");
        result.push_str(&pref_str);
        if has_root && !pref_str.ends_with('/') {
            result.push('/');
        }
    } else if has_root {
        result.push('/');
    }

    for (idx, part) in parts.iter().enumerate() {
        let needs_sep = !(result.is_empty()
            || result.ends_with('/')
            || result.ends_with(':')
            || (result.starts_with("//") && idx == 0));
        if needs_sep {
            result.push('/');
        }
        if result.ends_with(':') {
            result.push('/');
        }
        result.push_str(&part.to_string_lossy().replace('\\', "/"));
    }

    if result.is_empty() {
        PathBuf::new()
    } else {
        PathBuf::from(result)
    }
}

/// Check if a path matches any extension in the given filter set.
/// Supports three types of matching:
/// 1. Full filename (for extensionless files like "justfile")
/// 2. Multi-dot extensions (for files like "file.tar.gz")
/// 3. Single extensions (for files like "file.rs")
fn path_matches_extension_filters<S: ::std::hash::BuildHasher>(
    p: &Path,
    filters: &HashSet<String, S>,
) -> bool {
    let filename = p.file_name().and_then(|name| name.to_str()).unwrap_or("");

    if filename.is_empty() {
        return false;
    }

    // 1. Try full filename as extension (for extensionless files like "justfile")
    let full_filename_ext = format!(".{}", filename.to_lowercase());
    if filters.contains(&full_filename_ext) {
        return true;
    }

    // 2. Try multi-dot extensions (for files like "file.tar.gz")
    let filename_lower = filename.to_lowercase();
    let mut dot_pos = 0;
    while let Some(pos) = filename_lower[dot_pos..].find('.') {
        let actual_pos = dot_pos + pos;
        let multi_ext = &filename_lower[actual_pos..];
        if filters.contains(multi_ext) {
            return true;
        }
        dot_pos = actual_pos + 1;
    }

    // 3. Try single extension (current behavior for files like "file.rs")
    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
        let single_ext = format!(".{}", ext.to_lowercase());
        if filters.contains(&single_ext) {
            return true;
        }
    }

    false
}

#[derive(Default, Debug)]
pub struct ScanStats {
    pub excluded_dirs_found: HashSet<String>,
    pub excluded_files_found: HashSet<String>,
}

#[derive(Debug)]
pub struct ScanResult {
    pub node: Node,
    pub stats: ScanStats,
}

#[must_use]
pub fn scan_dir_to_node<S: ::std::hash::BuildHasher>(
    dir: &Path,
    include_exts: &HashSet<String, S>,
    exclude_exts: &HashSet<String, S>,
    exclude_dirs: &HashSet<String, S>,
    exclude_files: &HashSet<String, S>,
) -> Node {
    scan_dir_to_node_internal(dir, include_exts, exclude_exts, exclude_dirs, exclude_files).node
}

pub fn scan_dir_to_node_with_stats<S: ::std::hash::BuildHasher>(
    dir: &Path,
    include_exts: &HashSet<String, S>,
    exclude_exts: &HashSet<String, S>,
    exclude_dirs: &HashSet<String, S>,
    exclude_files: &HashSet<String, S>,
) -> ScanResult {
    scan_dir_to_node_internal(dir, include_exts, exclude_exts, exclude_dirs, exclude_files)
}

fn scan_dir_to_node_internal<S: ::std::hash::BuildHasher>(
    dir: &Path,
    include_exts: &HashSet<String, S>,
    exclude_exts: &HashSet<String, S>,
    exclude_dirs: &HashSet<String, S>,
    exclude_files: &HashSet<String, S>,
) -> ScanResult {
    let name = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut node = Node {
        name,
        path: dir.to_path_buf(),
        is_dir: true,
        children: Vec::new(),
        expanded: true,
        has_children: false,
    };

    let (mut files, mut dirs, mut stats) =
        gather_dir_entries(dir, include_exts, exclude_exts, exclude_dirs, exclude_files);

    files.sort_by(|a, b| a.0.cmp(&b.0));
    dirs.sort_by(|a, b| a.0.cmp(&b.0));

    node.children.reserve(files.len() + dirs.len());

    for (basename, path) in files {
        node.has_children = true;
        node.children.push(Node {
            name: basename,
            path,
            is_dir: false,
            children: Vec::new(),
            expanded: false,
            has_children: false,
        });
    }

    let include_mode = !include_exts.is_empty();
    for (_basename, path) in dirs {
        let ScanResult {
            node: child,
            stats: mut child_stats,
        } = scan_dir_to_node_internal(
            &path,
            include_exts,
            exclude_exts,
            exclude_dirs,
            exclude_files,
        );

        stats
            .excluded_dirs_found
            .extend(child_stats.excluded_dirs_found.drain());
        stats
            .excluded_files_found
            .extend(child_stats.excluded_files_found.drain());

        let child_visible = if include_mode {
            !child.children.is_empty() || child.has_children
        } else {
            true
        };

        if child_visible {
            node.has_children =
                node.has_children || !child.children.is_empty() || child.has_children;
            node.children.push(child);
        }
    }

    ScanResult { node, stats }
}

fn gather_dir_entries<S: ::std::hash::BuildHasher>(
    dir: &Path,
    include_exts: &HashSet<String, S>,
    exclude_exts: &HashSet<String, S>,
    exclude_dirs: &HashSet<String, S>,
    exclude_files: &HashSet<String, S>,
) -> (Vec<NamePath>, Vec<NamePath>, ScanStats) {
    let Ok(entries) = fs::read_dir(dir) else {
        return (Vec::new(), Vec::new(), ScanStats::default());
    };

    let mut dirs: Vec<NamePath> = Vec::new();
    let mut files: Vec<NamePath> = Vec::new();
    let mut stats = ScanStats::default();

    let include_mode = !include_exts.is_empty();
    let exclude_mode = !exclude_exts.is_empty();

    for ent in entries.flatten() {
        let path = ent.path();
        let base: String = ent.file_name().to_string_lossy().into_owned();

        let is_dir = ent.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        if is_dir {
            if exclude_dirs.contains(&base) {
                stats.excluded_dirs_found.insert(base);
                continue;
            }
            dirs.push((base, path));
            continue;
        }

        if exclude_files.contains(&base) {
            stats.excluded_files_found.insert(base);
            continue;
        }

        let matches_file = if include_mode || exclude_mode {
            if include_mode {
                path_matches_extension_filters(&path, include_exts)
            } else {
                !path_matches_extension_filters(&path, exclude_exts)
            }
        } else {
            true
        };

        if matches_file {
            files.push((base, path));
        }
    }

    (files, dirs, stats)
}

#[must_use]
pub fn gather_paths_set(root: &Node) -> HashSet<PathBuf> {
    let mut set = HashSet::new();
    gather_paths_set_rec(root, &mut set);
    set
}

fn gather_paths_set_rec(n: &Node, set: &mut HashSet<PathBuf>) {
    set.insert(n.path.clone());
    for c in &n.children {
        gather_paths_set_rec(c, set);
    }
}

#[must_use]
pub const fn dir_contains_file(node: &Node) -> bool {
    !node.is_dir || node.has_children
}

pub fn collect_selected_paths<T: ::std::hash::BuildHasher>(
    node: &Node,
    explicit: &HashMap<PathBuf, bool, T>,
    inherited: Option<bool>,
    files_out: &mut Vec<PathBuf>,
    dirs_out: &mut Vec<PathBuf>,
) {
    let my_effective = explicit
        .get(&node.path)
        .copied()
        .or(inherited)
        .unwrap_or(false);

    if node.is_dir {
        if my_effective && node.has_children {
            dirs_out.push(node.path.clone());
        }
        let next_inherited = my_effective;
        for c in &node.children {
            collect_selected_paths(c, explicit, Some(next_inherited), files_out, dirs_out);
        }
    } else if my_effective {
        files_out.push(node.path.clone());
    }
}

#[must_use]
pub fn drain_channel_nonblocking<T>(rx: &std::sync::mpsc::Receiver<T>) -> bool {
    let mut any = false;
    while rx.try_recv().is_ok() {
        any = true;
    }
    any
}

#[must_use]
pub fn is_event_path_relevant<S: ::std::hash::BuildHasher>(
    project_root: &std::path::Path,
    abs_path: &std::path::Path,
    include_exts: &std::collections::HashSet<String, S>,
    exclude_exts: &std::collections::HashSet<String, S>,
    exclude_dirs: &std::collections::HashSet<String, S>,
    exclude_files: &std::collections::HashSet<String, S>,
) -> bool {
    if !abs_path.starts_with(project_root) {
        return false;
    }
    let Ok(rel) = abs_path.strip_prefix(project_root) else {
        return false;
    };

    // Root itself: always relevant (forces one rescan if the root flips metadata)
    if rel.as_os_str().is_empty() {
        return true;
    }

    // If any component matches an excluded directory name, ignore.
    for comp in rel.components() {
        if let Component::Normal(os) = comp {
            let name = os.to_string_lossy();
            if exclude_dirs.contains(name.as_ref()) {
                return false;
            }
        }
    }

    // If the basename is excluded, ignore.
    if let Some(fname) = rel.file_name() {
        let fname_s = fname.to_string_lossy().to_string();
        if exclude_files.contains(&fname_s) {
            return false;
        }
    }

    // Apply extension rules (primarily for files; directories usually have no ext).
    let include_mode = !include_exts.is_empty();
    let exclude_mode = !exclude_exts.is_empty();

    if include_mode {
        // Only consider files that match an included extension.
        return path_matches_extension_filters(abs_path, include_exts);
    }

    if exclude_mode {
        // Ignore files that match an excluded extension.
        if path_matches_extension_filters(abs_path, exclude_exts) {
            return false;
        }
        return true;
    }

    // No ext filters -> relevant (given it passed dir/file name filters).
    true
}
