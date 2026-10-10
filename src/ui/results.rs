use super::icons::{self, IconCache};
use super::{Palette, RissApp};
use crate::search::{self, MatchType, SearchResult};
use eframe::egui::{
    self, pos2, Color32, CornerRadius, FontId, Rect, Response, RichText, Sense, Stroke, Vec2,
};
use std::sync::Arc;

/// Gap between rows. With [`RowStyle::height`] it forms the fixed pitch the
/// virtualised list measures itself in.
const ROW_SPACING: f32 = 2.0;

/// Height reserved for one line of app names, …
const NAME_LINE: f32 = 21.0;
/// … for the subtitle line, …
const SUB_LINE: f32 = 15.0;
/// … and for the tag line. Rows are sized from these so that text never has to
/// wrap, which is what lets the list measure itself without laying out.
const TAG_LINE: f32 = 14.0;

/// Row metrics derived from the appearance settings. Every row shares one
/// height, which is what allows the list to lay out and paint only the rows
/// that are on screen rather than a hundred rows per frame while you scroll.
#[derive(Clone, Copy)]
struct RowStyle {
    height: f32,
    radius: CornerRadius,
    h_margin: f32,
    icon: f32,
    show_sub: bool,
    show_tags: bool,
    highlight_exact: bool,
}

/// What the user asked a row to do.
///
/// Rows are drawn while the result list is borrowed, so a row cannot rebuild
/// the list — or launch an app, which refreshes history — as a side effect.
/// Rows record what should happen and [`RissApp::apply_row_actions`] runs it
/// once the list is finished. That indirection is what replaced the
/// `self.results.clone()` that used to run on every frame.
enum RowAction {
    Launch(usize),
    ToggleFavorite(String),
    ResetRank(String),
    ExcludeFromHistory { name: String, exec: String },
    IncludeInHistory { name: String, exec: String },
    EditTags { exec: String, tags: Vec<String> },
}

/// Everything a row is drawn against. Bundling it keeps [`RowContext::draw`]
/// from taking a mutable `RissApp`, whose borrow would collide with reading
/// `self.results`.
struct RowContext<'a> {
    style: RowStyle,
    palette: Palette,
    icons: &'a mut IconCache,
    selected: &'a mut usize,
    actions: &'a mut Vec<RowAction>,
    follow_selection: bool,
}

