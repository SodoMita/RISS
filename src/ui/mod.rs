//! Launcher user interface.
//!
//! The screen is split into focused submodules; this module owns the app
//! state, the frame entry point and the launcher layout that hosts them:
//!
//! * [`colors`] — theme and colour management
//! * [`search_bar`] — search input and keyboard handling
//! * [`results`] — results list rendering
//! * [`settings`] — settings screen
//! * [`touch`] — touch gesture handling

mod colors;
mod results;
mod search_bar;
mod settings;
mod touch;

#[cfg(target_os = "android")]
use crate::android_app_entry::{self as app_entry, AppEntry};
#[cfg(not(target_os = "android"))]
use crate::app_entry::{self, AppEntry};
use crate::history::{HistoryData, HistoryMode};
use crate::search::{MatchType, SearchEngine, SearchResult};
use crate::settings::SettingsData;
use colors::Palette;
use eframe::egui;
use egui::RichText;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

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
    icon_textures: HashMap<String, egui::TextureHandle>,
    icons_without_image: HashSet<String>,
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
            icon_textures: HashMap::new(),
            icons_without_image: HashSet::new(),
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

    fn show_launcher(&mut self, ctx: &egui::Context, p: Palette) {
        let search_height = if self.settings.enabled("large-search-bar") {
            92.0
        } else {
            72.0
        };
        let has_favorites = self.apps.iter().any(|app| app.is_favorite);
        let favorites_height = if has_favorites
            && self.settings.enabled("enable-favorites-bar")
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
        let status_height = if self.status_message.is_some() {
            18.0
        } else {
            0.0
        };

        // Panels are laid out from the window edges, so the search bar stays
        // above the Android keyboard and pinned to the bottom on every resize.
        egui::TopBottomPanel::bottom("launcher-search")
            .exact_height(search_height + status_height + 20.0)
            .frame(
                egui::Frame::NONE
                    .fill(p.bg)
                    .inner_margin(egui::Margin::same(10)),
            )
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    self.show_search_bar(ui, p);
                    if let Some((message, _)) = &self.status_message {
                        ui.label(RichText::new(message).size(11.0).color(p.dim));
                    }
                });
            });

        if favorites_height > 0.0 {
            egui::TopBottomPanel::bottom("launcher-favorites")
                .exact_height(favorites_height + 8.0)
                .frame(
                    egui::Frame::NONE
                        .fill(p.bg)
                        .inner_margin(egui::Margin::symmetric(10, 4)),
                )
                .show(ctx, |ui| self.show_favorites(ui, p));
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(p.bg)
                    .inner_margin(egui::Margin::same(10)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.show_results(ui, p));
            });
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
