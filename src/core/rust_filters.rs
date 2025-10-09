// `syn` no longer used in this module

// Helpers for scanning string literals in a byte buffer
fn scan_string_literal(bytes: &[u8], len_bytes: usize, mut cursor: usize, quote: u8) -> usize {
    let mut is_escaped = false;
    cursor += 1;
    while cursor < len_bytes {
        let current = bytes[cursor];
        cursor += 1;
        if !is_escaped && current == quote {
            break;
        }
        is_escaped = current == b'\\' && !is_escaped;
    }
    cursor
}

fn scan_raw_string_literal(bytes: &[u8], len_bytes: usize, mut cursor: usize) -> usize {
    let mut lookahead = cursor + 1;
    let mut num_hashes = 0usize;
    while lookahead < len_bytes && bytes[lookahead] == b'#' {
        num_hashes += 1;
        lookahead += 1;
    }
    if lookahead < len_bytes && bytes[lookahead] == b'"' {
        cursor = lookahead + 1;
    } else {
        return cursor + 1;
    }
    while cursor < len_bytes {
        if bytes[cursor] == b'"' {
            let mut tmp_index = cursor + 1;
            let mut matched = 0usize;
            while matched < num_hashes && tmp_index < len_bytes && bytes[tmp_index] == b'#' {
                tmp_index += 1;
                matched += 1;
            }
            if matched == num_hashes {
                return tmp_index;
            }
        }
        cursor += 1;
    }
    cursor
}

// Helper shared by comment stripping logic
fn trim_trailing_ws_current_line(buf: &mut String) {
    let mut idx = buf.len();
    while idx > 0 {
        let b = buf.as_bytes()[idx - 1];
        if b == b' ' || b == b'\t' {
            idx -= 1;
            continue;
        }
        if b == b'\n' {
            break;
        }
        break;
    }
    if idx < buf.len() {
        buf.truncate(idx);
    }
}
// no token printing — we preserve original formatting; only function bodies are replaced

#[derive(Debug, Clone, Default)]
pub struct RustFilterOptions {
    pub remove_inline_regular_comments: bool,
    pub remove_doc_comments: bool,
    pub function_signatures_only: bool,
}

/// Returns true if the given path ends with ".rs" (case-sensitive like Rust filenames on most systems).
#[must_use]
pub fn is_rust_file_path(path: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("rs")
}

/// Apply Rust-specific filters to a source file's string contents.
/// This function only transforms when at least one option is enabled; otherwise returns input as-is.
#[must_use]
pub fn apply_rust_filters(source: &str, opts: &RustFilterOptions) -> String {
    if !(opts.remove_inline_regular_comments
        || opts.remove_doc_comments
        || opts.function_signatures_only)
    {
        return source.to_string();
    }

    apply_rust_filters_inner(source, opts)
}

fn apply_rust_filters_inner(source: &str, opts: &RustFilterOptions) -> String {
    if opts.function_signatures_only {
        let maybe_cleaned = if opts.remove_inline_regular_comments || opts.remove_doc_comments {
            Some(remove_comments_textual(
                source,
                opts.remove_inline_regular_comments,
                opts.remove_doc_comments,
            ))
        } else {
            None
        };
        let base: std::borrow::Cow<str> = maybe_cleaned
            .map_or(std::borrow::Cow::Borrowed(source), |s| {
                std::borrow::Cow::Owned(s)
            });
        let transformed = transform_functions_to_signatures(&base);
        let collapsed = crate::core::collapse_consecutive_blank_lines(&transformed);
        return trim_leading_blank_lines(&collapsed);
    }
    // Comment removal only
    let cleaned = remove_comments_textual(
        source,
        opts.remove_inline_regular_comments,
        opts.remove_doc_comments,
    );
    let collapsed = crate::core::collapse_consecutive_blank_lines(&cleaned);
    trim_leading_blank_lines(&collapsed)
}

#[derive(Copy, Clone)]
struct SignatureBounds {
    sig_end: usize,
    body_start: usize,
}

struct SignatureReducer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    len: usize,
    index: usize,
    last_emit: usize,
    output: String,
}

impl<'a> SignatureReducer<'a> {
    fn new(src: &'a str) -> Self {
        let bytes = src.as_bytes();
        Self {
            src,
            bytes,
            len: bytes.len(),
            index: 0,
            last_emit: 0,
            output: String::with_capacity(bytes.len()),
        }
    }

    fn run(mut self) -> String {
        while self.index < self.len {
            self.handle_code_state();
        }
        self.flush_tail();
        self.output
    }

