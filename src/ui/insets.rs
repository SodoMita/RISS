//! Keeps the UI inside the part of the window that is actually visible.
//!
//! On Android the app renders through a `NativeActivity`, whose surface always
//! covers the whole window. With `windowSoftInputMode="adjustResize"` Android
//! shrinks the activity's *content rect* when the on-screen keyboard opens, but
//! the surface (and therefore egui's screen rect) keeps its full size, and
//! winit 0.30 drops the `ContentRectChanged` event on the floor. Without help,
//! everything laid out at the bottom of the screen — the search bar — ends up
//! underneath the keyboard.
//!
//! [`InsetTracker`] reads the content rect directly, converts it into insets
//! (the parts of the window covered by the keyboard or system bars), and asks
//! egui to repaint whenever it changes, since nothing else would wake the UI
//! when the keyboard is shown or dismissed (e.g. with the back button).

use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

/// Window size and visible content rect, in physical pixels and window
/// coordinates, as reported by the platform.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowGeometry {
    pub width: i32,
    pub height: i32,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Space on each window edge that is covered by system UI or the on-screen
/// keyboard and must not hold content.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenInsets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl ScreenInsets {
    /// Smallest share of each window dimension a content rect must keep to be
    /// trusted. Protects the layout from nonsense rects.
    const MIN_VISIBLE_FRACTION: i32 = 5;

    /// Derive insets (in physical pixels) from the window geometry.
    ///
    /// Returns `None` when the content rect is unknown or does not fit the
    /// window. That happens before Android reports the first rect (it starts
    /// out empty) and briefly during rotation, when the window has already
    /// changed orientation but the content rect has not caught up yet; using
    /// the stale rect then would push the UI off screen for a frame or two.
    pub fn from_geometry(g: WindowGeometry) -> Option<Self> {
        if g.width <= 0 || g.height <= 0 {
            return None;
        }
        let visible_w = g.right - g.left;
        let visible_h = g.bottom - g.top;
        if visible_w <= 0 || visible_h <= 0 {
            return None;
        }
        if g.left < 0 || g.top < 0 || g.right > g.width || g.bottom > g.height {
            return None;
        }
        if visible_w * Self::MIN_VISIBLE_FRACTION < g.width
            || visible_h * Self::MIN_VISIBLE_FRACTION < g.height
        {
            return None;
        }
        Some(Self {
            left: g.left as f32,
            top: g.top as f32,
            right: (g.width - g.right) as f32,
            bottom: (g.height - g.bottom) as f32,
        })
    }

    /// Convert physical pixels to egui points.
    pub fn to_points(self, pixels_per_point: f32) -> Self {
        if pixels_per_point <= 0.0 {
            return Self::default();
        }
        Self {
            left: self.left / pixels_per_point,
            top: self.top / pixels_per_point,
            right: self.right / pixels_per_point,
            bottom: self.bottom / pixels_per_point,
        }
    }

    /// Whether these insets (in points) leave a usable area on a screen of
    /// the given size.
    pub fn fits(&self, screen: egui::Vec2) -> bool {
        let min = 1.0 / Self::MIN_VISIBLE_FRACTION as f32;
        screen.x - self.left - self.right >= screen.x * min
            && screen.y - self.top - self.bottom >= screen.y * min
    }

    /// These insets with at least `bottom` points kept free at the bottom.
    pub fn with_min_bottom(self, bottom: f32) -> Self {
        Self {
            bottom: self.bottom.max(bottom),
            ..self
        }
    }

    pub fn is_zero(&self) -> bool {
        *self == Self::default()
    }

    /// Reserve the covered edges with empty panels so every panel shown
    /// afterwards (search bar, results, settings) is laid out inside the
    /// visible area. Must run before any other panel of the frame.
    pub fn reserve(&self, ctx: &egui::Context, fill: egui::Color32) {
        const MIN: f32 = 0.5;
        let frame = egui::Frame::NONE.fill(fill);
        if self.bottom >= MIN {
            egui::TopBottomPanel::bottom("system-inset-bottom")
                .exact_height(self.bottom)
                .resizable(false)
                .show_separator_line(false)
                .frame(frame)
                .show(ctx, |_| {});
        }
        if self.top >= MIN {
            egui::TopBottomPanel::top("system-inset-top")
                .exact_height(self.top)
                .resizable(false)
                .show_separator_line(false)
                .frame(frame)
                .show(ctx, |_| {});
        }
        if self.left >= MIN {
            egui::SidePanel::left("system-inset-left")
                .exact_width(self.left)
                .resizable(false)
                .show_separator_line(false)
                .frame(frame)
                .show(ctx, |_| {});
        }
        if self.right >= MIN {
            egui::SidePanel::right("system-inset-right")
                .exact_width(self.right)
                .resizable(false)
                .show_separator_line(false)
                .frame(frame)
                .show(ctx, |_| {});
        }
    }
}

