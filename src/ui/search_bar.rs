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
        egui::Frame::NONE
            .fill(if transparent {
                Color32::TRANSPARENT
            } else {
                p.surface
            })
            .corner_radius(CornerRadius::same(rounded))
            .stroke(Stroke::new(1.0_f32, p.border))
            .inner_margin(egui::Margin::symmetric(8, 5))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // KISS layout (`main.xml` + `InterfaceTweaks`): the
                    // launcher circle is always accent-tinted; menu/clear use
                    // the search (text) color, or the accent color when the
                    // search bar is transparent.
                    let swap = self.settings.enabled("pref-swap-kiss-button-with-menu");
                    let show_glyph = !self.settings.enabled("pref-hide-circle");
                    let menu_color = if transparent { p.accent } else { p.text };
                    let (left_icon, left_tooltip) = if swap {
                        (BarIcon::Menu, "Settings")
                    } else {
                        (BarIcon::Kiss, "All apps")
                    };
                    let left_color = if left_icon == BarIcon::Kiss {
                        p.accent
                    } else {
                        menu_color
                    };
                    let left_response = bar_icon_button(
                        ui,
                        left_icon,
                        left_color,
                        p.hover,
                        left_tooltip,
                        show_glyph,
                    );
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
                    // KISS swaps the menu button for a clear (X) button while
                    // a query is typed.
                    let (right_icon, right_tooltip) = if !self.query.is_empty() {
                        (BarIcon::Clear, "Clear")
                    } else if swap {
                        (BarIcon::Kiss, "All apps")
                    } else {
                        (BarIcon::Menu, "Settings")
                    };
                    let right_color = if right_icon == BarIcon::Kiss {
                        p.accent
                    } else {
                        menu_color
                    };
                    let right_response = bar_icon_button(
                        ui,
                        right_icon,
                        right_color,
                        p.hover,
                        right_tooltip,
                        show_glyph,
                    );
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
