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
    Node, Profile, ProfileScope, RustFilterOptions, WorkspaceSettings, apply_rust_filters,
    clean_remove_regex, collapse_consecutive_blank_lines, collect_selected_paths,
    compile_remove_regex_opt, delete_profile, ensure_profiles_dirs, ensure_workspace_dir,
    gather_paths_set, is_ancestor_of, is_rust_file_path, list_profiles, load_local_settings,
    load_profile, load_workspace, parse_extension_filters, parse_hierarchy_text, path_to_unix,
    render_unicode_tree_from_paths, save_local_settings, save_profile, save_workspace,
    scan_dir_to_node, signatures_filter_matches, split_prefix_list,
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

    // No autosave – let the user save; just update button state
    update_save_button_state(app, state);
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

        app.set_project_path(format_project_path_for_title(&dir).into());

        let _ = ensure_workspace_dir(&dir);
        let _ = ensure_profiles_dirs(&dir);

        let ws_opt = load_workspace(&dir);
        if let Some(ws) = ws_opt.as_ref() {
            app.set_ext_filter(ws.ext_filter.clone().into());
            app.set_exclude_dirs(ws.exclude_dirs.clone().into());
            app.set_exclude_files(ws.exclude_files.clone().into());
            app.set_remove_prefix(ws.remove_prefix.clone().into());
            app.set_remove_regex(ws.remove_regex.clone().into());
            app.set_hierarchy_only(ws.hierarchy_only);
            app.set_dirs_only(ws.dirs_only);
            app.set_show_rust_section(false);
            app.set_rust_remove_inline_comments(ws.rust_remove_inline_comments);
            app.set_rust_remove_doc_comments(ws.rust_remove_doc_comments);
            app.set_rust_function_signatures_only(ws.rust_function_signatures_only);
            app.set_rust_signatures_only_filter(ws.rust_signatures_only_filter.clone().into());

            state.borrow_mut().workspace_baseline = Some(ws.clone());
        } else {
            let seed = WorkspaceSettings {
                version: 1,
                ext_filter: app.get_ext_filter().to_string(),
                exclude_dirs: app.get_exclude_dirs().to_string(),
                exclude_files: app.get_exclude_files().to_string(),
                remove_prefix: app.get_remove_prefix().to_string(),
                remove_regex: app.get_remove_regex().to_string(),
                hierarchy_only: app.get_hierarchy_only(),
                dirs_only: app.get_dirs_only(),
                rust_remove_inline_comments: app.get_rust_remove_inline_comments(),
                rust_remove_doc_comments: app.get_rust_remove_doc_comments(),
                rust_function_signatures_only: app.get_rust_function_signatures_only(),
                rust_signatures_only_filter: app.get_rust_signatures_only_filter().to_string(),
            };
            let _ = save_workspace(&dir, &seed);
            state.borrow_mut().workspace_baseline = Some(seed);
        }

        {
            let mut s = state.borrow_mut();
            s.profiles = list_profiles(&dir);
        }
        refresh_profiles_ui(app, state);

        if let Some(local_settings) = load_local_settings(&dir)
            && let Some(name) = local_settings.current_profile
        {
            if let Some((profile, _)) = load_profile(&dir, &name) {
                apply_profile_to_ui(app, state, &profile);
            } else {
                let _ = stitch::core::clear_stale_current_profile(&dir);
                refresh_profiles_ui(app, state);
            }
        }

        parse_filters_from_ui(app, state);

        // Only start fs watcher if it's not disabled
        if !state.borrow().disable_fs_watcher {
            let _ = start_fs_watcher(app, state);
        }
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

        // Reflect unsaved changes instead of autosaving
        update_save_button_state(app, state);
    }
}

pub fn on_toggle_fs_watcher(app: &AppWindow, state: &SharedState) {
    let disable_fs_watcher = app.get_disable_fs_watcher();

    {
        let mut s = state.borrow_mut();
        s.disable_fs_watcher = disable_fs_watcher;
    }

    if disable_fs_watcher {
        // Stop the fs watcher
        {
            let mut s = state.borrow_mut();
            s.watcher = None;
            s.fs_event_rx = None;
        }
    } else {
        // Start the fs watcher
        let _ = start_fs_watcher(app, state);
    }
}

