//! Room for on-screen keyboards on Linux.
//!
//! Wayland never tells apps where the keyboard is. KWin, sway and phoc shrink
//! windowed/maximized windows for it, GNOME slides them up, but fullscreen
//! windows get covered. Then RISS keeps the bottom of the window free itself.
//!
//! A keyboard is expected when D-Bus says so (see [`dbus`]) or, with no
//! keyboard service around, when a text field is focused after touch input
//! (GNOME's own rule). Its height is measured whenever the compositor shrinks
//! a screen-sized window for it, and otherwise comes from settings.

use crate::settings::SettingsData;
use eframe::egui::{self, Vec2};
use std::sync::{Arc, Mutex};

#[cfg(all(target_os = "linux", feature = "osk-dbus"))]
mod dbus;

/// What the desktop reports about on-screen keyboards.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OskVisibility {
    /// No keyboard service could be asked.
    #[default]
    Unknown,
    Hidden,
    Visible,
}

impl OskVisibility {
    /// Combine reports of several services (`None` = not running).
    pub fn combine<I: IntoIterator<Item = Option<bool>>>(reports: I) -> Self {
        let reports: Vec<bool> = reports.into_iter().flatten().collect();
        if reports.contains(&true) {
            Self::Visible
        } else if reports.is_empty() {
            Self::Unknown
        } else {
            Self::Hidden
        }
    }

    fn expected(self, text_focused: bool, touch_input: bool) -> bool {
        match self {
            Self::Visible => true,
            Self::Hidden => false,
            Self::Unknown => text_focused && touch_input,
        }
    }
}

/// Whether to reserve space, per the `osk-space` setting.
fn needs_room(mode: &str, keyboard: bool, text_focused: bool, covers_monitor: bool) -> bool {
    match mode {
        "off" => false,
        "always" => keyboard || text_focused,
        _ => keyboard && covers_monitor,
    }
}

/// A window this size fills the monitor, allowing for panels along an edge.
fn covers_monitor(window: Vec2, monitor: Vec2) -> bool {
    monitor.min_elem() > 0.0 && window.x >= monitor.x * 0.9 && window.y >= monitor.y * 0.9
}

/// The window shrank by more than any panel would since typing started.
fn shrunk(start: Vec2, now: Vec2) -> bool {
    now.y < start.y * 0.9
}

/// Keyboard height in % of the monitor, from a screen-sized window that the
/// compositor shrank for it.
fn measure(start: Vec2, now: Vec2, monitor: Vec2) -> Option<usize> {
    (covers_monitor(start, monitor) && shrunk(start, now))
        .then(|| ((start.y - now.y) / monitor.y * 100.0).round() as usize)
}

fn height_key(size: Vec2) -> &'static str {
    if size.x > size.y {
        "osk-height-landscape"
    } else {
        "osk-height-portrait"
    }
}

pub struct OskTracker {
    visibility: Arc<Mutex<OskVisibility>>,
    touch_input: bool,
    /// Window size when the current typing session started.
    start: Option<Vec2>,
}

impl OskTracker {
    pub fn new(ctx: &egui::Context) -> Self {
        let visibility = Arc::default();
        #[cfg(all(target_os = "linux", feature = "osk-dbus"))]
        dbus::spawn_watcher(ctx.clone(), Arc::clone(&visibility));
        let _ = ctx;
        Self {
            visibility,
            touch_input: false,
            start: None,
        }
    }

