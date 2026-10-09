//! The launcher UI.
//!
//! The layout follows KISS: an optional info bar, the result list, a favourites
//! bar and the search bar at the bottom. Everything can be moved, resized or
//! hidden from the settings screen.
//!
//! Touch behaviour mirrors KISS as closely as a desktop toolkit allows:
//!
//! * a tap activates a result (or only selects it when "prevent fast launch"
//!   is enabled); a double click can be required instead;
//! * a long press — or a right click — opens the context menu;
//! * rows animate while pressed and keep a comfortable touch target;
//! * widgets inside a row (favourite star, tags) never leak their click to the
//!   row, and scrolling never launches anything.

mod results;
mod settings_view;
mod widgets;

#[cfg(target_os = "android")]
use crate::android_app_entry::{self as app_entry, AppEntry, EXEC_SETTINGS};
#[cfg(not(target_os = "android"))]
use crate::app_entry::{self as app_entry, AppEntry, EXEC_SETTINGS};
use crate::history::HistoryData;
use crate::search::{
    self, MatchType, ResultAction, ResultView, SearchConfig, SearchEngine, SearchResult,
};
use crate::settings::{
    catalog, BarPosition, GestureAction, HistoryMode, SearchBarPosition, Settings,
};
use crate::theme::{self, Palette};
use eframe::egui::{
    self, Color32, CornerRadius, Key, Margin, Modifiers, RichText, Sense, Stroke, Vec2,
};
use std::time::{Duration, Instant};

use widgets::Icon;

/// Which list is currently displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Default list: search results, or favorites + history.
    Apps,
    /// Every installed application, alphabetically.
    AllApps,
    /// Only applications that were launched before.
    History,
    /// Applications hidden from the launcher.
    Excluded,
}

/// Something a result row asked for.
#[derive(Debug, Clone)]
pub enum RowAction {
    Activate(usize),
    ToggleFavorite(usize),
    EditTags(usize),
    Menu(usize),
}

/// Something the favourites bar asked for.
#[derive(Debug, Clone)]
pub enum FavAction {
    Activate(usize),
    Menu(usize),
}

/// What the context menu was opened on.
#[derive(Debug, Clone)]
pub enum MenuTarget {
    Result(usize),
    Favorite(String),
    Launcher,
}

/// Commands produced by the context menu.
#[derive(Debug, Clone)]
pub enum MenuCommand {
    Launch(usize),
    ToggleFavorite(String),
    EditTags(String),
    Exclude(String),
    ExcludeFromHistory(String),
    Restore(String),
    ResetRank(String),
    Rename(String),
    SetShortcut(String, u8),
    PickShortcut(String),
    ClearShortcut(String),
    CopyText(String),
    Details(String),
    Refresh,
    Settings,
    AllApps,
    History,
    Excluded,
    ToggleFavoritesBar,
    ClearQuery,
    Close,
}

/// Confirmation dialogs, mirroring KISS' "reset …" buttons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogAction {
    ResetHistory,
    ResetFavorites,
    ResetTags,
    ResetExcluded,
    ResetShortcuts,
    ResetAll,
    Export,
    Import,
    Confirm,
    Cancel,
}

pub struct MenuState {
    pub target: MenuTarget,
    /// Clicks are ignored for a moment so the menu does not close instantly.
    pub guard_until: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dialog {
    pub title: String,
    pub message: String,
    pub confirm: String,
    pub cancel: String,
    pub action: DialogAction,
}

pub struct TagEditor {
    pub exec: String,
    pub input: String,
}

pub struct RenameEditor {
    pub exec: String,
    pub input: String,
}

/// Read only data handed to the rendering helpers.
pub struct ViewState<'a> {
    pub ctx: &'a egui::Context,
    pub settings: &'a Settings,
    pub palette: &'a Palette,
    pub history: &'a HistoryData,
    pub results: &'a [SearchResult],
    pub favorites: &'a [SearchResult],
    pub selected: usize,
    pub hovered: Option<usize>,
    pub query: &'a str,
    pub view: View,
    pub active_tags: &'a [String],
    pub scroll_to_selection: bool,
}

/// Main application state
pub struct RissApp {
    history: HistoryData,
    settings: Settings,
    palette: Palette,
    base_style: egui::Style,

    apps: Vec<AppEntry>,
    search_pool: Vec<AppEntry>,
    results: Vec<SearchResult>,
    favorites: Vec<SearchResult>,
    query: String,

    search_engine: SearchEngine,
    selected: usize,
    hovered: Option<usize>,
    view: View,
    active_tags: Vec<String>,

    // touch state
    press_started: Option<Instant>,
    press_index: Option<usize>,
    last_long_press: Option<(usize, Instant)>,
    last_tap: Option<(usize, Instant)>,
    favorites_press_started: Option<Instant>,
    favorites_press_index: Option<usize>,
    favorites_hovered: Option<usize>,

    // overlays
    menu: Option<MenuState>,
    dialog: Option<Dialog>,
    tag_editor: Option<TagEditor>,
    rename_editor: Option<RenameEditor>,
    settings_open: bool,
    settings_ui: settings_view::SettingsUi,
    status: Option<(String, Instant)>,

    // misc
    timer: Option<(u64, Instant)>,
    results_height: f32,
    scroll_request: Option<f32>,
    applied_fullscreen: Option<bool>,
    applied_size: Vec2,
    visible_request: Option<bool>,
    favorite_focus: Option<usize>,
    shortcut_pad: Option<String>,
    search_revealed: bool,
}

