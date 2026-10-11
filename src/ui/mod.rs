#[cfg(target_os = "android")]
use crate::android_app_entry::{self as app_entry, AppEntry};
#[cfg(not(target_os = "android"))]
use crate::app_entry::{self, AppEntry};
use crate::history::{HistoryData, HistoryMode};
use crate::kiss::index::AppIndex;
use crate::kiss::prefs;
use crate::kiss::searcher::{SearchParams, Searcher as KissSearcher};
use crate::kiss::worker::{AppLoader, SearchWorker};
use crate::search::{self, MatchType, ResultAction, ResultView, SearchEngine, SearchResult};
use crate::settings::SettingsData;
use eframe::egui::{self, RichText};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod colors;
mod favorites;
mod fling;
mod icons;
pub(crate) mod insets;
mod osk;
mod results;
mod search_bar;
mod settings;
mod touch;
mod widgets;
pub(crate) mod window_mode;

use colors::Palette;
use favorites::FavDrag;
use icons::IconCache;
use window_mode::WindowMode;

/// The blocking half of a reload, run on the provider-loading worker thread.
///
/// KISS's `AppProvider` loads on an `AsyncTask` and is re-triggered by
/// `LauncherApps.Callback`; nothing about it happens on the UI thread.
fn load_app_entries() -> Vec<AppEntry> {
    let mut apps = app_entry::discover_apps();
    for builtin in app_entry::builtin_entries() {
        if !apps
            .iter()
            .any(|a| a.name.eq_ignore_ascii_case(&builtin.name))
        {
            apps.push(builtin);
        }
    }
    apps
}

/// A result row asked to activate while `results` was borrowed for painting.
enum Deferred {
    Activate(usize),
}

/// How long the Uninstall action waits for its confirmation click before
/// the first press expires.
const UNINSTALL_CONFIRM_WINDOW: Duration = Duration::from_secs(6);

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Launcher,
    Settings,
    /// Ordered list of favorites that can be deleted, reordered and tagged.
    Favorites,
}

pub struct RissApp {
    query: String,
    /// Every discovered app, including ones excluded from the lists.
    all_apps: Vec<AppEntry>,
    /// Apps shown in the normal lists (exclusions applied).
    apps: Vec<AppEntry>,
    results: Vec<SearchResult>,
    search_engine: SearchEngine,
    /// The loaded application data the live search runs over: every entry
    /// carries its own `StringNormalizer.Result`, normalized once here and
    /// never per keystroke. Shared with the search worker behind an `Arc`.
    index: Arc<AppIndex>,
    /// Bumped on every index rebuild so a result computed against the old
    /// snapshot can be recognised and dropped.
    data_generation: u64,
    /// KISS's `Searcher.SEARCH_THREAD`: one persistent serial executor.
    search_worker: SearchWorker,
    /// The provider-loading worker. Scanning `.desktop` files / crossing JNI
    /// never happens inside a frame.
    app_loader: AppLoader<Vec<AppEntry>>,
    /// Generation of the search whose outcome we are willing to paint.
    /// Anything else is stale and is thrown away.
    expected_search_generation: u64,
    /// Generation of the search currently on screen. Equal to
    /// `expected_search_generation` once the answer has landed.
    applied_search_generation: u64,
    /// Test-only: ignore the provider-loading worker.
    ///
    /// The constructor kicks off a real app discovery, so a headless test
    /// that installs a known app list would have it overwritten the moment
    /// the host's `.desktop` scan finished. Production never sets this.
    #[cfg(test)]
    ignore_loader: bool,
    /// A UI-thread searcher for the modal favourites picker, so there is one
    /// scoring dialect in the process and not two.
    picker_searcher: KissSearcher,
    /// `QuerySearcher.MAX_RESULT_COUNT`, cached exactly like upstream caches
    /// it, and cleared by `clearMaxResultCountCache()` when settings change.
    max_result_count: Option<usize>,
    history: HistoryData,
    settings: SettingsData,
    icons: IconCache,
    selected_index: usize,
    screen: Screen,
    show_all_apps: bool,
    show_hidden: bool,
    status_message: Option<(String, Instant)>,
    editing_tags: Option<String>,
    tag_input: String,
    editing_alias: Option<String>,
    alias_input: String,
    editing_icon: Option<String>,
    icon_input: String,
    settings_query: String,
    /// Query used by the "Manage favorites" screen to find apps to favorite.
    favorites_query: String,
    /// Screen the favorites manager returns to when closed.
    favorites_return: Screen,
    /// Favorite being dragged along the bar, if any.
    fav_drag: Option<FavDrag>,
    touch_start: Option<(egui::Pos2, f64)>,
    last_empty_tap: Option<(egui::Pos2, f64)>,
    startup_notes: Vec<String>,
    /// Running countdown: total seconds and the start time.
    timer: Option<(u64, Instant)>,
    /// Room for on-screen keyboards on Linux desktops.
    osk: osk::OskTracker,
    /// Window mode last applied from settings (a command-line override
    /// stays until the setting changes).
    window_mode: WindowMode,
    /// Pending uninstall confirmation: the entry's exec and the moment
    /// "Uninstall" was first pressed. A destructive operation should not
    /// run on a single stray tap.
    pending_uninstall: Option<(String, Instant)>,
    /// Accumulating wheel fling. Positive increases the scroll offset.
    fling_velocity: f32,
    /// Arrow keys moved the highlight; reveal that row in the virtual list.
    scroll_selection: bool,
    /// Bumped when `results` is rebuilt, so a paint that took the vec can
    /// tell a nested reload already installed a fresh one.
    list_generation: u64,
    deferred: Vec<Deferred>,
    first_frame: bool,
    surrender_search_focus: bool,
}

