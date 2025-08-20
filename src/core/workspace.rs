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

    /// Optional name of the currently-selected profile (if any).
    #[serde(default)]
    pub current_profile: Option<String>,
}

/* ================================ Profiles ================================= */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileScope {
    Shared,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
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

pub fn workspace_dir(project_root: &Path) -> PathBuf {
    project_root.join(".stitchworkspace")
}

pub fn workspace_file(project_root: &Path) -> PathBuf {
    workspace_dir(project_root).join("workspace.json")
}

pub fn ensure_workspace_dir(project_root: &Path) -> io::Result<PathBuf> {
    let dir = workspace_dir(project_root);
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/* ============================ Profiles locations ============================ */

fn profiles_shared_dir(project_root: &Path) -> PathBuf {
    workspace_dir(project_root).join("profiles")
}

fn profiles_local_dir(project_root: &Path) -> PathBuf {
    workspace_dir(project_root).join("local").join("profiles")
}

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

pub fn load_workspace(project_root: &Path) -> Option<WorkspaceSettings> {
    let path = workspace_file(project_root);
    let data = fs::read(&path).ok()?;
    serde_json::from_slice::<WorkspaceSettings>(&data).ok()
}

pub fn save_workspace(project_root: &Path, settings: &WorkspaceSettings) -> io::Result<()> {
    ensure_workspace_dir(project_root)?;

    let path = workspace_file(project_root);
    let tmp = path.with_extension("json.tmp");

    let data = serde_json::to_vec_pretty(settings).map_err(|e| io::Error::other(e.to_string()))?;

    fs::write(&tmp, data)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/* =============================== Profiles IO =============================== */

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

pub fn delete_profile(project_root: &Path, scope: ProfileScope, name: &str) -> io::Result<()> {
    let path = profile_path(project_root, scope, name);
    if path.exists() {
        // Best effort delete; ignore if it fails
        let _ = fs::remove_file(&path);
    }
    Ok(())
}

/// Lists all profiles found. If a name exists in both scopes, only the Local one is returned.
pub fn list_profiles(project_root: &Path) -> Vec<ProfileMeta> {
    fn scan(dir: &Path, scope: ProfileScope, out: &mut Vec<(String, ProfileScope)>) {
        if let Ok(rd) = fs::read_dir(dir) {
            for ent in rd.flatten() {
                if let Some(ext) = ent.path().extension()
                    && ext == "json"
                    && let Some(os) = ent.path().file_stem()
                {
                    let name = os.to_string_lossy().to_string();
                    out.push((name, scope));
                }
            }
        }
    }

    let mut raw: Vec<(String, ProfileScope)> = Vec::new();
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

    // keep a single entry per name, preferring Local
    use std::collections::BTreeMap;
    let mut by_name: BTreeMap<String, ProfileScope> = BTreeMap::new();
    for (n, s) in raw {
        match by_name.get(&n) {
            None => {
                by_name.insert(n, s);
            }
            Some(prev) => {
                if *prev == ProfileScope::Shared && s == ProfileScope::Local {
                    by_name.insert(n, s);
                }
            }
        }
    }

    by_name
        .into_iter()
        .map(|(name, scope)| ProfileMeta { name, scope })
        .collect()
}
