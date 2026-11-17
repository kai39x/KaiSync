//! "HDR follows the video": find the file the player has open and read whether it's HDR.

pub mod open_files;
pub mod probe;

use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

use serde::Serialize;

use self::probe::{HdrKind, Probe};

/// What the player is showing right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    /// File name (no folder).
    pub file: String,
    /// `Some(kind)` for an HDR video.
    pub hdr: Option<HdrKind>,
}

/// Re-checks only when the player's window title changes (new file), plus a slow refresh.
pub struct VideoWatch {
    pid: Option<u32>,
    title: Option<String>,
    checked: Instant,
    current: Option<NowPlaying>,
    cache: HashMap<PathBuf, Probe>,
}

const REFRESH: Duration = Duration::from_secs(10);

impl VideoWatch {
    pub fn new() -> Self {
        Self { pid: None, title: None, checked: Instant::now(), current: None, cache: HashMap::new() }
    }

    pub fn reset(&mut self) {
        self.pid = None;
        self.title = None;
        self.current = None;
    }

    pub fn check(&mut self, pid: u32) -> Option<NowPlaying> {
        let title = open_files::window_title(pid);
        let fresh = self.pid == Some(pid) && self.title == title && self.checked.elapsed() < REFRESH;
        if fresh {
            return self.current.clone();
        }
        self.pid = Some(pid);
        self.title = title;
        self.checked = Instant::now();

        let now = open_files::current_video(pid).map(|path| {
            let p = *self.cache.entry(path.clone()).or_insert_with(|| {
                let started = Instant::now();
                let r = probe::probe(&path);
                tracing::info!("video: {} -> {:?} [{} ms]", path.display(), r, started.elapsed().as_millis());
                r
            });
            NowPlaying {
                file: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                hdr: match p {
                    Probe::Hdr(k) => Some(k),
                    Probe::Sdr | Probe::Unknown => None,
                },
            }
        });
        if now != self.current {
            match &now {
                Some(n) => tracing::info!("video: playing {} ({})", n.file, n.hdr.map(|k| format!("{k:?}")).unwrap_or("SDR".into())),
                None => tracing::info!("video: no video file open in the player"),
            }
        }
        self.current = now.clone();
        if self.cache.len() > 200 {
            self.cache.clear();
        }
        now
    }
}
