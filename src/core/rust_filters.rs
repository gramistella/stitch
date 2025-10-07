use syn::{self};
// no token printing — we preserve original formatting; only function bodies are replaced

#[derive(Debug, Clone, Default)]
pub struct RustFilterOptions {
    pub remove_inline_regular_comments: bool,
    pub remove_doc_comments: bool,
    pub function_signatures_only: bool,
}

/// Returns true if the given path ends with ".rs" (case-sensitive like Rust filenames on most systems).
pub fn is_rust_file_path(path: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("rs")
}

/// Apply Rust-specific filters to a source file's string contents.
/// This function only transforms when at least one option is enabled; otherwise returns input as-is.
pub fn apply_rust_filters(source: &str, opts: &RustFilterOptions) -> String {
    if !(opts.remove_inline_regular_comments || opts.remove_doc_comments || opts.function_signatures_only) {
        return source.to_string();
    }

    // Fast path: if we only need to drop comments without altering code structure, we can use syn parsing
    // and reconstruct text with spans. We'll do a line-based fallback if parsing fails.
    match syn::parse_file(source) {
        Ok(_ast) => {
            if opts.function_signatures_only {
                // Apply comment removal first (if requested), then collapse function bodies only.
                let maybe_cleaned = if opts.remove_inline_regular_comments || opts.remove_doc_comments {
                    Some(remove_comments_textual(
                        source,
                        opts.remove_inline_regular_comments,
                        opts.remove_doc_comments,
                    ))
                } else {
                    None
                };
                let base: std::borrow::Cow<str> = match maybe_cleaned {
                    Some(s) => std::borrow::Cow::Owned(s),
                    None => std::borrow::Cow::Borrowed(source),
                };
                let transformed = transform_functions_to_signatures(&base);
                let collapsed = crate::core::collapse_consecutive_blank_lines(&transformed);
                return trim_leading_blank_lines(&collapsed);
            }
            // For comment removal only
            let cleaned = remove_comments_textual(
                source,
                opts.remove_inline_regular_comments,
                opts.remove_doc_comments,
            );
            let collapsed = crate::core::collapse_consecutive_blank_lines(&cleaned);
            trim_leading_blank_lines(&collapsed)
        }
        Err(_e) => {
            // Fallback: textual pass
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
                let base: std::borrow::Cow<str> = match maybe_cleaned {
                    Some(s) => std::borrow::Cow::Owned(s),
                    None => std::borrow::Cow::Borrowed(source),
                };
                let transformed = transform_functions_to_signatures(&base);
                let collapsed = crate::core::collapse_consecutive_blank_lines(&transformed);
                return trim_leading_blank_lines(&collapsed);
            }
            let cleaned = remove_comments_textual(
                source,
                opts.remove_inline_regular_comments,
                opts.remove_doc_comments,
            );
            let collapsed = crate::core::collapse_consecutive_blank_lines(&cleaned);
            trim_leading_blank_lines(&collapsed)
        }
    }
}

