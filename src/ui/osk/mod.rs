//! Room for the on-screen keyboard (OSK) on Linux desktops.
//!
//! Wayland's text-input protocol lets an app ask for the keyboard, but never
//! tells it where the keyboard is or how tall it is. Whether the search bar
//! stays visible therefore depends on the compositor:
//!
//! | Compositor               | What happens to the window when the keyboard opens       |
//! |--------------------------|----------------------------------------------------------|
//! | KWin                     | windowed/maximized: shrunk; fullscreen: overlapped       |
//! | sway, phoc (layer-shell) | tiled/maximized: shrunk; fullscreen/floating: overlapped |
//! | Mutter (GNOME Shell)     | slid upwards so the text cursor stays visible            |
//! | anything else / X11      | usually overlapped                                       |
//!
//! When the window is shrunk the normal layout already keeps the search bar
//! above the keyboard. For the overlapped cases RISS reserves the space
//! itself: while a keyboard is expected and the window covers the monitor,
//! the bottom of the window is kept free, sized from the `osk-height-*`
//! settings (the real height is not available).
//!
//! Whether a keyboard is expected comes from, in order of preference:
//! 1. the desktop over D-Bus (KWin's virtual keyboard, or any keyboard
//!    implementing `sm.puri.OSK0` such as squeekboard) — see [`dbus`];
//! 2. otherwise the same heuristic GNOME uses: a text field is focused and
//!    the last input came from a touchscreen.
//!
//! The `osk-space` setting picks `auto` (above), `always` (reserve whenever
//! a text field is focused, for setups nothing can detect) or `off`.

use crate::settings::SettingsData;
use eframe::egui;
use std::sync::{Arc, Mutex};

#[cfg(all(target_os = "linux", feature = "osk-dbus"))]
mod dbus;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OskSpaceMode {
    /// Reserve space when a keyboard is detected over a fullscreen window.
    Auto,
    /// Reserve space whenever a text field is focused.
    Always,
    /// Never reserve space.
    Off,
}

impl OskSpaceMode {
    /// Setting values, in the order shown in the settings screen.
    pub const KEYS: &'static [&'static str] = &["auto", "always", "off"];

    pub fn from_key(key: &str) -> Self {
        match key {
            "always" => Self::Always,
            "off" => Self::Off,
            _ => Self::Auto,
        }
    }
}

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
    /// Combine the reports of several keyboard services (`None` = service
    /// absent): any visible keyboard wins, then any hidden one.
    pub fn combine<I: IntoIterator<Item = Option<bool>>>(reports: I) -> Self {
        reports
            .into_iter()
            .flatten()
            .fold(Self::Unknown, |acc, visible| match (acc, visible) {
                (Self::Visible, _) | (_, true) => Self::Visible,
                _ => Self::Hidden,
            })
    }
}

/// Visibility shared with the background watcher.
type SharedVisibility = Arc<Mutex<OskVisibility>>;

/// Everything the reservation depends on, sampled once per frame.
#[derive(Clone, Copy, Debug)]
pub struct OskFrame {
    pub mode: OskSpaceMode,
    /// A text field has keyboard focus (so the keyboard was requested).
    pub text_focused: bool,
    pub visibility: OskVisibility,
    /// The most recent pointer input came from a touchscreen.
    pub touch_input: bool,
    /// The window is fullscreen or as large as the monitor, so a keyboard at
    /// the bottom of the screen covers its bottom edge.
    pub covers_monitor: bool,
    /// The window shrank after typing started: the compositor already made
    /// room for the keyboard and reserving more would leave a gap.
    pub compositor_made_room: bool,
    /// Window size in points.
    pub screen: egui::Vec2,
    /// Keyboard height as a share of the screen height.
    pub portrait_fraction: f32,
    pub landscape_fraction: f32,
}

impl OskFrame {
    /// Most a keyboard is assumed to cover, so the search bar always fits.
    const MAX_FRACTION: f32 = 0.8;

    /// Whether an on-screen keyboard is (probably) showing.
    pub fn keyboard_expected(&self) -> bool {
        match self.visibility {
            OskVisibility::Visible => true,
            OskVisibility::Hidden => false,
            OskVisibility::Unknown => self.text_focused && self.touch_input,
        }
    }