    fn handle_code_state(&mut self) {
        if self.try_skip_comment() || self.try_skip_string() || self.try_handle_function() {
            return;
        }
        self.index += 1;
    }

    fn try_skip_comment(&mut self) -> bool {
        if self.index + 1 >= self.len || self.bytes[self.index] != b'/' {
            return false;
        }
        match self.bytes[self.index + 1] {
            b'/' => {
                self.index = skip_line_comment(self.bytes, self.len, self.index + 2);
                true
            }
            b'*' => {
                self.index = skip_block_comment(self.bytes, self.len, self.index + 2, 1);
                true
            }
            _ => false,
        }
    }

    fn try_skip_string(&mut self) -> bool {
        match self.bytes[self.index] {
            b'"' => {
                self.index = scan_string_literal(self.bytes, self.len, self.index, b'"');
                true
            }
            b'\'' => {
                if is_probable_lifetime(self.bytes, self.len, self.index) {
                    self.index = skip_lifetime(self.bytes, self.len, self.index);
                } else {
                    self.index = scan_string_literal(self.bytes, self.len, self.index, b'\'');
                }
                true
            }
            b'r' => {
                let next = scan_raw_string_literal(self.bytes, self.len, self.index);
                self.index = next.max(self.index + 1);
                true
            }
            _ => false,
        }
    }

    fn try_handle_function(&mut self) -> bool {
        if !self.starts_function_keyword() {
            return false;
        }
        if let Some(bounds) = locate_body_start(self.bytes, self.len, self.index) {
            self.emit_signature(bounds);
        } else {
            self.index += 2;
        }
        true
    }

    fn emit_signature(&mut self, bounds: SignatureBounds) {
        self.output
            .push_str(&self.src[self.last_emit..bounds.sig_end]);
        self.output.push_str(";\n");
        let body_end = skip_function_body(self.bytes, self.len, bounds.body_start + 1);
        // Avoid creating an extra blank line: if the original source has trailing
        // spaces/tabs and a newline immediately after the function body's closing
        // brace, skip them because we already emitted a newline above.
        let mut resume = body_end;
        while resume < self.len && (self.bytes[resume] == b' ' || self.bytes[resume] == b'\t') {
            resume += 1;
        }
        // Skip CRLF (\r\n) if present, otherwise skip a single LF.
        if resume + 1 < self.len && self.bytes[resume] == b'\r' && self.bytes[resume + 1] == b'\n' {
            resume += 2;
        } else if resume < self.len && self.bytes[resume] == b'\n' {
            resume += 1;
        }
        self.last_emit = resume;
        self.index = resume;
    }

    fn flush_tail(&mut self) {
        let tail = &self.src[self.last_emit..];
        if !tail.trim().is_empty() {
            self.output.push_str(tail);
        }
    }

    fn starts_function_keyword(&self) -> bool {
        if self.index + 1 >= self.len {
            return false;
        }
        if self.bytes[self.index] != b'f' || self.bytes[self.index + 1] != b'n' {
            return false;
        }
        // Do not treat raw identifiers like `r#fn` as the `fn` keyword.
        // If the bytes immediately before "fn" are "r#", this is a raw identifier.
        if self.index >= 2
            && self.bytes[self.index - 1] == b'#'
            && self.bytes[self.index - 2] == b'r'
        {
            return false;
        }
        let prev_ok = self.index == 0 || !is_ident_byte(self.bytes[self.index - 1]);
        let next_ok = self.index + 2 >= self.len || !is_ident_byte(self.bytes[self.index + 2]);
        prev_ok && next_ok
    }
}

fn transform_functions_to_signatures(src: &str) -> String {
    SignatureReducer::new(src).run()
}

fn skip_line_comment(bytes: &[u8], len: usize, mut idx: usize) -> usize {
    while idx < len && bytes[idx] != b'\n' {
        idx += 1;
    }
    if idx < len {
        idx += 1;
    }
    idx
}

fn skip_block_comment(bytes: &[u8], len: usize, mut idx: usize, mut depth: usize) -> usize {
    while idx + 1 < len && depth > 0 {
        if bytes[idx] == b'/' && bytes[idx + 1] == b'*' {
            depth += 1;
            idx += 2;
            continue;
        }
        if bytes[idx] == b'*' && bytes[idx + 1] == b'/' {
            depth = depth.saturating_sub(1);
            idx += 2;
            if depth == 0 {
                break;
            }
            continue;
        }
        idx += 1;
    }
    idx.min(len)
}

