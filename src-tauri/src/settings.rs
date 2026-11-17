//! Persistent settings, stored as plain JSON in `%APPDATA%\KaiSync\settings.json`.

use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Sources {
    pub steam: bool,
    pub epic: bool,
    pub xbox: bool,
}

impl Default for Sources {
    fn default() -> Self {
        Self { steam: true, epic: true, xbox: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManualGame {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// "ar" (default) or "en".
    pub language: String,
    pub sources: Sources,
    /// Folders whose direct subfolders are each one game.
    pub game_folders: Vec<String>,
    /// For ambiguous folder games: game subfolder path (lowercase) -> chosen exe.
    pub folder_picks: BTreeMap<String, String>,
    pub manual_games: Vec<ManualGame>,
    /// Per-game enable overrides (game id -> enabled). Missing = the game's default.
    pub game_enabled: BTreeMap<String, bool>,
    /// HDR display to control (monitor device path). `None` = primary HDR display.
    pub hdr_display: Option<String>,
    /// The one video player. `None` = auto-detect PotPlayer.
    pub player_path: Option<String>,
    /// Start with Windows (tray only). Applied to the system in milestone 6.
    pub autostart: bool,
    /// Desktop shortcut. Applied to the system in milestone 6.
    pub desktop_shortcut: bool,
    pub debounce_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "ar".into(),
            sources: Sources::default(),
            game_folders: Vec::new(),
            folder_picks: BTreeMap::new(),
            manual_games: Vec::new(),
            game_enabled: BTreeMap::new(),
            hdr_display: None,
            player_path: None,
            autostart: true,
            desktop_shortcut: true,
            debounce_ms: 3000,
        }
    }
}

/// `%APPDATA%\KaiSync`
pub fn app_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("KaiSync")
}

fn settings_path() -> PathBuf {
    app_dir().join("settings.json")
}

impl Settings {
    pub fn load() -> Self {
        let path = settings_path();
        match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Settings>(&text) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("settings.json is invalid ({e}); keeping a backup and using defaults");
                    let _ = fs::copy(&path, path.with_extension("json.bad"));
                    Settings::default()
                }
            },
            Err(_) => {
                let s = Settings::default();
                s.save();
                s
            }
        }
    }

    pub fn save(&self) {
        let path = settings_path();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                // Write to a temp file first so a crash never leaves a half-written file.
                let tmp = path.with_extension("json.tmp");
                if fs::write(&tmp, text).is_ok() {
                    let _ = fs::rename(&tmp, &path);
                }
            }
            Err(e) => tracing::error!("could not serialize settings: {e}"),
        }
    }
}