impl RissApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let history = HistoryData::load();
        let settings = Settings::load();
        let palette = theme::build(&settings, matches!(cc.egui_ctx.theme(), egui::Theme::Dark));
        let base_style = (*cc.egui_ctx.style()).clone();

        let mut app = RissApp {
            history,
            settings,
            palette,
            base_style,
            apps: Vec::new(),
            search_pool: Vec::new(),
            results: Vec::new(),
            favorites: Vec::new(),
            query: String::new(),
            search_engine: SearchEngine::new(),
            selected: 0,
            hovered: None,
            view: View::Apps,
            active_tags: Vec::new(),
            press_started: None,
            press_index: None,
            last_long_press: None,
            last_tap: None,
            favorites_press_started: None,
            favorites_press_index: None,
            favorites_hovered: None,
            menu: None,
            dialog: None,
            tag_editor: None,
            rename_editor: None,
            settings_open: false,
            settings_ui: settings_view::SettingsUi::default(),
            status: None,
            timer: None,
            results_height: 320.0,
            scroll_request: None,
            applied_fullscreen: None,
            applied_size: Vec2::ZERO,
            visible_request: None,
            favorite_focus: None,
            shortcut_pad: None,
            search_revealed: false,
        };
        app.reload_apps();
        app.update_results();
        app.selected = if app.settings.select_last_result {
            app.results.len().saturating_sub(1)
        } else {
            0
        };
        app
    }

    // --- data ------------------------------------------------------------

    /// Re-scan the system for applications and re-apply the stored data.
    pub fn reload_apps(&mut self) {
        let mut apps = app_entry::discover_apps();
        for builtin in app_entry::builtin_entries() {
            let dominated = apps
                .iter()
                .any(|a| a.name.to_lowercase() == builtin.name.to_lowercase());
            if !dominated {
                apps.push(builtin);
            }
        }
        apps.sort_by_key(|a| a.name.to_lowercase());
        self.apps = apps;
        self.apply_history();
    }

    /// Copy history, favorites, tags and exclusions onto the app list.
    fn apply_history(&mut self) {
        let default_tags = self.settings.favorites_tags.clone();
        for app in &mut self.apps {
            app.launch_count = self.history.get_launch_count(&app.exec);
            app.last_launched = self.history.get_last_launched(&app.exec);
            app.is_favorite = self.history.is_favorite(&app.exec);
            app.tags = self.history.get_tags(&app.exec);
            if app.tags.is_empty() && app.is_favorite {
                // KISS tags favorites with a default tag so they can be found.
                app.tags = default_tags.clone();
            }
        }
        self.search_pool = self
            .apps
            .iter()
            .filter(|app| !self.history.is_excluded(&app.exec))
            .cloned()
            .collect();
        self.rebuild_favorites();
    }

    fn rebuild_favorites(&mut self) {
        let limit = self.settings.favorites_limit.clamp(1, 64);
        let mut favorites: Vec<SearchResult> = self
            .apps
            .iter()
            .filter(|app| app.is_favorite)
            .cloned()
            .map(|entry| SearchResult {
                entry,
                score: 0,
                match_type: MatchType::Exact,
                action: ResultAction::Launch,
            })
            .collect();
        favorites.sort_by_key(|result| result.entry.name.to_lowercase());
        favorites.truncate(limit);
        self.favorites = favorites;
    }

    fn search_config(&self, max_results: usize) -> SearchConfig {
        SearchConfig {
            max_results,
            precision: self.settings.min_match_precision,
            legacy_fuzzy: self.settings.legacy_fuzzy,
            exclude_favorites: self.settings.exclude_favorites_from_apps,
            active_tags: self.active_tags.clone(),
            show_untagged: self.settings.show_untagged,
            track_history: self.settings.enable_app_history,
        }
    }

    /// How many results fit on screen (KISS' "adaptive results").
    fn max_visible_results(&self) -> usize {
        let limit = self.settings.history_length.clamp(1, 150);
        if !self.settings.adaptive_results {
            return limit;
        }
        let row_height = self.settings.row_height(12).max(16.0);
        let fitting = (self.results_height / row_height).floor() as usize;
        fitting.clamp(1, limit)
    }

    fn update_results(&mut self) {
        let max_results = self.max_visible_results();
        let query = self.query.trim().to_string();
        let mut results: Vec<SearchResult>;

        match self.view {
            View::Apps => {
                if query.is_empty() {
                    results = search::default_results(
                        &self.search_pool,
                        &self.history.aliases,
                        self.settings.history_mode,
                        max_results,
                    );
                    if self.settings.exclude_favorites_from_apps {
                        results.retain(|result| !result.entry.is_favorite);
                    }
                } else {
                    results = self.search_engine.search(
                        &query,
                        &self.search_pool,
                        &self.history.aliases,
                        &self.search_config(max_results),
                    );
                    let app_matches = results.len();
                    results.extend(self.provider_results(&query, app_matches));
                }
            }
            View::AllApps => {
                let mut all: Vec<SearchResult> = self
                    .search_pool
                    .iter()
                    .cloned()
                    .map(|entry| SearchResult {
                        entry,
                        score: 0,
                        match_type: MatchType::Provider,
                        action: ResultAction::Launch,
                    })
                    .collect();
                all.sort_by_key(|result| result.entry.name.to_lowercase());
                results = all;
            }
            View::History => {
                let mut used: Vec<SearchResult> = self
                    .search_pool
                    .iter()
                    .filter(|app| app.launch_count > 0)
                    .cloned()
                    .map(|entry| SearchResult {
                        score: history_score(&entry, self.settings.history_mode),
                        entry,
                        match_type: MatchType::History,
                        action: ResultAction::Launch,
                    })
                    .collect();
                used.sort_by(|a, b| {
                    b.score.cmp(&a.score).then_with(|| {
                        a.entry
                            .name
                            .to_lowercase()
                            .cmp(&b.entry.name.to_lowercase())
                    })
                });
                if self.settings.exclude_favorites_from_history {
                    used.retain(|result| !result.entry.is_favorite);
                }
                results = used;
            }
            View::Excluded => {
                results = self
                    .apps
                    .iter()
                    .filter(|app| self.history.is_excluded(&app.exec))
                    .cloned()
                    .map(|entry| {
                        let exec = entry.exec.clone();
                        SearchResult {
                            entry,
                            score: 0,
                            match_type: MatchType::Provider,
                            action: ResultAction::Excluded { exec },
                        }
                    })
                    .collect();
                results.sort_by_key(|result| result.entry.name.to_lowercase());
            }
        }

        self.results = results;
        if self.results.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.results.len() {
            self.selected = self.results.len() - 1;
        }
    }

    /// Results contributed by the optional providers: shortcuts, settings,
    /// excluded apps, timer, previous searches, web search, shell commands and
    /// the calculator.
    fn provider_results(&self, query: &str, app_matches: usize) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = Vec::new();
        let settings = &self.settings;

        // A digit bound to a shortcut jumps straight to that application.
        if let Some(digit) = query.parse::<u8>().ok().filter(|d| (1..=9).contains(d)) {
            if let Some(exec) = self.history.shortcut_for(digit) {
                if let Some(app) = self.search_pool.iter().find(|app| &app.exec == exec) {
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

        // Settings provider: the settings can be searched from the query bar.
        if settings.provider_settings {
            let needle = query.to_lowercase();
            let mut hits: Vec<SearchResult> = catalog(settings)
                .into_iter()
                .filter(|entry| {
                    let haystack = format!(
                        "{} {} {} {}",
                        entry.title,
                        entry.keywords,
                        entry.section.title(),
                        entry.value
                    )
                    .to_lowercase();
                    haystack.contains(&needle)
                })
                .take(3)
                .map(|entry| {
                    search::setting_result(
                        entry.title,
                        entry.section.title(),
                        entry.id,
                        &entry.value,
                    )
                })
                .collect();
            hits.truncate(3);
            results.extend(hits);
        }

        // Excluded applications stay reachable through search.
        if settings.provider_excluded {
            for app in &self.apps {
                if self.history.is_excluded(&app.exec)
                    && search::display_name(app, &self.history.aliases)
                        .to_lowercase()
                        .contains(&query.to_lowercase())
                {
                    results.push(search::excluded_result(app));
                }
            }
        }

        // Timer provider.
        if settings.provider_timer {
            if let Some(seconds) = crate::providers::parse_timer(query) {
                results.push(search::timer_result(seconds));
            }
        }

        // Previously run web searches.
        if settings.search_through_history {
            for previous in self.history.recent_searches(20) {
                if previous.to_lowercase().contains(&query.to_lowercase()) {
                    let mut result =
                        self.web_search_result(&settings.default_web_provider, &previous);
                    result.entry.name = previous.clone();
                    result.score = 200;
                    results.push(result);
                }
            }
        }

        // The special lists stay reachable by typing their name.
        if query.len() > 2 {
            let lowered = query.to_lowercase();
            let special = match lowered.as_str() {
                "history" | "recent" => Some((ResultView::History, "History")),
                "apps" | "all apps" | "all" => Some((ResultView::AllApps, "All applications")),
                "settings" | "preferences" => Some((ResultView::Settings, "Settings")),
                "excluded" | "excluded apps" => Some((ResultView::Excluded, "Excluded apps")),
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
        if settings.provider_web {
            results.push(self.web_search_result(&settings.default_web_provider, query));
        }

        // Shell command, only when nothing else looks like a match.
        if settings.provider_exec && app_matches == 0 && query.contains(' ') {
            results.push(search::exec_result(query));
        }

        // The calculator always goes last.
        if settings.provider_calc {
            if let Some(answer) = search::try_calculate(query) {
                let value = answer.trim_start_matches("= ").trim().to_string();
                let mut result = search::copy_result(&value, "Calculator — activate to copy");
                result.entry.name = format!("{}   {}", query, answer);
                result.score = 50;
                results.push(result);
            }
        }

        results.sort_by_key(|result| std::cmp::Reverse(result.score));
        results
    }

    fn web_search_result(&self, provider: &str, query: &str) -> SearchResult {
        let template = self
            .settings
            .provider_by_name(provider)
            .map(|found| found.url)
            .unwrap_or_else(|| provider_url_for(provider));
        search::web_search_result(provider, &template, query)
    }

    // --- actions ---------------------------------------------------------

    fn set_status(&mut self, message: String) {
        self.status = Some((message, Instant::now()));
    }

    pub fn display_name(&self, entry: &AppEntry) -> String {
        search::display_name(entry, &self.history.aliases)
    }

    fn activate(&mut self, index: usize) {
        let action = match self.results.get(index) {
            Some(result) => result.action.clone(),
            None => return,
        };
        match action {
            ResultAction::Launch => self.launch_app(index),
            ResultAction::WebSearch { provider, query } => {
                let template = self
                    .settings
                    .provider_by_name(&provider)
                    .map(|found| found.url)
                    .unwrap_or_else(|| provider_url_for(&provider));
                let url = crate::providers::provider_url(&template, &query);
                match crate::providers::open_url(&url) {
                    Ok(()) => {
                        self.history.record_search(&query);
                        self.history.save();
                        self.set_status(format!("Searching {} for “{}”", provider, query));
                    }
                    Err(error) => self.set_status(error),
                }
                self.clear_query();
            }
            ResultAction::Exec { command } => match crate::providers::run_command(&command) {
                Ok(()) => {
                    self.history.record_exec(&command);
                    self.history.save();
                    self.set_status(format!("Ran {}", command));
                }
                Err(error) => self.set_status(error),
            },
            ResultAction::Timer { seconds } => {
                if self.timer.is_some() {
                    self.timer = None;
                    self.set_status("Timer cancelled".to_string());
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
                    self.set_status(format!("{} — install xclip to copy", text));
                }
            }
            ResultAction::Setting { id } => self.focus_setting(&id),
            ResultAction::View(view) => {
                self.view = match view {
                    ResultView::History => View::History,
                    ResultView::AllApps => View::AllApps,
                    ResultView::Settings => {
                        self.settings_open = true;
                        View::Apps
                    }
                    ResultView::Excluded => View::Excluded,
                };
                self.selected = 0;
                self.update_results();
            }
            ResultAction::Excluded { exec } => {
                self.history.remove_excluded(&exec);
                self.history.save();
                self.apply_history();
                self.update_results();
                self.set_status("Application restored".to_string());
            }
        }
    }

    fn launch_app(&mut self, index: usize) {
        let (exec, name) = match self.results.get(index) {
            Some(result) => (result.entry.exec.clone(), self.display_name(&result.entry)),
            None => return,
        };

        if exec == EXEC_SETTINGS {
            self.settings_open = true;
            return;
        }

        let frozen = self.settings.freeze_history;
        let launched = match self.results.get(index) {
            Some(result) => result.entry.launch(),
            None => return,
        };
        match launched {
            Ok(()) => {
                if !frozen && !self.history.is_excluded_from_history(&exec) {
                    self.history.record_launch(&exec);
                    self.history.prune();
                    self.history.save();
                }
                self.apply_history();
                self.set_status(format!("Launched {}", name));
                self.clear_query();
                if self.settings.hide_on_launch {
                    self.visible_request = Some(false);
                }
            }
            Err(error) => self.set_status(error),
        }
    }

    fn clear_query(&mut self) {
        if !self.query.is_empty() {
            self.query.clear();
            self.selected = 0;
            self.search_revealed = false;
            self.update_results();
        }
    }

    fn toggle_favorite(&mut self, exec: &str) {
        let is_favorite = self.history.toggle_favorite(exec);
        let name = self
            .apps
            .iter()
            .find(|app| app.exec == exec)
            .map(|app| self.display_name(app))
            .unwrap_or_else(|| exec.to_string());
        self.history.save();
        self.apply_history();
        self.update_results();
        self.set_status(if is_favorite {
            format!("Added {} to favorites", name)
        } else {
            format!("Removed {} from favorites", name)
        });
    }

    fn toggle_excluded(&mut self, exec: &str) {
        if self.history.is_excluded(exec) {
            self.history.remove_excluded(exec);
            self.set_status("Application restored".to_string());
        } else {
            self.history.add_excluded(exec);
            self.set_status("Application excluded — long press to restore".to_string());
        }
        self.history.save();
        self.apply_history();
        self.update_results();
    }

    fn reset_rank(&mut self, exec: &str) {
        self.history.reset_rank(exec);
        self.history.save();
        self.apply_history();
        self.update_results();
        self.set_status("Usage statistics cleared".to_string());
    }

    fn save_tags(&mut self, exec: &str, input: &str) {
        let tags: Vec<String> = input
            .split(',')
            .map(|tag| tag.trim().trim_start_matches('#').to_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        self.history.set_tags(exec, tags);
        self.history.save();
        self.apply_history();
        self.update_results();
        self.set_status("Tags saved".to_string());
    }

    fn set_shortcut(&mut self, exec: &str, key: u8) {
        self.history.set_shortcut(key, exec);
        self.history.save();
        self.set_status(format!("Pinned to {}", key));
    }

    fn focus_setting(&mut self, id: &str) {
        self.settings_open = true;
        self.settings_ui.filter.clear();
        self.settings_ui.highlight = None;
        for entry in catalog(&self.settings) {
            if entry.id == id {
                self.settings_ui.section = entry.section;
                self.settings_ui.highlight = Some(entry.id.to_string());
                break;
            }
        }
    }

    /// Change settings and persist them.
    fn patch_settings(&mut self, change: impl FnOnce(&mut Settings)) {
        let mut next = self.settings.clone();
        change(&mut next);
        next.save();
        self.settings = next;
        self.apply_history();
        self.update_results();
    }

    /// A tap on a result row.
    fn on_result_click(&mut self, index: usize) {
        let now = Instant::now();
        let timeout = Duration::from_millis(self.settings.tap_timeout_ms.max(200) as u64);
        let repeat = self
            .last_tap
            .filter(|(previous, at)| *previous == index && now.duration_since(*at) < timeout)
            .is_some();

        if self.settings.double_click_launches {
            if repeat {
                self.last_tap = None;
                self.activate(index);
            } else {
                self.last_tap = Some((index, now));
                self.selected = index;
                self.set_status("Double click to launch".to_string());
            }
            return;
        }

        if self.settings.prevent_fast_launch && !repeat {
            self.last_tap = Some((index, now));
            self.selected = index;
            self.set_status("Press again to launch".to_string());
            return;
        }

        self.last_tap = Some((index, now));
        self.activate(index);
    }

    fn open_menu(&mut self, target: MenuTarget) {
        self.menu = Some(MenuState {
            target,
            guard_until: Instant::now() + Duration::from_millis(180),
        });
    }

    fn on_favorite_click(&mut self, index: usize) {
        let exec = match self.favorites.get(index) {
            Some(result) => result.entry.exec.clone(),
            None => return,
        };
        self.favorite_focus = Some(index);
        if !self.settings.single_tap_favorite_launches {
            self.selected = index;
            self.set_status("Single tap launching is disabled in the settings".to_string());
            return;
        }
        if exec == EXEC_SETTINGS {
            self.settings_open = true;
            return;
        }
        let frozen = self.settings.freeze_history;
        let launched = self.favorites[index].entry.launch();
        match launched {
            Ok(()) => {
                if !frozen && !self.history.is_excluded_from_history(&exec) {
                    self.history.record_launch(&exec);
                    self.history.prune();
                    self.history.save();
                    self.apply_history();
                }
                let name = self.display_name(&self.favorites[index].entry);
                self.set_status(format!("Launched {}", name));
                if self.settings.hide_on_launch {
                    self.visible_request = Some(false);
                }
            }
            Err(error) => self.set_status(error),
        }
    }

    fn apply_menu_command(&mut self, command: MenuCommand) {
        match command {
            MenuCommand::Launch(index) => self.activate(index),
            MenuCommand::ToggleFavorite(exec) => self.toggle_favorite(&exec),
            MenuCommand::EditTags(exec) => {
                let tags = self
                    .apps
                    .iter()
                    .find(|app| app.exec == exec)
                    .map(|app| app.tags.join(", "))
                    .unwrap_or_default();
                self.tag_editor = Some(TagEditor { exec, input: tags });
            }
            MenuCommand::Exclude(exec) => self.toggle_excluded(&exec),
            MenuCommand::ExcludeFromHistory(exec) => {
                if self.history.is_excluded_from_history(&exec) {
                    self.history.remove_excluded_from_history(&exec);
                    self.set_status("Usage is recorded again".to_string());
                } else {
                    self.history.add_excluded_from_history(&exec);
                    self.set_status("Usage is no longer recorded".to_string());
                }
                self.history.save();
            }
            MenuCommand::Restore(exec) => {
                self.history.remove_excluded(&exec);
                self.history.save();
                self.apply_history();
                self.update_results();
                self.set_status("Application restored".to_string());
            }
            MenuCommand::ResetRank(exec) => self.reset_rank(&exec),
            MenuCommand::Rename(exec) => {
                let alias = self.history.alias(&exec).cloned().unwrap_or_else(|| {
                    self.apps
                        .iter()
                        .find(|app| app.exec == exec)
                        .map(|app| app.name.clone())
                        .unwrap_or_default()
                });
                self.rename_editor = Some(RenameEditor { exec, input: alias });
            }
            MenuCommand::SetShortcut(exec, key) => self.set_shortcut(&exec, key),
            MenuCommand::PickShortcut(exec) => self.shortcut_pad = Some(exec),
            MenuCommand::ClearShortcut(exec) => {
                for key in 1..=9u8 {
                    if self.history.shortcut_for(key) == Some(&exec) {
                        self.history.clear_shortcut(key);
                    }
                }
                self.history.save();
                self.set_status("Shortcut removed".to_string());
            }
            MenuCommand::CopyText(text) => {
                if crate::providers::copy_to_clipboard(&text) {
                    self.set_status("Copied to clipboard".to_string());
                } else {
                    self.set_status("Install xclip or wl-clipboard to copy".to_string());
                }
            }
            MenuCommand::Details(exec) => {
                let path = self
                    .apps
                    .iter()
                    .find(|app| app.exec == exec)
                    .map(|app| app.desktop_file.clone());
                if let Some(path) = path {
                    if let Some(parent) = path.parent() {
                        let command = format!("xdg-open \"{}\"", parent.display());
                        let _ = crate::providers::run_command(&command);
                    }
                }
            }
            MenuCommand::Refresh => {
                self.reload_apps();
                self.update_results();
                self.set_status("Applications reloaded".to_string());
            }
            MenuCommand::Settings => self.settings_open = true,
            MenuCommand::AllApps => {
                self.view = View::AllApps;
                self.selected = 0;
                self.update_results();
            }
            MenuCommand::History => {
                self.view = View::History;
                self.selected = 0;
                self.update_results();
            }
            MenuCommand::Excluded => {
                self.view = View::Excluded;
                self.selected = 0;
                self.update_results();
            }
            MenuCommand::ToggleFavoritesBar => {
                let enabled = !self.settings.favorites_bar;
                self.patch_settings(|settings| settings.favorites_bar = enabled);
            }
            MenuCommand::ClearQuery => self.clear_query(),
            MenuCommand::Close => self.menu = None,
        }
    }

    fn apply_dialog(&mut self, action: DialogAction) {
        match action {
            DialogAction::ResetHistory => {
                self.history.reset_history();
                self.history.save();
                self.apply_history();
                self.update_results();
                self.set_status("History cleared".to_string());
            }
            DialogAction::ResetFavorites => {
                self.history.reset_favorites();
                self.history.save();
                self.apply_history();
                self.update_results();
                self.set_status("Favorites cleared".to_string());
            }
            DialogAction::ResetTags => {
                self.history.reset_tags();
                self.history.save();
                self.apply_history();
                self.update_results();
                self.set_status("Tags cleared".to_string());
            }
            DialogAction::ResetExcluded => {
                self.history.reset_excluded();
                self.history.save();
                self.apply_history();
                self.update_results();
                self.set_status("Exclusion lists cleared".to_string());
            }
            DialogAction::ResetShortcuts => {
                self.history.clear_all_shortcuts();
                self.history.save();
                self.set_status("Shortcuts cleared".to_string());
            }
            DialogAction::ResetAll => {
                self.history.reset_all();
                self.history.save();
                self.apply_history();
                self.update_results();
                self.set_status("All data cleared".to_string());
            }
            DialogAction::Export => {
                let path = export_backup();
                self.set_status(format!("Exported to {}", path));
            }
            DialogAction::Import => match import_backup() {
                Ok(message) => {
                    self.history = HistoryData::load();
                    self.settings = Settings::load();
                    self.apply_history();
                    self.update_results();
                    self.set_status(message);
                }
                Err(error) => self.set_status(error),
            },
            DialogAction::Confirm | DialogAction::Cancel => {}
        }
    }

    // --- input -----------------------------------------------------------

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let settings = self.settings.clone();

        // KISS gestures, mapped to keyboard shortcuts (Ctrl+Alt + key).
        let gesture_keys = [
            (Key::ArrowUp, settings.gesture_up),
            (Key::ArrowDown, settings.gesture_down),
            (Key::ArrowLeft, settings.gesture_left),
            (Key::ArrowRight, settings.gesture_right),
            (Key::L, settings.gesture_long_press),
        ];
        for (key, action) in gesture_keys {
            if action == GestureAction::None {
                continue;
            }
            let pressed =
                ctx.input_mut(|input| input.consume_key(Modifiers::CTRL | Modifiers::ALT, key));
            if pressed {
                self.run_gesture(action);
                return;
            }
        }

        if ctx.input(|input| input.key_pressed(Key::F5)) {
            self.reload_apps();
            self.update_results();
            self.set_status("Applications reloaded".to_string());
            return;
        }

        if ctx.input(|input| input.key_pressed(Key::Escape)) {
            self.on_escape(ctx);
            return;
        }

        if ctx.input(|input| input.key_pressed(Key::Enter)) {
            if let Some(dialog) = self.dialog.take() {
                self.apply_dialog(dialog.action);
            } else if self.menu.is_some() {
                self.menu = None;
            } else if !self.settings_open && !self.results.is_empty() {
                self.on_result_click(self.selected);
            }
            return;
        }

        if self.settings_open {
            return;
        }

        // Number keys launch the nth result when the query is empty.
        if settings.number_keys_to_launch && self.query.is_empty() {
            let keys = [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
                Key::Num9,
            ];
            for (index, key) in keys.iter().enumerate() {
                if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, *key)) {
                    self.selected = index;
                    self.on_result_click(index);
                    return;
                }
            }
        }

        let count = self.results.len();
        if count == 0 {
            return;
        }

        let up = ctx.input(|input| input.key_pressed(Key::ArrowUp));
        let down = ctx.input(|input| input.key_pressed(Key::ArrowDown));
        let tab = ctx.input(|input| input.key_pressed(Key::Tab));
        let shift_tab = ctx.input(|input| input.key_pressed(Key::Tab) && input.modifiers.shift);
        let home = ctx.input(|input| input.key_pressed(Key::Home));
        let end = ctx.input(|input| input.key_pressed(Key::End));
        let page_down = ctx.input(|input| input.key_pressed(Key::PageDown));
        let page_up = ctx.input(|input| input.key_pressed(Key::PageUp));

        if page_down {
            self.selected = (self.selected + 5).min(count - 1);
        } else if page_up {
            self.selected = self.selected.saturating_sub(5);
        } else if shift_tab {
            self.selected = if self.selected == 0 {
                count - 1
            } else {
                self.selected - 1
            };
        } else if tab {
            self.selected = (self.selected + 1) % count;
        } else if home {
            self.selected = 0;
        } else if end {
            self.selected = count - 1;
        } else if up {
            self.selected = if self.selected == 0 {
                count - 1
            } else {
                self.selected - 1
            };
        } else if down {
            self.selected = (self.selected + 1) % count;
        } else {
            return;
        }
        self.request_scroll_to_selection();
    }

    fn run_gesture(&mut self, action: GestureAction) {
        match action {
            GestureAction::None => {}
            GestureAction::ShowHistory => {
                self.view = View::History;
                self.selected = 0;
                self.update_results();
            }
            GestureAction::ShowAllApps => {
                self.view = View::AllApps;
                self.selected = 0;
                self.update_results();
            }
            GestureAction::ShowSettings => self.settings_open = true,
            GestureAction::ShowExcluded => {
                self.view = View::Excluded;
                self.selected = 0;
                self.update_results();
            }
            GestureAction::ShowTagsMenu => {
                let enabled = !self.settings.tags_menu;
                self.patch_settings(|settings| settings.tags_menu = enabled);
            }
            GestureAction::ShowSearchBar => {
                let position = if self.settings.search_bar_position == SearchBarPosition::Hidden {
                    SearchBarPosition::Bottom
                } else {
                    SearchBarPosition::Hidden
                };
                self.patch_settings(|settings| settings.search_bar_position = position);
            }
            GestureAction::HideWindow => self.visible_request = Some(false),
            GestureAction::LaunchSelected => {
                if !self.results.is_empty() {
                    self.on_result_click(self.selected);
                }
            }
            GestureAction::ClearQuery => self.clear_query(),
            GestureAction::Relaunch => {
                self.reload_apps();
                self.update_results();
                self.set_status("Applications reloaded".to_string());
            }
        }
    }

    fn on_escape(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() {
            self.dialog = None;
        } else if self.menu.is_some() {
            self.menu = None;
        } else if self.tag_editor.is_some() || self.rename_editor.is_some() {
            self.tag_editor = None;
            self.rename_editor = None;
        } else if self.settings_open {
            self.settings_open = false;
            self.settings_ui.highlight = None;
        } else if self.view != View::Apps {
            self.view = View::Apps;
            self.selected = 0;
            self.update_results();
        } else if !self.active_tags.is_empty() {
            self.active_tags.clear();
            self.selected = 0;
            self.update_results();
        } else if !self.query.is_empty() {
            self.clear_query();
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }

    fn request_scroll_to_selection(&mut self) {
        self.scroll_request = Some(self.selected as f32);
    }

    /// Long press detection on the result list.
    fn update_press_state(&mut self, ctx: &egui::Context) {
        if ctx.input(|input| input.pointer.any_pressed()) {
            self.last_long_press = None;
            self.press_started = None;
            self.last_tap = None;
        }
        let pointer_down = ctx.input(|input| input.pointer.any_down());
        match (self.hovered, pointer_down) {
            (Some(index), true) => {
                if self.press_index != Some(index) || self.press_started.is_none() {
                    self.press_index = Some(index);
                    self.press_started = Some(Instant::now());
                }
                if self.settings.long_press_menu && self.last_long_press.is_none() {
                    let delay = Duration::from_millis(
                        self.settings.long_press_delay_ms.clamp(200, 2000) as u64,
                    );
                    let fired = self
                        .press_started
                        .map(|started| started.elapsed() >= delay)
                        .unwrap_or(false);
                    if fired {
                        self.last_long_press = Some((index, Instant::now()));
                        self.press_started = None;
                        self.open_menu(MenuTarget::Result(index));
                    }
                }
                if self.press_started.is_some() {
                    ctx.request_repaint_after(Duration::from_millis(50));
                }
            }
            _ => {
                self.press_index = None;
                self.press_started = None;
            }
        }
    }

    /// Long press detection on the favourites bar.
    fn update_favorites_press_state(&mut self, ctx: &egui::Context) {
        if !self.settings.long_press_menu {
            return;
        }
        let pointer_down = ctx.input(|input| input.pointer.any_down());
        match (self.favorites_hovered, pointer_down) {
            (Some(index), true) => {
                if self.favorites_press_index != Some(index)
                    || self.favorites_press_started.is_none()
                {
                    self.favorites_press_index = Some(index);
                    self.favorites_press_started = Some(Instant::now());
                }
                let delay = Duration::from_millis(
                    self.settings.long_press_delay_ms.clamp(200, 2000) as u64,
                );
                let fired = self
                    .favorites_press_started
                    .map(|started| started.elapsed() >= delay)
                    .unwrap_or(false);
                if fired {
                    self.favorites_press_started = None;
                    if let Some(result) = self.favorites.get(index) {
                        let exec = result.entry.exec.clone();
                        self.open_menu(MenuTarget::Favorite(exec));
                    }
                } else {
                    ctx.request_repaint_after(Duration::from_millis(50));
                }
            }
            _ => {
                self.favorites_press_index = None;
                self.favorites_press_started = None;
            }
        }
    }

    fn apply_window_settings(&mut self, ctx: &egui::Context) {
        if self.applied_fullscreen != Some(self.settings.fullscreen) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.settings.fullscreen));
            self.applied_fullscreen = Some(self.settings.fullscreen);
        }
        let width = self.settings.window_width.clamp(320.0, 1600.0);
        let height = if self.settings.force_portrait {
            (width / 0.62).clamp(400.0, 2000.0)
        } else {
            self.settings.window_height.clamp(360.0, 2000.0)
        };
        let wanted = Vec2::new(width, height);
        if self.applied_size != wanted {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(wanted));
            self.applied_size = wanted;
        }
    }

    fn apply_visuals(&mut self, ctx: &egui::Context) {
        let system_dark = matches!(ctx.theme(), egui::Theme::Dark);
        self.palette = theme::build(&self.settings, system_dark);
        let palette = self.palette;

        let mut style = self.base_style.clone();
        let scale = self.settings.font_scale.clamp(0.6, 2.0);
        for font in style.text_styles.values_mut() {
            font.size *= scale;
        }
        style.spacing.item_spacing.y = 2.0 + self.settings.widget_spacing.clamp(0.0, 24.0);

        let mut visuals = if palette.is_dark() {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.override_text_color = Some(palette.text);
        visuals.panel_fill = palette.bg;
        visuals.window_fill = palette.bg;
        visuals.extreme_bg_color = palette.bg;
        visuals.faint_bg_color = palette.surface;
        visuals.code_bg_color = palette.surface_alt;
        visuals.selection.bg_fill = palette.accent;
        visuals.selection.stroke = Stroke::new(1.0f32, palette.accent);
        visuals.hyperlink_color = palette.accent;
        visuals.warn_fg_color = palette.favorite;
        visuals.error_fg_color = palette.danger;
        visuals.widgets.noninteractive.bg_fill = palette.surface;
        visuals.widgets.noninteractive.weak_bg_fill = palette.surface;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0f32, palette.text);
        // egui draws the scroll bar with the non interactive background stroke.
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(6.0f32, self.palette.scrollbar());
        visuals.widgets.inactive.bg_fill = palette.surface_alt;
        visuals.widgets.inactive.weak_bg_fill = palette.surface_alt;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0f32, palette.text);
        visuals.widgets.hovered.bg_fill = palette.hover;
        visuals.widgets.hovered.weak_bg_fill = palette.hover;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0f32, palette.text);
        visuals.widgets.active.bg_fill = palette.selected;
        visuals.widgets.active.weak_bg_fill = palette.selected;
        visuals.widgets.active.fg_stroke = Stroke::new(1.5f32, palette.accent);
        style.visuals = visuals;
        ctx.set_style(style);
    }

    /// Read only snapshot of the state used by the rendering helpers.
    fn view_state<'a>(&'a self, ctx: &'a egui::Context) -> ViewState<'a> {
        ViewState {
            ctx,
            settings: &self.settings,
            palette: &self.palette,
            history: &self.history,
            results: &self.results,
            favorites: &self.favorites,
            selected: self.selected,
            hovered: self.hovered,
            query: &self.query,
            view: self.view,
            active_tags: &self.active_tags,
            scroll_to_selection: self.scroll_request.is_some(),
        }
    }

    fn show_launcher(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let settings = self.settings.clone();
        let palette = self.palette;
        let width = ui.available_width();

        let info_top = settings.info_bar_position == BarPosition::Top;
        let show_favorites = settings.favorites_bar && !self.favorites.is_empty();
        let show_search = settings.search_bar_position != SearchBarPosition::Hidden;
        let show_tags = settings.tags_menu && settings.tags_visible;
        let show_hints = settings.show_keyboard_hints && show_search;

        let info_height = 34.0;
        let favorites_height = if settings.large_favorites_bar {
            62.0
        } else {
            46.0
        };
        let revealed =
            !settings.hide_search_bar_on_start || self.search_revealed || !self.query.is_empty();
        let search_height = if !revealed {
            36.0
        } else if settings.large_search_bar {
            54.0
        } else {
            44.0
        } + if show_hints { 18.0 } else { 0.0 };
        let status_height = 18.0;
        let tags_height = if show_tags { 28.0 } else { 0.0 };

        let mut reserved = status_height + tags_height;
        if info_top {
            reserved += info_height;
        }
        if show_favorites {
            reserved += favorites_height;
        }
        if show_search {
            reserved += search_height;
        }

        let results_height = (ui.available_height() - reserved).max(60.0);
        self.results_height = results_height;

        let app_count = self.apps.len();
        let result_count = self.results.len();
        let view = self.view;
        let mut actions: Vec<RowAction> = Vec::new();
        let mut fav_actions: Vec<FavAction> = Vec::new();

        if info_top {
            let mut info_action = None;
            ui.allocate_ui(Vec2::new(width, info_height), |ui| {
                info_action = show_info_bar(ui, &settings, &palette, view, app_count, result_count);
            });
            if let Some(action) = info_action {
                self.apply_info_action(action);
            }
        }

        if show_favorites && settings.favorites_bar_position == BarPosition::Top {
            let outcome = {
                let state = self.view_state(ctx);
                results::show_favorites_bar(ui, &state)
            };
            self.favorites_hovered = outcome.hovered;
            fav_actions.extend(outcome.actions);
        }

        if settings.search_bar_position == SearchBarPosition::Top {
            self.show_search_bar(ui);
        }

        if show_tags {
            let mut tags = collect_tags(&self.apps);
            tags.truncate(24);
            let active = self.active_tags.clone();
            let mut toggled: Option<String> = None;
            ui.allocate_ui(Vec2::new(width, tags_height), |ui| {
                ui.horizontal_wrapped(|ui| {
                    for tag in &tags {
                        let is_active = active.contains(tag);
                        if widgets::chip(ui, tag, is_active, &palette) {
                            toggled = Some(tag.clone());
                        }
                    }
                });
            });
            if let Some(tag) = toggled {
                if self.active_tags.contains(&tag) {
                    self.active_tags.retain(|item| item != &tag);
                } else {
                    self.active_tags.push(tag);
                }
                self.selected = 0;
                self.update_results();
            }
        }

        let (outcome, empty) = {
            let state = self.view_state(ctx);
            let outcome = results::show_results(ui, &state, results_height);
            let empty = outcome.rows.is_empty();
            (outcome, empty)
        };
        self.hovered = outcome.hovered;
        actions.extend(outcome.actions);
        self.scroll_request = None;
        if empty {
            show_empty_hint(ui, &palette, view, &self.query);
        }

        if show_favorites && settings.favorites_bar_position == BarPosition::Bottom {
            let selected = self.favorite_focus.unwrap_or(0);
            let outcome = {
                let mut state = self.view_state(ctx);
                state.selected = selected;
                results::show_favorites_bar(ui, &state)
            };
            self.favorites_hovered = outcome.hovered;
            fav_actions.extend(outcome.actions);
        }

        if settings.search_bar_position == SearchBarPosition::Middle {
            self.show_search_bar(ui);
        }

        if !info_top {
            let mut info_action = None;
            ui.allocate_ui(Vec2::new(width, info_height), |ui| {
                info_action = show_info_bar(ui, &settings, &palette, view, app_count, result_count);
            });
            if let Some(action) = info_action {
                self.apply_info_action(action);
            }
        }

        if show_search && settings.search_bar_position == SearchBarPosition::Bottom {
            self.show_search_bar(ui);
        }

        ui.allocate_ui(Vec2::new(width, status_height), |ui| {
            let message = self
                .status
                .as_ref()
                .map(|(text, _)| text.clone())
                .unwrap_or_default();
            if message.is_empty() {
                let hint = match view {
                    View::Apps => "Type to search · ↑↓ select · ↵ launch",
                    View::AllApps => "All applications",
                    View::History => "Recently used applications",
                    View::Excluded => "Excluded applications — activate to restore",
                };
                let _ = ui.label(RichText::new(hint).size(10.0).color(palette.text_dim));
            } else {
                let _ = ui.label(RichText::new(message).size(11.0).color(palette.accent));
            }
        });

        for action in fav_actions {
            match action {
                FavAction::Activate(index) => self.on_favorite_click(index),
                FavAction::Menu(index) => {
                    if let Some(result) = self.favorites.get(index) {
                        let exec = result.entry.exec.clone();
                        self.open_menu(MenuTarget::Favorite(exec));
                    }
                }
            }
        }
        for action in actions {
            match action {
                RowAction::Activate(index) => {
                    if self.last_long_press.is_none() {
                        self.on_result_click(index);
                    }
                }
                RowAction::ToggleFavorite(index) => {
                    if let Some(result) = self.results.get(index) {
                        let exec = result.entry.exec.clone();
                        self.toggle_favorite(&exec);
                    }
                }
                RowAction::EditTags(index) => {
                    if let Some(result) = self.results.get(index) {
                        let exec = result.entry.exec.clone();
                        let tags = result.entry.tags.join(", ");
                        self.tag_editor = Some(TagEditor { exec, input: tags });
                    }
                }
                RowAction::Menu(index) => self.open_menu(MenuTarget::Result(index)),
            }
        }
    }

    fn apply_info_action(&mut self, action: InfoAction) {
        match action {
            InfoAction::Menu => self.open_menu(MenuTarget::Launcher),
            InfoAction::Settings => self.settings_open = true,
            InfoAction::Refresh => {
                self.reload_apps();
                self.update_results();
                self.set_status("Applications reloaded".to_string());
            }
            InfoAction::History => {
                self.view = if self.view == View::History {
                    View::Apps
                } else {
                    View::History
                };
                self.selected = 0;
                self.update_results();
            }
            InfoAction::AllApps => {
                self.view = if self.view == View::AllApps {
                    View::Apps
                } else {
                    View::AllApps
                };
                self.selected = 0;
                self.update_results();
            }
        }
    }

    fn show_search_bar(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette;
        let settings = self.settings.clone();
        let revealed =
            !settings.hide_search_bar_on_start || self.search_revealed || !self.query.is_empty();

        if !revealed {
            // KISS' "hide search bar on start": a slim bar that reveals the
            // query field on the first tap.
            let mut reveal = false;
            egui::Frame::NONE
                .fill(palette.surface)
                .inner_margin(Margin::symmetric(10, 8))
                .stroke(Stroke::new(1.0f32, palette.border))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let _ = widgets::icon(ui, Icon::Search, 15.0, palette.text_dim, &palette);
                        ui.add_space(6.0);
                        let hint = if settings.hide_search_bar_hint {
                            ""
                        } else {
                            "Tap to search"
                        };
                        if ui
                            .add(
                                egui::Label::new(
                                    RichText::new(hint).size(13.0).color(palette.text_dim),
                                )
                                .truncate(),
                            )
                            .clicked()
                        {
                            reveal = true;
                        }
                    });
                });
            if reveal {
                self.search_revealed = true;
            }
            return;
        }
        let bar_color = if settings.transparent_search_bar {
            Color32::TRANSPARENT
        } else {
            palette.surface
        };
        let radius: u8 = if settings.rounded_bars { 12 } else { 0 };
        let font_size = 15.0 * settings.font_scale.clamp(0.6, 2.0);
        let mut field: Option<egui::Response> = None;
        let mut cleared = false;

        let frame = egui::Frame::NONE
            .fill(bar_color)
            .corner_radius(CornerRadius::same(radius))
            .inner_margin(Margin::symmetric(10, 6))
            .stroke(Stroke::new(1.0f32, palette.border));

        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                let _ = widgets::icon(ui, Icon::Search, 16.0, palette.text_dim, &palette);
                ui.add_space(6.0);
                let hint = if settings.hide_search_bar_hint {
                    ""
                } else {
                    "Search apps, settings, the web…"
                };
                let width = (ui.available_width() - 40.0).max(40.0);
                let response = ui.add_sized(
                    [width, 20.0],
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text(hint)
                        .font(egui::FontId::proportional(font_size))
                        .text_color(palette.text)
                        .frame(false),
                );
                if response.changed() {
                    self.selected = 0;
                    self.view = View::Apps;
                    self.update_results();
                }
                field = Some(response);
                let has_query = !self.query.is_empty();
                if has_query
                    && widgets::icon_button(
                        ui,
                        Icon::Close,
                        24.0,
                        palette.text_dim,
                        &palette,
                        "Clear",
                    )
                    .clicked()
                {
                    cleared = true;
                }
            });
        });

        if cleared {
            self.clear_query();
        }

        // Keep the caret in the query field unless something else needs it.
        let focus_free = self.menu.is_none()
            && self.tag_editor.is_none()
            && self.rename_editor.is_none()
            && self.dialog.is_none()
            && !self.settings_open;
        if focus_free {
            if let Some(response) = field {
                if !response.has_focus() {
                    response.request_focus();
                }
            }
        }

        if settings.show_keyboard_hints {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                for (key, label) in [
                    ("\u{2191}\u{2193}", "select"),
                    ("\u{21b5}", "launch"),
                    ("Tab", "cycle"),
                    ("Esc", "back"),
                ] {
                    widgets::key_cap(ui, key, &palette);
                    let _ = ui.label(RichText::new(label).size(10.0).color(palette.text_dim));
                    ui.add_space(6.0);
                }
            });
        }
    }

    fn show_settings_panel(&mut self, ui: &mut egui::Ui) {
        let mut settings = self.settings.clone();
        let mut ui_state = std::mem::take(&mut self.settings_ui);
        let actions = settings_view::show(
            ui,
            &mut ui_state,
            &mut settings,
            &self.history,
            &self.apps,
            &self.palette,
        );
        self.settings_ui = ui_state;

        let changed = settings != self.settings;
        self.settings = settings;
        if changed {
            self.settings.save();
            self.apply_history();
            self.update_results();
        }

        for action in actions {
            match action {
                settings_view::SettingsAction::Close => {
                    self.settings_open = false;
                    self.settings_ui.highlight = None;
                }
                settings_view::SettingsAction::Confirm(dialog) => self.dialog = Some(dialog),
                settings_view::SettingsAction::AddProvider(name, url) => {
                    let mut next = self.settings.clone();
                    next.custom_web_providers
                        .push(crate::settings::SearchProvider { name, url });
                    next.save();
                    self.settings = next;
                    self.update_results();
                }
                settings_view::SettingsAction::RemoveProvider(index) => {
                    let mut next = self.settings.clone();
                    if index < next.custom_web_providers.len() {
                        next.custom_web_providers.remove(index);
                    }
                    next.save();
                    self.settings = next;
                    self.update_results();
                }
                settings_view::SettingsAction::RestoreExcluded(exec) => {
                    self.history.remove_excluded(&exec);
                    self.history.save();
                    self.apply_history();
                    self.update_results();
                    self.set_status("Application restored".to_string());
                }
                settings_view::SettingsAction::RefreshApps => {
                    self.reload_apps();
                    self.update_results();
                    self.set_status("Applications reloaded".to_string());
                }
            }
        }
    }

    // --- overlays --------------------------------------------------------

    /// Content of the context menu, KISS style: everything about one result.
    fn build_menu(&self, target: &MenuTarget) -> Vec<(Option<Icon>, String, MenuCommand, bool)> {
        let mut items: Vec<(Option<Icon>, String, MenuCommand, bool)> = Vec::new();
        match target {
            MenuTarget::Launcher => {
                items.push((
                    Some(Icon::Refresh),
                    "Reload applications".into(),
                    MenuCommand::Refresh,
                    true,
                ));
                items.push((
                    Some(Icon::Apps),
                    "All applications".into(),
                    MenuCommand::AllApps,
                    true,
                ));
                items.push((
                    Some(Icon::History),
                    "History".into(),
                    MenuCommand::History,
                    true,
                ));
                items.push((
                    Some(Icon::Filter),
                    "Excluded applications".into(),
                    MenuCommand::Excluded,
                    true,
                ));
                items.push((
                    Some(Icon::Star),
                    "Favorites bar".into(),
                    MenuCommand::ToggleFavoritesBar,
                    true,
                ));
                items.push((
                    Some(Icon::Close),
                    "Clear query".into(),
                    MenuCommand::ClearQuery,
                    true,
                ));
                items.push((
                    Some(Icon::Settings),
                    "Settings".into(),
                    MenuCommand::Settings,
                    true,
                ));
            }
            MenuTarget::Result(index) => {
                if let Some(result) = self.results.get(*index) {
                    let exec = result.entry.exec.clone();
                    let name = self.display_name(&result.entry);
                    let pinned =
                        (1..=9u8).find(|key| self.history.shortcut_for(*key) == Some(&exec));
                    let favorite_icon = if result.entry.is_favorite {
                        Icon::StarFilled
                    } else {
                        Icon::Star
                    };
                    let favorite_label = if result.entry.is_favorite {
                        "Remove from favorites".to_string()
                    } else {
                        "Add to favorites".to_string()
                    };
                    items.push((
                        Some(Icon::Info),
                        format!("Open {}", name),
                        MenuCommand::Launch(*index),
                        true,
                    ));
                    items.push((
                        Some(favorite_icon),
                        favorite_label,
                        MenuCommand::ToggleFavorite(exec.clone()),
                        true,
                    ));
                    items.push((
                        Some(Icon::Tag),
                        "Edit tags".into(),
                        MenuCommand::EditTags(exec.clone()),
                        true,
                    ));
                    items.push((
                        Some(Icon::Pencil),
                        "Rename".into(),
                        MenuCommand::Rename(exec.clone()),
                        true,
                    ));
                    items.push((
                        Some(Icon::Hash),
                        "Clear usage data".into(),
                        MenuCommand::ResetRank(exec.clone()),
                        true,
                    ));
                    let history_label = if self.history.is_excluded_from_history(&exec) {
                        "Track usage again"
                    } else {
                        "Do not track usage"
                    };
                    items.push((
                        Some(Icon::History),
                        history_label.into(),
                        MenuCommand::ExcludeFromHistory(exec.clone()),
                        true,
                    ));
                    if self.history.is_excluded(&exec) {
                        items.push((
                            Some(Icon::Refresh),
                            "Restore application".into(),
                            MenuCommand::Restore(exec.clone()),
                            true,
                        ));
                    } else {
                        items.push((
                            Some(Icon::Trash),
                            "Exclude application".into(),
                            MenuCommand::Exclude(exec.clone()),
                            true,
                        ));
                    }
                    items.push((
                        Some(Icon::Copy),
                        "Copy name".into(),
                        MenuCommand::CopyText(name),
                        true,
                    ));
                    items.push((
                        Some(Icon::Terminal),
                        "Copy exec".into(),
                        MenuCommand::CopyText(exec.clone()),
                        true,
                    ));
                    if !result.entry.desktop_file.as_os_str().is_empty() {
                        items.push((
                            Some(Icon::Folder),
                            "Open application file".into(),
                            MenuCommand::Details(exec.clone()),
                            true,
                        ));
                    }
                    if let Some(key) = pinned {
                        items.push((
                            Some(Icon::Pin),
                            format!("Remove shortcut {}", key),
                            MenuCommand::ClearShortcut(exec.clone()),
                            true,
                        ));
                    }
                    items.push((
                        Some(Icon::Pin),
                        "Pin to a number".into(),
                        MenuCommand::PickShortcut(exec.clone()),
                        true,
                    ));
                }
            }
            MenuTarget::Favorite(exec) => {
                let name = self
                    .apps
                    .iter()
                    .find(|app| &app.exec == exec)
                    .map(|app| self.display_name(app))
                    .unwrap_or_else(|| exec.clone());
                items.push((
                    Some(Icon::StarFilled),
                    format!("Unpin {}", name),
                    MenuCommand::ToggleFavorite(exec.clone()),
                    true,
                ));
                items.push((
                    Some(Icon::Tag),
                    "Edit tags".into(),
                    MenuCommand::EditTags(exec.clone()),
                    true,
                ));
                items.push((
                    Some(Icon::Hash),
                    "Clear usage data".into(),
                    MenuCommand::ResetRank(exec.clone()),
                    true,
                ));
                items.push((
                    Some(Icon::Trash),
                    "Exclude application".into(),
                    MenuCommand::Exclude(exec.clone()),
                    true,
                ));
            }
        }
        items
    }

    fn show_context_menu(&mut self, ctx: &egui::Context) {
        if let Some(exec) = self.shortcut_pad.clone() {
            self.show_shortcut_pad(ctx, &exec);
            return;
        }
        let target = match &self.menu {
            Some(state) => state.target.clone(),
            None => return,
        };
        let palette = self.palette;
        let items = self.build_menu(&target);
        let mut command: Option<MenuCommand> = None;

        let width = 236.0;
        let row_height = 28.0;
        let height = items.len() as f32 * row_height + 12.0;
        let pointer = ctx.input(|input| input.pointer.latest_pos().unwrap_or(egui::Pos2::ZERO));
        let screen = ctx.screen_rect();
        let mut position = pointer + Vec2::new(6.0, 6.0);
        if position.y + height > screen.bottom() - 8.0 {
            position.y = (screen.bottom() - height - 8.0).max(screen.top() + 8.0);
        }
        if position.x + width > screen.right() - 8.0 {
            position.x = (screen.right() - width - 8.0).max(screen.left() + 8.0);
        }

        let response = egui::Area::new(egui::Id::new("riss_context_menu"))
            .fixed_pos(position)
            .order(egui::Order::Middle)
            .sense(Sense::click())
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(palette.surface)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::same(6))
                    .stroke(Stroke::new(1.0f32, palette.border))
                    .show(ui, |ui| {
                        ui.set_width(width - 12.0);
                        for (icon, label, item, enabled) in &items {
                            ui.horizontal(|ui| {
                                if let Some(icon) = icon {
                                    let _ =
                                        widgets::icon(ui, *icon, 14.0, palette.text_dim, &palette);
                                    ui.add_space(4.0);
                                }
                                let color = if *enabled {
                                    palette.text
                                } else {
                                    palette.text_dim
                                };
                                let button =
                                    egui::Button::new(RichText::new(label).size(12.0).color(color))
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE)
                                        .min_size(Vec2::new(width - 40.0, row_height));
                                if ui.add_enabled(*enabled, button).clicked() {
                                    command = Some(item.clone());
                                }
                            });
                        }
                    });
            });

        let rect = response.response.rect;
        let clicked_outside =
            ctx.input(|input| input.pointer.any_click()) && !rect.contains(pointer);
        let guard_passed = self
            .menu
            .as_ref()
            .map(|state| Instant::now() > state.guard_until)
            .unwrap_or(true);
        if clicked_outside && guard_passed {
            self.menu = None;
        }
        if let Some(command) = command {
            self.menu = None;
            self.apply_menu_command(command);
        }
    }

    /// Number shortcuts are picked in a second step, like KISS does.
    fn show_shortcut_pad(&mut self, ctx: &egui::Context, exec: &str) {
        let palette = self.palette;
        let target = exec.to_string();
        let keys: Vec<(u8, bool)> = (1..=9u8)
            .map(|key| {
                let owner = self.history.shortcut_for(key).cloned();
                let mine = owner.as_deref() == Some(target.as_str());
                (key, mine)
            })
            .collect();
        let mut command: Option<MenuCommand> = None;

        let pointer = ctx.input(|input| input.pointer.latest_pos().unwrap_or(egui::Pos2::ZERO));
        let screen = ctx.screen_rect();
        let width = 9.0 * 30.0 + 16.0;
        let mut position = pointer + Vec2::new(-40.0, 24.0);
        if position.x + width > screen.right() - 8.0 {
            position.x = (screen.right() - width - 8.0).max(screen.left() + 8.0);
        }

        let response = egui::Area::new(egui::Id::new("riss_shortcut_pad"))
            .fixed_pos(position)
            .order(egui::Order::Middle)
            .sense(Sense::click())
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(palette.surface)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::same(8))
                    .stroke(Stroke::new(1.0f32, palette.border))
                    .show(ui, |ui| {
                        let _ = ui.label(
                            RichText::new("Pin to a number")
                                .size(11.0)
                                .color(palette.text_dim),
                        );
                        ui.horizontal(|ui| {
                            for (key, mine) in &keys {
                                let color = if *mine { palette.bg } else { palette.text };
                                let button = egui::Button::new(
                                    RichText::new(key.to_string()).size(12.0).color(color),
                                )
                                .fill(if *mine {
                                    palette.accent
                                } else {
                                    palette.surface_alt
                                })
                                .stroke(Stroke::NONE)
                                .min_size(Vec2::new(26.0, 26.0));
                                if ui.add(button).clicked() {
                                    command = Some(MenuCommand::SetShortcut(target.clone(), *key));
                                }
                            }
                        });
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Remove shortcut")
                                        .size(11.0)
                                        .color(palette.text_dim),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                                .min_size(Vec2::new(width - 24.0, 22.0)),
                            )
                            .clicked()
                        {
                            command = Some(MenuCommand::ClearShortcut(target.clone()));
                        }
                    });
            });

        let rect = response.response.rect;
        let clicked_outside =
            ctx.input(|input| input.pointer.any_click()) && !rect.contains(pointer);
        if command.is_some() || clicked_outside {
            self.shortcut_pad = None;
        }
        if let Some(command) = command {
            self.apply_menu_command(command);
        }
    }

    fn show_tag_editor(&mut self, ctx: &egui::Context) {
        let palette = self.palette;
        let editor = match &self.tag_editor {
            Some(editor) => editor,
            None => return,
        };
        let exec = editor.exec.clone();
        let mut input = editor.input.clone();
        let mut save = false;
        let mut cancel = false;

        let pointer =
            ctx.input(|input_state| input_state.pointer.latest_pos().unwrap_or(egui::Pos2::ZERO));
        egui::Area::new(egui::Id::new("riss_tag_editor"))
            .fixed_pos(pointer + Vec2::new(-70.0, -80.0))
            .order(egui::Order::Foreground)
            .sense(Sense::click())
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(palette.surface)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::same(8))
                    .stroke(Stroke::new(1.0f32, palette.accent))
                    .show(ui, |ui| {
                        let _ = ui.label(
                            RichText::new("Tags (comma separated)")
                                .size(11.0)
                                .color(palette.text_dim),
                        );
                        let _ = ui.add(egui::TextEdit::singleline(&mut input).desired_width(210.0));
                        ui.horizontal(|ui| {
                            if ui.button("Save").clicked() {
                                save = true;
                            }
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });
                    });
            });

        if save {
            self.save_tags(&exec, &input);
            self.tag_editor = None;
        } else if cancel {
            self.tag_editor = None;
            self.set_status("Tag editing cancelled".to_string());
        } else {
            self.tag_editor = Some(TagEditor { exec, input });
        }
    }

    fn show_rename_editor(&mut self, ctx: &egui::Context) {
        let palette = self.palette;
        let editor = match &self.rename_editor {
            Some(editor) => editor,
            None => return,
        };
        let exec = editor.exec.clone();
        let mut input = editor.input.clone();
        let mut save = false;
        let mut cancel = false;

        let pointer =
            ctx.input(|input_state| input_state.pointer.latest_pos().unwrap_or(egui::Pos2::ZERO));
        egui::Area::new(egui::Id::new("riss_rename_editor"))
            .fixed_pos(pointer + Vec2::new(-70.0, -80.0))
            .order(egui::Order::Foreground)
            .sense(Sense::click())
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(palette.surface)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::same(8))
                    .stroke(Stroke::new(1.0f32, palette.accent))
                    .show(ui, |ui| {
                        let _ = ui.label(
                            RichText::new("Rename application")
                                .size(11.0)
                                .color(palette.text_dim),
                        );
                        let _ = ui.add(egui::TextEdit::singleline(&mut input).desired_width(210.0));
                        ui.horizontal(|ui| {
                            if ui.button("Save").clicked() {
                                save = true;
                            }
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });
                    });
            });

        if save {
            self.history.set_alias(&exec, input);
            self.history.save();
            self.apply_history();
            self.update_results();
            self.rename_editor = None;
            self.set_status("Application renamed".to_string());
        } else if cancel {
            self.rename_editor = None;
            self.set_status("Renaming cancelled".to_string());
        } else {
            self.rename_editor = Some(RenameEditor { exec, input });
        }
    }

    fn show_dialog(&mut self, ctx: &egui::Context) {
        let dialog = match self.dialog.clone() {
            Some(dialog) => dialog,
            None => return,
        };
        let palette = self.palette;
        let screen = ctx.screen_rect();

        let mut confirmed = false;
        let mut cancelled = false;

        // Dim the whole launcher.
        egui::Area::new(egui::Id::new("riss_dialog_backdrop"))
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .sense(Sense::click())
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(rect, 0.0, palette.overlay());
            });

        let width = 300.0;
        let height = 150.0;
        let position = screen.center() - Vec2::new(width * 0.5, height * 0.5);
        egui::Area::new(egui::Id::new("riss_dialog"))
            .fixed_pos(position)
            .order(egui::Order::Foreground)
            .sense(Sense::click())
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(palette.surface)
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::same(14))
                    .stroke(Stroke::new(1.0f32, palette.border))
                    .show(ui, |ui| {
                        ui.set_width(width - 28.0);
                        let _ = ui.label(
                            RichText::new(&dialog.title)
                                .size(14.0)
                                .strong()
                                .color(palette.text),
                        );
                        ui.add_space(6.0);
                        let _ = ui.add(
                            egui::Label::new(
                                RichText::new(&dialog.message)
                                    .size(12.0)
                                    .color(palette.text_dim),
                            )
                            .wrap(),
                        );
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Button::new(RichText::new(&dialog.confirm).size(12.0))
                                        .fill(palette.accent)
                                        .corner_radius(CornerRadius::same(6))
                                        .min_size(Vec2::new(90.0, 28.0)),
                                )
                                .clicked()
                            {
                                confirmed = true;
                            }
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(&dialog.cancel)
                                            .size(12.0)
                                            .color(palette.text),
                                    )
                                    .fill(palette.surface_alt)
                                    .corner_radius(CornerRadius::same(6))
                                    .min_size(Vec2::new(90.0, 28.0)),
                                )
                                .clicked()
                            {
                                cancelled = true;
                            }
                        });
                    });
            });

        self.dialog = None;
        if confirmed {
            self.apply_dialog(dialog.action);
        } else if cancelled {
            self.apply_dialog(DialogAction::Cancel);
        } else {
            self.dialog = Some(dialog);
        }
    }

    fn show_timer(&mut self, ctx: &egui::Context) {
        let (seconds, started) = match self.timer {
            Some(timer) => timer,
            None => return,
        };
        let elapsed = started.elapsed().as_secs();
        let remaining = seconds.saturating_sub(elapsed);
        if remaining == 0 {
            self.timer = None;
            self.set_status("Timer finished".to_string());
            // Audible feedback, like the KISS timer notification.
            #[cfg(not(target_os = "android"))]
            println!("\u{7}");
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(200));

        let palette = self.palette;
        let screen = ctx.screen_rect();
        let width = 190.0;
        let position = egui::Pos2::new(screen.center().x - width * 0.5, screen.top() + 16.0);
        let mut cancel = false;
        egui::Area::new(egui::Id::new("riss_timer"))
            .fixed_pos(position)
            .order(egui::Order::Foreground)
            .sense(Sense::click())
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(palette.surface)
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::symmetric(12, 8))
                    .stroke(Stroke::new(1.0f32, palette.accent))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let _ = widgets::icon(ui, Icon::Clock, 18.0, palette.accent, &palette);
                            let _ = ui.label(
                                RichText::new(crate::providers::format_duration(remaining))
                                    .size(16.0)
                                    .strong()
                                    .color(palette.text),
                            );
                            if widgets::icon_button(
                                ui,
                                Icon::Close,
                                22.0,
                                palette.text_dim,
                                &palette,
                                "Cancel",
                            )
                            .clicked()
                            {
                                cancel = true;
                            }
                        });
                    });
            });
        if cancel {
            self.timer = None;
            self.set_status("Timer cancelled".to_string());
        }
    }
}

