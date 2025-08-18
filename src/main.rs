// src/main.rs
// A Rust rewrite of the Tkinter app using Slint for the desktop UI.
// - Left: folder selection, filters, and a "flat" tree view with expand/collapse + checkboxes
// - Right: Generate Output button, last refresh label, and an output text area
// Notes:
// * The tree view is implemented as a flattened list for simplicity. Each row shows an expand glyph, a checkbox, and a name, indented by level.
// * We scan the full tree (respecting filters) when the folder or filters change.
// * Check state inheritance: a node uses its own explicit state if present; otherwise inherits the nearest ancestor's effective state (default false).
// * Toggling a directory clears explicit states for all its descendants, matching the Python semantics.

#![allow(clippy::needless_return)] // What about a comment here?

/* 
This is a multi-line comment.
It can span
multiple lines.
*/

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    time::SystemTime,
};

use anyhow::Context;
use chrono::Local;
use regex::Regex;
use slint::{Model, ModelRc, VecModel};

slint::include_modules!();

#[derive(Clone, Debug)]
struct Node {
    name: String,
    path: PathBuf,
    is_dir: bool,
    children: Vec<Node>,
    expanded: bool,
    has_children: bool, // after filtering
}

#[derive(Default)]
struct AppState {
    selected_directory: Option<PathBuf>,
    root_node: Option<Node>,
    explicit_states: HashMap<PathBuf, bool>, // explicit True/False per path
    last_mod_times: HashMap<PathBuf, Option<SystemTime>>,
    poll_interval_ms: u64,
    path_snapshot: Option<HashSet<PathBuf>>,
    remove_prefixes: Vec<String>,
    remove_regex_str: Option<String>,
    remove_regex: Option<Regex>,
    include_exts: HashSet<String>,
    exclude_exts: HashSet<String>,
    exclude_dirs: HashSet<String>,
    exclude_files: HashSet<String>,
    copy_toast_timer: slint::Timer,
}

