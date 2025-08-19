#![allow(clippy::needless_return)]

#[cfg(feature = "ui")]
mod ui;

#[cfg(feature = "ui")]
use std::{cell::RefCell, rc::Rc};

#[cfg(feature = "ui")]
use slint::ComponentHandle;

#[cfg(feature = "ui")]
use ui::{
    AppState, AppWindow, Row, SelectFromTextDialog, apply_selection_from_text, on_check_updates,
    on_copy_output, on_filter_changed, on_generate_output, on_select_folder, on_toggle_check,
    on_toggle_expand,
};

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
    app.set_tree_model(slint::ModelRc::new(slint::VecModel::<Row>::default()));
    app.set_output_text("".into());
    app.set_show_copy_toast(false);
    app.set_copy_toast_text("".into());
    app.set_output_stats("0 chars • 0 tokens".into());

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

    // Select-from-text dialog wiring kept here to avoid another extra file.
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

#[cfg(not(feature = "ui"))]
fn main() -> anyhow::Result<()> {
    eprintln!(
        "Built without the `ui` feature; nothing to run. \
Enable it with `--features ui`, or just run tests with `--no-default-features`."
    );
    Ok(())
}
