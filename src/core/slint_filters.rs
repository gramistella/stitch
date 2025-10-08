#[derive(Debug, Clone, Default)]
pub struct SlintFilterOptions {
    pub remove_line_comments: bool,
    pub remove_block_comments: bool,
}

/// Returns true if the given path ends with ".slint" (case-sensitive like most filesystems).
#[must_use]
pub fn is_slint_file_path(path: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("slint")
}

/// Apply Slint-specific filters to a source file's string contents.
/// This function only transforms when at least one option is enabled; otherwise returns input as-is.
#[must_use]
#[allow(clippy::too_many_lines, clippy::items_after_statements)]
pub fn apply_slint_filters(source: &str, opts: &SlintFilterOptions) -> String {
    if !(opts.remove_line_comments || opts.remove_block_comments) {
        return source.to_string();
    }

    enum State {
        Code,
        Block,    // inside /* ... */
        Dq(bool), // inside "..." with escaped flag
        Sq(bool), // inside '...' with escaped flag
    }

    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut i = 0usize;
    let n = bytes.len();
    let mut state = State::Code;
    let mut line_has_content = false;
    let mut suppress_newline_once = false;

    while i < n {
        match state {
            State::Code => {
                // Line comments: // ... until newline
                if opts.remove_line_comments
                    && i + 1 < n
                    && bytes[i] == b'/'
                    && bytes[i + 1] == b'/'
                {
                    // consume until end of line (but keep the trailing newline if present)
                    i += 2;
                    while i < n && bytes[i] != b'\n' {
                        i += 1;
                    }
                    // copy the newline if present
                    if i < n && bytes[i] == b'\n' {
                        if line_has_content {
                            out.push('\n');
                        }
                        i += 1;
                        line_has_content = false;
                        suppress_newline_once = false;
                    }
                    continue;
                }

                // Block comments: /* ... */ (no nesting required)
                if opts.remove_block_comments
                    && i + 1 < n
                    && bytes[i] == b'/'
                    && bytes[i + 1] == b'*'
                {
                    i += 2;
                    state = State::Block;
                    continue;
                }

                // Enter string states
                if bytes[i] == b'"' {
                    out.push('"');
                    i += 1;
                    state = State::Dq(false);
                    line_has_content = true;
                    continue;
                }
                if bytes[i] == b'\'' {
                    out.push('\'');
                    i += 1;
                    state = State::Sq(false);
                    line_has_content = true;
                    continue;
                }

                // Copy next UTF-8 scalar
                let ch = source[i..].chars().next().unwrap_or('\u{FFFD}');
                if ch == '\n' {
                    if suppress_newline_once {
                        suppress_newline_once = false;
                        line_has_content = false;
                        i += ch.len_utf8();
                        continue;
                    }
                    out.push('\n');
                    line_has_content = false;
                } else {
                    out.push(ch);
                    line_has_content = true;
                }
                i += ch.len_utf8();
            }
            State::Block => {
                // Skip until closing */
                if i + 1 < n && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    state = State::Code;
                    if !line_has_content {
                        suppress_newline_once = true;
                    }
                    continue;
                }
                // Otherwise, just advance; do not copy content inside comments
                // We do not attempt to preserve newlines inside block comments.
                i += 1;
            }
            State::Dq(escaped) => {
                if escaped {
                    // copy escaped char as-is
                    let ch = source[i..].chars().next().unwrap_or('\u{FFFD}');
                    out.push(ch);
                    i += ch.len_utf8();
                    state = State::Dq(false);
                    line_has_content = ch != '\n';
                    continue;
                }
                if bytes[i] == b'\\' {
                    out.push('\\');
                    i += 1;
                    state = State::Dq(true);
                    line_has_content = true;
                    continue;
                }
                if bytes[i] == b'"' {
                    out.push('"');
                    i += 1;
                    state = State::Code;
                    line_has_content = true;
                    continue;
                }
                let ch = source[i..].chars().next().unwrap_or('\u{FFFD}');
                out.push(ch);
                i += ch.len_utf8();
                line_has_content = ch != '\n';
            }
            State::Sq(escaped) => {
                if escaped {
                    let ch = source[i..].chars().next().unwrap_or('\u{FFFD}');
                    out.push(ch);
                    i += ch.len_utf8();
                    state = State::Sq(false);
                    line_has_content = ch != '\n';
                    continue;
                }
                if bytes[i] == b'\\' {
                    out.push('\\');
                    i += 1;
                    state = State::Sq(true);
                    line_has_content = true;
                    continue;
                }
                if bytes[i] == b'\'' {
                    out.push('\'');
                    i += 1;
                    state = State::Code;
                    line_has_content = true;
                    continue;
                }
                let ch = source[i..].chars().next().unwrap_or('\u{FFFD}');
                out.push(ch);
                i += ch.len_utf8();
                line_has_content = ch != '\n';
            }
        }
    }

    out
}