impl RowContext<'_> {
    fn draw(&mut self, ui: &mut egui::Ui, index: usize, result: &SearchResult, excluded: bool) {
        let size = Vec2::new(ui.available_width(), self.style.height);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());

        let mut selected = *self.selected == index;
        if response.clicked() {
            *self.selected = index;
            selected = true;
            self.actions.push(RowAction::Launch(index));
        }
        // Hovering selects, which is what Enter then launches. While a pointer
        // is down the user is *scrolling*, so the highlight must not chase the
        // finger across the list.
        if !selected && response.hovered() && !response.ctx.input(|i| i.pointer.any_down()) {
            *self.selected = index;
            selected = true;
        }
        if response.long_touched() {
            egui::Popup::open_id(&response.ctx, egui::Popup::default_response_id(&response));
        }
        if selected && self.follow_selection {
            // Arrow keys move the selection off screen otherwise.
            response.scroll_to_me(Some(egui::Align::Center));
        }

        if ui.is_rect_visible(rect) {
            self.paint(ui, rect, result, selected);
        }
        self.show_menu(result, index, &response, excluded);
    }

    fn paint(&mut self, ui: &mut egui::Ui, rect: Rect, result: &SearchResult, selected: bool) {
        let style = self.style;
        let p = self.palette;
        let entry = &result.entry;
        let painter = ui.painter();

        if selected {
            painter.rect_filled(rect, style.radius, p.hover);
            painter.rect_stroke(
                rect,
                style.radius,
                Stroke::new(1.0_f32, p.accent),
                egui::StrokeKind::Inside,
            );
        }

        let mut x = rect.min.x + style.h_margin;
        if style.icon > 0.0 {
            let icon_rect = Rect::from_min_size(
                pos2(x, rect.center().y - style.icon * 0.5),
                Vec2::splat(style.icon),
            );
            match self.icons.texture(ui.ctx(), entry) {
                Some(texture_id) => {
                    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                    painter.image(texture_id, icon_rect, uv, Color32::WHITE);
                }
                None => {
                    // Either the app has no icon or its decode is still queued.
                    let favorite = entry.is_favorite;
                    painter.circle_filled(
                        icon_rect.center(),
                        style.icon * 0.5,
                        if favorite { p.accent } else { p.surface },
                    );
                    painter.text(
                        icon_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        icons::initial(&entry.name),
                        FontId::proportional(18.0),
                        if favorite { p.bg } else { p.text },
                    );
                }
            }
            x = icon_rect.max.x + 8.0;
        }

        let max_width = (rect.max.x - style.h_margin - x).max(16.0);
        let name_color = if style.highlight_exact && result.match_type == MatchType::Exact {
            p.accent
        } else {
            p.text
        };
        let name = line(ui, &entry.name, 16.0, name_color, max_width);
        let sub = if style.show_sub && !entry.comment.is_empty() {
            Some(line(ui, &entry.comment, 11.0, p.dim, max_width))
        } else {
            None
        };
        let tags = if style.show_tags && !entry.tags.is_empty() {
            Some(line(ui, &tag_line(&entry.tags), 10.0, p.accent, max_width))
        } else {
            None
        };

        // The text block is centred in the row, like the icon beside it.
        let name_height = name.size().y;
        let sub_height = sub.as_ref().map_or(0.0, |galley| galley.size().y);
        let total = name_height + sub_height + tags.as_ref().map_or(0.0, |galley| galley.size().y);
        let top = (rect.center().y - total * 0.5).max(rect.min.y + 2.0);
        let left = pos2(x, top);
        painter.galley(left, name, name_color);
        if let Some(sub) = sub {
            painter.galley(pos2(x, top + name_height), sub, p.dim);
        }
        if let Some(tags) = tags {
            painter.galley(pos2(x, top + name_height + sub_height), tags, p.accent);
        }
    }

    fn show_menu(
        &mut self,
        result: &SearchResult,
        index: usize,
        response: &Response,
        excluded: bool,
    ) {
        let entry = &result.entry;
        let dim = self.palette.dim;
        response.context_menu(|ui| {
            ui.set_min_width(210.0);
            ui.label(RichText::new(&entry.name).strong());
            ui.separator();
            if ui.button("Open").clicked() {
                self.actions.push(RowAction::Launch(index));
                ui.close();
            }
            if ui
                .button(if entry.is_favorite {
                    "Remove from favorites"
                } else {
                    "Add to favorites"
                })
                .clicked()
            {
                self.actions
                    .push(RowAction::ToggleFavorite(entry.exec.clone()));
                ui.close();
            }
            if ui.button("Edit tags").clicked() {
                self.actions.push(RowAction::EditTags {
                    exec: entry.exec.clone(),
                    tags: entry.tags.clone(),
                });
                ui.close();
            }
            if ui.button("Reset usage rank").clicked() {
                self.actions.push(RowAction::ResetRank(entry.exec.clone()));
                ui.close();
            }
            if ui
                .button(if excluded {
                    "Include in history"
                } else {
                    "Exclude from history"
                })
                .clicked()
            {
                let (name, exec) = (entry.name.clone(), entry.exec.clone());
                self.actions.push(if excluded {
                    RowAction::IncludeInHistory { name, exec }
                } else {
                    RowAction::ExcludeFromHistory { name, exec }
                });
                ui.close();
            }
            ui.label(
                RichText::new("Long-press any app for actions")
                    .small()
                    .color(dim),
            );
        });
    }
}

/// Lay out `text` on a single line, elided with an ellipsis when it is too
/// wide. Eliding instead of wrapping keeps rows uniform in height.
fn line(ui: &egui::Ui, text: &str, size: f32, color: Color32, max_width: f32) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(
        text.to_owned(),
        FontId::proportional(size),
        color,
        max_width,
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    ui.painter().layout_job(job)
}

/// `#tag  #tag` for the tag line of a row.
fn tag_line(tags: &[String]) -> String {
    let mut line = String::with_capacity(tags.iter().map(|tag| tag.len() + 4).sum());
    for (index, tag) in tags.iter().enumerate() {
        if index > 0 {
            line.push_str("  ");
        }
        line.push('#');
        line.push_str(tag);
    }
    line
}

impl RissApp {
    /// Height and padding of one result row, from the settings.
    fn row_style(&self) -> RowStyle {
        let minimum = match self.settings.value("results-size") {
            "small" => 44.0,
            "large" => 72.0,
            _ => 56.0,
        };
        let show_sub = self.settings.enabled("subicon-visible");
        let show_tags = self.settings.enabled("tags-visible");
        let icon = if self.settings.enabled("icons-hide") {
            0.0
        } else {
            40.0
        };
        let text = NAME_LINE
            + if show_sub { SUB_LINE } else { 0.0 }
            + if show_tags { TAG_LINE } else { 0.0 };
        RowStyle {
            // Text decides the height; the configured size only sets a floor.
            height: (text.max(icon) + 12.0).max(minimum),
            radius: CornerRadius::same(if self.settings.enabled("pref-rounded-list") {
                14
            } else {
                5
            }),
            h_margin: if self.settings.enabled("large-result-list-margins") {
                18.0
            } else {
                10.0
            },
            icon,
            show_sub,
            show_tags,
            highlight_exact: !self.query.is_empty(),
        }
    }

