#![allow(clippy::needless_return)]

use regex::Regex;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

/// UI-free representation of a filesystem node.
#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: Vec<Node>,
    pub expanded: bool,
    pub has_children: bool,
}

/* =========================== Parsing & Text utils =========================== */

pub fn parse_hierarchy_text(text: &str) -> Option<HashSet<String>> {
    let mut lines = text.lines();
    let _root = lines.next()?;

    let mut paths: HashSet<String> = HashSet::new();
    let mut parts: Vec<String> = Vec::new();

    for raw in lines {
        let line = raw.trim_end();
        if line.is_empty() {
            continue;
        }

        let mut name_char_idx: Option<usize> = None;
        for (i, ch) in line.chars().enumerate() {
            if ch != '│' && ch != '└' && ch != '├' && ch != '─' && !ch.is_whitespace() {
                name_char_idx = Some(i);
                break;
            }
        }
        let name_char_idx = match name_char_idx {
            Some(i) => i,
            None => continue,
        };

        let level = if name_char_idx > 0 {
            (name_char_idx.saturating_sub(1)) / 4
        } else {
            0
        };

        let byte_start = line
            .char_indices()
            .nth(name_char_idx)
            .map(|(b, _)| b)
            .unwrap_or(0);
        let name = line[byte_start..].trim();
        if name.is_empty() {
            continue;
        }

        if parts.len() > level {
            parts.truncate(level);
        }
        parts.push(name.to_string());

        let rel = parts.join("/");
        paths.insert(rel);
    }

    Some(paths)
}

pub fn split_prefix_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn render_unicode_tree_from_paths(paths: &[String], root_name: Option<&str>) -> String {
    #[derive(Default)]
    struct T {
        children: BTreeMap<String, Box<T>>,
    }
    fn insert_path(root: &mut T, parts: &[&str]) {
        if parts.is_empty() {
            return;
        }
        let head = parts[0].to_string();
        let entry = root.children.entry(head).or_default();
        if parts.len() > 1 {
            insert_path(entry, &parts[1..]);
        }
    }
    fn render(node: &T, prefix: &str, out: &mut String) {
        let len = node.children.len();
        for (idx, (name, child)) in node.children.iter().enumerate() {
            let last = idx + 1 == len;
            out.push_str(prefix);
            out.push_str(if last { "└── " } else { "├── " });
            out.push_str(name);
            out.push('\n');

            if !child.children.is_empty() {
                let child_prefix = format!("{}{}", prefix, if last { "    " } else { "│   " });
                render(child, &child_prefix, out);
            }
        }
    }

    let mut root = T::default();
    for p in paths {
        let parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
        insert_path(&mut root, &parts);
    }

    let mut out = String::new();
    if let Some(name) = root_name {
        out.push_str(name);
        out.push('\n');
    }
    render(&root, "", &mut out);
    out
}

