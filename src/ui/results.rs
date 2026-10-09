//! Result list, grid layout and favourites bar.

use super::{FavAction, RowAction, ViewState};
use crate::search::{self, MatchType, SearchResult};
use crate::settings::EdgePosition;
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Sense, Stroke, Vec2};

#[cfg(target_os = "android")]
use crate::android_app_entry::AppEntry;
#[cfg(not(target_os = "android"))]
use crate::app_entry::AppEntry;

/// What happened while the result list was drawn.
pub struct ResultsOutcome {
    pub rows: Vec<egui::Rect>,
    pub hovered: Option<usize>,
    pub actions: Vec<RowAction>,
}

/// What happened while the favourites bar was drawn.
pub struct FavoritesOutcome {
    pub hovered: Option<usize>,
    pub actions: Vec<FavAction>,
}

fn name_of(state: &ViewState, entry: &AppEntry) -> String {
    search::display_name(entry, &state.history.aliases)
}

fn subtitle_of(result: &SearchResult) -> String {
    if !result.entry.comment.is_empty() {
        result.entry.comment.clone()
    } else {
        result.entry.categories.join(" · ")
    }
}

/// The whole result list, wrapped in a scroll area that keeps the keyboard
/// selection visible.
pub fn show_results(ui: &mut egui::Ui, state: &ViewState, height: f32) -> ResultsOutcome {
    let settings = state.settings;
    let mut outcome = ResultsOutcome {
        rows: Vec::new(),
        hovered: state.hovered,
        actions: Vec::new(),
    };

    let scroll_id = egui::Id::new("riss_results_scroll");
    let output = egui::ScrollArea::vertical()
        .id_salt(scroll_id)
        .max_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let count = state.results.len();
            if count == 0 {
                return;
            }
            let width = ui.available_width();
            let icon_size = settings.icon_size(count);
            if settings.use_grid(count, width) {
                let columns = settings.columns(width, count, icon_size).max(1);
                egui::Grid::new("riss_grid")
                    .num_columns(columns)
                    .spacing(Vec2::new(4.0, 4.0))
                    .show(ui, |ui| {
                        for (index, result) in state.results.iter().enumerate() {
                            let rect = render_cell(ui, index, result, state, &mut outcome);
                            outcome.rows.push(rect);
                            if (index + 1) % columns == 0 {
                                ui.end_row();
                            }
                        }
                    });
            } else {
                let spacing = settings.widget_spacing.clamp(0.0, 16.0);
                for (index, result) in state.results.iter().enumerate() {
                    let rect = render_row(ui, index, result, state, &mut outcome);
                    outcome.rows.push(rect);
                    ui.add_space(spacing);
                }
            }
        });

    let content_height = output.content_size.y;
    let viewport = output.inner_rect.height();
    let mut scroll_state = output.state;

    if state.scroll_to_selection {
        if let Some(rect) = outcome.rows.get(state.selected) {
            let offset = scroll_state.offset.y;
            let desired = if rect.top() < offset {
                rect.top() - 4.0
            } else if rect.bottom() > offset + viewport {
                rect.bottom() - viewport + 4.0
            } else {
                offset
            };
            let max_offset = (content_height - viewport).max(0.0);
            let clamped = desired.clamp(0.0, max_offset);
            if (clamped - offset).abs() > 0.5 {
                scroll_state.offset.y = clamped;
                scroll_state.store(state.ctx, scroll_id);
            }
        }
    }

    outcome
}

