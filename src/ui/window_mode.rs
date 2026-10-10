//! Desktop window mode, from the `window-mode` setting or, for one run, the
//! `--windowed` / `--maximized` / `--fullscreen` flags.

use crate::settings::SettingsData;
use eframe::egui::{self, ViewportBuilder, ViewportCommand};

/// The one application identifier this program exposes everywhere.
///
/// Wayland's `xdg_toplevel.set_app_id`, the `StartupWMClass` of the installed
/// desktop file and the Android package id all have to agree, or a compositor
/// cannot match the launcher window to its `.desktop` entry: no icon, no
/// grouping, and a launcher launched from its own menu appears as a second,
/// anonymous window. Compositors match the desktop file by *file name*, which is
/// why `install.sh` installs `com.risslauncher.app.desktop`.
pub const APP_ID: &str = "com.risslauncher.app";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowMode {
    Windowed,
    Maximized,
    Fullscreen,
}

impl WindowMode {
    fn from_key(key: &str) -> Option<Self> {
        match key {
            "windowed" => Some(Self::Windowed),
            "maximized" => Some(Self::Maximized),
            "fullscreen" => Some(Self::Fullscreen),
            _ => None,
        }
    }

    pub fn from_settings(settings: &SettingsData) -> Self {
        Self::from_key(settings.value("window-mode")).unwrap_or(Self::Windowed)
    }

    /// The last mode flag on the command line wins over settings.
    pub fn at_startup(settings: &SettingsData) -> Self {
        std::env::args()
            .rev()
            .find_map(|arg| arg.strip_prefix("--").and_then(Self::from_key))
            .unwrap_or_else(|| Self::from_settings(settings))
    }

    pub fn apply_to_builder(self, builder: ViewportBuilder) -> ViewportBuilder {
        builder
            .with_maximized(self == Self::Maximized)
            .with_fullscreen(self == Self::Fullscreen)
    }

    /// The whole startup viewport policy, so every entry point opens the window
    /// the same way: window mode first, then an alpha-capable surface when the
    /// wallpaper is meant to show through the launcher.
    ///
    /// Transparency is fixed when the window is created (winit picks the visual /
    /// framebuffer config once), so changing `theme` needs a restart — the
    /// panels themselves do follow the setting live.
    pub fn apply_startup(settings: &SettingsData, builder: ViewportBuilder) -> ViewportBuilder {
        Self::at_startup(settings)
            .apply_to_builder(builder)
            .with_app_id(APP_ID)
            .with_transparent(super::colors::wallpaper_visible(settings))
    }

    pub fn apply_to_window(self, ctx: &egui::Context) {
        // Leave fullscreen first so a maximize request is not swallowed.
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self == Self::Fullscreen));
        ctx.send_viewport_cmd(ViewportCommand::Maximized(self == Self::Maximized));
    }
}
