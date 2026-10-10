#[cfg(target_os = "android")]
use crate::android_app_entry::{self as app_entry, AppEntry};
#[cfg(not(target_os = "android"))]
use crate::app_entry::{self, AppEntry};
use crate::history::{HistoryData, HistoryMode};
use crate::search::{self, MatchType, SearchEngine, SearchResult};
use crate::settings::{self, SettingKind, SettingsData};
use crate::storage;
use eframe::egui;
use egui::{Color32, CornerRadius, FontId, RichText, Sense, Stroke, Vec2};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
struct Palette {
    bg: Color32,
    surface: Color32,
    hover: Color32,
    text: Color32,
    dim: Color32,
    accent: Color32,
    border: Color32,
}

impl Palette {
    fn from_settings(settings: &SettingsData) -> Self {
        let light = settings.value("theme") == "light"
            || (settings.value("night-mode") == "light" && settings.value("theme") != "dark");
        let accent =
            parse_hex(settings.value("primary-color")).unwrap_or(Color32::from_rgb(137, 180, 250));
        if light {
            Self {
                bg: Color32::from_rgb(246, 247, 251),
                surface: Color32::WHITE,
                hover: Color32::from_rgb(230, 234, 244),
                text: Color32::from_rgb(30, 32, 40),
                dim: Color32::from_rgb(99, 104, 120),
                accent,
                border: Color32::from_rgb(210, 214, 224),
            }
        } else {
            Self {
                bg: Color32::from_rgb(20, 20, 28),
                surface: Color32::from_rgb(35, 35, 47),
                hover: Color32::from_rgb(51, 52, 68),
                text: Color32::from_rgb(232, 234, 245),
                dim: Color32::from_rgb(151, 155, 174),
                accent,
                border: Color32::from_rgb(67, 68, 84),
            }
        }
    }
}

fn parse_hex(value: &str) -> Option<Color32> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(hex, 16).ok()?;
    Some(Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Launcher,
    Settings,
}

pub struct RissApp {
    query: String,
    /// Every discovered app, including ones excluded from the lists.
    all_apps: Vec<AppEntry>,
    /// Apps shown in the normal lists (exclusions applied).
    apps: Vec<AppEntry>,
    results: Vec<SearchResult>,
    search_engine: SearchEngine,
    history: HistoryData,
    settings: SettingsData,
    selected_index: usize,
    screen: Screen,
    show_all_apps: bool,
    status_message: Option<(String, Instant)>,
    editing_tags: Option<String>,
    tag_input: String,
    settings_query: String,
    touch_start: Option<(egui::Pos2, f64)>,
    last_empty_tap: Option<(egui::Pos2, f64)>,
    startup_notes: Vec<String>,
}

