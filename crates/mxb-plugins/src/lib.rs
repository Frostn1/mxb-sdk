//! Public manifest contract and safe filesystem operations for MX Bikes game plugins.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};

fn find_case(dir: &Path, rel: &str) -> PathBuf {
    let mut out = dir.to_path_buf();
    for part in rel.trim_end_matches('/').split('/') {
        let candidate = fs::read_dir(&out)
            .ok()
            .and_then(|rd| {
                rd.flatten()
                    .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))
            })
            .map(|e| e.path());
        out = candidate.unwrap_or_else(|| out.join(part));
    }
    out
}

fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(root) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                out.extend(walk_files(&e.path()))
            } else {
                out.push(e.path())
            }
        }
    }
    out
}

fn prune_empty(root: &Path, moves: &[(PathBuf, PathBuf)]) {
    let mut dirs: Vec<PathBuf> = moves
        .iter()
        .flat_map(|(from, _)| from.ancestors().skip(1).map(Path::to_path_buf).collect::<Vec<_>>())
        .filter(|d| d.starts_with(root) && d != root)
        .collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    dirs.dedup();
    for d in dirs {
        let _ = fs::remove_dir(d);
    }
}




// ===========================================================================
// The plugin manifest: `<plugin>.mxbplugin.json`, beside the `.dlo`. Optional, and every
// field in it is. Documented for authors in docs/plugins.md; keep the two in step.
// ===========================================================================

/// The longest manifest the app reads. A real one is a few hundred bytes.
pub const MAX_MANIFEST: u64 = 64 * 1024;

/// Optional metadata read from `<name>.mxbplugin.json` beside a plugin binary.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Manifest {
    /// Display name, at most 60 characters.
    pub name: Option<String>,
    /// Author-provided version string.
    pub version: Option<String>,
    /// Display author, at most 60 characters.
    pub author: Option<String>,
    /// Plain-text description, at most 400 characters.
    pub description: Option<String>,
    /// One of `hud`, `graphics`, `physics`, `fixes`, `timing`, or `other`.
    pub category: Option<String>,
    /// Optional HTTPS project page.
    pub homepage: Option<String>,
    /// MX Bikes beta the plugin was built for.
    pub game_version: Option<String>,
    /// Additional files and folders relative to the plugins folder.
    pub files: Vec<String>,
    /// Settings files exposed by the app.
    pub settings: Vec<ManifestSetting>,
    /// Keyboard shortcuts shown by the app.
    pub keys: Vec<ManifestKey>,
    /// Racing-mode default: `off`, `stay`, or `always-on`.
    pub racing: Option<String>,
    /// Other plugins this author declares incompatible.
    pub conflicts: Vec<ManifestConflict>,
    /// Optional update source.
    pub updates: Option<ManifestUpdates>,
}
/// A settings file declared by a plugin manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ManifestSetting {
    /// Relative settings path.
    pub file: String,
    /// Label shown in the app.
    pub label: Option<String>,
    /// `plugins` (default) or `documents` (`Documents\PiBoSo\MX Bikes`).
    #[serde(rename = "where")]
    pub where_: Option<String>,
}
/// A keyboard shortcut declared by a plugin manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ManifestKey {
    /// Key combination, such as `Ctrl+F7`.
    pub key: String,
    /// Human-readable action label.
    pub label: String,
}
/// A conflict declared by a plugin author.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ManifestConflict {
    /// Another plugin: its `.dlo` file name, or a catalog id like `graphics-overhaul`.
    pub with: String,
    /// Plain-text explanation shown to the player.
    pub reason: String,
}
/// Update metadata declared by a plugin manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ManifestUpdates {
    /// `owner/repo`: the newest GitHub release's tag is the latest version.
    pub github: Option<String>,
}

/// A short, single-line string from someone else's file, or nothing.
fn clean(s: Option<&str>, max: usize) -> Option<String> {
    let s: String = s?.chars().filter(|c| !c.is_control()).take(max).collect();
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}
fn clean_rel(s: &str) -> Option<String> {
    let s = s.trim().replace('\\', "/");
    let dir = s.ends_with('/');
    let rel = Path::new(s.trim_end_matches('/'));
    (!s.is_empty() && safe_rel(rel) && !stock(&rel.file_name()?.to_string_lossy()))
        .then(|| if dir { format!("{}/", rel.to_string_lossy().replace('\\', "/")) } else { rel.to_string_lossy().replace('\\', "/") })
}
fn valid_repo(s: &str) -> bool {
    let mut parts = s.split('/');
    let ok = |p: Option<&str>| {
        p.is_some_and(|p| !p.is_empty() && p.len() <= 100 && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)))
    };
    ok(parts.next()) && ok(parts.next()) && parts.next().is_none()
}

