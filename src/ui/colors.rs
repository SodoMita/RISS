use super::RissApp;
use crate::settings::SettingsData;
use eframe::egui::{self, Color32, Vec2};

#[derive(Clone, Copy)]
pub(super) struct Palette {
    /// The solid theme colour. Contrast choices and icon "punch-outs" are made
    /// against this even when the launcher itself is see-through.
    pub(super) bg: Color32,
    /// What is painted behind the launcher: `bg`, or fully transparent when the
    /// system wallpaper should show through.
    pub(super) panel: Color32,
    pub(super) surface: Color32,
    pub(super) hover: Color32,
    pub(super) text: Color32,
    pub(super) dim: Color32,
    pub(super) accent: Color32,
    pub(super) border: Color32,
}

/// Should the system wallpaper be visible behind the launcher?
///
/// On Android the launcher *is* the home screen: the wallpaper belongs to the
/// system, so the window has to be transparent for it to be seen at all (issue
/// #32). Desktop windows keep the solid palette — there the launcher is an
/// ordinary window and a transparent one needs a compositing window manager.
pub(super) fn wallpaper_visible(settings: &SettingsData) -> bool {
    wallpaper_visible_on(settings, cfg!(target_os = "android"))
}

/// The same question with the platform capability (`transparent_window`) passed
/// in, so the rule can be exercised on any host.
pub(super) fn wallpaper_visible_on(settings: &SettingsData, transparent_window: bool) -> bool {
    transparent_window
        // Only the "transparent" theme is see-through; light and dark are solid.
        && settings.value("theme") == "transparent"
        // KISS's "Wallpaper visibility" preference: "hide" paints a solid
        // background over the wallpaper.
        && settings.value("theme-wallpaper") != "hide"
}

impl Palette {
    pub(super) fn from_settings(settings: &SettingsData) -> Self {
        Self::for_platform(settings, cfg!(target_os = "android"))
    }

    fn for_platform(settings: &SettingsData, transparent_window: bool) -> Self {
        let light = settings.value("theme") == "light"
            || (settings.value("night-mode") == "light" && settings.value("theme") != "dark");
        let accent =
            parse_hex(settings.value("primary-color")).unwrap_or(Color32::from_rgb(137, 180, 250));
        let (bg, surface, hover, text, dim, border) = if light {
            (
                Color32::from_rgb(246, 247, 251),
                Color32::WHITE,
                Color32::from_rgb(230, 234, 244),
                Color32::from_rgb(30, 32, 40),
                Color32::from_rgb(99, 104, 120),
                Color32::from_rgb(210, 214, 224),
            )
        } else {
            (
                Color32::from_rgb(20, 20, 28),
                Color32::from_rgb(35, 35, 47),
                Color32::from_rgb(51, 52, 68),
                Color32::from_rgb(232, 234, 245),
                Color32::from_rgb(151, 155, 174),
                Color32::from_rgb(67, 68, 84),
            )
        };
        Self {
            bg,
            panel: if wallpaper_visible_on(settings, transparent_window) {
                Color32::TRANSPARENT
            } else {
                bg
            },
            surface,
            hover,
            text,
            dim,
            accent,
            border,
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
        // `panel_fill` is what a panel paints when it is not given an explicit
        // frame; it stays transparent while the wallpaper shows through.
        // `window_fill` backs tooltips and popups, which have to stay readable
        // over an arbitrary wallpaper.
        visuals.panel_fill = p.panel;
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

/// Muted colours used for the generated application badges (from PR #3).
pub(super) const BADGE_COLORS: [[u8; 3]; 8] = [
    [137, 180, 250],
    [166, 227, 161],
    [249, 226, 175],
    [243, 139, 168],
    [180, 190, 254],
    [148, 226, 213],
    [247, 208, 166],
    [203, 166, 247],
];

/// Deterministic badge colour for an application name (FNV-1a of the name).
pub(super) fn badge_color(seed: &str) -> Color32 {
    let mut hash: u32 = 2166136261;
    for byte in seed.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    let color = BADGE_COLORS[(hash % BADGE_COLORS.len() as u32) as usize];
    Color32::from_rgb(color[0], color[1], color[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with(theme: &str, wallpaper: &str) -> SettingsData {
        let mut settings = SettingsData::default();
        settings.values.insert("theme".to_owned(), theme.to_owned());
        settings.values.insert("theme-wallpaper".to_owned(), wallpaper.to_owned());
        settings
    }

    #[test]
    fn transparent_theme_lets_the_wallpaper_through() {
        let settings = settings_with("transparent", "default");
        assert!(wallpaper_visible_on(&settings, true));
        let palette = Palette::for_platform(&settings, true);
        assert_eq!(palette.panel, Color32::TRANSPARENT);
        // The solid theme colour is still there for contrast decisions.
        assert_eq!(palette.bg.a(), 255);
    }

    #[test]
    fn hiding_the_wallpaper_paints_a_solid_panel() {
        let settings = settings_with("transparent", "hide");
        assert!(!wallpaper_visible_on(&settings, true));
        let palette = Palette::for_platform(&settings, true);
        assert_eq!(palette.panel, palette.bg);
    }

    #[test]
    fn solid_themes_never_show_the_wallpaper() {
        for theme in ["light", "dark"] {
            let settings = settings_with(theme, "default");
            assert!(!wallpaper_visible_on(&settings, true), "{theme} is solid");
            let palette = Palette::for_platform(&settings, true);
            assert_eq!(palette.panel, palette.bg, "{theme} is solid");
        }
    }

    #[test]
    fn platforms_without_transparent_windows_stay_solid() {
        let settings = settings_with("transparent", "show");
        assert!(!wallpaper_visible_on(&settings, false));
        let palette = Palette::for_platform(&settings, false);
        assert_eq!(palette.panel, palette.bg);
    }
}
