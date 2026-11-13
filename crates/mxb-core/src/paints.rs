//! List and safely place public paint files.

use crate::uninstall::{SystemTrash, Trash};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
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
    install_with(source.as_ref(), paints_dir.as_ref(), &SystemTrash)
}

pub fn install_with(source: &Path, paints_dir: &Path, trash: &dyn Trash) -> Result<Paint> {
    let s = source;
    let file_name = s
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::UnsafePath("paint must have a valid file name".into()))?;
    validate_file_name(file_name)?;
    if !s
        .extension()
        .and_then(|x| x.to_str())
        .is_some_and(|x| x.eq_ignore_ascii_case("pnt"))
    {
        return Err(Error::Invalid("paint must be a .pnt file".into()));
    }
    let d = paints_dir;
    std::fs::create_dir_all(d)?;
    let p = d.join(file_name);
    let bytes = std::fs::read(s)?;
    replace_if_needed(&p, &bytes, trash)?;
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
    write_with(paints_dir.as_ref(), name, bytes.as_ref(), &SystemTrash)
}

pub fn write_with(paints_dir: &Path, name: &str, bytes: &[u8], trash: &dyn Trash) -> Result<Paint> {
    let file_name = if name.to_ascii_lowercase().ends_with(".pnt") {
        name.to_string()
    } else {
        format!("{name}.pnt")
    };
    validate_file_name(&file_name)?;
    let dir = paints_dir;
    std::fs::create_dir_all(dir)?;
    let path = dir.join(file_name);
    replace_if_needed(&path, bytes, trash)?;
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

fn validate_file_name(file_name: &str) -> Result<()> {
    let mut components = Path::new(file_name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(Error::UnsafePath("paint name must be one file name".into()));
    }
    if file_name.len() > 200
        || file_name.contains([':', '/', '\\', '\0'])
        || file_name.chars().any(char::is_control)
        || file_name.ends_with(['.', ' '])
    {
        return Err(Error::UnsafePath(
            "paint name is not safe on Windows".into(),
        ));
    }
    let device = file_name.split('.').next().unwrap_or(file_name);
    let device_bytes = device.as_bytes();
    let reserved = matches!(
        device.to_ascii_uppercase().as_str(),
        "CON" | "PRN" | "AUX" | "NUL"
    ) || device_bytes.len() == 4
        && (device_bytes[..3].eq_ignore_ascii_case(b"COM")
            || device_bytes[..3].eq_ignore_ascii_case(b"LPT"))
        && matches!(device_bytes[3], b'1'..=b'9');
    if reserved {
        return Err(Error::UnsafePath(
            "paint name is a reserved Windows device".into(),
        ));
    }
    Ok(())
}

fn replace_if_needed(path: &Path, bytes: &[u8], trash: &dyn Trash) -> Result<()> {
    if path.exists() {
        if path.is_file() && std::fs::read(path)? == bytes {
            return Ok(());
        }
        trash.remove(path)?;
    }
    std::fs::write(path, bytes)?;
    Ok(())
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
    use std::sync::Mutex;

    struct FakeTrash(Mutex<Vec<PathBuf>>);
    impl Trash for FakeTrash {
        fn remove(&self, path: &Path) -> Result<()> {
            self.0.lock().unwrap().push(path.to_path_buf());
            Ok(())
        }
    }
    #[test]
    fn copies_and_lists() {
        let t = tempfile::tempdir().unwrap();
        let s = t.path().join("Red.pnt");
        std::fs::write(&s, b"PNT\0").unwrap();
        install(&s, t.path().join("mods/bikes/Bike/paints")).unwrap();
        assert_eq!(list(t.path().join("mods"), "Bike").len(), 1);
    }

    #[test]
    fn rejects_windows_unsafe_names() {
        let too_long = format!("{}.pnt", "a".repeat(197));
        for name in [
            "C:evil.pnt",
            "foo.pnt:stream",
            "CON.pnt",
            "nul.pnt",
            "COM1.anything",
            "lpt9",
            "bad.pnt.",
            "bad.pnt ",
            "../bad.pnt",
            "dir/bad.pnt",
            "dir\\bad.pnt",
            "bad\0.pnt",
            "bad\n.pnt",
            &too_long,
        ] {
            assert!(validate_file_name(name).is_err(), "accepted {name:?}");
        }
    }

    #[test]
    fn replacement_uses_trash_and_identical_bytes_are_skipped() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("paints");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Blue.pnt");
        std::fs::write(&path, b"old").unwrap();
        let trash = FakeTrash(Mutex::new(Vec::new()));

        write_with(&dir, "Blue", b"new", &trash).unwrap();
        write_with(&dir, "Blue", b"new", &trash).unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        assert_eq!(trash.0.lock().unwrap().as_slice(), [path]);
    }

    #[test]
    fn install_validates_and_replaces_by_source_file_name() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("Red.pnt");
        let dir = temp.path().join("paints");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&source, b"new").unwrap();
        std::fs::write(dir.join("Red.pnt"), b"old").unwrap();
        let trash = FakeTrash(Mutex::new(Vec::new()));

        install_with(&source, &dir, &trash).unwrap();

        assert_eq!(trash.0.lock().unwrap().as_slice(), [dir.join("Red.pnt")]);
    }
}
