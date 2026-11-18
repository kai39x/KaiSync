//! Game + video-player detection.
//!
//! `build_library` scans every enabled source and produces a [`Library`]: the list of
//! known games (for the UI) plus a [`Matcher`] that classifies a running process.

pub mod epic;
pub mod folders;
pub mod ignore;
pub mod player;
pub mod steam;
pub mod xbox;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Instant,
};

use serde::Serialize;

use crate::settings::Settings;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Steam,
    Epic,
    Xbox,
    Folder,
    Manual,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    /// Stable id: `steam:<appid>`, `epic:<AppName>`, `xbox:<folder>`, `folder:<path>`, `manual:<path>`.
    pub id: String,
    pub name: String,
    pub source: Source,
    /// Any exe running inside this folder counts as the game (Steam / Epic / Xbox).
    pub install_dir: Option<PathBuf>,
    /// The main exe (Epic launch exe, chosen folder exe, manual exe).
    pub exe: Option<PathBuf>,
    /// Folder games only: exe candidates, best first.
    pub candidates: Vec<PathBuf>,
    /// Folder game with several plausible exes and no pick yet.
    pub needs_pick: bool,
    /// Default before user overrides (some always-running Steam apps default to off).
    pub default_enabled: bool,
    /// Resolved: default + user override.
    pub enabled: bool,
    /// Folder games: which configured game folder this came from.
    pub folder_root: Option<String>,
}

