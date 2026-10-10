use super::RissApp;
use crate::settings::SettingsData;
use eframe::egui::{self, Color32, Vec2};

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub(super) bg: Color32,
    pub(super) surface: Color32,
    pub(super) hover: Color32,
    pub(super) text: Color32,
    pub(super) dim: Color32,
    pub(super) accent: Color32,
    pub(super) border: Color32,
}

impl Palette {
    pub(super) fn from_settings(settings: &SettingsData) -> Self {
        let light = settings.value("theme") == "light"
            || (settings.value("night-mode") == "light" && settings.value("theme") != "dark");
        let accent =
            parse_hex(settings.value("primary-color")).unwrap_or(Color32::from_rgb(137, 180, 250));
        if light {
            Self {
                bg: Color32::from_rgb(246, 247, 251),
                surface: Color32::WHITE,
                hover: Color32::from_rgb(230, 234, 244),
                text: Color32::from_rgb(30, 32, 40),
                dim: Color32::from_rgb(99, 104, 120),
                accent,
                border: Color32::from_rgb(210, 214, 224),
            }
        } else {
            Self {
                bg: Color32::from_rgb(20, 20, 28),
                surface: Color32::from_rgb(35, 35, 47),
                hover: Color32::from_rgb(51, 52, 68),
                text: Color32::from_rgb(232, 234, 245),
                dim: Color32::from_rgb(151, 155, 174),
                accent,
                border: Color32::from_rgb(67, 68, 84),
            }
        }
    }
}

fn parse_hex(value: &str) -> Option<Color32> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(hex, 16).ok()?;
    Some(Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

/// FNV-1a over the settings that decide the look. Equal keys mean an equal
/// result, which is what lets [`RissApp::apply_visuals`] skip its work.
fn visuals_key(settings: &SettingsData) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for key in ["theme", "night-mode", "primary-color"] {
        for byte in settings.value(key).as_bytes() {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash = (hash ^ 0x1f).wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

impl RissApp {
    /// Push the palette into egui's `Visuals` and `Style`.
    ///
    /// This runs once per appearance change rather than once per frame:
    /// rebuilding `Visuals` and cloning the whole `Style` on every repaint is
    /// pure overhead when nothing about the theme moved.
    pub(super) fn apply_visuals(&mut self, ctx: &egui::Context, p: Palette) {
        let key = visuals_key(&self.settings);
        if self.visuals_key == Some(key) {
            return;
        }
        self.visuals_key = Some(key);
        let mut visuals = if self.settings.value("theme") == "light" {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        visuals.override_text_color = Some(p.text);
        visuals.panel_fill = p.bg;
        visuals.window_fill = p.surface;
        visuals.widgets.inactive.bg_fill = p.surface;
        visuals.widgets.hovered.bg_fill = p.hover;
        visuals.widgets.active.bg_fill = p.accent;
        visuals.selection.bg_fill = p.accent;
        ctx.set_visuals(visuals);
        let mut style = (*ctx.style()).clone();
        style.spacing.interact_size.y = 44.0;
        style.spacing.button_padding = Vec2::new(14.0, 10.0);
        // egui animates widget colours (and the scroll bar fade) over
        // `animation_time`, and each animated step is another full repaint of
        // the launcher. On a phone the fade is invisible anyway, so it is
        // switched off to keep scrolling to the frames the user actually caused.
        style.animation_time = 0.0;
        ctx.set_style(style);
    }
}
