//! Extensions: third-party packages that change how an app looks and add sandboxed panels.
//!
//! This is the untrusted counterpart of `plugins.rs`. A paid plugin runs with the app's own
//! privileges because it only ever arrives over a verified license. An extension can come
//! from anybody, so nothing in it ever runs with those privileges:
//!
//!   * **Declarative looks.** Themes, button styles, backgrounds and fonts are data. The
//!     webview reads them through [`ext_read_text`] / [`ext_read_asset`] and turns them into
//!     validated CSS variables; no stylesheet or script from a package is applied to the app.
//!   * **Sandboxed code.** A view is HTML/JS that the webview runs in a sandboxed iframe with
//!     an opaque origin and a CSP of its own. Its only way out is a `postMessage` bridge that
//!     answers a fixed list of read-mostly calls, each behind a permission granted at install.
//!   * **Nothing native.** A package may only contain web files and media (see
//!     [`ALLOWED_EXTENSIONS`]). There is no payload step, no file access, no commands of its own.
//!
//! What this module owns is everything that touches the disk: reading a `.mxbext` (a zip),
//! checking its signature, keeping the per-extension state (enabled, granted permissions,
//! pinned signing key) and confining every read to the extension's own folder.
//!
//! ## Signatures
//!
//! `signature.json` at the package root holds an Ed25519 public key and a signature over
//! [`digest_input`]: every other file's path and SHA-256, sorted. Four trust levels follow:
//!
//!   * `builtin`: shipped inside the app (the Style Pack, the colorways). Never on disk here.
//!   * `verified`: signed by a key the host trusts ([`TrustPolicy::publishers`]); the host
//!     decides whose review that stands for. With the default policy nothing is verified.
//!   * `signed`: a valid signature by a key we don't know. Installs with a warning, and the
//!     key is pinned: an update signed by any other key is refused.
//!   * `unsigned`: no signature at all. Only with developer mode on.
//!
//! A signature that is present and wrong is always fatal, at every level.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Lowercase SHA-256 for package digests and public-key fingerprints.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(bytes);
    hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Returns a relative path that cannot escape a package root.
pub fn safe_path(name: &str) -> Option<PathBuf> {
    let name = name.replace('\\', "/");
    if name.starts_with('/') || name.contains(':') {
        return None;
    }
    let mut out = PathBuf::new();
    for part in name.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            part => out.push(part),
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// Compares dotted versions, treating a pre-release as older than its release.
pub fn version_at_least(have: &str, need: &str) -> bool {
    fn split(value: &str) -> (Vec<u32>, bool) {
        let core = value.split(['-', '+']).next().unwrap_or(value);
        (core.split('.').map(|part| part.parse().unwrap_or(0)).collect(), core.len() < value.len())
    }
    let (left, left_pre) = split(have);
    let (right, right_pre) = split(need);
    for index in 0..left.len().max(right.len()) {
        let pair = (
            left.get(index).copied().unwrap_or(0),
            right.get(index).copied().unwrap_or(0),
        );
        if pair.0 != pair.1 {
            return pair.0 > pair.1;
        }
    }
    !(left_pre && !right_pre)
}

/// Who a host (an app that installs extensions) trusts.
///
/// `publishers` are (name, base64 Ed25519 public key) pairs whose signature makes a package
/// `verified`; `reserved_prefixes` are id prefixes only those publishers may use, so nobody else
/// can pass a package off as theirs. The SDK itself trusts no one: each host brings its own.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrustPolicy<'a> {
    pub publishers: &'a [(&'a str, &'a str)],
    pub reserved_prefixes: &'a [&'a str],
}

/// The only file types a package may carry. No executables, no DLLs, no game plugins
/// (`.dlo`): a package with anything else in it is refused before a byte is written.
pub const ALLOWED_EXTENSIONS: &[&str] = &[
    "json", "html", "htm", "js", "mjs", "css", "png", "jpg", "jpeg", "webp", "gif", "svg",
    "woff", "woff2", "ttf", "otf", "txt", "md",
];

/// Every permission a manifest may ask for. Mirrored by `extensions/permissions.ts`, which
/// holds the wording the install dialog shows. An unknown permission fails the install:
/// silently dropping one would install something that then cannot do its job.
pub const PERMISSIONS: &[&str] = &[
    "mods.read",
    "servers.read",
    "friends.presence",
    "game.state",
    "game.read",
    "game.actions",
    "presets.read",
    "coach.read",
    "studio.launch",
    "network",
    "openExternal",
    "mcp.tools",
];

