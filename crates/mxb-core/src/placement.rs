//! Filesystem-only install placement, moved from the manager verbatim.

use anyhow::Context;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

pub(crate) const STAGED_DIR: &str = "extracted";

/// Does `p` end in `ext`, whatever case it was written in?
pub fn has_ext(p: &Path, ext: &str) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(ext))
        .unwrap_or(false)
}

/// A file that came along with a mod but is not part of it — a readme, a link, a notes file.
pub fn is_junk(name: &str) -> bool {
    let n = name.to_lowercase();
    n.starts_with("readme")
        || n.ends_with(".txt")
        || n.ends_with(".url")
        || n.ends_with(".nfo")
        || n.ends_with(".md")
}

/// Flatten the characters a filesystem will not take. Not a path sanitiser — separators are
/// replaced rather than rejected, because this names a single file, never a path.
pub fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect()
}

/// Every content folder name any known title puts directly under `mods/`.
///
/// The union across titles, not one game's list, because its job is *recognition*: when
/// an extracted archive already contains a `mods/<something>/` tree, this is what decides
/// that it does. Routing — where a file actually lands — is per-game and uses
/// [`GameProfile::mods_dirs`]. Being generous here costs nothing (an MX Bikes archive
/// simply never contains a GP-only folder) and being narrow would silently mis-place a
/// correctly-packed archive.
pub const ALL_MODS_DIRS: [&str; 5] = ["bikes", "tracks", "rider", "tyres", "misc"];

/// Every folder under `mods/rider` that holds *models*, each model a folder of its own.
///
/// The union across titles, for the reason [`ALL_MODS_DIRS`] is one: its job is recognition
/// — deciding whether a chosen destination names an *area* (so what is being installed is a
/// new model, which needs a folder) or names a model already (so the content merges into
/// it). Routing stays per-game. `riders` is listed too: a profile is a folder of files
/// exactly as a helmet is, even though it carries no [`RiderArea`] of its own.
pub const RIDER_MODEL_AREAS: [&str; 6] =
    ["helmets", "boots", "protections", "protection", "animations", "riders"];

/// Whether `name` is one of [`RIDER_MODEL_AREAS`], case-insensitively.
pub fn is_rider_model_area(name: &str) -> bool {
    RIDER_MODEL_AREAS.iter().any(|a| a.eq_ignore_ascii_case(name))
}

pub(crate) fn read_identity(path: &Path) -> Option<()> {
    let stem = path.file_name()?.to_string_lossy().into_owned();
    let ini = std::fs::read(path.join(format!("{stem}.ini"))).ok();
    let cfg = std::fs::read(path.join(format!("{stem}.cfg"))).ok();
    (ini.is_some() || cfg.is_some()).then_some(())
}

pub fn detect_ext(archive: &Path) -> anyhow::Result<String> {
    if let Some(ext) = archive
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
    {
        if ["zip", "7z", "rar", "pkz", "pnt"].contains(&ext.as_str()) {
            return Ok(ext);
        }
    }
    // Sniff magic bytes when the name has no useful extension.
    let mut buf = [0u8; 8];
    let n = File::open(archive)?.read(&mut buf)?;
    let magic = &buf[..n];
    if magic.starts_with(b"PK\x03\x04") || magic.starts_with(b"PK\x05\x06") {
        return Ok("zip".to_string());
    }
    if magic.starts_with(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return Ok("7z".to_string());
    }
    if magic.starts_with(b"Rar!") {
        return Ok("rar".to_string());
    }
    anyhow::bail!("Could not determine the archive type of the downloaded file.")
}
/// Content categories that live directly under `mods/`, across every title the app
/// drives — see [`crate::game::ALL_MODS_DIRS`] for why this is a union rather than the
/// active game's own list. Re-exported because the dropzone reports on it too.
pub use crate::game::ALL_MODS_DIRS as CATEGORY_DIRS;

/// Which rule in [`plan_placement`] decided the destination.
///
/// The dropzone renders this as the reason it shows the user ("found engine.scl + sfx.cfg"),
/// so the explanation and the routing can never disagree — they come from the same match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteRule {
    /// The archive carries a whole `mods/` tree; the caller's category is ignored.
    ModsTree,
    /// The archive root holds `bikes/`, `tracks/`, … — merged category by category.
    CategoryDirs,
    /// A `<Bike>/paints/…` bundle: bike content whatever the caller asked for.
    PaintsBundle,
    /// `<Bike>/{engine.scl,sfx.cfg}` — a packaged sound set.
    SoundBundle,
    /// Loose `engine.scl`+`sfx.cfg` at the root, destined for a bike the caller picked.
    LooseSound,
    /// A loose tyre folder identified by its front/rear wheel meshes.
    TyreSet,
    /// Nothing self-describing was found; the caller's category decides.
    Typed,
}

/// What a placement will actually do, decided but not yet performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// Merge one tree into one destination.
    Merge { src: PathBuf, dst: PathBuf },
    /// Merge several trees, each into its own destination (the category dirs).
    MergeEach { pairs: Vec<(PathBuf, PathBuf)> },
    /// The `place_plain` rules: `.pkz`-at-root wins, junk is dropped, loose files may
    /// get wrapped in a folder of their own.
    Plain {
        src: PathBuf,
        dst: PathBuf,
        slug: String,
        wrap_loose: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub rule: RouteRule,
    pub placement: Placement,
}

