//! Keeps the UI clear of the Android keyboard and system bars.
//!
//! NativeActivity's surface never shrinks when the keyboard opens: with
//! `adjustResize` only the activity's content rect does, and winit 0.30 drops
//! `ContentRectChanged`. So the content rect is read directly and the covered
//! edges are padded with empty panels.

use eframe::egui;

/// Window size and visible content rect, in physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowGeometry {
    pub width: i32,
    pub height: i32,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Covered space on each window edge, in points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenInsets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl ScreenInsets {
    /// `None` while the content rect is unknown (Android starts with an empty
    /// one) or stale (it does not fit the window right after a rotation).
    pub fn from_geometry(g: WindowGeometry, pixels_per_point: f32) -> Option<Self> {
        let fits = 0 <= g.left && g.left < g.right && g.right <= g.width;
        let fits = fits && 0 <= g.top && g.top < g.bottom && g.bottom <= g.height;
        let px = |v: i32| v as f32 / pixels_per_point;
        (fits && pixels_per_point > 0.0).then(|| Self {
            left: px(g.left),
            top: px(g.top),
            right: px(g.width - g.right),
            bottom: px(g.height - g.bottom),
        })
    }

    pub fn with_min_bottom(self, bottom: f32) -> Self {
        Self {
            bottom: self.bottom.max(bottom),
            ..self
        }
    }

    /// Pad the covered edges. Must run before any other panel of the frame.
    pub fn reserve(&self, ctx: &egui::Context, fill: egui::Color32) {
        use egui::{SidePanel, TopBottomPanel};
        let frame = egui::Frame::NONE.fill(fill);
        macro_rules! pad {
            ($size:expr, $panel:expr) => {
                if $size >= 0.5 {
                    let panel = $panel.resizable(false).show_separator_line(false);
                    panel.frame(frame).show(ctx, |_| {});
                }
            };
        }
        pad!(
            self.bottom,
            TopBottomPanel::bottom("inset-b").exact_height(self.bottom)
        );
        pad!(
            self.top,
            TopBottomPanel::top("inset-t").exact_height(self.top)
        );
        pad!(self.left, SidePanel::left("inset-l").exact_width(self.left));
        pad!(
            self.right,
            SidePanel::right("inset-r").exact_width(self.right)
        );
    }
}

/// Insets for this frame (always zero off Android).
pub fn current(ctx: &egui::Context) -> ScreenInsets {
    #[cfg(target_os = "android")]
    if let Some(g) = crate::android_app_entry::android_jni::window_geometry() {
        return ScreenInsets::from_geometry(g, ctx.pixels_per_point()).unwrap_or_default();
    }
    let _ = ctx;
    ScreenInsets::default()
}

/// Repaint whenever the window geometry changes. Android does not tell egui
/// (e.g. when the keyboard is dismissed with Back), so poll in the background.
pub fn watch(ctx: &egui::Context) {
    #[cfg(target_os = "android")]
    {
        use std::sync::Mutex;
        use std::time::Duration;
        // One thread per process; an activity restart only swaps the context.
        static CTX: Mutex<Option<egui::Context>> = Mutex::new(None);
        let Ok(mut slot) = CTX.lock() else { return };
        if slot.replace(ctx.clone()).is_some() {
            return;
        }
        std::thread::spawn(|| {
            use crate::android_app_entry::android_jni::window_geometry;
            let mut last = window_geometry();
            loop {
                std::thread::sleep(Duration::from_millis(150));
                let now = window_geometry();
                if now != last {
                    last = now;
                    if let Some(ctx) = CTX.lock().ok().and_then(|c| c.clone()) {
                        ctx.request_repaint();
                    }
                }
            }
        });
    }
    let _ = ctx;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insets(
        width: i32,
        height: i32,
        [left, top, right, bottom]: [i32; 4],
    ) -> Option<ScreenInsets> {
        let g = WindowGeometry {
            width,
            height,
            left,
            top,
            right,
            bottom,
        };
        ScreenInsets::from_geometry(g, 2.0)
    }

    #[test]
    fn keyboard_becomes_bottom_inset_in_points() {
        let expected = ScreenInsets {
            top: 40.0,
            bottom: 500.0,
            ..Default::default()
        };
        assert_eq!(insets(1080, 2400, [0, 80, 1080, 1400]), Some(expected));
    }

    #[test]
    fn unknown_or_stale_rects_are_ignored() {
        assert_eq!(insets(1080, 2400, [0, 0, 0, 0]), None);
        assert_eq!(insets(2400, 1080, [0, 80, 1080, 2400]), None);
        assert_eq!(insets(1080, 2400, [0, 0, 2400, 1080]), None);
    }
}
