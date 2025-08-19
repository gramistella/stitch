use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// On-disk workspace settings for a project folder.
/// Stored at `<project>/.stitchworkspace/workspace.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceSettings {
    pub version: u32, // for future migrations; currently 1
    pub ext_filter: String,
    pub exclude_dirs: String,
    pub exclude_files: String,
    pub remove_prefix: String,
    pub remove_regex: String,
    pub hierarchy_only: bool,
    pub dirs_only: bool,
}

pub fn workspace_dir(project_root: &Path) -> PathBuf {
    project_root.join(".stitchworkspace")
}

pub fn workspace_file(project_root: &Path) -> PathBuf {
    workspace_dir(project_root).join("workspace.json")
}

/// Ensure `.stitchworkspace/` exists; return its path.
pub fn ensure_workspace_dir(project_root: &Path) -> io::Result<PathBuf> {
    let dir = workspace_dir(project_root);
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/// Try to load settings; returns `None` if file is missing or invalid.
pub fn load_workspace(project_root: &Path) -> Option<WorkspaceSettings> {
    let path = workspace_file(project_root);
    let data = fs::read(&path).ok()?;
    serde_json::from_slice::<WorkspaceSettings>(&data).ok()
}

/// Save settings atomically to `workspace.json`.
pub fn save_workspace(project_root: &Path, settings: &WorkspaceSettings) -> io::Result<()> {
    ensure_workspace_dir(project_root)?;

    let path = workspace_file(project_root);
    let tmp = path.with_extension("json.tmp");

    let data = serde_json::to_vec_pretty(settings).map_err(|e| io::Error::other(e.to_string()))?;

    fs::write(&tmp, data)?;
    // Atomic on most platforms when same directory
    fs::rename(&tmp, &path)?;
    Ok(())
}
