#![allow(clippy::needless_return)]

#[cfg(feature = "ui")]
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    time::SystemTime,
};

#[cfg(feature = "ui")]
use chrono::Local;

#[cfg(feature = "ui")]
use regex::Regex;

#[cfg(feature = "ui")]
use slint::{Model, ModelRc, VecModel};

// 🔽 bring in the UI-generated types
#[cfg(feature = "ui")]
slint::include_modules!();

// 🔽 use our new core module (pure logic)
#[cfg(feature = "ui")]
use stitch::core::{
    Node, clean_remove_regex, collapse_consecutive_blank_lines, collect_selected_paths,
    compile_remove_regex_opt, drain_channel_nonblocking, gather_paths_set, is_ancestor_of,
    parse_extension_filters, parse_hierarchy_text, path_to_unix, render_unicode_tree_from_paths,
    scan_dir_to_node, split_prefix_list, strip_lines_and_inline_comments,
};

#[cfg(feature = "ui")]
const UI_OUTPUT_CHAR_LIMIT: usize = 10_000;

#[cfg(feature = "ui")]
#[derive(Default)]
struct AppState {
    selected_directory: Option<PathBuf>,
    root_node: Option<Node>,
    explicit_states: HashMap<PathBuf, bool>,
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
    select_dialog: Option<SelectFromTextDialog>,
    fs_dirty: bool,
    watcher: Option<notify::RecommendedWatcher>,
    fs_event_rx: Option<std::sync::mpsc::Receiver<notify::Result<notify::Event>>>,
    fs_pump_timer: slint::Timer,
    full_output_text: String,
}

#[cfg(feature = "ui")]
fn main() -> anyhow::Result<()> {
    let app = AppWindow::new()?;

    app.set_app_version(env!("CARGO_PKG_VERSION").into());
    app.set_ext_filter("".into());
    app.set_exclude_dirs(".git, node_modules, target, _target, .elan, .lake, .idea, .vscode, _app, .svelte-kit, .sqlx, venv, .venv, __pycache__, LICENSES, fixtures".into());
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
        poll_interval_ms: 45_000,
        ..Default::default()
    }));

    let poll_timer = slint::Timer::default();
    {
        let app_weak = app.as_weak();
        let state = Rc::clone(&state);
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

    {
        let app_weak = app.as_weak();
        let state = Rc::clone(&state);

        app.on_select_from_text(move || {
            if let Some(dlg) = state.borrow().select_dialog.as_ref() {
                dlg.set_text("".into());
                let _ = dlg.show();
                return;
            }

            let dlg = SelectFromTextDialog::new().expect("create SelectFromTextDialog");
            dlg.set_text("".into());

            let dlg_weak_apply = dlg.as_weak();
            let state_apply = Rc::clone(&state);
            let app_weak_apply = app_weak.clone();
            dlg.on_apply(move |text| {
                if let Some(app) = app_weak_apply.upgrade() {
                    apply_selection_from_text(&app, &state_apply, text.as_ref());
                }
                if let Some(d) = dlg_weak_apply.upgrade() {
                    let _ = d.hide();
                }
            });

            let dlg_weak_cancel = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = dlg_weak_cancel.upgrade() {
                    let _ = d.hide();
                }
            });

            state.borrow_mut().select_dialog = Some(dlg);
            let _ = state.borrow().select_dialog.as_ref().unwrap().show();
        });
    }

    app.run()?;
    Ok(())
}

// When the UI feature is disabled, provide a tiny stub so the bin compiles.
#[cfg(not(feature = "ui"))]
fn main() -> anyhow::Result<()> {
    eprintln!(
        "Built without the `ui` feature; nothing to run. \
Enable it with `--features ui`, or just run tests with `--no-default-features`."
    );
    Ok(())
}

