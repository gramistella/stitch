use super::{AppWindow, Row};
use crate::ui::state::SharedState;
use chrono::Local;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc;

use stitch::core::{
    Node, WorkspaceSettings, clean_remove_regex, collapse_consecutive_blank_lines,
    collect_selected_paths, compile_remove_regex_opt, ensure_workspace_dir, gather_paths_set,
    is_ancestor_of, load_workspace, parse_extension_filters, parse_hierarchy_text, path_to_unix,
    render_unicode_tree_from_paths, save_workspace, scan_dir_to_node, split_prefix_list,
};

const UI_OUTPUT_CHAR_LIMIT: usize = 50_000;

/* =============================== UI Actions =============================== */

pub fn apply_selection_from_text(app: &AppWindow, state: &SharedState, text: &str) {
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
        } else if let Ok(rel) = node.path.strip_prefix(project_root) {
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

pub fn on_select_folder(app: &AppWindow, state: &SharedState) {
    if let Some(dir) = rfd::FileDialog::new().set_directory(".").pick_folder() {
        {
            let mut s = state.borrow_mut();
            s.selected_directory = Some(dir.clone());
            s.explicit_states.clear();
            s.last_mod_times.clear();
            s.fs_dirty = true;
        }

        // Ensure `.stitchworkspace/` exists and load settings if present.
        let _ = ensure_workspace_dir(&dir);
        if let Some(ws) = load_workspace(&dir) {
            // Apply persisted settings to the UI.
            app.set_ext_filter(ws.ext_filter.into());
            app.set_exclude_dirs(ws.exclude_dirs.into());
            app.set_exclude_files(ws.exclude_files.into());
            app.set_remove_prefix(ws.remove_prefix.into());
            app.set_remove_regex(ws.remove_regex.into());
            app.set_hierarchy_only(ws.hierarchy_only);
            app.set_dirs_only(ws.dirs_only);
        } else {
            // Seed a new workspace with current UI values (startup defaults).
            let seed = WorkspaceSettings {
                version: 1,
                ext_filter: app.get_ext_filter().to_string(),
                exclude_dirs: app.get_exclude_dirs().to_string(),
                exclude_files: app.get_exclude_files().to_string(),
                remove_prefix: app.get_remove_prefix().to_string(),
                remove_regex: app.get_remove_regex().to_string(),
                hierarchy_only: app.get_hierarchy_only(),
                dirs_only: app.get_dirs_only(),
            };
            let _ = save_workspace(&dir, &seed);
        }

        // With the UI now reflecting per-project settings, parse and proceed.
        parse_filters_from_ui(app, state);

        let _ = start_fs_watcher(app, state);
        rebuild_tree_and_ui(app, state);
        update_last_refresh(app);

        state.borrow_mut().fs_dirty = false;
    }
}

pub fn on_filter_changed(app: &AppWindow, state: &SharedState) {
    parse_filters_from_ui(app, state);
    rebuild_tree_and_ui(app, state);
    on_generate_output(app, state);
    update_last_refresh(app);
}

pub fn on_toggle_expand(app: &AppWindow, state: &SharedState, index: usize) {
    if let Some(row) = get_row_by_index(app, index) {
        let path = PathBuf::from(row.path.as_str());
        if toggle_node_expanded(state, &path) {
            refresh_flat_model(app, state);
        }
    }
}

pub fn on_toggle_check(app: &AppWindow, state: &SharedState, index: usize) {
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

pub fn on_generate_output(app: &AppWindow, state: &SharedState) {
    parse_filters_from_ui(app, state);

    let want_dirs_only = app.get_dirs_only();
    let hierarchy_only = app.get_hierarchy_only();

    let no_folder_selected = {
        let s = state.borrow();
        s.selected_directory.is_none() || s.root_node.is_none()
    };
    if no_folder_selected {
        set_output(app, state, "No folder selected.\n");
        update_last_refresh(app);
        return;
    }

    let (selected_files, selected_dirs, relative_paths) = {
        let s = state.borrow();
        let root = s.root_node.as_ref().unwrap();
        let mut files = Vec::new();
        let mut dirs = Vec::new();
        collect_selected_paths(root, &s.explicit_states, None, &mut files, &mut dirs);

        let selected_dir = s.selected_directory.as_ref().unwrap();
        let mut rels = Vec::new();
        if want_dirs_only {
            for d in &dirs {
                if let Ok(r) = d.strip_prefix(selected_dir)
                    && !r.as_os_str().is_empty()
                {
                    rels.push(path_to_unix(r));
                }
            }
        } else {
            for f in &files {
                if let Ok(r) = f.strip_prefix(selected_dir)
                    && !r.as_os_str().is_empty()
                {
                    rels.push(path_to_unix(r));
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
            (s.remove_prefixes.clone(), s.remove_regex.clone())
        };

        let s_dir = { state.borrow().selected_directory.clone().unwrap() };

        for fp in selected_files {
            let rel: PathBuf = fp
                .strip_prefix(&s_dir)
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|_| PathBuf::from(fp.file_name().unwrap_or_default()));

            let mut contents = match fs::read_to_string(&fp) {
                Ok(c) => c,
                Err(_) => continue,
            };

            if !remove_prefixes.is_empty() {
                contents =
                    stitch::core::strip_lines_and_inline_comments(&contents, &remove_prefixes);
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

pub fn on_copy_output(app: &AppWindow, state: &SharedState) {
    let text = { state.borrow().full_output_text.clone() };

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

pub fn rebuild_tree_and_ui(app: &AppWindow, state: &SharedState) {
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

fn refresh_flat_model(app: &AppWindow, state: &SharedState) {
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

fn parse_filters_from_ui(app: &AppWindow, state: &SharedState) {
    // Raw strings from the UI (these are what we persist)
    let ext_raw = app.get_ext_filter().to_string();
    let exclude_dirs_raw = app.get_exclude_dirs().to_string();
    let exclude_files_raw = app.get_exclude_files().to_string();
    let remove_prefix_raw = app.get_remove_prefix().to_string();
    let remove_regex_raw = app.get_remove_regex().to_string();

    // Parse extension filters
    let (include_exts, exclude_exts) = parse_extension_filters(&ext_raw);

    // Parse CSVs for names
    let mut exclude_dirs_set = split_csv_set(&exclude_dirs_raw.clone().into());
    let exclude_files_set = split_csv_set(&exclude_files_raw.clone().into());

    // Always exclude `.stitchworkspace` internally (even if user forgets)
    exclude_dirs_set.insert(".stitchworkspace".to_string());

    // Clean the regex string for compilation, but persist the raw input.
    let remove_regex_str = {
        let cleaned = clean_remove_regex(&remove_regex_raw);
        if cleaned.trim().is_empty() {
            None
        } else {
            Some(cleaned)
        }
    };

    // Update state
    {
        let mut st = state.borrow_mut();
        st.include_exts = include_exts;
        st.exclude_exts = exclude_exts;
        st.exclude_dirs = exclude_dirs_set;
        st.exclude_files = exclude_files_set;
        st.remove_prefixes = split_prefix_list(&remove_prefix_raw);
        st.remove_regex_str = remove_regex_str.clone();
        st.remove_regex = compile_remove_regex_opt(remove_regex_str.as_deref());
    }

    // Persist current UI preferences into the workspace, if we have a project
    if let Some(dir) = state.borrow().selected_directory.clone() {
        let ws = WorkspaceSettings {
            version: 1,
            ext_filter: ext_raw,
            exclude_dirs: exclude_dirs_raw,
            exclude_files: exclude_files_raw,
            remove_prefix: remove_prefix_raw,
            remove_regex: remove_regex_raw,
            hierarchy_only: app.get_hierarchy_only(),
            dirs_only: app.get_dirs_only(),
        };
        let _ = save_workspace(&dir, &ws);
    }
}

fn toggle_node_expanded(state: &SharedState, path: &Path) -> bool {
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

fn clear_descendant_explicit_states(state: &SharedState, dir: &Path) {
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

fn set_output(app: &AppWindow, state: &SharedState, s: &str) {
    let normalized = collapse_consecutive_blank_lines(s);

    {
        let mut st = state.borrow_mut();
        st.full_output_text = normalized.clone();
    }

    let total_chars = normalized.chars().count();

    #[cfg(feature = "tokens")]
    {
        app.set_output_stats(format!("{total_chars} chars • … tokens").into());

        const MAX_TOKENIZE_BYTES: usize = 16 * 1024 * 1024;
        let text = normalized.clone();
        let app_weak = app.as_weak();

        if text.len() <= MAX_TOKENIZE_BYTES {
            std::thread::spawn(move || {
                let tokens = count_tokens(&text);
                let chars = text.chars().count();
                let label = format!("{chars} chars • {tokens} tokens");
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_weak.upgrade() {
                        app.set_output_stats(label.into());
                    }
                });
            });
        } else {
            app.set_output_stats(
                format!("{total_chars} chars • (token count skipped for large output)").into(),
            );
        }
    }

    #[cfg(not(feature = "tokens"))]
    {
        let total_tokens = count_tokens(&normalized);
        app.set_output_stats(format!("{total_chars} chars • {total_tokens} tokens").into());
    }

    let displayed: String = if total_chars <= UI_OUTPUT_CHAR_LIMIT {
        normalized.clone()
    } else {
        let footer = format!(
            "\n… [truncated: showing {} of {} chars — use “Copy Output” to copy all]\n",
            UI_OUTPUT_CHAR_LIMIT, total_chars
        );
        let keep = UI_OUTPUT_CHAR_LIMIT.saturating_sub(footer.chars().count());
        let mut head: String = normalized.chars().take(keep).collect();
        head.push_str(&footer);
        head
    };

    app.set_output_text(displayed.clone().into());

    let lines: Vec<slint::SharedString> = displayed
        .lines()
        .map(|l| l.replace(' ', "\u{00A0}").into())
        .collect();

    let model = slint::VecModel::from(lines);
    app.set_output_lines(slint::ModelRc::new(model));
}

fn update_last_refresh(app: &AppWindow) {
    let now_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    app.set_last_refresh(format!("Last refresh: {}", now_str).into());
}

fn split_csv_set(s: &slint::SharedString) -> std::collections::HashSet<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

pub fn on_check_updates(app: &AppWindow, state: &SharedState) {
    let should_scan = {
        let s = state.borrow();
        s.selected_directory.is_some() && s.root_node.is_some() && s.fs_dirty
    };
    if !should_scan {
        return;
    }
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

    let want_dirs_only = app.get_dirs_only();
    if !want_dirs_only {
        on_generate_output(app, state);
    }
}

fn start_fs_watcher(app: &AppWindow, state: &SharedState) -> notify::Result<()> {
    {
        let mut s = state.borrow_mut();
        s.watcher = None;
        s.fs_event_rx = None;
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
        let state_rc = state.clone();

        s.fs_pump_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(250),
            move || {
                if let Some(app) = app_weak.upgrade() {
                    let any_relevant = {
                        let s = state_rc.borrow();
                        let Some(rx_ref) = s.fs_event_rx.as_ref() else {
                            return;
                        };

                        // Snapshot current filters
                        let project_root = match &s.selected_directory {
                            Some(p) => p.clone(),
                            None => return,
                        };
                        let include_exts = s.include_exts.clone();
                        let exclude_exts = s.exclude_exts.clone();
                        let exclude_dirs = s.exclude_dirs.clone();
                        let exclude_files = s.exclude_files.clone();

                        // Drain and check relevance with the shared helper
                        let mut relevant = false;
                        while let Ok(ev_res) = rx_ref.try_recv() {
                            if let Ok(ev) = ev_res {
                                for p in ev.paths {
                                    if stitch::core::is_event_path_relevant(
                                        &project_root,
                                        &p,
                                        &include_exts,
                                        &exclude_exts,
                                        &exclude_dirs,
                                        &exclude_files,
                                    ) {
                                        relevant = true;
                                        break;
                                    }
                                }
                            }
                        }
                        relevant
                    };

                    if any_relevant {
                        state_rc.borrow_mut().fs_dirty = true;
                        on_check_updates(&app, &state_rc);
                    }
                }
            },
        );
    }

    Ok(())
}

/* ============================ Token counting ============================ */

#[cfg(all(feature = "ui", feature = "tokens"))]
fn count_tokens(text: &str) -> usize {
    use std::sync::OnceLock;
    use tiktoken_rs::{CoreBPE, o200k_base};
    static BPE: OnceLock<CoreBPE> = OnceLock::new();
    let bpe = BPE.get_or_init(|| o200k_base().expect("failed to load o200k_base BPE"));
    bpe.encode_with_special_tokens(text).len()
}

#[cfg(all(feature = "ui", not(feature = "tokens")))]
fn count_tokens(text: &str) -> usize {
    text.split_whitespace().filter(|s| !s.is_empty()).count()
}