pub fn strip_lines_and_inline_comments(contents: &str, prefixes: &[String]) -> String {
    if prefixes.is_empty() {
        return contents.to_string();
    }

    let mut out = String::with_capacity(contents.len());

    'line: for line in contents.lines() {
        // 1) Full-line comments: remove if the first non-ws starts with any prefix.
        let first_non_ws = line
            .char_indices()
            .find(|&(_, ch)| !ch.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or_else(|| line.len());

        if prefixes
            .iter()
            .any(|p| !p.is_empty() && line[first_non_ws..].starts_with(p))
        {
            continue 'line;
        }

        // 2) Inline comments: scan while being quote-aware.
        #[derive(Copy, Clone, Debug, PartialEq, Eq)]
        enum State {
            Normal,
            Dq { escaped: bool },
            Sq { escaped: bool },
            Raw { hashes: usize },
            TripleDq,
            TripleSq,
        }

        let mut state = State::Normal;
        let mut cut_at: Option<usize> = None;

        let mut pos = 0usize;
        let len = line.len();
        let bytes = line.as_bytes();

        // Track the previous *character* for the whitespace check.
        let mut prev_char: Option<char> = None;

        while pos < len {
            // helpers
            let slice = &line[pos..];
            let ch = slice.chars().next().unwrap();
            let ch_w = ch.len_utf8();

            match state {
                State::Normal => {
                    // Detect triple quotes first (language-agnostic Python-like).
                    if slice.starts_with("\"\"\"") {
                        state = State::TripleDq;
                        pos += 3;
                        prev_char = Some('"');
                        continue;
                    } else if slice.starts_with("'''") {
                        state = State::TripleSq;
                        pos += 3;
                        prev_char = Some('\'');
                        continue;
                    }

                    // Detect Rust-style raw string start: r###" ... "###
                    if ch == 'r' {
                        let mut j = pos + ch_w; // after 'r'
                        let mut hashes = 0usize;
                        while j < len && bytes[j] == b'#' {
                            hashes += 1;
                            j += 1;
                        }
                        if j < len && bytes[j] == b'"' {
                            state = State::Raw { hashes };
                            pos = j + 1; // skip opening quote too
                            prev_char = Some('"');
                            continue;
                        }
                    }

                    // Regular quote starts
                    if ch == '"' {
                        state = State::Dq { escaped: false };
                        pos += ch_w;
                        prev_char = Some(ch);
                        continue;
                    } else if ch == '\'' {
                        state = State::Sq { escaped: false };
                        pos += ch_w;
                        prev_char = Some(ch);
                        continue;
                    }

                    // Only consider prefixes when not inside any quotes and past leading ws
                    if pos >= first_non_ws {
                        for p in prefixes {
                            if p.is_empty() {
                                continue;
                            }
                            if slice.starts_with(p) {
                                // Require whitespace immediately before the prefix
                                if prev_char.map(|c| c.is_whitespace()).unwrap_or(false) {
                                    cut_at = Some(pos);
                                    break;
                                }
                            }
                        }
                        if cut_at.is_some() {
                            break;
                        }
                    }

                    pos += ch_w;
                    prev_char = Some(ch);
                }

                State::Dq { mut escaped } => {
                    if !escaped && ch == '"' {
                        state = State::Normal;
                    }
                    escaped = ch == '\\' && !escaped;
                    // update state with new escape flag
                    if let State::Dq { .. } = state {
                        state = State::Dq { escaped };
                    }
                    pos += ch_w;
                    prev_char = Some(ch);
                }

                State::Sq { mut escaped } => {
                    if !escaped && ch == '\'' {
                        state = State::Normal;
                    }
                    escaped = ch == '\\' && !escaped;
                    if let State::Sq { .. } = state {
                        state = State::Sq { escaped };
                    }
                    pos += ch_w;
                    prev_char = Some(ch);
                }

                State::Raw { hashes } => {
                    // End when we see '"' followed by exactly `hashes` '#' chars
                    if bytes[pos] == b'"' {
                        let j = pos + 1;
                        let end = j + hashes;
                        if end <= len && bytes[j..end].iter().all(|&b| b == b'#') {
                            state = State::Normal;
                            pos = end;
                            prev_char = Some('"');
                            continue;
                        }
                    }
                    pos += ch_w;
                    prev_char = Some(ch);
                }

                State::TripleDq => {
                    if slice.starts_with("\"\"\"") {
                        state = State::Normal;
                        pos += 3;
                        prev_char = Some('"');
                        continue;
                    }
                    pos += ch_w;
                    prev_char = Some(ch);
                }

                State::TripleSq => {
                    if slice.starts_with("'''") {
                        state = State::Normal;
                        pos += 3;
                        prev_char = Some('\'');
                        continue;
                    }
                    pos += ch_w;
                    prev_char = Some(ch);
                }
            }
        }

        let kept = if let Some(cut) = cut_at {
            let left = &line[..cut];
            left.trim_end_matches([' ', '\t']).to_string()
        } else {
            line.to_string()
        };

        out.push_str(&kept);
        out.push('\n');
    }

    out
}

pub fn compile_remove_regex_opt(raw: Option<&str>) -> Option<Regex> {
    raw.and_then(|s| {
        let pattern = format!("(?ms){}", s);
        Regex::new(&pattern).ok()
    })
}

pub fn clean_remove_regex(s: &str) -> String {
    let mut t = s.trim().to_string();
    let triple_dq = t.starts_with("\"\"\"") && t.ends_with("\"\"\"");
    let triple_sq = t.starts_with("'''") && t.ends_with("'''");
    let dq = t.starts_with('"') && t.ends_with('"');
    let sq = t.starts_with('\'') && t.ends_with('\'');

    if triple_dq || triple_sq {
        t = t[3..t.len() - 3].to_string();
    } else if dq || sq {
        t = t[1..t.len() - 1].to_string();
    }
    t
}

