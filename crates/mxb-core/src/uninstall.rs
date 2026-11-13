//! Safely move known mods to the operating system trash.

use crate::{Error, Result};
use std::path::{Path, PathBuf};

pub trait Trash {
    fn remove(&self, path: &Path) -> Result<()>;
}
pub struct SystemTrash;
impl Trash for SystemTrash {
    fn remove(&self, path: &Path) -> Result<()> {
        trash::delete(path).map_err(|e| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })
    }
}

pub fn uninstall(mods_dir: impl AsRef<Path>, path: impl AsRef<Path>) -> Result<PathBuf> {
    uninstall_with(mods_dir.as_ref(), path.as_ref(), &SystemTrash)
}

pub fn uninstall_with(mods_dir: &Path, path: &Path, trash: &dyn Trash) -> Result<PathBuf> {
    let root = mods_dir.canonicalize()?;
    let target = path.canonicalize()?;
    let rel = target.strip_prefix(&root).map_err(|_| {
        Error::UnsafePath("refusing to remove a path outside the mods folder".into())
    })?;
    let first = rel
        .components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .unwrap_or("");
    if !["bikes", "tracks", "rider", "tyres", "misc"]
        .iter()
        .any(|x| x.eq_ignore_ascii_case(first))
    {
        return Err(Error::UnsafePath(
            "that path is not in a known mod folder".into(),
        ));
    }
    let known = crate::mods::list(&root, None)
        .into_iter()
        .any(|m| m.path.canonicalize().is_ok_and(|p| p == target));
    if !known {
        return Err(Error::UnsafePath(
            "refusing to remove a file that is not a listed mod".into(),
        ));
    }
    trash.remove(&target)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    struct Fake(Mutex<Vec<PathBuf>>);
    impl Trash for Fake {
        fn remove(&self, p: &Path) -> Result<()> {
            self.0.lock().unwrap().push(p.into());
            Ok(())
        }
    }
    #[test]
    fn refuses_outside_and_unknown() {
        let t = tempfile::tempdir().unwrap();
        let mods = t.path().join("mods");
        std::fs::create_dir_all(mods.join("bikes/Known")).unwrap();
        std::fs::write(mods.join("bikes/loose.txt"), b"x").unwrap();
        let f = Fake(Mutex::new(vec![]));
        assert!(uninstall_with(&mods, t.path(), &f).is_err());
        assert!(uninstall_with(&mods, &mods.join("bikes/loose.txt"), &f).is_err());
        assert!(uninstall_with(&mods, &mods.join("bikes/Known"), &f).is_ok());
    }
}
