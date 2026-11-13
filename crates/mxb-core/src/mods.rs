//! Inventory installed mods without interpreting protected content.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Bikes,
    Tracks,
    Rider,
    Paints,
    Tyres,
    Sounds,
    Plugins,
    Misc,
}

impl Kind {
    pub fn folder(self) -> &'static str {
        match self {
            Self::Bikes => "bikes",
            Self::Tracks => "tracks",
            Self::Rider | Self::Paints => "rider",
            Self::Tyres => "tyres",
            Self::Sounds => "bikes",
            Self::Plugins => "plugins",
            Self::Misc => "misc",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "bike" | "bikes" => Some(Self::Bikes),
            "track" | "tracks" => Some(Self::Tracks),
            "rider" | "gear" => Some(Self::Rider),
            "paint" | "paints" => Some(Self::Paints),
            "tyre" | "tyres" | "tire" | "tires" => Some(Self::Tyres),
            "sound" | "sounds" => Some(Self::Sounds),
            "plugin" | "plugins" => Some(Self::Plugins),
            "misc" => Some(Self::Misc),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Packaged {
    Plain,
    Pkz,
    Protected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mod {
    pub kind: Kind,
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub files: u64,
    pub packaged: Packaged,
}

pub fn list(mods_dir: impl AsRef<Path>, kind: Option<Kind>) -> Vec<Mod> {
    let mods_dir = mods_dir.as_ref();
    let kinds: Vec<Kind> = kind.map(|k| vec![k]).unwrap_or_else(|| {
        vec![
            Kind::Bikes,
            Kind::Tracks,
            Kind::Rider,
            Kind::Paints,
            Kind::Tyres,
            Kind::Sounds,
            Kind::Misc,
            Kind::Plugins,
        ]
    });
    let mut out = Vec::new();
    for k in kinds {
        let base = if k == Kind::Plugins {
            mods_dir.parent().unwrap_or(mods_dir).join("plugins")
        } else {
            mods_dir.join(k.folder())
        };
        if k == Kind::Paints {
            for entry in walkdir::WalkDir::new(mods_dir)
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
                push_path(&mut out, k, entry.path().to_path_buf());
            }
            continue;
        }
        if k == Kind::Sounds {
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries
                    .flatten()
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                {
                    let path = entry.path();
                    let mut scl = false;
                    let mut sfx = false;
                    if let Ok(files) = std::fs::read_dir(&path) {
                        for f in files.flatten() {
                            let p = f.path();
                            scl |= p
                                .extension()
                                .and_then(|x| x.to_str())
                                .is_some_and(|x| x.eq_ignore_ascii_case("scl"));
                            sfx |= f
                                .file_name()
                                .to_string_lossy()
                                .eq_ignore_ascii_case("sfx.cfg");
                        }
                    }
                    if scl && sfx {
                        push_path(&mut out, k, path);
                    }
                }
            }
            continue;
        }
        if k == Kind::Rider {
            if let Ok(areas) = std::fs::read_dir(&base) {
                for area in areas
                    .flatten()
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                {
                    if let Ok(items) = std::fs::read_dir(area.path()) {
                        for item in items.flatten() {
                            push_path(&mut out, k, item.path());
                        }
                    }
                }
            }
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.') || n.eq_ignore_ascii_case("desktop.ini"))
            {
                continue;
            }
            let is_pkz = path
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("pkz"));
            if path.is_file() && !is_pkz && k != Kind::Plugins {
                continue;
            }
            push_path(&mut out, k, path);
        }
    }
    out.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    out
}

fn push_path(out: &mut Vec<Mod>, kind: Kind, path: PathBuf) {
    let is_pkz = path
        .extension()
        .and_then(|x| x.to_str())
        .is_some_and(|x| x.eq_ignore_ascii_case("pkz"));
    let packaged = if is_pkz {
        if mxb_content::is_plain_zip(&path) {
            Packaged::Pkz
        } else {
            Packaged::Protected
        }
    } else {
        Packaged::Plain
    };
    let (size_bytes, files) = measure(&path);
    let name = path
        .file_stem()
        .or_else(|| path.file_name())
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    out.push(Mod {
        kind,
        name,
        path,
        size_bytes,
        files,
        packaged,
    });
}

fn measure(path: &Path) -> (u64, u64) {
    if path.is_file() {
        return (path.metadata().map(|m| m.len()).unwrap_or(0), 1);
    }
    walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .fold((0, 0), |(s, n), e| {
            (s + e.metadata().map(|m| m.len()).unwrap_or(0), n + 1)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lists_a_folder() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("bikes/One");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("One.ini"), b"x").unwrap();
        let got = list(t.path(), Some(Kind::Bikes));
        assert_eq!(got[0].name, "One");
        assert_eq!(got[0].files, 1);
    }
}
