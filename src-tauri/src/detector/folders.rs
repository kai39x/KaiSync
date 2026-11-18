//! User "game folders": every direct subfolder is one game. We look for its main exe
//! (depth <= 4), drop installers/helpers, score the rest and auto-pick a clear winner.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{
        Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW},
        UI::Shell::ExtractIconExW,
    },
};

use super::{ignore, norm, FolderReport, Game, Source};

const MAX_DEPTH: usize = 4;
/// Score gap between #1 and #2 needed to auto-pick.
const CLEAR_MARGIN: f64 = 12.0;
const MAX_CANDIDATES: usize = 8;

const SKIP_DIRS: &[&str] = &[
    "_commonredist", "commonredist", "redist", "_redist", "redistributables", "directx",
    "support", "__installer", "installer", "installers", "vcredist", "dotnetfx", "prereqs",
    "prerequisites",
];

const SKIP_EXES: &[&str] = &[
    "unins*", "setup*", "install*", "vcredist*", "vc_redist*", "dxsetup*", "dxwebsetup*",
    "unitycrashhandler*", "crashreport*", "ue4prereqsetup*", "ueprereqsetup*", "dotnetfx*",
    "oalinst*", "physx*", "uninstall*",
];

/// Names that are usually tools rather than the game itself (penalized, not removed).
const TOOLISH: &[&str] = &[
    "*server*", "*editor*", "*config*", "*settings*", "*benchmark*", "*crash*", "*report*",
    "*helper*", "*updater*", "*patcher*", "*redist*", "*touchup*", "*cleanup*", "*modmanager*",
    "*tool*", "*debug*", "*cefprocess*", "*webhelper*",
];

fn skip_dir(path: &Path, name_lower: &str) -> bool {
    if SKIP_DIRS.contains(&name_lower) {
        return true;
    }
    norm(path).contains(r"\engine\binaries\thirdparty")
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<(PathBuf, usize)>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        let path = e.path();
        let name = e.file_name().to_string_lossy().to_lowercase();
        if ft.is_dir() {
            if depth < MAX_DEPTH && !skip_dir(&path, &name) {
                collect(&path, depth + 1, out);
            }
        } else if name.ends_with(".exe") {
            out.push((path, depth));
        }
    }
}

