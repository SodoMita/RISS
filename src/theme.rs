//! Colour palettes.
//!
//! KISS ships a dark, a light and a solarized theme, with an automatic night
//! mode switch. The same set is implemented here, plus a configurable accent
//! colour (`primary_color`).

use crate::settings::Settings;
use eframe::egui::Color32;

/// Resolved colours for the current frame.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub accent: Color32,
    pub favorite: Color32,
    pub danger: Color32,
    pub calc_bg: Color32,
    pub calc_text: Color32,
}

impl Palette {
    pub fn is_dark(&self) -> bool {
        let [r, g, b, _] = self.bg.to_array();
        let luma = 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32;
        luma < 128.0
    }

    /// A colour that reads well on top of [`Palette::bg`].
    pub fn overlay(&self) -> Color32 {
        if self.is_dark() {
            Color32::from_black_alpha(160)
        } else {
            Color32::from_black_alpha(90)
        }
    }

    pub fn scrollbar(&self) -> Color32 {
        if self.is_dark() {
            Color32::from_white_alpha(60)
        } else {
            Color32::from_black_alpha(60)
        }
    }
}

/// Build the palette for the current settings and system preference.
pub fn build(settings: &Settings, system_dark: bool) -> Palette {
    let dark = settings.prefers_dark(system_dark);
    let accent = Color32::from_rgba_unmultiplied(
        settings.primary_color[0],
        settings.primary_color[1],
        settings.primary_color[2],
        settings.primary_color[3],
    );

    let mut palette = match (settings.theme, dark) {
        (crate::settings::ThemeKind::Solarized, true) => Palette {
            bg: Color32::from_rgb(0, 43, 54),
            surface: Color32::from_rgb(7, 54, 66),
            surface_alt: Color32::from_rgb(13, 65, 78),
            hover: Color32::from_rgb(22, 78, 92),
            selected: Color32::from_rgb(30, 92, 106),
            border: Color32::from_rgb(38, 76, 90),
            text: Color32::from_rgb(238, 232, 213),
            text_dim: Color32::from_rgb(131, 148, 150),
            favorite: Color32::from_rgb(181, 137, 0),
            danger: Color32::from_rgb(220, 80, 75),
            calc_bg: Color32::from_rgb(13, 65, 78),
            calc_text: Color32::from_rgb(133, 153, 0),
        },
        (crate::settings::ThemeKind::Solarized, false) => Palette {
            bg: Color32::from_rgb(253, 246, 227),
            surface: Color32::from_rgb(246, 237, 219),
            surface_alt: Color32::from_rgb(238, 232, 213),
            hover: Color32::from_rgb(232, 223, 200),
            selected: Color32::from_rgb(224, 214, 190),
            border: Color32::from_rgb(205, 196, 173),
            text: Color32::from_rgb(88, 110, 117),
            text_dim: Color32::from_rgb(131, 148, 150),
            favorite: Color32::from_rgb(181, 137, 0),
            danger: Color32::from_rgb(203, 66, 86),
            calc_bg: Color32::from_rgb(246, 237, 219),
            calc_text: Color32::from_rgb(133, 153, 0),
        },
        (_, false) => Palette {
            bg: Color32::from_rgb(246, 246, 248),
            surface: Color32::from_rgb(255, 255, 255),
            surface_alt: Color32::from_rgb(238, 238, 244),
            hover: Color32::from_rgb(228, 228, 236),
            selected: Color32::from_rgb(214, 219, 236),
            border: Color32::from_rgb(212, 212, 220),
            text: Color32::from_rgb(24, 24, 30),
            text_dim: Color32::from_rgb(112, 112, 126),
            favorite: Color32::from_rgb(214, 158, 20),
            danger: Color32::from_rgb(203, 66, 86),
            calc_bg: Color32::from_rgb(232, 244, 232),
            calc_text: Color32::from_rgb(26, 120, 40),
        },
        (_, true) => Palette {
            bg: Color32::from_rgb(24, 24, 28),
            surface: Color32::from_rgb(31, 31, 38),
            surface_alt: Color32::from_rgb(38, 38, 46),
            hover: Color32::from_rgb(49, 49, 59),
            selected: Color32::from_rgb(43, 51, 68),
            border: Color32::from_rgb(58, 58, 70),
            text: Color32::from_rgb(236, 236, 240),
            text_dim: Color32::from_rgb(150, 150, 162),
            favorite: Color32::from_rgb(250, 204, 94),
            danger: Color32::from_rgb(243, 139, 168),
            calc_bg: Color32::from_rgb(33, 48, 40),
            calc_text: Color32::from_rgb(166, 227, 161),
        },
    };

    palette.accent = accent;
    palette
}

/// Muted colours used for the generated application badges.
pub const BADGE_COLORS: [[u8; 3]; 8] = [
    [137, 180, 250],
    [166, 227, 161],
    [249, 226, 175],
    [243, 139, 168],
    [180, 190, 254],
    [148, 226, 213],
    [247, 208, 166],
    [203, 166, 247],
];

/// Deterministic badge colour for an application name.
pub fn badge_color(seed: &str) -> Color32 {
    let mut hash: u32 = 2166136261;
    for byte in seed.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    let color = BADGE_COLORS[(hash % BADGE_COLORS.len() as u32) as usize];
    Color32::from_rgb(color[0], color[1], color[2])
}