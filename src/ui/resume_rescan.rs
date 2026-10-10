//! Coalesced app-list refresh when the launcher returns from the background
//! (issue #20).
//!
//! The KISS-correct mechanism is `LauncherApps.Callback` or a
//! `BroadcastReceiver` for `PACKAGE_ADDED`/`REMOVED`/`REPLACED`, and the
//! module that would own it already says so in `ui/mod.rs`. That route is
//! closed *here*, today: `android/` ships a manifest and no Kotlin/Java
//! host, and a broadcast receiver is a Java class — it cannot be conjured
//! from the Rust side of JNI. What every winit-android app *does* get for
//! free is window focus following the activity: home/gesture away drops it,
//! returning restores it. A resume is therefore the cheapest honest proxy
//! for "the installed set may have changed while we were away".
//!
//! The rescan is deliberately *one* `reload_apps()` per real resume:
//!   * it only fires on the rising edge of focus, so a steady foreground
//!     never rescans;
//!   * the launcher must have been unfocused for at least
//!     [`BACKGROUND_THRESHOLD`] — keyboard doodles, notification-shade
//!     pulls and momentary dialog blips do not rescan;
//!   * a [`RESCAN_COOLDOWN`] absorbs screen-off storms so a flapping
//!     lockscreen cannot rescan twice.
//!
//! Whatever changed between the last rescan and this resume is caught by
//! that single pass; scans are not queued.
//!
//! Verification: the state machine below is unit-tested on the host. The
//! winit-focus behaviour itself needs a device run (install/remove an app
//! while the launcher is backgrounded, return, list updates without a
//! relaunch) — flagged as such on the PR; this sandbox has no emulator.

use std::time::Duration;

/// How long the launcher must have been unfocused before a return counts
/// as a resume worth a rescan. Shorter absences are treated as blips.
pub(crate) const BACKGROUND_THRESHOLD: Duration = Duration::from_secs(10);

/// Minimum time between two rescans; a second resume inside the window is
/// a no-op because the first scan's listing already covers it.
pub(crate) const RESCAN_COOLDOWN: Duration = Duration::from_secs(60);

/// Watches the egui focused flag and decides when a resume deserves an
/// app-list rescan. Pure state machine: time is handed in, not read, so
/// the behaviour is testable without a clock or a compositor.
pub(crate) struct ResumeRescan {
    was_focused: bool,
    /// When the current unfocused stretch began.
    unfocused_since: Option<Duration>,
    /// When the last rescan fired.
    last_fired: Option<Duration>,
}

impl ResumeRescan {
    /// `initially_focused` should be true for a launcher that starts in
    /// the foreground: the startup scan already ran, so the first frame
    /// must not count as a resume.
    pub(crate) fn new(initially_focused: bool) -> Self {
        Self {
            was_focused: initially_focused,
            unfocused_since: None,
            last_fired: None,
        }
    }

    /// Observe the window's focused state at time `now`; returns true
    /// exactly once per real resume — the moment a single rescan should
    /// run.
    pub(crate) fn observe(&mut self, focused: bool, now: Duration) -> bool {
        if !focused {
            if self.was_focused {
                self.unfocused_since.get_or_insert(now);
            }
            self.was_focused = false;
            return false;
        }
        let away_since = self.unfocused_since.take();
        let rising = !self.was_focused;
        self.was_focused = true;
        if !rising {
            return false;
        }
        // A rising edge with no recorded absence (or a short one) is a
        // blip, not a resume.
        let Some(since) = away_since else {
            return false;
        };
        if now.saturating_sub(since) < BACKGROUND_THRESHOLD {
            return false;
        }
        if self
            .last_fired
            .is_some_and(|t| now.saturating_sub(t) < RESCAN_COOLDOWN)
        {
            return false;
        }
        self.last_fired = Some(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    #[test]
    fn steady_foreground_never_rescans() {
        let mut w = ResumeRescan::new(true);
        for i in 0..1000 {
            assert!(!w.observe(true, t(i)));
        }
    }

    #[test]
    fn short_blip_does_not_rescan() {
        let mut w = ResumeRescan::new(true);
        assert!(!w.observe(false, t(100)));
        assert!(!w.observe(true, t(102))); // 2s away: notification shade, not a resume
        for i in 0..50 {
            assert!(!w.observe(true, t(102 + i)));
        }
    }

    #[test]
    fn long_background_fires_once() {
        let mut w = ResumeRescan::new(true);
        assert!(!w.observe(false, t(100)));
        assert!(!w.observe(false, t(130)));
        assert!(w.observe(true, t(140))); // 40s away: a real resume
                                          // And the very same resume never fires twice.
        assert!(!w.observe(true, t(141)));
        assert!(!w.observe(true, t(142)));
    }

    #[test]
    fn threshold_is_an_at_least_boundary() {
        // One second short of the threshold: a blip, no rescan.
        let mut w = ResumeRescan::new(true);
        assert!(!w.observe(false, t(0)));
        assert!(!w.observe(true, BACKGROUND_THRESHOLD - Duration::from_secs(1)));
        // Exactly the threshold: away long enough, rescan.
        let mut w = ResumeRescan::new(true);
        assert!(!w.observe(false, t(0)));
        assert!(w.observe(true, BACKGROUND_THRESHOLD));
    }

    #[test]
    fn cooldown_coalesces_a_flapping_screen() {
        let mut w = ResumeRescan::new(true);
        assert!(!w.observe(false, t(0)));
        assert!(w.observe(true, t(30))); // first resume fires
        assert!(!w.observe(false, t(40)));
        assert!(!w.observe(true, t(80))); // second long resume, still in cooldown
    }

    #[test]
    fn resumes_after_cooldown_fire_again() {
        let mut w = ResumeRescan::new(true);
        assert!(!w.observe(false, t(0)));
        assert!(w.observe(true, t(30)));
        assert!(!w.observe(false, t(50)));
        assert!(w.observe(true, t(30) + RESCAN_COOLDOWN + t(30)));
    }

    #[test]
    fn away_time_is_measured_from_the_first_unfocused_observation() {
        let mut w = ResumeRescan::new(true);
        // Focus flutters off, back below threshold, then off again: the
        // short first absence must not accumulate toward the threshold.
        assert!(!w.observe(false, t(0)));
        assert!(!w.observe(true, t(2)));
        assert!(!w.observe(false, t(3)));
        assert!(!w.observe(true, t(12))); // 9s into the second absence: still no
        assert!(!w.observe(false, t(13)));
        assert!(w.observe(true, t(30)));
    }
}
