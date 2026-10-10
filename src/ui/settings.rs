use super::{Palette, RissApp, Screen};
use crate::settings::{SettingKind, SettingSpec};
use crate::storage;
use eframe::egui::{self, CornerRadius, RichText, Vec2};

/// Rows of the settings list, as indices into [`crate::settings::specs`].
///
/// The list is static apart from the filter box, so it is filtered once per
/// keystroke instead of once per repaint: the old code built a lowercased
/// `format!` string per setting, for every setting, every frame.
fn matching_rows(query: &str) -> Vec<usize> {
    let specs = crate::settings::specs();
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return (0..specs.len()).collect();
    }
    specs
        .iter()
        .enumerate()
        .filter(|(_, spec)| {
            spec.section.to_lowercase().contains(&needle)
                || spec.title.to_lowercase().contains(&needle)
                || spec.key.to_lowercase().contains(&needle)
        })
        .map(|(index, _)| index)
        .collect()
}

impl RissApp {
    pub(super) fn show_settings(&mut self, ctx: &egui::Context, p: Palette) {
        egui::TopBottomPanel::top("settings-header")
            .frame(
                egui::Frame::NONE
                    .fill(p.bg)
                    .inner_margin(egui::Margin::same(10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new("‹")
                                .frame(false)
                                .min_size(Vec2::splat(44.0)),
                        )
                        .clicked()
                    {
                        self.screen = Screen::Launcher;
                        self.apply_exclusions();
                        self.update_results_in_place();
                    }
                    ui.heading("Settings");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("KISS-compatible").small().color(p.dim));
                    });
                });
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.settings_query)
                        .hint_text("Search settings")
                        .desired_width(f32::INFINITY),
                );
                if response.changed() {
                    self.settings_rows = None;
                }
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(p.bg)
                    .inner_margin(egui::Margin::symmetric(12, 4)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.settings_rows.is_none() {
                            self.settings_rows = Some(matching_rows(&self.settings_query));
                        }
                        // Taken out of `self` so the loop can borrow the
                        // settings it filters while mutating the app state.
                        let rows = self.settings_rows.take().unwrap_or_default();
                        let specs = crate::settings::specs();
                        let mut section = "";
                        for &index in &rows {
                            let spec = &specs[index];
                            if spec.section != section {
                                section = spec.section;
                                ui.add_space(14.0);
                                ui.label(
                                    RichText::new(section.to_uppercase())
                                        .size(12.0)
                                        .strong()
                                        .color(p.accent),
                                );
                                ui.add_space(3.0);
                            }
                            if self.setting_row(ui, spec, p) {
                                self.save_settings();
                                if spec.section == "Icons" {
                                    // Icon packs change what a name resolves to.
                                    self.icons.clear();
                                }
                                if spec.key == "edit-excluded-apps" {
                                    self.apply_exclusions();
                                }
                                self.update_results_in_place();
                            }
                            ui.add_space(3.0);
                        }
                        self.settings_rows = Some(rows);
                        ui.add_space(30.0);
                    });
            });
    }

    /// Draw one preference and report whether the user changed it.
    fn setting_row(&mut self, ui: &mut egui::Ui, spec: &SettingSpec, p: Palette) -> bool {
        let mut changed = false;
        egui::Frame::NONE
            .fill(p.surface)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(12, 6))
            .show(ui, |ui| match spec.kind {
                SettingKind::Toggle => {
                    let value = self
                        .settings
                        .bools
                        .entry(spec.key.to_owned())
                        .or_insert(false);
                    if ui.checkbox(value, spec.title).changed() {
                        changed = true;
                    }
                }
                SettingKind::Choice(options) => {
                    ui.horizontal(|ui| {
                        ui.label(spec.title);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let value =
                                self.settings.values.entry(spec.key.to_owned()).or_default();
                            egui::ComboBox::from_id_salt(spec.key)
                                .selected_text(value.as_str())
                                .width(140.0)
                                .show_ui(ui, |ui| {
                                    for option in options {
                                        if ui
                                            .selectable_value(value, (*option).to_owned(), *option)
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                    }
                                });
                        });
                    });
                }
                SettingKind::Number { min, max } => {
                    ui.horizontal(|ui| {
                        ui.label(spec.title);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let value =
                                self.settings.values.entry(spec.key.to_owned()).or_default();
                            let mut number = value.parse::<usize>().unwrap_or(min).clamp(min, max);
                            if ui
                                .add(egui::DragValue::new(&mut number).range(min..=max))
                                .changed()
                            {
                                *value = number.to_string();
                                changed = true;
                            }
                        });
                    });
                }
                SettingKind::Text => {
                    ui.label(spec.title);
                    let value = self.settings.values.entry(spec.key.to_owned()).or_default();
                    if ui
                        .add(
                            egui::TextEdit::singleline(value)
                                .desired_width(f32::INFINITY)
                                .hint_text(spec.summary),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                }
                SettingKind::Action => {
                    if ui
                        .add_sized([ui.available_width(), 42.0], egui::Button::new(spec.title))
                        .clicked()
                    {
                        self.run_setting_action(spec.key);
                    }
                }
            });
        changed
    }

    fn run_setting_action(&mut self, key: &str) {
        match key {
            "reset-history" => {
                // Favorites and tags are deliberately preserved.
                self.history.clear_history();
                self.save_history();
                self.apply_history_fields();
                self.update_results_in_place();
                self.set_status("History cleared (favorites kept)");
            }
            "reset-favorites" => {
                self.history.favorites.clear();
                self.save_history();
                self.apply_history_fields();
                self.update_results_in_place();
                self.set_status("Favorites cleared");
            }
            "reset-excluded-apps" => {
                self.settings
                    .values
                    .insert("edit-excluded-apps".into(), String::new());
                self.save_settings();
                self.apply_exclusions();
                self.update_results_in_place();
            }
            "reset-excluded-from-history-apps" => {
                self.settings
                    .values
                    .insert("edit-excluded-from-history-apps".into(), String::new());
                self.save_settings();
                self.update_results_in_place();
            }
            "reset-excluded-app-shortcuts" => {
                self.settings
                    .values
                    .insert("edit-excluded-app-shortcuts".into(), String::new());
                self.save_settings();
            }
            "reset-all" => {
                let save_failed = self.settings.reset().err();
                self.apply_exclusions();
                self.icons.clear();
                self.update_results();
                match save_failed {
                    Some(err) => self.set_status(format!("Settings reset but not saved: {err}")),
                    None => self.set_status("Settings reset"),
                }
            }
            "restart" => self.set_status("Settings applied — restart is not required"),
            "export-settings" => self.set_status(format!(
                "Settings are stored in {}",
                storage::file_path(storage::SETTINGS_FILE).display()
            )),
            "import-settings" => self.set_status(format!(
                "Replace {}, then restart RISS",
                storage::file_path(storage::SETTINGS_FILE).display()
            )),
            "default-launcher" => self.set_status("Choose RISS in your system’s default apps"),
            "rate-app" => self.set_status("Thank you for using RISS"),
            "reset-shortcuts" | "reset-search-providers" => self.set_status("Provider data reset"),
            _ => self.set_status("Action completed"),
        }
    }
}