pub fn on_generate_output(app: &AppWindow, state: &SharedState) {
    // Coalesce clicks: if a run is active, queue a single re-run and show a tiny placeholder.
    {
        let mut s = state.borrow_mut();
        if s.is_generating {
            s.regen_after = true;
            app.set_output_text("⏳ Generating… (queued)".into());
            app.set_output_stats("".into());
            return;
        }
    }

    parse_filters_from_ui(app, state);

    let want_dirs_only = app.get_dirs_only();
    let hierarchy_only = app.get_hierarchy_only();
    let disable_notes = app.get_disable_notes_section();

    // Basic guards
    let no_folder_selected = {
        let s = state.borrow();
        s.selected_directory.is_none() || s.root_node.is_none()
    };
    if no_folder_selected {
        set_output(app, state, NO_FOLDER_SELECTED);
        update_last_refresh(app);
        return;
    }

    // Collect the selection & render the hierarchy header on the UI thread (cheap)
    let (selected_files, selected_dirs, relative_paths, selected_dir, root_name) = {
        let s = state.borrow();
        let root = s.root_node.as_ref().unwrap();
        let mut files = Vec::new();
        let mut dirs = Vec::new();
        collect_selected_paths(root, &s.explicit_states, None, &mut files, &mut dirs);

        let selected_dir = s.selected_directory.as_ref().unwrap().clone();

        let mut rels = Vec::new();
        if want_dirs_only {
            for d in &dirs {
                if let Ok(r) = d.strip_prefix(&selected_dir)
                    && !r.as_os_str().is_empty()
                {
                    rels.push(path_to_unix(r));
                }
            }
        } else {
            for f in &files {
                if let Ok(r) = f.strip_prefix(&selected_dir)
                    && !r.as_os_str().is_empty()
                {
                    rels.push(path_to_unix(r));
                }
            }
        }

        let root_name = selected_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        (files, dirs, rels, selected_dir, root_name)
    };

    if (!want_dirs_only && selected_files.is_empty())
        || (want_dirs_only && selected_dirs.is_empty())
    {
        set_output(app, state, NO_ITEMS_SELECTED);
        update_last_refresh(app);
        return;
    }

    // Touch mtimes (cheap metadata)
    if !want_dirs_only {
        let mut s = state.borrow_mut();
        for fp in &selected_files {
            let mtime = fs::metadata(fp).ok().and_then(|m| m.modified().ok());
            s.last_mod_times.insert(fp.clone(), mtime);
        }
    }

    // Render the hierarchy now (instant feedback)
    let mut header = String::new();
    header.push_str("=== FILE HIERARCHY ===\n\n");
    header.push_str(&render_unicode_tree_from_paths(
        &relative_paths,
        Some(&root_name),
    ));

    // Optional Notes section
    if !disable_notes {
        let notes = build_notes_section(state, &selected_dir, &relative_paths);
        if !notes.trim().is_empty() {
            header.push_str("\n=== NOTES ===\n\n");
            header.push_str(&notes);
            header.push('\n');
        }
    }

    // If only hierarchy/dirs, finish synchronously.
    if hierarchy_only || want_dirs_only {
        set_output(app, state, &header);
        update_last_refresh(app);
        return;
    }

    // ---- Heavy path: read + scrub files on a worker thread ----

    // Tiny in-place placeholder (don’t clobber state.full_output_text; set_output() will do that later)
    app.set_output_text("⏳ Generating…".into());
    app.set_output_stats("".into());

    // Snapshot params for the worker
    let remove_prefixes = { state.borrow().remove_prefixes.clone() };
    let remove_regex_opt = { state.borrow().remove_regex.clone() };
    let files_to_read = selected_files;
    let selected_dir_for_rel = selected_dir;
    let rust_opts = {
        let s = state.borrow();
        RustFilterOptions {
            remove_inline_regular_comments: s.rust_remove_inline_comments,
            remove_doc_comments: s.rust_remove_doc_comments,
            function_signatures_only: s.rust_function_signatures_only,
        }
    };
    let rust_sig_filter = app.get_rust_signatures_only_filter().to_string();

    // Ensure a result channel & start a UI-thread pump (if not already running)
    {
        let mut s = state.borrow_mut();
        if s.gen_result_tx.is_none() || s.gen_result_rx.is_none() {
            let (tx, rx) = mpsc::channel::<(u64, String)>();
            s.gen_result_tx = Some(tx);
            s.gen_result_rx = Some(rx);

            let app_weak = app.as_weak();
            let state_rc = state.clone();
            s.gen_pump_timer.start(
                slint::TimerMode::Repeated,
                std::time::Duration::from_millis(120),
                move || {
                    if let (Some(app), Some(out)) = (app_weak.upgrade(), {
                        // Drain to the latest message, if any
                        let mut last: Option<(u64, String)> = None;
                        if let Some(rx) = state_rc.borrow().gen_result_rx.as_ref() {
                            while let Ok(msg) = rx.try_recv() {
                                last = Some(msg);
                            }
                        }
                        last.map(|(_, s)| s)
                    }) {
                        // Apply final output on the UI thread
                        set_output(&app, &state_rc, &out);
                        update_last_refresh(&app);

                        // Flip flags and maybe coalesced re-run
                        let rerun = {
                            let mut st = state_rc.borrow_mut();
                            st.is_generating = false;
                            let again = st.regen_after;
                            st.regen_after = false;
                            again
                        };

                        if rerun {
                            on_generate_output(&app, &state_rc);
                        }
                    }
                },
            );
        }

        s.is_generating = true;
        s.regen_after = false;
        s.gen_seq = s.gen_seq.wrapping_add(1);
    }
    let my_seq = { state.borrow().gen_seq };

    // Spawn worker (captures only Send things)
    let tx = { state.borrow().gen_result_tx.as_ref().unwrap().clone() };
    std::thread::spawn(move || {
        let mut out = header;
        out.push_str("\n=== FILE CONTENTS ===\n\n");

        for fp in files_to_read {
            let rel: PathBuf = fp
                .strip_prefix(&selected_dir_for_rel).map_or_else(|_| PathBuf::from(fp.file_name().unwrap_or_default()), std::path::Path::to_path_buf);

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

            if is_rust_file_path(&fp) {
                // Signatures-only may be restricted via filter; compute rel path for matching
                let rel_for_match = rel
                    .iter()
                    .map(|c| c.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                let mut eff = rust_opts.clone();
                if !rust_sig_filter.trim().is_empty()
                    && !signatures_filter_matches(&rel_for_match, &rust_sig_filter)
                {
                    eff.function_signatures_only = false;
                }
                contents = apply_rust_filters(&contents, &eff);
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

        // Send final result back to the UI thread pump
        let _ = tx.send((my_seq, out));
    });
}

fn build_notes_section(
    state: &SharedState,
    _project_root: &std::path::Path,
    rel_selected_paths: &[String],
) -> String {
    use std::collections::BTreeSet;
    let s = state.borrow();
    let mut lines: Vec<String> = Vec::new();

    // Helpers: presence filtering
    let selected_set: BTreeSet<String> = rel_selected_paths.iter().cloned().collect();
    let exists_rel_file = |rel: &str| selected_set.contains(rel);
    let exists_rel_dir = |rel: &str| {
        selected_set
            .iter()
            .any(|p| p == rel || p.starts_with(&(rel.to_string() + "/")))
    };

    // Excluded directories that actually exist
    if !s.exclude_dirs.is_empty() {
        let mut present: Vec<String> = s
            .exclude_dirs
            .iter()
            .filter_map(|d| {
                let drel = d.as_str();
                if exists_rel_dir(drel) {
                    Some(drel.to_string())
                } else {
                    None
                }
            })
            .collect();
        present.sort();
        if !present.is_empty() {
            lines.push(format!("Excluded directories: {}", present.join(", ")));
        }
    }

    // Excluded files that actually exist
    if !s.exclude_files.is_empty() {
        let mut present: Vec<String> = s
            .exclude_files
            .iter()
            .filter_map(|f| {
                if exists_rel_file(f) {
                    Some(f.clone())
                } else {
                    None
                }
            })
            .collect();
        present.sort();
        if !present.is_empty() {
            lines.push(format!("Excluded files: {}", present.join(", ")));
        }
    }

    // Extension filters (only note those that actually apply to selected files)
    if !s.include_exts.is_empty() {
        let mut present: BTreeSet<String> = BTreeSet::new();
        for rel in rel_selected_paths {
            if let Some(ext) = std::path::Path::new(rel)
                .extension()
                .and_then(|e| e.to_str())
            {
                let dot = format!(".{}", ext.to_lowercase());
                if s.include_exts.contains(&dot) {
                    present.insert(dot);
                }
            }
        }
        if !present.is_empty() {
            lines.push(format!(
                "Included extensions: {}",
                present.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
    }
    if !s.exclude_exts.is_empty() {
        let mut present: BTreeSet<String> = BTreeSet::new();
        for rel in rel_selected_paths {
            if let Some(ext) = std::path::Path::new(rel)
                .extension()
                .and_then(|e| e.to_str())
            {
                let dot = format!(".{}", ext.to_lowercase());
                if s.exclude_exts.contains(&dot) {
                    present.insert(dot);
                }
            }
        }
        if !present.is_empty() {
            lines.push(format!(
                "Excluded extensions: {}",
                present.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
    }

    // Remove prefixes (only if enabled and selected files contain any line starting with them). We keep a light note.
    if !s.remove_prefixes.is_empty() {
        lines.push(format!(
            "Removed lines starting with: {}",
            s.remove_prefixes.join(", ")
        ));
    }
    if s.remove_regex.is_some() {
        lines.push("Applied remove-regex".to_string());
    }

    // Rust-specific notes, with scoping
    if s.rust_remove_inline_comments {
        lines.push("Removed Rust inline comments (//, /* */)".to_string());
    }
    if s.rust_remove_doc_comments {
        lines.push("Removed Rust doc comments (///, //!, /** */)".to_string());
    }
    if s.rust_function_signatures_only {
        if s.rust_signatures_only_filter.trim().is_empty() {
            lines.push("Functions bodies omitted (signatures only) for all Rust files".to_string());
        } else {
            lines.push(format!(
                "Functions bodies omitted (signatures only) for: {}",
                s.rust_signatures_only_filter
            ));
        }
    }

    lines.join("\n")
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

    // Detect presence of any .rs file to toggle Rust section visibility
    let has_rs = {
        let s = state.borrow();
        let mut any = false;
        if let Some(root) = &s.root_node {
            fn rec(n: &Node, any: &mut bool) {
                if *any {
                    return;
                }
                if !n.is_dir
                    && n.path.extension().and_then(|e| e.to_str()) == Some("rs") {
                        *any = true;
                        return;
                    }
                for c in &n.children {
                    rec(c, any);
                }
            }
            rec(root, &mut any);
        }
        any
    };
    app.set_show_rust_section(has_rs);
    {
        let mut s = state.borrow_mut();
        s.has_rust_files = has_rs;
    }
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
    let ext_raw = app.get_ext_filter().to_string();
    let exclude_dirs_raw = app.get_exclude_dirs().to_string();
    let exclude_files_raw = app.get_exclude_files().to_string();
    let remove_prefix_raw = app.get_remove_prefix().to_string();
    let remove_regex_raw = app.get_remove_regex().to_string();

    let (include_exts, exclude_exts) = parse_extension_filters(&ext_raw);

    let mut exclude_dirs_set = split_csv_set(&exclude_dirs_raw.into());
    let exclude_files_set = split_csv_set(&exclude_files_raw.into());

    exclude_dirs_set.insert(".stitchworkspace".to_string());

    let remove_regex_str = {
        let cleaned = clean_remove_regex(&remove_regex_raw);
        if cleaned.trim().is_empty() {
            None
        } else {
            Some(cleaned)
        }
    };

    {
        let mut st = state.borrow_mut();
        st.include_exts = include_exts;
        st.exclude_exts = exclude_exts;
        st.exclude_dirs = exclude_dirs_set;
        st.exclude_files = exclude_files_set;
        st.remove_prefixes = split_prefix_list(&remove_prefix_raw);
        st.remove_regex_str = remove_regex_str.clone();
        st.remove_regex = compile_remove_regex_opt(remove_regex_str.as_deref());
        st.rust_remove_inline_comments = app.get_rust_remove_inline_comments();
        st.rust_remove_doc_comments = app.get_rust_remove_doc_comments();
        st.rust_function_signatures_only = app.get_rust_function_signatures_only();
        st.rust_signatures_only_filter = app.get_rust_signatures_only_filter().to_string();
    }

    let Some(dir) = state.borrow().selected_directory.clone() else {
        app.set_save_enabled(false);
        return;
    };
    let idx = app.get_selected_profile_index();

    if idx < 0 {
        app.set_save_enabled(false);
        return;
    }

    if idx > 0
        && let mut local_settings = load_local_settings(&dir).unwrap_or_default()
        && let Some(meta) = state.borrow().profiles.get((idx as usize) - 1)
    {
        local_settings.current_profile = Some(meta.name.clone());
        let _ = save_local_settings(&dir, &local_settings);
    }

    update_save_button_state(app, state);
}

fn toggle_node_expanded(state: &SharedState, path: &Path) -> bool {
    fn rec(n: &mut Node, target: &Path) -> bool {
        if n.path == target {
            if n.is_dir {
                n.expanded = !n.expanded;
                return true;
            }
            return false;
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

    // Check if this is a placeholder message that shouldn't count towards stats
    let is_placeholder = is_placeholder_message(&normalized);

    let total_chars = if is_placeholder {
        0
    } else {
        normalized.chars().count()
    };
    let total_lines = if is_placeholder || normalized.is_empty() {
        0
    } else {
        normalized.lines().count()
    };

    #[cfg(feature = "tokens")]
    {
        app.set_output_stats(format!("{total_chars} chars • … tokens • {total_lines} LOC").into());

        const MAX_TOKENIZE_BYTES: usize = 16 * 1024 * 1024;
        let text = normalized.clone();
        let app_weak = app.as_weak();

        if text.len() <= MAX_TOKENIZE_BYTES {
            std::thread::spawn(move || {
                let is_placeholder = is_placeholder_message(&text);
                let tokens = if is_placeholder {
                    0
                } else {
                    count_tokens(&text)
                };
                let chars = if is_placeholder {
                    0
                } else {
                    text.chars().count()
                };
                let lines = if is_placeholder || text.is_empty() {
                    0
                } else {
                    text.lines().count()
                };
                let label = format!("{chars} chars • {tokens} tokens • {lines} LOC");
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_weak.upgrade() {
                        app.set_output_stats(label.into());
                    }
                });
            });
        } else {
            app.set_output_stats(
                format!("{total_chars} chars • (token count skipped for large output) • {total_lines} LOC").into(),
            );
        }
    }

    #[cfg(not(feature = "tokens"))]
    {
        let total_tokens = if is_placeholder {
            0
        } else {
            count_tokens(&normalized)
        };
        app.set_output_stats(
            format!("{total_chars} chars • {total_tokens} tokens • {total_lines} LOC").into(),
        );
    }

    let displayed: String = if total_chars <= UI_OUTPUT_CHAR_LIMIT {
        normalized
    } else {
        let footer = format!(
            "\n… [truncated: showing {UI_OUTPUT_CHAR_LIMIT} of {total_chars} chars — use “Copy Output” to copy all]\n"
        );
        let keep = UI_OUTPUT_CHAR_LIMIT.saturating_sub(footer.chars().count());
        let mut head: String = normalized.chars().take(keep).collect();
        head.push_str(&footer);
        head
    };

    app.set_output_text(displayed.into());
}

fn update_last_refresh(app: &AppWindow) {
    let now_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    app.set_last_refresh(format!("Last refresh: {now_str}").into());
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

/* ============================ Placeholder detection ============================ */

// Constants for placeholder messages that shouldn't count towards statistics
const NO_FOLDER_SELECTED: &str = "No folder selected.\n";
const NO_ITEMS_SELECTED: &str = "No items selected.\n";

/// Determines if the given text is a placeholder message that shouldn't count towards statistics
fn is_placeholder_message(text: &str) -> bool {
    text == NO_FOLDER_SELECTED || text == NO_ITEMS_SELECTED
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

fn refresh_profiles_ui(app: &AppWindow, state: &SharedState) {
    // If no folder is selected: show an empty ComboBox and no selection
    let no_folder = { state.borrow().selected_directory.is_none() };
    if no_folder {
        app.set_profiles(slint::ModelRc::new(slint::VecModel::from(Vec::<
            slint::SharedString,
        >::new())));
        app.set_selected_profile_index(-1);
        return;
    }

    let (names, desired_idx) = {
        let s = state.borrow();

        let mut names: Vec<slint::SharedString> = Vec::new();
        names.push("— Workspace —".into());
        for p in &s.profiles {
            names.push(p.name.clone().into());
        }

        let current_profile_name = load_local_settings(
            s.selected_directory
                .as_deref()
                .unwrap_or_else(|| std::path::Path::new("")),
        )
        .and_then(|s| s.current_profile);

        let idx = if let Some(sel) = current_profile_name {
            let mut found = 0i32;
            for (i, name) in names.iter().enumerate().skip(1) {
                if name.as_str() == sel {
                    found = i as i32;
                    break;
                }
            }
            found
        } else {
            0
        };

        (names, idx)
    };

    app.set_profiles(slint::ModelRc::new(slint::VecModel::from(names)));

    let model_len = app.get_profiles().row_count();
    let clamped_idx = if model_len == 0 {
        -1
    } else if desired_idx >= 0 && (desired_idx as usize) < model_len {
        desired_idx
    } else {
        0
    };
    app.set_selected_profile_index(clamped_idx);

    // Ensure UI reflects the index after the model swap
    let app_weak = app.as_weak();
    slint::invoke_from_event_loop(move || {
        if let Some(app) = app_weak.upgrade() {
            app.set_selected_profile_index(clamped_idx);
        }
    })
    .ok();
}

fn capture_profile_from_ui(app: &AppWindow, state: &SharedState, name: &str) -> Option<Profile> {
    let dir = { state.borrow().selected_directory.clone()? };

    let ws = WorkspaceSettings {
        version: 1,
        ext_filter: app.get_ext_filter().to_string(),
        exclude_dirs: app.get_exclude_dirs().to_string(),
        exclude_files: app.get_exclude_files().to_string(),
        remove_prefix: app.get_remove_prefix().to_string(),
        remove_regex: app.get_remove_regex().to_string(),
        hierarchy_only: app.get_hierarchy_only(),
        dirs_only: app.get_dirs_only(),
        rust_remove_inline_comments: app.get_rust_remove_inline_comments(),
        rust_remove_doc_comments: app.get_rust_remove_doc_comments(),
        rust_function_signatures_only: app.get_rust_function_signatures_only(),
        rust_signatures_only_filter: app.get_rust_signatures_only_filter().to_string(),
    };

    // NOTE: Preserve root selection by storing an empty relative path ("")
    // when the explicit key equals the project root.
    let explicit = {
        let s = state.borrow();
        s.explicit_states
            .iter()
            .filter_map(|(abs, &st)| {
                if let Ok(rel) = abs.strip_prefix(&dir) {
                    let path = if rel.as_os_str().is_empty() {
                        String::new() // represents project root selected
                    } else {
                        path_to_unix(rel)
                    };
                    Some(stitch::core::ProfileSelection { path, state: st })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };

    Some(Profile {
        name: name.to_string(),
        settings: ws,
        explicit,
    })
}

fn apply_profile_to_ui(app: &AppWindow, state: &SharedState, profile: &Profile) {
    app.set_ext_filter(profile.settings.ext_filter.clone().into());
    app.set_exclude_dirs(profile.settings.exclude_dirs.clone().into());
    app.set_exclude_files(profile.settings.exclude_files.clone().into());
    app.set_remove_prefix(profile.settings.remove_prefix.clone().into());
    app.set_remove_regex(profile.settings.remove_regex.clone().into());
    app.set_hierarchy_only(profile.settings.hierarchy_only);
    app.set_dirs_only(profile.settings.dirs_only);
    app.set_rust_remove_inline_comments(profile.settings.rust_remove_inline_comments);
    app.set_rust_remove_doc_comments(profile.settings.rust_remove_doc_comments);
    app.set_rust_function_signatures_only(profile.settings.rust_function_signatures_only);
    app.set_rust_signatures_only_filter(
        profile.settings.rust_signatures_only_filter.clone().into(),
    );

    app.set_profile_name(profile.name.clone().into());

    parse_filters_from_ui(app, state);

    let base = { state.borrow().selected_directory.clone() };
    {
        let mut s = state.borrow_mut();
        s.explicit_states.clear();
        if let Some(root) = base.as_ref() {
            let sep = std::path::MAIN_SEPARATOR.to_string();
            for sel in &profile.explicit {
                let abs = if sel.path.is_empty() {
                    // Empty relative path means: project root itself.
                    root.clone()
                } else {
                    root.join(sel.path.replace('/', sep.as_str()))
                };
                s.explicit_states.insert(abs, sel.state);
            }
        }
        s.profile_baseline = Some(profile.clone());
    }

    rebuild_tree_and_ui(app, state);
    on_generate_output(app, state);

    app.set_save_enabled(false);
}

pub fn on_select_profile(app: &AppWindow, state: &SharedState, index: i32) {
    let project_root = { state.borrow().selected_directory.clone() };
    let Some(root) = project_root else {
        return;
    };

    if index <= 0 {
        let mut local_settings = load_local_settings(&root).unwrap_or_default();
        local_settings.current_profile = None;
        let _ = save_local_settings(&root, &local_settings);
        if let Some(ws) = load_workspace(&root) {
            app.set_ext_filter(ws.ext_filter.clone().into());
            app.set_exclude_dirs(ws.exclude_dirs.clone().into());
            app.set_exclude_files(ws.exclude_files.clone().into());
            app.set_remove_prefix(ws.remove_prefix.clone().into());
            app.set_remove_regex(ws.remove_regex.clone().into());
            app.set_hierarchy_only(ws.hierarchy_only);
            app.set_dirs_only(ws.dirs_only);

            state.borrow_mut().workspace_baseline = Some(ws);

            parse_filters_from_ui(app, state);

            state.borrow_mut().explicit_states.clear();
            state.borrow_mut().profile_baseline = None;
            app.set_profile_name("".into());
            app.set_save_enabled(false);

            rebuild_tree_and_ui(app, state);
            on_generate_output(app, state);
        }
        return;
    }

    let (name, _scope) = {
        let s = state.borrow();
        let Some(meta) = s.profiles.get((index as usize) - 1) else {
            return;
        };
        (meta.name.clone(), meta.scope)
    };

    if let Some((profile, _)) = load_profile(&root, &name) {
        let mut local_settings = load_local_settings(&root).unwrap_or_default();
        local_settings.current_profile = Some(name.clone());
        let _ = save_local_settings(&root, &local_settings);
        apply_profile_to_ui(app, state, &profile);
    }
}

pub fn on_save_profile_current(app: &AppWindow, state: &SharedState) {
    let idx = app.get_selected_profile_index();
    if idx < 0 {
        return;
    }

    if idx == 0 {
        let Some(project_root) = state.borrow().selected_directory.clone() else {
            return;
        };

        let ws = WorkspaceSettings {
            version: 1,
            ext_filter: app.get_ext_filter().to_string(),
            exclude_dirs: app.get_exclude_dirs().to_string(),
            exclude_files: app.get_exclude_files().to_string(),
            remove_prefix: app.get_remove_prefix().to_string(),
            remove_regex: app.get_remove_regex().to_string(),
            hierarchy_only: app.get_hierarchy_only(),
            dirs_only: app.get_dirs_only(),
            rust_remove_inline_comments: app.get_rust_remove_inline_comments(),
            rust_remove_doc_comments: app.get_rust_remove_doc_comments(),
            rust_function_signatures_only: app.get_rust_function_signatures_only(),
            rust_signatures_only_filter: app.get_rust_signatures_only_filter().to_string(),
        };

        let _ = save_workspace(&project_root, &ws);

        {
            let mut s = state.borrow_mut();
            s.workspace_baseline = Some(ws);
        }
        app.set_save_enabled(false);
        return;
    }

    let (old_name, scope, project_root) = {
        let s = state.borrow();
        let Some(dir) = s.selected_directory.clone() else {
            return;
        };
        let Some(meta) = s.profiles.get((idx as usize).saturating_sub(1)) else {
            return;
        };
        (meta.name.clone(), meta.scope, dir)
    };

    let new_name = app.get_profile_name().to_string();
    if new_name.trim().is_empty() {
        app.set_save_enabled(false);
        return;
    }

    let Some(profile) = capture_profile_from_ui(app, state, &new_name) else {
        return;
    };

    let _ = save_profile(&project_root, &profile, scope);

    if new_name != old_name {
        let _ = delete_profile(&project_root, scope, &old_name);
    }

    let mut local_settings = load_local_settings(&project_root).unwrap_or_default();
    local_settings.current_profile = Some(new_name);
    let _ = save_local_settings(&project_root, &local_settings);

    {
        let mut s = state.borrow_mut();
        s.profiles = list_profiles(&project_root);
        s.profile_baseline = Some(profile);
    }
    refresh_profiles_ui(app, state);

    app.set_save_enabled(false);
}

pub fn on_save_profile_as(app: &AppWindow, state: &SharedState) {
    if let Some(d) = state.borrow().save_profile_dialog.as_ref() {
        let _ = d.show();
        return;
    }

    let dlg = crate::ui::SaveProfileDialog::new().expect("create SaveProfileDialog");
    dlg.set_name("".into());
    dlg.set_is_local(false);

    let dlg_apply = dlg.as_weak();
    let state_apply = state.clone();
    let app_apply = app.as_weak();

    dlg.on_apply(move |name, is_local| {
        if let (Some(app), Some(state_rc)) = (app_apply.upgrade(), Some(state_apply.clone())) {
            let scope = if is_local {
                ProfileScope::Local
            } else {
                ProfileScope::Shared
            };

            let project_root = { state_rc.borrow().selected_directory.clone() };
            if let Some(root) = project_root
                && let Some(profile) = capture_profile_from_ui(&app, &state_rc, name.as_str())
            {
                let _ = save_profile(&root, &profile, scope);

                let mut local_settings = load_local_settings(&root).unwrap_or_default();
                local_settings.current_profile = Some(profile.name.clone());
                let _ = save_local_settings(&root, &local_settings);

                {
                    let mut s = state_rc.borrow_mut();
                    s.profiles = list_profiles(&root);
                }

                // 4) Update the profiles UI; now it will compute the index using the NEW current_profile.
                refresh_profiles_ui(&app, &state_rc);

                // 5) Belt-and-suspenders: explicitly set selection to the new profile.
                //    (Index 0 is "— Workspace —", profiles start at 1.)
                let new_idx: i32 = {
                    let s = state_rc.borrow();
                    (s.profiles
                        .iter()
                        .position(|m| m.name == profile.name)
                        .unwrap_or(0) as i32)
                        + 1
                };
                app.set_selected_profile_index(new_idx);
                // Also schedule on the event loop to avoid races with UI updates.
                let app_weak_for_idx = app.as_weak();
                slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_weak_for_idx.upgrade() {
                        app.set_selected_profile_index(new_idx);
                    }
                })
                .ok();

                // 6) Apply the saved profile to the UI immediately (fields + baseline).
                apply_profile_to_ui(&app, &state_rc, &profile);
            }
        }
        if let Some(d) = dlg_apply.upgrade() {
            let _ = d.hide();
        }
    });

    let dlg_cancel = dlg.as_weak();
    dlg.on_cancel(move || {
        if let Some(d) = dlg_cancel.upgrade() {
            let _ = d.hide();
        }
    });

    state.borrow_mut().save_profile_dialog = Some(dlg);
    let _ = state.borrow().save_profile_dialog.as_ref().unwrap().show();
}

fn profiles_equal(a: &Profile, b: &Profile) -> bool {
    if a.name != b.name {
        return false;
    }
    let sa = &a.settings;
    let sb = &b.settings;
    if sa.version != sb.version
        || sa.ext_filter != sb.ext_filter
        || sa.exclude_dirs != sb.exclude_dirs
        || sa.exclude_files != sb.exclude_files
        || sa.remove_prefix != sb.remove_prefix
        || sa.remove_regex != sb.remove_regex
        || sa.hierarchy_only != sb.hierarchy_only
        || sa.dirs_only != sb.dirs_only
        || sa.rust_remove_inline_comments != sb.rust_remove_inline_comments
        || sa.rust_remove_doc_comments != sb.rust_remove_doc_comments
        || sa.rust_function_signatures_only != sb.rust_function_signatures_only
        || sa.rust_signatures_only_filter != sb.rust_signatures_only_filter
    {
        return false;
    }
    // Compare explicit selections ignoring order
    use std::cmp::Ordering;
    let mut ea = a.explicit.clone();
    let mut eb = b.explicit.clone();
    ea.sort_by(|x, y| match x.path.cmp(&y.path) {
        Ordering::Equal => x.state.cmp(&y.state),
        o => o,
    });
    eb.sort_by(|x, y| match x.path.cmp(&y.path) {
        Ordering::Equal => x.state.cmp(&y.state),
        o => o,
    });
    ea == eb
}

fn update_save_button_state(app: &AppWindow, state: &SharedState) {
    let idx = app.get_selected_profile_index();

    if idx < 0 {
        app.set_save_enabled(false);
        return;
    }

    // Workspace (— Workspace —)
    if idx == 0 {
        let current = WorkspaceSettings {
            version: 1,
            ext_filter: app.get_ext_filter().to_string(),
            exclude_dirs: app.get_exclude_dirs().to_string(),
            exclude_files: app.get_exclude_files().to_string(),
            remove_prefix: app.get_remove_prefix().to_string(),
            remove_regex: app.get_remove_regex().to_string(),
            hierarchy_only: app.get_hierarchy_only(),
            dirs_only: app.get_dirs_only(),
            rust_remove_inline_comments: app.get_rust_remove_inline_comments(),
            rust_remove_doc_comments: app.get_rust_remove_doc_comments(),
            rust_function_signatures_only: app.get_rust_function_signatures_only(),
            rust_signatures_only_filter: app.get_rust_signatures_only_filter().to_string(),
        };

        let baseline_opt = { state.borrow().workspace_baseline.clone() };
        let dirty = match baseline_opt {
            Some(b) => !workspace_settings_equal(&b, &current),
            None => true,
        };
        app.set_save_enabled(dirty);
        return;
    }

    // Personal profile
    let name = app.get_profile_name().to_string();
    if name.trim().is_empty() {
        app.set_save_enabled(false);
        return;
    }

    let current_opt = capture_profile_from_ui(app, state, &name);
    let Some(current) = current_opt else {
        app.set_save_enabled(false);
        return;
    };

    let baseline_opt = { state.borrow().profile_baseline.clone() };
    let dirty = match baseline_opt {
        Some(b) => !profiles_equal(&b, &current),
        None => true,
    };
    app.set_save_enabled(dirty);
}

pub fn on_profile_name_changed(app: &AppWindow, state: &SharedState) {
    update_save_button_state(app, state);
}

pub fn on_delete_profile(app: &AppWindow, state: &SharedState) {
    let (idx, root, meta_opt) = {
        let s = state.borrow();
        (
            app.get_selected_profile_index(),
            s.selected_directory.clone(),
            s.profiles
                .get((app.get_selected_profile_index() as usize).saturating_sub(1))
                .cloned(),
        )
    };
    if idx <= 0 {
        return;
    }
    let Some(project_root) = root else {
        return;
    };
    let Some(meta) = meta_opt else {
        return;
    };

    let _ = delete_profile(&project_root, meta.scope, &meta.name);

    let mut local_settings = load_local_settings(&project_root).unwrap_or_default();
    local_settings.current_profile = None;
    let _ = save_local_settings(&project_root, &local_settings);
    if let Some(ws) = load_workspace(&project_root) {
        app.set_ext_filter(ws.ext_filter.clone().into());
        app.set_exclude_dirs(ws.exclude_dirs.clone().into());
        app.set_exclude_files(ws.exclude_files.clone().into());
        app.set_remove_prefix(ws.remove_prefix.clone().into());
        app.set_remove_regex(ws.remove_regex.clone().into());
        app.set_hierarchy_only(ws.hierarchy_only);
        app.set_dirs_only(ws.dirs_only);

        // Update baseline
        state.borrow_mut().workspace_baseline = Some(ws);
    }

    {
        let mut s = state.borrow_mut();
        s.profiles = list_profiles(&project_root);
        s.profile_baseline = None;
        s.explicit_states.clear();
    }
    refresh_profiles_ui(app, state);
    app.set_profile_name("".into());
    app.set_save_enabled(false);

    parse_filters_from_ui(app, state);
    rebuild_tree_and_ui(app, state);
    on_generate_output(app, state);
}

pub fn on_discard_changes(app: &AppWindow, state: &SharedState) {
    let idx = app.get_selected_profile_index();
    if idx < 0 {
        return;
    }

    // Workspace mode: restore workspace_baseline and clear selections
    if idx == 0 {
        let ws_opt = {
            let s = state.borrow();
            s.workspace_baseline.clone()
        }
        .or_else(|| {
            let dir_opt = { state.borrow().selected_directory.clone() };
            dir_opt.and_then(|d| load_workspace(&d))
        });

        if let Some(ws) = ws_opt {
            app.set_ext_filter(ws.ext_filter.clone().into());
            app.set_exclude_dirs(ws.exclude_dirs.clone().into());
            app.set_exclude_files(ws.exclude_files.clone().into());
            app.set_remove_prefix(ws.remove_prefix.clone().into());
            app.set_remove_regex(ws.remove_regex.clone().into());
            app.set_hierarchy_only(ws.hierarchy_only);
            app.set_dirs_only(ws.dirs_only);

            parse_filters_from_ui(app, state);

            {
                let mut s = state.borrow_mut();
                s.explicit_states.clear();
                s.profile_baseline = None;
                s.workspace_baseline = Some(ws);
            }

            rebuild_tree_and_ui(app, state);
            on_generate_output(app, state);
            app.set_save_enabled(false);
        }
        return;
    }

    // Profile mode: re-apply profile_baseline (or reload from disk as fallback)
    if let Some(baseline) = { state.borrow().profile_baseline.clone() } {
        apply_profile_to_ui(app, state, &baseline);
        app.set_save_enabled(false);
        return;
    }

    // Fallback: load from disk if baseline isn't present
    let (name_opt, root_opt) = {
        let s = state.borrow();
        let name = s
            .profiles
            .get((idx as usize).saturating_sub(1))
            .map(|m| m.name.clone());
        (name, s.selected_directory.clone())
    };

    if let (Some(name), Some(root)) = (name_opt, root_opt)
        && let Some((profile, _scope)) = load_profile(&root, &name)
    {
        {
            let mut s = state.borrow_mut();
            s.profile_baseline = Some(profile.clone());
        }
        apply_profile_to_ui(app, state, &profile);
        app.set_save_enabled(false);
    }
}

fn workspace_settings_equal(a: &WorkspaceSettings, b: &WorkspaceSettings) -> bool {
    a.version == b.version
        && a.ext_filter == b.ext_filter
        && a.exclude_dirs == b.exclude_dirs
        && a.exclude_files == b.exclude_files
        && a.remove_prefix == b.remove_prefix
        && a.remove_regex == b.remove_regex
        && a.hierarchy_only == b.hierarchy_only
        && a.dirs_only == b.dirs_only
        && a.rust_remove_inline_comments == b.rust_remove_inline_comments
        && a.rust_remove_doc_comments == b.rust_remove_doc_comments
        && a.rust_function_signatures_only == b.rust_function_signatures_only
        && a.rust_signatures_only_filter == b.rust_signatures_only_filter
    // Note: we intentionally ignore `current_profile` here for dirtiness comparison
}

fn format_project_path_for_title(dir: &Path) -> String {
    // Prefer HOME (Unix) or USERPROFILE (Windows) for "~" replacement.
    let home_opt = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    if let Some(home_os) = home_opt {
        let home = std::path::PathBuf::from(home_os);
        if let Ok(rel) = dir.strip_prefix(&home) {
            if rel.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", path_to_unix(rel));
        }
    }
    path_to_unix(dir)
}
