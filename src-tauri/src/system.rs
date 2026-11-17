//! Windows integration: start with Windows (tray only) and the desktop shortcut.
//!
//! Debug builds never touch the system (they'd register the dev exe, which needs the
//! Vite server). Set `KAISYNC_SYSTEM=1` to force it while testing.

use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;
use windows::{
    core::{Interface, HSTRING},
    Win32::{
        System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED},
        UI::Shell::{IShellLinkW, ShellLink},
    },
};

pub const BACKGROUND_ARG: &str = "--background";

fn enabled() -> bool {
    !cfg!(debug_assertions) || std::env::var("KAISYNC_SYSTEM").is_ok_and(|v| v == "1")
}

/// Start with Windows: `HKCU\...\Run` entry launching `KaiSync.exe --background`.
pub fn set_autostart(app: &AppHandle, on: bool) {
    if !enabled() {
        tracing::info!("autostart: {} (skipped in a dev build)", if on { "on" } else { "off" });
        return;
    }
    let al = app.autolaunch();
    let r = if on {
        // Always (re)write it: the entry must point at *this* exe, e.g. after moving from a
        // portable copy or a dev build to the installed one.
        al.enable()
    } else if al.is_enabled().unwrap_or(false) {
        al.disable()
    } else {
        Ok(())
    };
    match r {
        Ok(()) => tracing::info!("autostart: {}", if on { "on" } else { "off" }),
        Err(e) => tracing::error!("autostart: could not turn {}: {e}", if on { "on" } else { "off" }),
    }
}

pub fn shortcut_path() -> Option<PathBuf> {
    dirs::desktop_dir().map(|d| d.join("KaiSync.lnk"))
}

pub fn shortcut_exists() -> bool {
    shortcut_path().is_some_and(|p| p.exists())
}

/// Create or remove `Desktop\KaiSync.lnk` (opens the window: no `--background`).
pub fn set_shortcut(on: bool) -> Result<(), String> {
    if !enabled() {
        tracing::info!("shortcut: {} (skipped in a dev build)", if on { "create" } else { "remove" });
        return Ok(());
    }
    let path = shortcut_path().ok_or("no Desktop folder")?;
    if !on {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("could not remove the shortcut: {e}"))?;
        }
        tracing::info!("shortcut: removed");
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // COM on its own thread: the main thread's apartment belongs to WebView2.
    std::thread::spawn(move || unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let r = (|| -> windows::core::Result<()> {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.SetPath(&HSTRING::from(exe.as_os_str()))?;
            if let Some(dir) = exe.parent() {
                link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))?;
            }
            link.SetIconLocation(&HSTRING::from(exe.as_os_str()), 0)?;
            link.SetDescription(&HSTRING::from("KaiSync"))?;
            let file: IPersistFile = link.cast()?;
            file.Save(&HSTRING::from(path.as_os_str()), true)
        })();
        CoUninitialize();
        r.map_err(|e| e.to_string())
    })
    .join()
    .map_err(|_| "shortcut thread panicked".to_string())??;
    tracing::info!("shortcut: created");
    Ok(())
}