fn alnum(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

fn clean_stem(stem: &str) -> String {
    let mut s = stem.to_lowercase();
    for suf in ["-win64-shipping", "-wingdk-shipping", "-shipping", "_x64", "-x64", "x64", "_dx12", "_dx11", "-dx12", "-dx11"] {
        if let Some(t) = s.strip_suffix(suf) {
            s = t.to_string();
        }
    }
    s
}

fn wide(p: &Path) -> HSTRING {
    HSTRING::from(p.as_os_str())
}

/// Read a string from the exe's version resource (e.g. "FileDescription").
pub fn version_string(path: &Path, key: &str) -> Option<String> {
    unsafe {
        let file = wide(path);
        let size = GetFileVersionInfoSizeW(&file, None);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        GetFileVersionInfoW(&file, None, size, buf.as_mut_ptr().cast()).ok()?;

        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        let trans = HSTRING::from(r"\VarFileInfo\Translation");
        let mut langs: Vec<(u16, u16)> = Vec::new();
        if VerQueryValueW(buf.as_ptr().cast(), &trans, &mut ptr, &mut len).as_bool() && len >= 4 {
            let words = std::slice::from_raw_parts(ptr as *const u16, (len / 2) as usize);
            for c in words.as_chunks::<2>().0 {
                langs.push((c[0], c[1]));
            }
        }
        langs.extend([(0x0409, 0x04b0), (0x0409, 0x04e4), (0x0000, 0x04b0)]);
        for (lang, cp) in langs {
            let q = HSTRING::from(format!(r"\StringFileInfo\{lang:04x}{cp:04x}\{key}"));
            let mut p = std::ptr::null_mut();
            let mut l = 0u32;
            if VerQueryValueW(buf.as_ptr().cast(), &q, &mut p, &mut l).as_bool() && l > 0 {
                let s = std::slice::from_raw_parts(p as *const u16, l as usize);
                let s = String::from_utf16_lossy(s).trim_end_matches('\0').to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
        None
    }
}

fn has_version_info(path: &Path) -> bool {
    unsafe { GetFileVersionInfoSizeW(&wide(path), None) > 0 }
}

fn icon_count(path: &Path) -> u32 {
    let file = wide(path);
    unsafe { ExtractIconExW(PCWSTR(file.as_ptr()), -1, None, None, 0) }
}

fn score(folder_name: &str, path: &Path, depth: usize) -> f64 {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let lower = path.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let a = alnum(folder_name);
    let b = alnum(&clean_stem(&stem));

    let mut sim = strsim::jaro_winkler(&a, &b);
    if a.len().min(b.len()) >= 3 && (a.contains(&b) || b.contains(&a)) {
        sim = sim.max(0.9);
    }
    let mut s = sim * 40.0;
    s -= depth as f64 * 5.0;

    let size_mb = fs::metadata(path).map(|m| m.len() as f64 / 1_048_576.0).unwrap_or(0.0);
    s += ((size_mb + 1.0).log2() * 3.0).min(24.0);

    if has_version_info(path) {
        s += 6.0;
    }
    if icon_count(path) > 0 {
        s += 6.0;
    }
    if lower.ends_with("-win64-shipping.exe") || lower.ends_with("-wingdk-shipping.exe") {
        s += 20.0;
    }
    if TOOLISH.iter().any(|p| ignore::glob(p, &lower)) {
        s -= 15.0;
    }
    s
}

/// Ranked exe candidates for one game subfolder, best first, with scores.
pub fn rank_exes(game_dir: &Path) -> Vec<(PathBuf, f64)> {
    let folder_name = game_dir.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut found = Vec::new();
    collect(game_dir, 0, &mut found);

    found.retain(|(p, _)| {
        let n = p.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        !SKIP_EXES.iter().any(|pat| ignore::glob(&format!("{pat}.exe"), &n)) && !ignore::is_ignored(&n)
    });
    // `*launcher*` only survives if it's the only exe.
    let non_launchers = found
        .iter()
        .filter(|(p, _)| !p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().contains("launcher")))
        .count();
    if non_launchers > 0 {
        found.retain(|(p, _)| !p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().contains("launcher")));
    }

    let mut ranked: Vec<(PathBuf, f64)> = found
        .into_iter()
        .map(|(p, d)| {
            let s = score(&folder_name, &p, d);
            (p, s)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked
}

pub fn scan_root(root: &Path, picks: &BTreeMap<String, String>) -> (Vec<Game>, FolderReport) {
    let mut report = FolderReport {
        path: root.to_string_lossy().into_owned(),
        exists: root.is_dir(),
        games: 0,
        needs_pick: 0,
    };
    let mut games = Vec::new();
    let Ok(rd) = fs::read_dir(root) else {
        tracing::warn!("folders: cannot read {}", root.display());
        return (games, report);
    };
    for e in rd.flatten() {
        if !e.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let dir = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        let ranked = rank_exes(&dir);
        if ranked.is_empty() {
            tracing::debug!("folders: {} has no game exe, skipped", dir.display());
            continue;
        }
        let key = norm(&dir);
        let mut g = Game::new(format!("folder:{key}"), name, Source::Folder);
        g.folder_root = Some(report.path.clone());
        g.candidates = ranked.iter().take(MAX_CANDIDATES).map(|(p, _)| p.clone()).collect();

        let picked = picks.get(&key).map(PathBuf::from).filter(|p| p.exists());
        if let Some(p) = picked {
            g.exe = Some(p);
        } else if ranked.len() == 1 || ranked[0].1 - ranked[1].1 >= CLEAR_MARGIN {
            g.exe = Some(ranked[0].0.clone());
        } else {
            g.needs_pick = true;
            report.needs_pick += 1;
        }
        tracing::debug!(
            "folders: {} -> {:?} [{}]",
            dir.display(),
            g.exe,
            ranked.iter().take(4).map(|(p, s)| format!("{}={s:.1}", p.file_name().unwrap_or_default().to_string_lossy())).collect::<Vec<_>>().join(", ")
        );
        games.push(g);
    }
    report.games = games.len();
    (games, report)
}
