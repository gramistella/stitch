use regex::Regex;
use std::collections::{BTreeMap, HashSet};

// ============================== Unicode glyphs ===============================
// Box-drawing characters used for tree parsing/rendering
pub const GLYPH_VERT: char = '│';
pub const GLYPH_END: char = '└';
pub const GLYPH_TEE: char = '├';
pub const GLYPH_HORI: char = '─';
pub const GLYPH_BRANCH_END: &str = "└── ";
pub const GLYPH_BRANCH_TEE: &str = "├── ";
pub const GLYPH_VERT_PREFIX: &str = "│   ";
pub const GLYPH_INDENT: &str = "    ";

// Common UI glyphs shared by multiple components
pub const GLYPH_BULLET: &str = "•";
pub const GLYPH_ELLIPSIS: &str = "…";
pub const GLYPH_HOURGLASS: &str = "⏳";

// No encoding repair needed for hierarchy or file contents when read correctly as UTF-8.
#[inline]
fn normalize_mojibake_tree_input(input: &str) -> String {
    input.to_string()
}

#[must_use]
pub fn repair_mojibake_if_present(input: &str) -> String {
    input.to_string()
}

/* =========================== Parsing & Text utils =========================== */

#[must_use]
pub fn parse_hierarchy_text(text: &str) -> Option<HashSet<String>> {
    let normalized_input = normalize_mojibake_tree_input(text);
    let mut lines = normalized_input.lines();
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
            if ch != GLYPH_VERT
                && ch != GLYPH_END
                && ch != GLYPH_TEE
                && ch != GLYPH_HORI
                && !ch.is_whitespace()
            {
                name_char_idx = Some(i);
                name_byte_idx = byte_pos;
                break;
            }
            byte_pos += ch.len_utf8();
        }
        let Some(name_char_idx) = name_char_idx else {
            continue;
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
            out.push_str(if last {
                GLYPH_BRANCH_END
            } else {
                GLYPH_BRANCH_TEE
            });
            out.push_str(name);
            out.push('\n');

            if !child.children.is_empty() {
                let saved = prefix.len();
                prefix.push_str(if last {
                    GLYPH_INDENT
                } else {
                    GLYPH_VERT_PREFIX
                });
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum StripState {
    Normal,
    Dq { escaped: bool },
    Sq { escaped: bool },
    Raw { hashes: usize },
    TripleDq,
    TripleSq,
}

struct PrefixBuckets<'a> {
    buckets: [Vec<&'a [u8]>; 256],
}

impl<'a> PrefixBuckets<'a> {
    fn new(prefixes: &'a [String]) -> Self {
        let mut buckets: [Vec<&'a [u8]>; 256] = std::array::from_fn(|_| Vec::new());
        for prefix in prefixes.iter().filter(|p| !p.is_empty()) {
            let bytes = prefix.as_bytes();
            let first = bytes[0] as usize;
            buckets[first].push(bytes);
        }
        Self { buckets }
    }

    fn matches(&self, bytes: &[u8], start: usize) -> bool {
        let Some(&head) = bytes.get(start) else {
            return false;
        };
        self.buckets[head as usize]
            .iter()
            .any(|candidate| bytes[start..].starts_with(candidate))
    }
}

struct CommentStripper<'a> {
    state: StripState,
    buckets: PrefixBuckets<'a>,
}

impl<'a> CommentStripper<'a> {
    fn new(prefixes: &'a [String]) -> Self {
        Self {
            state: StripState::Normal,
            buckets: PrefixBuckets::new(prefixes),
        }
    }

    fn strip(mut self, contents: &str) -> String {
        let mut out = String::with_capacity(contents.len());
        for line in contents.lines() {
            self.process_line(line, &mut out);
        }
        out
    }

    fn process_line(&mut self, line: &str, out: &mut String) {
        if matches!(self.state, StripState::Dq { .. } | StripState::Sq { .. }) {
            self.state = StripState::Normal;
        }
        let bytes = line.as_bytes();
        let first_non_ws = first_non_whitespace(line, bytes.len());

        if self.should_strip_full_line(bytes, first_non_ws) {
            return;
        }

        if let Some(cut) = self.scan_inline_cut(line, first_non_ws) {
            push_trimmed_prefix(out, line, cut);
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }

    fn should_strip_full_line(&self, bytes: &[u8], first_non_ws: usize) -> bool {
        if first_non_ws >= bytes.len() {
            return false;
        }
        if matches!(
            self.state,
            StripState::Raw { .. } | StripState::TripleDq | StripState::TripleSq
        ) {
            return false;
        }
        self.buckets.matches(bytes, first_non_ws)
    }

    fn scan_inline_cut(&mut self, line: &str, first_non_ws: usize) -> Option<usize> {
        let bytes = line.as_bytes();
        let chars: Vec<(usize, char)> = line.char_indices().collect();
        let total_chars = chars.len();
        let mut prev_was_ws = false;
        let mut idx = 0usize;

        while idx < total_chars {
            let (pos, ch) = chars[idx];
            let slice = &bytes[pos..];

            match self.state {
                StripState::Normal => {
                    if self.try_open_triple(slice, &mut idx, StripState::TripleDq, total_chars) {
                        prev_was_ws = false;
                        continue;
                    }
                    if self.try_open_triple(slice, &mut idx, StripState::TripleSq, total_chars) {
                        prev_was_ws = false;
                        continue;
                    }
                    if ch == 'r' && self.try_open_raw(bytes, pos, &mut idx, total_chars) {
                        prev_was_ws = false;
                        continue;
                    }
                    if ch == '"' {
                        self.state = StripState::Dq { escaped: false };
                        prev_was_ws = false;
                        idx += 1;
                        continue;
                    }
                    if ch == '\'' {
                        self.state = StripState::Sq { escaped: false };
                        prev_was_ws = false;
                        idx += 1;
                        continue;
                    }
                    if pos >= first_non_ws && prev_was_ws && self.buckets.matches(bytes, pos) {
                        return Some(pos);
                    }
                    prev_was_ws = ch.is_whitespace();
                    idx += 1;
                }
                StripState::Dq { escaped } => {
                    self.state = if !escaped && ch == '"' {
                        StripState::Normal
                    } else {
                        let next_escaped = ch == '\\' && !escaped;
                        StripState::Dq {
                            escaped: next_escaped,
                        }
                    };
                    prev_was_ws = false;
                    idx += 1;
                }
                StripState::Sq { escaped } => {
                    self.state = if !escaped && ch == '\'' {
                        StripState::Normal
                    } else {
                        let next_escaped = ch == '\\' && !escaped;
                        StripState::Sq {
                            escaped: next_escaped,
                        }
                    };
                    prev_was_ws = false;
                    idx += 1;
                }
                StripState::Raw { hashes } => {
                    if bytes[pos] == b'"'
                        && Self::consume_raw_closer(bytes, pos, &mut idx, hashes, total_chars)
                    {
                        self.state = StripState::Normal;
                        prev_was_ws = false;
                        continue;
                    }
                    prev_was_ws = false;
                    idx += 1;
                }
                StripState::TripleDq => {
                    if Self::try_close_triple(slice, &mut idx, total_chars, StripState::TripleDq) {
                        self.state = StripState::Normal;
                        prev_was_ws = false;
                        continue;
                    }
                    prev_was_ws = false;
                    idx += 1;
                }
                StripState::TripleSq => {
                    if Self::try_close_triple(slice, &mut idx, total_chars, StripState::TripleSq) {
                        self.state = StripState::Normal;
                        prev_was_ws = false;
                        continue;
                    }
                    prev_was_ws = false;
                    idx += 1;
                }
            }
        }
        None
    }

    fn try_open_triple(
        &mut self,
        slice: &[u8],
        idx: &mut usize,
        target: StripState,
        total: usize,
    ) -> bool {
        let needle = match target {
            StripState::TripleDq => b"\"\"\"",
            StripState::TripleSq => b"'''",
            _ => return false,
        };
        if slice.starts_with(needle) {
            self.state = target;
            *idx = (*idx + 3).min(total);
            return true;
        }
        false
    }

    fn try_close_triple(slice: &[u8], idx: &mut usize, total: usize, target: StripState) -> bool {
        let needle = match target {
            StripState::TripleDq => b"\"\"\"",
            StripState::TripleSq => b"'''",
            _ => return false,
        };
        if slice.starts_with(needle) {
            *idx = (*idx + 3).min(total);
            return true;
        }
        false
    }

    fn try_open_raw(
        &mut self,
        bytes: &[u8],
        pos: usize,
        idx: &mut usize,
        total_chars: usize,
    ) -> bool {
        let mut probe = pos + 1;
        let mut hashes = 0usize;
        while probe < bytes.len() && bytes[probe] == b'#' {
            hashes += 1;
            probe += 1;
        }
        if probe < bytes.len() && bytes[probe] == b'"' {
            self.state = StripState::Raw { hashes };
            *idx = (*idx + hashes + 2).min(total_chars);
            return true;
        }
        false
    }

    fn consume_raw_closer(
        bytes: &[u8],
        pos: usize,
        idx: &mut usize,
        hashes: usize,
        total_chars: usize,
    ) -> bool {
        let end = pos + 1 + hashes;
        if end > bytes.len() {
            return false;
        }
        if bytes[pos + 1..end].iter().all(|&b| b == b'#') {
            *idx = (*idx + hashes + 1).min(total_chars);
            return true;
        }
        false
    }
}

fn first_non_whitespace(line: &str, default: usize) -> usize {
    line.char_indices()
        .find(|&(_, ch)| !ch.is_whitespace())
        .map_or(default, |(i, _)| i)
}

fn push_trimmed_prefix(out: &mut String, line: &str, mut end: usize) {
    let bytes = line.as_bytes();
    while end > 0 {
        let b = bytes[end - 1];
        if b == b' ' || b == b'\t' {
            end -= 1;
        } else {
            break;
        }
    }
    out.push_str(&line[..end]);
}

#[must_use]
pub fn strip_lines_and_inline_comments(contents: &str, prefixes: &[String]) -> String {
    if prefixes.is_empty() {
        return contents.to_string();
    }
    CommentStripper::new(prefixes).strip(contents)
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
    let triple_double_quote = t.starts_with("\"\"\"") && t.ends_with("\"\"\"");
    let triple_single_quote = t.starts_with("'''") && t.ends_with("'''");
    let dq = t.starts_with('"') && t.ends_with('"');
    let sq = t.starts_with('\'') && t.ends_with('\'');

    if triple_double_quote || triple_single_quote {
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

        let (is_exclude, rest) = tok
            .strip_prefix('-')
            .map_or((false, tok), |stripped| (true, stripped.trim()));

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