pub fn parse_extension_filters(
    raw: &str,
) -> (
    std::collections::HashSet<String>,
    std::collections::HashSet<String>,
) {
    use std::collections::HashSet;

    let mut include_exts = HashSet::new();
    let mut exclude_exts = HashSet::new();

    for token in raw.split(',') {
        let tok = token.trim();
        if tok.is_empty() {
            continue;
        }

        let (is_exclude, rest) = if let Some(stripped) = tok.strip_prefix('-') {
            (true, stripped.trim())
        } else {
            (false, tok)
        };

        // Allow tokens with or without leading dot(s). Ignore if they’re only dot(s) or empty.
        let stripped = rest.trim_start_matches('.');
        if stripped.is_empty() {
            continue; // skip ".", "-.", "...", etc.
        }

        let norm = format!(".{}", stripped.to_lowercase());

        if is_exclude {
            exclude_exts.insert(norm);
        } else {
            include_exts.insert(norm);
        }
    }

    (include_exts, exclude_exts)
}

pub fn collapse_consecutive_blank_lines(s: &str) -> String {
    let mut out_lines = Vec::new();
    let mut prev_blank = false;

    for line in s.lines() {
        let is_blank = line.trim().is_empty();
        if is_blank && prev_blank {
            continue;
        }
        out_lines.push(line);
        prev_blank = is_blank;
    }

    let mut normalized = out_lines.join("\n");
    if s.ends_with('\n') {
        normalized.push('\n');
    }
    normalized
}

/* =========================== Filesystem & paths ============================ */

pub fn path_to_unix(p: &Path) -> String {
    p.iter()
        .map(|c| c.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn is_ancestor_of(ancestor: &Path, p: &Path) -> bool {
    let anc = normalize_path(ancestor);
    let pp = normalize_path(p);
    pp.starts_with(&anc)
}

pub fn normalize_path(p: &Path) -> PathBuf {
    // 1) Fast path: fully canonicalizable
    if let Ok(c) = dunce::canonicalize(p) {
        return c;
    }

    // 2) Build an absolute version to anchor comparisons to the same base dir
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|cd| dunce::canonicalize(cd).ok())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    };

    // 3) Peel off tail components until we reach an existing ancestor
    let mut cur = abs.as_path();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    while !cur.exists() {
        match (cur.parent(), cur.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name.to_os_string());
                cur = parent;
            }
            _ => break, // hit root or nothing exists
        }
    }

    // 4) Canonicalize the existing base to resolve symlinks (/var ↔ /private/var)
    let mut base = if cur.exists() {
        dunce::canonicalize(cur).unwrap_or_else(|_| cur.to_path_buf())
    } else {
        // For absolute paths, this should rarely happen; fall back to abs
        abs.clone()
    };

    // 5) Reattach the (non-existent) tail in the right order
    for c in tail.iter().rev() {
        base.push(c);
    }

    // 6) Lexically normalize '.' and '..'
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

pub fn scan_dir_to_node(
    dir: &Path,
    include_exts: &HashSet<String>,
    exclude_exts: &HashSet<String>,
    exclude_dirs: &HashSet<String>,
    exclude_files: &HashSet<String>,
) -> Node {
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

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for ent in entries.flatten() {
        let path = ent.path();
        let base = ent.file_name().to_string_lossy().to_string();

        if ent.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            if exclude_dirs.contains(&base) {
                continue;
            }
            dirs.push(path);
        } else {
            if exclude_files.contains(&base) {
                continue;
            }
            let ext = path
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
                .unwrap_or_default();

            let matches_file = if !include_exts.is_empty() {
                include_exts.contains(&ext)
            } else if !exclude_exts.is_empty() {
                !exclude_exts.contains(&ext)
            } else {
                true
            };

            if matches_file {
                files.push(path);
            }
        }
    }

    dirs.sort();
    files.sort();

    for f in files {
        node.has_children = true;
        node.children.push(Node {
            name: f
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            path: f,
            is_dir: false,
            children: Vec::new(),
            expanded: false,
            has_children: false,
        });
    }

    for d in dirs {
        let child = scan_dir_to_node(&d, include_exts, exclude_exts, exclude_dirs, exclude_files);

        let child_visible = if !include_exts.is_empty() {
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

pub fn dir_contains_file(node: &Node) -> bool {
    if !node.is_dir {
        return true;
    }
    for c in &node.children {
        if !c.is_dir {
            return true;
        }
        if dir_contains_file(c) {
            return true;
        }
    }
    false
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
        if my_effective && dir_contains_file(node) {
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
