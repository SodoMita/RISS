#[cfg(target_os = "android")]
use crate::android_app_entry::{self as app_entry, AppEntry};
#[cfg(not(target_os = "android"))]
use crate::app_entry::{self, AppEntry};
use crate::history::{HistoryData, HistoryMode};
use crate::search::{self, MatchType, SearchEngine, SearchResult};
use crate::settings::SettingsData;
use eframe::egui::{self, RichText};
use std::collections::HashSet;
use std::time::{Duration, Instant};

mod colors;
mod icons;
mod results;
mod search_bar;
mod settings;
mod touch;

use colors::Palette;
use icons::IconCache;

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
    /// Icon textures, keyed by icon name, with a per-frame decode budget.
    icons: IconCache,
    selected_index: usize,
    screen: Screen,
    show_all_apps: bool,
    /// Snap the result list back to the top on the next frame.
    list_scroll_to_top: bool,
    /// Scroll the selected row into view on the next frame.
    list_follow_selection: bool,
    /// Identity of the last `Visuals`/`Style` pushed to egui, so restyling is
    /// skipped while nothing about the appearance changed.
    visuals_key: Option<u64>,
    status_message: Option<(String, Instant)>,
    editing_tags: Option<String>,
    tag_input: String,
    settings_query: String,
    /// Which `settings::specs()` match `settings_query`. Cached because the
    /// settings screen otherwise filters and relayouts every spec per frame.
    settings_rows: Option<Vec<usize>>,
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
            icons: IconCache::default(),
            selected_index: 0,
            screen: Screen::Launcher,
            show_all_apps: false,
            list_scroll_to_top: false,
            list_follow_selection: false,
            visuals_key: None,
            status_message: None,
            editing_tags: None,
            tag_input: String::new(),
            settings_query: String::new(),
            settings_rows: None,
            touch_start: None,
            last_empty_tap: None,
            startup_notes,
        };
        app.reload_apps();
        app
    }

    /// Read the installed apps again from disk or from `PackageManager`.
    /// That is the most expensive thing the launcher does, so it is reserved
    /// for startup and for moments where the app list really may have changed;
    /// see [`Self::refresh_apps_in_place`].
    fn reload_apps(&mut self) {
        let mut apps = app_entry::discover_apps();
        // A `name -> index` map would be exact, but a set of the names already
        // present is enough and keeps this linear instead of quadratic.
        let mut names: HashSet<String> = apps.iter().map(|app| app.name.to_lowercase()).collect();
        for builtin in app_entry::builtin_entries() {
            if names.insert(builtin.name.to_lowercase()) {
                apps.push(builtin);
            }
        }
        self.all_apps = apps;
        self.apply_history_fields();
        self.apply_exclusions();
        self.update_results();
    }

    /// Re-derive usage counts, favorites and tags for the apps already in
    /// memory. Launching an app used to trigger a full re-discovery, which is
    /// a visible freeze on a phone exactly when the launcher comes back.
    fn refresh_apps_in_place(&mut self) {
        self.apply_history_fields();
        self.update_results_in_place();
    }

    fn apply_history_fields(&mut self) {
        let favorites: HashSet<&str> = self.history.favorites.iter().map(String::as_str).collect();
        for app in &mut self.all_apps {
            app.launch_count = self.history.get_launch_count(&app.exec);
            app.last_launched = self.history.get_last_launched(&app.exec);
            // Membership test on a set: `HistoryData::is_favorite` scans a
            // vector, which made this a quadratic loop over the app list.
            app.is_favorite = favorites.contains(app.exec.as_str());
            let custom = self.history.get_tags(&app.exec);
            if !custom.is_empty() {
                app.tags = custom;
            }
        }
    }

    /// Re-filter `all_apps` into `apps` from the `edit-excluded-apps` setting.
    fn apply_exclusions(&mut self) {
        let excluded: Vec<&str> = self
            .settings
            .value("edit-excluded-apps")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        self.apps = if excluded.is_empty() {
            self.all_apps.clone()
        } else {
            self.all_apps
                .iter()
                .filter(|app| {
                    !excluded.iter().any(|x| {
                        app.exec.eq_ignore_ascii_case(x) || app.name.eq_ignore_ascii_case(x)
                    })
                })
                .cloned()
                .collect()
        };
    }

    /// Re-run search/filter/order and start the list at the top again.
    fn update_results(&mut self) {
        self.rebuild_list(true);
    }

    /// Re-run search/filter/order but keep the viewport where it is, for
    /// changes that leave the list recognisably the same (a star, a tag edit).
    fn update_results_in_place(&mut self) {
        self.rebuild_list(false);
    }

    fn rebuild_list(&mut self, snap_to_top: bool) {
        let limit = self.settings.number("number-of-display-elements", 20);
        let limit = if limit == 0 { usize::MAX } else { limit };
        if !self.query.trim().is_empty() {
            // "Search excluded apps" lets typed queries find excluded apps;
            // they never appear in the normal lists either way.
            let pool = if self.settings.enabled("enable-excluded-apps") {
                &self.all_apps
            } else {
                &self.apps
            };
            self.results = self.search_engine.search(&self.query, pool, limit);
            if self.settings.enabled("exclude-favorites-apps") {
                self.results.retain(|r| !r.entry.is_favorite);
            }
        } else if self.show_all_apps {
            self.results = self
                .apps
                .iter()
                .map(|entry| SearchResult {
                    entry: entry.clone(),
                    score: 0,
                    match_type: MatchType::Fuzzy,
                })
                .collect();
            self.results
                .sort_by(|a, b| search::compare_names(&a.entry.name, &b.entry.name));
        } else if self.settings.enabled("history-hide") {
            self.results.clear();
        } else {
            // Rank every candidate first, then apply the limit, so filters
            // like "hide favorites from history" do not shorten the list.
            self.results = self.search_engine.get_default_apps(&self.apps, usize::MAX);
            self.apply_history_ordering();
            self.results.truncate(limit);
        }
        self.selected_index = self
            .selected_index
            .min(self.results.len().saturating_sub(1));
        if snap_to_top {
            self.list_scroll_to_top = true;
        }
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
            self.results
                .sort_by(|a, b| search::compare_names(&a.entry.name, &b.entry.name));
            return;
        }
        let now = HistoryData::now_secs();
        // Score each row once instead of once per comparison: a comparator
        // that recomputes a score (two hash lookups plus `powf`) and lowercases
        // both names made this the slow part of every keystroke.
        self.results.sort_by_cached_key(|r| {
            let score = (self.history.rank_score(mode, &r.entry.exec, now) * 1024.0) as i64;
            (std::cmp::Reverse(score), r.entry.name.to_lowercase())
        });
    }

    /// Exec commands the user excluded from history recording/display.
    pub(super) fn history_excluded_set(&self) -> HashSet<String> {
        self.settings
            .value("edit-excluded-from-history-apps")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    }

    fn is_excluded_from_history(&self, entry: &AppEntry) -> bool {
        let excluded = self.history_excluded_set();
        excluded.contains(&entry.exec)
            || excluded
                .iter()
                .any(|item| entry.name.eq_ignore_ascii_case(item))
    }

    /// Add or remove an app from the `edit-excluded-from-history-apps` list.
    fn set_excluded_from_history(&mut self, exec: &str, name: &str, excluded: bool) {
        let mut items: Vec<String> = self
            .settings
            .value("edit-excluded-from-history-apps")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        items.retain(|item| !exec.eq_ignore_ascii_case(item) && !name.eq_ignore_ascii_case(item));
        if excluded {
            items.push(exec.to_owned());
        }
        self.settings.values.insert(
            "edit-excluded-from-history-apps".to_owned(),
            items.join(","),
        );
        self.save_settings();
        self.update_results_in_place();
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
                // Only the usage counters changed: re-derive them instead of
                // scanning every desktop entry / package again.
                self.apply_history_fields();
                self.update_results();
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

    /// Toggling a star updates the entries in place: rebuilding the list from
    /// scratch (let alone re-discovering apps) would drop the scroll position
    /// the user is browsing from.
    fn toggle_favorite_exec(&mut self, exec: &str) {
        let favorite = self.history.toggle_favorite(exec);
        self.save_history();
        for app in &mut self.all_apps {
            if app.exec == exec {
                app.is_favorite = favorite;
            }
        }
        for app in &mut self.apps {
            if app.exec == exec {
                app.is_favorite = favorite;
            }
        }
        // `exclude-favorites-apps` / `exclude-favorites-history` decide whether
        // this row still belongs in the list at all.
        self.update_results_in_place();
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
                // `show_results` owns the scroll area: virtualising the list
                // needs the area and the row height to be decided together.
                self.show_results(ui, p);
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
        // Reset the icon decode budget before anything draws: the favorites bar
        // asks for textures too, and it is laid out before the result list.
        self.icons.begin_frame();
        match self.screen {
            Screen::Launcher => self.show_launcher(ctx, p),
            Screen::Settings => self.show_settings(ctx, p),
        }
        // Repaint only while something is still moving, and only for as long
        // as it takes: a fixed 250 ms loop used to redraw the whole launcher
        // four times a second for the life of every toast.
        let mut next = None;
        if let Some((_, at)) = &self.status_message {
            let left = Duration::from_secs(3).saturating_sub(at.elapsed());
            next = Some(left.max(Duration::from_millis(16)));
        }
        if self.icons.take_pending() {
            // Icons are decoded a handful per frame; keep painting until they
            // have all landed, then stop.
            let soon = Duration::from_millis(16);
            next = Some(next.map_or(soon, |left| left.min(soon)));
        }
        if let Some(delay) = next {
            ctx.request_repaint_after(delay);
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
