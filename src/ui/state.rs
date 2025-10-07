use regex::Regex;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    sync::mpsc,
    time::SystemTime,
};

#[derive(Default)]
pub struct FsState {
    pub dirty: bool,
    pub watcher_disabled: bool,
}

#[derive(Default)]
pub struct GenerationState {
    pub in_progress: bool,
    pub queue_another: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommentRemoval {
    #[default]
    None,
    InlineOnly,
    DocOnly,
    InlineAndDoc,
}

impl CommentRemoval {
    #[must_use]
    pub const fn removes_inline(self) -> bool {
        matches!(self, Self::InlineOnly | Self::InlineAndDoc)
    }

    #[must_use]
    pub const fn removes_doc(self) -> bool {
        matches!(self, Self::DocOnly | Self::InlineAndDoc)
    }

    #[must_use]
    pub const fn from_flags(remove_inline: bool, remove_doc: bool) -> Self {
        match (remove_inline, remove_doc) {
            (true, true) => Self::InlineAndDoc,
            (true, false) => Self::InlineOnly,
            (false, true) => Self::DocOnly,
            (false, false) => Self::None,
        }
    }
}

#[derive(Default)]
pub struct RustUiState {
    pub has_files: bool,
    pub comment_removal: CommentRemoval,
    pub signatures_filter: Option<String>,
}

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
    pub existing_excluded_dirs: HashSet<String>,
    pub existing_excluded_files: HashSet<String>,
    pub copy_toast_timer: slint::Timer,
    pub select_dialog: Option<crate::ui::SelectFromTextDialog>,
    pub fs: FsState,
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

    pub workspace_baseline: Option<stitch::core::WorkspaceSettings>,

    pub generation: GenerationState,
    pub gen_seq: u64,
    pub gen_result_tx: Option<mpsc::Sender<(u64, String)>>,
    pub gen_result_rx: Option<mpsc::Receiver<(u64, String)>>,
    pub gen_pump_timer: slint::Timer,
    // Rust-specific filters and detection
    pub rust_ui: RustUiState,
}

pub type SharedState = Rc<RefCell<AppState>>;