/// Reads the current window geometry from the platform.
pub type GeometryProbe = Arc<dyn Fn() -> Option<WindowGeometry> + Send + Sync>;

#[cfg(target_os = "android")]
fn platform_probe() -> Option<GeometryProbe> {
    crate::android_app_entry::android_jni::window_geometry_probe()
        .map(|probe| Arc::new(probe) as GeometryProbe)
}

#[cfg(not(target_os = "android"))]
fn platform_probe() -> Option<GeometryProbe> {
    // Desktop windows are resized by the window manager and egui already
    // sees the real size, so there is nothing to compensate for.
    None
}

/// State shared with the watcher thread. The thread holds only a `Weak`
/// reference and exits once the tracker (and thus the app) is dropped.
struct WatchState {
    /// The keyboard currently covers part of the window; it can be dismissed
    /// at any moment (the back button closes it without telling egui).
    keyboard_open: AtomicBool,
    /// Poll quickly until this moment: a text field just gained focus, so the
    /// keyboard is about to open.
    fast_until: Mutex<Option<Instant>>,
}

impl WatchState {
    fn poll_fast(&self) -> bool {
        if self.keyboard_open.load(Ordering::Relaxed) {
            return true;
        }
        let until = self.fast_until.lock().ok().and_then(|until| *until);
        until.is_some_and(|until| Instant::now() < until)
    }
}

pub struct InsetTracker {
    probe: Option<GeometryProbe>,
    state: Arc<WatchState>,
    /// Last insets that were applied, in points.
    current: ScreenInsets,
    had_keyboard_focus: bool,
}

impl InsetTracker {
    const FAST_POLL: Duration = Duration::from_millis(80);
    const IDLE_POLL: Duration = Duration::from_millis(500);
    /// How long to watch closely after a text field gains focus. Covers the
    /// keyboard's slide-in animation with plenty of margin.
    const FOCUS_WATCH: Duration = Duration::from_secs(2);
    /// A bottom inset taller than this share of the screen is the keyboard,
    /// not just the navigation bar.
    const KEYBOARD_MIN_FRACTION: f32 = 0.15;

    pub fn new(ctx: &egui::Context) -> Self {
        let tracker = Self {
            probe: platform_probe(),
            state: Arc::new(WatchState {
                keyboard_open: AtomicBool::new(false),
                fast_until: Mutex::new(None),
            }),
            current: ScreenInsets::default(),
            had_keyboard_focus: false,
        };
        if let Some(probe) = &tracker.probe {
            Self::spawn_watcher(ctx.clone(), probe.clone(), Arc::downgrade(&tracker.state));
        }
        tracker
    }

    /// Wake the UI whenever the window geometry changes. Android does not
    /// deliver content rect changes to egui, so without this the layout would
    /// only catch up with the keyboard on the next touch.
    fn spawn_watcher(ctx: egui::Context, probe: GeometryProbe, state: Weak<WatchState>) {
        let spawned = std::thread::Builder::new()
            .name("riss-insets".into())
            .spawn(move || {
                let mut last = probe();
                loop {
                    let interval = match state.upgrade() {
                        Some(state) if state.poll_fast() => Self::FAST_POLL,
                        Some(_) => Self::IDLE_POLL,
                        None => break,
                    };
                    std::thread::sleep(interval);
                    let now = probe();
                    if now != last {
                        last = now;
                        ctx.request_repaint();
                    }
                }
            });
        if let Err(err) = spawned {
            log::error!("Could not start the window inset watcher: {err}");
        }
    }

