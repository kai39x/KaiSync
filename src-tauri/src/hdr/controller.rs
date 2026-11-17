//! The HDR restore rules. KaiSync only undoes what KaiSync did:
//!
//! - Game: HDR on. When the game closes, HDR goes back off only if KaiSync turned it on.
//! - Video (the player): HDR off. When the player closes, HDR comes back on only if
//!   KaiSync turned it off.
//! - Idle: undo whichever of the two KaiSync did.
//!
//! Both flags are persisted in `%APPDATA%\KaiSync\runtime.json`, so the rules still hold
//! if KaiSync crashes or is killed mid-game or mid-episode.

use std::fs;

use serde::{Deserialize, Serialize};

use super::Display;
use crate::settings::app_dir;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Runtime {
    /// KaiSync turned HDR on (for a game).
    hdr_on_by_kaisync: bool,
    /// KaiSync turned HDR off (for the video player).
    hdr_off_by_kaisync: bool,
    /// Which display KaiSync changed (device path).
    hdr_display: Option<String>,
}

fn runtime_path() -> std::path::PathBuf {
    app_dir().join("runtime.json")
}

fn load() -> Runtime {
    fs::read_to_string(runtime_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save(r: &Runtime) {
    let _ = fs::create_dir_all(app_dir());
    if let Ok(t) = serde_json::to_string_pretty(r) {
        let _ = fs::write(runtime_path(), t);
    }
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HdrStatus {
    pub display: Option<String>,
    pub supported: bool,
    pub on: bool,
    pub on_by_kaisync: bool,
    pub off_by_kaisync: bool,
}

pub struct HdrController {
    rt: Runtime,
}

impl HdrController {
    pub fn new() -> Self {
        let rt = load();
        if rt.hdr_on_by_kaisync {
            tracing::info!("hdr: last session left HDR on (KaiSync turned it on); will restore when no game runs");
        }
        if rt.hdr_off_by_kaisync {
            tracing::info!("hdr: last session left HDR off (KaiSync turned it off); will restore when the player closes");
        }
        Self { rt }
    }

    fn remember(&mut self, on_by_us: bool, off_by_us: bool, display: Option<&Display>) {
        self.rt.hdr_on_by_kaisync = on_by_us;
        self.rt.hdr_off_by_kaisync = off_by_us;
        self.rt.hdr_display = if on_by_us || off_by_us { display.map(|d| d.id.clone()) } else { None };
        save(&self.rt);
    }

    /// The display KaiSync changed last (if still connected), else the chosen one.
    fn target(&self, chosen: Option<&str>) -> Option<Display> {
        let last = self.rt.hdr_display.clone();
        super::list()
            .into_iter()
            .find(|d| Some(&d.id) == last.as_ref())
            .or_else(|| super::resolve(chosen))
    }

    /// Game: make sure HDR is on.
    pub fn ensure_on(&mut self, chosen: Option<&str>) {
        let Some(d) = super::resolve(chosen) else {
            tracing::warn!("hdr: no HDR-capable display connected; skipping");
            return;
        };
        if d.hdr_on {
            tracing::info!(
                "hdr: already on for {} ({}), leaving it",
                d.name,
                if self.rt.hdr_on_by_kaisync { "KaiSync turned it on earlier" } else { "turned on by you" }
            );
            return;
        }
        match super::set(&d, true) {
            Ok(_) => {
                if self.rt.hdr_off_by_kaisync {
                    // It was on before the player: this is just undoing our "off".
                    tracing::info!("hdr: ON for {} (back as it was before the video)", d.name);
                    self.remember(false, false, None);
                } else {
                    tracing::info!("hdr: ON for {} ({:?} API), KaiSync will turn it off later", d.name, d.api);
                    self.remember(true, false, Some(&d));
                }
            }
            Err(e) => tracing::error!("hdr: could not turn on for {}: {e}", d.name),
        }
    }

    /// Video player: make sure HDR is off.
    pub fn ensure_off(&mut self, chosen: Option<&str>) {
        let Some(d) = self.target(chosen) else { return };
        if !d.hdr_on {
            if self.rt.hdr_on_by_kaisync {
                self.remember(false, false, None);
            }
            tracing::info!("hdr: already off for {}, leaving it", d.name);
            return;
        }
        match super::set(&d, false) {
            Ok(_) => {
                if self.rt.hdr_on_by_kaisync {
                    // A game turned it on: the game closed, so this is the normal restore.
                    tracing::info!("hdr: OFF for {} (game closed, player open)", d.name);
                    self.remember(false, false, None);
                } else {
                    tracing::info!("hdr: OFF for {} (video), KaiSync will turn it back on later", d.name);
                    self.remember(false, true, Some(&d));
                }
            }
            Err(e) => tracing::error!("hdr: could not turn off for {}: {e}", d.name),
        }
    }

    /// Idle / exit: undo whatever KaiSync changed.
    pub fn restore(&mut self, chosen: Option<&str>) {
        let want = match (self.rt.hdr_on_by_kaisync, self.rt.hdr_off_by_kaisync) {
            (true, _) => false, // we turned it on -> off again
            (_, true) => true,  // we turned it off -> on again
            _ => return,
        };
        let Some(d) = self.target(chosen) else {
            tracing::warn!("hdr: the display KaiSync changed is gone; forgetting it");
            self.remember(false, false, None);
            return;
        };
        if d.hdr_on == want {
            tracing::info!("hdr: already {} for {} (you changed it?)", if want { "on" } else { "off" }, d.name);
            self.remember(false, false, None);
            return;
        }
        match super::set(&d, want) {
            Ok(_) => {
                tracing::info!("hdr: {} for {} (restored)", if want { "ON" } else { "OFF" }, d.name);
                self.remember(false, false, None);
            }
            // Keep the flags so we retry on the next apply.
            Err(e) => tracing::error!("hdr: could not restore for {}: {e}", d.name),
        }
    }

    /// A manual test from the UI: no restore duty either way.
    pub fn manual(&mut self, chosen: Option<&str>, on: bool) -> Result<bool, String> {
        let d = super::resolve(chosen).ok_or("no HDR-capable display connected")?;
        let r = super::set(&d, on)?;
        tracing::info!("hdr: manual {} for {}", if on { "ON" } else { "OFF" }, d.name);
        self.remember(false, false, None);
        Ok(r)
    }

    pub fn status(&self, chosen: Option<&str>) -> HdrStatus {
        let base = HdrStatus {
            on_by_kaisync: self.rt.hdr_on_by_kaisync,
            off_by_kaisync: self.rt.hdr_off_by_kaisync,
            ..Default::default()
        };
        match super::resolve(chosen) {
            Some(d) => HdrStatus { display: Some(d.name), supported: d.hdr_supported, on: d.hdr_on, ..base },
            None => base,
        }
    }
}
