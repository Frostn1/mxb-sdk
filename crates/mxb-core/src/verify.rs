//! Structural checks for installed mods.

use crate::mods::{Kind, Mod, Packaged};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub code: String,
    pub message: String,
    pub path: Option<PathBuf>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub problems: Vec<Problem>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Duplicate {
    pub name: String,
    pub paths: Vec<PathBuf>,
}

pub fn verify(item: &Mod) -> Report {
    let mut problems = Vec::new();
    match item.packaged {
        Packaged::Protected => problems.push(problem(
            "protected",
            "Package is protected and cannot be inspected.",
            Some(item.path.clone()),
        )),
        Packaged::Pkz => {
            if let Err(e) = mxb_content::entry_names(&item.path) {
                problems.push(problem(
                    "badPackage",
                    &format!("Package is not a readable zip: {e}"),
                    Some(item.path.clone()),
                ))
            }
        }
        Packaged::Plain => {}
    }
    if item.path.is_dir() {
        let id = crate::identify::identify(&item.path).ok();
        if id
            .as_ref()
            .and_then(|x| x.kind)
            .is_some_and(|k| !compatible(k, item.kind))
        {
            problems.push(problem(
                "structure",
                "The folder structure does not match its installed category.",
                Some(item.path.clone()),
            ));
        }
        for e in walkdir::WalkDir::new(&item.path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| x.eq_ignore_ascii_case("edf"))
            })
        {
            match std::fs::read(e.path()) {
                Ok(b) if b.len() >= 92 && b.starts_with(b"EDF\0") => {}
                _ => problems.push(problem(
                    "badEdf",
                    "Model has an invalid EDF header.",
                    Some(e.path().into()),
                )),
            }
        }
    }
    Report { problems }
}
fn compatible(a: Kind, b: Kind) -> bool {
    a == b
        || matches!(
            (a, b),
            (Kind::Paints, Kind::Rider) | (Kind::Sounds, Kind::Bikes)
        )
}
fn problem(code: &str, message: &str, path: Option<PathBuf>) -> Problem {
    Problem {
        code: code.into(),
        message: message.into(),
        path,
    }
}
pub fn duplicates(mods_dir: impl AsRef<Path>) -> Vec<Duplicate> {
    let mut by: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for m in crate::mods::list(mods_dir, None) {
        let paths = by.entry(m.name.to_ascii_lowercase()).or_default();
        if !paths.contains(&m.path) {
            paths.push(m.path);
        }
    }
    by.into_iter()
        .filter(|(_, p)| p.len() > 1)
        .map(|(name, paths)| Duplicate { name, paths })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_bad_protected_and_duplicates() {
        let t = tempfile::tempdir().unwrap();
        for d in ["bikes/Same", "tracks/Same"] {
            std::fs::create_dir_all(t.path().join(d)).unwrap();
        }
        assert_eq!(duplicates(t.path()).len(), 1);
        let protected = Mod {
            kind: Kind::Tracks,
            name: "x".into(),
            path: t.path().join("x.pkz"),
            size_bytes: 1,
            files: 1,
            packaged: Packaged::Protected,
        };
        assert_eq!(verify(&protected).problems[0].code, "protected");
        let bad = t.path().join("bad.pkz");
        std::fs::write(&bad, b"PK\x03\x04bad").unwrap();
        let broken = Mod {
            path: bad,
            packaged: Packaged::Pkz,
            ..protected
        };
        assert_eq!(verify(&broken).problems[0].code, "badPackage");
    }
}
