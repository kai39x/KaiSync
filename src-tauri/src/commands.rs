//! Tauri commands used by the UI.

use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{
    detector::{self, norm, FolderReport, Game, PlayerInfo},
    engine::{Cmd, Engine, Status},
    settings::{ManualGame, Settings},
    tray,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryView {
    games: Vec<Game>,
    folders: Vec<FolderReport>,
    player: Option<PlayerInfo>,
}

#[tauri::command]
pub fn get_status(engine: State<Engine>) -> Status {
    engine.status.lock().unwrap().clone()
}

#[tauri::command]
pub fn get_library(engine: State<Engine>) -> LibraryView {
    let lib = engine.library.read().unwrap();
    LibraryView { games: lib.games.clone(), folders: lib.folders.clone(), player: lib.player.clone() }
}

#[tauri::command]
pub fn get_settings(engine: State<Engine>) -> Settings {
    engine.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_paused(engine: State<Engine>, paused: bool) {
    engine.send(Cmd::SetPaused(paused));
}

#[tauri::command]
pub fn rescan(engine: State<Engine>) {
    engine.send(Cmd::Rescan);
}

#[tauri::command]
pub fn set_source(engine: State<Engine>, source: String, enabled: bool) -> Settings {
    engine.update_settings(true, |s| match source.as_str() {
        "steam" => s.sources.steam = enabled,
        "epic" => s.sources.epic = enabled,
        "xbox" => s.sources.xbox = enabled,
        _ => {}
    })
}

#[tauri::command]
pub fn add_game_folder(engine: State<Engine>, path: String) -> Settings {
    engine.update_settings(true, |s| {
        if !s.game_folders.iter().any(|f| norm(Path::new(f)) == norm(Path::new(&path))) {
            s.game_folders.push(path);
        }
    })
}

#[tauri::command]
pub fn remove_game_folder(engine: State<Engine>, path: String) -> Settings {
    engine.update_settings(true, |s| s.game_folders.retain(|f| f != &path))
}

#[tauri::command]
pub fn add_manual_game(engine: State<Engine>, path: String) -> Settings {
    let p = Path::new(&path);
    let name = detector::folders::version_string(p, "ProductName")
        .or_else(|| detector::folders::version_string(p, "FileDescription"))
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
    engine.update_settings(true, |s| {
        if !s.manual_games.iter().any(|g| norm(Path::new(&g.path)) == norm(p)) {
            s.manual_games.push(ManualGame { path: path.clone(), name: name.trim().to_string() });
        }
    })
}

#[tauri::command]
pub fn remove_manual_game(engine: State<Engine>, path: String) -> Settings {
    engine.update_settings(true, |s| s.manual_games.retain(|g| g.path != path))
}

#[tauri::command]
pub fn set_game_enabled(engine: State<Engine>, id: String, enabled: bool) -> Settings {
    engine.update_settings(true, |s| {
        s.game_enabled.insert(id, enabled);
    })
}

/// Choose the exe for an ambiguous folder game (`id` = `folder:<path>`).
#[tauri::command]
pub fn pick_folder_exe(engine: State<Engine>, id: String, exe: String) -> Settings {
    let key = id.strip_prefix("folder:").unwrap_or(&id).to_string();
    engine.update_settings(true, |s| {
        s.folder_picks.insert(key, exe);
    })
}

#[tauri::command]
pub fn set_language(app: AppHandle, engine: State<Engine>, language: String) -> Settings {
    let s = engine.update_settings(false, |s| s.language = language);
    let st = engine.status.lock().unwrap().clone();
    tray::update(&app, &st);
    s
}

#[tauri::command]
pub fn open_logs_folder() {
    let dir = crate::settings::app_dir().join("logs");
    let _ = std::process::Command::new("explorer").arg(dir).spawn();
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplaysView {
    /// HDR-capable displays only.
    displays: Vec<crate::hdr::Display>,
    /// The chosen one (device path), or the default if none is saved.
    selected: Option<String>,
}

#[tauri::command]
pub fn list_displays(engine: State<Engine>) -> DisplaysView {
    let chosen = engine.settings.lock().unwrap().hdr_display.clone();
    let displays = crate::hdr::list();
    let selected = crate::hdr::resolve(chosen.as_deref()).map(|d| d.id);
    DisplaysView { displays, selected }
}

#[tauri::command]
pub fn set_hdr_display(engine: State<Engine>, id: String) -> Settings {
    tracing::info!("hdr: display chosen: {id}");
    let s = engine.update_settings(false, |s| s.hdr_display = Some(id));
    engine.send(Cmd::HdrRefresh);
    s
}

/// HDR test buttons. Runs on the engine thread; returns the state read back.
#[tauri::command]
pub async fn hdr_test(engine: State<'_, Engine>, on: bool) -> Result<bool, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    engine.send(Cmd::HdrManual(on, tx));
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv_timeout(std::time::Duration::from_secs(15)).map_err(|_| "timed out".to_string())?
    })
    .await
    .map_err(|e| e.to_string())?
}

/// General toggles: "autostart" | "desktopShortcut".
#[tauri::command]
pub fn set_general(app: AppHandle, engine: State<Engine>, key: String, value: bool) -> Result<Settings, String> {
    match key.as_str() {
        "autostart" => {
            crate::system::set_autostart(&app, value);
            Ok(engine.update_settings(false, |s| s.autostart = value))
        }
        "desktopShortcut" => {
            crate::system::set_shortcut(value)?;
            Ok(engine.update_settings(false, |s| s.desktop_shortcut = value))
        }
        other => Err(format!("unknown setting {other}")),
    }
}

/// Glass tray menu actions: "open" | "pause" | "exit".
#[tauri::command]
pub fn tray_action(app: AppHandle, engine: State<Engine>, action: String) {
    tray::hide_menu(&app);
    match action.as_str() {
        "open" => tray::show_main(&app),
        "pause" => engine.send(Cmd::TogglePause),
        "exit" => crate::exit_app(&app),
        "close" => {}
        other => tracing::warn!("tray: unknown action {other}"),
    }
}

/// Choose the video player exe (file picker in Settings).
#[tauri::command]
pub fn set_player(engine: State<Engine>, path: String) -> Settings {
    tracing::info!("player chosen: {path}");
    engine.update_settings(true, |s| s.player_path = Some(path))
}