/// `<stem>.mxbplugin.json` for a plugin whose `.dlo` is `dlo_name`, under `root`.
pub fn manifest_name(dlo_name: &str) -> String {
    let stem = Path::new(dlo_name).file_stem().unwrap_or_default().to_string_lossy().into_owned();
    format!("{stem}.mxbplugin.json")
}
/// Reads a nearby manifest, returning `None` when it is absent, oversized, or malformed.
pub fn read_manifest(root: &Path, dlo_name: &str) -> Option<Manifest> {
    let path = find_case(root, &manifest_name(dlo_name));
    if fs::metadata(&path).ok()?.len() > MAX_MANIFEST {
        log::warn!("[plugins] {} is too large to read", path.display());
        return None;
    }
    match serde_json::from_slice(&fs::read(&path).ok()?) {
        Ok(m) => Some(m),
        Err(e) => {
            log::warn!("[plugins] couldn't read {}: {e}", path.display());
            None
        }
    }
}

/// Game-owned filenames that are never listed, moved, or uninstalled.
pub const STOCK_FILES: &[&str] = &["proxy64.dlo", "proxy_udp64.dlo", "xinput64.dli"];

/// A plugin found in the active or disabled plugins folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plugin {
    /// Stable lower-case identifier derived from the `.dlo` stem.
    pub id: String,
    /// Path to the plugin binary.
    pub file: PathBuf,
    /// Whether the binary is in the active plugins folder.
    pub enabled: bool,
    /// Parsed manifest, when one exists and passes its limits.
    pub manifest: Option<Manifest>,
    /// Relative files and folders that move with the plugin.
    pub files: Vec<String>,
}

/// Returns the disabled plugins folder used by MXB App.
pub fn disabled_dir(game_dir: &Path) -> PathBuf {
    game_dir.join("mxbapp_disabled").join("plugins")
}

/// Validates every documented manifest limit and path rule.
pub fn validate_manifest(manifest: &Manifest) -> anyhow::Result<()> {
    for (value, maximum, field) in [
        (manifest.name.as_deref(), 60, "name"),
        (manifest.author.as_deref(), 60, "author"),
        (manifest.description.as_deref(), 400, "description"),
        (manifest.game_version.as_deref(), 16, "gameVersion"),
    ] {
        if value.is_some_and(|value| value.chars().count() > maximum || value.chars().any(char::is_control)) {
            anyhow::bail!("{field} is longer than {maximum} characters or contains controls");
        }
    }
    if manifest.homepage.as_ref().is_some_and(|url| !url.starts_with("https://") || url.len() > 300) {
        anyhow::bail!("homepage must be an https:// address");
    }
    if manifest.category.as_ref().is_some_and(|category| {
        !["hud", "graphics", "physics", "fixes", "timing", "other"].contains(&category.as_str())
    }) {
        anyhow::bail!("unknown plugin category");
    }
    if manifest.files.len() > 64 || manifest.settings.len() > 16 || manifest.keys.len() > 16 || manifest.conflicts.len() > 16 {
        anyhow::bail!("manifest contains more entries than the plugin format allows");
    }
    for file in &manifest.files {
        if clean_rel(file).is_none() {
            anyhow::bail!("unsafe plugin file path: {file}");
        }
    }
    for setting in &manifest.settings {
        if clean_rel(&setting.file).is_none()
            || setting.where_.as_deref().is_some_and(|place| place != "plugins" && place != "documents")
        {
            anyhow::bail!("invalid settings entry");
        }
    }
    if manifest.updates.as_ref().and_then(|updates| updates.github.as_deref()).is_some_and(|repo| !valid_repo(repo)) {
        anyhow::bail!("updates.github must be owner/repo");
    }
    Ok(())
}

/// Reads a manifest and returns validation failures instead of treating them as absent.
pub fn read_manifest_checked(root: &Path, dlo_name: &str) -> anyhow::Result<Option<Manifest>> {
    let path = find_case(root, &manifest_name(dlo_name));
    if !path.is_file() {
        return Ok(None);
    }
    if fs::metadata(&path)?.len() > MAX_MANIFEST {
        anyhow::bail!("{} is larger than 64 KB", path.display());
    }
    let manifest: Manifest = serde_json::from_slice(&fs::read(&path)?)?;
    validate_manifest(&manifest)?;
    Ok(Some(manifest))
}

/// Returns the files and folders belonging to one plugin under `root`.
pub fn plugin_files(root: &Path, id: &str) -> anyhow::Result<Vec<PathBuf>> {
    if stock(&format!("{id}.dlo")) {
        anyhow::bail!("stock game files cannot be changed");
    }
    let dlo = fs::read_dir(root)?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("dlo"))
                && path.file_stem().is_some_and(|stem| stem.to_string_lossy().eq_ignore_ascii_case(id))
        })
        .ok_or_else(|| anyhow::anyhow!("plugin files were not found"))?;
    let dlo_name = dlo.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let mut files = vec![dlo];
    let manifest_path = find_case(root, &manifest_name(&dlo_name));
    if manifest_path.is_file() {
        files.push(manifest_path);
    }
    if let Some(manifest) = read_manifest_checked(root, &dlo_name)? {
        for relative in manifest.files.iter().filter_map(|file| clean_rel(file)).take(64) {
            let path = find_case(root, &relative);
            if path.exists() {
                files.push(path);
            }
        }
    }
    for relative in [format!("{id}_data"), id.into(), format!("{id}.ini"), format!("{id}.log")] {
        let path = find_case(root, &relative);
        if path.exists() {
            files.push(path);
        }
    }
    files.sort();
    files.dedup();
    let roots = files.clone();
    files.retain(|path| {
        !roots.iter().any(|parent| parent != path && path.starts_with(parent) && parent.is_dir())
    });
    Ok(files)
}

