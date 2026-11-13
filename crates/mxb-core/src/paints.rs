//! List and safely place public paint files.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paint {
    pub name: String,
    pub path: PathBuf,
    pub item: String,
}
pub fn list(mods_dir: impl AsRef<Path>, item: &str) -> Vec<Paint> {
    let mut out = Vec::new();
    let needle = item.to_ascii_lowercase();
    for e in walkdir::WalkDir::new(mods_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| x.eq_ignore_ascii_case("pnt"))
        })
    {
        let p = e.path();
        if p.ancestors().any(|a| {
            a.file_name()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case(&needle))
        }) {
            out.push(Paint {
                name: p
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                path: p.into(),
                item: item.into(),
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}
pub fn install(source: impl AsRef<Path>, paints_dir: impl AsRef<Path>) -> Result<Paint> {
    let s = source.as_ref();
    if !s
        .extension()
        .and_then(|x| x.to_str())
        .is_some_and(|x| x.eq_ignore_ascii_case("pnt"))
    {
        return Err(Error::Invalid("paint must be a .pnt file".into()));
    }
    let d = paints_dir.as_ref();
    std::fs::create_dir_all(d)?;
    let p = d.join(
        s.file_name()
            .ok_or_else(|| Error::Invalid("paint has no file name".into()))?,
    );
    std::fs::copy(s, &p)?;
    Ok(Paint {
        name: p
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        path: p,
        item: d
            .parent()
            .and_then(|x| x.file_name())
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    })
}
pub fn read(paint: &Paint) -> Result<Vec<u8>> {
    Ok(std::fs::read(&paint.path)?)
}
pub fn write(paints_dir: impl AsRef<Path>, name: &str, bytes: impl AsRef<[u8]>) -> Result<Paint> {
    let file_name = if name.to_ascii_lowercase().ends_with(".pnt") {
        name.to_string()
    } else {
        format!("{name}.pnt")
    };
    if file_name.contains(['/', '\\']) || matches!(file_name.as_str(), ".pnt" | "..pnt") {
        return Err(Error::UnsafePath("paint name must be one file name".into()));
    }
    let dir = paints_dir.as_ref();
    std::fs::create_dir_all(dir)?;
    let path = dir.join(file_name);
    std::fs::write(&path, bytes)?;
    Ok(Paint {
        name: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        path,
        item: dir
            .parent()
            .and_then(|x| x.file_name())
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    })
}
pub fn remove(paint: &Paint) -> Result<()> {
    trash::delete(&paint.path).map_err(|e| {
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        ))
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copies_and_lists() {
        let t = tempfile::tempdir().unwrap();
        let s = t.path().join("Red.pnt");
        std::fs::write(&s, b"PNT\0").unwrap();
        install(&s, t.path().join("mods/bikes/Bike/paints")).unwrap();
        assert_eq!(list(t.path().join("mods"), "Bike").len(), 1);
    }
}
