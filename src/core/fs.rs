use crate::core::Node;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

/* =========================== Filesystem & paths ============================ */

#[must_use] 
pub fn path_to_unix(p: &Path) -> String {
    let mut s = String::new();
    for (i, comp) in p.iter().enumerate() {
        if i > 0 {
            s.push('/');
        }
        s.push_str(&comp.to_string_lossy());
    }
    s
}

#[must_use] 
pub fn is_ancestor_of(ancestor: &Path, p: &Path) -> bool {
    let anc = normalize_path(ancestor);
    let pp = normalize_path(p);
    pp.starts_with(&anc)
}

#[must_use] 
pub fn normalize_path(p: &Path) -> PathBuf {
    if let Ok(c) = dunce::canonicalize(p) {
        return c;
    }

    let cwd = std::env::current_dir()
        .ok()
        .and_then(|cd| dunce::canonicalize(cd).ok())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    };

    let mut cur = abs.as_path();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    while !cur.exists() {
        match (cur.parent(), cur.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name.to_os_string());
                cur = parent;
            }
            _ => break,
        }
    }

    let mut base = if cur.exists() {
        dunce::canonicalize(cur).unwrap_or_else(|_| cur.to_path_buf())
    } else {
        abs.clone()
    };

    for c in tail.iter().rev() {
        base.push(c);
    }

    use std::path::Component;
    let mut cleaned = PathBuf::new();
    for comp in base.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = cleaned.pop();
            }
            _ => cleaned.push(comp.as_os_str()),
        }
    }
    cleaned
}

#[must_use] 
pub fn scan_dir_to_node(
    dir: &Path,
    include_exts: &HashSet<String>,
    exclude_exts: &HashSet<String>,
    exclude_dirs: &HashSet<String>,
    exclude_files: &HashSet<String>,
) -> Node {
    #[inline]
    fn dot_lower_last_ext(p: &Path) -> String {
        match p.extension() {
            Some(os) => {
                if let Some(s) = os.to_str() {
                    let mut out = String::with_capacity(s.len() + 1);
                    out.push('.');
                    for b in s.bytes() {
                        let lb = if b.is_ascii_uppercase() { b + 32 } else { b };
                        out.push(lb as char);
                    }
                    out
                } else {
                    let lossy = os.to_string_lossy();
                    let lower = lossy.to_lowercase();
                    let mut out = String::with_capacity(lower.len() + 1);
                    out.push('.');
                    out.push_str(&lower);
                    out
                }
            }
            None => String::new(),
        }
    }

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

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return node,
    };

    let mut dirs: Vec<(String, PathBuf)> = Vec::new();
    let mut files: Vec<(String, PathBuf)> = Vec::new();

    let include_mode = !include_exts.is_empty();
    let exclude_mode = !exclude_exts.is_empty();

    for ent in entries.flatten() {
        let path = ent.path();
        let base: String = ent.file_name().to_string_lossy().into_owned();

        let is_dir = ent.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        if is_dir {
            if exclude_dirs.contains(&base) {
                continue;
            }
            dirs.push((base, path));
            continue;
        }

        if exclude_files.contains(&base) {
            continue;
        }

        let matches_file = if include_mode || exclude_mode {
            let ext = dot_lower_last_ext(&path);
            if include_mode {
                include_exts.contains(&ext)
            } else {
                !exclude_exts.contains(&ext)
            }
        } else {
            true
        };

        if matches_file {
            files.push((base, path));
        }
    }

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

    for (_basename, path) in dirs {
        let child = scan_dir_to_node(
            &path,
            include_exts,
            exclude_exts,
            exclude_dirs,
            exclude_files,
        );

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

    node
}

#[must_use] 
pub fn gather_paths_set(root: &Node) -> HashSet<PathBuf> {
    let mut set = HashSet::new();
    fn rec(n: &Node, set: &mut HashSet<PathBuf>) {
        set.insert(n.path.clone());
        for c in &n.children {
            rec(c, set);
        }
    }
    rec(root, &mut set);
    set
}

#[must_use] 
pub const fn dir_contains_file(node: &Node) -> bool {
    !node.is_dir || node.has_children
}

pub fn collect_selected_paths(
    node: &Node,
    explicit: &HashMap<PathBuf, bool>,
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
pub fn is_event_path_relevant(
    project_root: &std::path::Path,
    abs_path: &std::path::Path,
    include_exts: &std::collections::HashSet<String>,
    exclude_exts: &std::collections::HashSet<String>,
    exclude_dirs: &std::collections::HashSet<String>,
    exclude_files: &std::collections::HashSet<String>,
) -> bool {
    use std::path::{Component, Path};

    #[inline]
    fn dot_lower_last_ext(p: &Path) -> String {
        match p.extension() {
            Some(os) => {
                if let Some(s) = os.to_str() {
                    let mut out = String::with_capacity(s.len() + 1);
                    out.push('.');
                    for b in s.bytes() {
                        let lb = if b.is_ascii_uppercase() { b + 32 } else { b };
                        out.push(lb as char);
                    }
                    out
                } else {
                    let lossy = os.to_string_lossy();
                    let lower = lossy.to_lowercase();
                    let mut out = String::with_capacity(lower.len() + 1);
                    out.push('.');
                    out.push_str(&lower);
                    out
                }
            }
            None => String::new(),
        }
    }

    if !abs_path.starts_with(project_root) {
        return false;
    }
    let rel = match abs_path.strip_prefix(project_root) {
        Ok(r) => r,
        Err(_) => return false,
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
    let ext = dot_lower_last_ext(abs_path);

    if include_mode {
        // Only consider files that match an included extension.
        if ext.is_empty() {
            return false;
        }
        return include_exts.contains(&ext);
    }

    if exclude_mode {
        // Ignore files that match an excluded extension.
        if !ext.is_empty() && exclude_exts.contains(&ext) {
            return false;
        }
        return true;
    }

    // No ext filters -> relevant (given it passed dir/file name filters).
    true
}