fn transform_functions_to_signatures(src: &str) -> String {
    // Replace each function body with ";\n" while preserving all other text verbatim.
    #[derive(Copy, Clone, Eq, PartialEq)]
    enum S { Code, Line, Block{depth: usize}, Dq{esc: bool}, Sq{esc: bool}, Raw{hashes: usize} }

    let bytes = src.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;              // scan index
    let mut last_copied = 0usize;    // start index of next slice to copy
    let mut st = S::Code;            // top-level scan state
    let mut out = String::with_capacity(n);

    let is_ident = |b: u8| b == b'_' || (b as char).is_ascii_alphanumeric();

    fn scan_string(bytes: &[u8], n: usize, mut j: usize, quote: u8) -> usize {
        let mut esc = false; j += 1;
        while j < n { let b = bytes[j]; j += 1; if !esc && b == quote { break; } esc = b == b'\\' && !esc; }
        j
    }
    fn scan_raw_string(bytes: &[u8], n: usize, mut j: usize) -> usize {
        let mut k = j + 1; let mut h = 0usize; while k < n && bytes[k] == b'#' { h += 1; k += 1; }
        if k < n && bytes[k] == b'"' { j = k + 1; } else { return j + 1; }
        while j < n { if bytes[j] == b'"' { let mut t = j + 1; let mut m = 0usize; while m < h && t < n && bytes[t] == b'#' { t += 1; m += 1; } if m == h { return t; } } j += 1; }
        j
    }

    while i < n {
        match st {
            S::Code => {
                // quickly skip comments/strings/raw
                if i + 1 < n && bytes[i] == b'/' && bytes[i+1] == b'/' { st = S::Line; i += 2; continue; }
                if i + 1 < n && bytes[i] == b'/' && bytes[i+1] == b'*' { st = S::Block{depth:1}; i += 2; continue; }
                if bytes[i] == b'"' { i = scan_string(bytes, n, i, b'"'); continue; }
                if bytes[i] == b'\'' { i = scan_string(bytes, n, i, b'\''); continue; }
                if bytes[i] == b'r' { i = scan_raw_string(bytes, n, i); continue; }

                // try match 'fn' token at boundary
                if bytes[i] == b'f' && i + 1 < n && bytes[i+1] == b'n' {
                    let prev_ok = i == 0 || !is_ident(bytes[i.saturating_sub(1)]);
                    let next_ok = i + 2 >= n || !is_ident(bytes[i+2]);
                    if prev_ok && next_ok {
                        // find opening '{' of body, respecting parentheses and inner strings/comments
                        let mut j = i + 2; let mut par = 0i32; let mut st2 = S::Code;
                        while j < n {
                            match st2 {
                                S::Code => {
                                    if bytes[j] == b'(' { par += 1; j += 1; continue; }
                                    if bytes[j] == b')' { par -= 1; j += 1; continue; }
                                    if bytes[j] == b'{' && par <= 0 { break; }
                                    if j + 1 < n && bytes[j] == b'/' && bytes[j+1] == b'/' { st2 = S::Line; j += 2; continue; }
                                    if j + 1 < n && bytes[j] == b'/' && bytes[j+1] == b'*' { st2 = S::Block{depth:1}; j += 2; continue; }
                                    if bytes[j] == b'"' { st2 = S::Dq{esc:false}; j += 1; continue; }
                                    if bytes[j] == b'\'' { st2 = S::Sq{esc:false}; j += 1; continue; }
                                    if bytes[j] == b'r' { j = scan_raw_string(bytes, n, j); continue; }
                                    j += 1;
                                }
                                S::Line => { while j < n && bytes[j] != b'\n' { j += 1; } if j < n { j += 1; } st2 = S::Code; }
                                S::Block{mut depth} => { while j + 1 < n && depth > 0 { if bytes[j] == b'/' && bytes[j+1] == b'*' { depth += 1; j += 2; continue; } if bytes[j] == b'*' && bytes[j+1] == b'/' { depth -= 1; j += 2; continue; } j += 1; } if depth == 0 { st2 = S::Code; } }
                                S::Dq{..} => { j = scan_string(bytes, n, j.saturating_sub(1), b'"'); st2 = S::Code; }
                                S::Sq{..} => { j = scan_string(bytes, n, j.saturating_sub(1), b'\''); st2 = S::Code; }
                                S::Raw{..} => { j = scan_raw_string(bytes, n, j.saturating_sub(1)); st2 = S::Code; }
                            }
                        }
                        if j >= n || bytes[j] != b'{' { i += 2; continue; }

                        // emit text before body and collapse body to ";\n"
                        let mut sig_end = j; while sig_end > i && bytes[sig_end - 1].is_ascii_whitespace() { sig_end -= 1; }
                        out.push_str(&src[last_copied..sig_end]); out.push_str(";\n");

                        // skip body
                        let mut k = j + 1; let mut depth = 1usize; let mut st3 = S::Code;
                        while k < n && depth > 0 {
                            match st3 {
                                S::Code => {
                                    if k + 1 < n && bytes[k] == b'/' && bytes[k+1] == b'/' { st3 = S::Line; k += 2; continue; }
                                    if k + 1 < n && bytes[k] == b'/' && bytes[k+1] == b'*' { st3 = S::Block{depth:1}; k += 2; continue; }
                                    if bytes[k] == b'"' { st3 = S::Dq{esc:false}; k += 1; continue; }
                                    if bytes[k] == b'\'' { st3 = S::Sq{esc:false}; k += 1; continue; }
                                    if bytes[k] == b'r' { k = scan_raw_string(bytes, n, k); continue; }
                                    if bytes[k] == b'{' { depth += 1; k += 1; continue; }
                                    if bytes[k] == b'}' { depth -= 1; k += 1; continue; }
                                    k += 1;
                                }
                                S::Line => { while k < n && bytes[k] != b'\n' { k += 1; } if k < n { k += 1; } st3 = S::Code; }
                                S::Block{mut depth} => { while k + 1 < n && depth > 0 { if bytes[k] == b'/' && bytes[k+1] == b'*' { depth += 1; k += 2; continue; } if bytes[k] == b'*' && bytes[k+1] == b'/' { depth -= 1; k += 2; continue; } k += 1; } if depth == 0 { st3 = S::Code; } }
                                S::Dq{..} => { k = scan_string(bytes, n, k.saturating_sub(1), b'"'); st3 = S::Code; }
                                S::Sq{..} => { k = scan_string(bytes, n, k.saturating_sub(1), b'\''); st3 = S::Code; }
                                S::Raw{..} => { k = scan_raw_string(bytes, n, k.saturating_sub(1)); st3 = S::Code; }
                            }
                        }
                        last_copied = k; i = k; st = S::Code; continue;
                    }
                }
                i += 1;
            }
            S::Line => { while i < n && bytes[i] != b'\n' { i += 1; } if i < n { i += 1; } st = S::Code; }
            S::Block{mut depth} => { while i + 1 < n && depth > 0 { if bytes[i] == b'/' && bytes[i+1] == b'*' { depth += 1; i += 2; continue; } if bytes[i] == b'*' && bytes[i+1] == b'/' { depth -= 1; i += 2; continue; } i += 1; } if depth == 0 { st = S::Code; } }
            S::Dq{..} => { i = scan_string(bytes, n, i, b'"'); st = S::Code; }
            S::Sq{..} => { i = scan_string(bytes, n, i, b'\''); st = S::Code; }
            S::Raw{..} => { i = scan_raw_string(bytes, n, i); st = S::Code; }
        }
    }

    out.push_str(&src[last_copied..]);
    out
}

