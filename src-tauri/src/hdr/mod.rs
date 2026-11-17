//! Windows HDR via the DisplayConfig API (no admin needed).
//!
//! Windows 11 24H2+ uses `GET_ADVANCED_COLOR_INFO_2` / `SET_HDR_STATE` (HDR is separate
//! from the new SDR "auto color management"). Older builds use
//! `GET_ADVANCED_COLOR_INFO` / `SET_ADVANCED_COLOR_STATE`.

pub mod controller;

use std::{mem::size_of, thread, time::Duration};

use serde::Serialize;
use windows::Win32::{
    Devices::Display::{
        DisplayConfigGetDeviceInfo, DisplayConfigSetDeviceInfo, GetDisplayConfigBufferSizes,
        QueryDisplayConfig, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
        DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_DEVICE_INFO_TYPE,
        DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE, DISPLAYCONFIG_PATH_INFO,
        DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
    },
    Foundation::{ERROR_SUCCESS, LUID},
};

const GET_ADVANCED_COLOR_INFO: i32 = 9;
const SET_ADVANCED_COLOR_STATE: i32 = 10;
const GET_ADVANCED_COLOR_INFO_2: i32 = 15;
const SET_HDR_STATE: i32 = 16;
const COLOR_MODE_HDR: i32 = 2;

// Own definitions (the 24H2 structs aren't in the `windows` crate yet). Layouts from wingdi.h.
#[repr(C)]
struct AdvancedColorInfo {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    value: u32,
    color_encoding: i32,
    bits_per_color_channel: u32,
}

#[repr(C)]
struct AdvancedColorInfo2 {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    value: u32,
    color_encoding: i32,
    bits_per_color_channel: u32,
    active_color_mode: i32,
}