/// One row of the list.
fn render_row(
    ui: &mut egui::Ui,
    index: usize,
    result: &SearchResult,
    state: &ViewState,
    outcome: &mut ResultsOutcome,
) -> egui::Rect {
    let settings = state.settings;
    let palette = state.palette;
    let count = state.results.len();
    let width = ui.available_width();
    let height = settings.row_height(count);
    let icon_size = settings.icon_size(count);
    let name = name_of(state, &result.entry);
    let selected = index == state.selected;
    let mut hovered = state.hovered == Some(index);

    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
    hovered |= response.hovered();

    let pressed = response.is_pointer_button_down_on();
    let progress = if settings.press_feedback {
        widgets::press_progress(ui, &format!("riss_row_{}", index), pressed)
    } else {
        0.0
    };

    let mut fill = if selected {
        palette.selected
    } else if hovered {
        palette.hover
    } else {
        Color32::TRANSPARENT
    };
    // Calculator and copy results stand out with their own colour.
    let is_copy_row = matches!(result.action, crate::search::ResultAction::Copy { .. });
    if is_copy_row && !selected {
        fill = if hovered {
            widgets::mix(palette.calc_bg, palette.hover, 0.5)
        } else {
            palette.calc_bg
        };
    }
    if progress > 0.01 {
        fill = widgets::mix(fill, palette.accent, progress * 0.45);
    }
    let corner: u8 = if settings.rounded_list { 10 } else { 0 };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(corner), fill);

    if selected {
        let bar = egui::Rect::from_min_size(
            rect.left_top() + Vec2::new(2.0, height * 0.2),
            Vec2::new(3.0, height * 0.6),
        );
        ui.painter().rect_filled(bar, 2.0, palette.accent);
    }
    if settings.show_separators && index > 0 {
        ui.painter().line_segment(
            [rect.left_top(), rect.right_top()],
            Stroke::new(1.0, palette.border),
        );
    }

    let padding = if settings.large_result_list_margins { 12.0 } else { 6.0 };
    let inner = egui::Rect::from_min_max(
        rect.min + Vec2::new(padding, 4.0),
        rect.max - Vec2::new(padding, 4.0),
    );

    let number = shortcut_number(state, result);
    let show_number = settings.show_app_names && (index < 9 || number.is_some());
    let show_actions = settings.show_row_actions;
    let actions_width = if show_actions { 56.0 } else { 0.0 };
    let number_width = if show_number { 18.0 } else { 0.0 };
    let icon_width = if settings.hide_main_icons { 0.0 } else { icon_size + 8.0 };

    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.horizontal(|ui| {
            if settings.grid_position == EdgePosition::End {
                let _ = ui.allocate_ui(Vec2::new(actions_width, inner.height()), |ui| {
                    render_actions(ui, index, result, state, outcome)
                });
            }
            if show_number {
                ui.allocate_ui(Vec2::new(number_width, inner.height()), |ui| {
                    ui.vertical_centered(|ui| {
                        let text = number
                            .map(|key| key.to_string())
                            .unwrap_or_else(|| (index + 1).to_string());
                        let color = if number.is_some() {
                            palette.accent
                        } else {
                            palette.text_dim
                        };
                        let _ = ui.label(RichText::new(text).size(11.0).color(color));
                    });
                });
            }
            if icon_width > 0.0 {
                ui.allocate_ui(Vec2::new(icon_width, inner.height()), |ui| {
                    ui.vertical_centered(|ui| {
                        widgets::app_badge(ui, &result.entry, &name, icon_size, palette);
                    });
                });
            }
            ui.allocate_ui(
                Vec2::new(
                    (inner.width() - icon_width - number_width - actions_width).max(20.0),
                    inner.height(),
                ),
                |ui| render_text(ui, result, state),
            );
            if settings.grid_position == EdgePosition::Start {
                let _ = ui.allocate_ui(Vec2::new(actions_width, inner.height()), |ui| {
                    render_actions(ui, index, result, state, outcome)
                });
            }
        });
    });

    if response.hovered() {
        outcome.hovered = Some(index);
    }
    if response.secondary_clicked() {
        outcome.actions.push(RowAction::Menu(index));
    }
    if response.clicked() {
        outcome.actions.push(RowAction::Activate(index));
    }
    rect
}

/// The number bound to this result, if any (KISS' number shortcuts).
fn shortcut_number(state: &ViewState, result: &SearchResult) -> Option<u8> {
    (1..=9u8).find(|key| {
        state
            .history
            .shortcut_for(*key)
            .map(|exec| exec == &result.entry.exec)
            .unwrap_or(false)
    })
}