/// Buttons of the top information bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoAction {
    Menu,
    Settings,
    Refresh,
    History,
    AllApps,
}

fn show_info_bar(
    ui: &mut egui::Ui,
    _settings: &Settings,
    palette: &Palette,
    view: View,
    app_count: usize,
    result_count: usize,
) -> Option<InfoAction> {
    let mut action = None;
    ui.horizontal_centered(|ui| {
        ui.add_space(6.0);
        let _ = widgets::icon(ui, Icon::Power, 16.0, palette.accent, palette);
        ui.add_space(6.0);
        let _ = ui.label(
            RichText::new("RISS")
                .size(13.0)
                .strong()
                .color(palette.accent),
        );
        let _ = ui.label(
            RichText::new(format!("{} \u{b7} {} shown", app_count, result_count))
                .size(10.0)
                .color(palette.text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_button(ui, Icon::Kebab, 26.0, palette.text_dim, palette, "Menu")
                .clicked()
            {
                action = Some(InfoAction::Menu);
            }
            if widgets::icon_button(
                ui,
                Icon::Settings,
                26.0,
                palette.text_dim,
                palette,
                "Settings",
            )
            .clicked()
            {
                action = Some(InfoAction::Settings);
            }
            if widgets::icon_button(ui, Icon::Refresh, 26.0, palette.text_dim, palette, "Reload")
                .clicked()
            {
                action = Some(InfoAction::Refresh);
            }
            let history_active = view == View::History;
            let color = if history_active {
                palette.accent
            } else {
                palette.text_dim
            };
            if widgets::icon_button(ui, Icon::History, 26.0, color, palette, "History").clicked() {
                action = Some(InfoAction::History);
            }
            let all_active = view == View::AllApps;
            let color = if all_active {
                palette.accent
            } else {
                palette.text_dim
            };
            if widgets::icon_button(ui, Icon::Apps, 26.0, color, palette, "All apps").clicked() {
                action = Some(InfoAction::AllApps);
            }
        });
    });
    action
}

