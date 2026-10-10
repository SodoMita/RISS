use super::widgets::{self, Icon};
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
                    let shown_name = self.display_name(&app);
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::splat(icon_size), Sense::click());
                    let response = response.on_hover_text(shown_name.as_str());
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
                    } else if self.settings.enabled("transparent-favorites") {
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            widgets::first_letter(&shown_name),
                            FontId::proportional(if icon_size > 44.0 { 22.0 } else { 18.0 }),
                            p.text,
                        );
                    } else {
                        widgets::paint_app_badge(ui, rect, &shown_name, icon_size, &p);
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
        if self.settings.enabled("enable-calculator") {
            if let Some(value) = search::try_calculate(&self.query) {
                let answer = value.trim_start_matches("= ").trim().to_string();
                let mut copy_answer = None;
                egui::Frame::NONE
                    .fill(p.surface)
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(egui::Margin::same(14))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(value.clone()).size(24.0).color(p.accent));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("Copy").clicked() {
                                        copy_answer = Some(answer.clone());
                                    }
                                },
                            );
                        });
                    });
                if let Some(text) = copy_answer {
                    if crate::providers::copy_to_clipboard(&text) {
                        self.set_status(format!("Copied {}", text));
                    } else {
                        self.set_status(format!("{} — clipboard helper not found", text));
                    }
                }
                ui.add_space(6.0);
            }
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
        let shown_name = self.display_name(&result.entry);
        let mut icon_check_rect = ui.available_rect_before_wrap();
        icon_check_rect.max.y = (icon_check_rect.min.y + height).min(icon_check_rect.max.y);
        let icon_texture = if !self.settings.enabled("icons-hide")
            && !result.entry.is_virtual()
            && ui.is_rect_visible(icon_check_rect)
        {
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
        let mut toggle = false;
        let mut open_menu = false;
        let actions_width = if result.entry.is_virtual() { 0.0 } else { 60.0 };
        let shown = frame.show(ui, |ui| {
            ui.set_min_height(height - 12.0);
            ui.horizontal(|ui| {
                if !self.settings.enabled("icons-hide") {
                    if let Some(texture_id) = icon_texture {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
                        ui.painter().image(
                            texture_id,
                            rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        widgets::app_badge(ui, &result.entry, &shown_name, 40.0, &p);
                    }
                }
                ui.add_space(5.0);
                let text_width = (ui.available_width() - actions_width).max(20.0);
                ui.allocate_ui(Vec2::new(text_width, height - 12.0), |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&shown_name).size(16.0).color(
                            if result.match_type == MatchType::Exact && !self.query.is_empty() {
                                p.accent
                            } else {
                                p.text
                            },
                        ));
                        if self.settings.enabled("subicon-visible")
                            && !result.entry.comment.is_empty()
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
                if actions_width > 0.0 {
                    ui.allocate_ui(Vec2::new(actions_width, height - 12.0), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.horizontal(|ui| {
                            let star = if result.entry.is_favorite {
                                Icon::StarFilled
                            } else {
                                Icon::Star
                            };
                            let star_color = if result.entry.is_favorite {
                                p.accent
                            } else {
                                p.dim
                            };
                            if widgets::icon_button(
                                ui,
                                star,
                                28.0,
                                star_color,
                                &p,
                                if result.entry.is_favorite {
                                    "Remove from favorites"
                                } else {
                                    "Add to favorites"
                                },
                            )
                            .clicked()
                            {
                                toggle = true;
                            }
                            if widgets::icon_button(
                                ui,
                                Icon::Kebab,
                                28.0,
                                p.dim,
                                &p,
                                "More actions",
                            )
                            .clicked()
                            {
                                open_menu = true;
                            }
                        });
                    });
                }
            });
        });
        // The row click area stops before the action buttons so that pressing
        // ★ or ⋮ does not also launch the application.
        let response = {
            let mut click_rect = shown.response.rect;
            click_rect.max.x -= actions_width;
            ui.interact(click_rect, shown.response.id, Sense::click())
        };
        if open_menu {
            egui::Popup::open_id(&response.ctx, egui::Popup::default_response_id(&response));
        }
        if response.clicked() {
            self.selected_index = index;
            self.activate(index);
        }
        if response.hovered() {
            self.selected_index = index;
        }
        let mut edit = false;
        let mut rename = false;
        let mut launch = false;
        let mut reset_rank = false;
        let mut toggle_history_exclusion = false;
        let mut toggle_exclusion = false;
        let mut copy_name = false;
        let mut copy_exec = false;
        let mut open_desktop = false;
        let mut pin: Option<u8> = None;
        let excluded_from_history = self.is_excluded_from_history(&result.entry);
        let excluded_from_search = self.is_excluded_from_search(&result.entry);
        // Precomputed so the context-menu closure needs no access to `self`.
        let mut pin_labels: Vec<(u8, String)> = Vec::new();
        for key in 1..=9u8 {
            let bound = self.history.shortcut_for(key).map(String::as_str)
                == Some(result.entry.exec.as_str());
            let label = if bound {
                format!("✓ {}", key)
            } else {
                key.to_string()
            };
            pin_labels.push((key, label));
        }
        if response.long_touched() && !result.entry.is_virtual() {
            egui::Popup::open_id(&response.ctx, egui::Popup::default_response_id(&response));
        }
        if !result.entry.is_virtual() {
            response.context_menu(|ui| {
                ui.set_min_width(210.0);
                ui.label(RichText::new(&shown_name).strong());
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
                if ui.button("Rename…").clicked() {
                    rename = true;
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
                if ui
                    .button(if excluded_from_search {
                        "Restore to results"
                    } else {
                        "Hide from results"
                    })
                    .clicked()
                {
                    toggle_exclusion = true;
                    ui.close();
                }
                ui.menu_button("Pin to number", |ui| {
                    for (key, label) in &pin_labels {
                        if ui.button(label.clone()).clicked() {
                            pin = Some(*key);
                            ui.close();
                        }
                    }
                });
                ui.separator();
                if ui.button("Copy name").clicked() {
                    copy_name = true;
                    ui.close();
                }
                if ui.button("Copy command").clicked() {
                    copy_exec = true;
                    ui.close();
                }
                if !result.entry.desktop_file.as_os_str().is_empty()
                    && ui.button("Open .desktop file").clicked()
                {
                    open_desktop = true;
                    ui.close();
                }
            });
        }
        if launch {
            self.activate(index);
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
                format!("{} returns to history", shown_name)
            } else {
                format!("{} hidden from history", shown_name)
            });
            self.update_results();
        }
        if toggle_exclusion {
            let entry = result.entry.clone();
            self.set_excluded_from_search(&entry, !excluded_from_search);
            self.set_status(if excluded_from_search {
                format!("{} restored to results", shown_name)
            } else {
                format!("{} hidden from results", shown_name)
            });
            self.reload_apps();
        }
        if edit {
            self.editing_tags = Some(result.entry.exec.clone());
            self.tag_input = result.entry.tags.join(", ");
        }
        if rename {
            self.editing_alias = Some(result.entry.exec.clone());
            self.alias_input = self
                .history
                .alias(&result.entry.exec)
                .cloned()
                .unwrap_or_default();
        }
        if copy_name {
            if crate::providers::copy_to_clipboard(&shown_name) {
                self.set_status(format!("Copied {}", shown_name));
            } else {
                self.set_status("Clipboard helper not found");
            }
        }
        if copy_exec {
            if crate::providers::copy_to_clipboard(&result.entry.exec) {
                self.set_status(format!("Copied {}", result.entry.exec));
            } else {
                self.set_status("Clipboard helper not found");
            }
        }
        if open_desktop {
            let path = result.entry.desktop_file.clone();
            match crate::providers::open_path(&path) {
                Ok(()) => self.set_status(format!("Opened {}", path.display())),
                Err(error) => self.set_status(error),
            }
        }
        if let Some(key) = pin {
            let exec = result.entry.exec.clone();
            self.history.set_shortcut(key, &exec);
            self.save_history();
            self.set_status(format!("Pinned to {}", key));
        }
        if self.editing_tags.as_deref() == Some(result.entry.exec.as_str()) {
            self.show_tag_editor(ui, &result.entry.exec, p);
        }
        if self.editing_alias.as_deref() == Some(result.entry.exec.as_str()) {
            self.show_alias_editor(ui, &result.entry.exec, p);
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

    fn show_alias_editor(&mut self, ui: &mut egui::Ui, exec: &str, p: Palette) {
        egui::Frame::NONE
            .fill(p.surface)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.alias_input)
                            .hint_text("new name — empty restores the original")
                            .desired_width(ui.available_width() - 70.0),
                    );
                    if ui.button("Save").clicked()
                        || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    {
                        let alias = self.alias_input.clone();
                        self.history.set_alias(exec, alias);
                        self.save_history();
                        self.editing_alias = None;
                        self.reload_apps();
                    }
                })
            });
    }
}
