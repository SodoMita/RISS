//! Favorites management: the actions behind a favorite icon in the bar and the
//! "Manage favorites" screen.
//!
//! Modelled on KISS (`fr.neamar.kiss`, master): the bar answers a long press
//! with the very popup menu a search row shows
//! (`forwarder/Favorites.onLongClick` -> `Result.getPopupMenu`), and that menu
//! offers "Add to favorites" or "Remove favorite" depending on
//! `DataHandler.hasFavorite`, calling `DataHandler.addToFavorites` /
//! `removeFromFavorites`. Favorites are an ordered id list, `getFavorites()`
//! skips ids whose pojo is gone, and order is persisted by
//! `DataHandler.setFavoritePositions`, which clamps the target index.
//!
//! One difference in mechanism, not behaviour: KISS reorders by dragging
//! icons sideways through an `ItemTouchHelper` on a `RecyclerView`
//! (`forwarder/Favorites`, long-press start, popup dismissed on move,
//! positions saved on release). egui has no such helper, and the bar is a
//! horizontal `ScrollArea` that owns plain swipes for scrolling, so
//! [`RissApp::track_favorite_drag`] follows the raw pointer after a long
//! press arms it — same gesture, same order of events, no scroll stolen.
//! **Move left** / **Move right** stay in the menu for one-handed and
//! keyboard-only use.
use super::{Palette, RissApp, Screen};
#[cfg(target_os = "android")]
use crate::android_app_entry::AppEntry;
#[cfg(not(target_os = "android"))]
use crate::app_entry::AppEntry;
use eframe::egui::{self, CornerRadius, RichText, Vec2};

/// A favorite the user is dragging along the bar. Mirrors the long-press drag
/// KISS gets from `ItemTouchHelper` in `forwarder/Favorites` (its
/// `ItemMoveCallback` keeps the default long-press start, dismisses the popup
/// once the item moves, and persists the order when the drag ends).
pub(super) struct FavDrag {
    /// Exec command of the favorite under the pointer.
    exec: String,
    /// Pointer x the current slot was measured from.
    anchor_x: f32,
    /// Set once the drag has actually changed the order, so the release knows
    /// there is something to persist.
    moved: bool,
}

/// Action picked from a favorite, applied once the frame is drawn so the app
/// list is never mutated while widgets are being laid out.
pub(super) enum FavoriteAction {
    /// Launch the app behind the favorite.
    Open(String),
    /// Take the app off the favorites bar.
    Remove(String),
    /// Shift the favorite along the bar; negative moves it left.
    Move { exec: String, delta: isize },
    /// Open the tag editor for the favorite.
    EditTags(String),
    /// Open the "Manage favorites" screen.
    Manage,
}

/// State of one slot of the favorites bar. Excluded and uninstalled favorites
/// stay listed so they can still be removed instead of being unreachable.
pub(super) enum FavoriteState {
    /// Installed and part of the visible app list.
    Available(AppEntry),
    /// Installed, but hidden by the excluded-apps list.
    Excluded(AppEntry),
    /// No installed app answers to this exec any more.
    Missing,
}

impl FavoriteState {
    fn entry(&self) -> Option<&AppEntry> {
        match self {
            Self::Available(entry) | Self::Excluded(entry) => Some(entry),
            Self::Missing => None,
        }
    }

    /// Why the favorites bar cannot draw this favorite, if it cannot.
    fn note(&self) -> Option<&'static str> {
        match self {
            Self::Excluded(_) => Some("hidden by the excluded-apps list"),
            Self::Missing => Some("no installed app matches"),
            Self::Available(_) => None,
        }
    }
}

impl RissApp {
    /// Resolve one favorites-bar slot against the installed apps.
    pub(super) fn favorite_state(&self, exec: &str) -> FavoriteState {
        if let Some(entry) = self.apps.iter().find(|a| a.exec == exec) {
            return FavoriteState::Available(entry.clone());
        }
        if let Some(entry) = self.all_apps.iter().find(|a| a.exec == exec) {
            return FavoriteState::Excluded(entry.clone());
        }
        FavoriteState::Missing
    }

