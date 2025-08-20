use regex::Regex;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    time::SystemTime,
};

#[derive(Default)]
pub struct AppState {
    pub selected_directory: Option<PathBuf>,
    pub root_node: Option<stitch::core::Node>,
    pub explicit_states: HashMap<PathBuf, bool>,
    pub last_mod_times: HashMap<PathBuf, Option<SystemTime>>,
    pub poll_interval_ms: u64,
    pub path_snapshot: Option<HashSet<PathBuf>>,
    pub remove_prefixes: Vec<String>,
    pub remove_regex_str: Option<String>,
    pub remove_regex: Option<Regex>,
    pub include_exts: HashSet<String>,
    pub exclude_exts: HashSet<String>,
    pub exclude_dirs: HashSet<String>,
    pub exclude_files: HashSet<String>,
    pub copy_toast_timer: slint::Timer,
    pub select_dialog: Option<crate::ui::SelectFromTextDialog>,
    pub fs_dirty: bool,
    pub watcher: Option<notify::RecommendedWatcher>,
    pub fs_event_rx: Option<std::sync::mpsc::Receiver<notify::Result<notify::Event>>>,
    pub fs_pump_timer: slint::Timer,
    pub full_output_text: String,
    pub poll_timer: slint::Timer,

    /// Available profiles (name + scope). Order is alphabetical by name.
    pub profiles: Vec<stitch::core::ProfileMeta>,

    /// The "Save As…" dialog instance, if shown.
    pub save_profile_dialog: Option<crate::ui::SaveProfileDialog>,
    pub profile_baseline: Option<stitch::core::Profile>,
}

pub type SharedState = Rc<RefCell<AppState>>;