fn locate_body_start(bytes: &[u8], len: usize, fn_start: usize) -> Option<SignatureBounds> {
    let mut idx = fn_start + 2;
    let mut paren_depth = 0i32;
    while idx < len {
        if idx + 1 < len && bytes[idx] == b'/' {
            if bytes[idx + 1] == b'/' {
                idx = skip_line_comment(bytes, len, idx + 2);
                continue;
            }
            if bytes[idx + 1] == b'*' {
                idx = skip_block_comment(bytes, len, idx + 2, 1);
                continue;
            }
        }
        match bytes[idx] {
            b'"' => {
                idx = scan_string_literal(bytes, len, idx, b'"');
                continue;
            }
            b'\'' => {
                if is_probable_lifetime(bytes, len, idx) {
                    idx = skip_lifetime(bytes, len, idx);
                    continue;
                }
                idx = scan_string_literal(bytes, len, idx, b'\'');
                continue;
            }
            b'r' => {
                idx = scan_raw_string_literal(bytes, len, idx);
                continue;
            }
            b'(' => {
                paren_depth += 1;
                idx += 1;
                continue;
            }
            b')' => {
                paren_depth -= 1;
                idx += 1;
                continue;
            }
            b'{' if paren_depth <= 0 => {
                let sig_end = trim_signature_end(bytes, fn_start, idx);
                return Some(SignatureBounds {
                    sig_end,
                    body_start: idx,
                });
            }
            _ => {}
        }
        idx += 1;
    }
    None
}

fn trim_signature_end(bytes: &[u8], start: usize, brace_index: usize) -> usize {
    let mut sig_end = brace_index;
    while sig_end > start && bytes[sig_end - 1].is_ascii_whitespace() {
        sig_end -= 1;
    }
    sig_end
}

fn skip_function_body(bytes: &[u8], len: usize, mut idx: usize) -> usize {
    let mut depth = 1usize;
    while idx < len && depth > 0 {
        if idx + 1 < len && bytes[idx] == b'/' {
            if bytes[idx + 1] == b'/' {
                idx = skip_line_comment(bytes, len, idx + 2);
                continue;
            }
            if bytes[idx + 1] == b'*' {
                idx = skip_block_comment(bytes, len, idx + 2, 1);
                continue;
            }
        }
        match bytes[idx] {
            b'"' => idx = scan_string_literal(bytes, len, idx, b'"'),
            b'\'' => {
                if is_probable_lifetime(bytes, len, idx) {
                    idx = skip_lifetime(bytes, len, idx);
                } else {
                    idx = scan_string_literal(bytes, len, idx, b'\'');
                }
            }
            b'r' => idx = scan_raw_string_literal(bytes, len, idx),
            b'{' => {
                depth += 1;
                idx += 1;
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                idx += 1;
            }
            _ => idx += 1,
        }
    }
    idx
}

const fn is_ident_byte(b: u8) -> bool {
    b == b'_' || (b as char).is_ascii_alphanumeric()
}

const fn is_ident_start_byte(b: u8) -> bool {
    b == b'_' || (b as char).is_ascii_alphabetic()
}

fn is_probable_lifetime(bytes: &[u8], len: usize, idx: usize) -> bool {
    let next = idx + 1;
    if next >= len {
        return false;
    }
    let first = bytes[next];
    if !is_ident_start_byte(first) {
        return false;
    }
    let mut cursor = next + 1;
    while cursor < len {
        let current = bytes[cursor];
        if current == b'\'' {
            return false;
        }
        if is_ident_byte(current) {
            cursor += 1;
            continue;
        }
        break;
    }
    true
}

fn skip_lifetime(bytes: &[u8], len: usize, idx: usize) -> usize {
    let mut cursor = idx + 1;
    while cursor < len {
        let current = bytes[cursor];
        if !is_ident_byte(current) {
            break;
        }
        cursor += 1;
    }
    cursor
}

/// Match a relative path against a simple, comma-separated filter with '*' wildcards.
/// If a pattern has no '/', it matches the basename; empty/whitespace-only filters yield false.
#[must_use]
pub fn signatures_filter_matches(rel_path: &str, filter: &str) -> bool {
    let rel = rel_path;
    let name = rel.rsplit('/').next().unwrap_or(rel);
    for pat in filter.split(',') {
        let p = pat.trim();
        if p.is_empty() {
            continue;
        }
        let target = if p.contains('/') { rel } else { name };
        if wildcard_match(p, target) {
            return true;
        }
    }
    false
}

