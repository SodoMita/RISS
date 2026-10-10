use super::{Palette, RissApp, Screen};
use crate::settings::SettingKind;
use crate::storage;
use eframe::egui::{self, CornerRadius, RichText, Vec2};

impl RissApp {
    /// The settings screen stays solid (`p.bg`, not `p.panel`) the way KISS
    /// opens its preferences in an ordinary opaque activity: a long scrolling
    /// list of small text needs a predictable background. The launcher screen
    /// is the one that shows the wallpaper.
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
                        self.reload_apps();
                    }
                    ui.heading("Settings");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("KISS-compatible").small().color(p.dim));
                    });
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings_query)
                        .hint_text("Search settings")
                        .desired_width(f32::INFINITY),
                );
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
                        let needle = self.settings_query.to_lowercase();
                        let mut section = "";
                        for spec in crate::settings::specs() {
                            if !needle.is_empty()
                                && !format!("{} {} {}", spec.section, spec.title, spec.key)
                                    .to_lowercase()
                                    .contains(&needle)
                            {
                                continue;
                            }
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
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    let value = self
                                                        .settings
                                                        .values
                                                        .entry(spec.key.to_owned())
                                                        .or_default();
                                                    egui::ComboBox::from_id_salt(spec.key)
                                                        .selected_text(value.as_str())
                                                        .width(140.0)
                                                        .show_ui(ui, |ui| {
                                                            for option in options {
                                                                if ui
                                                                    .selectable_value(
                                                                        value,
                                                                        (*option).to_owned(),
                                                                        *option,
                                                                    )
                                                                    .changed()
                                                                {
                                                                    changed = true;
                                                                }
                                                            }
                                                        });
                                                },
                                            );
                                        });
                                    }
                                    SettingKind::Number { min, max } => {
                                        ui.horizontal(|ui| {
                                            ui.label(spec.title);
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    let value = self
                                                        .settings
                                                        .values
                                                        .entry(spec.key.to_owned())
                                                        .or_default();
                                                    let mut number = value
                                                        .parse::<usize>()
                                                        .unwrap_or(min)
                                                        .clamp(min, max);
                                                    if ui
                                                        .add(
                                                            egui::DragValue::new(&mut number)
                                                                .range(min..=max),
                                                        )
                                                        .changed()
                                                    {
                                                        *value = number.to_string();
                                                        changed = true;
                                                    }
                                                },
                                            );
                                        });
                                    }
                                    SettingKind::Text => {
                                        ui.label(spec.title);
                                        let value = self
                                            .settings
                                            .values
                                            .entry(spec.key.to_owned())
                                            .or_default();
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
                                            .add_sized(
                                                [ui.available_width(), 42.0],
                                                egui::Button::new(spec.title),
                                            )
                                            .clicked()
                                        {
                                            self.run_setting_action(spec.key);
                                        }
                                    }
                                });
                            if changed {
                                self.save_settings();
                                self.update_results();
                            }
                            ui.add_space(3.0);
                        }
                        ui.add_space(30.0);
                    });
            });
    }

    fn run_setting_action(&mut self, key: &str) {
        match key {
            "reset-history" => {
                // Favorites and tags are deliberately preserved.
                self.history.clear_history();
                self.save_history();
                self.reload_apps();
                self.set_status("History cleared (favorites kept)");
            }
            "reset-favorites" => {
                self.history.favorites.clear();
                self.save_history();
                self.reload_apps();
                self.set_status("Favorites cleared");
            }
            "reset-excluded-apps" => {
                self.settings
                    .values
                    .insert("edit-excluded-apps".into(), String::new());
                self.save_settings();
                self.reload_apps();
            }
            "reset-excluded-from-history-apps" => {
                self.settings
                    .values
                    .insert("edit-excluded-from-history-apps".into(), String::new());
                self.save_settings();
                self.update_results();
            }
            "reset-excluded-app-shortcuts" => {
                self.settings
                    .values
                    .insert("edit-excluded-app-shortcuts".into(), String::new());
                self.save_settings();
            }
            "reset-all" => {
                let save_failed = self.settings.reset().err();
                self.reload_apps();
                match save_failed {
                    Some(err) => self.set_status(format!("Settings reset but not saved: {err}")),
                    None => self.set_status("Settings reset"),
                }
            }
            "restart" => self.set_status("Settings applied — restart is not required"),
            "export-settings" => match export_backup(&self.settings, &self.history) {
                Ok(path) => self.set_status(format!("Exported to {}", path.display())),
                Err(error) => self.set_status(error),
            },
            "import-settings" => match import_backup() {
                Ok((settings, history)) => {
                    self.settings = settings;
                    self.history = history;
                    self.save_settings();
                    self.save_history();
                    self.reload_apps();
                    self.set_status("Backup imported");
                }
                Err(error) => self.set_status(error),
            },
            "default-launcher" => self.set_status("Choose RISS in your system’s default apps"),
            "rate-app" => self.set_status("Thank you for using RISS"),
            "reset-shortcuts" => {
                self.history.clear_all_shortcuts();
                self.save_history();
                self.set_status("Shortcuts cleared");
            }
            "reset-search-providers" => {
                self.settings
                    .values
                    .insert("custom-search-provider-add".into(), String::new());
                self.settings
                    .values
                    .insert("deleting-search-providers-names".into(), String::new());
                self.settings.values.insert(
                    "selected-search-provider-names".into(),
                    "duckduckgo,google,wikipedia".into(),
                );
                self.settings
                    .values
                    .insert("default-search-provider".into(), "duckduckgo".into());
                self.save_settings();
                self.update_results();
                self.set_status("Search providers reset");
            }
            _ => self.set_status("Action completed"),
        }
    }
}

/// Export settings and history to a single backup file in the data directory.
fn export_backup(
    settings: &crate::settings::SettingsData,
    history: &crate::history::HistoryData,
) -> Result<std::path::PathBuf, String> {
    let path = storage::file_path(storage::BACKUP_FILE);
    let payload = serde_json::json!({
        "settings": settings,
        "history": history,
    });
    let text = serde_json::to_string_pretty(&payload)
        .map_err(|error| format!("Export failed: {}", error))?;
    std::fs::write(&path, text).map_err(|error| format!("Export failed: {}", error))?;
    Ok(path)
}

/// Import a backup written by [`export_backup`].
fn import_backup() -> Result<(crate::settings::SettingsData, crate::history::HistoryData), String> {
    let path = storage::file_path(storage::BACKUP_FILE);
    let text =
        std::fs::read_to_string(&path).map_err(|error| format!("Import failed: {}", error))?;
    let payload: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| format!("Import failed: {}", error))?;
    let empty = serde_json::json!({});
    let mut settings: crate::settings::SettingsData =
        serde_json::from_value(payload.get("settings").unwrap_or(&empty).clone())
            .map_err(|error| format!("Import failed: {}", error))?;
    crate::settings::merge_defaults(&mut settings);
    let history: crate::history::HistoryData =
        serde_json::from_value(payload.get("history").unwrap_or(&empty).clone())
            .map_err(|error| format!("Import failed: {}", error))?;
    Ok((settings, history))
}