// src/main.rs
fn main() -> anyhow::Result<()> {
    let app = AppWindow::new()?;

    // Initialize default UI values (match the Python defaults where reasonable)
    app.set_ext_filter("".into());
    app.set_exclude_dirs(".git, node_modules, target, _target, .elan, .lake, .idea, .vscode, _app, .svelte-kit, .sqlx, venv, .venv, __pycache__".into());
    app.set_exclude_files("LICENSE, Cargo.lock, package-lock.json, yarn.lock, .DS_Store, .dockerignore, .gitignore, .npmignore, .pre-commit-config.yaml, .prettierignore, .prettierrc, eslint.config.js, .env, Thumbs.db".into());
    app.set_remove_prefix("".into());
    app.set_remove_regex("".into());
    app.set_hierarchy_only(false);
    app.set_dirs_only(false);
    app.set_last_refresh("Last refresh: N/A".into());
    app.set_tree_model(ModelRc::new(VecModel::<Row>::default()));
    app.set_output_text("".into());
    app.set_show_copy_toast(false);
    app.set_copy_toast_text("".into());
    
    let state = Rc::new(RefCell::new(AppState {
        poll_interval_ms: 30_000,
        ..Default::default()
    }));

    // Timer for periodic polling
    let poll_timer = slint::Timer::default();
    {
        let app_weak = app.as_weak();
        let state = Rc::clone(&state);
        // Take the interval out before capturing `state` in the closure to avoid borrow/move clash
        let interval_ms = { state.borrow().poll_interval_ms };
        poll_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(interval_ms),
            move || {
                if let Some(app) = app_weak.upgrade() {
                    on_check_updates(&app, &state);
                }
            },
        );
    }

    // Hook up callbacks
    {
        let app_weak = app.as_weak();
        let state = Rc::clone(&state);
        app.on_select_folder(move || {
            if let Some(app) = app_weak.upgrade() {
                on_select_folder(&app, &state);
            }
        });
    }
    {
        let app_weak = app.as_weak();
        let state = Rc::clone(&state);
        app.on_filter_changed(move || {
            if let Some(app) = app_weak.upgrade() {
                on_filter_changed(&app, &state);
            }
        });
    }
    {
        let app_weak = app.as_weak();
        let state = Rc::clone(&state);
        app.on_toggle_expand(move |idx| {
            if let Some(app) = app_weak.upgrade() {
                on_toggle_expand(&app, &state, idx as usize);
            }
        });
    }
    {
        let app_weak = app.as_weak();
        let state = Rc::clone(&state);
        app.on_toggle_check(move |idx| {
            if let Some(app) = app_weak.upgrade() {
                on_toggle_check(&app, &state, idx as usize);
            }
        });
    }
    {
        let app_weak = app.as_weak();
        let state = Rc::clone(&state);
        app.on_generate_output(move || {
            if let Some(app) = app_weak.upgrade() {
                on_generate_output(&app, &state);
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let state = Rc::clone(&state);
        app.on_copy_output(move || {
            if let Some(app) = app_weak.upgrade() {
                on_copy_output(&app, &state);
            }
        });
    }



    app.run()?;
    Ok(())
}


/* === UI EVENT HANDLERS === */

fn on_select_folder(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    if let Some(dir) = rfd::FileDialog::new().set_directory(".").pick_folder() {
        {
            let mut s = state.borrow_mut();
            s.selected_directory = Some(dir.clone());
            s.explicit_states.clear();
            s.last_mod_times.clear();
        }
        rebuild_tree_and_ui(app, state);
        update_last_refresh(app);
    }
}

fn on_filter_changed(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    parse_filters_from_ui(app, state);
    rebuild_tree_and_ui(app, state);
    on_generate_output(app, state);
    update_last_refresh(app);
}

fn on_toggle_expand(app: &AppWindow, state: &Rc<RefCell<AppState>>, index: usize) {
    if let Some(row) = get_row_by_index(app, index) {
        let path = PathBuf::from(row.path.as_str());
        if toggle_node_expanded(state, &path) {
            refresh_flat_model(app, state);
        }
    }
}

fn on_toggle_check(app: &AppWindow, state: &Rc<RefCell<AppState>>, index: usize) {
    if let Some(row) = get_row_by_index(app, index) {
        let path = PathBuf::from(row.path.as_str());
        let is_dir = row.is_dir;
        let effective = row.checked;
        let new_state = !effective;

        {
            let mut s = state.borrow_mut();
            s.explicit_states.insert(path.clone(), new_state);
        }

        if is_dir {
            clear_descendant_explicit_states(state, &path);
        }

        refresh_flat_model(app, state);
        on_generate_output(app, state);
    }
}

// src/main.rs
fn on_generate_output(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    parse_filters_from_ui(app, state);

    let want_dirs_only = app.get_dirs_only();
    let hierarchy_only = app.get_hierarchy_only();

    let (selected_files, selected_dirs, relative_paths) = {
        let s = state.borrow();
        if s.selected_directory.is_none() || s.root_node.is_none() {
            set_output(app, "No folder selected.\n");
            update_last_refresh(app);
            return;
        }
        let root = s.root_node.as_ref().unwrap();
        let mut files = Vec::new();
        let mut dirs = Vec::new();
        collect_selected_paths(root, &s.explicit_states, None, &mut files, &mut dirs);

        let selected_dir = s.selected_directory.as_ref().unwrap();
        let mut rels = Vec::new();
        if want_dirs_only {
            for d in &dirs {
                if let Some(r) = pathdiff::diff_paths(d, selected_dir) {
                    if r != PathBuf::from("") {
                        rels.push(path_to_unix(&r));
                    }
                }
            }
        } else {
            for f in &files {
                if let Some(r) = pathdiff::diff_paths(f, selected_dir) {
                    if r != PathBuf::from("") {
                        rels.push(path_to_unix(&r));
                    }
                }
            }
        }
        (files, dirs, rels)
    };

    if (!want_dirs_only && selected_files.is_empty()) || (want_dirs_only && selected_dirs.is_empty()) {
        set_output(app, "No items selected.\n");
        update_last_refresh(app);
        return;
    }

    // Track last mod times for selected files when not directories-only
    {
        let mut s = state.borrow_mut();
        if !want_dirs_only {
            for fp in &selected_files {
                let mtime = fs::metadata(fp).ok().and_then(|m| m.modified().ok());
                s.last_mod_times.insert(fp.clone(), mtime);
            }
        }
    }

    let mut out = String::new();
    out.push_str("=== FILE HIERARCHY ===\n\n");

    // Render directly from relative paths
    let root_name = {
        let s = state.borrow();
        s.selected_directory
            .as_ref()
            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
            .unwrap_or_else(|| "root".into())
    };
    out.push_str(&render_unicode_tree_from_paths(&relative_paths, Some(&root_name)));

    if !hierarchy_only && !app.get_dirs_only() {
        out.push_str("\n=== FILE CONTENTS ===\n\n");

        // prepare content filters
        let (remove_prefixes, remove_regex_opt) = {
            let s = state.borrow();
            let rp = s.remove_prefixes.clone();
            let rr = s.remove_regex.clone();
            (rp, rr)
        };

        for fp in selected_files {
            let s_dir = state.borrow().selected_directory.clone().unwrap();
            let rel = pathdiff::diff_paths(&fp, &s_dir).unwrap_or_else(|| PathBuf::from(fp.file_name().unwrap_or_default()));
            out.push_str(&format!("--- Start of file: {} ---\n", rel.to_string_lossy()));

            let mut contents = fs::read_to_string(&fp).unwrap_or_else(|e| format!("Error reading file: {e}"));

            if !remove_prefixes.is_empty() {
                contents = strip_lines_and_inline_comments(&contents, &remove_prefixes);
            }

            if let Some(rr) = &remove_regex_opt {
                contents = rr.replace_all(&contents, "").to_string();
            }

            out.push_str(&contents);
            out.push('\n');
            out.push_str(&format!("--- End of file: {} ---\n\n", rel.to_string_lossy()));
        }
    }

    set_output(app, &out);
    update_last_refresh(app);
}

fn on_copy_output(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let text = app.get_output_text().to_string();
    if text.is_empty() {
        // Optional: show a gentle message for empty state
        app.set_copy_toast_text("Nothing to copy".into());
        app.set_show_copy_toast(true);

        // Hide after a moment
        {
            let mut s = state.borrow_mut();
            let app_weak = app.as_weak();
            s.copy_toast_timer.start(
                slint::TimerMode::SingleShot,
                std::time::Duration::from_millis(900),
                move || {
                    if let Some(app) = app_weak.upgrade() {
                        app.set_show_copy_toast(false);
                    }
                },
            );
        }
        return;
    }

    // Try to copy to clipboard
    let mut ok = false;
    if let Ok(mut cb) = arboard::Clipboard::new() {
        ok = cb.set_text(text).is_ok();
    }

    app.set_copy_toast_text(if ok { "Copied!" } else { "Copy failed" }.into());
    app.set_show_copy_toast(true);

    // Auto-hide the toast
    {
        let mut s = state.borrow_mut();
        let app_weak = app.as_weak();
        s.copy_toast_timer.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_millis(1200),
            move || {
                if let Some(app) = app_weak.upgrade() {
                    app.set_show_copy_toast(false);
                }
            },
        );
    }
}


/* === CORE LOGIC === */

fn rebuild_tree_and_ui(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    parse_filters_from_ui(app, state);
    {
        let s = state.borrow();
        if s.selected_directory.is_none() {
            set_tree_model(app, Vec::new());
            return;
        }
    }
    {
        let (root, snapshot) = {
            let s = state.borrow();
            let dir = s.selected_directory.as_ref().unwrap().clone();
            let include = s.include_exts.clone();
            let exclude = s.exclude_exts.clone();
            let ex_dirs = s.exclude_dirs.clone();
            let ex_files = s.exclude_files.clone();
            let root = scan_dir_to_node(&dir, &include, &exclude, &ex_dirs, &ex_files);
            let snap = gather_paths_set(&root);
            (root, snap)
        };

        // cache regex
        {
            let mut s = state.borrow_mut();
            s.path_snapshot = Some(snapshot);
            s.root_node = Some(root);
            s.remove_regex = compile_remove_regex_opt(s.remove_regex_str.as_deref());
        }
    }

    refresh_flat_model(app, state);
}

fn refresh_flat_model(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let rows = {
        let s = state.borrow();
        if let Some(root) = &s.root_node {
            flatten_tree(
                root,
                &s.explicit_states,
                None,
                0,
                &mut Vec::new(), // placeholder, replaced by ret
            )
        } else {
            Vec::new()
        }
    };
    set_tree_model(app, rows);
}

fn parse_filters_from_ui(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let ext_raw = app.get_ext_filter().to_string();
    let (include_exts, exclude_exts) = parse_extension_filters(&ext_raw);

    let exclude_dirs = split_csv_set(&app.get_exclude_dirs());
    let exclude_files = split_csv_set(&app.get_exclude_files());

    let remove_prefixes = split_prefix_list(&app.get_remove_prefix());

    let remove_regex_str = {
        let raw = app.get_remove_regex().to_string();
        let cleaned = clean_remove_regex(&raw);
        if cleaned.trim().is_empty() { None } else { Some(cleaned) }
    };

    let mut st = state.borrow_mut();
    st.include_exts = include_exts;
    st.exclude_exts = exclude_exts;
    st.exclude_dirs = exclude_dirs;
    st.exclude_files = exclude_files;
    st.remove_prefixes = remove_prefixes;
    st.remove_regex_str = remove_regex_str.clone();
    // NEW: keep compiled regex up to date
    st.remove_regex = compile_remove_regex_opt(remove_regex_str.as_deref());
}

fn toggle_node_expanded(state: &Rc<RefCell<AppState>>, path: &Path) -> bool {
    fn rec(n: &mut Node, target: &Path) -> bool {
        if n.path == target {
            if n.is_dir {
                n.expanded = !n.expanded;
                return true;
            } else {
                return false;
            }
        }
        for c in &mut n.children {
            if rec(c, target) {
                return true;
            }
        }
        false
    }
    if let Some(root) = state.borrow_mut().root_node.as_mut() {
        return rec(root, path);
    }
    false
}

fn clear_descendant_explicit_states(state: &Rc<RefCell<AppState>>, dir: &Path) {
    let mut to_clear = Vec::new();
    {
        let s = state.borrow();
        for p in s.explicit_states.keys() {
            if is_ancestor_of(dir, p) && p != dir {
                to_clear.push(p.clone());
            }
        }
    }
    if !to_clear.is_empty() {
        let mut s = state.borrow_mut();
        for p in to_clear {
            s.explicit_states.remove(&p);
        }
    }
}

fn collect_selected_paths(
    node: &Node,
    explicit: &HashMap<PathBuf, bool>,
    inherited: Option<bool>,
    files_out: &mut Vec<PathBuf>,
    dirs_out: &mut Vec<PathBuf>,
) {
    let my_effective = explicit.get(&node.path).copied().or(inherited).unwrap_or(false);

    if node.is_dir {
        if my_effective && dir_contains_file(node) {
            dirs_out.push(node.path.clone());
        }
        // If a directory has an explicit state, children inherit unless they override
        let next_inherited = my_effective;
        for c in &node.children {
            collect_selected_paths(c, explicit, Some(next_inherited), files_out, dirs_out);
        }
    } else {
        if my_effective {
            files_out.push(node.path.clone());
        }
    }
}

fn dir_contains_file(node: &Node) -> bool {
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

// src/main.rs
fn build_tree_from_rel_paths(_paths: &[String]) -> BTreeMap<String, BTreeMap<String, serde_json::Value>> {
    // Legacy helper no longer used by rendering. Keep a harmless implementation to satisfy the type.
    BTreeMap::new()
}

fn split_prefix_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}


fn render_unicode_tree(
    _tree: &BTreeMap<String, BTreeMap<String, serde_json::Value>>,
    root_name: Option<&str>,
) -> String {
    let mut out = String::new();
    if let Some(root) = root_name {
        out.push_str(root);
        out.push('\n');
    }
    out
}

// src/main.rs
fn render_unicode_tree_from_paths(paths: &[String], root_name: Option<&str>) -> String {
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Node {
        children: BTreeMap<String, Box<Node>>,
    }

    fn insert_path(root: &mut Node, parts: &[&str]) {
        if parts.is_empty() {
            return;
        }
        let head = parts[0].to_string();
        let entry = root.children.entry(head).or_default();
        if parts.len() > 1 {
            insert_path(entry, &parts[1..]);
        }
    }

    fn render(node: &Node, prefix: &str, out: &mut String) {
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

    // Build the tree
    let mut root = Node::default();
    for p in paths {
        let parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
        insert_path(&mut root, &parts);
    }

    // Render
    let mut out = String::new();
    if let Some(name) = root_name {
        out.push_str(name);
        out.push('\n');
    }
    render(&root, "", &mut out);
    out
}


/// Drops full lines that start with any prefix (after leading whitespace),
/// and trims inline comments *only* when the prefix is preceded by whitespace.
/// Example kept: `http://example.com` (no space before `//`)
fn strip_lines_and_inline_comments(contents: &str, prefixes: &[String]) -> String {
    if prefixes.is_empty() {
        return contents.to_string();
    }

    let mut out = String::with_capacity(contents.len());

    'line: for line in contents.lines() {
        // Find first non-whitespace position
        let first_non_ws = line
            .char_indices()
            .find(|&(_, ch)| !ch.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or_else(|| line.len());

        // Full-line comment? (after leading whitespace)
        if prefixes.iter().any(|p| !p.is_empty() && line[first_non_ws..].starts_with(p)) {
            continue 'line; // drop the whole line
        }

        // Otherwise, optionally trim an inline comment preceded by whitespace.
        let mut cut_at: Option<usize> = None;
        let mut prev_ch: Option<char> = None;

        for (pos, ch) in line.char_indices() {
            // Only consider prefixes from the first non-ws onward
            if pos < first_non_ws { 
                prev_ch = Some(ch);
                continue;
            }

            for p in prefixes {
                if p.is_empty() { continue; }
                if line[pos..].starts_with(p) {
                    // Only cut if the previous character exists and is whitespace
                    if prev_ch.map(|c| c.is_whitespace()).unwrap_or(false) {
                        cut_at = Some(cut_at.map_or(pos, |old| old.min(pos)));
                        break;
                    }
                }
            }
            if cut_at.is_some() { break; }
            prev_ch = Some(ch);
        }

        let kept = if let Some(pos) = cut_at {
            // Trim trailing spaces/tabs before the comment we cut
            let left = &line[..pos];
            left.trim_end_matches(|c: char| c == ' ' || c == '\t').to_string()
        } else {
            line.to_string()
        };

        out.push_str(&kept);
        out.push('\n');
    }

    out
}

fn compile_remove_regex_opt(raw: Option<&str>) -> Option<Regex> {
    raw.and_then(|s| {
        let pattern = format!("(?ms){}", s);
        Regex::new(&pattern).ok()
    })
}

fn clean_remove_regex(s: &str) -> String {
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

fn parse_extension_filters(raw: &str) -> (HashSet<String>, HashSet<String>) {
    let mut include_exts = HashSet::new();
    let mut exclude_exts = HashSet::new();

    for token in raw.split(',') {
        let mut t = token.trim().to_lowercase();
        if t.is_empty() {
            continue;
        }
        let is_exclude = t.starts_with('-');
        if is_exclude {
            t = t[1..].trim().to_string();
        }
        if !t.starts_with('.') && !t.is_empty() {
            t = format!(".{}", t);
        }
        if t.is_empty() {
            continue;
        }
        if is_exclude {
            exclude_exts.insert(t);
        } else {
            include_exts.insert(t);
        }
    }
    (include_exts, exclude_exts)
}

fn split_csv_set(s: &slint::SharedString) -> HashSet<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

fn scan_dir_to_node(
    dir: &Path,
    include_exts: &HashSet<String>,
    exclude_exts: &HashSet<String>,
    exclude_dirs: &HashSet<String>,
    exclude_files: &HashSet<String>,
) -> Node {
    let name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
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
        let name = ent
            .file_name()
            .to_string_lossy()
            .to_string();

        if ent.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            if exclude_dirs.contains(&name) {
                continue;
            }
            dirs.push(path);
        } else {
            if exclude_files.contains(&name) {
                continue;
            }
            let ext = path
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
                .unwrap_or_default();

            let matches = if !include_exts.is_empty() {
                include_exts.contains(&ext)
            } else if !exclude_exts.is_empty() {
                !exclude_exts.contains(&ext)
            } else {
                true
            };
            if matches {
                files.push(path);
            }
        }
    }

    dirs.sort();
    files.sort();

    for d in dirs {
        let child = scan_dir_to_node(&d, include_exts, exclude_exts, exclude_dirs, exclude_files);
        // show directory even if empty (consistent with Python behavior unless explicitly excluded)
        node.has_children = node.has_children || !child.children.is_empty() || child.has_children;
        node.children.push(child);
    }
    for f in files {
        node.has_children = true;
        node.children.push(Node {
            name: f.file_name().unwrap_or_default().to_string_lossy().to_string(),
            path: f,
            is_dir: false,
            children: Vec::new(),
            expanded: false,
            has_children: false,
        });
    }
    node
}

fn gather_paths_set(root: &Node) -> HashSet<PathBuf> {
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

// src/main.rs
fn flatten_tree(
    root: &Node,
    explicit: &HashMap<PathBuf, bool>,
    inherited: Option<bool>,
    level: usize,
    _scratch: &mut Vec<Row>, // unused, kept to mirror interface
) -> Vec<Row> {
    let mut rows = Vec::new();
    fn walk(
        n: &Node,
        explicit: &HashMap<PathBuf, bool>,
        inherited: Option<bool>,
        level: usize,
        rows: &mut Vec<Row>,
    ) {
        let effective = explicit.get(&n.path).copied().or(inherited).unwrap_or(false);
        let has_children = !n.children.is_empty();
        rows.push(Row {
            path: n.path.to_string_lossy().to_string().into(),
            name: n.name.clone().into(),
            level: level as i32,
            is_dir: n.is_dir,
            expanded: if n.is_dir { n.expanded } else { false },
            checked: effective,
            has_children,
        });
        if n.is_dir && n.expanded {
            let next_inherited = effective;
            for c in &n.children {
                walk(c, explicit, Some(next_inherited), level + 1, rows);
            }
        }
    }
    walk(root, explicit, inherited, level, &mut rows);
    rows
}

/* === UTILITIES === */

fn get_row_by_index(app: &AppWindow, index: usize) -> Option<Row> {
    let model = app.get_tree_model();
    let len = model.row_count();
    if index >= len {
        return None;
    }
    model.row_data(index)
}

fn set_tree_model(app: &AppWindow, rows: Vec<Row>) {
    let model = VecModel::from(rows);
    app.set_tree_model(ModelRc::new(model));
}

fn set_output(app: &AppWindow, s: &str) {
    // keep the full text for clipboard/export
    app.set_output_text(s.into());

    // feed a virtualized list (one item per line)
    let lines: Vec<slint::SharedString> = s.lines().map(|l| l.into()).collect();
    let model = slint::VecModel::from(lines);
    app.set_output_lines(slint::ModelRc::new(model));
}


fn update_last_refresh(app: &AppWindow) {
    let now_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    app.set_last_refresh(format!("Last refresh: {}", now_str).into());
}

fn is_ancestor_of(ancestor: &Path, p: &Path) -> bool {
    let anc = normalize_path(ancestor);
    let pp = normalize_path(p);
    pp.starts_with(&anc)
}

fn normalize_path(p: &Path) -> PathBuf {
    // Basic normalization: canonicalize may fail (permissions), so fallback to clean path
    dunce::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn path_to_unix(p: &Path) -> String {
    p.iter()
        .map(|c| c.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/* === POLLING / AUTO REFRESH === */

// src/main.rs
fn on_check_updates(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let (selected_dir, root_opt) = {
        let s = state.borrow();
        (s.selected_directory.clone(), s.root_node.clone())
    };
    if selected_dir.is_none() || root_opt.is_none() {
        return;
    }

    // Compare snapshots
    let (changed, new_root, new_snapshot) = {
        let s = state.borrow();
        let include = s.include_exts.clone();
        let exclude = s.exclude_exts.clone();
        let ex_dirs = s.exclude_dirs.clone();
        let ex_files = s.exclude_files.clone();
        let dir = s.selected_directory.as_ref().unwrap().clone();

        let fresh_root = scan_dir_to_node(&dir, &include, &exclude, &ex_dirs, &ex_files);
        let fresh_snapshot = gather_paths_set(&fresh_root);

        let changed = match &s.path_snapshot {
            None => true,
            Some(old) => *old != fresh_snapshot,
        };
        (changed, fresh_root, fresh_snapshot)
    };

    if changed {
        {
            let mut s = state.borrow_mut();
            s.root_node = Some(new_root);
            s.path_snapshot = Some(new_snapshot);
        }
        refresh_flat_model(app, state);
        // Schedule next check via timer automatically
        return;
    }

    // If no structure change, check modtimes for selected files
    let want_dirs_only = app.get_dirs_only();
    if !want_dirs_only {
        let selected_files = {
            let s = state.borrow();
            let mut files = Vec::new();
            collect_selected_paths(
                s.root_node.as_ref().unwrap(),
                &s.explicit_states,
                None,
                &mut files,
                &mut Vec::new(),
            );
            files
        };

        let mut update_needed = false;
        {
            let s = state.borrow();
            for fp in &selected_files {
                let current = fs::metadata(fp).ok().and_then(|m| m.modified().ok());
                if current != s.last_mod_times.get(fp).cloned().unwrap_or(None) {
                    update_needed = true;
                    break;
                }
            }
        }
        if update_needed {
            on_generate_output(app, state);
        }
    }
}


/* === SLINT-GENERATED TYPES (from .slint) ===
AppWindow, Row
The .slint file is in ../ui/app.slint and included via slint::include_modules!() at top.
*/