/// Decide where `extracted` belongs — without touching the filesystem.
///
/// Split out of [`place_mod`] so a preview and the write that follows it are produced by the
/// same code: the dropzone shows the user what [`writes_for`] enumerates, then commits by
/// applying the very same [`Placement`].
pub fn plan_placement(
    extracted: &Path,
    mods_dir: &Path,
    type_folder: &str,
    dest_folder: &str,
    slug: &str,
) -> Route {
    let route = |rule, placement| Route { rule, placement };

    // Check the extracted root FIRST so a `<Bike>/paints/` bundle isn't unwrapped to bare `paints/`.
    let unwrapped = unwrap_wrapper(extracted);
    let candidates: Vec<&Path> = if unwrapped == extracted {
        vec![extracted]
    } else {
        vec![extracted, unwrapped.as_path()]
    };

    // MX Bikes kits use `[mods/]rider/<profile>/paints` inside their package. That is the
    // standard package layout, not the final on-disk path: the game reads the selected
    // profile's kit from `rider/riders/<profile>/paints`. The generic self-describing-tree
    // rules below would trust the package wrapper and override the profile the user picked,
    // placing the kit somewhere the game never reads. Apply the required `riders` segment
    // only when the caller identified profile paints and the package contains exactly one
    // kit folder. That keeps real gear trees (`rider/helmets`, `boots`, and so on) and
    // multi-profile packs on the ordinary review path.
    let dest_segs: Vec<&str> = dest_folder.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
    let profile_paints = type_folder.eq_ignore_ascii_case("rider")
        && matches!(dest_segs.as_slice(), [riders, _, paints]
            if riders.eq_ignore_ascii_case("riders") && paints.eq_ignore_ascii_case("paints"));
    if profile_paints {
        for base in &candidates {
            let rider = child_dir(base, "rider").or_else(|| {
                let mods = child_dir(base, "mods")?;
                child_dir(&mods, "rider")
            });
            let Some(rider) = rider else {
                continue;
            };
            if child_dir(&rider, "riders").is_some() {
                continue;
            }
            let kits: Vec<PathBuf> = std::fs::read_dir(&rider)
                .into_iter()
                .flatten()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                .filter_map(|entry| child_dir(&entry.path(), "paints"))
                .collect();
            if let [paints] = kits.as_slice() {
                let mut dst = mods_dir.join(type_folder);
                for seg in &dest_segs {
                    dst.push(sanitize(seg));
                }
                return route(
                    RouteRule::Typed,
                    Placement::Merge {
                        src: paints.clone(),
                        dst,
                    },
                );
            }
        }
    }

    for base in &candidates {
        if let Some(m) = child_dir(base, "mods") {
            // Some sound packs keep shared samples in a real `mods/` tree and put the
            // bike-specific sample list beside it in an INSTALL folder. When the caller
            // already has a bike choice, carry both halves instead of returning after the
            // tree and silently dropping the file that switches the sound on.
            if !dest_folder.trim().is_empty() {
                let sound_folders: Vec<PathBuf> = std::fs::read_dir(base)
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                    .map(|entry| entry.path())
                    .filter(|dir| dir != &m && is_sound_fragment(dir))
                    .collect();
                if let [sound] = sound_folders.as_slice() {
                    return route(
                        RouteRule::ModsTree,
                        Placement::MergeEach {
                            pairs: vec![
                                (m, mods_dir.to_path_buf()),
                                (sound.clone(), sound_destination(mods_dir, dest_folder)),
                            ],
                        },
                    );
                }
            }
            return route(
                RouteRule::ModsTree,
                Placement::Merge {
                    src: m,
                    dst: mods_dir.to_path_buf(),
                },
            );
        }
    }

    for base in &candidates {
        let cats: Vec<PathBuf> = CATEGORY_DIRS
            .iter()
            .filter_map(|c| child_dir(base, c))
            .collect();
        if !cats.is_empty() {
            let pairs = cats
                .into_iter()
                .map(|c| {
                    let dst = mods_dir.join(c.file_name().unwrap_or_default());
                    (c, dst)
                })
                .collect();
            return route(RouteRule::CategoryDirs, Placement::MergeEach { pairs });
        }
    }

    // Tyre downloads are often filed under a site's Bikes category, but the game never
    // looks for their wheel meshes there. The folder identifies itself by the pair of wheel
    // definitions and its compiled model, so keep its own name under `mods/tyres` regardless
    // of the category that supplied the download.
    for base in &candidates {
        // Only a folder the author named: tyre files loose at the archive root would
        // otherwise be filed under the staging folder's own name (`mods/tyres/extracted`),
        // which no bike asks for. Those keep the category route and the review.
        if is_staging_folder_name(base) {
            continue;
        }
        if is_tyre_set(base) {
            let name = base.file_name().unwrap_or_default();
            return route(
                RouteRule::TyreSet,
                Placement::Merge {
                    src: base.to_path_buf(),
                    dst: mods_dir.join("tyres").join(name),
                },
            );
        }
    }

    // A `<Bike>/paints/…` bundle is bike content → route to `mods/bikes` regardless of
    // the caller's default type. Rider paints are exempt (kept under `mods/rider` below).
    if !type_folder.eq_ignore_ascii_case("rider") {
        for base in &candidates {
            if contains_paints_bundle(base) {
                return route(
                    RouteRule::PaintsBundle,
                    Placement::Merge {
                        src: base.to_path_buf(),
                        dst: mods_dir.join("bikes"),
                    },
                );
            }
        }
    }

    // Sound bundle (`engine.scl`+`sfx.cfg`): bike content that belongs at the bike root,
    // NEVER inside `paints/` → route to `mods/bikes` and drop any trailing `paints` segment.
    for base in &candidates {
        if contains_sound_bundle(base) {
            let sets = sound_bundle_dirs(base);
            // A single wrapper named after the download is not a bike. When the user has
            // already picked a bike, unwrap that folder and put the sound in the bike root.
            // A folder that really names an installed bike keeps its authored layout, and a
            // pack with no bike choice stays a bundle for the review sheet to split.
            if !dest_folder.trim().is_empty()
                && matches!(sets.as_slice(), [set] if !is_installed_bike_folder(mods_dir, set))
            {
                return route(
                    RouteRule::LooseSound,
                    Placement::Plain {
                        src: sets[0].clone(),
                        dst: sound_destination(mods_dir, dest_folder),
                        slug: slug.to_string(),
                        wrap_loose: false,
                    },
                );
            }
            // `<Bike>/{engine.scl,sfx.cfg}` — merge the bike folder(s) as-is.
            return route(
                RouteRule::SoundBundle,
                Placement::Merge {
                    src: base.to_path_buf(),
                    dst: mods_dir.join("bikes"),
                },
            );
        }
        if is_sound_set(base) {
            // Loose `engine.scl`+`sfx.cfg` — drop into the chosen bike's root.
            return route(
                RouteRule::LooseSound,
                Placement::Plain {
                    src: base.to_path_buf(),
                    dst: sound_destination(mods_dir, dest_folder),
                    slug: slug.to_string(),
                    wrap_loose: false,
                },
            );
        }
    }

    // Plain placement into the type folder, honoring the chosen destination sub-folder.
    let mut type_dir = mods_dir.join(type_folder);
    let segs: Vec<&str> = dest_folder.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
    for seg in &segs {
        type_dir.push(sanitize(seg));
    }
    if let Some(name) = gear_model_folder(type_folder, &segs, extracted, &unwrapped, slug) {
        type_dir.push(name);
    }

    // A bike livery only loads from `<Bike>/paints/`. The picker offers a bike's root as a
    // destination too — that's where sounds and model swaps go — so a paint chosen there
    // would land one folder high and never show up in game. Drop it into `paints/` instead:
    // a loose `.pnt` at a bike root does nothing whatever, so this can only help.
    if type_folder.eq_ignore_ascii_case("bikes")
        && !segs.is_empty()
        && !segs[segs.len() - 1].eq_ignore_ascii_case("paints")
        && is_loose_paint_drop(&unwrapped)
    {
        type_dir.push("paints");
    }
    // The destination is already `<Bike>/paints` and the pack carries a `paints` folder of its
    // own beside a stray file or two (a preview image, say), so it was not unwrapped. Walking
    // it as-is copied the folder *into* the destination, `<Bike>/paints/paints/x.pnt`, which
    // the game never lists. The pack's own `paints` folder is what the destination names.
    let mut unwrapped = unwrapped;
    if type_folder.eq_ignore_ascii_case("bikes")
        && segs.last().is_some_and(|s| s.eq_ignore_ascii_case("paints"))
        && !has_root_pkz(&unwrapped)
    {
        if let Some(inner) = child_dir(&unwrapped, "paints") {
            unwrapped = inner;
        }
    }
    // Extracted tracks need their own folder; loose bike paints don't.
    let wrap_loose = type_folder.eq_ignore_ascii_case("tracks");
    route(
        RouteRule::Typed,
        Placement::Plain {
            src: unwrapped,
            dst: type_dir,
            slug: slug.to_string(),
            wrap_loose,
        },
    )
}

/// The folder a whole gear model needs under its area, or `None` when this placement isn't
/// installing one.
///
/// A gear model *is* a folder: the game loads `helmets/<Model>/helmet.edf`, and every picker
/// lists the folders an area contains. But the destination offered for a new model is the
/// bare area (`helmets`), and [`unwrap_wrapper`] has by then stripped the mod's own folder —
/// which is exactly the shape a packaged `.pkz` extracts to. Without this the model's files
/// landed loose in `mods/rider/helmets`, where nothing lists them and nothing can load them,
/// and the area picked up `paints`/`goggles` as if those were models of their own.
///
/// Named as the mod named itself, because that name is what the pickers will show. A mod
/// that ships its files bare has only the slug to go on.
///
/// Three placements are deliberately left alone. A paint drop names the model it belongs to
/// (`helmets/<Model>/paints`), so it is more than one segment and merges as before. An
/// archive packed area-first — a `helmets/` folder holding the models — is already in the
/// shape the destination expects; wrapping it would bury the models a level down. And a
/// packaged model is a single `.pkz` that belongs *directly* in the area, under its own file
/// name: [`walk_plain`] already places it that way, and a folder around it puts the package
/// one level below where the game and every picker look — `helmets/shop-44/<Helmet>.pkz`,
/// which loads nowhere. A shop download is exactly that shape, and having no folder of its
/// own it was named for the slug.
pub fn gear_model_folder(
    type_folder: &str,
    segs: &[&str],
    extracted: &Path,
    unwrapped: &Path,
    slug: &str,
) -> Option<String> {
    if !type_folder.eq_ignore_ascii_case("rider") {
        return None;
    }
    let [area] = segs else { return None };
    if !crate::game::is_rider_model_area(area) {
        return None;
    }
    // The same base and the same test `walk_plain` will apply, so the two can't drift.
    if has_root_pkz(unwrapped) {
        return None;
    }
    let own = (unwrapped != extracted)
        .then(|| unwrapped.file_name()?.to_str())
        .flatten()
        .filter(|n| !crate::game::is_rider_model_area(n));
    let name = sanitize(own.unwrap_or(slug));
    (!name.trim().is_empty()).then_some(name)
}
/// What a placement does with a file that is already sitting at the destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnConflict {
    /// Overwrite it. Re-installing a mod is how a player updates it.
    Overwrite,
    /// Keep what's on disk. A preset bundle carries whole asset folders — the sender's
    /// helmet mesh rides along with the one paint they meant to share — so an import must
    /// fill in what's missing and never trade the receiver's copy for the sender's.
    Keep,
}

