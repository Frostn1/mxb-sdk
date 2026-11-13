//! Identify mod content from a folder or an ordinary archive.

use crate::mods::Kind;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::fs::File;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveFormat {
    Zip,
    SevenZip,
    Rar,
    Pkz,
    Paint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Layout {
    Folder,
    ModsTree,
    Categories,
    Wrapped,
    Package,
    LooseFiles,
    NeedsExtraction { format: ArchiveFormat },
    Protected,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identified {
    pub kind: Option<Kind>,
    pub layout: Layout,
    pub suggested_name: String,
}

pub fn identify(path: impl AsRef<Path>) -> Result<Identified> {
    let path = path.as_ref();
    let suggested_name = path
        .file_stem()
        .or_else(|| path.file_name())
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    if path.is_dir() {
        return Ok(identify_names(folder_names(path), suggested_name, true));
    }
    let format = detect_ext(path)?;
    if matches!(format, ArchiveFormat::SevenZip | ArchiveFormat::Rar) {
        return Ok(Identified {
            kind: None,
            layout: Layout::NeedsExtraction { format },
            suggested_name,
        });
    }
    if format == ArchiveFormat::Paint {
        return Ok(Identified {
            kind: Some(Kind::Paints),
            layout: Layout::LooseFiles,
            suggested_name,
        });
    }
    if format == ArchiveFormat::Pkz && !mxb_content::is_plain_zip(path) {
        return Ok(Identified {
            kind: None,
            layout: Layout::Protected,
            suggested_name,
        });
    }
    let names = mxb_content::entry_names(path).map_err(|e| Error::Invalid(e.to_string()))?;
    Ok(identify_names(names, suggested_name, false))
}

pub fn detect_ext(path: &Path) -> Result<ArchiveFormat> {
    let ext = crate::placement::detect_ext(path)
        .map_err(|error| Error::Invalid(format!("{error:#}")))?;
    match ext.as_str() {
        "zip" => Ok(ArchiveFormat::Zip),
        "7z" => Ok(ArchiveFormat::SevenZip),
        "rar" => Ok(ArchiveFormat::Rar),
        "pkz" => Ok(ArchiveFormat::Pkz),
        "pnt" => Ok(ArchiveFormat::Paint),
        _ => unreachable!("placement::detect_ext only returns supported extensions"),
    }
}

fn folder_names(root: &Path) -> Vec<String> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            e.path()
                .strip_prefix(root)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn identify_names(names: Vec<String>, suggested_name: String, folder: bool) -> Identified {
    let lower: Vec<String> = names
        .iter()
        .map(|n| n.trim_start_matches("./").to_ascii_lowercase())
        .collect();
    let components = |needle: &str| lower.iter().any(|n| n.split('/').any(|x| x == needle));
    let has_ext = |ext: &str| {
        lower
            .iter()
            .any(|n| n.rsplit('.').next().is_some_and(|x| x == ext))
    };
    let kind = if components("tracks")
        || ["map", "trh", "tsc", "rdf", "ssc"]
            .iter()
            .filter(|x| has_ext(x))
            .count()
            >= 2
    {
        Some(Kind::Tracks)
    } else if components("tyres")
        || (lower.iter().any(|n| n.ends_with("fwheel.hrc"))
            && lower.iter().any(|n| n.ends_with("rwheel.hrc")))
    {
        Some(Kind::Tyres)
    } else if components("rider")
        || ["helmets", "boots", "protections", "riders"]
            .iter()
            .any(|x| components(x))
    {
        Some(Kind::Rider)
    } else if has_ext("pnt") {
        Some(Kind::Paints)
    } else if lower.iter().any(|n| n.ends_with("sfx.cfg")) && has_ext("scl") {
        Some(Kind::Sounds)
    } else if components("bikes") || has_ext("ini") && has_ext("cfg") {
        Some(Kind::Bikes)
    } else if has_ext("dlo") || has_ext("dll") {
        Some(Kind::Plugins)
    } else {
        None
    };
    let first: std::collections::BTreeSet<&str> =
        lower.iter().filter_map(|n| n.split('/').next()).collect();
    let layout = if first.contains("mods") {
        Layout::ModsTree
    } else if ["bikes", "tracks", "rider", "tyres", "misc"]
        .iter()
        .any(|x| first.contains(x))
    {
        Layout::Categories
    } else if !folder {
        Layout::Package
    } else if first.len() == 1 && lower.iter().any(|n| n.contains('/')) {
        Layout::Wrapped
    } else if kind.is_some() {
        Layout::Folder
    } else {
        Layout::Unknown
    };
    Identified {
        kind,
        layout,
        suggested_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn identifies_folder_and_zip() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join("track.map"), b"x").unwrap();
        std::fs::write(t.path().join("track.trh"), b"x").unwrap();
        assert_eq!(identify(t.path()).unwrap().kind, Some(Kind::Tracks));
        let p = t.path().join("bike.zip");
        let f = File::create(&p).unwrap();
        let mut z = zip::ZipWriter::new(f);
        z.start_file("bikes/A/A.ini", zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(b"x").unwrap();
        z.finish().unwrap();
        assert_eq!(identify(&p).unwrap().kind, Some(Kind::Bikes));
    }
}
