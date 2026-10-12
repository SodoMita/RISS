use super::favorites::FavoriteAction;
use super::widgets::{self, Icon};
use super::{Palette, RissApp};
#[cfg(target_os = "android")]
use crate::android_app_entry::AppEntry;
#[cfg(not(target_os = "android"))]
use crate::app_entry::AppEntry;
use crate::search::{self, MatchType, SearchResult};
use eframe::egui::{self, Color32, CornerRadius, FontId, RichText, Sense, Stroke, Vec2};
#[cfg(not(target_os = "android"))]
use std::time::Instant;

impl RissApp {
    pub(super) fn show_favorites(&mut self, ui: &mut egui::Ui, p: Palette) {
        let favorites = self.favorite_bar_entries();
        if favorites.is_empty() {
            return;
        }

        let icon_size = if self.settings.enabled("large-favorites-bar") {
            54.0
        } else {
            44.0
        };
        let font_size = if icon_size > 44.0 { 22.0 } else { 18.0 };
        let mut launch_exec = None;
        let mut action = None;
        let mut app_info_entry: Option<AppEntry> = None;
        let mut store_entry: Option<AppEntry> = None;
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                // One slot of the bar: what a drag has to cover to swap.
                let pitch = icon_size + ui.spacing().item_spacing.x;
                for (index, app) in favorites.iter().enumerate() {
                    let shown_name = self.display_name(app);
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::splat(icon_size), Sense::click());
                    let response = response.on_hover_text(shown_name.as_str());
                    let icon_texture =
                        if !self.settings.enabled("icons-hide") && ui.is_rect_visible(rect) {
                            self.icons.texture(ui.ctx(), app)
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
                            FontId::proportional(font_size),
                            p.text,
                        );
                    } else {
                        widgets::paint_app_badge(ui, rect, &shown_name, icon_size, &p);
                    }
                    let popup_id = egui::Popup::default_response_id(&response);
                    let dragging =
                        self.track_favorite_drag(ui.ctx(), &app.exec, popup_id, pitch, &response);
                    if dragging {
                        ui.painter().circle_stroke(
                            rect.center(),
                            icon_size * 0.5 + 2.0,
                            Stroke::new(2.0_f32, p.accent),
                        );
                    }
                    if response.clicked() && !dragging {
                        launch_exec = Some(app.exec.clone());
                    }
                    // KISS answers a long press on a favorite with the same
                    // popup menu a search row shows (forwarder/Favorites
                    // .onLongClick -> Result.getPopupMenu), whose
                    // "Remove favorite" item calls
                    // DataHandler.removeFromFavorites. Holding that same long
                    // press and dragging reorders the bar; Move and Edit tags
                    // in the menu are RISS additions.
                    // No explicit open for long-press: `secondary_clicked`
                    // already includes touch holds, so the context menu below
                    // opens with its pointer anchor on its own.
                    let exec = app.exec.clone();
                    response.context_menu(|ui| {
                        ui.set_min_width(210.0);
                        ui.label(RichText::new(shown_name.as_str()).strong());
                        ui.separator();
                        if ui.button("Open").clicked() {
                            action = Some(FavoriteAction::Open(exec.clone()));
                            ui.close();
                        }
                        if ui.button("Remove favorite").clicked() {
                            action = Some(FavoriteAction::Remove(exec.clone()));
                            ui.close();
                        }
                        if index > 0 && ui.button("Move left").clicked() {
                            action = Some(FavoriteAction::Move {
                                exec: exec.clone(),
                                delta: -1,
                            });
                            ui.close();
                        }
                        if index + 1 < favorites.len() && ui.button("Move right").clicked() {
                            action = Some(FavoriteAction::Move {
                                exec: exec.clone(),
                                delta: 1,
                            });
                            ui.close();
                        }
                        if ui.button("Edit tags").clicked() {
                            action = Some(FavoriteAction::EditTags(exec.clone()));
                            ui.close();
                        }
                        if ui.button("App info").clicked() {
                            app_info_entry = Some(app.clone());
                            ui.close();
                        }
                        if ui.button("View in store").clicked() {
                            store_entry = Some(app.clone());
                            ui.close();
                        }
                        if ui.button("Manage favorites…").clicked() {
                            action = Some(FavoriteAction::Manage);
                            ui.close();
                        }
                        let hint = RichText::new("Long-press a favorite for actions")
                            .small()
                            .color(p.dim);
                        ui.label(hint);
                    });
                }
            });
        });
        if let Some(exec) = launch_exec {
            self.launch_exec(&exec);
        }
        if let Some(action) = action {
            self.apply_favorite_action(action);
        }
        if let Some(entry) = app_info_entry {
            match entry.open_app_info() {
                Ok(()) => self.set_status(format!("Opened app info for {}", entry.name)),
                Err(e) => self.set_status(e),
            }
        }
        if let Some(entry) = store_entry {
            match entry.view_in_store() {
                Ok(()) => self.set_status(format!("Opened store for {}", entry.name)),
                Err(e) => self.set_status(e),
            }
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
        let editing = self.editing_tags.is_some()
            || self.editing_alias.is_some()
            || self.editing_icon.is_some();
        if editing {
            egui::ScrollArea::vertical()
                .id_salt("kiss-results")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let results = self.results.clone();
                    for (index, result) in results.iter().enumerate() {
                        self.show_result(ui, index, result, p);
                    }
                    self.gesture_below_rows(ui);
                });
            self.drain_deferred();
            return;
        }

        // KISS binds only the visible children of its ListView. IconCache
        // already stops a flick from decoding every PNG; this stops the flick
        // from *building* every row. That second half is the scroll latency.
        let row_height = self.result_row_pitch();
        let total = self.results.len();
        self.reveal_selection(ui, row_height);
        let generation = self.list_generation;
        let results = std::mem::take(&mut self.results);
        let output = egui::ScrollArea::vertical()
            .id_salt("kiss-results")
            .auto_shrink([false, false])
            .scroll_source(egui::containers::scroll_area::ScrollSource {
                scroll_bar: true,
                drag: true,
                mouse_wheel: false,
            })
            .show_rows(ui, row_height, total, |ui, range| {
                for index in range {
                    let width = ui.available_width();
                    ui.allocate_ui(Vec2::new(width, row_height), |ui| {
                        self.show_result(ui, index, &results[index], p);
                    });
                }
            });
        if self.list_generation == generation {
            self.results = results;
        }
        self.drive_wheel_fling(ui, output.inner_rect);
        self.gesture_band(ui, output.inner_rect, row_height, total);
        self.drain_deferred();
    }

    fn result_row_pitch(&self) -> f32 {
        let mut height = match self.settings.value("results-size") {
            "small" => 56.0,
            "large" => 88.0,
            _ => 72.0,
        };
        if self.settings.enabled("subicon-visible") {
            height += 16.0;
        }
        if self.settings.enabled("tags-visible") {
            height += 16.0;
        }
        height
    }

    fn reveal_selection(&mut self, ui: &mut egui::Ui, row_height: f32) {
        if !self.scroll_selection {
            return;
        }
        self.scroll_selection = false;
        let id = ui.make_persistent_id(egui::Id::new("kiss-results"));
        let Some(mut state) = egui::containers::scroll_area::State::load(ui.ctx(), id) else {
            return;
        };
        let pitch = row_height + ui.spacing().item_spacing.y;
        let row_y = self.selected_index as f32 * pitch;
        let view_h = ui.available_height().max(pitch);
        if row_y < state.offset.y {
            state.offset.y = (row_y - pitch).max(0.0);
        } else if row_y + row_height > state.offset.y + view_h {
            state.offset.y = (row_y + row_height + pitch - view_h).max(0.0);
        }
        state.store(ui.ctx(), id);
    }

    fn drive_wheel_fling(&mut self, ui: &mut egui::Ui, list_rect: egui::Rect) {
        use super::fling::{absorb_fling, step_fling, wheel_gesture};
        let id = ui.make_persistent_id(egui::Id::new("kiss-results"));
        if ui.rect_contains_pointer(list_rect) {
            let delta_y = ui.input(|input| input.smooth_scroll_delta.y);
            if delta_y != 0.0 {
                self.fling_velocity = absorb_fling(self.fling_velocity, wheel_gesture(delta_y));
                ui.input_mut(|input| {
                    input.smooth_scroll_delta = egui::Vec2::ZERO;
                });
            }
        }
        let dt = ui.input(|input| input.stable_dt);
        let (velocity, delta) = step_fling(self.fling_velocity, dt);
        self.fling_velocity = velocity;
        if delta == 0.0 {
            return;
        }
        if let Some(mut state) = egui::containers::scroll_area::State::load(ui.ctx(), id) {
            state.offset.y = (state.offset.y + delta).max(0.0);
            state.store(ui.ctx(), id);
            ui.ctx().request_repaint();
        }
    }

    fn gesture_below_rows(&mut self, ui: &mut egui::Ui) {
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

    fn gesture_band(
        &mut self,
        ui: &mut egui::Ui,
        list_rect: egui::Rect,
        row_height: f32,
        total: usize,
    ) {
        let spacing = ui.spacing().item_spacing.y;
        let content_h = (row_height + spacing) * total as f32;
        if list_rect.height() <= content_h + 8.0 {
            return;
        }
        let mut band = list_rect;
        band.min.y += content_h;
        if !band.is_positive() {
            return;
        }
        let response = ui.interact(
            band,
            egui::Id::new("empty-results-gestures"),
            Sense::click_and_drag(),
        );
        self.handle_empty_area_gestures(&response, ui.ctx());
    }

    fn drain_deferred(&mut self) {
        let deferred = std::mem::take(&mut self.deferred);
        for action in deferred {
            match action {
                super::Deferred::Activate(index) => self.activate(index),
            }
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
            self.icons.texture(ui.ctx(), &result.entry)
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
        let mut kebab: Option<egui::Response> = None;
        // Where ★ and ⋮ really ended up; the row's click area must stop there.
        let mut actions_rect: Option<egui::Rect> = None;
        let actions_width = if result.entry.is_virtual() { 0.0 } else { 60.0 };
        let shown = frame.show(ui, |ui| {
            ui.set_min_width(ui.available_width());
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
                // The text column claims its full width (issue #18):
                // `allocate_ui` advances by what the content used, so a short
                // name used to pull ★ and ⋮ into the middle of the row, under
                // the row's click area, where a tap launched the app instead.
                let spacing = if actions_width > 0.0 {
                    ui.spacing().item_spacing.x
                } else {
                    0.0
                };
                let text_width = (ui.available_width() - actions_width - spacing).max(20.0);
                ui.allocate_ui(Vec2::new(text_width, height - 12.0), |ui| {
                    ui.set_min_width(text_width);
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
                    let actions = ui.allocate_ui(Vec2::new(actions_width, height - 12.0), |ui| {
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
                            kebab = Some(widgets::icon_button(
                                ui,
                                Icon::Kebab,
                                28.0,
                                p.dim,
                                &p,
                                "More actions",
                            ));
                        });
                    });
                    actions_rect = Some(actions.response.rect);
                }
            });
        });
        // The row's click area is registered after ★ and ⋮, so it sits on
        // top of them and would win every tap it covers. It stops where the
        // buttons actually start, measured, not where the layout was meant
        // to put them.
        let response = {
            let mut click_rect = shown.response.rect;
            if let Some(actions) = actions_rect {
                click_rect.max.x = click_rect.max.x.min(actions.min.x);
            }
            ui.interact(click_rect, shown.response.id, Sense::click())
        };
        if response.clicked() {
            self.selected_index = index;
            // `activate` reads `self.results`, which the virtual list has
            // taken for the duration of the paint.
            self.deferred.push(super::Deferred::Activate(index));
        }
        if response.hovered() {
            self.selected_index = index;
        }
        let mut edit = false;
        let mut manage_favorites = false;
        let mut rename = false;
        let mut add_custom_icon = false;
        let mut app_info = false;
        let mut view_in_store = false;
        let mut launch = false;
        let mut reset_rank = false;
        let mut toggle_history_exclusion = false;
        let mut toggle_exclusion = false;
        let mut copy_name = false;
        let mut copy_exec = false;
        let mut open_desktop = false;
        let mut uninstall = false;
        let mut pin: Option<u8> = None;
        let excluded_from_history = self.is_excluded_from_history(&result.entry);
        let excluded_from_search = self.is_excluded_from_search(&result.entry);
        // A second press of "Uninstall" within the confirmation window runs
        // the removal; anything else just arms the confirmation.
        #[cfg(not(target_os = "android"))]
        let uninstall_pending = self.pending_uninstall.as_ref().is_some_and(|(exec, at)| {
            *exec == result.entry.exec && at.elapsed() <= super::UNINSTALL_CONFIRM_WINDOW
        });
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
        if !result.entry.is_virtual() {
            // One menu, two doors (issue #65): right-click / long-press on
            // the row, or the kebab button. `Popup::menu` toggles on the
            // button's click and anchors to it, so no pointer position has
            // to be stored by hand (the old `open_id` call stored none, and
            // the menu never rendered).
            let mut menu_content = |ui: &mut egui::Ui| {
                ui.set_min_width(210.0);
                ui.label(RichText::new(&shown_name).strong());
                ui.separator();
                if ui.button("Open").clicked() {
                    launch = true;
                    ui.close();
                }
                let favorite_label = if result.entry.is_favorite {
                    "Remove favorite"
                } else {
                    "Add to favorites"
                };
                if ui.button(favorite_label).clicked() {
                    toggle = true;
                    ui.close();
                }
                if ui.button("Rename").clicked() {
                    rename = true;
                    ui.close();
                }
                if ui.button("Edit tags").clicked() {
                    edit = true;
                    ui.close();
                }
                if ui.button("Manage favorites…").clicked() {
                    manage_favorites = true;
                    ui.close();
                }
                if ui.button("Add custom icon").clicked() {
                    add_custom_icon = true;
                    ui.close();
                }
                if ui.button("App info").clicked() {
                    app_info = true;
                    ui.close();
                }
                if ui.button("View in store").clicked() {
                    view_in_store = true;
                    ui.close();
                }
                if ui
                    .button(if excluded_from_search {
                        "Restore to results"
                    } else {
                        "Exclude app"
                    })
                    .clicked()
                {
                    toggle_exclusion = true;
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
                ui.menu_button("Pin to number", |ui| {
                    for (key, label) in &pin_labels {
                        if ui.button(label.clone()).clicked() {
                            pin = Some(*key);
                            ui.close();
                        }
                    }
                });
                if ui.button("Reset usage rank").clicked() {
                    reset_rank = true;
                    ui.close();
                }
                let uninstall_enabled = result.entry.can_uninstall();
                #[cfg(target_os = "android")]
                let uninstall_label = "Uninstall";
                #[cfg(not(target_os = "android"))]
                let uninstall_label = if uninstall_pending {
                    "Confirm uninstall"
                } else {
                    "Uninstall"
                };
                if ui
                    .add_enabled(uninstall_enabled, egui::Button::new(uninstall_label))
                    .clicked()
                {
                    uninstall = true;
                    #[cfg(target_os = "android")]
                    ui.close();
                    #[cfg(not(target_os = "android"))]
                    if uninstall_pending {
                        ui.close();
                    }
                }
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
                ui.label(
                    RichText::new("Long-press any app for actions")
                        .small()
                        .color(p.dim),
                );
            };
            response.context_menu(|ui| menu_content(ui));
            if let Some(kebab) = &kebab {
                egui::Popup::menu(kebab).show(|ui| menu_content(ui));
            }
        }
        if launch {
            self.deferred.push(super::Deferred::Activate(index));
        }
        if toggle {
            self.toggle_favorite_exec(&result.entry.exec);
        }
        if rename {
            self.editing_alias = Some(result.entry.exec.clone());
            self.alias_input = shown_name.clone();
            self.editing_tags = None;
            self.editing_icon = None;
        }
        if edit {
            self.editing_tags = Some(result.entry.exec.clone());
            self.tag_input = result.entry.tags.join(", ");
            self.editing_alias = None;
            self.editing_icon = None;
        }
        if add_custom_icon {
            self.editing_icon = Some(result.entry.exec.clone());
            self.icon_input = self
                .history
                .custom_icon(&result.entry.exec)
                .cloned()
                .unwrap_or_else(|| result.entry.icon.clone());
            self.editing_tags = None;
            self.editing_alias = None;
        }
        if app_info {
            let entry = result.entry.clone();
            match entry.open_app_info() {
                Ok(()) => {
                    self.set_status(format!("Opened app info for {}", shown_name));
                }
                Err(error) => self.set_status(error),
            }
        }
        if view_in_store {
            let entry = result.entry.clone();
            match entry.view_in_store() {
                Ok(()) => {
                    self.set_status(format!("Opened store for {}", shown_name));
                }
                Err(error) => self.set_status(error),
            }
        }
        if toggle_exclusion {
            let entry = result.entry.clone();
            self.set_excluded_from_search(&entry, !excluded_from_search);
            self.set_status(if excluded_from_search {
                format!("{} restored to results", shown_name)
            } else {
                format!("{} hidden from results", shown_name)
            });
            self.refresh_from_history();
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
        if manage_favorites {
            self.apply_favorite_action(FavoriteAction::Manage);
        }
        if uninstall {
            let entry = result.entry.clone();
            #[cfg(target_os = "android")]
            {
                match entry.uninstall() {
                    Ok(()) => {
                        self.set_status(format!("Opening uninstaller for {}", shown_name));
                    }
                    Err(error) => self.set_status(error),
                }
            }
            #[cfg(not(target_os = "android"))]
            {
                if uninstall_pending {
                    self.pending_uninstall = None;
                    match entry.uninstall() {
                        Ok(()) => {
                            self.reload_apps();
                            self.set_status(format!("Uninstalling {}", shown_name));
                        }
                        Err(error) => self.set_status(error),
                    }
                } else {
                    match entry.uninstall_preview() {
                        Ok(summary) => {
                            self.pending_uninstall = Some((entry.exec.clone(), Instant::now()));
                            self.set_status(format!("{summary} — press Confirm uninstall"));
                        }
                        Err(error) => self.set_status(error),
                    }
                }
            }
        }
        if reset_rank {
            let exec = result.entry.exec.clone();
            self.history.reset_rank(&exec);
            self.save_history();
            self.refresh_from_history();
            self.set_status("Usage rank reset");
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
        if self.editing_icon.as_deref() == Some(result.entry.exec.as_str()) {
            self.show_icon_editor(ui, &result.entry.exec, p);
        }
        ui.add_space(2.0);
    }

    pub(super) fn show_tag_editor(&mut self, ui: &mut egui::Ui, exec: &str, p: Palette) {
        egui::Frame::NONE
            .fill(p.surface)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.tag_input)
                            .hint_text("comma-separated tags")
                            .desired_width((ui.available_width() - 110.0).max(100.0)),
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
                        self.refresh_from_history();
                    }
                    if ui.button("Cancel").clicked() {
                        self.editing_tags = None;
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
                            .hint_text("app name")
                            .desired_width((ui.available_width() - 160.0).max(100.0)),
                    );
                    if ui.button("Save").clicked()
                        || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    {
                        let alias = self.alias_input.trim().to_owned();
                        self.history.set_alias(exec, alias);
                        self.save_history();
                        self.editing_alias = None;
                        self.refresh_from_history();
                        self.set_status("App renamed");
                    }
                    if ui.button("Reset").clicked() {
                        self.history.set_alias(exec, String::new());
                        self.save_history();
                        self.editing_alias = None;
                        self.refresh_from_history();
                        self.set_status("Name reset to default");
                    }
                    if ui.button("Cancel").clicked() {
                        self.editing_alias = None;
                    }
                })
            });
    }

    fn show_icon_editor(&mut self, ui: &mut egui::Ui, exec: &str, p: Palette) {
        egui::Frame::NONE
            .fill(p.surface)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.icon_input)
                            .hint_text("icon name, package or file path")
                            .desired_width((ui.available_width() - 160.0).max(100.0)),
                    );
                    if ui.button("Save").clicked()
                        || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    {
                        let icon = self.icon_input.trim().to_owned();
                        self.history.set_custom_icon(exec, icon);
                        self.save_history();
                        self.icons.clear();
                        self.editing_icon = None;
                        self.refresh_from_history();
                        self.set_status("Custom icon set");
                    }
                    if ui.button("Reset").clicked() {
                        self.history.set_custom_icon(exec, String::new());
                        self.save_history();
                        self.icons.clear();
                        self.editing_icon = None;
                        self.refresh_from_history();
                        self.set_status("Icon reset to default");
                    }
                    if ui.button("Cancel").clicked() {
                        self.editing_icon = None;
                    }
                })
            });
    }
}

/// Kebab menus (issue #65) rely on `Popup::menu` toggling open when its
/// button is clicked. Pins that contract headless: a press and release on
/// the button must render the menu content on the click frame.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_menu_opens_on_click() {
        let ctx = egui::Context::default();
        let press_at = egui::pos2(100.0, 120.0);
        let button_rect =
            egui::Rect::from_min_size(press_at - egui::vec2(5.0, 5.0), egui::vec2(40.0, 20.0));
        let frame_input = |pressed: Option<bool>| {
            let mut events = vec![egui::Event::PointerMoved(press_at)];
            if let Some(pressed) = pressed {
                events.push(egui::Event::PointerButton {
                    pos: press_at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            }
        };
        // Sticky across passes: `run` may execute the UI closure more than
        // once per frame.
        let mut menu_shown = false;
        // Render, press, release: hit-testing needs the widget registered
        // before the press lands.
        for pressed in [None, Some(true), Some(false)] {
            let _ = ctx.run(frame_input(pressed), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let button = ui.put(button_rect, egui::Button::new("\u{22ee}"));
                    egui::Popup::menu(&button).show(|_| {
                        menu_shown = true;
                    });
                });
            });
        }
        assert!(menu_shown, "button menu never rendered after a click");
    }
}
