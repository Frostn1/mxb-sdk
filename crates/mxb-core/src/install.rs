//! Plan and apply filesystem-only mod installs.

use crate::mods::Kind;
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
    let writes = plan.writes();
    let count = crate::placement::apply(
        &plan.placement,
        OnConflict::Overwrite,
        Staging::Preserve,
    )
    .map_err(placement_error)?;
    let written = writes
        .into_iter()
        .take(count)
        .map(|write| write.destination)
        .collect();
    Ok(Installed { written })
}

fn placement_error(error: anyhow::Error) -> Error {
    Error::Invalid(format!("{error:#}"))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