impl RissApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (settings, mut startup_notes) = SettingsData::load();
        let (history, history_notes) = HistoryData::load();
        startup_notes.extend(history_notes);
        let mut app = Self {
            query: String::new(),
            all_apps: Vec::new(),
            apps: Vec::new(),
            results: Vec::new(),
            search_engine: SearchEngine::new(),
            history,
            settings,
            selected_index: 0,
            screen: Screen::Launcher,
            show_all_apps: false,
            status_message: None,
            editing_tags: None,
            tag_input: String::new(),
            settings_query: String::new(),
            touch_start: None,
            last_empty_tap: None,
            startup_notes,
        };
        app.reload_apps();
        app
    }

    fn reload_apps(&mut self) {
        let mut apps = app_entry::discover_apps();
        for builtin in app_entry::builtin_entries() {
            if !apps
                .iter()
                .any(|a| a.name.eq_ignore_ascii_case(&builtin.name))
            {
                apps.push(builtin);
            }
        }
        for app in &mut apps {
            app.launch_count = self.history.get_launch_count(&app.exec);
            app.last_launched = self.history.get_last_launched(&app.exec);
            app.is_favorite = self.history.is_favorite(&app.exec);
            let custom = self.history.get_tags(&app.exec);
            if !custom.is_empty() {
                app.tags = custom;
            }
        }
        self.all_apps = apps;
        // Excluded apps disappear from the normal lists; the "Search excluded
        // apps" toggle decides whether typed queries can still find them.
        let excluded: Vec<String> = self
            .settings
            .value("edit-excluded-apps")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_lowercase)
            .collect();
        self.apps = self
            .all_apps
            .iter()
            .filter(|a| {
                !excluded
                    .iter()
                    .any(|x| a.exec.eq_ignore_ascii_case(x) || a.name.eq_ignore_ascii_case(x))
            })
            .cloned()
            .collect();
        self.update_results();
    }

    fn update_results(&mut self) {
        let limit = self.settings.number("number-of-display-elements", 20);
        if !self.query.trim().is_empty() {
            // "Search excluded apps" lets typed queries find excluded apps;
            // they never appear in the normal lists either way.
            let pool = if self.settings.enabled("enable-excluded-apps") {
                &self.all_apps
            } else {
                &self.apps
            };
            self.results = self.search_engine.search(
                &self.query,
                pool,
                if limit == 0 { usize::MAX } else { limit },
            );
            if self.settings.enabled("exclude-favorites-apps") {
                self.results.retain(|r| !r.entry.is_favorite);
            }
        } else if self.show_all_apps {
            self.results = self
                .apps
                .iter()
                .cloned()
                .map(|entry| SearchResult {
                    entry,
                    score: 0,
                    match_type: MatchType::Fuzzy,
                })
                .collect();
            self.results.sort_by_key(|r| r.entry.name.to_lowercase());
        } else if self.settings.enabled("history-hide") {
            self.results.clear();
        } else {
            // Rank every candidate first, then apply the limit, so filters
            // like "hide favorites from history" do not shorten the list.
            self.results = self.search_engine.get_default_apps(&self.apps, usize::MAX);
            self.apply_history_ordering();
            let limit = if limit == 0 { usize::MAX } else { limit };
            self.results.truncate(limit);
        }
        self.selected_index = self
            .selected_index
            .min(self.results.len().saturating_sub(1));
    }

    /// Order the default (no-query) list according to the `history-mode`
    /// setting, hiding apps the user excluded from history. Ties are broken
    /// by name so ordering is deterministic.
    fn apply_history_ordering(&mut self) {
        if self.settings.enabled("exclude-favorites-history") {
            self.results.retain(|r| !r.entry.is_favorite);
        }
        let excluded = self.history_excluded_set();
        if !excluded.is_empty() {
            self.results.retain(|r| !excluded.contains(&r.entry.exec));
        }
        let mode = HistoryMode::from_key(self.settings.value("history-mode"));
        if mode == HistoryMode::Alphabetical {
            self.results.sort_by_key(|r| r.entry.name.to_lowercase());
            return;
        }
        let now = HistoryData::now_secs();
        self.results.sort_by(|a, b| {
            let score_a = self.history.rank_score(mode, &a.entry.exec, now);
            let score_b = self.history.rank_score(mode, &b.entry.exec, now);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a.entry
                        .name
                        .to_lowercase()
                        .cmp(&b.entry.name.to_lowercase())
                })
        });
    }

    /// Exec commands the user excluded from history recording/display.
    fn history_excluded_set(&self) -> std::collections::HashSet<String> {
        self.settings
            .value("edit-excluded-from-history-apps")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    }

    fn is_excluded_from_history(&self, entry: &AppEntry) -> bool {
        let excluded = self.settings.value("edit-excluded-from-history-apps");
        excluded
            .split(',')
            .map(str::trim)
            .any(|x| !x.is_empty() && (entry.exec == x || entry.name.eq_ignore_ascii_case(x)))
    }

    /// Add or remove an app from the `edit-excluded-from-history-apps` list.
    fn set_excluded_from_history(&mut self, entry: &AppEntry, excluded: bool) {
        let mut items: Vec<String> = self
            .settings
            .value("edit-excluded-from-history-apps")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        items.retain(|item| {
            !entry.exec.eq_ignore_ascii_case(item) && !entry.name.eq_ignore_ascii_case(item)
        });
        if excluded {
            items.push(entry.exec.clone());
        }
        self.settings.values.insert(
            "edit-excluded-from-history-apps".to_owned(),
            items.join(","),
        );
        self.save_settings();
    }

    fn launch_exec(&mut self, exec: &str) {
        // Search the full list so excluded apps reached through search can
        // still be launched.
        let Some(entry) = self.all_apps.iter().find(|a| a.exec == exec).cloned() else {
            return;
        };
        match entry.launch() {
            Ok(()) => {
                if !self.settings.enabled("freeze-history")
                    && self.settings.enabled("enable-app-history")
                    && !self.is_excluded_from_history(&entry)
                {
                    self.history.record_launch(exec);
                    self.save_history();
                }
                self.query.clear();
                self.show_all_apps = false;
                self.reload_apps();
                self.set_status(format!("Opened {}", entry.name));
            }
            Err(error) => self.set_status(error),
        }
    }

    fn launch_result(&mut self, index: usize) {
        if let Some(result) = self.results.get(index) {
            let exec = result.entry.exec.clone();
            self.launch_exec(&exec);
        }
    }

    fn toggle_favorite_exec(&mut self, exec: &str) {
        self.history.toggle_favorite(exec);
        self.save_history();
        self.reload_apps();
    }

    fn set_status(&mut self, text: impl Into<String>) {
        self.status_message = Some((text.into(), Instant::now()));
    }

    /// Persist history, surfacing write failures instead of discarding them.
    fn save_history(&mut self) {
        if let Err(err) = self.history.save() {
            log::error!("Failed to save history: {err}");
            self.set_status(format!("Could not save history: {err}"));
        }
    }

    /// Persist settings, surfacing write failures instead of discarding them.
    fn save_settings(&mut self) {
        if let Err(err) = self.settings.save() {
            log::error!("Failed to save settings: {err}");
            self.set_status(format!("Could not save settings: {err}"));
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
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
            self.launch_result(self.selected_index);
        }
    }

    fn perform_gesture(&mut self, action: &str, ctx: &egui::Context) {
        match action {
            "display-keyboard" => ctx.memory_mut(|m| m.request_focus(egui::Id::new("riss-search"))),
            "hide-keyboard" => ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("riss-search"))),
            "display-apps" => {
                self.show_all_apps = true;
                self.update_results();
            }
            "display-history" => {
                self.show_all_apps = false;
                self.update_results();
            }
            "display-menu" => self.screen = Screen::Settings,
            "go-to-homescreen" => {
                self.query.clear();
                self.show_all_apps = false;
                self.update_results();
            }
            "launch-pojo" => self.set_status("Choose a launch target in gesture settings"),
            "display-notifications" | "display-quicksettings" => {
                self.set_status("This system gesture is not available on this platform")
            }
            _ => {}
        }
    }

    fn handle_empty_area_gestures(&mut self, response: &egui::Response, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if response.drag_started() {
            if let Some(p) = ctx.input(|i| i.pointer.press_origin()) {
                self.touch_start = Some((p, now));
            }
        }
        if response.drag_stopped() {
            if let (Some((start, _)), Some(end)) = (
                self.touch_start.take(),
                ctx.input(|i| i.pointer.interact_pos()),
            ) {
                let delta = end - start;
                if delta.length() > 55.0 {
                    let key = if delta.x.abs() > delta.y.abs() {
                        if delta.x > 0.0 {
                            "gesture-right"
                        } else {
                            "gesture-left"
                        }
                    } else if delta.y > 0.0 {
                        "gesture-down"
                    } else {
                        "gesture-up"
                    };
                    let action = self.settings.value(key).to_owned();
                    self.perform_gesture(&action, ctx);
                }
            }
        }
        if response.long_touched() {
            let action = self.settings.value("gesture-long-press").to_owned();
            self.perform_gesture(&action, ctx);
        }
        if response.clicked() {
            if self.settings.enabled("history-onclick") {
                self.show_all_apps = false;
                self.update_results();
            }
            if self.settings.enabled("double-tap") {
                let pos = ctx
                    .input(|i| i.pointer.interact_pos())
                    .unwrap_or(response.rect.center());
                if let Some((last, time)) = self.last_empty_tap {
                    if now - time < 0.35 && last.distance(pos) < 30.0 {
                        self.set_status(
                            "Double-tap lock requires Android accessibility permission",
                        );
                        self.last_empty_tap = None;
                        return;
                    }
                }
                self.last_empty_tap = Some((pos, now));
            }
        }
    }

    fn apply_visuals(&self, ctx: &egui::Context, p: Palette) {
        let mut visuals = if self.settings.value("theme") == "light" {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        visuals.override_text_color = Some(p.text);
        visuals.panel_fill = p.bg;
        visuals.window_fill = p.surface;
        visuals.widgets.inactive.bg_fill = p.surface;
        visuals.widgets.hovered.bg_fill = p.hover;
        visuals.widgets.active.bg_fill = p.accent;
        visuals.selection.bg_fill = p.accent;
        ctx.set_visuals(visuals);
        let mut style = (*ctx.style()).clone();
        style.spacing.interact_size.y = 44.0;
        style.spacing.button_padding = Vec2::new(14.0, 10.0);
        ctx.set_style(style);
    }

    fn show_launcher(&mut self, ctx: &egui::Context, p: Palette) {
        // A background-only target gives KISS-like swipes without stealing taps from results.
        let bg = egui::Area::new(egui::Id::new("gesture-background"))
            .order(egui::Order::Background)
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let rect = ctx.screen_rect();
                ui.allocate_rect(rect, Sense::click_and_drag())
            });
        self.handle_empty_area_gestures(&bg.inner, ctx);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(p.bg)
                    .inner_margin(egui::Margin::same(10)),
            )
            .show(ctx, |ui| {
                let bottom_height = if self.settings.enabled("large-search-bar") {
                    92.0
                } else {
                    72.0
                };
                let favorites_height = if self.settings.enabled("enable-favorites-bar")
                    && !self.settings.enabled("favorites-hide")
                {
                    if self.settings.enabled("large-favorites-bar") {
                        74.0
                    } else {
                        58.0
                    }
                } else {
                    0.0
                };
                let results_height =
                    (ui.available_height() - bottom_height - favorites_height).max(40.0);
                ui.allocate_ui(Vec2::new(ui.available_width(), results_height), |ui| {
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.show_results(ui, p));
                });
                if favorites_height > 0.0 {
                    ui.allocate_ui(Vec2::new(ui.available_width(), favorites_height), |ui| {
                        self.show_favorites(ui, p)
                    });
                }
                ui.allocate_ui(Vec2::new(ui.available_width(), bottom_height), |ui| {
                    self.show_search_bar(ui, p)
                });
            });
    }

    fn show_favorites(&mut self, ui: &mut egui::Ui, p: Palette) {
        let favorites: Vec<AppEntry> = self
            .apps
            .iter()
            .filter(|a| a.is_favorite)
            .cloned()
            .collect();
        if favorites.is_empty() {
            return;
        }
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
                    let button = egui::Button::new(
                        RichText::new(initial)
                            .size(if self.settings.enabled("large-favorites-bar") {
                                22.0
                            } else {
                                18.0
                            })
                            .color(p.text),
                    )
                    .fill(if self.settings.enabled("transparent-favorites") {
                        Color32::TRANSPARENT
                    } else {
                        p.surface
                    })
                    .corner_radius(CornerRadius::same(22))
                    .min_size(Vec2::splat(44.0));
                    if ui.add(button).on_hover_text(&app.name).clicked() {
                        self.launch_exec(&app.exec);
                    }
                }
            })
        });
    }

    fn show_results(&mut self, ui: &mut egui::Ui, p: Palette) {
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

    fn show_search_bar(&mut self, ui: &mut egui::Ui, p: Palette) {
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
        if let Some((message, _)) = &self.status_message {
            ui.label(RichText::new(message).size(11.0).color(p.dim));
        }
    }

    fn show_settings(&mut self, ctx: &egui::Context, p: Palette) {
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
                        for spec in settings::specs() {
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

impl eframe::App for RissApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let p = Palette::from_settings(&self.settings);
        self.apply_visuals(ctx, p);
        self.handle_keys(ctx);
        if self
            .status_message
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(3))
        {
            self.status_message = None;
        }
        if !self.startup_notes.is_empty() {
            let notes = std::mem::take(&mut self.startup_notes);
            self.set_status(notes.join(" • "));
        }
        match self.screen {
            Screen::Launcher => self.show_launcher(ctx, p),
            Screen::Settings => self.show_settings(ctx, p),
        }
        if self.status_message.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    #[cfg(not(target_os = "android"))]
    let paths = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ];
    #[cfg(target_os = "android")]
    let paths = [
        "/system/fonts/Roboto-Regular.ttf",
        "/system/fonts/NotoSans-Regular.ttf",
        "/system/fonts/DroidSans.ttf",
    ];
    for path in paths {
        if let Ok(data) = std::fs::read(path) {
            fonts
                .font_data
                .insert("system".into(), egui::FontData::from_owned(data).into());
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "system".into());
            break;
        }
    }
    ctx.set_fonts(fonts);
}