impl Game {
    pub fn new(id: String, name: String, source: Source) -> Self {
        Self {
            id,
            name,
            source,
            install_dir: None,
            exe: None,
            candidates: Vec::new(),
            needs_pick: false,
            default_enabled: true,
            enabled: true,
            folder_root: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderReport {
    pub path: String,
    pub exists: bool,
    pub games: usize,
    pub needs_pick: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerInfo {
    pub name: String,
    pub path: String,
    pub exists: bool,
}

/// What a process was classified as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hit {
    Game { id: String, name: String },
    Player,
}

#[derive(Debug, Default)]
pub struct Matcher {
    /// (normalized dir with trailing `\`, game index)
    dir_rules: Vec<(String, usize)>,
    /// normalized full exe path -> game index
    exe_rules: HashMap<String, usize>,
    /// lowercase exe file name -> game index (fallback when the process path is unreadable)
    name_rules: HashMap<String, usize>,
    player_path: Option<String>,
    player_name: Option<String>,
}

#[derive(Debug, Default)]
pub struct Library {
    pub games: Vec<Game>,
    pub folders: Vec<FolderReport>,
    pub player: Option<PlayerInfo>,
    pub matcher: Matcher,
}

/// Lowercase, backslashes, no trailing separator.
pub fn norm(p: &Path) -> String {
    let mut s = p.to_string_lossy().replace('/', "\\").to_lowercase();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        s = stripped.to_string();
    }
    while s.ends_with('\\') && s.len() > 3 {
        s.pop();
    }
    s
}

fn file_name_lower(p: &Path) -> Option<String> {
    p.file_name().map(|n| n.to_string_lossy().to_lowercase())
}

impl Matcher {
    fn build(games: &[Game], player: &Path) -> Self {
        let mut m = Matcher::default();
        for (i, g) in games.iter().enumerate() {
            if !g.enabled || g.needs_pick {
                continue;
            }
            if let Some(dir) = &g.install_dir {
                m.dir_rules.push((format!("{}\\", norm(dir)), i));
            }
            if let Some(exe) = &g.exe {
                m.exe_rules.insert(norm(exe), i);
                // The name fallback is only safe for exes we pinned explicitly.
                if matches!(g.source, Source::Folder | Source::Manual) {
                    if let Some(n) = file_name_lower(exe) {
                        m.name_rules.entry(n).or_insert(i);
                    }
                }
            }
        }
        // Longest dir first so nested installs resolve to the most specific game.
        m.dir_rules.sort_by_key(|r| std::cmp::Reverse(r.0.len()));
        // Full path when we know it; a bare exe name matches by name only.
        if player.is_absolute() {
            m.player_path = Some(norm(player));
        }
        m.player_name = file_name_lower(player);
        m
    }

    /// `exe` = normalized full path if readable; `name` = lowercase process name.
    pub fn classify(&self, games: &[Game], exe: Option<&str>, name: &str) -> Option<Hit> {
        if ignore::is_ignored(name) {
            return None;
        }
        let game = |i: usize| {
            let g = &games[i];
            Some(Hit::Game { id: g.id.clone(), name: g.name.clone() })
        };
        match exe {
            Some(exe) => {
                if self.player_path.as_deref() == Some(exe)
                    || (self.player_path.is_none() && self.player_name.as_deref() == Some(name))
                {
                    return Some(Hit::Player);
                }
                if let Some(&i) = self.exe_rules.get(exe) {
                    return game(i);
                }
                if let Some((_, i)) = self.dir_rules.iter().find(|(d, _)| exe.starts_with(d.as_str())) {
                    return game(*i);
                }
                None
            }
            None => {
                // Path unreadable: match the player by its exe name.
                if self.player_name.as_deref() == Some(name) {
                    return Some(Hit::Player);
                }
                self.name_rules.get(name).and_then(|&i| game(i))
            }
        }
    }
}

/// Scan all enabled sources. Slow-ish (disk IO); call from the engine thread.
pub fn build_library(settings: &Settings) -> Library {
    let started = Instant::now();
    let mut games = Vec::new();

    if settings.sources.steam {
        games.extend(steam::scan());
    }
    if settings.sources.epic {
        games.extend(epic::scan());
    }
    if settings.sources.xbox {
        games.extend(xbox::scan());
    }

    let mut folders = Vec::new();
    for root in &settings.game_folders {
        let (found, report) = folders::scan_root(Path::new(root), &settings.folder_picks);
        games.extend(found);
        folders.push(report);
    }

    for mg in &settings.manual_games {
        let path = PathBuf::from(&mg.path);
        let mut g = Game::new(format!("manual:{}", norm(&path)), mg.name.clone(), Source::Manual);
        g.exe = Some(path);
        games.push(g);
    }

    // Same game reachable twice (e.g. a folder that is also a Steam library): keep the first.
    let mut seen = std::collections::HashSet::new();
    games.retain(|g| seen.insert(g.id.clone()));

    for g in &mut games {
        g.enabled = settings.game_enabled.get(&g.id).copied().unwrap_or(g.default_enabled);
    }
    games.sort_by_key(|a| a.name.to_lowercase());

    let player_path = player::resolve(settings);
    let player = Some(PlayerInfo {
        name: player::display_name(&player_path),
        path: player_path.to_string_lossy().into_owned(),
        exists: player_path.exists(),
    });
    let matcher = Matcher::build(&games, &player_path);

    let count = |s: Source| games.iter().filter(|g| g.source == s).count();
    tracing::info!(
        "library: {} games (steam {}, epic {}, xbox {}, folder {}, manual {}), {} need a pick, player = {:?} [{} ms]",
        games.len(),
        count(Source::Steam),
        count(Source::Epic),
        count(Source::Xbox),
        count(Source::Folder),
        count(Source::Manual),
        games.iter().filter(|g| g.needs_pick).count(),
        player.as_ref().map(|p| &p.path),
        started.elapsed().as_millis()
    );
    for g in &games {
        tracing::debug!(
            "  [{}] {} {:?} dir={:?} exe={:?}{}",
            if g.enabled { "on " } else { "off" },
            g.name,
            g.source,
            g.install_dir,
            g.exe,
            if g.needs_pick { format!(" NEEDS PICK ({} candidates)", g.candidates.len()) } else { String::new() }
        );
    }

    Library { games, folders, player, matcher }
}