fn wildcard_match(pat: &str, text: &str) -> bool {
    // Simple '*' wildcard matcher. Case-sensitive; '*' matches any sequence including '/'.
    let (pbytes, tbytes) = (pat.as_bytes(), text.as_bytes());
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut star_ti) = (None, 0usize);
    while ti < tbytes.len() {
        if pi < pbytes.len() && (pbytes[pi] == tbytes[ti]) {
            pi += 1;
            ti += 1;
            continue;
        }
        if pi < pbytes.len() && pbytes[pi] == b'*' {
            star = Some(pi);
            pi += 1;
            star_ti = ti;
            continue;
        }
        if let Some(s) = star {
            // backtrack: let '*' consume one more char
            pi = s + 1;
            star_ti += 1;
            ti = star_ti;
            continue;
        }
        return false;
    }
    // Consume trailing '*' in pattern
    while pi < pbytes.len() && pbytes[pi] == b'*' {
        pi += 1;
    }
    pi == pbytes.len()
}

fn trim_leading_blank_lines(s: &str) -> String {
    let mut start = 0usize;
    let bytes = s.as_bytes();
    let n = bytes.len();
    while start < n {
        // find end of line
        let mut end = start;
        while end < n && bytes[end] != b'\n' {
            end += 1;
        }
        let line = &s[start..end];
        if line.trim().is_empty() {
            // skip this blank line and the trailing newline (if present)
            start = if end < n { end + 1 } else { end };
        } else {
            break;
        }
    }
    s[start..].to_string()
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum CommentState {
    Code,
    Block { remove: bool, depth: usize },
    Dq { escaped: bool },
    Sq { escaped: bool },
    Raw { hashes: usize },
}

struct CommentRemover<'a> {
    src: &'a str,
    bytes: &'a [u8],
    len: usize,
    index: usize,
    output: String,
    state: CommentState,
    remove_inline: bool,
    remove_doc: bool,
}

impl<'a> CommentRemover<'a> {
    fn new(input: &'a str, remove_inline: bool, remove_doc: bool) -> Self {
        Self {
            src: input,
            bytes: input.as_bytes(),
            len: input.len(),
            index: 0,
            output: String::with_capacity(input.len()),
            state: CommentState::Code,
            remove_inline,
            remove_doc,
        }
    }

    fn run(mut self) -> String {
        while self.index < self.len {
            match self.state {
                CommentState::Code => self.handle_code(),
                CommentState::Block { remove, depth } => self.handle_block(remove, depth),
                CommentState::Dq { escaped } => self.handle_quoted(b'"', escaped),
                CommentState::Sq { escaped } => self.handle_quoted(b'\'', escaped),
                CommentState::Raw { hashes } => self.handle_raw(hashes),
            }
        }
        self.output
    }

    fn handle_code(&mut self) {
        if self.starts_line_comment() {
            self.consume_line_comment();
            return;
        }
        if self.starts_block_comment() {
            self.enter_block_comment();
            return;
        }
        if self.bytes[self.index] == b'"' {
            self.output.push('"');
            self.index += 1;
            self.state = CommentState::Dq { escaped: false };
            return;
        }
        if self.bytes[self.index] == b'\'' {
            // Treat lifetimes like \'static, \'a, etc. as identifiers, not char literals.
            if is_probable_lifetime(self.bytes, self.len, self.index) {
                let end = skip_lifetime(self.bytes, self.len, self.index);
                self.output.push_str(&self.src[self.index..end]);
                self.index = end;
                return;
            }
            self.output.push('\'');
            self.index += 1;
            self.state = CommentState::Sq { escaped: false };
            return;
        }
        if self.bytes[self.index] == b'r' && self.try_enter_raw_string() {
            return;
        }
        // Copy the next Unicode scalar intact from the original source
        let ch = self.src[self.index..].chars().next().unwrap_or('\u{FFFD}');
        self.output.push(ch);
        self.index += ch.len_utf8();
    }

    fn starts_line_comment(&self) -> bool {
        self.index + 1 < self.len
            && self.bytes[self.index] == b'/'
            && self.bytes[self.index + 1] == b'/'
    }

    fn starts_block_comment(&self) -> bool {
        self.index + 1 < self.len
            && self.bytes[self.index] == b'/'
            && self.bytes[self.index + 1] == b'*'
    }