/// Whether a placement may take the source files away with it.
///
/// A downloaded mod is already on disk twice — the archive and the unpacked copy, both in a
/// staging directory we delete moments later. Copying a third time is the slowest part of an
/// install once the bytes are down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Staging {
    /// The source is ours and about to be deleted, so a file may be *moved* into place — on
    /// one volume that's a rename, and no bytes move.
    Consume,
    /// The source has to survive. The dropzone is why this is the default: it installs a
    /// folder straight from wherever the user keeps it.
    Preserve,
}
/// Every `(source file, destination file)` a placement would produce, in order.
///
/// This is the single enumeration behind both the dropzone's preview and [`apply`]'s copy
/// loop, which is what makes "what the review sheet promised" and "what landed on disk"
/// the same list by construction rather than by careful maintenance.
pub fn writes_for(placement: &Placement) -> Vec<(PathBuf, PathBuf)> {
    let mut out = Vec::new();
    match placement {
        Placement::Merge { src, dst } => walk_merge(src, dst, &mut out),
        Placement::MergeEach { pairs } => {
            for (src, dst) in pairs {
                walk_merge(src, dst, &mut out);
            }
        }
        Placement::Plain {
            src,
            dst,
            slug,
            wrap_loose,
        } => walk_plain(src, dst, slug, *wrap_loose, &mut out),
    }
    out
}

/// The directories a placement needs to exist even when it writes no files into them.
///
/// `place_plain` used to `create_dir_all` its target up front, and a track that extracts to
/// an empty folder still wants that folder; enumerating writes alone would silently drop it.
pub fn roots_for(placement: &Placement) -> Vec<PathBuf> {
    match placement {
        Placement::Merge { dst, .. } => vec![dst.clone()],
        Placement::MergeEach { pairs } => pairs.iter().map(|(_, dst)| dst.clone()).collect(),
        Placement::Plain { dst, .. } => vec![dst.clone()],
    }
}

/// Perform a placement, naming the file that failed.
///
/// The errors here reach the user verbatim, and an unadorned `io::Error` is close to
/// useless in a bug report: "os error 2" says a path was missing without saying which one,
/// or even whether it was the source or the destination. Every step says what it was doing
/// to what, and a failure is logged as well as returned — the toast is transient, the log
/// is what a player can send back.
/// Refuse a placement that would write outside the mods folder.
///
/// `type_folder` and `dest_folder` are joined onto `mods_dir`, and `Path::join` is happy to
/// accept `..` in either — a file-share code picks its type folder from a path the sender
/// wrote, and the dropzone's destination comes back from the frontend. Every install funnels
/// through [`place_mod_with`], so the check belongs here rather than at each of its doors.
pub fn guard_placement(mods_dir: &Path, placement: &Placement) -> anyhow::Result<()> {
    let inside = |dst: &Path| {
        // `starts_with` alone would pass `<mods>/..`: joining never normalises, so the
        // climb is still sitting there as a component.
        dst
            .strip_prefix(mods_dir)
            .map(|rel| !rel.components().any(|c| c == Component::ParentDir))
            .unwrap_or(false)
    };
    for root in roots_for(placement) {
        if !inside(&root) {
            anyhow::bail!(
                "refusing to install to {} — that's outside the mods folder",
                root.display()
            );
        }
        if root.exists() && !root.is_dir() {
            anyhow::bail!("refusing to replace {} with a folder", root.display());
        }
    }
    for (_, dst) in writes_for(placement) {
        if !inside(&dst) {
            anyhow::bail!("refusing to install to {} — that's outside the mods folder", dst.display());
        }
        // A file landing where a folder already is would have to take the folder's place:
        // a bike unpacked as `UserBike.pkz/` by hand, say. Never that.
        if dst.exists() && !dst.is_file() {
            anyhow::bail!("refusing to replace the folder {} with a file", dst.display());
        }
    }
    Ok(())
}

pub fn apply(
    placement: &Placement,
    on_conflict: OnConflict,
    staging: Staging,
) -> anyhow::Result<usize> {
    for root in roots_for(placement) {
        std::fs::create_dir_all(&root)
            .with_context(|| format!("creating {}", root.display()))?;
    }
    let writes = writes_for(placement);
    let mut written = 0;
    let mut kept = 0;
    for (src, dst) in &writes {
        // The skip sits here rather than in `writes_for` so the enumeration stays the one
        // description of what a placement covers — the dropzone's preview still lists every
        // file, whatever this install then decides to leave alone.
        if on_conflict == OnConflict::Keep && dst.exists() {
            kept += 1;
            continue;
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        // Only ever a shortcut: a cross-drive move fails, so does a held file, and both fall
        // through to `copy_staged` — which is where the wait-out-the-scanner retry lives.
        if staging == Staging::Consume && std::fs::rename(src, dst).is_ok() {
            written += 1;
            continue;
        }
        copy_staged(src, dst)?;
        written += 1;
    }
    if kept > 0 {
        log::info!("kept {kept} file(s) already on disk, wrote {written}");
    }
    Ok(written)
}

/// How long [`copy_staged`] keeps waiting out a copy that still looks transient.
///
/// Two seconds all told: long enough for a scanner to finish with a freshly unpacked file,
/// short enough that a genuinely doomed install still fails while the user is watching.
pub const COPY_ATTEMPTS: u32 = 13;
pub const COPY_RETRY_WAIT: Duration = Duration::from_millis(150);

/// Copy one staged file into place, waiting out a failure that may not be permanent.
///
/// Every source here was listed by [`writes_for`]'s `read_dir` moments earlier, so a copy
/// that reports the file as missing is describing something that happened *since* — and on
/// Windows that is nearly always a real-time virus scanner reacting to a mod file appearing
/// in `%TEMP%`. Track `.pkz`es are a standing false positive: the scanner opens the new file
/// exclusively (or quarantines it outright) in the gap between the walk and the copy, and the
/// install died with a bare "The system cannot find the file specified. (os error 2)" naming
/// a file that had been there a moment earlier. Most such holds are released within a moment
    /// — the integration manager already waits one out on its own binary — so wait here too
/// rather than fail a whole install over it.
pub fn copy_staged(src: &Path, dst: &Path) -> anyhow::Result<()> {
    let mut waited = false;
    for attempt in 1..=COPY_ATTEMPTS {
        let err = match std::fs::copy(src, dst) {
            Ok(_) => {
                if waited {
                    // Worth a line: it says the install only survived because of the wait,
                    // which is the signal that a player's scanner needs an exclusion.
                    log::warn!(
                        "copying {} succeeded on attempt {attempt} — something was holding it",
                        src.display()
                    );
                }
                return Ok(());
            }
            Err(e) => e,
        };
        if attempt == COPY_ATTEMPTS || !worth_retrying(dst, &err) {
            return Err(copy_failure(src, dst, err));
        }
        waited = true;
        std::thread::sleep(COPY_RETRY_WAIT);
    }
    unreachable!("the loop returns on its last attempt")
}

/// Whether a failed copy could plausibly succeed if we tried again in a moment.
///
/// Waiting is only ever worth it for a *transient* holder. A folder sitting where the file
/// has to land is not one — it fails identically forever — so that case fails immediately
/// rather than making the user watch the retries run out.
pub fn worth_retrying(dst: &Path, e: &std::io::Error) -> bool {
    if dst.is_dir() {
        return false;
    }
    matches!(
        e.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
    ) || matches!(
        e.raw_os_error(),
        // ERROR_SHARING_VIOLATION / ERROR_LOCK_VIOLATION: someone else has the file open.
        // Rust has no stable `ErrorKind` for either, so match the Windows codes directly.
        Some(32) | Some(33)
    )
}

/// Turn a failed copy into something a player can act on.
///
/// A staged file the copy can no longer read is the one case where the raw error actively
/// misleads: it reads as if the *mod* were broken, when the bytes downloaded fine and
/// something on the machine took them away afterwards. Name the culprit and the folder to
/// exclude, because "os error 2" leaves a player with nowhere to go.
///
/// The probe is a real read, not [`Path::exists`]: `exists` opens for no access at all, so a
/// scanner blocking a file's *contents* waves it through while `fs::copy`, which asks for
/// `GENERIC_READ`, is told "os error 2". Gating on `exists` meant this never fired.
pub fn copy_failure(src: &Path, dst: &Path, e: std::io::Error) -> anyhow::Error {
    if let Err(probe) = File::open(src) {
        let name = src.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let folder = src
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| src.display().to_string());
        // The copy says what it hit, the probe says why the file won't open; a report needs both.
        log::error!("staged {name} could not be reopened: copy said {e}, opening said {probe}");
        let fate = if probe.kind() == std::io::ErrorKind::NotFound {
            "deleted or quarantined"
        } else {
            "locked against being read"
        };
        return anyhow::anyhow!(
            "{name} could not be read back from the staging folder part-way through the \
             install — it was {fate} after the download finished. The download itself \
             worked, so this is almost always antivirus (mod .pkz files are a common false \
             positive) or a temp-folder cleaner. Add an exclusion for {folder} and install \
             it again."
        );
    }
    anyhow::Error::new(e).context(format!("copying {} to {}", src.display(), dst.display()))
}

