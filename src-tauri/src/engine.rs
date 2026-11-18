//! The automation engine: one background thread that polls processes, debounces,
//! and applies the resulting mode. UI/tray talk to it through [`Cmd`]s.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        mpsc::{self, RecvTimeoutError, Sender},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant, SystemTime},
};

use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, Debouncer as FsDebouncer};
use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tauri::{AppHandle, Emitter};

use crate::{
    detector::{self, norm, Hit, Library},
    hdr::controller::{HdrController, HdrStatus},
    settings::Settings,
    state_machine::{Debouncer, Mode},
    video::{NowPlaying, VideoWatch},
    tray,
};

/// How often running processes are checked. One scan costs ~7 ms, so this is cheap;
/// it has to be fast because games read the HDR state once while they start.
const POLL_INTERVAL: Duration = Duration::from_millis(400);

pub enum Cmd {
    SetPaused(bool),
    TogglePause,
    /// Settings changed or the user pressed refresh: rescan libraries.
    Rescan,
    /// Something changed in a game folder: rescan only if games were added or removed.
    FoldersChanged,
    /// HDR test buttons: turn on/off now (no restore duty).
    HdrManual(bool, mpsc::Sender<Result<bool, String>>),
    /// The chosen HDR display changed: refresh the status.
    HdrRefresh,
    /// Apply Idle, then reply (normal Exit).
    Exit(mpsc::Sender<()>),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// The applied mode.
    pub mode: Mode,
    /// Game name / player name for the applied mode.
    pub detail: Option<String>,
    pub paused: bool,
    /// What is running right now (before debouncing).
    pub raw_mode: Mode,
    pub raw_detail: Option<String>,
    pub scanning: bool,
    /// The file the video player is showing (when "HDR follows the video" is on).
    pub video: Option<NowPlaying>,
    pub hdr: HdrStatus,
}

pub struct Engine {
    app: AppHandle,
    tx: Sender<Cmd>,
    pub status: Arc<Mutex<Status>>,
    pub library: Arc<RwLock<Library>>,
    pub settings: Arc<Mutex<Settings>>,
}

impl Engine {
    pub fn start(app: AppHandle, settings: Settings) -> Self {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(Status {
            mode: Mode::Idle,
            detail: None,
            paused: false,
            raw_mode: Mode::Idle,
            raw_detail: None,
            scanning: true,
            hdr: HdrStatus::default(),
            video: None,
        }));
        let library = Arc::new(RwLock::new(Library::default()));
        let settings = Arc::new(Mutex::new(settings));

        let mut worker = Worker {
            app: app.clone(),
            tx: tx.clone(),
            status: status.clone(),
            library: library.clone(),
            settings: settings.clone(),
            cache: HashMap::new(),
            debouncer: Debouncer::new(Duration::from_millis(3000)),
            fs_watch: None,
            hdr: HdrController::new(),
            early_hdr: None,
            video: VideoWatch::new(),
            folder_snapshot: Vec::new(),
            display_present: None,
        };
        std::thread::Builder::new()
            .name("kaisync-engine".into())
            .spawn(move || worker.run(rx))
            .expect("spawn engine thread");

        Self { app, tx, status, library, settings }
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }

    /// Mutate settings, persist them, tell every window, and optionally trigger a rescan.
    pub fn update_settings(&self, rescan: bool, f: impl FnOnce(&mut Settings)) -> Settings {
        let snapshot = {
            let mut s = self.settings.lock().unwrap();
            f(&mut s);
            s.save();
            s.clone()
        };
        // The tray-menu window has its own copy of the settings (language…).
        let _ = self.app.emit("settings-changed", &snapshot);
        if rescan {
            self.send(Cmd::Rescan);
        }
        snapshot
    }

    /// Apply Idle and wait (bounded) for it to finish.
    pub fn shutdown(&self) {
        let (tx, rx) = mpsc::channel();
        if self.tx.send(Cmd::Exit(tx)).is_ok() {
            let _ = rx.recv_timeout(Duration::from_secs(10));
        }
    }
}

