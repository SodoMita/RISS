use super::{
    icons::{bar_icon_button, BarIcon},
    Palette, RissApp, Screen,
};
use eframe::egui::{self, Color32, CornerRadius, FontId, Stroke};

impl RissApp {
    pub(super) fn handle_keys(&mut self, ctx: &egui::Context) {
        if self.screen == Screen::Settings {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.screen = Screen::Launcher;
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if !self.query.is_empty() {
                self.query.clear();
            } else {
                self.show_all_apps = false;
            }
            self.update_results();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            self.selected_index = self.selected_index.saturating_sub(1);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) && !self.results.is_empty() {
            self.selected_index = (self.selected_index + 1).min(self.results.len() - 1);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && !self.results.is_empty() {
            self.launch_result(self.selected_index);
        }
    }

    pub(super) fn show_search_bar(&mut self, ui: &mut egui::Ui, p: Palette) {
        let rounded = if self.settings.enabled("pref-rounded-bars") {
            24
        } else {
            5
        };
        let transparent = self.settings.enabled("transparent-search");
        let all_apps = self.show_all_apps;
        // All-apps mode copies KISS's "kiss bar" (`main_kissbar.xml` +
        // `rounded_kiss_bar.xml`): solid `?attr/colorPrimary` fill, no
        // outline, white glyphs. The filled ring center is our own addition
        // (KISS keeps the ring hollow).
        let bar_fill = if all_apps {
            p.accent
        } else if transparent {
            Color32::TRANSPARENT
        } else {
            p.surface
        };
        let border = if all_apps {
            Color32::TRANSPARENT
        } else {
            p.border
        };
        egui::Frame::NONE
            .fill(bar_fill)
            .corner_radius(CornerRadius::same(rounded))
            .stroke(Stroke::new(1.0_f32, border))
            .inner_margin(egui::Margin::symmetric(8, 5))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // KISS layout (`main.xml` + `InterfaceTweaks`): the
                    // launcher circle is accent-tinted; settings/clear use
                    // the search (text) color, or the accent color when the
                    // search bar is transparent. All-apps mode overrides all
                    // of it with the white-on-accent "kiss bar" palette.
                    let swap = self.settings.enabled("pref-swap-kiss-button-with-menu");
                    let show_glyph = !self.settings.enabled("pref-hide-circle");
                    let kiss_color = if all_apps { Color32::WHITE } else { p.accent };
                    let menu_color = if all_apps {
                        Color32::WHITE
                    } else if transparent {
                        p.accent
                    } else {
                        p.text
                    };
                    let kiss = if all_apps {
                        (
                            BarIcon::Kiss {
                                filled_center: true,
                            },
                            "History",
                        )
                    } else {
                        (
                            BarIcon::Kiss {
                                filled_center: false,
                            },
                            "All apps",
                        )
                    };
                    let (left_icon, left_tooltip) = if swap {
                        (BarIcon::Cog, "Settings")
                    } else {
                        kiss
                    };
                    let left_color = if matches!(left_icon, BarIcon::Kiss { .. }) {
                        kiss_color
                    } else {
                        menu_color
                    };
                    let left_response =
                        bar_icon_button(ui, left_icon, left_color, left_tooltip, show_glyph);
                    if left_response.clicked() {
                        if swap {
                            self.screen = Screen::Settings;
                        } else {
                            self.show_all_apps = !self.show_all_apps;
                            self.update_results();
                        }
                    }
                    let hint = if self.settings.enabled("pref-hide-search-bar-hint") {
                        ""
                    } else {
                        "Search…"
                    };
                    // White typed text and hint on the accent-filled bar
                    // (`weak_text_color` derives from this override too).
                    if all_apps {
                        ui.visuals_mut().override_text_color = Some(Color32::WHITE);
                    }
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .id(egui::Id::new("riss-search"))
                            .hint_text(hint)
                            .font(FontId::proportional(17.0))
                            .desired_width((ui.available_width() - 52.0).max(40.0))
                            .frame(false),
                    );
                    if response.changed() {
                        self.show_all_apps = false;
                        self.update_results();
                    }
                    // KISS swaps the settings button for a clear (X) button
                    // while a query is typed.
                    let (right_icon, right_tooltip) = if !self.query.is_empty() {
                        (BarIcon::Clear, "Clear")
                    } else if swap {
                        kiss
                    } else {
                        (BarIcon::Cog, "Settings")
                    };
                    let right_color = if matches!(right_icon, BarIcon::Kiss { .. }) {
                        kiss_color
                    } else {
                        menu_color
                    };
                    let right_response =
                        bar_icon_button(ui, right_icon, right_color, right_tooltip, show_glyph);
                    if right_response.clicked() {
                        if !self.query.is_empty() {
                            self.query.clear();
                            self.update_results();
                        } else if swap {
                            self.show_all_apps = !self.show_all_apps;
                            self.update_results();
                        } else {
                            self.screen = Screen::Settings;
                        }
                    }
                })
            });
    }
}