pub fn walk_merge(src: &Path, dst: &Path, out: &mut Vec<(PathBuf, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(src) else {
        return;
    };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    // `read_dir` order is filesystem-dependent; sort so a preview and its commit list the
    // same files in the same order (and so tests are reproducible).
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let target = dst.join(entry.file_name());
        match entry.file_type() {
            Ok(t) if t.is_dir() => walk_merge(&entry.path(), &target, out),
            Ok(t) if t.is_file() => out.push((entry.path(), target)),
            // Neither a file nor a directory: a symlink, a junction, or a link whose target
            // has gone away. Handing one of these to `fs::copy` fails the *entire* install
            // over a single entry — a dangling link with "os error 2", a junction with a
            // permission error — so leave it out and say so. Following it is not the
            // alternative: this walk runs over freshly extracted archives too, where a link
            // is a stranger's idea of where our files should go (see `linkwalk`).
            Ok(_) => log::warn!("skipping {} — not a regular file", entry.path().display()),
            Err(e) => log::warn!("skipping {} — {e}", entry.path().display()),
        }
    }
}

pub fn walk_plain(
    base: &Path,
    type_dir: &Path,
    slug: &str,
    wrap_loose: bool,
    out: &mut Vec<(PathBuf, PathBuf)>,
) {
    // A single file is a unit in its own right. Every other caller hands this a directory —
    // a staged archive, a dropped folder — but the dropzone splits a pack into the mods it
    // holds (`dropzone::split_tree`), and one of those is a lone `.pkz` sitting among its
    // fifty siblings. There is no folder to point at, and copying it into one to make a
    // directory would mean 3.8 GB of staging for a pack this size.
    if base.is_file() {
        let name = base.file_name().unwrap_or_default();
        out.push((base.to_path_buf(), type_dir.join(name)));
        return;
    }
    let Ok(rd) = std::fs::read_dir(base) else {
        return;
    };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    let dirs: Vec<PathBuf> = entries
        .iter()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    let files: Vec<PathBuf> = entries
        .iter()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.path())
        .collect();

    let has_pkz = files.iter().any(|p| has_ext(p, "pkz"));
    let non_junk_files = files
        .iter()
        .filter(|p| !is_junk(&p.file_name().unwrap_or_default().to_string_lossy()))
        .count();

    // Loose files, no sub-folders: wrap in their own folder, or drop straight in.
    if !has_pkz && dirs.is_empty() && non_junk_files > 0 {
        let target = if wrap_loose {
            type_dir.join(sanitize(slug))
        } else {
            type_dir.to_path_buf()
        };
        walk_merge(base, &target, out);
        return;
    }

    // A `.pkz` is the complete, installable package. When one sits at the root,
    // sibling folders are almost always extras the archive bundles alongside it —
    // the dedicated-"server" build and the unpacked track source. Install ONLY the
    // `.pkz` file(s) so those extras don't get dumped into the game folder.
    if has_pkz {
        for p in files.iter().filter(|p| has_ext(p, "pkz")) {
            let name = p.file_name().unwrap_or_default();
            out.push((p.clone(), type_dir.join(name)));
        }
        return;
    }

    for p in &files {
        let name = p.file_name().unwrap_or_default();
        if is_junk(&name.to_string_lossy()) {
            continue;
        }
        out.push((p.clone(), type_dir.join(name)));
    }
    for d in &dirs {
        let name = d.file_name().unwrap_or_default();
        walk_merge(d, &type_dir.join(name), out);
    }
}

pub fn unwrap_wrapper(dir: &Path) -> PathBuf {
    let mut cur = dir.to_path_buf();
    while let Some(next) = unwrap_one(&cur) {
        cur = next;
    }
    cur
}

/// One step of [`unwrap_wrapper`]: the only folder inside `dir`, when it holds nothing else
/// but junk files.
pub fn unwrap_one(dir: &Path) -> Option<PathBuf> {
    let entries: Vec<_> = std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).collect();
    let dirs: Vec<_> = entries
        .iter()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    let only_junk_files = entries
        .iter()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .all(|f| is_junk(&f.file_name().to_string_lossy()));
    (dirs.len() == 1 && only_junk_files).then(|| dirs[0].path())
}

pub fn child_dir(parent: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(parent)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && e.file_name().to_string_lossy().eq_ignore_ascii_case(name)
        })
        .map(|e| e.path())
}

/// Both present in a folder = a sound mod: the `sfx.cfg` that wires the engine up, and the
/// sample list it points at.
///
/// Any `.scl`, not `engine.scl` by name — `sfx.cfg` states which file it means
/// (`scl = engineYZ250F.scl`), and authors do rename it. Requiring the conventional name read
/// a real sound mod as content of no recognisable kind.
pub fn dir_has_sound_markers(dir: &Path) -> bool {
    let (mut scl, mut sfx) = (false, false);
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.filter_map(|e| e.ok()) {
            if !e.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let path = e.path();
            scl |= has_ext(&path, "scl");
            sfx |= e.file_name().to_string_lossy().eq_ignore_ascii_case("sfx.cfg");
        }
    }
    scl && sfx
}

/// A sound set: `engine.scl` + `sfx.cfg`, and nothing saying it is a bike.
///
/// Every bike folder carries those two files as well, so the markers alone do not tell the
/// two apart — the `<Bike>.ini` beside `<Bike>.cfg` does. Without this an unpacked bike read
/// as a sound set, to be emptied into some other bike's folder.
pub fn is_sound_set(dir: &Path) -> bool {
    dir_has_sound_markers(dir) && crate::bikeswap::read_identity(dir).is_none()
}

pub fn is_sound_fragment(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.filter_map(|entry| entry.ok()).any(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_file()) && has_ext(&entry.path(), "scl")
        })
    })
}

pub fn contains_sound_bundle(base: &Path) -> bool {
    !sound_bundle_dirs(base).is_empty()
}

pub fn sound_bundle_dirs(base: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(base)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.path())
                .filter(|path| is_sound_set(path))
                .collect()
        })
        .unwrap_or_default()
}

pub fn is_installed_bike_folder(mods_dir: &Path, folder: &Path) -> bool {
    let Some(name) = folder.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    child_dir(&mods_dir.join("bikes"), name)
        .is_some_and(|installed| crate::bikeswap::read_identity(&installed).is_some())
}

pub fn sound_destination(mods_dir: &Path, dest_folder: &str) -> PathBuf {
    let mut dir = mods_dir.join("bikes");
    for seg in dest_folder.split(['/', '\\']).filter(|s| !s.is_empty()) {
        if !seg.eq_ignore_ascii_case("paints") {
            dir.push(sanitize(seg));
        }
    }
    dir
}

/// A loose tyre set has both wheel definitions and at least one compiled model. Requiring
/// all three markers avoids treating an ordinary bike's wheel override as a complete tyre.
/// A folder the app named while staging (`extracted`, a drop's numbered slot, a
/// `frost-…` work dir) rather than one the mod's author named.
pub fn is_staging_folder_name(dir: &Path) -> bool {
    let name = dir.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    name.is_empty()
        || name == STAGED_DIR
        || name.starts_with("frost-")
        || name.chars().all(|c| c.is_ascii_digit())
}

pub fn is_tyre_set(dir: &Path) -> bool {
    // An unpacked bike can carry its own wheel meshes beside its model; its `<Bike>.ini` and
    // `<Bike>.cfg` say it is a bike, and moving it into `tyres` would take it out of the game.
    if crate::bikeswap::read_identity(dir).is_some() {
        return false;
    }
    let (mut front, mut rear, mut model) = (false, false, false);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        front |= name == "fwheel.hrc";
        rear |= name == "rwheel.hrc";
        model |= has_ext(&entry.path(), "edf");
    }
    front && rear && model
}

pub fn sound_bikes_in(extracted: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(extracted)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_dir() {
            continue;
        }
        let p = entry.path();
        if !dir_has_sound_markers(p) {
            continue;
        }
        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
            if name.eq_ignore_ascii_case("sounds") {
                continue;
            }
            let name = name.to_string();
            if !out.iter().any(|n: &String| n.eq_ignore_ascii_case(&name)) {
                out.push(name);
            }
        }
    }
    out
}

/// Bare paints, ready to drop into a `paints/` folder: at least one `.pnt` sitting loose in
/// `base`, and no `.pkz`. The `.pkz` matters — it's a packaged bike or model set, which
/// belongs at the bike's root, and an archive that ships both is the package, not the paint.
pub fn is_loose_paint_drop(base: &Path) -> bool {
    let mut has_paint = false;
    let Ok(rd) = std::fs::read_dir(base) else {
        return false;
    };
    for e in rd.filter_map(|e| e.ok()) {
        if !e.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = e.path();
        if has_ext(&path, "pkz") {
            return false;
        }
        has_paint |= has_ext(&path, "pnt");
    }
    has_paint
}