    /// Name to show for a favorite, falling back to its exec command when the
    /// app is gone.
    pub(super) fn favorite_display_name(&self, exec: &str) -> String {
        match self.favorite_state(exec).entry() {
            Some(entry) => entry.name.clone(),
            None => exec.to_string(),
        }
    }

    /// Favorites the bar can draw, in bar order. Apps that were uninstalled or
    /// excluded keep their slot in `history.favorites` but are skipped here.
    pub(super) fn favorite_bar_entries(&self) -> Vec<AppEntry> {
        self.history
            .favorites
            .iter()
            .filter_map(|exec| match self.favorite_state(exec) {
                FavoriteState::Available(entry) => Some(entry),
                FavoriteState::Excluded(_) | FavoriteState::Missing => None,
            })
            .collect()
    }

    /// Apply an action chosen from a favorites context menu or manager row.
    pub(super) fn apply_favorite_action(&mut self, action: FavoriteAction) {
        match action {
            FavoriteAction::Open(exec) => self.launch_exec(&exec),
            FavoriteAction::Remove(exec) => self.remove_favorite_exec(&exec),
            FavoriteAction::Move { exec, delta } => self.move_favorite_exec(&exec, delta),
            FavoriteAction::EditTags(exec) => {
                let tags = self.history.get_tags(&exec).join(", ");
                if self.screen != Screen::Favorites {
                    self.open_favorites_manager(Screen::Launcher);
                }
                self.tag_input = tags;
                self.editing_tags = Some(exec);
            }
            FavoriteAction::Manage => {
                self.open_favorites_manager(Screen::Launcher);
                self.editing_tags = None;
            }
        }
    }

    /// Show the favorites manager, remembering where to return to.
    pub(super) fn open_favorites_manager(&mut self, from: Screen) {
        self.favorites_return = if from == Screen::Favorites {
            Screen::Launcher
        } else {
            from
        };
        self.favorites_query.clear();
        self.screen = Screen::Favorites;
    }

    /// Leave the favorites manager and refresh the launcher lists.
    pub(super) fn close_favorites_manager(&mut self) {
        self.editing_tags = None;
        self.favorites_query.clear();
        self.screen = self.favorites_return;
        self.reload_apps();
    }

    /// Add an app to the end of the favorites bar.
    pub(super) fn add_favorite_exec(&mut self, exec: &str) {
        if self.history.add_favorite(exec) {
            let name = self.favorite_display_name(exec);
            self.save_history();
            self.reload_apps();
            self.set_status(format!("{name} was added to favorites"));
        }
    }

    /// Remove one favorite — KISS' "Remove favorite" (`menu_favorites_remove`).
    pub(super) fn remove_favorite_exec(&mut self, exec: &str) {
        if self.history.remove_favorite(exec) {
            let name = self.favorite_display_name(exec);
            self.save_history();
            self.reload_apps();
            self.set_status(format!("{name} was removed from favorites"));
        }
    }

    /// Reorder one favorite; `delta` is negative to move it left.
    pub(super) fn move_favorite_exec(&mut self, exec: &str, delta: isize) {
        if self.history.move_favorite_by(exec, delta) {
            self.save_history();
            self.reload_apps();
        }
    }