/// Lists active and disabled game plugins, excluding the game's stock shims.
pub fn list(game_dir: &Path) -> Vec<Plugin> {
    let active = game_dir.join("plugins");
    let disabled = disabled_dir(game_dir);
    let mut plugins = Vec::new();
    for (root, enabled) in [(&active, true), (&disabled, false)] {
        for path in fs::read_dir(root).into_iter().flatten().flatten().map(|entry| entry.path()) {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !path.is_file() || !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("dlo")) || stock(&name) {
                continue;
            }
            let id = path.file_stem().unwrap_or_default().to_string_lossy().to_lowercase();
            let manifest = read_manifest_checked(root, &name).ok().flatten();
            let files = plugin_files(root, &id)
                .unwrap_or_else(|_| vec![path.clone()])
                .iter()
                .filter_map(|file| file.strip_prefix(root).ok())
                .map(|file| file.to_string_lossy().replace('\\', "/"))
                .collect();
            plugins.push(Plugin { id, file: path, enabled, manifest, files });
        }
    }
    plugins.sort_by(|left, right| left.id.cmp(&right.id).then_with(|| right.enabled.cmp(&left.enabled)));
    plugins
}

/// Enables or disables a plugin by moving all of its files without overwriting anything.
pub fn set_enabled(game_dir: &Path, id: &str, enabled: bool, game_running: bool) -> anyhow::Result<()> {
    if game_running {
        anyhow::bail!("close MX Bikes first: the game holds its plugins open");
    }
    let active = game_dir.join("plugins");
    let disabled = disabled_dir(game_dir);
    let (from, to) = if enabled { (&disabled, &active) } else { (&active, &disabled) };
    let sources = plugin_files(from, id)?;
    let mut moves = Vec::new();
    for source in sources {
        let relative = source.strip_prefix(from)?;
        if !safe_rel(relative) {
            anyhow::bail!("refusing an unsafe plugin path");
        }
        let target = to.join(relative);
        if source.is_dir() && target.is_dir() {
            for file in walk_files(&source) {
                let target = to.join(file.strip_prefix(from)?);
                moves.push((file, target));
            }
        } else {
            moves.push((source, target));
        }
    }
    for (_, target) in &moves {
        if target.exists() {
            anyhow::bail!("refusing to overwrite {}", target.display());
        }
    }
    let mut completed: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (source, target) in &moves {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Err(error) = fs::rename(source, target) {
            for (old, new) in completed.iter().rev() {
                let _ = fs::rename(new, old);
            }
            return Err(error.into());
        }
        completed.push((source.clone(), target.clone()));
    }
    prune_empty(from, &completed);
    Ok(())
}

/// Checks the lexical boundary before a path is sent to the operating-system trash.
pub fn check_uninstall_target(game_dir: &Path, target: &Path) -> anyhow::Result<()> {
    if target.components().any(|component| matches!(component, Component::ParentDir)) {
        anyhow::bail!("refusing a path containing '..'");
    }
    let active = game_dir.join("plugins");
    let disabled = disabled_dir(game_dir);
    let under = (target.starts_with(&active) && target != active)
        || (target.starts_with(&disabled) && target != disabled);
    if !under || target.file_name().is_some_and(|name| stock(&name.to_string_lossy())) {
        anyhow::bail!("refusing to remove a protected plugin path");
    }
    Ok(())
}

/// Moves a plugin's files to the operating-system trash.
pub fn uninstall(game_dir: &Path, id: &str, game_running: bool) -> anyhow::Result<Vec<PathBuf>> {
    uninstall_with(game_dir, id, game_running, &mxb_core::uninstall::SystemTrash)
}

/// Moves a plugin's files through the same injectable trash trait used by `mxb-core`.
pub fn uninstall_with(
    game_dir: &Path,
    id: &str,
    game_running: bool,
    trash: &dyn mxb_core::uninstall::Trash,
) -> anyhow::Result<Vec<PathBuf>> {
    if game_running {
        anyhow::bail!("close MX Bikes first: the game holds its plugins open");
    }
    let mut removed = Vec::new();
    for root in [game_dir.join("plugins"), disabled_dir(game_dir)] {
        for path in plugin_files(&root, id).unwrap_or_default() {
            check_uninstall_target(game_dir, &path)?;
            trash.remove(&path).map_err(|error| anyhow::anyhow!(error.to_string()))?;
            removed.push(path);
        }
    }
    if removed.is_empty() {
        anyhow::bail!("plugin files were not found");
    }
    Ok(removed)
}

fn stock(name: &str) -> bool {
    STOCK_FILES.iter().any(|stock| stock.eq_ignore_ascii_case(name))
}

fn safe_rel(relative: &Path) -> bool {
    !relative.is_absolute() && relative.components().all(|component| matches!(component, Component::Normal(_)))
}