    /// Height to keep free at the bottom this frame, in points. Returns
    /// `true` as well when a newly measured keyboard height was stored in
    /// `settings` (so the caller can save them).
    pub fn update(&mut self, ctx: &egui::Context, settings: &mut SettingsData) -> (f32, bool) {
        self.track_input_device(ctx);
        let screen = ctx.screen_rect().size();
        let focused = ctx.wants_keyboard_input();
        // A width change means rotation or a mode switch: start over.
        self.start = match self.start {
            Some(start) if focused && (start.x - screen.x).abs() < 1.0 => Some(start),
            _ => focused.then_some(screen),
        };
        let (fullscreen, monitor) = ctx.input(|i| {
            (
                i.viewport().fullscreen == Some(true),
                i.viewport().monitor_size,
            )
        });

        let mut stored = false;
        if let (Some(start), Some(monitor), false) = (self.start, monitor, fullscreen) {
            if let Some(percent) = measure(start, screen, monitor) {
                stored = store_height(settings, height_key(start), percent.clamp(10, 80));
            }
        }
        if self.start.is_some_and(|start| shrunk(start, screen)) {
            return (0.0, stored); // The compositor already made room.
        }

        let visibility = self.visibility.lock().map(|v| *v).unwrap_or_default();
        let keyboard = visibility.expected(focused, self.touch_input);
        let covers = fullscreen || monitor.is_some_and(|m| covers_monitor(screen, m));
        if !needs_room(settings.value("osk-space"), keyboard, focused, covers) {
            return (0.0, stored);
        }
        let percent = settings.number(height_key(screen), 40).min(80);
        ((screen.y * percent as f32 / 100.0).round(), stored)
    }

    /// Whether the latest pointer input was a touch: egui sends `Touch`
    /// events alongside the pointer events it emulates from them.
    fn track_input_device(&mut self, ctx: &egui::Context) {
        use egui::Event::{PointerButton, PointerMoved, Touch};
        let (touch, pointer) = ctx.input(|i| {
            let any = |f: fn(&egui::Event) -> bool| i.events.iter().any(f);
            (
                any(|e| matches!(e, Touch { .. })),
                any(|e| matches!(e, PointerMoved(_) | PointerButton { .. })),
            )
        });
        if touch || pointer {
            self.touch_input = touch;
        }
    }
}

/// Keep a measured height unless the user opted out; ignore jitter.
fn store_height(settings: &mut SettingsData, key: &str, percent: usize) -> bool {
    if !settings.enabled("osk-measure-height") || settings.number(key, 0).abs_diff(percent) < 2 {
        return false;
    }
    log::info!("Measured on-screen keyboard height: {percent}% ({key})");
    settings.values.insert(key.to_owned(), percent.to_string());
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const PHONE: Vec2 = egui::vec2(360.0, 800.0);

    #[test]
    fn combines_service_reports() {
        use OskVisibility::*;
        assert_eq!(OskVisibility::combine([None, None]), Unknown);
        assert_eq!(OskVisibility::combine([Some(false), None]), Hidden);
        assert_eq!(OskVisibility::combine([Some(false), Some(true)]), Visible);
    }

    #[test]
    fn touch_heuristic_only_without_services() {
        use OskVisibility::*;
        assert!(Unknown.expected(true, true));
        assert!(!Unknown.expected(true, false));
        assert!(!Hidden.expected(true, true));
        assert!(Visible.expected(false, false));
    }

    #[test]
    fn modes() {
        assert!(needs_room("auto", true, true, true));
        assert!(!needs_room("auto", true, true, false));
        assert!(needs_room("always", false, true, false));
        assert!(!needs_room("off", true, true, true));
    }

    #[test]
    fn measures_keyboard_from_compositor_shrink() {
        let typing = egui::vec2(360.0, 480.0);
        assert_eq!(measure(PHONE, typing, PHONE), Some(40));
        // Small shrinks and windows smaller than the monitor tell nothing.
        assert_eq!(measure(PHONE, egui::vec2(360.0, 780.0), PHONE), None);
        assert_eq!(measure(egui::vec2(300.0, 500.0), typing, PHONE), None);
    }

    #[test]
    fn stores_measurements_unless_disabled() {
        let mut settings = SettingsData::default();
        assert!(store_height(&mut settings, "osk-height-portrait", 33));
        assert_eq!(settings.value("osk-height-portrait"), "33");
        assert!(!store_height(&mut settings, "osk-height-portrait", 34));
        settings.bools.insert("osk-measure-height".into(), false);
        assert!(!store_height(&mut settings, "osk-height-portrait", 50));
    }
}