    /// Insets for this frame, in egui points.
    pub fn update(&mut self, ctx: &egui::Context) -> ScreenInsets {
        let Some(probe) = &self.probe else {
            return ScreenInsets::default();
        };
        let screen = ctx.screen_rect();
        match probe().and_then(ScreenInsets::from_geometry) {
            Some(insets) => self.current = insets.to_points(ctx.pixels_per_point()),
            // The rect is unknown or stale (mid-rotation). Keep the last good
            // insets unless they no longer fit the screen, so the layout does
            // not flicker while Android catches up.
            None if !self.current.fits(screen.size()) => self.current = ScreenInsets::default(),
            None => {}
        }

        let focused = ctx.wants_keyboard_input();
        if focused && !self.had_keyboard_focus {
            if let Ok(mut until) = self.state.fast_until.lock() {
                *until = Some(Instant::now() + Self::FOCUS_WATCH);
            }
        }
        self.had_keyboard_focus = focused;
        let keyboard_open = self.current.bottom > screen.height() * Self::KEYBOARD_MIN_FRACTION;
        self.state
            .keyboard_open
            .store(keyboard_open, Ordering::Relaxed);
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Insets for a window of `width` x `height` with the given content rect
    /// (`[left, top, right, bottom]`).
    fn insets(width: i32, height: i32, rect: [i32; 4]) -> Option<ScreenInsets> {
        let [left, top, right, bottom] = rect;
        ScreenInsets::from_geometry(WindowGeometry {
            width,
            height,
            left,
            top,
            right,
            bottom,
        })
    }

    fn bottom(px: f32) -> ScreenInsets {
        ScreenInsets {
            bottom: px,
            ..Default::default()
        }
    }

    #[test]
    fn keyboard_becomes_bottom_inset() {
        // 1080x2400 window, status bar 80px, keyboard + nav bar 1000px.
        let expected = ScreenInsets {
            left: 0.0,
            top: 80.0,
            right: 0.0,
            bottom: 1000.0,
        };
        assert_eq!(insets(1080, 2400, [0, 80, 1080, 1400]), Some(expected));
    }

    #[test]
    fn full_window_has_no_insets() {
        let full = insets(1080, 2400, [0, 0, 1080, 2400]);
        assert_eq!(full, Some(ScreenInsets::default()));
    }

    #[test]
    fn empty_rect_is_unknown() {
        // android-activity reports an all-zero rect until the first update.
        assert_eq!(insets(1080, 2400, [0, 0, 0, 0]), None);
        assert_eq!(insets(0, 0, [0, 0, 0, 0]), None);
    }

    #[test]
    fn stale_rect_after_rotation_is_ignored() {
        // Portrait rect against a landscape window and vice versa.
        assert_eq!(insets(2400, 1080, [0, 80, 1080, 2400]), None);
        assert_eq!(insets(1080, 2400, [0, 0, 2400, 1080]), None);
    }

    #[test]
    fn implausibly_small_rect_is_ignored() {
        assert_eq!(insets(1080, 2400, [0, 0, 1080, 100]), None);
    }

    #[test]
    fn fits_checks_remaining_area() {
        let portrait_keyboard = ScreenInsets {
            top: 25.0,
            ..bottom(330.0)
        };
        assert!(portrait_keyboard.fits(egui::vec2(360.0, 800.0)));
        // The same insets on a landscape screen would leave nothing visible.
        assert!(!portrait_keyboard.fits(egui::vec2(800.0, 360.0)));
        assert!(ScreenInsets::default().fits(egui::vec2(360.0, 800.0)));
    }

    #[test]
    fn converts_to_points() {
        let physical = ScreenInsets {
            left: 0.0,
            top: 60.0,
            right: 30.0,
            bottom: 900.0,
        };
        let expected = ScreenInsets {
            left: 0.0,
            top: 20.0,
            right: 10.0,
            bottom: 300.0,
        };
        assert_eq!(physical.to_points(3.0), expected);
        assert!(bottom(10.0).to_points(0.0).is_zero());
    }
}
