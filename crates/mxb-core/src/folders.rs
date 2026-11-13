//! Locate game installs and their user content folders.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Game {
    MxBikes,
    GpBikes,
    KartRacingPro,
}

impl Game {
    pub fn exe(self) -> &'static str {
        match self {
            Self::MxBikes => "mxbikes.exe",
            Self::GpBikes => "gpbikes.exe",
            Self::KartRacingPro => "krp.exe",
        }
    }
    pub fn user_dir_name(self) -> &'static str {
        match self {
            Self::MxBikes => "MX Bikes",
            Self::GpBikes => "GP Bikes",
            Self::KartRacingPro => "Kart Racing Pro",
        }
    }
    pub fn steam_app_id(self) -> &'static str {
        match self {
            Self::MxBikes => "655500",
            Self::GpBikes => "848050",
            Self::KartRacingPro => "415600",
        }
    }
    fn common_name(self) -> &'static str {
        self.user_dir_name()
    }
    const ALL: [Game; 3] = [Game::MxBikes, Game::GpBikes, Game::KartRacingPro];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Steam,
    Manual,
    Proton,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Install {
    pub game: Game,
    pub install_dir: PathBuf,
    pub user_dir: PathBuf,
    pub mods_dir: PathBuf,
    pub source: Source,
}

pub fn find_installs() -> Vec<Install> {
    let mut steam = Vec::new();
    if let Some(home) = dirs_next::home_dir() {
        steam.extend([
            home.join(".steam/steam"),
            home.join(".local/share/Steam"),
            home.join("Library/Application Support/Steam"),
        ]);
    }
    if cfg!(windows) {
        for var in ["PROGRAMFILES(X86)", "PROGRAMFILES"] {
            if let Some(v) = std::env::var_os(var) {
                steam.push(PathBuf::from(v).join("Steam"));
            }
        }
    }
    let docs = document_candidates();
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for root in steam {
        let libraries = steam_libraries(&root);
        for library in libraries {
            for game in Game::ALL {
                let dir = library.join("steamapps/common").join(game.common_name());
                let prefix = library
                    .join("steamapps/compatdata")
                    .join(game.steam_app_id())
                    .join("pfx/drive_c/users/steamuser");
                // A Proton prefix holds the user folder the game really uses, so it is checked
                // first; the plain Steam entry would otherwise claim the install and point at a
                // Documents folder the game never reads.
                if prefix.is_dir() && dir.join(game.exe()).is_file() {
                    let user = prefix.join("Documents/PiBoSo").join(game.user_dir_name());
                    push(&mut out, &mut seen, game, dir, user, Source::Proton);
                } else {
                    push_if_install(&mut out, &mut seen, game, dir, &docs, Source::Steam);
                }
            }
        }
    }
    for game in Game::ALL {
        if let Some(path) = std::env::var_os(format!(
            "{}_HOME",
            game.exe().trim_end_matches(".exe").to_ascii_uppercase()
        )) {
            push_if_install(
                &mut out,
                &mut seen,
                game,
                PathBuf::from(path),
                &docs,
                Source::Manual,
            );
        }
        let mut manual = Vec::new();
        if let Ok(cwd) = std::env::current_dir() {
            manual.push(cwd);
        }
        if let Some(home) = dirs_next::home_dir() {
            manual.push(home.join(game.common_name()));
        }
        for var in ["PROGRAMFILES", "PROGRAMFILES(X86)"] {
            if let Some(root) = std::env::var_os(var) {
                manual.push(PathBuf::from(root).join(game.common_name()));
            }
        }
        for dir in manual {
            push_if_install(&mut out, &mut seen, game, dir, &docs, Source::Manual);
        }
    }
    out
}

fn push_if_install(
    out: &mut Vec<Install>,
    seen: &mut BTreeSet<PathBuf>,
    game: Game,
    dir: PathBuf,
    docs: &[PathBuf],
    source: Source,
) {
    if !dir.join(game.exe()).is_file() {
        return;
    }
    let user = docs
        .iter()
        .map(|d| d.join("PiBoSo").join(game.user_dir_name()))
        .find(|p| p.is_dir())
        .or_else(|| {
            docs.first()
                .map(|d| d.join("PiBoSo").join(game.user_dir_name()))
        })
        .unwrap_or_default();
    push(out, seen, game, dir, user, source);
}

fn push(
    out: &mut Vec<Install>,
    seen: &mut BTreeSet<PathBuf>,
    game: Game,
    install_dir: PathBuf,
    user_dir: PathBuf,
    source: Source,
) {
    if seen.insert(install_dir.clone()) {
        out.push(Install {
            game,
            install_dir,
            mods_dir: user_dir.join("mods"),
            user_dir,
            source,
        });
    }
}

fn document_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for name in ["OneDriveCommercial", "OneDriveConsumer", "OneDrive"] {
        if let Some(v) = std::env::var_os(name) {
            out.push(PathBuf::from(v).join("Documents"));
        }
    }
    if let Some(p) = dirs_next::document_dir() {
        out.push(p);
    }
    if let Some(p) = dirs_next::home_dir() {
        out.push(p.join("Documents"));
    }
    out.sort();
    out.dedup();
    out
}

fn steam_libraries(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![root.to_path_buf()];
    let text =
        std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")).unwrap_or_default();
    for line in text.lines() {
        let fields: Vec<&str> = line.split('"').collect();
        if let Some(value) = fields
            .get(3)
            .filter(|_| fields.get(1).is_some_and(|k| *k == "path"))
        {
            out.push(PathBuf::from(value.replace("\\\\", "\\")));
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_fake_library_file() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(t.path().join("steamapps")).unwrap();
        std::fs::write(
            t.path().join("steamapps/libraryfolders.vdf"),
            "\"libraryfolders\"\n{\n \"0\"\n {\n  \"path\" \"D:\\\\Steam\"\n }\n}",
        )
        .unwrap();
        assert!(steam_libraries(t.path())
            .iter()
            .any(|p| p == &PathBuf::from(r"D:\Steam")));
    }
}
