use super::{Palette, RissApp};
#[cfg(target_os = "android")]
use crate::android_app_entry::{self as app_entry, AppEntry};
#[cfg(not(target_os = "android"))]
use crate::app_entry::{self, AppEntry};
use crate::search::{self, MatchType, SearchResult};
use eframe::egui::{self, Color32, CornerRadius, FontId, RichText, Sense, Stroke, Vec2};

impl RissApp {
    fn icon_texture(&mut self, ctx: &egui::Context, entry: &AppEntry) -> Option<egui::TextureId> {
        let key = if entry.icon.trim().is_empty() {
            entry.exec.clone()
        } else {
            entry.icon.clone()
        };
        if let Some(texture) = self.icon_textures.get(&key) {
            return Some(texture.id());
        }
        if self.icons_without_image.contains(&key) {
            return None;
        }

        let Some((width, height, rgba)) = app_entry::load_icon_rgba(entry) else {
            self.icons_without_image.insert(key);
            return None;
        };
        let valid_size = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4));
        if width == 0 || height == 0 || valid_size != Some(rgba.len()) {
            self.icons_without_image.insert(key);
            return None;
        }

        let image = egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba);
        let texture = ctx.load_texture(
            format!("app-icon:{key}"),
            image,
            egui::TextureOptions::LINEAR,
        );
        let id = texture.id();
        self.icon_textures.insert(key, texture);
        Some(id)
    }

    pub(super) fn show_favorites(&mut self, ui: &mut egui::Ui, p: Palette) {
        let favorites: Vec<AppEntry> = self
            .apps
            .iter()
            .filter(|a| a.is_favorite)
            .cloned()
            .collect();
        if favorites.is_empty() {
            return;
        }

        let icon_size = if self.settings.enabled("large-favorites-bar") {
            54.0
        } else {
            44.0
        };
        let mut launch_exec = None;
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for app in favorites {
                    let initial = app
                        .name
                        .chars()
                        .next()
                        .unwrap_or('?')
                        .to_uppercase()
                        .to_string();
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::splat(icon_size), Sense::click());
                    let response = response.on_hover_text(&app.name);
                    let icon_texture =
                        if !self.settings.enabled("icons-hide") && ui.is_rect_visible(rect) {
                            self.icon_texture(ui.ctx(), &app)
                        } else {
                            None
                        };
                    if let Some(texture_id) = icon_texture {
                        ui.painter().image(
                            texture_id,
                            rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        if !self.settings.enabled("transparent-favorites") {
                            ui.painter()
                                .circle_filled(rect.center(), icon_size * 0.5, p.surface);
                        }
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            initial,
                            FontId::proportional(if icon_size > 44.0 { 22.0 } else { 18.0 }),
                            p.text,
                        );
                    }
                    if response.clicked() {
                        launch_exec = Some(app.exec.clone());
                    }
                }
            });
        });
        if let Some(exec) = launch_exec {
            self.launch_exec(&exec);
        }
    }

    pub(super) fn show_results(&mut self, ui: &mut egui::Ui, p: Palette) {
        if let Some(value) = search::try_calculate(&self.query) {
            egui::Frame::NONE
                .fill(p.surface)
                .corner_radius(CornerRadius::same(12))
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.label(RichText::new(value).size(24.0).color(p.accent));
                });
            ui.add_space(6.0);
        }
        if self.results.is_empty() {
            let empty_rect = ui.available_rect_before_wrap();
            if empty_rect.is_positive() {
                let response = ui.interact(
                    empty_rect,
                    egui::Id::new("empty-results-gestures"),
                    Sense::click_and_drag(),
                );
                self.handle_empty_area_gestures(&response, ui.ctx());
            }
            ui.vertical_centered(|ui| {
                ui.add_space(32.0);
                ui.label(
                    RichText::new(if self.query.is_empty() {
                        "Swipe up for all apps".to_owned()
                    } else {
                        format!("No result for “{}”", self.query)
                    })
                    .color(p.dim),
                );
            });
            return;
        }
        let results = self.results.clone();
        for (index, result) in results.iter().enumerate() {
            self.show_result(ui, index, result, p);
        }

        // Keep gestures on unused space only. The result rows themselves stay
        // entirely available to scrolling and taps.
        let empty_rect = ui.available_rect_before_wrap();
        if empty_rect.is_positive() {
            let response = ui.interact(
                empty_rect,
                egui::Id::new("empty-results-gestures"),
                Sense::click_and_drag(),
            );
            self.handle_empty_area_gestures(&response, ui.ctx());
        }
    }

    fn show_result(&mut self, ui: &mut egui::Ui, index: usize, result: &SearchResult, p: Palette) {
        let rounded = if self.settings.enabled("pref-rounded-list") {
            14
        } else {
            5
        };
        let selected = index == self.selected_index;
        let height = match self.settings.value("results-size") {
            "small" => 48.0,
            "large" => 76.0,
            _ => 60.0,
        };
        let mut icon_check_rect = ui.available_rect_before_wrap();
        icon_check_rect.max.y = (icon_check_rect.min.y + height).min(icon_check_rect.max.y);
        let icon_texture =
            if !self.settings.enabled("icons-hide") && ui.is_rect_visible(icon_check_rect) {
                self.icon_texture(ui.ctx(), &result.entry)
            } else {
                None
            };
        let frame = egui::Frame::NONE
            .fill(if selected {
                p.hover
            } else {
                Color32::TRANSPARENT
            })
            .corner_radius(CornerRadius::same(rounded))
            .inner_margin(egui::Margin::symmetric(
                if self.settings.enabled("large-result-list-margins") {
                    18
                } else {
                    10
                },
                6,
            ))
            .stroke(if selected {
                Stroke::new(1.0_f32, p.accent)
            } else {
                Stroke::NONE
            });
        let shown = frame.show(ui, |ui| {
            ui.set_min_height(height - 12.0);
            ui.horizontal(|ui| {
                if !self.settings.enabled("icons-hide") {
                    let initial = result
                        .entry
                        .name
                        .chars()
                        .next()
                        .unwrap_or('?')
                        .to_uppercase()
                        .to_string();
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
                    if let Some(texture_id) = icon_texture {
                        ui.painter().image(
                            texture_id,
                            rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        ui.painter().circle_filled(
                            rect.center(),
                            19.0,
                            if result.entry.is_favorite {
                                p.accent
                            } else {
                                p.surface
                            },
                        );
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            initial,
                            FontId::proportional(18.0),
                            if result.entry.is_favorite {
                                p.bg
                            } else {
                                p.text
                            },
                        );
                    }
                }
                ui.add_space(5.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&result.entry.name).size(16.0).color(
                        if result.match_type == MatchType::Exact && !self.query.is_empty() {
                            p.accent
                        } else {
                            p.text
                        },
                    ));
                    if self.settings.enabled("subicon-visible") && !result.entry.comment.is_empty()
                    {
                        ui.label(RichText::new(&result.entry.comment).size(11.0).color(p.dim));
                    }
                    if self.settings.enabled("tags-visible") && !result.entry.tags.is_empty() {
                        ui.label(
                            RichText::new(
                                result
                                    .entry
                                    .tags
                                    .iter()
                                    .map(|t| format!("#{t}"))
                                    .collect::<Vec<_>>()
                                    .join("  "),
                            )
                            .size(10.0)
                            .color(p.accent),
                        );
                    }
                });
            });
        });
        let response = shown.response.interact(Sense::click());
        if response.clicked() {
            self.selected_index = index;
            self.launch_result(index);
        }
        if response.hovered() {
            self.selected_index = index;
        }
        let mut toggle = false;
        let mut edit = false;
        let mut launch = false;
        let mut reset_rank = false;
        let mut toggle_history_exclusion = false;
        let excluded_from_history = self.is_excluded_from_history(&result.entry);
        if response.long_touched() {
            egui::Popup::open_id(&response.ctx, egui::Popup::default_response_id(&response));
        }
        response.context_menu(|ui| {
            ui.set_min_width(210.0);
            ui.label(RichText::new(&result.entry.name).strong());
            ui.separator();
            if ui.button("Open").clicked() {
                launch = true;
                ui.close();
            }
            if ui
                .button(if result.entry.is_favorite {
                    "Remove from favorites"
                } else {
                    "Add to favorites"
                })
                .clicked()
            {
                toggle = true;
                ui.close();
            }
            if ui.button("Edit tags").clicked() {
                edit = true;
                ui.close();
            }
            if ui.button("Reset usage rank").clicked() {
                reset_rank = true;
                ui.close();
            }
            if ui
                .button(if excluded_from_history {
                    "Include in history"
                } else {
                    "Exclude from history"
                })
                .clicked()
            {
                toggle_history_exclusion = true;
                ui.close();
            }
            ui.label(
                RichText::new("Long-press any app for actions")
                    .small()
                    .color(p.dim),
            );
        });
        if launch {
            self.launch_result(index);
        }
        if toggle {
            self.toggle_favorite_exec(&result.entry.exec);
        }
        if reset_rank {
            let exec = result.entry.exec.clone();
            self.history.reset_rank(&exec);
            self.save_history();
            self.reload_apps();
            self.set_status("Usage rank reset");
        }
        if toggle_history_exclusion {
            let entry = result.entry.clone();
            self.set_excluded_from_history(&entry, !excluded_from_history);
            self.set_status(if excluded_from_history {
                format!("{} returns to history", entry.name)
            } else {
                format!("{} hidden from history", entry.name)
            });
            self.update_results();
        }
        if edit {
            self.editing_tags = Some(result.entry.exec.clone());
            self.tag_input = result.entry.tags.join(", ");
        }
        if self.editing_tags.as_deref() == Some(result.entry.exec.as_str()) {
            self.show_tag_editor(ui, &result.entry.exec, p);
        }
        ui.add_space(2.0);
    }

    fn show_tag_editor(&mut self, ui: &mut egui::Ui, exec: &str, p: Palette) {
        egui::Frame::NONE
            .fill(p.surface)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.tag_input)
                            .hint_text("comma-separated tags")
                            .desired_width(ui.available_width() - 70.0),
                    );
                    if ui.button("Save").clicked()
                        || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    {
                        let tags = self
                            .tag_input
                            .split(',')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_owned)
                            .collect();
                        self.history.set_tags(exec, tags);
                        self.save_history();
                        self.editing_tags = None;
                        self.reload_apps();
                    }
                })
            });
    }
}
