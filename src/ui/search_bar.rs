use super::{Palette, RissApp, Screen};
use eframe::egui::{self, Color32, CornerRadius, FontId, RichText, Stroke, Vec2};

/// The keys the launcher reacts to, collected in a single read of egui's
/// input state instead of one read per key.
struct NavKeys {
    up: bool,
    down: bool,
    page_up: bool,
    page_down: bool,
    home: bool,
    end: bool,
    enter: bool,
    escape: bool,
}

impl RissApp {
    /// Rows per page for the paging keys.
    const PAGE_STEP: usize = 8;

    pub(super) fn handle_keys(&mut self, ctx: &egui::Context) {
        // One `ctx.input` call, not one per key: each one locks egui's input
        // state, and this ran seven times per frame for keys the user is
        // mostly not pressing.
        let keys = ctx.input(|i| NavKeys {
            up: i.key_pressed(egui::Key::ArrowUp),
            down: i.key_pressed(egui::Key::ArrowDown),
            page_up: i.key_pressed(egui::Key::PageUp),
            page_down: i.key_pressed(egui::Key::PageDown),
            home: i.key_pressed(egui::Key::Home),
            end: i.key_pressed(egui::Key::End),
            enter: i.key_pressed(egui::Key::Enter),
            escape: i.key_pressed(egui::Key::Escape),
        });
        if self.screen == Screen::Settings {
            if keys.escape {
                self.screen = Screen::Launcher;
            }
            return;
        }
        if keys.escape {
            if !self.query.is_empty() {
                self.query.clear();
            } else {
                self.show_all_apps = false;
            }
            self.update_results();
            return;
        }
        let last = self.results.len().saturating_sub(1);
        let before = self.selected_index;
        if keys.up {
            self.selected_index = self.selected_index.saturating_sub(1);
        }
        if keys.down && !self.results.is_empty() {
            self.selected_index = (self.selected_index + 1).min(last);
        }
        // Paging keys, so a keyboard can cross a long app list without
        // stepping through it one row at a time.
        if keys.page_up {
            self.selected_index = self.selected_index.saturating_sub(Self::PAGE_STEP);
        }
        if keys.page_down {
            self.selected_index = (self.selected_index + Self::PAGE_STEP).min(last);
        }
        if keys.home && !self.results.is_empty() {
            self.selected_index = 0;
        }
        if keys.end && !self.results.is_empty() {
            self.selected_index = last;
        }
        if self.selected_index != before {
            // Scroll the highlighted row along, which the list otherwise has
            // no reason to do: it is not the row under the finger.
            self.list_follow_selection = true;
        }
        if keys.enter && !self.results.is_empty() {
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