    /// Height to keep free at the bottom of the window, in points.
    pub fn reserved_height(&self) -> f32 {
        if self.compositor_made_room || self.screen.y <= 0.0 {
            return 0.0;
        }
        let reserve = match self.mode {
            OskSpaceMode::Off => false,
            OskSpaceMode::Always => self.text_focused || self.visibility == OskVisibility::Visible,
            OskSpaceMode::Auto => self.keyboard_expected() && self.covers_monitor,
        };
        if !reserve {
            return 0.0;
        }
        let fraction = if self.screen.x > self.screen.y {
            self.landscape_fraction
        } else {
            self.portrait_fraction
        };
        (self.screen.y * fraction.clamp(0.0, Self::MAX_FRACTION)).round()
    }
}

/// Whether a window of `screen` size fills a monitor of `monitor` size,
/// allowing for panels and bars along the edges.
pub fn covers_monitor(screen: egui::Vec2, monitor: egui::Vec2) -> bool {
    const MIN_SHARE: f32 = 0.9;
    monitor.x > 0.0
        && monitor.y > 0.0
        && screen.x >= monitor.x * MIN_SHARE
        && screen.y >= monitor.y * MIN_SHARE
}

pub struct OskTracker {
    visibility: SharedVisibility,
    touch_input: bool,
    /// Window size when the current typing session started.
    baseline: Option<egui::Vec2>,
}

impl OskTracker {
    /// The window counts as shrunk by the compositor below this share of its
    /// height at the start of typing. Keyboards take far more than 10%.
    const SHRINK_SHARE: f32 = 0.9;

    pub fn new(ctx: &egui::Context) -> Self {
        let visibility = SharedVisibility::default();
        #[cfg(all(target_os = "linux", feature = "osk-dbus"))]
        dbus::spawn_watcher(ctx.clone(), visibility.clone());
        #[cfg(not(all(target_os = "linux", feature = "osk-dbus")))]
        let _ = ctx;
        Self {
            visibility,
            touch_input: false,
            baseline: None,
        }
    }

    /// Height to keep free at the bottom of the window this frame, in points.
    pub fn reserved_height(&mut self, ctx: &egui::Context, settings: &SettingsData) -> f32 {
        self.track_input_device(ctx);
        let screen = ctx.screen_rect().size();
        let text_focused = ctx.wants_keyboard_input();

        // Remember the window size when typing starts, so a compositor that
        // shrinks the window for the keyboard is recognised. A width change
        // means rotation or a mode switch: start over from the new size.
        self.baseline = match self.baseline {
            Some(base) if text_focused && (base.x - screen.x).abs() < 1.0 => Some(base),
            _ if text_focused => Some(screen),
            _ => None,
        };
        let compositor_made_room = self
            .baseline
            .is_some_and(|base| screen.y < base.y * Self::SHRINK_SHARE);

        let (fullscreen, monitor) =
            ctx.input(|i| (i.viewport().fullscreen, i.viewport().monitor_size));
        let covers = fullscreen == Some(true)
            || monitor.is_some_and(|monitor| covers_monitor(screen, monitor));

        let frame = OskFrame {
            mode: OskSpaceMode::from_key(settings.value("osk-space")),
            text_focused,
            visibility: self.visibility.lock().map(|v| *v).unwrap_or_default(),
            touch_input: self.touch_input,
            covers_monitor: covers,
            compositor_made_room,
            screen,
            portrait_fraction: settings.number("osk-height-portrait", 40) as f32 / 100.0,
            landscape_fraction: settings.number("osk-height-landscape", 50) as f32 / 100.0,
        };
        frame.reserved_height()
    }

