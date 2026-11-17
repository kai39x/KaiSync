//! The HDR restore rule: KaiSync only turns HDR off if KaiSync turned it on.
//!
//! The "we turned it on" flag is persisted in `%APPDATA%\KaiSync\runtime.json`, so the rule
//! still holds if KaiSync crashes or is killed while a game is running.

use std::fs;

use serde::{Deserialize, Serialize};

use super::Display;
use crate::settings::app_dir;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Runtime {
    hdr_on_by_kaisync: bool,
    /// Which display we turned on (device path).
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
        Self { rt }
    }

    fn set_flag(&mut self, on: bool, display: Option<&Display>) {
        self.rt.hdr_on_by_kaisync = on;
        self.rt.hdr_display = if on { display.map(|d| d.id.clone()) } else { None };
        save(&self.rt);
    }

    /// Entering Game: make sure HDR is on. Remember if we were the ones who turned it on.
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
                tracing::info!("hdr: ON for {} ({:?} API), KaiSync will turn it off later", d.name, d.api);
                self.set_flag(true, Some(&d));
            }
            Err(e) => tracing::error!("hdr: could not turn on for {}: {e}", d.name),
        }
    }

    /// Leaving Game / exit: turn HDR off only if KaiSync turned it on.
    pub fn restore(&mut self, chosen: Option<&str>) {
        if !self.rt.hdr_on_by_kaisync {
            return;
        }
        // Prefer the display we actually turned on, even if the choice changed since.
        let target = self.rt.hdr_display.clone();
        let d = super::list()
            .into_iter()
            .find(|d| Some(&d.id) == target.as_ref())
            .or_else(|| super::resolve(chosen));
        let Some(d) = d else {
            tracing::warn!("hdr: the display KaiSync turned on is gone; forgetting it");
            self.set_flag(false, None);
            return;
        };
        if !d.hdr_on {
            tracing::info!("hdr: already off for {} (you turned it off?)", d.name);
            self.set_flag(false, None);
            return;
        }
        match super::set(&d, false) {
            Ok(_) => {
                tracing::info!("hdr: OFF for {} (restored)", d.name);
                self.set_flag(false, None);
            }
            // Keep the flag so we retry on the next Idle apply.
            Err(e) => tracing::error!("hdr: could not turn off for {}: {e}", d.name),
        }
    }

    /// A manual test from the UI: no restore duty either way.
    pub fn manual(&mut self, chosen: Option<&str>, on: bool) -> Result<bool, String> {
        let d = super::resolve(chosen).ok_or("no HDR-capable display connected")?;
        let r = super::set(&d, on)?;
        tracing::info!("hdr: manual {} for {}", if on { "ON" } else { "OFF" }, d.name);
        self.set_flag(false, None);
        Ok(r)
    }

    pub fn status(&self, chosen: Option<&str>) -> HdrStatus {
        match super::resolve(chosen) {
            Some(d) => HdrStatus {
                display: Some(d.name),
                supported: d.hdr_supported,
                on: d.hdr_on,
                on_by_kaisync: self.rt.hdr_on_by_kaisync,
            },
            None => HdrStatus { on_by_kaisync: self.rt.hdr_on_by_kaisync, ..Default::default() },
        }
    }
}
