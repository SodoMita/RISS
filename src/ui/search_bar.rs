use super::{Palette, RissApp, Screen};
use eframe::egui::{self, Color32, CornerRadius, FontId, RichText, Stroke, Vec2};

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
            self.activate(self.selected_index);
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
                    let swap = self.settings.enabled("pref-swap-kiss-button-with-menu");
                    let left_icon = if swap { "⋮" } else { "⌁" };
                    if ui
                        .add(
                            egui::Button::new(RichText::new(left_icon).size(24.0))
                                .frame(false)
                                .min_size(Vec2::splat(44.0)),
                        )
                        .on_hover_text(if swap { "Settings" } else { "All apps" })
                        .clicked()
                    {
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
                            .desired_width(ui.available_width() - 52.0)
                            .frame(false),
                    );
                    if response.changed() {
                        self.show_all_apps = false;
                        self.update_results();
                    }
                    let icon = if self.query.is_empty() {
                        if swap {
                            "⌁"
                        } else {
                            "⋮"
                        }
                    } else {
                        "×"
                    };
                    if ui
                        .add(
                            egui::Button::new(RichText::new(icon).size(22.0))
                                .frame(false)
                                .min_size(Vec2::splat(44.0)),
                        )
                        .clicked()
                    {
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