impl RissApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (settings, notes) = SettingsData::load();
        Self::with_settings(cc, settings, notes)
    }

    /// Create the app from settings the caller already loaded (the desktop
    /// entry point needs them to pick the initial window mode).
    pub fn with_settings(
        cc: &eframe::CreationContext<'_>,
        settings: SettingsData,
        startup_notes: Vec<String>,
    ) -> Self {
        Self::with_context(&cc.egui_ctx, settings, startup_notes)
    }

    /// The real constructor, taking the context directly.
    ///
    /// `eframe::CreationContext` cannot be built outside eframe, so every
    /// headless test of the live path would otherwise have to re-implement
    /// the app's wiring and test a replica instead of the product.
    pub fn with_context(
        egui_ctx: &egui::Context,
        settings: SettingsData,
        mut startup_notes: Vec<String>,
    ) -> Self {
        let (history, history_notes) = HistoryData::load();
        startup_notes.extend(history_notes);
        let window_mode = WindowMode::from_settings(&settings);
        let mut app = Self {
            query: String::new(),
            all_apps: Vec::new(),
            apps: Vec::new(),
            results: Vec::new(),
            search_engine: SearchEngine::new(),
            index: Arc::new(AppIndex::new(0)),
            data_generation: 0,
            search_worker: {
                let ctx = egui_ctx.clone();
                SearchWorker::new(move || ctx.request_repaint())
            },
            app_loader: {
                let ctx = egui_ctx.clone();
                AppLoader::new(load_app_entries, move || ctx.request_repaint())
            },
            expected_search_generation: 0,
            applied_search_generation: 0,
            #[cfg(test)]
            ignore_loader: false,
            picker_searcher: KissSearcher::new(),
            max_result_count: None,
            history,
            settings,
            icons: IconCache::default(),
            selected_index: 0,
            screen: Screen::Launcher,
            show_all_apps: false,
            show_hidden: false,
            status_message: None,
            editing_tags: None,
            tag_input: String::new(),
            editing_alias: None,
            alias_input: String::new(),
            editing_icon: None,
            icon_input: String::new(),
            settings_query: String::new(),
            favorites_query: String::new(),
            favorites_return: Screen::Launcher,
            fav_drag: None,
            touch_start: None,
            last_empty_tap: None,
            startup_notes,
            timer: None,
            osk: osk::OskTracker::new(egui_ctx),
            window_mode,
            pending_uninstall: None,
            fling_velocity: 0.0,
            scroll_selection: false,
            list_generation: 0,
            deferred: Vec::new(),
            first_frame: true,
            surrender_search_focus: false,
        };
        // egui's native default is 40px per wheel line. The result list adds
        // velocity on top of this; the settings screen just needs to not crawl.
        egui_ctx.options_mut(|options| {
            options.input_options.line_scroll_speed = 96.0;
        });
        insets::watch(egui_ctx);
        app.reload_apps();
        app
    }

    /// Ask the provider-loading worker for a fresh app list.
    ///
    /// Non-blocking by construction: scanning `.desktop` files (Linux) or
    /// crossing JNI into `LauncherApps` (Android) must never run inside an
    /// egui frame. The answer is picked up in [`Self::poll_workers`].
    fn reload_apps(&mut self) {
        self.app_loader.request();
    }

    /// `AppProvider`'s loader finishing: stamp the user's data onto the
    /// entries, then rebuild the search index.
    fn apply_loaded_apps(&mut self, mut apps: Vec<AppEntry>) {
        for app in &mut apps {
            app.launch_count = self.history.get_launch_count(&app.exec);
            app.last_launched = self.history.get_last_launched(&app.exec);
            app.is_favorite = self.history.is_favorite(&app.exec);
            let custom = self.history.get_tags(&app.exec);
            if !custom.is_empty() {
                app.tags = custom;
            }
            // KISS's alias is applied through `Pojo.setName`, so the
            // normalized name is built from the name the user sees.
            if let Some(alias) = self.history.alias(&app.exec) {
                app.name = alias.clone();
            }
            if let Some(custom_icon) = self.history.custom_icon(&app.exec) {
                app.icon = custom_icon.clone();
            }
        }
        self.all_apps = apps;
        self.rebuild_index();
    }

    /// Recompute the exclusion-filtered list and the normalized search index
    /// from `all_apps`, without touching the filesystem.
    ///
    /// Bumping `data_generation` is what makes a reload racing a search safe:
    /// an outcome computed against the previous snapshot is refused rather
    /// than painted against positions that have moved.
    fn rebuild_index(&mut self) {
        // Excluded apps disappear from the normal lists; the "Search excluded
        // apps" preference decides whether typed queries can still find them,
        // which is `AppProvider.requestResults`' `enable-excluded-apps` test.
        let excluded: Vec<String> = self
            .settings
            .value("edit-excluded-apps")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_lowercase)
            .collect();
        let is_excluded = |a: &AppEntry| {
            excluded
                .iter()
                .any(|x| a.exec.eq_ignore_ascii_case(x) || a.name.eq_ignore_ascii_case(x))
        };
        self.apps = self
            .all_apps
            .iter()
            .filter(|a| !is_excluded(a))
            .cloned()
            .collect();
        self.data_generation = self.data_generation.wrapping_add(1);
        self.index = Arc::new(AppIndex::build(
            self.data_generation,
            self.all_apps.clone(),
            is_excluded,
            // `AppPojo.disabled` comes from `LauncherApps` on Android and has
            // no source on either RISS platform yet, so no entry is disabled.
            // The penalty itself is ported and tested; this is the input that
            // is missing, and it is listed as a gap rather than faked.
            |_| false,
        ));
        self.update_results();
    }

    /// `QuerySearcher.getMaxResultCount()`, cached as upstream caches it in
    /// its `MAX_RESULT_COUNT` static.
    fn max_result_count(&mut self) -> usize {
        if let Some(cached) = self.max_result_count {
            return cached;
        }
        let raw = self
            .settings
            .value("number-of-display-elements")
            .to_string();
        let value = prefs::max_result_count_usize(Some(&raw));
        self.max_result_count = Some(value);
        value
    }

    /// `QuerySearcher.clearMaxResultCountCache()`.
    pub(crate) fn clear_max_result_count_cache(&mut self) {
        self.max_result_count = None;
    }

    /// A setting that the search path reads has changed: drop the cached
    /// max-result count and re-run the current query.
    fn invalidate_search_cache(&mut self) {
        self.clear_max_result_count_cache();
        self.update_results();
    }

    /// Everything one search needs, resolved on the UI thread and then handed
    /// to the worker by value. The worker never reads app state, so the UI
    /// holds no lock while a search runs.
    fn search_params(&mut self) -> SearchParams {
        let max_results = self.max_result_count();
        let query = self.query.trim().to_string();
        let history_mode = prefs::HistoryMode::value_by_id(self.settings.value("history-mode"));
        // `DataHandler.getExcludedFavorites()`.
        let excluded_favorite_ids = if self.settings.enabled("exclude-favorites-apps") {
            self.history.favorites.clone()
        } else {
            Vec::new()
        };
        let known_ids = if self.settings.enabled("enable-app-history") {
            self.history.known_ids_for_query(&query, max_results)
        } else {
            Vec::new()
        };
        SearchParams {
            query,
            max_results,
            history_mode,
            enable_excluded_apps: self.settings.enabled("enable-excluded-apps"),
            known_ids,
            excluded_favorite_ids,
        }
    }

    /// Hand the current query to the search worker. The previous results stay
    /// on screen until the answer arrives, exactly as KISS's adapter keeps its
    /// rows until `onPostExecute`.
    fn submit_search(&mut self) {
        let params = self.search_params();
        self.expected_search_generation =
            self.search_worker.submit(params, Arc::clone(&self.index));
    }

    /// Drain the two workers. Called once per frame, before anything is
    /// painted; neither call blocks.
    fn poll_workers(&mut self) {
        #[cfg(test)]
        let loader_enabled = !self.ignore_loader;
        #[cfg(not(test))]
        let loader_enabled = true;
        if loader_enabled {
            if let Some(loaded) = self.app_loader.poll() {
                self.apply_loaded_apps(loaded.entries);
            }
        }
        if let Some(outcome) = self.search_worker.poll(self.data_generation) {
            // Stale-result guard: only the generation we are still waiting
            // for may be painted.
            if outcome.generation == self.expected_search_generation {
                self.applied_search_generation = outcome.generation;
                self.apply_search_outcome(&outcome.hits);
            }
        }
    }

    /// `Searcher.onPostExecute` -> `adapter.updateWithPojos`.
    fn apply_search_outcome(&mut self, hits: &[crate::kiss::searcher::Scored]) {
        let query = self.query.trim().to_string();
        self.results.clear();
        self.results.reserve(hits.len());
        for hit in hits {
            let Some(item) = self.index.get(hit.index) else {
                continue;
            };
            self.results.push(SearchResult {
                entry: item.entry.clone(),
                score: hit.relevance as i64,
                match_type: MatchType::Fuzzy,
                action: ResultAction::Launch,
            });
        }
        // RISS's own provider rows (web search, timers, settings, ...) are
        // appended below the application matches. That layout predates this
        // port and is left alone.
        let app_matches = self.results.len();
        let extra = self.provider_results(&query, app_matches);
        self.results.extend(extra);
        self.list_generation = self.list_generation.wrapping_add(1);
        if self.selected_index >= self.results.len() {
            self.selected_index = 0;
        }
    }

    /// Rebuild the displayed list.
    ///
    /// For a non-empty query this only *queues* the search: the KISS path
    /// runs on the search worker and the answer arrives through
    /// [`Self::poll_workers`]. The UI never waits for it.
    ///
    /// The empty-query branches below are the no-query lists (upstream's
    /// `HistorySearcher` / `ApplicationsSearcher` territory) and are outside
    /// the application-search path this port replaces; they are untouched
    /// apart from cancelling any search still in flight.
    fn update_results(&mut self) {
        self.list_generation = self.list_generation.wrapping_add(1);
        if !self.query.trim().is_empty() {
            self.submit_search();
            return;
        }
        // Leaving query mode supersedes anything in flight, so a late answer
        // cannot overwrite the list we are about to build.
        self.expected_search_generation = u64::MAX;
        let limit = self.settings.number("number-of-display-elements", 20);
        if self.show_hidden {
            // The hidden-apps list: activating a row restores it (KISS has no
            // such screen; pr-3 called it "Excluded").
            self.results = self
                .all_apps
                .iter()
                .filter(|entry| self.is_excluded_from_search(entry))
                .cloned()
                .map(|entry| SearchResult {
                    score: 0,
                    match_type: MatchType::Fuzzy,
                    action: ResultAction::Excluded {
                        exec: entry.exec.clone(),
                    },
                    entry,
                })
                .collect();
            self.results.sort_by_key(|r| r.entry.name.to_lowercase());
        } else if self.show_all_apps {
            self.results = self
                .apps
                .iter()
                .cloned()
                .map(|entry| SearchResult {
                    entry,
                    score: 0,
                    match_type: MatchType::Fuzzy,
                    action: ResultAction::Launch,
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

    fn provider_results(&self, query: &str, app_matches: usize) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = Vec::new();
        let query_lower = query.to_lowercase();

        // A digit bound to a shortcut jumps straight to that application.
        if let Some(digit) = query.parse::<u8>().ok().filter(|d| (1..=9).contains(d)) {
            if self.settings.enabled("enable-shortcuts") {
                if let Some(exec) = self.history.shortcut_for(digit) {
                    if let Some(app) = self.all_apps.iter().find(|app| &app.exec == exec) {
                        let mut entry = app.clone();
                        entry.is_favorite = true;
                        results.push(SearchResult {
                            entry,
                            score: 9000,
                            match_type: MatchType::Provider,
                            action: ResultAction::Launch,
                        });
                    }
                }
            }
        }

        // Settings provider: the settings can be searched from the query bar.
        if self.settings.enabled("enable-settings") {
            let mut hits: Vec<SearchResult> = crate::settings::specs()
                .into_iter()
                .filter(|spec| {
                    let value = match spec.kind {
                        crate::settings::SettingKind::Toggle => {
                            if self.settings.enabled(spec.key) {
                                "on".to_string()
                            } else {
                                "off".to_string()
                            }
                        }
                        _ => self.settings.value(spec.key).to_string(),
                    };
                    let haystack =
                        format!("{} {} {} {}", spec.section, spec.title, spec.key, value)
                            .to_lowercase();
                    haystack.contains(&query_lower)
                })
                .take(3)
                .map(|spec| {
                    let value = match spec.kind {
                        crate::settings::SettingKind::Toggle => {
                            if self.settings.enabled(spec.key) {
                                "On"
                            } else {
                                "Off"
                            }
                        }
                        _ => self.settings.value(spec.key),
                    };
                    search::setting_result(spec.title, spec.section, spec.key, value)
                })
                .collect();
            hits.truncate(3);
            results.extend(hits);
        }

        // Timer provider.
        if self.settings.enabled("enable-timer") {
            if let Some(seconds) = crate::providers::parse_timer(query) {
                results.push(search::timer_result(seconds));
            }
        }

        // Previously run web searches and shell commands resurface as the
        // query narrows.
        if self.settings.enabled("search-through-history") {
            for previous in self.history.recent_searches(20) {
                if previous.to_lowercase().contains(&query_lower) {
                    let mut result = self.web_search_result(&previous);
                    result.entry.name = previous.clone();
                    result.score = 200;
                    results.push(result);
                }
            }
        }
        if self.settings.enabled("enable-exec") {
            for previous in self.history.recent_execs(10) {
                if previous != query && previous.to_lowercase().contains(&query_lower) {
                    let mut result = search::exec_result(&previous);
                    result.score = 250;
                    results.push(result);
                }
            }
        }

        // The special lists stay reachable by typing their name.
        if query.len() > 2 {
            let special = match query_lower.as_str() {
                "history" | "recent" => Some((ResultView::History, "History")),
                "apps" | "all apps" | "all" => Some((ResultView::AllApps, "All applications")),
                "settings" | "preferences" => Some((ResultView::Settings, "Settings")),
                "excluded" | "hidden" | "hidden apps" | "hidden applications" => {
                    Some((ResultView::Excluded, "Hidden applications"))
                }
                _ => None,
            };
            if let Some((view, title)) = special {
                let mut result =
                    search::view_result(view, title, "Special list — activate to open");
                result.score = 850;
                results.push(result);
            }
        }

        // Web search, exactly like KISS which always offers one at the bottom.
        if self.settings.enabled("enable-search") {
            results.push(self.web_search_result(query));
        }

        // Shell command, only when nothing else looks like a match.
        if self.settings.enabled("enable-exec") && app_matches == 0 && query.contains(' ') {
            results.push(search::exec_result(query));
        }

        results.sort_by_key(|result| std::cmp::Reverse(result.score));
        results
    }

    /// Web search row for `query` with the configured provider.
    fn web_search_result(&self, query: &str) -> SearchResult {
        let name = self.settings.value("default-search-provider");
        let template = crate::providers::provider_template(
            name,
            self.settings.value("custom-search-provider-add"),
        )
        .unwrap_or_else(|| "https://duckduckgo.com/?q={}".to_string());
        search::web_search_result(name, &template, query)
    }

    /// Order the default (no-query) list according to the `history-mode`
    /// setting, hiding apps the user excluded from history. Ties are broken
    /// by name so ordering is deterministic.
    fn apply_history_ordering(&mut self) {
        let mode = HistoryMode::from_key(self.settings.value("history-mode"));
        let hide_favorites = self.settings.enabled("exclude-favorites-history");
        let excluded = self.history_excluded_set();
        Self::sort_history_results(
            &mut self.results,
            &self.history,
            mode,
            HistoryData::now_secs(),
            hide_favorites,
            &excluded,
        );
    }

    /// Order the default (no-query) list for the history view (issue #22).
    ///
    /// A `&self`-free helper so the ordering is unit-testable without a UI:
    /// the whole candidate list is ranked first and truncated later by the
    /// caller, so the most recently launched app lands on top in recency
    /// mode no matter its favorite status or lifetime launch count.
    fn sort_history_results(
        results: &mut Vec<SearchResult>,
        history: &HistoryData,
        mode: HistoryMode,
        now: u64,
        hide_favorites: bool,
        excluded: &HashSet<String>,
    ) {
        if hide_favorites {
            results.retain(|r| !r.entry.is_favorite);
        }
        if !excluded.is_empty() {
            results.retain(|r| !excluded.contains(&r.entry.exec));
        }
        if mode == HistoryMode::Alphabetical {
            results.sort_by_key(|r| r.entry.name.to_lowercase());
            return;
        }
        results.sort_by(|a, b| {
            let score_a = history.rank_score(mode, &a.entry.exec, now);
            let score_b = history.rank_score(mode, &b.entry.exec, now);
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

    /// Is the app on the `edit-excluded-apps` list (hidden from the lists)?
    fn is_excluded_from_search(&self, entry: &AppEntry) -> bool {
        let excluded = self.settings.value("edit-excluded-apps");
        excluded
            .split(',')
            .map(str::trim)
            .any(|x| !x.is_empty() && (entry.exec == x || entry.name.eq_ignore_ascii_case(x)))
    }

    /// Add or remove an app from the `edit-excluded-apps` list.
    fn set_excluded_from_search(&mut self, entry: &AppEntry, excluded: bool) {
        let mut items: Vec<String> = self
            .settings
            .value("edit-excluded-apps")
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
        self.settings
            .values
            .insert("edit-excluded-apps".to_owned(), items.join(","));
        self.save_settings();
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

    fn is_excluded_app(&self, entry: &AppEntry) -> bool {
        self.is_excluded_from_search(entry)
    }

    /// Add or remove an app from the `edit-excluded-apps` list.
    fn set_excluded_app(&mut self, entry: &AppEntry, excluded: bool) {
        self.set_excluded_from_search(entry, excluded);
    }

    /// Stamp usage onto both app lists without walking packages again.
    ///
    /// KISS updates the in-memory pojo. `PackageManager` is re-queried only
    /// from `LauncherApps.Callback`. `reload_apps` here was a full XDG or JNI
    /// walk on every launch — slower than the Java launcher, and the reason
    /// `apps` could drift from `all_apps`.
    fn reapply_usage(&mut self) {
        for app in &mut self.all_apps {
            app.launch_count = self.history.get_launch_count(&app.exec);
            app.last_launched = self.history.get_last_launched(&app.exec);
            app.is_favorite = self.history.is_favorite(&app.exec);
        }
        // Rebuilding the index is what makes the change *visible*: the rows
        // the UI paints come from `index`, not from `apps`, so stamping the
        // two Vecs and stopping there would leave a freshly starred app
        // showing a hollow star until the next disk rescan.
        //
        // No I/O happens here -- `rebuild_index` only re-derives the
        // exclusion filter and the normalized names from `all_apps`, which
        // is why a launch can afford it and a package change cannot be
        // skipped.
        self.rebuild_index();
    }

    fn launch_exec(&mut self, exec: &str) {
        // Search the full list so excluded apps reached through search can
        // still be launched.
        let Some(entry) = self.all_apps.iter().find(|a| a.exec == exec).cloned() else {
            return;
        };
        let name = self.display_name(&entry);
        match entry.launch() {
            Ok(()) => {
                if !self.settings.enabled("freeze-history")
                    && self.settings.enabled("enable-app-history")
                    && !self.is_excluded_from_history(&entry)
                {
                    self.history.record_launch(exec);
                    // The `query` column `QuerySearcher.knownIds` reads back.
                    self.history.record_launch_for_query(&self.query, exec);
                    self.save_history();
                }
                self.query.clear();
                self.show_all_apps = false;
                self.show_hidden = false;
                self.reapply_usage();
                self.set_status(format!("Opened {}", name));
                if self.settings.enabled("hide-keyboard") {
                    self.surrender_search_focus = true;
                }
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

    /// Shown name of an entry, honoring the user's rename.
    fn display_name(&self, entry: &AppEntry) -> String {
        search::display_name(entry, &self.history.aliases)
    }

    /// Activate the selected result: launch apps or run its provider action.
    fn activate(&mut self, index: usize) {
        let action = match self.results.get(index) {
            Some(result) => result.action.clone(),
            None => return,
        };
        match action {
            ResultAction::Launch => self.launch_result(index),
            ResultAction::WebSearch { provider, query } => {
                let template = crate::providers::provider_template(
                    &provider,
                    self.settings.value("custom-search-provider-add"),
                )
                .unwrap_or_else(|| "https://duckduckgo.com/?q={}".to_string());
                let url = crate::providers::provider_url(&template, &query);
                match crate::providers::open_url(&url) {
                    Ok(()) => {
                        self.history.record_search(&query);
                        self.save_history();
                        self.set_status(format!("Searching {} for “{}”", provider, query));
                    }
                    Err(error) => self.set_status(error),
                }
                self.query.clear();
                self.update_results();
            }
            ResultAction::Exec { command } => match crate::providers::run_command(&command) {
                Ok(()) => {
                    self.history.record_exec(&command);
                    self.save_history();
                    self.set_status(format!("Ran {}", command));
                }
                Err(error) => self.set_status(error),
            },
            ResultAction::Timer { seconds } => {
                if self.timer.is_some() {
                    self.timer = None;
                    self.set_status("Timer cancelled");
                } else {
                    self.timer = Some((seconds, Instant::now()));
                    self.set_status(format!(
                        "Timer started: {}",
                        crate::providers::format_duration(seconds)
                    ));
                }
            }
            ResultAction::Copy { text } => {
                if crate::providers::copy_to_clipboard(&text) {
                    self.set_status(format!("Copied {}", text));
                } else {
                    self.set_status(format!("{} — clipboard helper not found", text));
                }
            }
            ResultAction::Setting { id } => self.focus_setting(&id),
            ResultAction::View(view) => {
                match view {
                    ResultView::History => {
                        self.show_all_apps = false;
                        self.show_hidden = false;
                        self.query.clear();
                    }
                    ResultView::AllApps => {
                        self.show_all_apps = true;
                        self.show_hidden = false;
                        self.query.clear();
                    }
                    ResultView::Excluded => {
                        self.show_all_apps = false;
                        self.show_hidden = true;
                        self.query.clear();
                        self.set_status("Hidden applications — activate to restore");
                    }
                    ResultView::Settings => self.screen = Screen::Settings,
                }
                self.selected_index = 0;
                self.update_results();
            }
            ResultAction::Excluded { exec } => {
                let entry = self.all_apps.iter().find(|e| e.exec == exec).cloned();
                if let Some(entry) = entry {
                    self.set_excluded_from_search(&entry, false);
                    self.set_status(format!("{} restored to results", self.display_name(&entry)));
                    self.reload_apps();
                    self.update_results();
                }
            }
        }
    }

    /// Open the settings screen focused on one preference (KISS' settings
    /// provider): the filter jumps straight to the matching row.
    fn focus_setting(&mut self, key: &str) {
        self.screen = Screen::Settings;
        self.settings_query = crate::settings::specs()
            .into_iter()
            .find(|spec| spec.key == key)
            .map(|spec| spec.title.to_string())
            .unwrap_or_else(|| key.to_string());
    }

    fn toggle_favorite_exec(&mut self, exec: &str) {
        let favorited = self.history.toggle_favorite(exec);
        let name = self.favorite_display_name(exec);
        self.save_history();
        self.reapply_usage();
        self.set_status(if favorited {
            format!("{name} was added to favorites")
        } else {
            format!("{name} was removed from favorites")
        });
    }

    fn set_status(&mut self, text: impl Into<String>) {
        self.status_message = Some((text.into(), Instant::now()));
    }

    /// Advance a running countdown, ringing the terminal bell when it ends.
    /// The status line doubles as the timer display.
    fn timer_tick(&mut self) {
        if let Some((total, started)) = self.timer {
            let elapsed = started.elapsed().as_secs();
            if elapsed >= total {
                self.timer = None;
                self.set_status("Timer finished");
                // Audible feedback, like the KISS timer notification.
                #[cfg(not(target_os = "android"))]
                println!("\u{7}");
            } else {
                let remaining = total - elapsed;
                self.set_status(format!(
                    "{} left — activate the timer row to cancel",
                    crate::providers::format_duration(remaining)
                ));
            }
        }
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

    /// Follow changes of the `window-mode` setting.
    fn sync_window_mode(&mut self, ctx: &egui::Context) {
        let wanted = WindowMode::from_settings(&self.settings);
        if wanted != self.window_mode {
            wanted.apply_to_window(ctx);
            self.window_mode = wanted;
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

        // Panels are laid out inside the insets reserved in `update`, so the
        // search bar sits right above the keyboard.
        // Panels are laid out from the window edges, so the search bar stays
        // above the Android keyboard and pinned to the bottom on every resize.
        // They are painted with `p.panel`, which is transparent whenever the
        // home-screen wallpaper should show through behind the results.
        egui::TopBottomPanel::bottom("launcher-search")
            .exact_height(search_height + status_height + 20.0)
            .frame(
                egui::Frame::NONE
                    .fill(p.panel)
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
                        .fill(p.panel)
                        .inner_margin(egui::Margin::symmetric(10, 4)),
                )
                .show(ctx, |ui| self.show_favorites(ui, p));
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(p.panel)
                    .inner_margin(egui::Margin::same(10)),
            )
            .show(ctx, |ui| {
                // One scroll area, owned by the result list, so wheel ticks
                // can accumulate instead of being eaten by a parent scroller.
                self.show_results(ui, p);
            });
    }
}

impl eframe::App for RissApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Worker answers first, before anything is laid out: the frame paints
        // a cached list, it never searches or loads.
        self.poll_workers();
        let p = Palette::from_settings(&self.settings);
        self.apply_visuals(ctx, p);
        self.icons.begin_frame();
        self.handle_keys(ctx);
        if self.first_frame {
            self.first_frame = false;
            if self.settings.enabled("display-keyboard") {
                ctx.memory_mut(|m| m.request_focus(egui::Id::new("riss-search")));
            }
        }
        if self.surrender_search_focus {
            self.surrender_search_focus = false;
            ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("riss-search")));
        }
        if self
            .status_message
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(3))
        {
            self.status_message = None;
        }
        if self
            .pending_uninstall
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() > UNINSTALL_CONFIRM_WINDOW)
        {
            self.pending_uninstall = None;
        }
        self.timer_tick();
        if !self.startup_notes.is_empty() {
            let notes = std::mem::take(&mut self.startup_notes);
            self.set_status(notes.join(" • "));
        }
        #[cfg(not(target_os = "android"))]
        self.sync_window_mode(ctx);
        // Keep clear of the keyboard and system bars before any other layout.
        let insets = insets::current(ctx);
        #[cfg(not(target_os = "android"))]
        let insets = {
            let (keyboard, measured) = self.osk.update(ctx, &mut self.settings);
            if measured {
                self.save_settings();
            }
            insets.with_min_bottom(keyboard)
        };
        insets.reserve(ctx, p.bg);
        match self.screen {
            Screen::Launcher => self.show_launcher(ctx, p),
            Screen::Settings => self.show_settings(ctx, p),
            Screen::Favorites => self.show_favorites_screen(ctx, p),
        }
        // Rows that missed the icon decode budget ask for one more pass.
        if self.icons.take_pending() || self.fling_velocity.abs() > 1.0 {
            ctx.request_repaint();
        }
        if self.status_message.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    /// eframe clears the framebuffer with a nearly opaque dark grey, which
    /// would cover the home-screen wallpaper even when every panel is
    /// transparent — so clear with alpha 0 whenever the wallpaper shows
    /// through.
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        if colors::wallpaper_visible(&self.settings) {
            egui::Color32::TRANSPARENT.to_normalized_gamma_f32()
        } else {
            visuals.window_fill().to_normalized_gamma_f32()
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

impl RissApp {
    /// Run the ported KISS search synchronously on the calling thread.
    ///
    /// Used by the modal favourites picker, which is not the live launcher
    /// path. It deliberately shares [`crate::kiss::searcher::Searcher`] with
    /// the worker so the two can never disagree about scoring or ordering.
    pub(crate) fn search_apps_now(&mut self, query: &str, max_results: usize) -> Vec<SearchResult> {
        let mut params = self.search_params();
        params.query = query.trim().to_string();
        params.max_results = max_results;
        let index = Arc::clone(&self.index);
        let Some(hits) = self.picker_searcher.search(&params, &index, || false) else {
            return Vec::new();
        };
        hits.iter()
            .filter_map(|hit| index.get(hit.index))
            .map(|item| SearchResult {
                entry: item.entry.clone(),
                score: 0,
                match_type: MatchType::Fuzzy,
                action: ResultAction::Launch,
            })
            .collect()
    }
}

/// Hooks used by `crate::tests::live_app` to drive the real application
/// object without a window.
///
/// Each one is a thin wrapper over the production call the UI makes, so a
/// test exercises the shipped code rather than a replica. They are
/// `#[cfg(test)]`, so none of this exists in a release build.
#[cfg(test)]
impl RissApp {
    /// What the provider-loading worker's answer does when it lands.
    pub(crate) fn install_apps_for_test(&mut self, entries: Vec<AppEntry>) {
        // The constructor started a real discovery of the host's installed
        // apps. Pin the list so that scan cannot land on top of the fixture.
        self.ignore_loader = true;
        self.apply_loaded_apps(entries);
    }

    /// What a keystroke in the search bar does.
    pub(crate) fn set_query_for_test(&mut self, query: &str) {
        self.query = query.to_string();
        self.update_results();
    }

    /// What the first statement of `eframe::App::update` does.
    pub(crate) fn poll_workers_for_test(&mut self) {
        self.poll_workers();
    }

    /// True while a submitted search has not yet been applied.
    pub(crate) fn search_pending_for_test(&self) -> bool {
        !self.query.trim().is_empty()
            && self.expected_search_generation != u64::MAX
            && self.applied_search_generation != self.expected_search_generation
    }

    pub(crate) fn result_names_for_test(&self) -> Vec<String> {
        self.results
            .iter()
            .filter(|r| r.action == ResultAction::Launch && !r.entry.is_virtual())
            .map(|r| r.entry.name.clone())
            .collect()
    }

    pub(crate) fn result_is_favorite_for_test(&self, index: usize) -> bool {
        self.results[index].entry.is_favorite
    }

    /// What the star button does.
    pub(crate) fn toggle_favorite_for_test(&mut self, exec: &str) {
        self.history.toggle_favorite(exec);
        self.reapply_usage();
    }

    /// What a launch records.
    pub(crate) fn record_query_launch_for_test(&mut self, query: &str, exec: &str) {
        self.history.record_launch(exec);
        self.history.record_launch_for_query(query, exec);
        self.reapply_usage();
    }

    /// What the settings screen does.
    pub(crate) fn set_setting_for_test(&mut self, key: &str, value: &str) {
        self.settings
            .values
            .insert(key.to_string(), value.to_string());
        self.invalidate_search_cache();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history_row(exec: &str, name: &str, favorite: bool) -> SearchResult {
        let mut entry = AppEntry::virtual_entry(name, "", exec);
        entry.is_favorite = favorite;
        SearchResult {
            entry,
            score: 0,
            match_type: MatchType::Fuzzy,
            action: ResultAction::Launch,
        }
    }

    #[test]
    fn recency_puts_most_recent_first() {
        let mut history = HistoryData::default();
        // A heavy old favorite must still lose to a light recent launch.
        for _ in 0..11 {
            history.record_launch_at("heavy-old", 1000);
        }
        history.record_launch_at("recent-light", 5000);
        let mut results = vec![
            history_row("heavy-old", "Heavy Old", true),
            history_row("recent-light", "Recent Light", false),
        ];
        RissApp::sort_history_results(
            &mut results,
            &history,
            HistoryMode::Recency,
            6000,
            false,
            &HashSet::new(),
        );
        assert_eq!(results[0].entry.exec, "recent-light");
        assert_eq!(results[1].entry.exec, "heavy-old");
    }

    #[test]
    fn hide_favorites_from_history_is_respected() {
        let mut history = HistoryData::default();
        history.record_launch_at("fav", 5000);
        history.record_launch_at("plain", 4000);
        let mut results = vec![
            history_row("fav", "Fav", true),
            history_row("plain", "Plain", false),
        ];
        RissApp::sort_history_results(
            &mut results,
            &history,
            HistoryMode::Recency,
            6000,
            true,
            &HashSet::new(),
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.exec, "plain");
    }
}