    /// Remember whether the latest pointer input was a touch. egui reports
    /// touches as `Touch` events alongside the emulated pointer events.
    fn track_input_device(&mut self, ctx: &egui::Context) {
        let (touch, pointer) = ctx.input(|i| {
            i.events
                .iter()
                .fold((false, false), |(touch, pointer), event| {
                    (
                        touch || matches!(event, egui::Event::Touch { .. }),
                        pointer
                            || matches!(
                                event,
                                egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. }
                            ),
                    )
                })
        });
        if touch {
            self.touch_input = true;
        } else if pointer {
            self.touch_input = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORTRAIT: egui::Vec2 = egui::vec2(360.0, 800.0);
    const LANDSCAPE: egui::Vec2 = egui::vec2(800.0, 360.0);

    /// A fullscreen portrait window with a focused search field and no
    /// D-Bus keyboard service.
    fn frame() -> OskFrame {
        OskFrame {
            mode: OskSpaceMode::Auto,
            text_focused: true,
            visibility: OskVisibility::Unknown,
            touch_input: false,
            covers_monitor: true,
            compositor_made_room: false,
            screen: PORTRAIT,
            portrait_fraction: 0.4,
            landscape_fraction: 0.5,
        }
    }

    #[test]
    fn combines_service_reports() {
        assert_eq!(
            OskVisibility::combine(Vec::<Option<bool>>::new()),
            OskVisibility::Unknown
        );
        assert_eq!(OskVisibility::combine([None, None]), OskVisibility::Unknown);
        assert_eq!(
            OskVisibility::combine([Some(false), None]),
            OskVisibility::Hidden
        );
        assert_eq!(
            OskVisibility::combine([Some(false), Some(true)]),
            OskVisibility::Visible
        );
        assert_eq!(
            OskVisibility::combine([Some(true), Some(false)]),
            OskVisibility::Visible
        );
    }

    #[test]
    fn reported_keyboard_reserves_space_over_fullscreen() {
        let visible = OskFrame {
            visibility: OskVisibility::Visible,
            ..frame()
        };
        assert_eq!(visible.reserved_height(), 320.0);
        let landscape = OskFrame {
            screen: LANDSCAPE,
            ..visible
        };
        assert_eq!(landscape.reserved_height(), 180.0);
    }

    #[test]
    fn hidden_keyboard_reserves_nothing() {
        let hidden = OskFrame {
            visibility: OskVisibility::Hidden,
            touch_input: true,
            ..frame()
        };
        assert_eq!(hidden.reserved_height(), 0.0);
    }

    #[test]
    fn unknown_keyboard_follows_touch_heuristic() {
        assert_eq!(frame().reserved_height(), 0.0);
        let touched = OskFrame {
            touch_input: true,
            ..frame()
        };
        assert_eq!(touched.reserved_height(), 320.0);
        let unfocused = OskFrame {
            text_focused: false,
            ..touched
        };
        assert_eq!(unfocused.reserved_height(), 0.0);
    }

    #[test]
    fn auto_leaves_resizable_windows_to_the_compositor() {
        let windowed = OskFrame {
            visibility: OskVisibility::Visible,
            covers_monitor: false,
            ..frame()
        };
        assert_eq!(windowed.reserved_height(), 0.0);
        let shrunk = OskFrame {
            visibility: OskVisibility::Visible,
            compositor_made_room: true,
            ..frame()
        };
        assert_eq!(shrunk.reserved_height(), 0.0);
    }

    #[test]
    fn always_and_off_modes() {
        let always = OskFrame {
            mode: OskSpaceMode::Always,
            covers_monitor: false,
            ..frame()
        };
        assert_eq!(always.reserved_height(), 320.0);
        let idle = OskFrame {
            text_focused: false,
            ..always
        };
        assert_eq!(idle.reserved_height(), 0.0);
        let off = OskFrame {
            mode: OskSpaceMode::Off,
            visibility: OskVisibility::Visible,
            ..frame()
        };
        assert_eq!(off.reserved_height(), 0.0);
    }

    #[test]
    fn keyboard_height_is_capped() {
        let huge = OskFrame {
            visibility: OskVisibility::Visible,
            portrait_fraction: 3.0,
            ..frame()
        };
        assert_eq!(huge.reserved_height(), 640.0);
    }

    #[test]
    fn monitor_coverage_allows_for_panels() {
        let monitor = egui::vec2(1920.0, 1080.0);
        assert!(covers_monitor(monitor, monitor));
        assert!(covers_monitor(egui::vec2(1920.0, 1040.0), monitor));
        assert!(!covers_monitor(egui::vec2(800.0, 600.0), monitor));
        assert!(!covers_monitor(monitor, egui::Vec2::ZERO));
    }

    #[test]
    fn parses_mode_keys() {
        assert_eq!(OskSpaceMode::from_key("always"), OskSpaceMode::Always);
        assert_eq!(OskSpaceMode::from_key("off"), OskSpaceMode::Off);
        assert_eq!(OskSpaceMode::from_key("auto"), OskSpaceMode::Auto);
        assert_eq!(OskSpaceMode::from_key(""), OskSpaceMode::Auto);
    }
}