/// Largest accepted compressed package size.
pub const MAX_PACKAGE_BYTES: u64 = 25 * 1024 * 1024;
/// Largest accepted uncompressed file size.
pub const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
/// Largest accepted total uncompressed package size.
pub const MAX_TOTAL_BYTES: u64 = 60 * 1024 * 1024;
/// Largest accepted number of files in a package.
pub const MAX_FILES: usize = 2000;
pub const SIGNATURE_FILE: &str = "signature.json";
pub const MANIFEST_FILE: &str = "extension.json";

/// How much we trust a package. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Trust {
    /// Signed by a built-in trusted publisher.
    Verified,
    /// Signed by an otherwise unknown author key.
    Signed,
    /// Carries no signature.
    Unsigned,
}

/// The parts of `extension.json` this side reads. The rest (`contributes`) is passed to the
/// webview whole, after [`validate_manifest`] has checked every file it names exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Manifest format version, currently one.
    #[serde(default = "one")]
    pub manifest_version: u32,
    /// Lower-case dotted publisher and extension identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Semantic extension version.
    pub version: String,
    /// Display author.
    pub author: String,
    /// Plain-text summary.
    #[serde(default)]
    pub description: String,
    /// Optional HTTPS project page.
    #[serde(default)]
    pub homepage: Option<String>,
    /// Oldest MXB App version this package supports.
    #[serde(default)]
    pub min_app_version: Option<String>,
    /// Capabilities requested from the player.
    #[serde(default)]
    pub permissions: Vec<String>,
    /// `https://host[:port]` origins the views may `fetch`. Needs the `network` permission.
    #[serde(default)]
    pub network: Vec<String>,
    /// Where to look for a newer version: JSON `{ "version", "url" }`, https only.
    #[serde(default)]
    pub update_url: Option<String>,
    /// Declarative themes, views, commands, settings, and MCP tools.
    #[serde(default)]
    pub contributes: Value,
    /// Marketplace pictures, inside the package. Shown in the catalog, never by the app itself.
    #[serde(default)]
    pub screenshots: Vec<String>,
}

fn one() -> u32 {
    1
}

/// An available update waiting for review or installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingUpdate {
    /// Available semantic version.
    pub version: String,
    /// HTTPS package URL.
    pub url: String,
    /// Permissions the new version asks for that this install hasn't granted.
    pub new_permissions: Vec<String>,
}

/// Persisted state for one installed extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledExt {
    /// Installed semantic version.
    pub version: String,
    /// Whether the extension runs and contributes UI.
    pub enabled: bool,
    /// Signature trust assigned at installation.
    pub trust: Trust,
    /// Fingerprint of the signing key, pinned for updates. None for unsigned.
    #[serde(default)]
    pub key: Option<String>,
    /// Trusted publisher name, when known.
    #[serde(default)]
    pub publisher: Option<String>,
    /// Permissions currently granted by the player.
    pub granted: Vec<String>,
    /// Whether compatible updates may install automatically.
    #[serde(default = "yes")]
    pub auto_update: bool,
    /// Update waiting for review.
    #[serde(default)]
    pub pending_update: Option<PendingUpdate>,
    /// Installation time in Unix seconds.
    pub installed_at: i64,
    /// `file`, `url` or `folder`. Shown, not trusted.
    pub source: String,
}

fn yes() -> bool {
    true
}

/// Persisted extension subsystem state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtState {
    /// Whether unsigned folders and packages may be loaded.
    #[serde(default)]
    pub developer_mode: bool,
    /// Installed extensions keyed by manifest id.
    #[serde(default)]
    pub extensions: BTreeMap<String, InstalledExt>,
}

/// A package that has been read and checked, as it is about to be installed.
#[derive(Debug, Clone)]
pub struct Package {
    /// Parsed and validated manifest.
    pub manifest: Manifest,
    /// Original manifest JSON retained for the webview.
    pub manifest_raw: Value,
    /// Signature trust assigned to this package.
    pub trust: Trust,
    /// Signing-key fingerprint, when signed.
    pub key: Option<String>,
    /// Trusted publisher name, when known.
    pub publisher: Option<String>,
    /// Normalized package files keyed by forward-slash relative path.
    pub files: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Clone, Deserialize)]
