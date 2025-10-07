use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/* ============================ Workspace settings ============================ */

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceSettings {
    pub version: u32,
    pub ext_filter: String,
    pub exclude_dirs: String,
    pub exclude_files: String,
    pub remove_prefix: String,
    pub remove_regex: String,
    pub hierarchy_only: bool,
    pub dirs_only: bool,
    #[serde(flatten)]
    pub rust: RustOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RustOptions {
    #[serde(default)]
    pub rust_remove_inline_comments: bool,
    #[serde(default)]
    pub rust_remove_doc_comments: bool,
    #[serde(default)]
    pub rust_function_signatures_only: bool,
    #[serde(default)]
    pub rust_signatures_only_filter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalSettings {
    #[serde(default)]
    pub current_profile: Option<String>,
}

/* ================================ Profiles ================================= */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileScope {
    Shared,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ProfileSelection {
    /// Project-relative path using forward slashes.
    pub path: String,
    pub state: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Profile {
    pub name: String,
    pub settings: WorkspaceSettings,
    /// Explicit on/off checks captured relative to project root.
    pub explicit: Vec<ProfileSelection>,
}

#[derive(Debug, Clone)]
pub struct ProfileMeta {
    pub name: String,
    pub scope: ProfileScope,
}

/* ========================= Paths & basic workspace ========================= */

#[must_use]
pub fn workspace_dir(project_root: &Path) -> PathBuf {
    project_root.join(".stitchworkspace")
}

#[must_use]
pub fn workspace_file(project_root: &Path) -> PathBuf {
    workspace_dir(project_root).join("workspace.json")
}

#[must_use]
pub fn local_settings_file(project_root: &Path) -> PathBuf {
    workspace_dir(project_root)
        .join("local")
        .join("settings.json")
}

/// Create the workspace directory structure if missing.
///
/// # Errors
/// Returns I/O errors from directory creation or file operations when persisting metadata.
pub fn ensure_workspace_dir(project_root: &Path) -> io::Result<PathBuf> {
    let dir = workspace_dir(project_root);
    let created = if dir.exists() {
        false
    } else {
        fs::create_dir_all(&dir)?;
        true
    };

    // On first creation, try to ensure the local folder is ignored by git.
    if created {
        // Best-effort; ignore errors so we don't block the UI or creation flow.
        let _ = try_ensure_gitignore_local_exclusion(project_root);
    }

    Ok(dir)
}

// In `src/core/workspace.rs`, add this new helper (top-level, near the other IO helpers):
fn try_ensure_gitignore_local_exclusion(project_root: &Path) -> io::Result<()> {
    let gi_path = project_root.join(".gitignore");
    if !gi_path.exists() {
        // No .gitignore at project root; nothing to do.
        return Ok(());
    }

    // Read existing .gitignore content (lossily; keep going even if weird encodings).
    let mut contents = fs::read_to_string(&gi_path).unwrap_or_default();

    // Check if any existing rule already ignores the local folder.
    // We accept common variants like ".stitchworkspace/local", ".stitchworkspace/local/", or "**/.stitchworkspace/local".
    let already_present = contents.lines().any(|line| {
        let s = line.trim();
        // Ignore pure comments and empties for the presence check
        if s.is_empty() || s.starts_with('#') {
            return false;
        }
        let normalized = s.trim_end_matches('/');
        normalized.ends_with(".stitchworkspace/local")
    });

    if already_present {
        return Ok(());
    }

    // Prepare an idempotent block to append.
    let eol = if contents.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let block = format!("{eol}# Stitch workspace (per-user){eol}.stitchworkspace/local/{eol}");

    // Ensure the file ends with a single newline before appending our block.
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str(&block);

    fs::write(&gi_path, contents)?;
    Ok(())
}

/* ============================ Profiles locations ============================ */

fn profiles_shared_dir(project_root: &Path) -> PathBuf {
    workspace_dir(project_root).join("profiles")
}

fn profiles_local_dir(project_root: &Path) -> PathBuf {
    workspace_dir(project_root).join("local").join("profiles")
}

/// Ensure shared and local profiles directories exist.
///
/// # Errors
/// Returns I/O errors if directory creation fails.
pub fn ensure_profiles_dirs(project_root: &Path) -> io::Result<()> {
    fs::create_dir_all(profiles_shared_dir(project_root))?;
    fs::create_dir_all(profiles_local_dir(project_root))?;
    Ok(())
}

fn sanitize_profile_name(name: &str) -> String {
    // keep it simple & predictable for file names
    let mut s = name.trim().to_string();
    if s.is_empty() {
        s.push_str("unnamed");
    }
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn profile_path(project_root: &Path, scope: ProfileScope, name: &str) -> PathBuf {
    let base = match scope {
        ProfileScope::Shared => profiles_shared_dir(project_root),
        ProfileScope::Local => profiles_local_dir(project_root),
    };
    base.join(format!("{}.json", sanitize_profile_name(name)))
}

/* =============================== Workspace IO ============================== */

#[must_use]
pub fn load_workspace(project_root: &Path) -> Option<WorkspaceSettings> {
    let path = workspace_file(project_root);
    let data = fs::read(&path).ok()?;
    serde_json::from_slice::<WorkspaceSettings>(&data).ok()
}

/// Save the workspace settings atomically.
///
/// # Errors
/// Returns I/O errors from writing/renaming files, or serialization errors.
pub fn save_workspace(project_root: &Path, settings: &WorkspaceSettings) -> io::Result<()> {
    ensure_workspace_dir(project_root)?;

    let path = workspace_file(project_root);
    let tmp = path.with_extension("json.tmp");

    let data = serde_json::to_vec_pretty(settings).map_err(|e| io::Error::other(e.to_string()))?;

    fs::write(&tmp, data)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Load per-user local settings for this workspace.
#[must_use]
pub fn load_local_settings(project_root: &Path) -> Option<LocalSettings> {
    let path = local_settings_file(project_root);
    let data = fs::read(&path).ok()?;
    serde_json::from_slice::<LocalSettings>(&data).ok()
}

/// Save per-user local settings atomically.
///
/// # Errors
/// Returns I/O errors from writing/renaming files, or serialization errors.
pub fn save_local_settings(project_root: &Path, settings: &LocalSettings) -> io::Result<()> {
    ensure_workspace_dir(project_root)?;
    let path = local_settings_file(project_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");

    let data = serde_json::to_vec_pretty(settings).map_err(|e| io::Error::other(e.to_string()))?;

    fs::write(&tmp, data)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/* =============================== Profiles IO =============================== */

/// Save a profile JSON file atomically for the given scope.
///
/// # Errors
/// Returns I/O errors from writing/renaming files, or serialization errors.
pub fn save_profile(project_root: &Path, profile: &Profile, scope: ProfileScope) -> io::Result<()> {
    ensure_profiles_dirs(project_root)?;
    let path = profile_path(project_root, scope, &profile.name);
    let tmp = path.with_extension("tmp");
    let data = serde_json::to_vec_pretty(profile).map_err(|e| io::Error::other(e.to_string()))?;
    fs::write(&tmp, data)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Returns (Profile, Scope) preferring Local if both exist.
#[must_use]
pub fn load_profile(project_root: &Path, name: &str) -> Option<(Profile, ProfileScope)> {
    let local = profile_path(project_root, ProfileScope::Local, name);
    if let Ok(bytes) = fs::read(&local)
        && let Ok(p) = serde_json::from_slice::<Profile>(&bytes)
    {
        return Some((p, ProfileScope::Local));
    }
    let shared = profile_path(project_root, ProfileScope::Shared, name);
    if let Ok(bytes) = fs::read(&shared)
        && let Ok(p) = serde_json::from_slice::<Profile>(&bytes)
    {
        return Some((p, ProfileScope::Shared));
    }
    None
}

/// Delete a profile JSON file for the given scope if it exists.
///
/// # Errors
/// Returns I/O errors only on unexpected failures writing to the filesystem.
pub fn delete_profile(project_root: &Path, scope: ProfileScope, name: &str) -> io::Result<()> {
    let path = profile_path(project_root, scope, name);
    if path.exists() {
        // Best effort delete; ignore if it fails
        let _ = fs::remove_file(&path);
    }
    Ok(())
}

/// Lists all profiles found. If a name exists in both scopes, only the Local one is returned.
#[must_use]
pub fn list_profiles(project_root: &Path) -> Vec<ProfileMeta> {
    // Scan a directory for *.json profiles and capture (display_name, scope, timestamp-key)
    // Display name comes from the Profile JSON's `name` field (unsanitized),
    // so symbols like parentheses are preserved in the UI.
    fn scan(dir: &Path, scope: ProfileScope, out: &mut Vec<(String, ProfileScope, u128)>) {
        if let Ok(rd) = fs::read_dir(dir) {
            for ent in rd.flatten() {
                let path = ent.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }

                // Prefer creation time; fall back to modified time; otherwise 0.
                let ts_key: u128 = fs::metadata(&path)
                    .ok()
                    .and_then(|m| {
                        m.created().or_else(|_| m.modified()).ok().map(|t| {
                            t.duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                                .as_micros()
                        })
                    })
                    .unwrap_or(0);

                // Read display name from file contents; fallback to file stem if parse fails.
                let display_name = match fs::read(&path)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Profile>(&bytes).ok())
                {
                    Some(p) if !p.name.trim().is_empty() => p.name,
                    _ => path.file_stem().map_or_else(
                        || "unnamed".to_string(),
                        |os| os.to_string_lossy().to_string(),
                    ),
                };

                out.push((display_name, scope, ts_key));
            }
        }
    }

    let mut raw: Vec<(String, ProfileScope, u128)> = Vec::new();
    scan(
        &profiles_shared_dir(project_root),
        ProfileScope::Shared,
        &mut raw,
    );
    scan(
        &profiles_local_dir(project_root),
        ProfileScope::Local,
        &mut raw,
    );

    // Deduplicate by *display name*, prefer Local over Shared, and for same scope prefer newest ts.
    let mut by_name: std::collections::BTreeMap<String, (ProfileScope, u128)> =
        std::collections::BTreeMap::new();
    for (name, scope, ts) in raw {
        match by_name.get(&name) {
            None => {
                by_name.insert(name, (scope, ts));
            }
            Some(&(prev_scope, prev_ts)) => {
                let should_replace = (prev_scope == ProfileScope::Shared
                    && scope == ProfileScope::Local)
                    || (prev_scope == scope && ts > prev_ts);
                if should_replace {
                    by_name.insert(name, (scope, ts));
                }
            }
        }
    }

    // Sort by timestamp (newest first) and return.
    let mut merged: Vec<(String, ProfileScope, u128)> = by_name
        .into_iter()
        .map(|(name, (scope, ts))| (name, scope, ts))
        .collect();
    merged.sort_by(|a, b| b.2.cmp(&a.2));

    merged
        .into_iter()
        .map(|(name, scope, _)| ProfileMeta { name, scope })
        .collect()
}

/// Remove a stale current profile reference if the profile no longer exists.
///
/// # Errors
/// Returns I/O errors from saving updated local settings.
pub fn clear_stale_current_profile(project_root: &Path) -> io::Result<bool> {
    let mut local_settings = load_local_settings(project_root).unwrap_or_default();

    let Some(name) = local_settings.current_profile.clone() else {
        return Ok(false);
    };

    if load_profile(project_root, &name).is_some() {
        return Ok(false);
    }

    local_settings.current_profile = None;
    save_local_settings(project_root, &local_settings)?;
    Ok(true)
}
