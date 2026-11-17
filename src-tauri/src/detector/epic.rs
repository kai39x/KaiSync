//! Epic Games: `%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests\*.item` (JSON).

use std::{fs, path::PathBuf};

use serde::Deserialize;

use super::{Game, Source};

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Item {
    display_name: Option<String>,
    install_location: Option<String>,
    launch_executable: Option<String>,
    app_name: Option<String>,
    main_game_app_name: Option<String>,
    #[serde(default)]
    app_categories: Vec<String>,
    #[serde(default, rename = "bIsIncompleteInstall")]
    incomplete: bool,
}

fn manifests_dir() -> PathBuf {
    let pd = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
    PathBuf::from(pd).join(r"Epic\EpicGamesLauncher\Data\Manifests")
}

pub fn scan() -> Vec<Game> {
    let dir = manifests_dir();
    let Ok(rd) = fs::read_dir(&dir) else {
        tracing::info!("epic: no manifests at {}", dir.display());
        return Vec::new();
    };
    let mut games = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("item")) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else { continue };
        let item: Item = match serde_json::from_str(&text) {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!("epic: bad manifest {}: {e}", path.display());
                continue;
            }
        };
        if item.incomplete {
            continue;
        }
        // Skip Unreal Engine installs, plugins, etc. (old manifests have no categories: keep).
        if !item.app_categories.is_empty()
            && !item.app_categories.iter().any(|c| c.eq_ignore_ascii_case("games"))
        {
            continue;
        }
        let Some(loc) = item.install_location.filter(|l| !l.is_empty()) else { continue };
        let loc = PathBuf::from(loc);
        if !loc.is_dir() {
            continue;
        }
        let id = item.main_game_app_name.or(item.app_name).unwrap_or_else(|| super::norm(&loc));
        let name = item
            .display_name
            .unwrap_or_else(|| loc.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        let mut g = Game::new(format!("epic:{id}"), name, Source::Epic);
        g.exe = item.launch_executable.filter(|e| !e.is_empty()).map(|e| loc.join(e));
        g.install_dir = Some(loc);
        games.push(g);
    }
    games
}