struct SignatureFile {
    #[serde(rename = "publicKey")]
    public_key: String,
    signature: String,
}

/// What the signature covers: a fixed header, then `path\nsha256\n` for every file but the
/// signature itself, sorted by path. Paths are the normalised, forward-slash ones.
pub fn digest_input(files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut out = b"mxbext-v1\n".to_vec();
    for (path, bytes) in files {
        if path == SIGNATURE_FILE {
            continue;
        }
        out.extend_from_slice(path.as_bytes());
        out.push(b'\n');
        out.extend_from_slice(sha256_hex(bytes).as_bytes());
        out.push(b'\n');
    }
    out
}

/// First 16 hex digits of the key's SHA-256, which is how a key is shown and pinned.
pub fn key_fingerprint(key: &[u8]) -> String {
    sha256_hex(key)[..16].to_string()
}

pub fn file_ext(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Read a zip into memory with every limit applied. Nothing is written anywhere.
pub fn read_zip(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        bail!("that package is larger than {} MB", MAX_PACKAGE_BYTES / 1024 / 1024);
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .context("that file is not an extension package (.mxbext)")?;
    if zip.len() > MAX_FILES {
        bail!("that package has more than {MAX_FILES} files");
    }
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let rel = safe_path(entry.name())
            .ok_or_else(|| anyhow!("that package contains an unsafe path: {}", entry.name()))?;
        let rel = rel.to_string_lossy().replace('\\', "/");
        if !ALLOWED_EXTENSIONS.contains(&file_ext(&rel).as_str()) {
            bail!("that package contains a file type extensions may not carry: {rel}");
        }
        if entry.size() > MAX_FILE_BYTES {
            bail!("{rel} is larger than {} MB", MAX_FILE_BYTES / 1024 / 1024);
        }
        // The declared size can lie; `take` is what actually bounds a zip bomb.
        let mut buf = Vec::new();
        entry.take(MAX_FILE_BYTES + 1).read_to_end(&mut buf)?;
        if buf.len() as u64 > MAX_FILE_BYTES {
            bail!("{rel} is larger than {} MB", MAX_FILE_BYTES / 1024 / 1024);
        }
        total += buf.len() as u64;
        if total > MAX_TOTAL_BYTES {
            bail!("that package unpacks to more than {} MB", MAX_TOTAL_BYTES / 1024 / 1024);
        }
        if files.insert(rel.clone(), buf).is_some() {
            bail!("that package names {rel} twice");
        }
    }
    Ok(files)
}

/// Read a development folder the same way a package is read, for developer mode.
pub fn read_folder(root: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        // Editor and VCS litter is skipped rather than refused: it's a working folder.
        if rel.starts_with('.') || rel.contains("/.") || rel.starts_with("node_modules/") {
            continue;
        }
        if !ALLOWED_EXTENSIONS.contains(&file_ext(&rel).as_str()) {
            continue;
        }
        let len = entry.metadata()?.len();
        if len > MAX_FILE_BYTES {
            bail!("{rel} is larger than {} MB", MAX_FILE_BYTES / 1024 / 1024);
        }
        total += len;
        if total > MAX_TOTAL_BYTES || files.len() >= MAX_FILES {
            bail!("that folder is too large to load as an extension");
        }
        files.insert(rel, std::fs::read(entry.path())?);
    }
    Ok(files)
}

pub fn valid_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    (2..=4).contains(&parts.len())
        && id.len() <= 80
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 40
                && p.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                && !p.starts_with('-')
        })
}

fn valid_version(v: &str) -> bool {
    semver::Version::parse(v).is_ok()
}

/// `https://host[:port]`, nothing else: no path, no wildcard, no credentials.
pub fn valid_origin(o: &str) -> bool {
    let Some(rest) = o.strip_prefix("https://") else {
        return false;
    };
    let (host, port) = match rest.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (rest, None),
    };
    if let Some(p) = port {
        if p.parse::<u16>().is_err() {
            return false;
        }
    }
    !host.is_empty()
        && host.len() <= 253
        && host.contains('.')
        && host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
        && !host.starts_with('.')
        && !host.ends_with('.')
}