    pub(super) fn show_favorites(&mut self, ui: &mut egui::Ui, p: Palette) {
        let icon_size = if self.settings.enabled("large-favorites-bar") {
            54.0
        } else {
            44.0
        };
        let hide_icons = self.settings.enabled("icons-hide");
        let transparent = self.settings.enabled("transparent-favorites");
        let mut launch_exec = None;
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                // Iterating `apps` keeps this borrow-light: the old code cloned
                // every favorite, including its tags and categories, per frame.
                for app in self.apps.iter().filter(|app| app.is_favorite) {
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::splat(icon_size), Sense::click());
                    let response = response.on_hover_text(&app.name);
                    let texture = if hide_icons || !ui.is_rect_visible(rect) {
                        None
                    } else {
                        self.icons.texture(ui.ctx(), app)
                    };
                    if let Some(texture_id) = texture {
                        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                        ui.painter().image(texture_id, rect, uv, Color32::WHITE);
                    } else {
                        if !transparent {
                            ui.painter()
                                .circle_filled(rect.center(), icon_size * 0.5, p.surface);
                        }
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            icons::initial(&app.name),
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
        ui.spacing_mut().item_spacing.y = ROW_SPACING;

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

        let viewport = ui.available_rect_before_wrap();
        if self.results.is_empty() {
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
            // There is nothing to scroll, so the whole area carries gestures.
            let free = ui.available_rect_before_wrap();
            self.show_gestures(ui, free);
            return;
        }

        let style = self.row_style();
        let rows = self.results.len();
        let content_height = (style.height + ROW_SPACING) * rows as f32 - ROW_SPACING;
        let scrollable = content_height > viewport.height() + 1.0;
        let excluded = self.history_excluded_set();
        let follow_selection = self.list_follow_selection;
        let mut actions: Vec<RowAction> = Vec::new();

        let mut area = egui::ScrollArea::vertical()
            .id_salt("results")
            .auto_shrink([false, false])
            .drag_to_scroll(true);
        if self.list_scroll_to_top {
            // A new query or a new list (all apps vs. history) starts at the
            // top: the saved offset belongs to the list that is gone now.
            area = area.vertical_scroll_offset(0.0);
        }

        {
            let results = &self.results;
            let mut row = RowContext {
                style,
                palette: p,
                icons: &mut self.icons,
                selected: &mut self.selected_index,
                actions: &mut actions,
                follow_selection,
            };
            area.show_rows(ui, style.height, rows, |ui, range| {
                for index in range {
                    let Some(result) = results.get(index) else {
                        continue;
                    };
                    let is_excluded = excluded.contains(&result.entry.exec);
                    row.draw(ui, index, result, is_excluded);
                }
            });
        }
        self.list_scroll_to_top = false;
        self.list_follow_selection = false;

        // Gestures live on the free space below the list, and only while there
        // is nothing to scroll: an overlay above the rows would eat the drags
        // the scroll area needs, and that drag is exactly the swipe that is
        // meant to scroll.
        if !scrollable {
            let top = (viewport.min.y + content_height).min(viewport.max.y);
            let free = Rect::from_min_max(pos2(viewport.min.x, top), viewport.max);
            self.show_gestures(ui, free);
        }

        self.apply_row_actions(actions);
        if let Some(exec) = self.editing_tags.clone() {
            self.show_tag_editor(ui, &exec, p);
        }
    }

    /// Swipes and taps over the launcher's free space.
    fn show_gestures(&mut self, ui: &mut egui::Ui, rect: Rect) {
        if !rect.is_positive() {
            return;
        }
        let response = ui.interact(
            rect,
            egui::Id::new("empty-results-gestures"),
            Sense::click_and_drag(),
        );
        self.handle_empty_area_gestures(&response, ui.ctx());
    }

    fn apply_row_actions(&mut self, actions: Vec<RowAction>) {
        for action in actions {
            match action {
                RowAction::Launch(index) => self.launch_result(index),
                RowAction::ToggleFavorite(exec) => self.toggle_favorite_exec(&exec),
                RowAction::ResetRank(exec) => {
                    self.history.reset_rank(&exec);
                    self.save_history();
                    self.refresh_apps_in_place();
                    self.set_status("Usage rank reset");
                }
                RowAction::ExcludeFromHistory { name, exec } => {
                    self.set_excluded_from_history(&exec, &name, true);
                    self.set_status(format!("{name} hidden from history"));
                }
                RowAction::IncludeInHistory { name, exec } => {
                    self.set_excluded_from_history(&exec, &name, false);
                    self.set_status(format!("{name} returns to history"));
                }
                RowAction::EditTags { exec, tags } => {
                    self.tag_input = tags.join(", ");
                    self.editing_tags = Some(exec);
                }
            }
        }
    }

    fn show_tag_editor(&mut self, ui: &mut egui::Ui, exec: &str, p: Palette) {
        let mut save = false;
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
                        save = true;
                    }
                });
            });
        if save {
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
            self.refresh_apps_in_place();
        }
    }
}
