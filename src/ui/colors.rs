//! Theme and colour management for the launcher.
//!
//! Builds the [`Palette`] from the persisted settings and pushes it into the
//! egui [`Visuals`](egui::Visuals) and style for the current frame.

use super::RissApp;
use crate::settings::SettingsData;
use eframe::egui;
use egui::{Color32, Vec2};

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

impl RissApp {
    pub(super) fn apply_visuals(&self, ctx: &egui::Context, p: Palette) {
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
        ctx.set_style(style);
    }
}