struct Worker {
    app: AppHandle,
    tx: Sender<Cmd>,
    status: Arc<Mutex<Status>>,
    library: Arc<RwLock<Library>>,
    settings: Arc<Mutex<Settings>>,
    /// (pid, start time) -> classification. Cleared when the library changes.
    cache: HashMap<(Pid, u64), Option<Hit>>,
    debouncer: Debouncer,
    fs_watch: Option<FsDebouncer<notify_debouncer_mini::notify::RecommendedWatcher>>,
    hdr: HdrController,
    /// Set when HDR was turned on early for a game that isn't confirmed yet (debounce).
    /// Holds the last time that game was seen.
    early_hdr: Option<Instant>,
    video: VideoWatch,
    /// Direct subfolders of the game folders at the last scan.
    folder_snapshot: Vec<String>,
    /// Was an HDR display connected at the last check? (`None` = not checked yet)
    display_present: Option<bool>,
}

impl Worker {
    fn run(&mut self, rx: mpsc::Receiver<Cmd>) {
        let mut sys = match System::new() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("sysinfo unavailable: {e:?}; automation disabled");
                return;
            }
        };
        self.rescan();

        let poll = POLL_INTERVAL;
        let mut last_poll = Instant::now() - poll;
        let mut last_wall = SystemTime::now();
        let mut first = true;

        loop {
            let wait = poll.saturating_sub(last_poll.elapsed());
            match rx.recv_timeout(wait) {
                Ok(cmd) => {
                    if self.handle(cmd, &mut sys) {
                        return;
                    }
                    if last_poll.elapsed() < poll {
                        continue;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }

            // Sleep/hibernate shows up as a big wall-clock jump between polls.
            let now_wall = SystemTime::now();
            let gap = now_wall.duration_since(last_wall).unwrap_or_default();
            let woke = gap > poll + Duration::from_secs(20);
            if woke {
                tracing::info!("wake from sleep detected ({}s gap): re-checking now", gap.as_secs());
            }
            last_wall = now_wall;
            last_poll = Instant::now();

            let (raw, detail) = self.detect(&mut sys);
            self.step(raw, detail, first || woke);
            first = false;
            if self.refresh_hdr() {
                self.publish();
            }
            self.check_display_return();
        }
    }

    /// Returns true when the engine should stop.
    fn handle(&mut self, cmd: Cmd, sys: &mut System) -> bool {
        match cmd {
            Cmd::SetPaused(p) => self.set_paused(p, sys),
            Cmd::TogglePause => {
                let p = !self.status.lock().unwrap().paused;
                self.set_paused(p, sys);
            }
            Cmd::Rescan => self.rescan(),
            Cmd::FoldersChanged => {
                let now = folder_snapshot(&self.settings.lock().unwrap().game_folders);
                if now != self.folder_snapshot {
                    tracing::info!("game folder: games added or removed, rescanning");
                    self.rescan();
                }
            }
            Cmd::HdrManual(on, reply) => {
                let chosen = self.hdr_choice();
                let r = self.hdr.manual(chosen.as_deref(), on);
                let _ = reply.send(r);
                self.refresh_hdr();
                self.publish();
            }
            Cmd::HdrRefresh => {
                self.refresh_hdr();
                self.publish();
            }
            Cmd::Exit(reply) => {
                tracing::info!("exit: applying Idle");
                self.apply(Mode::Idle, None, "exit");
                let _ = reply.send(());
                return true;
            }
        }
        false
    }

    fn set_paused(&mut self, paused: bool, sys: &mut System) {
        {
            let mut st = self.status.lock().unwrap();
            if st.paused == paused {
                return;
            }
            st.paused = paused;
        }
        tracing::info!("automation {}", if paused { "paused" } else { "resumed" });
        if paused {
            self.publish();
        } else {
            let (raw, detail) = self.detect(sys);
            self.step(raw, detail, true);
        }
    }

    fn rescan(&mut self) {
        self.status.lock().unwrap().scanning = true;
        self.publish();
        let settings = self.settings.lock().unwrap().clone();
        self.debouncer.set_hold(Duration::from_millis(settings.debounce_ms.clamp(0, 30_000)));
        let lib = detector::build_library(&settings);
        *self.library.write().unwrap() = lib;
        self.cache.clear();
        self.watch_folders(&settings.game_folders);
        self.folder_snapshot = folder_snapshot(&settings.game_folders);
        self.status.lock().unwrap().scanning = false;
        self.publish();
        let _ = self.app.emit("library-changed", ());
    }

    /// Rescan automatically when something is added to / removed from a game folder.
    fn watch_folders(&mut self, folders: &[String]) {
        self.fs_watch = None;
        if folders.is_empty() {
            return;
        }
        let tx = self.tx.clone();
        let deb = new_debouncer(Duration::from_secs(3), move |res: notify_debouncer_mini::DebounceEventResult| {
            // Games write saves/logs into their own folders all the time; the engine only
            // rescans if the list of game subfolders actually changed.
            if res.is_ok() {
                let _ = tx.send(Cmd::FoldersChanged);
            }
        });
        match deb {
            Ok(mut d) => {
                for f in folders {
                    if let Err(e) = d.watcher().watch(Path::new(f), RecursiveMode::NonRecursive) {
                        tracing::warn!("cannot watch {f}: {e}");
                    }
                }
                self.fs_watch = Some(d);
            }
            Err(e) => tracing::warn!("folder watcher unavailable: {e}"),
        }
    }

    /// One process scan -> raw mode (Game > Video > Idle) + detail.
    fn detect(&mut self, sys: &mut System) -> (Mode, Option<String>) {
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        let library = self.library.clone();
        let lib = library.read().unwrap();
        let mut alive = HashSet::new();
        let mut games: Vec<String> = Vec::new();
        let mut player: Option<u32> = None;

        for (pid, proc_) in sys.processes() {
            let key = (*pid, proc_.start_time());
            alive.insert(key);
            let hit = self.cache.entry(key).or_insert_with(|| {
                let name = proc_.name().to_string_lossy().to_lowercase();
                let exe = proc_.exe().map(norm);
                let hit = lib.matcher.classify(&lib.games, exe.as_deref(), &name);
                if let Some(h) = &hit {
                    tracing::debug!("process {} ({}) -> {:?}", name, exe.as_deref().unwrap_or("<no path>"), h);
                }
                hit
            });
            match hit {
                Some(Hit::Game { name, .. }) => games.push(name.clone()),
                Some(Hit::Player) => player = Some(pid.as_u32()),
                None => {}
            }
        }
        self.cache.retain(|k, _| alive.contains(k));

        let player_name = lib.player.as_ref().map(|p| p.name.clone());
        drop(lib);

        // What the player shows (only looked up when "HDR follows the video" is on).
        let match_hdr = self.settings.lock().unwrap().video_match_hdr;
        let playing = match player {
            Some(pid) if match_hdr => self.video.check(pid),
            _ => {
                self.video.reset();
                None
            }
        };
        self.status.lock().unwrap().video = playing.clone();

        if !games.is_empty() {
            // Keep the current name if that game is still running (no flapping between two).
            let cur = self.status.lock().unwrap().raw_detail.clone();
            games.sort();
            let name = cur.filter(|c| games.contains(c)).unwrap_or_else(|| games[0].clone());
            return (Mode::Game, Some(name));
        }
        if player.is_some() {
            return match playing {
                Some(np) if np.hdr.is_some() => (Mode::VideoHdr, Some(np.file)),
                Some(np) => (Mode::Video, Some(np.file)),
                None => (Mode::Video, player_name),
            };
        }
        (Mode::Idle, None)
    }

    fn step(&mut self, raw: Mode, detail: Option<String>, immediate: bool) {
        let paused;
        let raw_changed;
        {
            let mut st = self.status.lock().unwrap();
            paused = st.paused;
            raw_changed = st.raw_mode != raw || st.raw_detail != detail;
            if raw_changed {
                tracing::info!("detected: {:?} {}", raw, detail.as_deref().unwrap_or(""));
            }
            st.raw_mode = raw;
            st.raw_detail = detail.clone();
        }
        if paused {
            if raw_changed {
                self.publish();
            }
            return;
        }

        let now = Instant::now();

        // Games (and video renderers) read the display's HDR state once while starting, so
        // turning HDR on can't wait for the debounce: do it the moment it's needed.
        let wants_on = matches!(raw, Mode::Game | Mode::VideoHdr);
        let applied_on = matches!(self.debouncer.applied(), Some(Mode::Game | Mode::VideoHdr));
        if wants_on && !immediate && !applied_on {
            if self.early_hdr.is_none() {
                tracing::info!("{:?} seen: turning HDR on now, before the {}s confirmation", raw, self.debouncer.hold().as_secs_f32());
                let chosen = self.hdr_choice();
                self.hdr.ensure_on(chosen.as_deref());
                self.refresh_hdr();
                self.publish();
            }
            self.early_hdr = Some(now);
        }

        let apply = if immediate {
            Some(self.debouncer.force(raw, now))
        } else {
            self.debouncer.observe(raw, now)
        };

        if let Some(mode) = apply {
            self.apply(mode, detail, if immediate { "immediate" } else { "debounced" });
        } else {
            // Same mode, different game (e.g. one game closed, another still running).
            let mut st = self.status.lock().unwrap();
            if self.debouncer.applied() == Some(raw) && st.detail != detail {
                st.detail = detail;
                drop(st);
                self.publish();
            } else if raw_changed {
                drop(st);
                self.publish();
            }
        }

        // It vanished before it was confirmed (splash, quick restart): back to the mode's HDR.
        if let Some(seen) = self.early_hdr {
            if !wants_on && now.duration_since(seen) >= self.debouncer.hold() {
                tracing::info!("gone before confirmation: back to the current mode's HDR");
                self.early_hdr = None;
                let mode = self.debouncer.applied().unwrap_or(Mode::Idle);
                self.hdr_for(mode);
                self.refresh_hdr();
                self.publish();
            }
        }
    }

    /// The HDR state each mode wants.
    fn hdr_for(&mut self, mode: Mode) {
        let chosen = self.hdr_choice();
        match mode {
            Mode::Game | Mode::VideoHdr => self.hdr.ensure_on(chosen.as_deref()),
            // Watching SDR: HDR off (back on afterwards if KaiSync was the one who turned it off).
            Mode::Video => self.hdr.ensure_off(chosen.as_deref()),
            // Nothing running: undo whatever KaiSync changed.
            Mode::Idle => self.hdr.restore(chosen.as_deref()),
        }
    }

    /// The HDR display came back (TV switched on, cable re-plugged): Windows may have reset
    /// its HDR state, so apply what the current mode wants again.
    fn check_display_return(&mut self) {
        let (present, paused) = {
            let st = self.status.lock().unwrap();
            (st.hdr.display.is_some(), st.paused)
        };
        let returned = self.display_present == Some(false) && present;
        self.display_present = Some(present);
        if returned && !paused {
            if let Some(mode) = self.debouncer.applied() {
                tracing::info!("hdr: display is back, re-applying {:?}", mode);
                self.hdr_for(mode);
                self.refresh_hdr();
                self.publish();
            }
        }
    }

    fn hdr_choice(&self) -> Option<String> {
        self.settings.lock().unwrap().hdr_display.clone()
    }

    /// Re-read the HDR state of the chosen display. Returns true if it changed.
    fn refresh_hdr(&mut self) -> bool {
        let s = self.hdr.status(self.hdr_choice().as_deref());
        let mut st = self.status.lock().unwrap();
        if st.hdr != s {
            st.hdr = s;
            true
        } else {
            false
        }
    }

    /// Apply a mode: HDR on for games, otherwise restore it (only if KaiSync turned it on).
    fn apply(&mut self, mode: Mode, detail: Option<String>, why: &str) {
        tracing::info!("APPLY {:?} {} [{}]", mode, detail.as_deref().unwrap_or(""), why);
        self.early_hdr = None;
        self.hdr_for(mode);
        self.refresh_hdr();
        {
            let mut st = self.status.lock().unwrap();
            st.mode = mode;
            st.detail = detail;
        }
        self.publish();
    }

    fn publish(&self) {
        let st = self.status.lock().unwrap().clone();
        tray::update(&self.app, &st);
        let _ = self.app.emit("status-changed", &st);
    }
}

/// Sorted list of the direct subfolders of every game folder (cheap: one read_dir each).
fn folder_snapshot(folders: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for f in folders {
        if let Ok(rd) = std::fs::read_dir(f) {
            for e in rd.flatten() {
                if e.file_type().is_ok_and(|t| t.is_dir()) {
                    out.push(norm(&e.path()));
                }
            }
        }
    }
    out.sort();
    out
}