fn collect_tags(apps: &[AppEntry]) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for app in apps {
        for tag in &app.tags {
            if !tag.is_empty() && !tags.contains(tag) {
                tags.push(tag.clone());
            }
        }
    }
    tags.sort();
    tags
}

fn show_empty_hint(ui: &mut egui::Ui, palette: &Palette, view: View, query: &str) {
    let message = match view {
        View::Excluded => {
            "Nothing is excluded.\nLong press a result and pick \u{201c}Exclude application\u{201d}.".to_string()
        }
        _ => {
            if query.is_empty() {
                "No application yet — use the reload button".to_string()
            } else {
                format!("No result for \u{201c}{}\u{201d}", query)
            }
        }
    };
    ui.centered_and_justified(|ui| {
        let _ = ui.label(
            RichText::new(message)
                .size(12.0)
                .color(palette.text_dim)
                .weak(),
        );
    });
}

fn history_score(app: &AppEntry, mode: HistoryMode) -> i64 {
    let count = app.launch_count as i64;
    let recency = app.last_launched as i64;
    match mode {
        HistoryMode::Recency => recency,
        HistoryMode::UsageCount => count * 1000,
        HistoryMode::Frecent => count * 1000 + recency % 100_000,
    }
}

fn provider_url_for(name: &str) -> String {
    crate::providers::builtin_web_providers()
        .into_iter()
        .find(|(provider, _)| provider.eq_ignore_ascii_case(name))
        .map(|(_, url)| url.to_string())
        .unwrap_or_else(|| "https://duckduckgo.com/?q={}".to_string())
}

