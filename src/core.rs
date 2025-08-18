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

        // Find name start once, tracking both char index and byte index.
        let mut name_char_idx: Option<usize> = None;
        let mut name_byte_idx: usize = 0;
        let mut byte_pos: usize = 0;

        for (i, ch) in line.chars().enumerate() {
            if ch != '│' && ch != '└' && ch != '├' && ch != '─' && !ch.is_whitespace() {
                name_char_idx = Some(i);
                name_byte_idx = byte_pos;
                break;
            }
            byte_pos += ch.len_utf8();
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

        let name = line[name_byte_idx..].trim();
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
        let entry = root.children.entry(parts[0].to_string()).or_default();
        if parts.len() > 1 {
            insert_path(entry, &parts[1..]);
        }
    }
    fn render(node: &T, prefix: &mut String, out: &mut String) {
        let len = node.children.len();
        for (idx, (name, child)) in node.children.iter().enumerate() {
            let last = idx + 1 == len;
            out.push_str(prefix);
            out.push_str(if last { "└── " } else { "├── " });
            out.push_str(name);
            out.push('\n');

            if !child.children.is_empty() {
                let saved = prefix.len();
                prefix.push_str(if last { "    " } else { "│   " });
                render(child, prefix, out);
                prefix.truncate(saved);
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
    let mut prefix = String::new();
    render(&root, &mut prefix, &mut out);
    out
}

pub fn strip_lines_and_inline_comments(contents: &str, prefixes: &[String]) -> String {
    if prefixes.is_empty() {
        return contents.to_string();
    }

    // Precompute non-empty prefixes as byte slices and group by first byte for O(1) dispatch.
    let mut by_first: [Vec<&[u8]>; 256] = std::array::from_fn(|_| Vec::new());
    for p in prefixes.iter().filter(|p| !p.is_empty()) {
        let b = p.as_bytes();
        by_first[*b.first().unwrap_or(&0) as usize].push(b);
    }

    let mut out = String::with_capacity(contents.len());

    // ===== NEW: carry string/comment state across lines =====
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

    'line: for line in contents.lines() {
        let bytes = line.as_bytes();
        let len = bytes.len();

        // First non-whitespace (unicode-aware) index, as byte offset.
        let first_non_ws = line
            .char_indices()
            .find(|&(_, ch)| !ch.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or(len);

        // Full-line comments: only when we're NOT inside a multi-line construct.
        if matches!(state, State::Normal) && first_non_ws < len {
            let bucket = &by_first[bytes[first_non_ws] as usize];
            for p in bucket {
                if bytes[first_non_ws..].starts_with(p) {
                    continue 'line; // drop whole line
                }
            }
        }

        let mut cut_at: Option<usize> = None;

        // Track whether previous char (in Normal state) was whitespace.
        let mut prev_was_ws = false;

        // Single pass over char indices; use next byte index to avoid len_utf8.
        let mut iter = line.char_indices().peekable();
        while let Some((pos, ch)) = iter.next() {
            let next_pos = iter.peek().map(|(i, _)| *i).unwrap_or(len);
            let slice = &bytes[pos..];

            match state {
                State::Normal => {
                    // Triple-quote openers first.
                    if slice.starts_with(b"\"\"\"") {
                        state = State::TripleDq;
                        // fast-skip 2 more bytes (we've already consumed one char this loop)
                        for _ in 0..2 {
                            iter.next();
                        }
                        prev_was_ws = false;
                        continue;
                    } else if slice.starts_with(b"'''") {
                        state = State::TripleSq;
                        for _ in 0..2 {
                            iter.next();
                        }
                        prev_was_ws = false;
                        continue;
                    }

                    // Raw string opener: r####"
                    if ch == 'r' {
                        // Count '#' after 'r'
                        let mut j = next_pos; // byte index after 'r'
                        let mut hashes = 0usize;
                        while j < len && bytes[j] == b'#' {
                            hashes += 1;
                            // advance iterator to align with 'j'
                            let _ = iter.next_if(|(idx, _)| *idx == j);
                            j += 1;
                        }
                        if j < len && bytes[j] == b'"' {
                            // consume the '"' (advance iterator to j)
                            let _ = iter.next_if(|(idx, _)| *idx == j);
                            state = State::Raw { hashes };
                            prev_was_ws = false;
                            continue;
                        }
                    }

                    // Normal string openers
                    if ch == '"' {
                        state = State::Dq { escaped: false };
                        prev_was_ws = false;
                        continue;
                    } else if ch == '\'' {
                        state = State::Sq { escaped: false };
                        prev_was_ws = false;
                        continue;
                    }

                    // Inline comment detection (only in Normal):
                    //  - only after the first non-ws
                    //  - require immediate whitespace before prefix
                    if pos >= first_non_ws && prev_was_ws {
                        let b0 = bytes[pos];
                        let bucket = &by_first[b0 as usize];
                        if !bucket.is_empty() {
                            for p in bucket {
                                if slice.starts_with(p) {
                                    cut_at = Some(pos);
                                    break;
                                }
                            }
                            if cut_at.is_some() {
                                break;
                            }
                        }
                    }

                    prev_was_ws = ch.is_whitespace();
                }

                State::Dq { mut escaped } => {
                    if !escaped && ch == '"' {
                        state = State::Normal;
                    }
                    escaped = ch == '\\' && !escaped;
                    if let State::Dq { .. } = state {
                        state = State::Dq { escaped };
                    }
                    prev_was_ws = false;
                }

                State::Sq { mut escaped } => {
                    if !escaped && ch == '\'' {
                        state = State::Normal;
                    }
                    escaped = ch == '\\' && !escaped;
                    if let State::Sq { .. } = state {
                        state = State::Sq { escaped };
                    }
                    prev_was_ws = false;
                }

                State::Raw { hashes } => {
                    // Close when we see '"' followed by exactly `hashes` '#'
                    if bytes[pos] == b'"' {
                        let end = pos + 1 + hashes;
                        if end <= len && bytes[pos + 1..end].iter().all(|&b| b == b'#') {
                            state = State::Normal;
                            // advance iterator to end-1 (end is next start)
                            while iter.peek().is_some_and(|(i, _)| *i < end) {
                                iter.next();
                            }
                            prev_was_ws = false;
                            continue;
                        }
                    }
                    prev_was_ws = false;
                }

                State::TripleDq => {
                    if slice.starts_with(b"\"\"\"") {
                        state = State::Normal;
                        for _ in 0..2 {
                            iter.next();
                        }
                        prev_was_ws = false;
                        continue;
                    }
                    prev_was_ws = false;
                }

                State::TripleSq => {
                    if slice.starts_with(b"'''") {
                        state = State::Normal;
                        for _ in 0..2 {
                            iter.next();
                        }
                        prev_was_ws = false;
                        continue;
                    }
                    prev_was_ws = false;
                }
            }
        }

        // Emit the line (possibly trimmed up to cut_at), trimming only ASCII space/tab before the prefix.
        if let Some(mut end) = cut_at {
            while end > 0 {
                let b = bytes[end - 1];
                if b == b' ' || b == b'\t' {
                    end -= 1;
                } else {
                    break;
                }
            }
            out.push_str(&line[..end]);
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
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
    let mut out = String::with_capacity(s.len());
    let mut prev_blank = false;
    for line in s.lines() {
        let is_blank = line.trim().is_empty();
        if is_blank && prev_blank {
            continue;
        }
        out.push_str(line);
        out.push('\n');
        prev_blank = is_blank;
    }
    if !s.ends_with('\n') && out.ends_with('\n') {
        out.pop(); // remove trailing newline if input had none
    }
    out
}

/* =========================== Filesystem & paths ============================ */

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
    #[inline]
    fn dot_lower_last_ext(p: &Path) -> String {
        match p.extension() {
            Some(os) => {
                if let Some(s) = os.to_str() {
                    // ASCII-fast path, avoids Unicode-lowering allocs for common cases.
                    let mut out = String::with_capacity(s.len() + 1);
                    out.push('.');
                    for b in s.bytes() {
                        let lb = if b.is_ascii_uppercase() { b + 32 } else { b };
                        out.push(lb as char);
                    }
                    out
                } else {
                    // Fallback for non-UTF8: use lossy + lowercase to keep semantics.
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

    // Keep basenames with paths so we don't recompute names later.
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

        // Only compute/normalize extension when we actually need it.
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

    // Per-directory deterministic ordering: files by name, then dirs by name.
    files.sort_by(|a, b| a.0.cmp(&b.0));
    dirs.sort_by(|a, b| a.0.cmp(&b.0));

    node.children.reserve(files.len() + dirs.len());

    // Emit files first.
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

    // Then recurse into directories.
    for (_basename, path) in dirs {
        let child = scan_dir_to_node(
            &path,
            include_exts,
            exclude_exts,
            exclude_dirs,
            exclude_files,
        );

        // Hide empty dirs when include-mode is active (preserves existing behavior).
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
    // Fast path using the flag computed in `scan_dir_to_node`.
    !node.is_dir || node.has_children
}

// src/core.rs

pub fn collect_selected_paths(
    node: &Node,
    explicit: &HashMap<PathBuf, bool>,
    inherited: Option<bool>,
    files_out: &mut Vec<PathBuf>,
    dirs_out: &mut Vec<PathBuf>,
) {
    // Effective selection at this node (explicit overrides inherited).
    let my_effective = explicit
        .get(&node.path)
        .copied()
        .or(inherited)
        .unwrap_or(false);

    if node.is_dir {
        // Use the precomputed flag instead of rescanning the subtree.
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

pub fn drain_channel_nonblocking<T>(rx: &std::sync::mpsc::Receiver<T>) -> bool {
    let mut any = false;
    while rx.try_recv().is_ok() {
        any = true;
    }
    any
}
