//! Plan and apply filesystem-only mod installs.

use crate::mods::Kind;
use crate::uninstall::{SystemTrash, Trash};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub use crate::placement::{OnConflict, Placement, Route, RouteRule, Staging};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Write {
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub kind: Kind,
    pub rule: RouteRule,
    pub source_dir: PathBuf,
    pub mods_dir: PathBuf,
    pub files: Vec<Write>,
    #[serde(skip)]
    placement: Placement,
}

impl Plan {
    pub fn writes(&self) -> Vec<Write> {
        crate::placement::writes_for(&self.placement)
            .into_iter()
            .map(|(source, destination)| Write {
                source,
                destination,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub written: Vec<PathBuf>,
    pub replaced: Vec<PathBuf>,
}

pub fn plan(
    source_dir: impl AsRef<Path>,
    mods_dir: impl AsRef<Path>,
    kind_hint: Option<Kind>,
) -> Result<Plan> {
    let source_dir = source_dir.as_ref();
    let mods_dir = mods_dir.as_ref();
    if !source_dir.exists() {
        return Err(Error::Invalid(format!(
            "{} does not exist",
            source_dir.display()
        )));
    }
    let identified = crate::identify::identify(source_dir)?;
    let kind = kind_hint
        .or(identified.kind)
        .ok_or_else(|| Error::Invalid("could not identify this mod; provide a kind".into()))?;
    let slug = source_dir
        .file_stem()
        .or_else(|| source_dir.file_name())
        .unwrap_or_default()
        .to_string_lossy();
    let route = crate::placement::plan_placement(source_dir, mods_dir, kind.folder(), "", &slug);
    crate::placement::guard_placement(mods_dir, &route.placement).map_err(placement_error)?;
    let files = crate::placement::writes_for(&route.placement)
        .into_iter()
        .map(|(source, destination)| Write {
            source,
            destination,
        })
        .collect();
    Ok(Plan {
        kind,
        rule: route.rule,
        source_dir: source_dir.to_path_buf(),
        mods_dir: mods_dir.to_path_buf(),
        files,
        placement: route.placement,
    })
}

pub fn apply(plan: &Plan) -> Result<Installed> {
    apply_with(plan, &SystemTrash)
}

pub fn apply_with(plan: &Plan, trash: &dyn Trash) -> Result<Installed> {
    let writes = plan.writes();
    let mut written = Vec::new();
    let mut replaced = Vec::new();
    crate::placement::guard_placement(&plan.mods_dir, &plan.placement).map_err(placement_error)?;
    for root in crate::placement::roots_for(&plan.placement) {
        std::fs::create_dir_all(root)?;
    }
    for write in &writes {
        if write.destination.exists() {
            if write.destination.is_file()
                && std::fs::read(&write.source)? == std::fs::read(&write.destination)?
            {
                continue;
            }
            trash.remove(&write.destination)?;
            replaced.push(write.destination.clone());
        }
        if let Some(parent) = write.destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        crate::placement::copy_staged(&write.source, &write.destination)
            .map_err(placement_error)?;
        written.push(write.destination.clone());
    }
    Ok(Installed { written, replaced })
}

fn placement_error(error: anyhow::Error) -> Error {
    Error::Invalid(format!("{error:#}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeTrash(Mutex<Vec<PathBuf>>);
    impl Trash for FakeTrash {
        fn remove(&self, path: &Path) -> Result<()> {
            self.0.lock().unwrap().push(path.to_path_buf());
            Ok(())
        }
    }

    #[test]
    fn public_plan_and_apply_use_the_placement_engine() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("Track");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("a.map"), b"x").unwrap();
        std::fs::write(source.join("a.trh"), b"x").unwrap();
        let plan = plan(&source, temp.path().join("mods"), Some(Kind::Tracks)).unwrap();
        assert!(plan.writes()[0]
            .destination
            .starts_with(temp.path().join("mods/tracks")));
        assert_eq!(apply(&plan).unwrap().written.len(), 2);
    }

    #[test]
    fn apply_trashes_replacements_and_skips_identical_files() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("Track");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("a.map"), b"new").unwrap();
        std::fs::write(source.join("a.trh"), b"same").unwrap();
        let plan = plan(&source, temp.path().join("mods"), Some(Kind::Tracks)).unwrap();
        for write in plan.writes() {
            std::fs::create_dir_all(write.destination.parent().unwrap()).unwrap();
            let bytes: &[u8] = if write.source.extension().unwrap() == "map" {
                b"old"
            } else {
                b"same"
            };
            std::fs::write(write.destination, bytes).unwrap();
        }
        let trash = FakeTrash(Mutex::new(Vec::new()));

        let installed = apply_with(&plan, &trash).unwrap();

        assert_eq!(installed.written.len(), 1);
        assert_eq!(installed.replaced.len(), 1);
        assert_eq!(*trash.0.lock().unwrap(), installed.replaced);
        assert_eq!(std::fs::read(&installed.written[0]).unwrap(), b"new");
    }
}