    /// Follow a favorite being dragged sideways along the bar.
    ///
    /// A long press arms the drag — the same gesture that opens the KISS
    /// popup menu, which is why the menu is closed here as soon as the item
    /// actually moves, the way KISS' `ItemMoveCallback.onMove` dismisses it.
    /// The bar icons keep `Sense::click()`, so a plain swipe still belongs to
    /// the horizontal `ScrollArea` and scrolls the bar; only a long-press
    /// drag reorders. The order is changed in memory on every slot crossed
    /// and written to disk once, on release, like KISS persisting positions
    /// when the drag settles.
    ///
    /// `pitch` is one slot's width (icon plus spacing). Returns `true` while
    /// this favorite is being dragged, and on the release frame when the drag
    /// moved something, so the caller can suppress the launch tap.
    pub(super) fn track_favorite_drag(
        &mut self,
        ctx: &egui::Context,
        exec: &str,
        popup_id: egui::Id,
        pitch: f32,
        response: &egui::Response,
    ) -> bool {
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        let released = ctx.input(|i| i.pointer.any_released());
        let down = ctx.input(|i| i.pointer.any_pressed());
        if released || !down {
            let moved = self.fav_drag.take().is_some_and(|drag| drag.moved);
            if moved {
                self.save_history();
                self.set_status("Favorites reordered");
            }
            return moved;
        }
        if self.fav_drag.is_none() {
            if response.long_touched() {
                if let Some(pointer) = pointer {
                    self.fav_drag = Some(FavDrag {
                        exec: exec.to_string(),
                        anchor_x: pointer.x,
                        moved: false,
                    });
                }
            }
            return false;
        }
        let (dragged, anchor_x) = match &self.fav_drag {
            Some(drag) => (drag.exec.clone(), drag.anchor_x),
            None => return false,
        };
        if dragged != exec {
            return false;
        }
        let Some(pointer) = pointer else {
            return true;
        };
        if pitch <= 0.0 {
            return true;
        }
        let slots = ((pointer.x - anchor_x) / pitch).round() as isize;
        if slots != 0 && self.history.move_favorite_by(exec, slots) {
            if let Some(drag) = self.fav_drag.as_mut() {
                drag.anchor_x += slots as f32 * pitch;
                drag.moved = true;
            }
            self.reload_apps();
            // The item moved, so its menu is no longer the point.
            egui::Popup::close_id(ctx, popup_id);
            ctx.request_repaint();
        }
        true
    }

    /// Drop every favorite, reporting how many went.
    pub(super) fn clear_favorites(&mut self) {
        let count = self.history.clear_favorites();
        self.save_history();
        self.reload_apps();
        let message = if count == 0 {
            "No favorites to clear".to_owned()
        } else {
            format!("{count} favorites cleared")
        };
        self.set_status(message);
    }
}