/// Title, subtitle and tags of a row.
fn render_text(ui: &mut egui::Ui, result: &SearchResult, state: &ViewState) {
    let palette = state.palette;
    let settings = state.settings;
    let name = name_of(state, &result.entry);
    let subtitle = subtitle_of(result);
    let accent = match result.match_type {
        MatchType::Exact | MatchType::Prefix => palette.accent,
        MatchType::Provider => palette.text_dim,
        _ => palette.text,
    };
    let title_color = if matches!(result.action, crate::search::ResultAction::Copy { .. }) {
        palette.calc_text
    } else if result.entry.is_favorite {
        palette.favorite
    } else {
        accent
    };

    ui.vertical_centered(|ui| {
        ui.horizontal_wrapped(|ui| {
            if settings.show_app_names {
                let _ = ui.add(
                    egui::Label::new(RichText::new(&name).size(14.0).strong().color(title_color))
                        .truncate(),
                );
            }
            if settings.show_subicons && !subtitle.is_empty() {
                if settings.show_app_names {
                    let _ = ui.label(RichText::new("·").size(11.0).color(palette.text_dim));
                }
                let _ = ui.add(
                    egui::Label::new(
                        RichText::new(&subtitle).size(11.0).color(palette.text_dim),
                    )
                    .truncate(),
                );
            }
        });
        if settings.show_launch_count && result.entry.launch_count > 0 {
            let _ = ui.label(
                RichText::new(format!("launched {} times", result.entry.launch_count))
                    .size(10.0)
                    .color(palette.text_dim),
            );
        }
        if settings.show_tags && settings.tags_visible && !result.entry.tags.is_empty() {
            ui.horizontal_wrapped(|ui| {
                for tag in result.entry.tags.iter().take(6) {
                    tag_label(ui, tag, palette);
                }
            });
        }
    });
}

fn tag_label(ui: &mut egui::Ui, tag: &str, palette: &crate::theme::Palette) {
    let text = RichText::new(format!("#{}", tag))
        .size(10.0)
        .color(palette.text_dim);
    egui::Frame::NONE
        .fill(palette.surface_alt)
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::symmetric(4, 1))
        .show(ui, |ui| {
            let _ = ui.label(text);
        });
}

/// Favourite star and tag buttons of a row.
fn render_actions(
    ui: &mut egui::Ui,
    index: usize,
    result: &SearchResult,
    state: &ViewState,
    outcome: &mut ResultsOutcome,
) {
    let palette = state.palette;
    let is_favorite = result.entry.is_favorite;
    ui.horizontal(|ui| {
        if widgets::icon_button(
            ui,
            if is_favorite {
                Icon::StarFilled
            } else {
                Icon::Star
            },
            24.0,
            if is_favorite {
                palette.favorite
            } else {
                palette.text_dim
            },
            palette,
            if is_favorite {
                "Remove from favorites"
            } else {
                "Add to favorites"
            },
        )
        .clicked()
        {
            outcome.actions.push(RowAction::ToggleFavorite(index));
        }
        if widgets::icon_button(ui, Icon::Tag, 24.0, palette.text_dim, palette, "Edit tags").clicked()
        {
            outcome.actions.push(RowAction::EditTags(index));
        }
    });
}

/// One cell of the adaptive grid.
fn render_cell(
    ui: &mut egui::Ui,
    index: usize,
    result: &SearchResult,
    state: &ViewState,
    outcome: &mut ResultsOutcome,
) -> egui::Rect {
    let settings = state.settings;
    let palette = state.palette;
    let count = state.results.len();
    let icon_size = settings.icon_size(count);
    let available = ui.available_width();
    let cell_width = available.max(40.0);
    let cell_height = (icon_size * 1.7).max(40.0);
    let name = name_of(state, &result.entry);
    let selected = index == state.selected;

    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(cell_width, cell_height), Sense::click_and_drag());

    let pressed = response.is_pointer_button_down_on();
    let progress = if settings.press_feedback {
        widgets::press_progress(ui, &format!("riss_cell_{}", index), pressed)
    } else {
        0.0
    };
    let mut fill = if selected {
        palette.selected
    } else if response.hovered() {
        palette.hover
    } else {
        Color32::TRANSPARENT
    };
    if progress > 0.01 {
        fill = widgets::mix(fill, palette.accent, progress * 0.45);
    }
    let corner: u8 = if settings.rounded_list { 10 } else { 0 };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(corner), fill);

    let badge_size = (icon_size * 0.72).min(cell_height * 0.6).max(18.0);
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.vertical_centered(|ui| {
            widgets::app_badge(ui, &result.entry, &name, badge_size, palette);
            if settings.show_app_names {
                let _ = ui.add(
                    egui::Label::new(RichText::new(&name).size(11.0).color(palette.text))
                        .truncate(),
                );
            }
        });
    });

    if response.hovered() {
        outcome.hovered = Some(index);
    }
    if response.secondary_clicked() {
        outcome.actions.push(RowAction::Menu(index));
    }
    if response.clicked() {
        outcome.actions.push(RowAction::Activate(index));
    }
    rect
}

