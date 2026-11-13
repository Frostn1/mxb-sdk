//! Read and write the app's plain preset store.

use crate::Result;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub name: String,
    #[serde(default)]
    pub data: Value,
}
fn path(dir: &Path) -> std::path::PathBuf {
    dir.join("presets.json")
}
pub fn list(dir: impl AsRef<Path>) -> Vec<Preset> {
    load_typed(dir.as_ref())
}
pub fn read(dir: impl AsRef<Path>, name: &str) -> Option<Preset> {
    list(dir)
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
}
pub fn write(dir: impl AsRef<Path>, preset: Preset) -> Result<()> {
    let dir = dir.as_ref();
    let mut all = list(dir);
    all.retain(|p| !p.name.eq_ignore_ascii_case(&preset.name));
    all.push(preset);
    all.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    store_typed(dir, &all)
}

#[doc(hidden)]
pub fn load_typed<T: DeserializeOwned>(dir: &Path) -> Vec<T> {
    std::fs::read_to_string(path(dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

#[doc(hidden)]
pub fn store_typed<T: Serialize>(dir: &Path, values: &[T]) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(path(dir), serde_json::to_vec_pretty(values)?)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip() {
        let t = tempfile::tempdir().unwrap();
        write(
            t.path(),
            Preset {
                name: "Race".into(),
                data: serde_json::json!({"bike":"A"}),
            },
        )
        .unwrap();
        assert_eq!(read(t.path(), "race").unwrap().name, "Race");
    }
}