fn export_backup() -> String {
    let path = crate::settings::config_dir().join("riss-backup.json");
    let payload = serde_json::json!({
        "settings": Settings::load(),
        "history": HistoryData::load(),
    });
    match serde_json::to_string_pretty(&payload) {
        Ok(text) => match std::fs::write(&path, text) {
            Ok(()) => path.display().to_string(),
            Err(error) => format!("Export failed: {}", error),
        },
        Err(error) => format!("Export failed: {}", error),
    }
}

fn import_backup() -> Result<String, String> {
    let path = crate::settings::config_dir().join("riss-backup.json");
    let text =
        std::fs::read_to_string(&path).map_err(|error| format!("Import failed: {}", error))?;
    let payload: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| format!("Import failed: {}", error))?;
    if let Some(value) = payload.get("settings") {
        let settings: Settings = serde_json::from_value(value.clone())
            .map_err(|error| format!("Import failed: {}", error))?;
        settings.save();
    }
    if let Some(value) = payload.get("history") {
        let history: HistoryData = serde_json::from_value(value.clone())
            .map_err(|error| format!("Import failed: {}", error))?;
        history.save();
    }
    Ok("Settings imported".to_string())
}

/// Load the system font so the launcher looks native.
pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    #[cfg(not(target_os = "android"))]
    let candidates = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    ];

    #[cfg(target_os = "android")]
    let candidates = [
        "/system/fonts/Roboto-Regular.ttf",
        "/system/fonts/NotoSans-Regular.ttf",
        "/system/fonts/DroidSans.ttf",
    ];

    for path in &candidates {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                "system_font".to_owned(),
                egui::FontData::from_owned(data).into(),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "system_font".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "system_font".to_owned());
            break;
        }
    }

    ctx.set_fonts(fonts);
}

impl eframe::App for RissApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_visuals(ctx);

        if self
            .status
            .as_ref()
            .map(|(_, at)| at.elapsed() > Duration::from_secs(4))
            .unwrap_or(false)
        {
            self.status = None;
        }

        self.handle_keys(ctx);
        self.update_press_state(ctx);
        self.update_favorites_press_state(ctx);
        self.apply_window_settings(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(self.palette.bg))
            .show(ctx, |ui| {
                if self.settings_open {
                    self.show_settings_panel(ui);
                } else {
                    self.show_launcher(ui, ctx);
                }
            });

        self.show_context_menu(ctx);
        self.show_tag_editor(ctx);
        self.show_rename_editor(ctx);
        self.show_dialog(ctx);
        self.show_timer(ctx);

        if let Some(visible) = self.visible_request.take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(visible));
        }
    }
}