/// Returns true if the given relative path (using forward slashes) matches any of the
/// comma-separated glob-like patterns in `filter`. Supports `*` wildcard; if a pattern
/// contains no slash, it matches against the basename only. Empty/whitespace-only filter
/// yields false (i.e., no restriction by itself).
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
            pi += 1; ti += 1; continue;
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
    while pi < pbytes.len() && pbytes[pi] == b'*' { pi += 1; }
    pi == pbytes.len()
}

fn trim_leading_blank_lines(s: &str) -> String {
    let mut start = 0usize;
    let bytes = s.as_bytes();
    let n = bytes.len();
    while start < n {
        // find end of line
        let mut end = start;
        while end < n && bytes[end] != b'\n' { end += 1; }
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

fn remove_comments_textual(input: &str, remove_inline: bool, remove_doc: bool) -> String {
    // We implement a small state machine to remove // and /* */ comments. If remove_doc is true,
    // we also remove ///, //! and /** */ style docs.
    // We keep strings and raw strings intact.
    #[derive(Copy, Clone, Eq, PartialEq)]
    enum S { Code, BlockComment{ remove: bool, depth: usize }, Dq{esc: bool}, Sq{esc: bool}, Raw{hashes: usize} }

    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());

    fn trim_trailing_ws_current_line(buf: &mut String) {
        let mut idx = buf.len();
        while idx > 0 {
            let b = buf.as_bytes()[idx - 1];
            if b == b' ' || b == b'\t' {
                idx -= 1;
                continue;
            }
            if b == b'\n' { break; }
            break;
        }
        if idx < buf.len() { buf.truncate(idx); }
    }
    let mut i = 0usize;
    let n = bytes.len();
    let mut state = S::Code;

    while i < n {
        match state {
            S::Code => {
                if i + 1 < n && bytes[i] == b'/' && bytes[i+1] == b'/' {
                    // Detect doc line comments: /// or //!
                    let is_doc = if i + 2 < n && (bytes[i+2] == b'/' || bytes[i+2] == b'!') { true } else { false };
                    let should_remove = if is_doc { remove_doc } else { remove_inline };
                    if should_remove {
                        // trim trailing spaces before the comment marker, and keep a single newline
                        trim_trailing_ws_current_line(&mut out);
                        // skip to end of line
                        while i < n && bytes[i] != b'\n' { i += 1; }
                        if i < n && bytes[i] == b'\n' { i += 1; out.push('\n'); }
                        continue;
                    } else {
                        // keep as-is
                        out.push_str("//");
                        i += 2;
                        continue;
                    }
                }
                if i + 1 < n && bytes[i] == b'/' && bytes[i+1] == b'*' {
                    // Block comment start; determine doc style /** or /*!.
                    let is_doc = if i + 2 < n && (bytes[i+2] == b'*' || bytes[i+2] == b'!') { true } else { false };
                    let should_remove = if is_doc { remove_doc } else { remove_inline };
                    // Emit the opener if we are preserving
                    if !should_remove {
                        out.push('/'); out.push('*');
                    }
                    i += 2;
                    state = S::BlockComment { remove: should_remove, depth: 1 };
                    continue;
                }
                // Strings
                if bytes[i] == b'"' {
                    out.push('"');
                    i += 1;
                    state = S::Dq{esc:false};
                    continue;
                }
                if bytes[i] == b'\'' {
                    out.push('\'');
                    i += 1;
                    state = S::Sq{esc:false};
                    continue;
                }
                // Raw string r#"..."#
                if bytes[i] == b'r' {
                    // Count hashes after r
                    let mut j = i + 1;
                    let mut hashes = 0;
                    while j < n && bytes[j] == b'#' { hashes += 1; j += 1; }
                    if j < n && bytes[j] == b'"' {
                        // raw string
                        out.push('r');
                        for _ in 0..hashes { out.push('#'); }
                        out.push('"');
                        i = j + 1;
                        state = S::Raw{hashes};
                        continue;
                    }
                }
                out.push(bytes[i] as char);
                i += 1;
            }
            S::BlockComment{ remove, depth } => {
                // Consume until matching */ with nesting
                let mut d = depth;
                if i + 1 < n && bytes[i] == b'/' && bytes[i+1] == b'*' {
                    if !remove { out.push('/'); out.push('*'); }
                    d += 1; i += 2; state = S::BlockComment { remove, depth: d }; continue;
                }
                if i + 1 < n && bytes[i] == b'*' && bytes[i+1] == b'/' {
                    d = d.saturating_sub(1);
                    if !remove { out.push('*'); out.push('/'); }
                    i += 2;
                    if d == 0 {
                        // If we removed a full-line block and the next char is a newline, swallow it to avoid extra blank line.
                        if remove && i < n && bytes[i] == b'\n' { i += 1; }
                        state = S::Code;
                    } else {
                        state = S::BlockComment { remove, depth: d };
                    }
                    continue;
                }
                if !remove { out.push(bytes[i] as char); }
                i += 1;
            }
            S::Dq{mut esc} => {
                let b = bytes[i];
                if !esc && b == b'"' { i += 1; state = S::Code; out.push('"'); continue; }
                out.push(b as char);
                esc = b == b'\\' && !esc;
                state = S::Dq{esc};
                i += 1;
            }
            S::Sq{mut esc} => {
                let b = bytes[i];
                if !esc && b == b'\'' { i += 1; state = S::Code; out.push('\''); continue; }
                out.push(b as char);
                esc = b == b'\\' && !esc;
                state = S::Sq{esc};
                i += 1;
            }
            S::Raw{hashes} => {
                let b = bytes[i];
                out.push(b as char);
                if b == b'"' {
                    // Check closing with matching hashes; emit trailing #...# when closing
                    let mut j = i + 1;
                    let mut k = 0;
                    while k < hashes && j < n && bytes[j] == b'#' { j += 1; k += 1; }
                    if k == hashes {
                        for _ in 0..hashes { out.push('#'); }
                        state = S::Code; i = j; continue;
                    }
                }
                i += 1;
            }
        }
    }

    out
}