impl RissApp {
    /// The "Manage favorites" screen: reorder, delete and tag the bar, and
    /// search for apps to add.
    pub(super) fn show_favorites_screen(&mut self, ctx: &egui::Context, p: Palette) {
        egui::TopBottomPanel::top("favorites-header")
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
                        .on_hover_text("Back")
                        .clicked()
                    {
                        self.close_favorites_manager();
                    }
                    ui.heading("Favorites");
                });
                let hint = "◀ ▶ reorder · ✕ removes · long-press a bar favorite for the same menu";
                ui.label(RichText::new(hint).size(11.0).color(p.dim));
                ui.add_space(6.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.favorites_query)
                        .id(egui::Id::new("riss-favorites-search"))
                        .hint_text("Search apps to add to favorites")
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
                        self.show_favorite_rows(ui, p);
                        self.show_favorite_candidates(ui, p);
                        ui.add_space(30.0);
                    });
            });
    }

    /// The favorites bar itself, as an ordered, editable list.
    fn show_favorite_rows(&mut self, ui: &mut egui::Ui, p: Palette) {
        let count = self.history.favorites.len();
        ui.add_space(12.0);
        ui.label(
            RichText::new(format!("FAVORITES BAR ({count})"))
                .size(12.0)
                .strong()
                .color(p.accent),
        );
        ui.add_space(3.0);
        if count == 0 {
            self.surface_frame(ui, p, |ui| {
                ui.label(RichText::new("No favorites yet").strong());
                let empty = "Search below, or long-press any app and pick “Add to favorites”.";
                ui.label(RichText::new(empty).size(11.0).color(p.dim));
            });
            return;
        }

        let mut action = None;
        // Cloned so the row callbacks can mutate the app through `self`.
        let favorites = self.history.favorites.clone();
        for (index, exec) in favorites.iter().enumerate() {
            action = self.show_favorite_row(ui, index, exec, p).or(action);
            if self.editing_tags.as_deref() == Some(exec.as_str()) {
                self.show_tag_editor(ui, exec, p);
            }
            ui.add_space(3.0);
        }
        if let Some(action) = action {
            self.apply_favorite_action(action);
        }

        ui.add_space(6.0);
        let clear = egui::Button::new("Clear all favorites");
        if ui.add_sized([ui.available_width(), 42.0], clear).clicked() {
            self.clear_favorites();
        }
    }

    /// One row of the favorites manager, returning the action the user picked.
    fn show_favorite_row(
        &self,
        ui: &mut egui::Ui,
        index: usize,
        exec: &str,
        p: Palette,
    ) -> Option<FavoriteAction> {
        let last = index + 1 == self.history.favorites.len();
        let state = self.favorite_state(exec);
        let left_hint = if index == 0 {
            "Already first"
        } else {
            "Move left"
        };
        let right_hint = if last { "Already last" } else { "Move right" };
        let mut action = None;
        self.surface_frame(ui, p, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{}.", index + 1))
                        .size(13.0)
                        .color(p.dim),
                );
                ui.vertical(|ui| {
                    let name = state
                        .entry()
                        .map(|entry| entry.name.clone())
                        .unwrap_or_else(|| exec.to_string());
                    ui.label(RichText::new(name).size(15.0).color(p.text));
                    if let Some(note) = state.note() {
                        ui.label(RichText::new(note).size(10.0).color(p.dim));
                    }
                });
                // Right-to-left, so the first button added ends up furthest
                // right: remove, tags, then the two reorder arrows.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if row_button(ui, "✕", "Remove favorite") {
                        action = Some(FavoriteAction::Remove(exec.to_string()));
                    }
                    if row_button(ui, "🏷", "Edit tags") {
                        action = Some(FavoriteAction::EditTags(exec.to_string()));
                    }
                    if row_button(ui, "▶", right_hint) {
                        action = Some(FavoriteAction::Move {
                            exec: exec.to_string(),
                            delta: 1,
                        });
                    }
                    if row_button(ui, "◀", left_hint) {
                        action = Some(FavoriteAction::Move {
                            exec: exec.to_string(),
                            delta: -1,
                        });
                    }
                });
            });
        });
        action
    }

    /// Search results for the "add to favorites" field.
    fn show_favorite_candidates(&mut self, ui: &mut egui::Ui, p: Palette) {
        ui.add_space(14.0);
        ui.label(
            RichText::new("ADD FAVORITES")
                .size(12.0)
                .strong()
                .color(p.accent),
        );
        ui.add_space(3.0);
        let query = self.favorites_query.trim().to_owned();
        if query.is_empty() {
            let hint = "Type a name, tag or category to find apps to favorite.";
            ui.label(RichText::new(hint).size(11.0).color(p.dim));
            return;
        }
        let matches = self.search_engine.search(&query, &self.apps, 12);
        if matches.is_empty() {
            let text = format!("No app matches “{query}”");
            let note = RichText::new(text).size(11.0).color(p.dim);
            ui.label(note);
            return;
        }
        let mut add = None;
        for result in matches {
            let already = result.entry.is_favorite;
            let glyph = if already { "★" } else { "☆" };
            let hint = if already {
                "Already a favorite"
            } else {
                "Add to favorites"
            };
            self.surface_frame(ui, p, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&result.entry.name).size(15.0).color(p.text));
                        ui.label(RichText::new(&result.entry.exec).size(10.0).color(p.dim));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if row_button(ui, glyph, hint) && !already {
                            add = Some(result.entry.exec.clone());
                        }
                    });
                });
            });
            ui.add_space(3.0);
        }
        if let Some(exec) = add {
            self.add_favorite_exec(&exec);
        }
    }

    /// Card background shared by every row of this screen.
    fn surface_frame<R>(
        &self,
        ui: &mut egui::Ui,
        p: Palette,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        egui::Frame::NONE
            .fill(p.surface)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, add_contents)
            .inner
    }
}

/// Small square icon button used by the favorites rows.
fn row_button(ui: &mut egui::Ui, glyph: &str, tooltip: &str) -> bool {
    ui.add(
        egui::Button::new(RichText::new(glyph).size(16.0))
            .frame(false)
            .min_size(Vec2::splat(40.0)),
    )
    .on_hover_text(tooltip)
    .clicked()
}
