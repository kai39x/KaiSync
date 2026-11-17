//! Modes (Game > Video > Idle) and debouncing. Video is split by the file's HDR.

use std::time::{Duration, Instant};

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Idle,
    /// The player is open with an SDR video (or nothing loaded): HDR off.
    Video,
    /// The player is showing an HDR video: HDR on.
    #[serde(rename = "videoHdr")]
    VideoHdr,
    Game,
}

/// A raw state change must hold for `hold` before it is applied, so launchers,
/// splash screens and quick restarts don't make the state flicker.
#[derive(Debug)]
pub struct Debouncer {
    applied: Option<Mode>,
    pending: Mode,
    since: Instant,
    hold: Duration,
}

impl Debouncer {
    pub fn new(hold: Duration) -> Self {
        Self { applied: None, pending: Mode::Idle, since: Instant::now(), hold }
    }

    pub fn set_hold(&mut self, hold: Duration) {
        self.hold = hold;
    }

    pub fn hold(&self) -> Duration {
        self.hold
    }

    pub fn applied(&self) -> Option<Mode> {
        self.applied
    }

    /// Feed the latest raw mode. Returns `Some(mode)` when `mode` should be applied now.
    pub fn observe(&mut self, raw: Mode, now: Instant) -> Option<Mode> {
        if raw != self.pending {
            self.pending = raw;
            self.since = now;
        }
        if self.applied != Some(raw) && now.duration_since(self.since) >= self.hold {
            self.applied = Some(raw);
            return Some(raw);
        }
        None
    }

    /// Apply `raw` immediately (startup, resume, wake from sleep).
    pub fn force(&mut self, raw: Mode, now: Instant) -> Mode {
        self.pending = raw;
        self.since = now;
        self.applied = Some(raw);
        raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debounce_holds_and_ignores_blips() {
        let t0 = Instant::now();
        let s = |ms| t0 + Duration::from_millis(ms);
        let mut d = Debouncer::new(Duration::from_secs(3));
        d.force(Mode::Idle, t0);

        // Splash screen for 2 s, then gone: nothing applied.
        assert_eq!(d.observe(Mode::Game, s(0)), None);
        assert_eq!(d.observe(Mode::Game, s(1500)), None);
        assert_eq!(d.observe(Mode::Idle, s(2000)), None);
        assert_eq!(d.observe(Mode::Idle, s(6000)), None);

        // Real game: applied after 3 s.
        assert_eq!(d.observe(Mode::Game, s(7000)), None);
        assert_eq!(d.observe(Mode::Game, s(8500)), None);
        assert_eq!(d.observe(Mode::Game, s(10000)), Some(Mode::Game));
        assert_eq!(d.observe(Mode::Game, s(11500)), None);

        // Game closes, player still open -> Video after the hold.
        assert_eq!(d.observe(Mode::Video, s(12000)), None);
        assert_eq!(d.observe(Mode::Video, s(15000)), Some(Mode::Video));
    }
}