pub fn contains_paints_bundle(base: &Path) -> bool {
    std::fs::read_dir(base)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .any(|d| child_dir(&d.path(), "paints").is_some())
        })
        .unwrap_or(false)
}

/// Whether a `.pkz` sits at the top of `dir` — i.e. whether this placement is installing a
/// finished package rather than a mod's loose files.
///
/// The planner's copy of the test [`walk_plain`] makes on the same base from the entries it
/// has already read. Both must answer alike: one decides the package goes in under its own
/// name, the other that no model folder is wrapped around it.
pub fn has_root_pkz(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                .any(|e| has_ext(&e.path(), "pkz"))
        })
        .unwrap_or(false)
}

impl serde::Serialize for RouteRule {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(match self {
            Self::ModsTree => "modsTree",
            Self::CategoryDirs => "categoryDirs",
            Self::PaintsBundle => "paintsBundle",
            Self::SoundBundle => "soundBundle",
            Self::LooseSound => "looseSound",
            Self::TyreSet => "tyreSet",
            Self::Typed => "typed",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place_tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("frost-place-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }

    fn place_mod(
        extracted: &Path,
        mods_dir: &Path,
        type_folder: &str,
        dest_folder: &str,
        slug: &str,
    ) -> anyhow::Result<usize> {
        place_mod_with(
            extracted,
            mods_dir,
            type_folder,
            dest_folder,
            slug,
            OnConflict::Overwrite,
            Staging::Preserve,
        )
    }

    fn place_mod_with(
        extracted: &Path,
        mods_dir: &Path,
        type_folder: &str,
        dest_folder: &str,
        slug: &str,
        on_conflict: OnConflict,
        staging: Staging,
    ) -> anyhow::Result<usize> {
        let route = plan_placement(extracted, mods_dir, type_folder, dest_folder, slug);
        guard_placement(mods_dir, &route.placement)?;
        apply(&route.placement, on_conflict, staging)
    }

    #[test]
    fn detect_ext_sniffs_magic_bytes() -> anyhow::Result<()> {
        let base = std::env::temp_dir().join(format!("frost-magic-{}", std::process::id()));
        std::fs::create_dir_all(&base)?;

        let rar = base.join("download.bin");
        std::fs::write(&rar, b"Rar!\x1a\x07\x01\x00")?;
        assert_eq!(detect_ext(&rar)?, "rar");

        let zip = base.join("blob");
        std::fs::write(&zip, b"PK\x03\x04rest")?;
        assert_eq!(detect_ext(&zip)?, "zip");

        let named = base.join("track.7z");
        std::fs::write(&named, b"not really 7z")?;
        assert_eq!(detect_ext(&named)?, "7z");

        let _ = std::fs::remove_dir_all(&base);
        Ok(())
    }
    #[test]
    fn sanitize_strips_separators() {
        assert_eq!(sanitize("a/b\\c:d"), "a_b_c_d");
    }
    /// Every bike folder holds `engine.scl` + `sfx.cfg`. Read as a loose sound drop, an
    /// unpacked bike gets emptied into whatever bike the user had selected.
    #[test]
    fn an_unpacked_bike_is_not_a_loose_sound_drop() {
        let dir = std::env::temp_dir().join(format!("frost-test-bikesnd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let bike = dir.join("staged/Modded Surron");
        std::fs::create_dir_all(&bike).unwrap();
        for (n, b) in [
            ("Modded Surron.ini", "name = Modded Surron"),
            ("Modded Surron.cfg", "id { surron }"),
            ("engine.scl", "e"),
            ("sfx.cfg", "s"),
        ] {
            std::fs::write(bike.join(n), b).unwrap();
        }

        let mods = dir.join("mods");
        let route = plan_placement(&dir.join("staged"), &mods, "bikes", "KX450", "surron");
        assert_ne!(route.rule, RouteRule::LooseSound);
        assert_ne!(route.rule, RouteRule::SoundBundle);
        let _ = std::fs::remove_dir_all(&dir);
    }
    /// Re-installing a mod is how a player updates it, so the default has to keep clobbering.
    /// Only [`OnConflict::Keep`] — the preset-bundle import — steps around what's on disk.
    #[test]
    fn reinstalling_overwrites_by_default() {
        let root = place_tmp("overwrite");
        let ex = root.join("ex");
        std::fs::create_dir_all(&ex).unwrap();
        std::fs::write(ex.join("track.pkz"), b"new").unwrap();

        let mods = root.join("mods");
        std::fs::create_dir_all(mods.join("tracks")).unwrap();
        std::fs::write(mods.join("tracks/track.pkz"), b"old").unwrap();

        let placed = place_mod(&ex, &mods, "tracks", "", "slug").unwrap();

        assert_eq!(placed, 1);
        assert_eq!(std::fs::read(mods.join("tracks/track.pkz")).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&root);
    }
    /// The saving itself: the file lands without its bytes being written again, so the
    /// staged copy is gone rather than duplicated.
    #[test]
    fn consuming_a_staged_tree_moves_the_files() {
        let root = place_tmp("consume");
        let ex = root.join("ex");
        std::fs::create_dir_all(&ex).unwrap();
        std::fs::write(ex.join("track.pkz"), b"bytes").unwrap();

        let mods = root.join("mods");
        let placed = place_mod_with(
            &ex,
            &mods,
            "tracks",
            "",
            "slug",
            OnConflict::Overwrite,
            Staging::Consume,
        )
        .unwrap();

        assert_eq!(placed, 1);
        assert_eq!(std::fs::read(mods.join("tracks/track.pkz")).unwrap(), b"bytes");
        assert!(!ex.join("track.pkz").exists(), "the staged copy was moved, not copied");
        let _ = std::fs::remove_dir_all(&root);
    }
    /// The dropzone installs from the user's own folder, so the default must not touch it.
    #[test]
    fn preserving_leaves_the_source_alone() {
        let root = place_tmp("preserve");
        let ex = root.join("ex");
        std::fs::create_dir_all(&ex).unwrap();
        std::fs::write(ex.join("track.pkz"), b"bytes").unwrap();

        let mods = root.join("mods");
        let placed = place_mod(&ex, &mods, "tracks", "", "slug").unwrap();

        assert_eq!(placed, 1);
        assert_eq!(std::fs::read(mods.join("tracks/track.pkz")).unwrap(), b"bytes");
        assert_eq!(
            std::fs::read(ex.join("track.pkz")).unwrap(),
            b"bytes",
            "the user's own copy is still theirs"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
    /// Re-installing is how a player updates a mod, so the rename has to replace the
    /// destination rather than fail on it.
    #[test]
    fn consuming_overwrites_an_existing_file() {
        let root = place_tmp("consume-overwrite");
        let ex = root.join("ex");
        std::fs::create_dir_all(&ex).unwrap();
        std::fs::write(ex.join("track.pkz"), b"new").unwrap();

        let mods = root.join("mods");
        std::fs::create_dir_all(mods.join("tracks")).unwrap();
        std::fs::write(mods.join("tracks/track.pkz"), b"old").unwrap();

        let placed = place_mod_with(
            &ex,
            &mods,
            "tracks",
            "",
            "slug",
            OnConflict::Overwrite,
            Staging::Consume,
        )
        .unwrap();

        assert_eq!(placed, 1);
        assert_eq!(std::fs::read(mods.join("tracks/track.pkz")).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn pkz_alongside_server_and_source_folders_installs_only_pkz() {
        // Mirrors the I40 MX bundle: the client `.pkz` plus a dedicated-server
        // folder and the unpacked track source. Only the `.pkz` should install.
        let root = place_tmp("bundle-pkz");
        let ex = root.join("ex");
        touch(&ex.join("I40 MX.pkz"));
        touch(&ex.join("I40 MX server/server.cfg"));
        touch(&ex.join("I40 MX server/I40 MX.pkz"));
        touch(&ex.join("I40 MX!/track.trk"));
        touch(&ex.join("I40 MX!/textures/asphalt.tga"));
        let mods = root.join("mods");
        let placed = place_mod(&ex, &mods, "tracks", "", "i40-mx").unwrap();

        assert_eq!(placed, 1);
        assert!(mods.join("tracks/I40 MX.pkz").exists());
        assert!(!mods.join("tracks/I40 MX server").exists());
        assert!(!mods.join("tracks/I40 MX!").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn places_plain_pkz_into_type_folder() {
        let root = place_tmp("plain");
        let ex = root.join("ex");
        touch(&ex.join("track.pkz"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "tracks", "", "slug").unwrap();
        assert!(mods.join("tracks/track.pkz").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn places_bike_livery_into_bike_paints() {
        let root = place_tmp("livery");
        let ex = root.join("ex");
        touch(&ex.join("MX1OEM_2023_KTM_450_SX-F/paints/cool.pnt"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "bikes", "", "slug").unwrap();
        assert!(mods
            .join("bikes/MX1OEM_2023_KTM_450_SX-F/paints/cool.pnt")
            .exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn livery_bundle_routes_to_bikes_even_with_tracks_default() {
        let root = place_tmp("livery-tracks-default");
        let ex = root.join("ex");
        touch(&ex.join("MX1OEM_2023_KTM_450_SX-F/paints/cool.pnt"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "tracks", "", "slug").unwrap();
        assert!(mods
            .join("bikes/MX1OEM_2023_KTM_450_SX-F/paints/cool.pnt")
            .exists());
        assert!(!mods.join("tracks/MX1OEM_2023_KTM_450_SX-F").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    /// The type folder is not always ours to trust — a file-share code picks it from a path
    /// the sender wrote. `mods_dir.join("..")` is the MX Bikes folder itself, next to the
    /// executable, so the placement has to be refused rather than merely misrouted.
    #[test]
    fn refuses_a_destination_outside_the_mods_folder() {
        let root = place_tmp("escape-place");
        let ex = root.join("ex");
        touch(&ex.join("evil.dll"));
        let mods = root.join("game/mods");
        std::fs::create_dir_all(&mods).unwrap();

        for (type_folder, dest_folder) in [("..", ""), ("tracks", "../.."), ("../..", "x")] {
            let err = place_mod(&ex, &mods, type_folder, dest_folder, "slug").unwrap_err();
            assert!(err.to_string().contains("outside the mods folder"), "{err}");
        }
        assert!(!root.join("game/evil.dll").exists(), "a placement escaped the mods folder");
        assert!(!root.join("evil.dll").exists(), "a placement escaped the mods folder");
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn refuses_to_replace_a_folder_with_an_install_file() {
        let root = place_tmp("file-over-folder");
        let staged = root.join("UserBike.pkz");
        touch(&staged);
        let mods = root.join("mods");
        let existing = mods.join("bikes/UserBike.pkz");
        std::fs::create_dir_all(&existing).unwrap();

        let err = place_mod(&staged, &mods, "bikes", "", "UserBike").unwrap_err();
        assert!(err.to_string().contains("replace the folder"), "{err}");
        assert!(existing.is_dir());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn places_rider_kit_into_profile_paints() {
        let root = place_tmp("rider-kit");
        let ex = root.join("ex");
        touch(&ex.join("2026 ASTARS TECHSTAR UNITY.pnt")); // loose outfit paint
        let mods = root.join("mods");
        place_mod(&ex, &mods, "rider", "riders/default_mx/paints", "kit").unwrap();
        assert!(mods
            .join("rider/riders/default_mx/paints/2026 ASTARS TECHSTAR UNITY.pnt")
            .exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn rider_paint_bundle_not_routed_to_bikes() {
        let root = place_tmp("rider-bundle");
        let ex = root.join("ex");
        touch(&ex.join("default_mx/paints/kit.pnt")); // <profile>/paints bundle
        let mods = root.join("mods");
        place_mod(&ex, &mods, "rider", "riders/default_mx/paints", "kit").unwrap();
        assert!(mods.join("rider/riders/default_mx/paints/kit.pnt").exists());
        assert!(!mods.join("bikes/default_mx").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    /// A packaged helmet extracts to a single `<Model>/` folder, which `unwrap_wrapper`
    /// strips — and the destination offered for a new model is the bare area. Together
    /// those scattered the mesh and its `paints/` straight into `mods/rider/helmets`,
    /// where the game can't load it and the picker lists `paints` as a helmet.
    #[test]
    fn a_new_gear_model_keeps_a_folder_of_its_own() {
        let root = place_tmp("gear-model-folder");
        let ex = root.join("ex");
        touch(&ex.join("Astars_SM10_EKS/gfx.cfg"));
        touch(&ex.join("Astars_SM10_EKS/helmet.edf"));
        touch(&ex.join("Astars_SM10_EKS/paints/Red.pnt"));
        touch(&ex.join("Astars_SM10_EKS/goggles/Smoke.pnt"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "rider", "helmets", "astars-sm10").unwrap();

        let model = mods.join("rider/helmets/Astars_SM10_EKS");
        assert!(model.join("helmet.edf").exists(), "the mesh is under the model's own name");
        assert!(model.join("gfx.cfg").exists());
        assert!(model.join("paints/Red.pnt").exists(), "paints travel with the model");
        assert!(model.join("goggles/Smoke.pnt").exists());
        assert!(
            !mods.join("rider/helmets/helmet.edf").exists(),
            "nothing is left loose in the area folder"
        );
        assert!(
            !mods.join("rider/helmets/paints").exists(),
            "`paints` must not become a sibling of the models — the picker reads it as one"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
    /// Nothing to take the name from, so the slug is what's left. Still a folder: loose
    /// files in the area root are unloadable however they got there.
    #[test]
    fn a_bare_gear_model_is_named_from_the_slug() {
        let root = place_tmp("gear-model-slug");
        let ex = root.join("ex");
        touch(&ex.join("gfx.cfg"));
        touch(&ex.join("boots.edf"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "rider", "boots", "tech-10").unwrap();
        assert!(mods.join("rider/boots/tech-10/boots.edf").exists());
        assert!(!mods.join("rider/boots/boots.edf").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    /// Packed area-first: the archive's `helmets/` already *is* the destination, so its
    /// children are the models. Wrapping that would bury every one of them a level down.
    #[test]
    fn an_area_first_archive_is_not_wrapped_again() {
        let root = place_tmp("gear-area-first");
        let ex = root.join("ex");
        touch(&ex.join("helmets/Fox V3/gfx.cfg"));
        touch(&ex.join("helmets/Fox V3/helmet.edf"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "rider", "helmets", "fox-v3").unwrap();
        assert!(mods.join("rider/helmets/Fox V3/helmet.edf").exists());
        assert!(!mods.join("rider/helmets/helmets").exists(), "no doubled area folder");
        assert!(!mods.join("rider/helmets/fox-v3").exists(), "no slug folder over the models");
        let _ = std::fs::remove_dir_all(&root);
    }
    /// The shop's shape: the download *is* the model, so `extract_archive` carries the `.pkz`
    /// through and there is no folder to take a name from. Wrapping it named the folder for
    /// the slug and put the package a level below where anything looks — `helmets/shop-44/`,
    /// which the game can't load and the rider viewer answers with "no gear mesh found".
    #[test]
    fn a_packaged_gear_model_goes_straight_into_its_area() {
        let root = place_tmp("gear-packaged");
        let ex = root.join("ex");
        touch(&ex.join("B Helmet 100 Goggles.pkz"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "rider", "helmets", "shop-44").unwrap();
        assert!(
            mods.join("rider/helmets/B Helmet 100 Goggles.pkz").exists(),
            "the package sits in the area under its own name"
        );
        assert!(!mods.join("rider/helmets/shop-44").exists(), "no slug folder around it");
        let _ = std::fs::remove_dir_all(&root);
    }
    /// Same for a packaged rider profile, which `riders/` takes as a `.pkz` too.
    #[test]
    fn a_packaged_rider_profile_goes_straight_into_riders() {
        let root = place_tmp("rider-packaged");
        let ex = root.join("ex");
        touch(&ex.join("Suit 1.pkz"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "rider", "riders", "shop-91").unwrap();
        assert!(mods.join("rider/riders/Suit 1.pkz").exists());
        assert!(!mods.join("rider/riders/shop-91").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn helmet_paint_bundle_stays_in_rider() {
        let root = place_tmp("helmet-paint");
        let ex = root.join("ex");
        touch(&ex.join("Fox V3/paints/red.pnt"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "rider", "helmets/Fox V3/paints", "paint").unwrap();
        assert!(mods.join("rider/helmets/Fox V3/paints/red.pnt").exists());
        assert!(!mods.join("bikes/Fox V3").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn merges_full_mods_tree() {
        let root = place_tmp("modstree");
        let ex = root.join("ex");
        touch(&ex.join("mods/bikes/KTM.pkz"));
        touch(&ex.join("mods/tracks/T.pkz"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "tracks", "", "slug").unwrap();
        assert!(mods.join("bikes/KTM.pkz").exists());
        assert!(mods.join("tracks/T.pkz").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    /// A lone file is a placement in its own right — what the dropzone hands over when it
    /// splits a pack into the mods inside it and one of those is a single `.pkz` among
    /// fifty siblings. Its neighbours must not come with it.
    #[test]
    fn places_a_single_file_without_its_siblings() {
        let root = place_tmp("onefile");
        let ex = root.join("ex");
        touch(&ex.join("MX1OEM_KTM.pkz"));
        touch(&ex.join("MX2OEM_KTM.pkz"));
        let mods = root.join("mods");

        let wrote = place_mod(&ex.join("MX1OEM_KTM.pkz"), &mods, "bikes", "", "slug").unwrap();

        assert_eq!(wrote, 1);
        assert!(mods.join("bikes/MX1OEM_KTM.pkz").exists());
        assert!(
            !mods.join("bikes/MX2OEM_KTM.pkz").exists(),
            "the sibling was not asked for"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn merges_top_level_category_folders() {
        let root = place_tmp("cats");
        let ex = root.join("ex");
        touch(&ex.join("bikes/Y.pkz"));
        touch(&ex.join("tracks/X.pkz"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "tracks", "", "slug").unwrap();
        assert!(mods.join("bikes/Y.pkz").exists());
        assert!(mods.join("tracks/X.pkz").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn wraps_loose_extracted_track_files() {
        let root = place_tmp("loose");
        let ex = root.join("ex");
        touch(&ex.join("round3.cfg"));
        touch(&ex.join("round3.map"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "tracks", "", "MyTrack").unwrap();
        assert!(mods.join("tracks/MyTrack/round3.cfg").exists());
        assert!(mods.join("tracks/MyTrack/round3.map").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn plain_pkz_honors_chosen_dest_folder() {
        let root = place_tmp("dest");
        let ex = root.join("ex");
        touch(&ex.join("track.pkz"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "tracks", "Supercross/Round 1", "slug").unwrap();
        assert!(mods.join("tracks/Supercross/Round 1/track.pkz").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn standard_rider_kit_tree_uses_the_selected_profile_paints_folder() {
        for (case, prefix) in [("category", "rider"), ("mods-tree", "mods/rider")] {
            let root = place_tmp(&format!("standard-rider-kit-{case}"));
            let ex = root.join("ex");
            touch(&ex.join(prefix).join("default_mx/paints/Factory.pnt"));
            let mods = root.join("mods");

            place_mod(
                &ex,
                &mods,
                "rider",
                "riders/default_sm/paints",
                "factory-kit",
            )
            .unwrap();

            assert!(mods
                .join("rider/riders/default_sm/paints/Factory.pnt")
                .exists());
            assert!(!mods.join("rider/default_mx/paints/Factory.pnt").exists());
            let _ = std::fs::remove_dir_all(&root);
        }
    }
    #[test]
    fn rider_kit_rule_does_not_reclassify_a_gear_install() {
        let root = place_tmp("rider-kit-gear");
        let ex = root.join("ex");
        touch(&ex.join("rider/Fox/paints/Factory.pnt"));
        let mods = root.join("mods");

        place_mod(
            &ex,
            &mods,
            "rider",
            "helmets/Fox/paints",
            "factory-helmet",
        )
        .unwrap();

        assert!(mods.join("rider/Fox/paints/Factory.pnt").exists());
        assert!(!mods
            .join("rider/riders/Fox/paints/Factory.pnt")
            .exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn loose_livery_goes_into_chosen_bike_paints() {
        let root = place_tmp("loose-livery");
        let ex = root.join("ex");
        touch(&ex.join("cool.pnt")); // loose paint, no bike folder
        let mods = root.join("mods");
        place_mod(
            &ex,
            &mods,
            "bikes",
            "MX1OEM_2023_KTM_450_SX-F/paints",
            "cool-livery",
        )
        .unwrap();
        assert!(mods
            .join("bikes/MX1OEM_2023_KTM_450_SX-F/paints/cool.pnt")
            .exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn loose_livery_chosen_against_a_bike_root_lands_in_paints() {
        // The picker offers `<Bike>` as well as `<Bike>/paints` (sounds and model swaps need
        // the root), and `resolveInitialFolder` will preselect the root when that's what was
        // remembered. A `.pnt` left there is invisible to the game.
        let root = place_tmp("livery-bike-root");
        let ex = root.join("ex");
        touch(&ex.join("cool.pnt"));
        touch(&ex.join("preview.jpg")); // paints ship a thumbnail; it travels along
        let mods = root.join("mods");
        place_mod(&ex, &mods, "bikes", "MX1OEM_2023_KTM_450_SX-F", "cool-livery").unwrap();
        assert!(mods
            .join("bikes/MX1OEM_2023_KTM_450_SX-F/paints/cool.pnt")
            .exists());
        assert!(!mods
            .join("bikes/MX1OEM_2023_KTM_450_SX-F/cool.pnt")
            .exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn bike_package_chosen_against_a_bike_root_stays_at_the_root() {
        // A `.pkz` is a bike or model set — the root is exactly where it belongs.
        let root = place_tmp("pkz-bike-root");
        let ex = root.join("ex");
        touch(&ex.join("Addon Models.pkz"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "bikes", "MX1OEM_2023_KTM_450_SX-F", "swap").unwrap();
        assert!(mods
            .join("bikes/MX1OEM_2023_KTM_450_SX-F/Addon Models.pkz")
            .exists());
        assert!(!mods.join("bikes/MX1OEM_2023_KTM_450_SX-F/paints").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn paints_destination_is_not_doubled_up() {
        let root = place_tmp("livery-already-paints");
        let ex = root.join("ex");
        touch(&ex.join("cool.pnt"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "bikes", "MX1OEM_2023_KTM_450_SX-F/paints", "slug").unwrap();
        assert!(mods
            .join("bikes/MX1OEM_2023_KTM_450_SX-F/paints/cool.pnt")
            .exists());
        assert!(!mods
            .join("bikes/MX1OEM_2023_KTM_450_SX-F/paints/paints")
            .exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    /// A paint pack that is a `paints/` folder with a preview image beside it. The picker offers
    /// `<Bike>/paints`, so the folder was copied *into* it: `<Bike>/paints/paints/x.pnt`, which
    /// the game never lists.
    #[test]
    fn a_paints_folder_in_the_pack_is_not_nested_under_the_paints_destination() {
        for dest in ["MX1OEM_2023_KTM_450_SX-F/paints", "MX1OEM_2023_KTM_450_SX-F"] {
            let root = place_tmp("paints-folder-pack");
            let ex = root.join("ex");
            touch(&ex.join("paints/cool.pnt"));
            touch(&ex.join("preview.jpg"));
            let mods = root.join("mods");
            place_mod(&ex, &mods, "bikes", dest, "slug").unwrap();
            assert!(
                mods.join("bikes/MX1OEM_2023_KTM_450_SX-F/paints/cool.pnt").exists(),
                "dest {dest}"
            );
            assert!(
                !mods.join("bikes/MX1OEM_2023_KTM_450_SX-F/paints/paints").exists(),
                "dest {dest}"
            );
            let _ = std::fs::remove_dir_all(&root);
        }
    }
    #[test]
    fn loose_paint_at_the_bikes_root_is_left_alone() {
        // No bike was chosen, so there's no `paints` folder to redirect into — inventing
        // `bikes/paints` would be just as dead and harder to find.
        let root = place_tmp("livery-no-bike");
        let ex = root.join("ex");
        touch(&ex.join("cool.pnt"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "bikes", "", "slug").unwrap();
        assert!(mods.join("bikes/cool.pnt").exists());
        assert!(!mods.join("bikes/paints").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    /// An install that fails must say which file it died on. "os error 2" on its own is
    /// unactionable in a bug report — it names neither the path nor which end it was.
    #[test]
    fn a_failed_copy_names_both_paths() {
        let root = place_tmp("copy-err");
        let ex = root.join("ex");
        touch(&ex.join("cool.pnt"));
        // A directory sitting where the file has to land: `copy` cannot overwrite it.
        let mods = root.join("mods");
        std::fs::create_dir_all(mods.join("bikes/cool.pnt")).unwrap();

        // Refused before any copy is tried now (`guard_placement`), and the refusal still
        // names the path it would not overwrite.
        let err = place_mod(&ex, &mods, "bikes", "", "slug").unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("replace the folder"), "{msg}");
        assert!(msg.contains("cool.pnt"), "{msg}");
        assert!(mods.join("bikes/cool.pnt").is_dir(), "the folder is left alone");
        assert!(msg.contains(&mods.display().to_string()), "{msg}");
        let _ = std::fs::remove_dir_all(&root);
    }
    /// A staged file that disappears mid-install is not a broken mod, and the raw error
    /// says the opposite: "cannot find the file specified" reads as if the download had
    /// failed. Name the file and the folder to exclude instead.
    #[test]
    fn a_vanished_staged_file_blames_the_right_thing() {
        let root = place_tmp("vanished");
        let staging = root.join("ex");
        std::fs::create_dir_all(&staging).unwrap();
        let mods = root.join("mods");
        std::fs::create_dir_all(&mods).unwrap();

        // The state `apply` finds itself in once a scanner has taken the file away: the
        // walk listed it, the copy can no longer see it.
        let err = copy_staged(&staging.join("Scottsdale.pkz"), &mods.join("Scottsdale.pkz"))
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("Scottsdale.pkz"), "{msg}");
        assert!(msg.contains(&staging.display().to_string()), "{msg}");
        assert!(msg.to_lowercase().contains("antivirus"), "{msg}");
        let _ = std::fs::remove_dir_all(&root);
    }
    /// The other half of the same trap, and the one that got past us: the staged file is
    /// still listed, only its contents are out of reach. `Path::exists` answers yes — it
    /// opens for no access at all — so gating the advice on it handed the player a bare
    /// "os error 2" for a scanner hold. Unreadable has to count as gone.
    #[cfg(unix)]
    #[test]
    fn an_unreadable_staged_file_blames_the_right_thing() {
        use std::os::unix::fs::PermissionsExt;
        let root = place_tmp("unreadable");
        let staging = root.join("ex");
        let mods = root.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        let src = staging.join("Scottsdale.pkz");
        touch(&src);
        std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o000)).unwrap();
        if File::open(&src).is_ok() {
            // Running as root, where the mode is advisory and there is nothing to stand in
            // for the scanner.
            let _ = std::fs::remove_dir_all(&root);
            return;
        }
        assert!(src.exists(), "the entry is still there — that is the whole trap");

        let err = copy_staged(&src, &mods.join("Scottsdale.pkz")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("Scottsdale.pkz"), "{msg}");
        assert!(msg.contains(&staging.display().to_string()), "{msg}");
        assert!(msg.to_lowercase().contains("antivirus"), "{msg}");

        std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o644)).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }
    /// A scanner holding a just-unpacked file lets go within a moment. Waiting it out is
    /// the difference between an install that works and one that fails on a file the user
    /// can plainly see on disk.
    #[test]
    fn a_copy_waits_out_whatever_is_holding_the_file() {
        let root = place_tmp("copy-retry");
        let staging = root.join("ex");
        std::fs::create_dir_all(&staging).unwrap();
        let mods = root.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        let src = staging.join("Scottsdale.pkz");
        let dst = mods.join("Scottsdale.pkz");

        // Stands in for the scanner releasing the file: unreadable at the first attempt,
        // there well inside the retry window.
        //
        // Written aside and moved in, not `fs::write`. `fs::write` creates the file and then
        // fills it, so a retry landing in that window sees it at nought bytes or mid-write —
        // the file exists but isn't the file yet. This raced on Windows CI often enough to
        // fail runs that had nothing to do with installing, sometimes copying an empty file
        // and sometimes failing the read outright. A rename is atomic, so the retry either
        // doesn't see it or sees all of it, which is what the test is actually about.
        let late = src.clone();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            let tmp = late.with_extension("pkz.part");
            std::fs::write(&tmp, b"track").unwrap();
            std::fs::rename(&tmp, &late).unwrap();
        });

        copy_staged(&src, &dst).expect("the copy should have waited");
        writer.join().unwrap();
        assert_eq!(std::fs::read(&dst).unwrap(), b"track");
        let _ = std::fs::remove_dir_all(&root);
    }
    /// Waiting is for holders that let go. A folder sitting where the file has to land
    /// never will, so that failure must not sit through the whole retry window.
    #[test]
    fn a_hopeless_copy_gives_up_immediately() {
        let root = place_tmp("copy-hopeless");
        let staging = root.join("ex");
        touch(&staging.join("cool.pnt"));
        let dst = root.join("mods/cool.pnt");
        std::fs::create_dir_all(&dst).unwrap();

        // The decision itself, which is what this test is about: a folder in the way is
        // never worth another attempt, whatever the error says.
        let err = std::fs::copy(staging.join("cool.pnt"), &dst).unwrap_err();
        assert!(!worth_retrying(&dst, &err), "a directory in the way would be retried");

        // And end to end, measured against the retry window rather than a single wait. One
        // copy attempt on a busy Windows CI runner (the whole suite in parallel) took longer
        // than the 150 ms `COPY_RETRY_WAIT` and failed this test on a PR that touched no
        // Rust; a retried copy sits through ~1.8 s of waits, so this still catches one.
        let start = std::time::Instant::now();
        assert!(copy_staged(&staging.join("cool.pnt"), &dst).is_err());
        assert!(
            start.elapsed() < COPY_RETRY_WAIT * (COPY_ATTEMPTS - 1),
            "a directory in the way was retried"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
    /// A link whose target is gone is one entry, not a reason to fail the whole install.
    /// `fs::copy` follows it into nothing and reports the same "os error 2" a missing
    /// source does, which used to take the other files down with it.
    #[cfg(unix)]
    #[test]
    fn a_dangling_link_is_skipped_rather_than_fatal() {
        let root = place_tmp("dangling");
        let ex = root.join("ex");
        touch(&ex.join("real.pnt"));
        std::os::unix::fs::symlink(ex.join("gone.pnt"), ex.join("broken.pnt")).unwrap();

        let mods = root.join("mods");
        let n = place_mod(&ex, &mods, "bikes", "KTM/paints", "slug").unwrap();
        assert_eq!(n, 1, "only the real file is installed");
        assert!(mods.join("bikes/KTM/paints/real.pnt").exists());
        assert!(!mods.join("bikes/KTM/paints/broken.pnt").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn unwraps_single_wrapper_folder() {
        let root = place_tmp("wrap");
        let ex = root.join("ex");
        touch(&ex.join("Downloaded Mod/track.pkz"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "tracks", "", "slug").unwrap();
        assert!(mods.join("tracks/track.pkz").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn packaged_sound_mod_merges_to_bike_root() {
        let root = place_tmp("sound-packaged");
        let ex = root.join("ex");
        let bike = "ASS KTM250-0.1/mods/bikes/MX2OEM_2023_KTM_250_SX-F";
        touch(&ex.join(format!("{bike}/engine.scl")));
        touch(&ex.join(format!("{bike}/sfx.cfg")));
        touch(&ex.join("ASS KTM250-0.1/mods/bikes/sounds/idle.wav"));
        let mods = root.join("mods");
        let installed = mods.join("bikes/MX2OEM_2023_KTM_250_SX-F");
        touch(&installed.join("MX2OEM_2023_KTM_250_SX-F.ini"));
        touch(&installed.join("MX2OEM_2023_KTM_250_SX-F.cfg"));
        // Picker may pass `<Bike>/paints`; a self-structured archive ignores it.
        place_mod(&ex, &mods, "bikes", "MX2OEM_2023_KTM_250_SX-F/paints", "slug").unwrap();
        assert!(mods
            .join("bikes/MX2OEM_2023_KTM_250_SX-F/engine.scl")
            .exists());
        assert!(mods.join("bikes/MX2OEM_2023_KTM_250_SX-F/sfx.cfg").exists());
        assert!(mods.join("bikes/sounds/idle.wav").exists());
        // Never inside a paints folder.
        assert!(!mods.join("bikes/MX2OEM_2023_KTM_250_SX-F/paints").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn nested_sound_bundle_routes_to_bikes() {
        let root = place_tmp("sound-nested");
        let ex = root.join("ex");
        touch(&ex.join("MX2OEM_2023_KTM_250_SX-F/engine.scl"));
        touch(&ex.join("MX2OEM_2023_KTM_250_SX-F/sfx.cfg"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "bikes", "", "slug").unwrap();
        assert!(mods
            .join("bikes/MX2OEM_2023_KTM_250_SX-F/engine.scl")
            .exists());
        assert!(mods.join("bikes/MX2OEM_2023_KTM_250_SX-F/sfx.cfg").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn loose_sound_files_never_land_in_paints() {
        let root = place_tmp("sound-loose");
        let ex = root.join("ex");
        touch(&ex.join("engine.scl"));
        touch(&ex.join("sfx.cfg"));
        let mods = root.join("mods");
        place_mod(&ex, &mods, "bikes", "MX2OEM_2023_KTM_250_SX-F/paints", "slug").unwrap();
        assert!(mods
            .join("bikes/MX2OEM_2023_KTM_250_SX-F/engine.scl")
            .exists());
        assert!(!mods
            .join("bikes/MX2OEM_2023_KTM_250_SX-F/paints/engine.scl")
            .exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