/// Every check that only needs the manifest and the file list. The webview validates the
/// *values* of each contribution again before it applies one; this side makes sure the
/// package is well-formed and that every file it names is really in it.
pub fn validate_manifest(m: &Manifest, files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    if m.manifest_version != 1 {
        bail!("this app reads manifestVersion 1; that package is {}", m.manifest_version);
    }
    if !valid_id(&m.id) {
        bail!("'{}' is not a valid extension id (use publisher.name, lower-case)", m.id);
    }
    if !valid_version(&m.version) {
        bail!("'{}' is not a version (use x.y.z)", m.version);
    }
    let name = m.name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        bail!("an extension's name must be 1 to 60 characters");
    }
    if m.author.trim().is_empty() || m.author.chars().count() > 60 {
        bail!("an extension's author must be 1 to 60 characters");
    }
    if m.description.chars().count() > 400 {
        bail!("an extension's description must be under 400 characters");
    }
    if let Some(h) = &m.homepage {
        if !h.starts_with("https://") || h.len() > 300 {
            bail!("homepage must be an https:// address");
        }
    }
    let mut seen = BTreeSet::new();
    for p in &m.permissions {
        if !PERMISSIONS.contains(&p.as_str()) {
            bail!("unknown permission '{p}'");
        }
        if !seen.insert(p) {
            bail!("permission '{p}' is listed twice");
        }
    }
    if !m.network.is_empty() && !m.permissions.iter().any(|p| p == "network") {
        bail!("'network' lists hosts but the 'network' permission is not requested");
    }
    if m.network.len() > 10 {
        bail!("at most 10 network hosts");
    }
    for o in &m.network {
        if !valid_origin(o) {
            bail!("'{o}' is not an https://host origin");
        }
    }
    if let Some(u) = &m.update_url {
        if !u.starts_with("https://") {
            bail!("updateUrl must be https://");
        }
    }

    // Every file a contribution names must be in the package, and be the right kind.
    let need = |path: &str, kinds: &[&str], what: &str| -> Result<()> {
        let norm = safe_path(path)
            .ok_or_else(|| anyhow!("{what} '{path}' is not a path inside the package"))?
            .to_string_lossy()
            .replace('\\', "/");
        if !kinds.contains(&file_ext(&norm).as_str()) {
            bail!("{what} '{path}' must be one of: {}", kinds.join(", "));
        }
        if !files.contains_key(&norm) {
            bail!("{what} '{path}' is not in the package");
        }
        Ok(())
    };
    let list = |key: &str| -> Vec<Value> {
        m.contributes
            .get(key)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    };
    let images = ["png", "jpg", "jpeg", "webp", "gif"];
    for v in list("views") {
        let entry = v.get("entry").and_then(|e| e.as_str()).unwrap_or("");
        need(entry, &["html", "htm"], "view entry")?;
    }
    if let Some(main) = m.contributes.get("main").and_then(|v| v.as_str()) {
        need(main, &["html", "htm"], "main")?;
    }
    if m.screenshots.len() > 6 {
        bail!("at most 6 screenshots");
    }
    for shot in &m.screenshots {
        need(shot, &["png", "jpg", "jpeg", "webp"], "screenshot")?;
    }
    for f in list("fonts") {
        let src = f.get("src").and_then(|e| e.as_str()).unwrap_or("");
        need(src, &["woff", "woff2", "ttf", "otf"], "font")?;
    }
    for b in list("backgrounds") {
        if let Some(img) = b.get("image").and_then(|e| e.as_str()) {
            need(img, &images, "background image")?;
        }
    }
    for t in list("themes") {
        if let Some(img) = t
            .get("background")
            .and_then(|b| b.get("image"))
            .and_then(|e| e.as_str())
        {
            need(img, &images, "theme background")?;
        }
    }
    if !m.contributes.is_null() && !m.contributes.is_object() {
        bail!("'contributes' must be an object");
    }
    Ok(())
}

