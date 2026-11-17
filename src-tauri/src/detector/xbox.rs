//! Xbox / Microsoft Store (best effort): the `XboxGames` folder on each drive,
//! plus whatever folder `X:\.GamingRoot` points to.

use std::{
    fs,
    path::{Path, PathBuf},
};

use super::{Game, Source};

/// `.GamingRoot` = "RGBX" + u32 count + UTF-16LE NUL-terminated paths relative to the drive root.
fn gaming_roots(drive: &Path) -> Vec<PathBuf> {
    let Ok(bytes) = fs::read(drive.join(".GamingRoot")) else { return Vec::new() };
    if bytes.len() < 8 || &bytes[..4] != b"RGBX" {
        return Vec::new();
    }
    let units: Vec<u16> = bytes[8..]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    units
        .split(|&u| u == 0)
        .filter(|s| !s.is_empty())
        .map(|s| {
            let rel = String::from_utf16_lossy(s);
            let rel = rel.trim_start_matches(['\\', '/']);
            drive.join(rel)
        })
        .collect()
}

fn attr(xml: &str, tag: &str, attr: &str) -> Option<String> {
    let start = xml.find(&format!("<{tag}"))?;
    let rest = &xml[start..];
    let end = rest.find('>')?;
    let el = &rest[..end];
    let key = format!("{attr}=\"");
    let a = el.find(&key)? + key.len();
    let b = el[a..].find('"')?;
    Some(el[a..a + b].to_string())
}

pub fn scan() -> Vec<Game> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for letter in b'A'..=b'Z' {
        let drive = PathBuf::from(format!("{}:\\", letter as char));
        if !drive.exists() {
            continue;
        }
        let mut candidates = gaming_roots(&drive);
        candidates.push(drive.join("XboxGames"));
        for c in candidates {
            if c.is_dir() && !roots.iter().any(|r| super::norm(r) == super::norm(&c)) {
                roots.push(c);
            }
        }
    }

    let mut games = Vec::new();
    for root in roots {
        let Ok(rd) = fs::read_dir(&root) else { continue };
        for entry in rd.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let folder = entry.file_name().to_string_lossy().into_owned();
            let config = dir.join(r"Content\MicrosoftGame.config");
            // Real installs have a Content folder; skips e.g. `GameSave`.
            if !dir.join("Content").is_dir() {
                continue;
            }
            let xml = fs::read_to_string(&config).unwrap_or_default();
            let name = attr(&xml, "ShellVisuals", "DefaultDisplayName")
                .filter(|n| !n.is_empty() && !n.starts_with("ms-resource:"))
                .unwrap_or_else(|| folder.clone());
            let mut g = Game::new(format!("xbox:{}", folder.to_lowercase()), name, Source::Xbox);
            g.exe = attr(&xml, "Executable", "Name").map(|e| dir.join("Content").join(e));
            g.install_dir = Some(dir);
            games.push(g);
        }
    }
    games
}