    fn consume_line_comment(&mut self) {
        let is_doc = self.index + 2 < self.len && matches!(self.bytes[self.index + 2], b'/' | b'!');
        let remove = if is_doc {
            self.remove_doc
        } else {
            self.remove_inline
        };
        if remove {
            trim_trailing_ws_current_line(&mut self.output);
            let mut cursor = self.index + 2;
            while cursor < self.len && self.bytes[cursor] != b'\n' {
                cursor += 1;
            }
            if cursor < self.len && self.bytes[cursor] == b'\n' {
                cursor += 1;
                self.output.push('\n');
            }
            self.index = cursor;
            return;
        }
        while self.index < self.len {
            let ch = self.src[self.index..].chars().next().unwrap_or('\u{FFFD}');
            if ch == '\n' {
                break;
            }
            self.output.push(ch);
            self.index += ch.len_utf8();
        }
        if self.index < self.len {
            self.output.push('\n');
            self.index += 1;
        }
    }

    fn enter_block_comment(&mut self) {
        let is_doc = self.index + 2 < self.len && matches!(self.bytes[self.index + 2], b'*' | b'!');
        let remove = if is_doc {
            self.remove_doc
        } else {
            self.remove_inline
        };
        if !remove {
            self.output.push('/');
            self.output.push('*');
        }
        self.index += 2;
        self.state = CommentState::Block { remove, depth: 1 };
    }

    fn handle_block(&mut self, remove: bool, depth: usize) {
        let mut current_depth = depth;
        while self.index < self.len {
            if self.index + 1 < self.len
                && self.bytes[self.index] == b'/'
                && self.bytes[self.index + 1] == b'*'
            {
                if !remove {
                    self.output.push('/');
                    self.output.push('*');
                }
                current_depth += 1;
                self.index += 2;
                continue;
            }
            if self.index + 1 < self.len
                && self.bytes[self.index] == b'*'
                && self.bytes[self.index + 1] == b'/'
            {
                if !remove {
                    self.output.push('*');
                    self.output.push('/');
                }
                current_depth = current_depth.saturating_sub(1);
                self.index += 2;
                if current_depth == 0 {
                    if remove && self.index < self.len && self.bytes[self.index] == b'\n' {
                        self.index += 1;
                    }
                    self.state = CommentState::Code;
                    return;
                }
                continue;
            }
            if remove {
                self.index += 1;
            } else {
                let ch = self.src[self.index..].chars().next().unwrap_or('\u{FFFD}');
                self.output.push(ch);
                self.index += ch.len_utf8();
            }
        }
        self.state = CommentState::Block {
            remove,
            depth: current_depth,
        };
    }

    fn handle_quoted(&mut self, quote: u8, escaped: bool) {
        if self.index >= self.len {
            self.state = CommentState::Code;
            return;
        }
        let ch = self.src[self.index..].chars().next().unwrap_or('\u{FFFD}');
        self.output.push(ch);
        self.index += ch.len_utf8();
        if !escaped && (ch as u32) == u32::from(quote) {
            self.state = CommentState::Code;
            return;
        }
        let next_escaped = ch == '\\' && !escaped;
        self.state = if quote == b'"' {
            CommentState::Dq {
                escaped: next_escaped,
            }
        } else {
            CommentState::Sq {
                escaped: next_escaped,
            }
        };
    }

    fn handle_raw(&mut self, hashes: usize) {
        if self.index >= self.len {
            return;
        }
        let ch = self.src[self.index..].chars().next().unwrap_or('\u{FFFD}');
        self.output.push(ch);
        if ch == '"' {
            let mut lookahead = self.index + 1;
            let mut matched = 0usize;
            while matched < hashes && lookahead < self.len && self.bytes[lookahead] == b'#' {
                lookahead += 1;
                matched += 1;
            }
            if matched == hashes {
                for offset in 0..hashes {
                    let idx = self.index + 1 + offset;
                    if idx < self.len {
                        self.output.push(self.bytes[idx] as char);
                    }
                }
                self.index = lookahead;
                self.state = CommentState::Code;
                return;
            }
        }
        self.index += ch.len_utf8();
    }

    fn try_enter_raw_string(&mut self) -> bool {
        let mut lookahead = self.index + 1;
        let mut hashes = 0usize;
        while lookahead < self.len && self.bytes[lookahead] == b'#' {
            hashes += 1;
            lookahead += 1;
        }
        if lookahead < self.len && self.bytes[lookahead] == b'"' {
            self.output.push('r');
            for _ in 0..hashes {
                self.output.push('#');
            }
            self.output.push('"');
            self.index = lookahead + 1;
            self.state = CommentState::Raw { hashes };
            return true;
        }
        false
    }
}

fn remove_comments_textual(input: &str, remove_inline: bool, remove_doc: bool) -> String {
    CommentRemover::new(input, remove_inline, remove_doc).run()
}