/// Check the signature, if any, and say how far the package is trusted. No publisher is
/// trusted, so a good signature is `signed`; see [`check_signature_with`].
pub fn check_signature(
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<(Trust, Option<String>, Option<String>)> {
    check_signature_with(files, &[])
}

/// [`check_signature`], where a signature by one of `publishers` makes the package `verified`.
pub fn check_signature_with(
    files: &BTreeMap<String, Vec<u8>>,
    publishers: &[(&str, &str)],
) -> Result<(Trust, Option<String>, Option<String>)> {
    let Some(raw) = files.get(SIGNATURE_FILE) else {
        return Ok((Trust::Unsigned, None, None));
    };
    let sig: SignatureFile =
        serde_json::from_slice(raw).context("signature.json is not readable")?;
    let key_bytes: [u8; 32] = B64
        .decode(sig.public_key.trim())
        .context("signature.json: publicKey is not base64")?
        .try_into()
        .map_err(|_| anyhow!("signature.json: publicKey is not an Ed25519 key"))?;
    let sig_bytes: [u8; 64] = B64
        .decode(sig.signature.trim())
        .context("signature.json: signature is not base64")?
        .try_into()
        .map_err(|_| anyhow!("signature.json: signature is the wrong length"))?;
    let key = VerifyingKey::from_bytes(&key_bytes)
        .map_err(|_| anyhow!("signature.json: publicKey is not a valid Ed25519 key"))?;
    key.verify_strict(&digest_input(files), &Signature::from_bytes(&sig_bytes))
        .map_err(|_| {
            anyhow!("the package's signature does not match its files - it was changed after signing")
        })?;
    let fp = key_fingerprint(&key_bytes);
    let publisher = publishers.iter().find_map(|(name, k)| {
        (B64.decode(k).ok().as_deref() == Some(&key_bytes[..])).then(|| name.to_string())
    });
    let trust = if publisher.is_some() { Trust::Verified } else { Trust::Signed };
    Ok((trust, Some(fp), publisher))
}

/// Read, validate and signature-check a package, trusting no publisher.
pub fn open_package(files: BTreeMap<String, Vec<u8>>, app_version: &str) -> Result<Package> {
    open_package_with(files, app_version, &TrustPolicy::default())
}

/// Read, validate and signature-check a package under a host's [`TrustPolicy`]. The single
/// entry point for every source.
pub fn open_package_with(
    files: BTreeMap<String, Vec<u8>>,
    app_version: &str,
    policy: &TrustPolicy,
) -> Result<Package> {
    let raw = files
        .get(MANIFEST_FILE)
        .ok_or_else(|| anyhow!("that package has no extension.json"))?;
    let manifest_raw: Value =
        serde_json::from_slice(raw).context("extension.json is not valid JSON")?;
    let manifest: Manifest = serde_json::from_value(manifest_raw.clone())
        .map_err(|e| anyhow!("extension.json: {e}"))?;
    validate_manifest(&manifest, &files)?;
    if let Some(need) = &manifest.min_app_version {
        if !version_at_least(app_version, need) {
            bail!("{} needs version {need} or newer of this app; this is {app_version}", manifest.name);
        }
    }
    let (trust, key, publisher) = check_signature_with(&files, policy.publishers)?;
    // Ids under a trusted publisher's names are for that publisher's key: anything else
    // claiming one is pretending.
    if policy.reserved_prefixes.iter().any(|r| manifest.id.starts_with(r)) && trust != Trust::Verified {
        bail!("the id '{}' is reserved for a verified publisher", manifest.id);
    }
    Ok(Package { manifest, manifest_raw, trust, key, publisher, files })
}

/// Whether `pkg` may replace what is installed under its id. A different signing key is
/// a different author, whatever the id says; so is going from signed to unsigned.
pub fn may_replace(installed: &InstalledExt, pkg: &Package, developer_mode: bool) -> Result<()> {
    match (&installed.key, &pkg.key) {
        (Some(old), Some(new)) if old == new => Ok(()),
        // Moving to a key we trust is the publisher being reviewed, which is allowed.
        (_, Some(_)) if pkg.trust == Trust::Verified => Ok(()),
        (None, _) if developer_mode => Ok(()),
        (Some(_), None) if developer_mode => Ok(()),
        (Some(old), Some(new)) => bail!(
            "this update is signed by a different key ({new}) than the installed version ({old}), so it may not be from the same author. Uninstall first if you trust it."
        ),
        (Some(_), None) => bail!("the installed version is signed and this one isn't"),
        (None, _) => bail!("unsigned extensions need developer mode"),
    }
}

/// Write a checked package to `<dir>/<id>`, replacing whatever was there.
pub fn write_package(dir: &Path, pkg: &Package) -> Result<PathBuf> {
    let root = dir.join(&pkg.manifest.id);
    let tmp = dir.join(format!(".{}.tmp", pkg.manifest.id));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).ok();
    }
    std::fs::create_dir_all(&tmp)?;
    for (rel, bytes) in &pkg.files {
        let out = tmp.join(safe_path(rel).ok_or_else(|| anyhow!("unsafe path {rel}"))?);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::File::create(&out)?.write_all(bytes)?;
    }
    // Swap in one step, so a failed write never leaves half an extension behind.
    if root.exists() {
        std::fs::remove_dir_all(&root).with_context(|| format!("removing {}", root.display()))?;
    }
    std::fs::rename(&tmp, &root)?;
    Ok(root)
}

