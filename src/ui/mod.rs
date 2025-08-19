// This module is only used when the `ui` feature is enabled.
slint::include_modules!();

pub mod handlers;
pub mod state;

pub use handlers::{
    apply_selection_from_text, on_check_updates, on_copy_output, on_filter_changed,
    on_generate_output, on_select_folder, on_toggle_check, on_toggle_expand,
};
pub use state::AppState;