/// The favourites bar: one tap launches, long press opens the menu.
pub fn show_favorites_bar(ui: &mut egui::Ui, state: &ViewState) -> FavoritesOutcome {
    let settings = state.settings;
    let palette = state.palette;
    let mut outcome = FavoritesOutcome {
        hovered: None,
        actions: Vec::new(),
    };
    if state.favorites.is_empty() {
        return outcome;
    }

    let height = if settings.large_favorites_bar { 62.0 } else { 46.0 };
    let bar_color = if settings.transparent_favorites_bar {
        Color32::TRANSPARENT
    } else {
        palette.surface
    };
    let corner: u8 = if settings.rounded_bars { 12 } else { 0 };

    egui::Frame::NONE
        .fill(bar_color)
        .corner_radius(CornerRadius::same(corner))
        .inner_margin(Margin::symmetric(8, 6))
        .stroke(Stroke::new(1.0, palette.border))
        .show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt(egui::Id::new("riss_favorites_scroll"))
                .auto_shrink([false, false])
                .max_height(height)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.horizontal(|ui| {
                        for (index, favorite) in state.favorites.iter().enumerate() {
                            render_favorite(ui, index, favorite, state, &mut outcome);
                        }
                    });
                });
        });

    outcome
}

fn render_favorite(
    ui: &mut egui::Ui,
    index: usize,
    result: &SearchResult,
    state: &ViewState,
    outcome: &mut FavoritesOutcome,
) {
    let settings = state.settings;
    let palette = state.palette;
    let name = name_of(state, &result.entry);
    let cell_width = if settings.large_favorites_bar { 74.0 } else { 58.0 };
    let height = if settings.large_favorites_bar { 50.0 } else { 34.0 };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(cell_width, height), Sense::click_and_drag());

    let selected = state.selected == index;
    let pressed = response.is_pointer_button_down_on();
    let progress = if settings.press_feedback {
        widgets::press_progress(ui, &format!("riss_fav_{}", index), pressed)
    } else {
        0.0
    };
    let mut fill = if selected {
        palette.selected
    } else if response.hovered() {
        palette.hover
    } else {
        Color32::TRANSPARENT
    };
    if progress > 0.01 {
        fill = widgets::mix(fill, palette.accent, progress * 0.45);
    }
    let corner: u8 = if settings.rounded_bars { 8 } else { 0 };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(corner), fill);

    let badge = (height * 0.62).min(28.0);
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        if settings.large_favorites_bar {
            ui.vertical_centered(|ui| {
                widgets::app_badge(ui, &result.entry, &name, badge, palette);
                if settings.show_app_names {
                    let _ = ui.add(
                        egui::Label::new(RichText::new(&name).size(9.0).color(palette.text))
                            .truncate(),
                    );
                }
            });
        } else {
            ui.horizontal_centered(|ui| {
                widgets::app_badge(ui, &result.entry, &name, badge, palette);
            });
        }
    });

    if response.hovered() {
        outcome.hovered = Some(index);
    }
    if response.secondary_clicked() {
        outcome.actions.push(FavAction::Menu(index));
    }
    if response.clicked() {
        outcome.actions.push(FavAction::Activate(index));
    }
}