#[cfg(feature = "ui")]
fn apply_selection_from_text(app: &AppWindow, state: &Rc<RefCell<AppState>>, text: &str) {
    let (root_opt, selected_dir_opt) = {
        let s = state.borrow();
        (s.root_node.clone(), s.selected_directory.clone())
    };
    let (root, selected_dir) = match (root_opt, selected_dir_opt) {
        (Some(r), Some(d)) => (r, d),
        _ => return,
    };

    let wanted = match parse_hierarchy_text(text) {
        Some(s) if !s.is_empty() => s,
        _ => return,
    };

    {
        let mut s = state.borrow_mut();
        s.explicit_states.clear();
    }

    fn walk_and_mark(
        node: &Node,
        project_root: &Path,
        wanted: &std::collections::HashSet<String>,
        explicit: &mut std::collections::HashMap<PathBuf, bool>,
    ) {
        if node.is_dir {
            for c in &node.children {
                walk_and_mark(c, project_root, wanted, explicit);
            }
        } else if let Some(rel) = pathdiff::diff_paths(&node.path, project_root) {
            let key = rel
                .iter()
                .map(|c| c.to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            if wanted.contains(&key) {
                explicit.insert(node.path.clone(), true);
            }
        }
    }

    {
        let mut s = state.borrow_mut();
        walk_and_mark(&root, &selected_dir, &wanted, &mut s.explicit_states);
    }

    refresh_flat_model(app, state);
    on_generate_output(app, state);
}

#[cfg(feature = "ui")]
fn on_select_folder(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    if let Some(dir) = rfd::FileDialog::new().set_directory(".").pick_folder() {
        {
            let mut s = state.borrow_mut();
            s.selected_directory = Some(dir.clone());
            s.explicit_states.clear();
            s.last_mod_times.clear();
            s.fs_dirty = true; // initial build trigger (manual rebuild happens next)
        }
        let _ = start_fs_watcher(app, state);

        rebuild_tree_and_ui(app, state);
        update_last_refresh(app);

        // Prevent the pump (250ms) from immediately rescanning the tree we just rebuilt.
        state.borrow_mut().fs_dirty = false;
    }
}

#[cfg(feature = "ui")]
fn on_filter_changed(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    parse_filters_from_ui(app, state);
    rebuild_tree_and_ui(app, state);
    on_generate_output(app, state);
    update_last_refresh(app);
}

#[cfg(feature = "ui")]
fn on_toggle_expand(app: &AppWindow, state: &Rc<RefCell<AppState>>, index: usize) {
    if let Some(row) = get_row_by_index(app, index) {
        let path = PathBuf::from(row.path.as_str());
        if toggle_node_expanded(state, &path) {
            refresh_flat_model(app, state);
        }
    }
}

#[cfg(feature = "ui")]
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

#[cfg(feature = "ui")]
fn on_generate_output(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    parse_filters_from_ui(app, state);

    let want_dirs_only = app.get_dirs_only();
    let hierarchy_only = app.get_hierarchy_only();

    let (selected_files, selected_dirs, relative_paths) = {
        let s = state.borrow();
        if s.selected_directory.is_none() || s.root_node.is_none() {
            set_output(app, state, "No folder selected.\n");
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
                if let Some(r) = pathdiff::diff_paths(d, selected_dir)
                    && r != PathBuf::from("")
                {
                    rels.push(path_to_unix(&r));
                }
            }
        } else {
            for f in &files {
                if let Some(r) = pathdiff::diff_paths(f, selected_dir)
                    && r != PathBuf::from("")
                {
                    rels.push(path_to_unix(&r));
                }
            }
        }
        (files, dirs, rels)
    };

    if (!want_dirs_only && selected_files.is_empty())
        || (want_dirs_only && selected_dirs.is_empty())
    {
        set_output(app, state, "No items selected.\n");
        update_last_refresh(app);
        return;
    }

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

    let root_name = {
        let s = state.borrow();
        s.selected_directory
            .as_ref()
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or_else(|| "root".into())
    };
    out.push_str(&render_unicode_tree_from_paths(
        &relative_paths,
        Some(&root_name),
    ));

    if !hierarchy_only && !app.get_dirs_only() {
        out.push_str("\n=== FILE CONTENTS ===\n\n");

        let (remove_prefixes, remove_regex_opt) = {
            let s = state.borrow();
            let rp = s.remove_prefixes.clone();
            let rr = s.remove_regex.clone();
            (rp, rr)
        };

        let s_dir = state.borrow().selected_directory.clone().unwrap();

        for fp in selected_files {
            let rel = pathdiff::diff_paths(&fp, &s_dir)
                .unwrap_or_else(|| PathBuf::from(fp.file_name().unwrap_or_default()));

            let mut contents = match fs::read_to_string(&fp) {
                Ok(c) => c,
                Err(_) => continue,
            };

            if !remove_prefixes.is_empty() {
                contents = strip_lines_and_inline_comments(&contents, &remove_prefixes);
            }
            if let Some(rr) = &remove_regex_opt {
                contents = rr.replace_all(&contents, "").to_string();
            }

            out.push_str(&format!(
                "--- Start of file: {} ---\n",
                rel.to_string_lossy()
            ));
            out.push_str(&contents);
            out.push('\n');
            out.push_str(&format!(
                "--- End of file: {} ---\n\n",
                rel.to_string_lossy()
            ));
        }
    }

    set_output(app, state, &out);
    update_last_refresh(app);
}