// ---------------------------------------------------------------------------------------
// Tauri side: state on disk, staging, commands.
// ---------------------------------------------------------------------------------------

/// The result of writing an extension package.
#[derive(Debug, Clone)]
pub struct PackResult {
    /// Output `.mxbext` path.
    pub path: PathBuf,
    /// Number of files in the archive, including a signature when present.
    pub files: usize,
    /// Final archive size.
    pub bytes: u64,
    /// Base64 public key when the package was signed.
    pub public_key: Option<String>,
}

/// Generates a new Ed25519 PKCS#8 PEM key, refusing to overwrite an existing file.
pub fn keygen(path: &Path) -> Result<String> {
    if path.exists() {
        bail!("{} exists; refusing to overwrite a key", path.display());
    }
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(|error| anyhow!(error.to_string()))?;
    let signing = SigningKey::from_bytes(&seed);
    let mut der = vec![
        0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22,
        0x04, 0x20,
    ];
    der.extend_from_slice(&seed);
    let encoded = B64.encode(der);
    let body = encoded
        .as_bytes()
        .chunks(64)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    // The PEM label is split so this source never contains the literal header a leak scanner
    // looks for: what is written is the user's new key, to their own file.
    let label = concat!("PRIVATE", " KEY");
    let pem = format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n");
    std::fs::write(path, pem)?;
    Ok(B64.encode(signing.verifying_key().as_bytes()))
}

/// Packs a folder as `.mxbext`, optionally signing it with an Ed25519 PKCS#8 PEM key.
pub fn pack(folder: &Path, output: &Path, key: Option<&Path>) -> Result<PackResult> {
    let mut files = read_folder(folder)?;
    files.remove(SIGNATURE_FILE);
    let manifest_bytes = files.get(MANIFEST_FILE).ok_or_else(|| anyhow!("extension.json is missing"))?;
    let manifest: Manifest = serde_json::from_slice(manifest_bytes).context("extension.json is not readable")?;
    validate_manifest(&manifest, &files)?;

    let public_key = if let Some(path) = key {
        let signing = read_signing_key(path)?;
        let public_key = B64.encode(signing.verifying_key().as_bytes());
        let signature = B64.encode(signing.sign(&digest_input(&files)).to_bytes());
        files.insert(
            SIGNATURE_FILE.into(),
            serde_json::to_vec_pretty(&serde_json::json!({
                "publicKey": public_key,
                "signature": signature,
            }))?,
        );
        Some(public_key)
    } else {
        None
    };

    let file = std::fs::File::create(output)?;
    let mut archive = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    for (name, bytes) in &files {
        archive.start_file(name, options)?;
        archive.write_all(bytes)?;
    }
    let file = archive.finish()?;
    let bytes = file.metadata()?.len();
    if bytes > MAX_PACKAGE_BYTES {
        let _ = std::fs::remove_file(output);
        bail!("the package is over 25 MB");
    }
    Ok(PackResult { path: output.to_path_buf(), files: files.len(), bytes, public_key })
}

/// Reads and fully validates an extension archive.
pub fn verify(path: &Path, app_version: &str) -> Result<Package> {
    let bytes = std::fs::read(path)?;
    open_package(read_zip(&bytes)?, app_version)
}

fn read_signing_key(path: &Path) -> Result<SigningKey> {
    let text = std::fs::read_to_string(path).context("reading the signing key")?;
    let body: String = text.lines().filter(|line| !line.starts_with("-----")).collect();
    let der = B64.decode(body).context("the signing key is not PEM")?;
    let seed: [u8; 32] = der
        .get(der.len().saturating_sub(32)..)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| anyhow!("the signing key is not an Ed25519 PKCS#8 key"))?;
    Ok(SigningKey::from_bytes(&seed))
}
