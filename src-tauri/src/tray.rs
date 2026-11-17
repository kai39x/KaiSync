//! Tray icon (a small glass orb in the mode's colors) + the glass tray menu.
//!
//! Left click opens the main window. Right click opens a small frameless, transparent
//! window (`tray-menu`) rendered by the frontend, styled like `design/Tray.html`.

use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder,
};

use crate::{
    engine::{Engine, Status},
    state_machine::Mode,
};

const TRAY_ID: &str = "main";
const SIZE: u32 = 32;
pub const MENU_LABEL: &str = "tray-menu";
/// Logical size of the menu window (the frontend's menu card fills it).
const MENU_W: f64 = 296.0;
const MENU_H: f64 = 246.0;

fn modes(lang: &str) -> [&'static str; 3] {
    if lang == "en" {
        ["Idle", "Game mode", "Paused"]
    } else {
        ["وضع عادي", "وضع اللعب", "الأتمتة موقفة"]
    }
}

fn lang(app: &AppHandle) -> String {
    app.try_state::<Engine>()
        .map(|e| e.settings.lock().unwrap().language.clone())
        .unwrap_or_else(|| "ar".into())
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// The hidden glass menu window, created once at startup.
pub fn create_menu_window(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, MENU_LABEL, WebviewUrl::App("index.html".into()))
        .title("KaiSync")
        .inner_size(MENU_W, MENU_H)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .focused(false)
        .build()?;
    Ok(())
}

pub fn hide_menu(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MENU_LABEL) {
        let _ = w.hide();
    }
}

/// Open the glass menu just above the click, kept inside the screen's work area.
fn show_menu(app: &AppHandle, at: PhysicalPosition<f64>) {
    let Some(w) = app.get_webview_window(MENU_LABEL) else { return };
    let scale = w
        .monitor_from_point(at.x, at.y)
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0);
    // The frontend sizes the window to its card; fall back to the design size.
    let (ww, wh) = match w.outer_size() {
        Ok(s) if s.width > 0 && s.height > 0 => (s.width as i32, s.height as i32),
        _ => ((MENU_W * scale) as i32, (MENU_H * scale) as i32),
    };
    let mut x = at.x as i32 - ww;
    let mut y = at.y as i32 - wh - (6.0 * scale) as i32;
    if let Ok(Some(m)) = w.monitor_from_point(at.x, at.y) {
        let wa = m.work_area();
        let (l, t) = (wa.position.x, wa.position.y);
        let (r, b) = (l + wa.size.width as i32, t + wa.size.height as i32);
        x = x.clamp(l, (r - ww).max(l));
        y = y.clamp(t, (b - wh).max(t));
    }
    let _ = w.set_position(PhysicalPosition::new(x, y));
    let _ = w.show();
    let _ = w.set_focus();
    let _ = app.emit_to(MENU_LABEL, "tray-menu-opened", ());
}

pub fn create(app: &AppHandle, _language: &str) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(orb_icon(Mode::Idle, false))
        .tooltip("KaiSync")
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button, button_state: MouseButtonState::Up, position, .. } = event {
                match button {
                    MouseButton::Left => {
                        hide_menu(tray.app_handle());
                        show_main(tray.app_handle());
                    }
                    MouseButton::Right => show_menu(tray.app_handle(), position),
                    _ => {}
                }
            }
        })
        .build(app)?;
    Ok(())
}

/// Refresh icon and tooltip from the current status (the glass menu follows status events).
pub fn update(app: &AppHandle, st: &Status) {
    let m = modes(&lang(app));
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(orb_icon(st.mode, st.paused)));
        let mode = if st.paused {
            m[2]
        } else {
            match st.mode {
                Mode::Idle => m[0],
                Mode::Game => m[1],
            }
        };
        let tip = match (&st.detail, st.paused) {
            (Some(d), false) => format!("KaiSync · {mode} · {d}"),
            _ => format!("KaiSync · {mode}"),
        };
        let _ = tray.set_tooltip(Some(tip));
    }
}

fn hex(c: u32) -> [f32; 3] {
    [((c >> 16) & 0xff) as f32, ((c >> 8) & 0xff) as f32, (c & 0xff) as f32]
}

/// Render a glass orb: conic mix of the mode colors, specular dot top-left, bright rim.
pub fn orb_icon(mode: Mode, paused: bool) -> Image<'static> {
    // Same palettes as the design mockups.
    let stops: [u32; 4] = if paused {
        [0x9a9a9a, 0x6a6a6a, 0xb0b0b0, 0x5a5a5a]
    } else {
        match mode {
            Mode::Game => [0x5cc8ff, 0x9b8cff, 0x6ee7ff, 0x5b6cff],
            Mode::Idle => [0xe8e8e8, 0x8a8a8a, 0xf5f5f5, 0x5a5a5a],
        }
    };
    let stops: Vec<[f32; 3]> = stops.iter().map(|&c| hex(c)).collect();
    let n = SIZE as f32;
    let (cx, cy, r) = (n / 2.0, n / 2.0, n / 2.0 - 1.5);
    let mut px = vec![0u8; (SIZE * SIZE * 4) as usize];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let (dx, dy) = (fx - cx, fy - cy);
            let d = (dx * dx + dy * dy).sqrt();
            let cover = (r + 0.5 - d).clamp(0.0, 1.0); // anti-aliased edge
            if cover <= 0.0 {
                continue;
            }
            // Conic gradient around the center (4 stops, wrapping).
            let ang = (dy.atan2(dx) + std::f32::consts::PI) / (2.0 * std::f32::consts::PI) * 4.0;
            let i = ang.floor() as usize % 4;
            let t = ang.fract();
            let t = t * t * (3.0 - 2.0 * t);
            let a = stops[i];
            let b = stops[(i + 1) % 4];
            let mut c = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];

            let rn = d / r;
            // Slightly darker toward the center (clear glass), brighter rim.
            let shade = 0.78 + 0.22 * rn;
            for v in &mut c {
                *v *= shade;
            }
            let rim = ((rn - 0.82) / 0.18).clamp(0.0, 1.0);
            for v in &mut c {
                *v += (255.0 - *v) * rim * 0.55;
            }
            // Specular dot at (32%, 26%).
            let (sx, sy) = (n * 0.32, n * 0.26);
            let sd = ((fx - sx).powi(2) + (fy - sy).powi(2)).sqrt() / n;
            let spec = (1.0 - sd / 0.16).clamp(0.0, 1.0).powf(1.6);
            for v in &mut c {
                *v += (255.0 - *v) * spec * 0.9;
            }
            // Thin dark outline so the orb reads on a light taskbar too.
            let outline = ((d - (r - 0.9)) / 0.9).clamp(0.0, 1.0);
            for v in &mut c {
                *v *= 1.0 - outline * 0.55;
            }

            let mut alpha = cover;
            if paused {
                alpha *= 0.55;
                // Two pause bars.
                let in_bar = |x0: f32| fx >= x0 && fx < x0 + n * 0.11 && fy >= n * 0.3 && fy < n * 0.7;
                if in_bar(n * 0.34) || in_bar(n * 0.55) {
                    c = [250.0, 250.0, 250.0];
                    alpha = cover;
                }
            }
            let o = ((y * SIZE + x) * 4) as usize;
            px[o] = c[0].clamp(0.0, 255.0) as u8;
            px[o + 1] = c[1].clamp(0.0, 255.0) as u8;
            px[o + 2] = c[2].clamp(0.0, 255.0) as u8;
            px[o + 3] = (alpha * 255.0) as u8;
        }
    }
    Image::new_owned(px, SIZE, SIZE)
}