#[cfg(feature = "ui")]
fn on_copy_output(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let text = app.get_output_text().to_string();
    if text.is_empty() {
        app.set_copy_toast_text("Nothing to copy".into());
        app.set_show_copy_toast(true);

        {
            let s = state.borrow_mut();
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

    let mut ok = false;
    if let Ok(mut cb) = arboard::Clipboard::new() {
        ok = cb.set_text(text).is_ok();
    }

    app.set_copy_toast_text(if ok { "Copied!" } else { "Copy failed" }.into());
    app.set_show_copy_toast(true);

    {
        let s = state.borrow_mut();
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

#[cfg(feature = "ui")]
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

        {
            let mut s = state.borrow_mut();
            s.path_snapshot = Some(snapshot);
            s.root_node = Some(root);
            s.remove_regex = compile_remove_regex_opt(s.remove_regex_str.as_deref());
        }
    }

    refresh_flat_model(app, state);
}

#[cfg(feature = "ui")]
fn refresh_flat_model(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let rows = {
        let s = state.borrow();
        if let Some(root) = &s.root_node {
            flatten_tree(root, &s.explicit_states, None, 0)
        } else {
            Vec::new()
        }
    };
    set_tree_model(app, rows);
}

#[cfg(feature = "ui")]
fn parse_filters_from_ui(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let ext_raw = app.get_ext_filter().to_string();
    let (include_exts, exclude_exts) = parse_extension_filters(&ext_raw);

    let exclude_dirs = split_csv_set(&app.get_exclude_dirs());
    let exclude_files = split_csv_set(&app.get_exclude_files());

    let remove_prefixes = split_prefix_list(&app.get_remove_prefix());

    let remove_regex_str = {
        let raw = app.get_remove_regex().to_string();
        let cleaned = clean_remove_regex(&raw);
        if cleaned.trim().is_empty() {
            None
        } else {
            Some(cleaned)
        }
    };

    let mut st = state.borrow_mut();
    st.include_exts = include_exts;
    st.exclude_exts = exclude_exts;
    st.exclude_dirs = exclude_dirs;
    st.exclude_files = exclude_files;
    st.remove_prefixes = remove_prefixes;
    st.remove_regex_str = remove_regex_str.clone();
    st.remove_regex = compile_remove_regex_opt(remove_regex_str.as_deref());
}

#[cfg(feature = "ui")]
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

#[cfg(feature = "ui")]
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

#[cfg(feature = "ui")]
fn flatten_tree(
    root: &Node,
    explicit: &HashMap<PathBuf, bool>,
    inherited: Option<bool>,
    level: usize,
) -> Vec<Row> {
    let mut rows = Vec::new();
    fn walk(
        n: &Node,
        explicit: &HashMap<PathBuf, bool>,
        inherited: Option<bool>,
        level: usize,
        rows: &mut Vec<Row>,
    ) {
        let effective = explicit
            .get(&n.path)
            .copied()
            .or(inherited)
            .unwrap_or(false);
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

#[cfg(feature = "ui")]
fn get_row_by_index(app: &AppWindow, index: usize) -> Option<Row> {
    let model = app.get_tree_model();
    let len = model.row_count();
    if index >= len {
        return None;
    }
    model.row_data(index)
}

#[cfg(feature = "ui")]
fn set_tree_model(app: &AppWindow, rows: Vec<Row>) {
    let model = VecModel::from(rows);
    app.set_tree_model(ModelRc::new(model));
}

#[cfg(feature = "ui")]
fn set_output(app: &AppWindow, state: &Rc<RefCell<AppState>>, s: &str) {
    let normalized = collapse_consecutive_blank_lines(s);

    // store the full output for "Copy Output"
    {
        let mut st = state.borrow_mut();
        st.full_output_text = normalized.clone();
    }

    let total_chars = normalized.chars().count();

    // Build the displayed string (≤ limit) and add a concise footer if truncated
    let displayed: String = if total_chars <= UI_OUTPUT_CHAR_LIMIT {
        normalized.clone()
    } else {
        let footer = format!(
            "\n… [truncated: showing {} of {} chars — use “Copy Output” to copy all]\n",
            UI_OUTPUT_CHAR_LIMIT, total_chars
        );
        // Ensure we stay within the hard UI limit, including the footer itself
        let keep = UI_OUTPUT_CHAR_LIMIT.saturating_sub(footer.chars().count());
        let mut head: String = normalized.chars().take(keep).collect();
        head.push_str(&footer);
        head
    };

    app.set_output_text(displayed.clone().into());

    // keep the side "lines" panel synced with what’s displayed
    let lines: Vec<slint::SharedString> = displayed
        .lines()
        .map(|l| l.replace(' ', "\u{00A0}").into())
        .collect();

    let model = slint::VecModel::from(lines);
    app.set_output_lines(slint::ModelRc::new(model));
}

#[cfg(feature = "ui")]
fn update_last_refresh(app: &AppWindow) {
    let now_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    app.set_last_refresh(format!("Last refresh: {}", now_str).into());
}

#[cfg(feature = "ui")]
fn split_csv_set(s: &slint::SharedString) -> HashSet<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

#[cfg(feature = "ui")]
fn on_check_updates(app: &AppWindow, state: &Rc<RefCell<AppState>>) {
    // Fast bail-out when nothing changed.
    let should_scan = {
        let s = state.borrow();
        s.selected_directory.is_some() && s.root_node.is_some() && s.fs_dirty
    };
    if !should_scan {
        return;
    }
    // clear dirty before work (events arriving during scan will re-mark it)
    {
        state.borrow_mut().fs_dirty = false;
    }

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
        return;
    }

    // If the tree didn't structurally change but files were modified,
    // regenerate output only when not in "dirs only".
    let want_dirs_only = app.get_dirs_only();
    if !want_dirs_only {
        on_generate_output(app, state);
    }
}

#[cfg(feature = "ui")]
fn start_fs_watcher(app: &AppWindow, state: &Rc<RefCell<AppState>>) -> notify::Result<()> {
    use notify::{RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;

    {
        let mut s = state.borrow_mut();
        s.watcher = None;
        s.fs_event_rx = None;
        // (optional) stop any prior pump before restarting
        // s.fs_pump_timer.stop(); // call if available in your Slint version
    }

    let root = {
        let s = state.borrow();
        match &s.selected_directory {
            Some(p) => p.clone(),
            None => return Ok(()),
        }
    };

    let (tx, rx) = mpsc::channel();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(tx)?;
    watcher.watch(&root, RecursiveMode::Recursive)?;

    {
        let mut s = state.borrow_mut();
        s.watcher = Some(watcher);
        s.fs_event_rx = Some(rx);

        let app_weak = app.as_weak();
        let state_rc = Rc::clone(state);
        s.fs_pump_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(250),
            move || {
                if let Some(app) = app_weak.upgrade() {
                    let any = {
                        let s = state_rc.borrow();
                        s.fs_event_rx
                            .as_ref()
                            .map(drain_channel_nonblocking)
                            .unwrap_or(false)
                    };
                    if any {
                        state_rc.borrow_mut().fs_dirty = true;
                        on_check_updates(&app, &state_rc);
                    }
                }
            },
        );
    }

    Ok(())
}