#[repr(C)]
struct SetState {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    value: u32,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Api {
    /// 24H2+: ADVANCED_COLOR_INFO_2 / SET_HDR_STATE
    V2,
    /// older: ADVANCED_COLOR_INFO / SET_ADVANCED_COLOR_STATE
    V1,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Display {
    /// Stable id: the monitor device path.
    pub id: String,
    pub name: String,
    pub primary: bool,
    pub hdr_supported: bool,
    pub hdr_on: bool,
    pub api: Api,
    #[serde(skip)]
    adapter: LUID,
    #[serde(skip)]
    target: u32,
}

fn header(kind: i32, size: usize, adapter: LUID, id: u32) -> DISPLAYCONFIG_DEVICE_INFO_HEADER {
    DISPLAYCONFIG_DEVICE_INFO_HEADER {
        r#type: DISPLAYCONFIG_DEVICE_INFO_TYPE(kind),
        size: size as u32,
        adapterId: adapter,
        id,
    }
}

fn wstr(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn active_paths() -> Result<(Vec<DISPLAYCONFIG_PATH_INFO>, Vec<DISPLAYCONFIG_MODE_INFO>), String> {
    // The topology can change between the two calls; retry a few times.
    for _ in 0..3 {
        let (mut np, mut nm) = (0u32, 0u32);
        let rc = unsafe { GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) };
        if rc != ERROR_SUCCESS {
            return Err(format!("GetDisplayConfigBufferSizes failed: {rc:?}"));
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
        let rc = unsafe {
            QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), None)
        };
        if rc == ERROR_SUCCESS {
            paths.truncate(np as usize);
            modes.truncate(nm as usize);
            return Ok((paths, modes));
        }
    }
    Err("QueryDisplayConfig failed".into())
}

/// Read (supported, on, api) for one target.
fn read_color(adapter: LUID, target: u32) -> Option<(bool, bool, Api)> {
    let mut v2 = AdvancedColorInfo2 {
        header: header(GET_ADVANCED_COLOR_INFO_2, size_of::<AdvancedColorInfo2>(), adapter, target),
        value: 0,
        color_encoding: 0,
        bits_per_color_channel: 0,
        active_color_mode: 0,
    };
    if unsafe { DisplayConfigGetDeviceInfo(&mut v2.header) } == 0 {
        let supported = v2.value & (1 << 4) != 0; // highDynamicRangeSupported
        let user_on = v2.value & (1 << 5) != 0; // highDynamicRangeUserEnabled
        return Some((supported, user_on || v2.active_color_mode == COLOR_MODE_HDR, Api::V2));
    }
    let mut v1 = AdvancedColorInfo {
        header: header(GET_ADVANCED_COLOR_INFO, size_of::<AdvancedColorInfo>(), adapter, target),
        value: 0,
        color_encoding: 0,
        bits_per_color_channel: 0,
    };
    if unsafe { DisplayConfigGetDeviceInfo(&mut v1.header) } == 0 {
        let supported = v1.value & 1 != 0; // advancedColorSupported
        let enabled = v1.value & 2 != 0; // advancedColorEnabled
        let force_disabled = v1.value & 8 != 0; // advancedColorForceDisabled
        return Some((supported && !force_disabled, enabled, Api::V1));
    }
    None
}

/// All active displays (HDR-capable or not).
pub fn list_all() -> Result<Vec<Display>, String> {
    let (paths, modes) = active_paths()?;
    let mut out: Vec<Display> = Vec::new();
    for p in &paths {
        let adapter = p.targetInfo.adapterId;
        let target = p.targetInfo.id;

        let mut name = DISPLAYCONFIG_TARGET_DEVICE_NAME {
            header: header(
                DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME.0,
                size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>(),
                adapter,
                target,
            ),
            ..Default::default()
        };
        let (friendly, path) = if unsafe { DisplayConfigGetDeviceInfo(&mut name.header) } == 0 {
            (wstr(&name.monitorFriendlyDeviceName), wstr(&name.monitorDevicePath))
        } else {
            (String::new(), String::new())
        };
        let id = if path.is_empty() { format!("target:{target}") } else { path };
        if out.iter().any(|d| d.id == id) {
            continue; // cloned display: same monitor on two paths
        }

        // The primary display's source sits at desktop position (0, 0).
        let idx = unsafe { p.sourceInfo.Anonymous.modeInfoIdx } as usize;
        let primary = modes.get(idx).is_some_and(|m| {
            m.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE && unsafe {
                let pos = m.Anonymous.sourceMode.position;
                pos.x == 0 && pos.y == 0
            }
        });

        let (hdr_supported, hdr_on, api) = read_color(adapter, target).unwrap_or((false, false, Api::V1));
        out.push(Display {
            id,
            name: if friendly.is_empty() { format!("Display {}", out.len() + 1) } else { friendly },
            primary,
            hdr_supported,
            hdr_on,
            api,
            adapter,
            target,
        });
    }
    Ok(out)
}

/// HDR-capable displays only (what the UI offers).
pub fn list() -> Vec<Display> {
    match list_all() {
        Ok(v) => v.into_iter().filter(|d| d.hdr_supported).collect(),
        Err(e) => {
            tracing::error!("hdr: cannot list displays: {e}");
            Vec::new()
        }
    }
}

/// The chosen display, else the primary HDR display, else the first HDR display.
pub fn resolve(chosen: Option<&str>) -> Option<Display> {
    let displays = list();
    if let Some(id) = chosen {
        if let Some(d) = displays.iter().find(|d| d.id == id) {
            return Some(d.clone());
        }
        tracing::warn!("hdr: chosen display {id} not connected; using the default");
    }
    displays.iter().find(|d| d.primary).or(displays.first()).cloned()
}

/// Turn HDR on/off and verify. Returns the state read back afterwards.
pub fn set(d: &Display, on: bool) -> Result<bool, String> {
    let kind = match d.api {
        Api::V2 => SET_HDR_STATE,
        Api::V1 => SET_ADVANCED_COLOR_STATE,
    };
    let packet = SetState { header: header(kind, size_of::<SetState>(), d.adapter, d.target), value: on as u32 };
    let rc = unsafe { DisplayConfigSetDeviceInfo(&packet.header) };
    if rc != 0 {
        return Err(format!("DisplayConfigSetDeviceInfo({kind}) failed with code {rc}"));
    }
    // The mode switch takes a moment; read back to confirm.
    for _ in 0..10 {
        thread::sleep(Duration::from_millis(150));
        if let Some((_, now_on, _)) = read_color(d.adapter, d.target) {
            if now_on == on {
                return Ok(now_on);
            }
        }
    }
    let now = read_color(d.adapter, d.target).map(|(_, o, _)| o).unwrap_or(!on);
    if now == on {
        Ok(now)
    } else {
        Err(format!("HDR is still {} after the request", if now { "on" } else { "off" }))
    }
}

#[cfg(test)]
mod tests {
    /// Read-only: prints what KaiSync sees. `cargo test hdr_probe -- --nocapture`
    #[test]
    fn hdr_probe() {
        let all = super::list_all().expect("list displays");
        for d in &all {
            println!("{:?} primary={} supported={} on={} api={:?} id={}", d.name, d.primary, d.hdr_supported, d.hdr_on, d.api, d.id);
        }
        println!("default HDR display: {:?}", super::resolve(None).map(|d| d.name));
    }
}
