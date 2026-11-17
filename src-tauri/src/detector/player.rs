//! The one video player (default: PotPlayer). While it runs, KaiSync turns HDR off.

use std::path::{Path, PathBuf};

use winreg::{enums::*, RegKey};

use crate::settings::Settings;

pub const DEFAULT_PLAYER_EXE: &str = "PotPlayerMini64.exe";

/// Try to find PotPlayer's install path.
pub fn find_potplayer() -> Option<PathBuf> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(k) = hkcu.open_subkey(r"Software\DAUM\PotPlayer64") {
        if let Ok(p) = k.get_value::<String, _>("ProgramPath") {
            let p = PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        if let Ok(dir) = k.get_value::<String, _>("ProgramFolder") {
            let p = Path::new(&dir).join(DEFAULT_PLAYER_EXE);
            if p.exists() {
                return Some(p);
            }
        }
    }
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(k) = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PotPlayer64") {
        if let Ok(icon) = k.get_value::<String, _>("DisplayIcon") {
            let p = PathBuf::from(icon.trim_matches('"').split(',').next().unwrap_or_default());
            if p.exists() {
                return Some(p);
            }
        }
    }
    for base in [
        std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into()),
        std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into()),
    ] {
        for sub in [r"DAUM\PotPlayer", "PotPlayer"] {
            let p = Path::new(&base).join(sub).join(DEFAULT_PLAYER_EXE);
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

/// The chosen player, else auto-detected PotPlayer, else the bare default exe name
/// (matched by name when the path can't be read).
pub fn resolve(settings: &Settings) -> PathBuf {
    match &settings.player_path {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => find_potplayer().unwrap_or_else(|| PathBuf::from(DEFAULT_PLAYER_EXE)),
    }
}

pub fn display_name(p: &Path) -> String {
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    if stem.to_lowercase().starts_with("potplayer") {
        return "PotPlayer".into();
    }
    if let Some(desc) = super::folders::version_string(p, "FileDescription") {
        if !desc.trim().is_empty() {
            return desc.trim().to_string();
        }
    }
    stem
}
