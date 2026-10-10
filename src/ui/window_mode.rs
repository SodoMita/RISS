//! Desktop window mode: windowed, maximized or fullscreen.
//!
//! Chosen with the `window-mode` setting, or for one run with the
//! `--windowed`, `--maximized` and `--fullscreen` command-line flags (handy
//! for compositor configs such as `exec riss_launcher --fullscreen`).
//!
//! Maximized windows are shrunk by most compositors (KWin, sway when tiled,
//! phoc) to make room for an on-screen keyboard. Fullscreen windows are not,
//! so the launcher reserves that space itself (see the `osk` module).

use crate::settings::SettingsData;
use eframe::egui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowMode {
    Windowed,
    Maximized,
    Fullscreen,
}

impl WindowMode {
    /// Setting values, in the order shown in the settings screen.
    pub const KEYS: &'static [&'static str] = &["windowed", "maximized", "fullscreen"];

    pub fn from_key(key: &str) -> Self {
        match key {
            "maximized" => Self::Maximized,
            "fullscreen" => Self::Fullscreen,
            _ => Self::Windowed,
        }
    }

    /// The mode requested on the command line, if any. The last flag wins.
    pub fn from_args<I, S>(args: I) -> Option<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        args.into_iter()
            .filter_map(|arg| match arg.as_ref() {
                "--windowed" => Some(Self::Windowed),
                "--maximized" => Some(Self::Maximized),
                "--fullscreen" => Some(Self::Fullscreen),
                _ => None,
            })
            .last()
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Windowed => "windowed",
            Self::Maximized => "maximized",
            Self::Fullscreen => "fullscreen",
        }
    }

    pub fn from_settings(settings: &SettingsData) -> Self {
        Self::from_key(settings.value("window-mode"))
    }

    /// The mode to open the window in: the command line wins over settings.
    pub fn at_startup(settings: &SettingsData) -> Self {
        Self::from_args(std::env::args().skip(1)).unwrap_or_else(|| Self::from_settings(settings))
    }

    pub fn apply_to_builder(self, builder: egui::ViewportBuilder) -> egui::ViewportBuilder {
        match self {
            Self::Windowed => builder,
            Self::Maximized => builder.with_maximized(true),
            Self::Fullscreen => builder.with_fullscreen(true),
        }
    }

    /// Switch an open window to this mode.
    pub fn apply_to_window(self, ctx: &egui::Context) {
        let (maximized, fullscreen) = match self {
            Self::Windowed => (false, false),
            Self::Maximized => (true, false),
            Self::Fullscreen => (false, true),
        };
        // Leave fullscreen first so the maximize request is not swallowed.
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(maximized));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_setting_keys() {
        for key in WindowMode::KEYS {
            assert_eq!(WindowMode::from_key(key).key(), *key);
        }
        assert_eq!(WindowMode::from_key("bogus"), WindowMode::Windowed);
    }

    #[test]
    fn last_command_line_flag_wins() {
        assert_eq!(WindowMode::from_args(["--verbose"]), None);
        assert_eq!(
            WindowMode::from_args(["--fullscreen"]),
            Some(WindowMode::Fullscreen)
        );
        assert_eq!(
            WindowMode::from_args(["--fullscreen", "--maximized"]),
            Some(WindowMode::Maximized)
        );
    }
}
