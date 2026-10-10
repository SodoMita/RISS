//! Desktop window mode, from the `window-mode` setting or, for one run, the
//! `--windowed` / `--maximized` / `--fullscreen` flags.

use crate::settings::SettingsData;
use eframe::egui::{self, ViewportBuilder, ViewportCommand};

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

    pub fn apply_to_window(self, ctx: &egui::Context) {
        // Leave fullscreen first so a maximize request is not swallowed.
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self == Self::Fullscreen));
        ctx.send_viewport_cmd(ViewportCommand::Maximized(self == Self::Maximized));
    }
}
