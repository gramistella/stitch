use regex::Regex;
use std::collections::{BTreeMap, HashSet};

/* =========================== Parsing & Text utils =========================== */

#[must_use] 
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

#[must_use] 
pub fn split_prefix_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[must_use] 
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

#[must_use] 
pub fn strip_lines_and_inline_comments(contents: &str, prefixes: &[String]) -> String {
    if prefixes.is_empty() {
        return contents.to_string();
    }

    // Group prefixes by their first byte for a quick first-char dispatch.
    let mut by_first: [Vec<&[u8]>; 256] = std::array::from_fn(|_| Vec::new());
    for p in prefixes.iter().filter(|p| !p.is_empty()) {
        let b = p.as_bytes();
        by_first[*b.first().unwrap_or(&0) as usize].push(b);
    }

    let mut out = String::with_capacity(contents.len());

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    enum State {
        Normal,
        Dq { escaped: bool },  // "..."
        Sq { escaped: bool },  // '...'
        Raw { hashes: usize }, // r#"..."#  (hashes = number of #)
        TripleDq,              // """..."""
        TripleSq,              // '''...'''
    }
    let mut state = State::Normal;

    'line: for line in contents.lines() {
        // Treat single/double quoted strings as single-line: clear them on newline.
        if matches!(state, State::Dq { .. } | State::Sq { .. }) {
            state = State::Normal;
        }

        let bytes = line.as_bytes();
        let len = bytes.len();

        // First non-whitespace (unicode-aware) index, as byte offset.
        let first_non_ws = line
            .char_indices()
            .find(|&(_, ch)| !ch.is_whitespace())
            .map_or(len, |(i, _)| i);

        // Full-line comments: allowed unless we're inside a *true* multi-line construct.
        let in_true_multiline =
            matches!(state, State::Raw { .. } | State::TripleDq | State::TripleSq);

        if !in_true_multiline && first_non_ws < len {
            let bucket = &by_first[bytes[first_non_ws] as usize];
            for p in bucket {
                if bytes[first_non_ws..].starts_with(p) {
                    // Drop the whole line.
                    continue 'line;
                }
            }
        }

        let mut cut_at: Option<usize> = None;

        // Track whether previous char (in Normal state) was whitespace.
        let mut prev_was_ws = false;

        // Iterate characters with their starting byte indices.
        let mut iter = line.char_indices().peekable();
        while let Some((pos, ch)) = iter.next() {
            let next_pos = iter.peek().map_or(len, |(i, _)| *i);
            let slice = &bytes[pos..];

            match state {
                State::Normal => {
                    // Triple-quote openers first (so we don't mis-handle """ in Normal).
                    if slice.starts_with(b"\"\"\"") {
                        state = State::TripleDq;
                        // Skip the next 2 bytes; we've already consumed one char this loop.
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

                    // Raw string start: r#"..."# / r##"..."## / etc.
                    if ch == 'r' {
                        let mut j = next_pos;
                        let mut hashes = 0usize;
                        while j < len && bytes[j] == b'#' {
                            hashes += 1;
                            let _ = iter.next_if(|(idx, _)| *idx == j);
                            j += 1;
                        }
                        if j < len && bytes[j] == b'"' {
                            let _ = iter.next_if(|(idx, _)| *idx == j);
                            state = State::Raw { hashes };
                            prev_was_ws = false;
                            continue;
                        }
                    }

                    // Single / double quoted strings (single-line in our lexer).
                    if ch == '"' {
                        state = State::Dq { escaped: false };
                        prev_was_ws = false;
                        continue;
                    } else if ch == '\'' {
                        state = State::Sq { escaped: false };
                        prev_was_ws = false;
                        continue;
                    }

                    // Inline comment prefixes: only if preceded by whitespace and after leading ws.
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
                    // Raw strings close with a '"' followed by exactly `hashes` #'s.
                    if bytes[pos] == b'"' {
                        let end = pos + 1 + hashes;
                        if end <= len && bytes[pos + 1..end].iter().all(|&b| b == b'#') {
                            state = State::Normal;
                            // Consume to `end`.
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

        if let Some(mut end) = cut_at {
            // Trim trailing spaces before the inline comment marker.
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

#[must_use] 
pub fn compile_remove_regex_opt(raw: Option<&str>) -> Option<Regex> {
    raw.and_then(|s| {
        let pattern = format!("(?ms){s}");
        Regex::new(&pattern).ok()
    })
}

#[must_use] 
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

#[must_use] 
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

        let stripped = rest.trim_start_matches('.');
        if stripped.is_empty() {
            continue;
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

#[must_use] 
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
        out.pop();
    }
    out
}
