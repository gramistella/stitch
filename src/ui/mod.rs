slint::include_modules!();

pub mod handlers;
pub mod state;

pub use handlers::{
    apply_selection_from_text, on_check_updates, on_copy_output, on_delete_profile,
    on_filter_changed, on_generate_output, on_profile_name_changed, on_save_profile_as,
    on_save_profile_current, on_select_folder, on_select_profile, on_toggle_check,
    on_toggle_expand,
};

pub use state::AppState;
