#[cfg(target_os = "android")]
use crate::android_app_entry::{self as app_entry, AppEntry};
#[cfg(not(target_os = "android"))]
use crate::app_entry::{self, AppEntry};
use crate::history::HistoryData;
use crate::search::{self, MatchType, SearchEngine, SearchResult};
use crate::settings::{
    AccentColor, GestureAction, HistorySort, HomeView, LauncherSettings, LongPressAction,
    ResultDensity, ThemeMode, WebSearchProvider,
};
use eframe::egui;
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Stroke, Vec2};
use std::time::{Duration, Instant};

/// Colors used by the launcher. Rebuilt from the saved theme/accent on each frame so changes in
/// the settings page are visible immediately.
#[derive(Clone, Copy)]
struct Colors {
    bg: Color32,
    bg_light: Color32,
    bg_hover: Color32,
    text: Color32,
    text_dim: Color32,
    accent: Color32,
    accent_dim: Color32,
    favorite: Color32,
    border: Color32,
    search_bg: Color32,
    calc_bg: Color32,
    calc_text: Color32,
}

impl Colors {
    fn from_settings(settings: &LauncherSettings, system_theme: Option<egui::Theme>) -> Self {
        let light = match settings.theme {
            ThemeMode::System => system_theme == Some(egui::Theme::Light),
            ThemeMode::Light => true,
            ThemeMode::Dark | ThemeMode::Amoled => false,
        };
        let amoled = settings.theme == ThemeMode::Amoled;
        let (accent, accent_dim) = match settings.accent {
            AccentColor::Blue => (
                Color32::from_rgb(137, 180, 250),
                Color32::from_rgb(89, 129, 198),
            ),
            AccentColor::Purple => (
                Color32::from_rgb(203, 166, 247),
                Color32::from_rgb(145, 105, 198),
            ),
            AccentColor::Teal => (
                Color32::from_rgb(148, 226, 213),
                Color32::from_rgb(82, 165, 151),
            ),
            AccentColor::Green => (
                Color32::from_rgb(166, 227, 161),
                Color32::from_rgb(99, 166, 95),
            ),
            AccentColor::Orange => (
                Color32::from_rgb(250, 179, 135),
                Color32::from_rgb(196, 119, 72),
            ),
            AccentColor::Rose => (
                Color32::from_rgb(245, 194, 231),
                Color32::from_rgb(188, 116, 169),
            ),
        };

        if light {
            Self {
                bg: Color32::from_rgb(247, 248, 252),
                bg_light: Color32::WHITE,
                bg_hover: Color32::from_rgb(231, 236, 246),
                text: Color32::from_rgb(38, 42, 52),
                text_dim: Color32::from_rgb(100, 107, 121),
                accent,
                accent_dim,
                favorite: Color32::from_rgb(185, 126, 13),
                border: Color32::from_rgb(215, 221, 232),
                search_bg: Color32::WHITE,
                calc_bg: Color32::from_rgb(229, 246, 230),
                calc_text: Color32::from_rgb(47, 117, 54),
            }
        } else if amoled {
            Self {
                bg: Color32::BLACK,
                bg_light: Color32::from_rgb(14, 14, 16),
                bg_hover: Color32::from_rgb(27, 29, 36),
                text: Color32::from_rgb(230, 232, 240),
                text_dim: Color32::from_rgb(145, 151, 169),
                accent,
                accent_dim,
                favorite: Color32::from_rgb(250, 204, 94),
                border: Color32::from_rgb(47, 49, 58),
                search_bg: Color32::from_rgb(12, 12, 14),
                calc_bg: Color32::from_rgb(20, 36, 23),
                calc_text: Color32::from_rgb(166, 227, 161),
            }
        } else {
            Self {
                bg: Color32::from_rgb(30, 30, 46),
                bg_light: Color32::from_rgb(40, 40, 58),
                bg_hover: Color32::from_rgb(55, 55, 80),
                text: Color32::from_rgb(205, 214, 244),
                text_dim: Color32::from_rgb(147, 153, 178),
                accent,
                accent_dim,
                favorite: Color32::from_rgb(250, 204, 94),
                border: Color32::from_rgb(69, 71, 90),
                search_bg: Color32::from_rgb(49, 50, 68),
                calc_bg: Color32::from_rgb(50, 70, 50),
                calc_text: Color32::from_rgb(166, 227, 161),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Launcher,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsCategory {
    Search,
    Appearance,
    Touch,
    History,
    HiddenApps,
    Advanced,
}

#[derive(Debug, Clone, Copy)]
enum ConfirmAction {
    ClearHistory,
    ClearFavorites,
    ResetSettings,
}

#[derive(Debug, Clone, Copy)]
enum ResultAction {
    Launch,
    ToggleFavorite,
    EditTags,
    HideApp,
}

/// Main application state.
pub struct RissApp {
    query: String,
    apps: Vec<AppEntry>,
    results: Vec<SearchResult>,
    search_engine: SearchEngine,
    history: HistoryData,
    settings: LauncherSettings,
    colors: Colors,
    selected_index: usize,
    gesture_delta: Vec2,
    search_focus_pending: bool,
    search_surrender_pending: bool,
    tag_focus_pending: bool,
    page: Page,
    settings_category: SettingsCategory,
    status_message: Option<(String, Instant)>,
    editing_tags: Option<String>,
    tag_input: String,
    confirm_action: Option<ConfirmAction>,
    import_dialog_open: bool,
    import_json: String,
    import_error: Option<String>,
}

impl RissApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let history = HistoryData::load();
        let settings = LauncherSettings::load();
        let apps = Self::discover_apps();
        let colors = Colors::from_settings(&settings, None);

        let mut app = Self {
            query: String::new(),
            apps,
            results: Vec::new(),
            search_engine: SearchEngine::new(),
            history,
            search_focus_pending: settings.focus_search_on_start,
            search_surrender_pending: false,
            tag_focus_pending: false,
            settings,
            colors,
            selected_index: 0,
            gesture_delta: Vec2::ZERO,
            page: Page::Launcher,
            settings_category: SettingsCategory::Search,
            status_message: None,
            editing_tags: None,
            tag_input: String::new(),
            confirm_action: None,
            import_dialog_open: false,
            import_json: String::new(),
            import_error: None,
        };
        app.sync_app_metadata();
        app.update_results();
        app
    }

    fn discover_apps() -> Vec<AppEntry> {
        let mut apps = app_entry::discover_apps();
        for builtin in app_entry::builtin_entries() {
            let already_listed = apps
                .iter()
                .any(|app| app.name.eq_ignore_ascii_case(&builtin.name));
            if !already_listed {
                apps.push(builtin);
            }
        }
        apps.sort_by_key(|app| app.name.to_lowercase());
        apps
    }

    fn sync_app_metadata(&mut self) {
        for app in &mut self.apps {
            app.launch_count = self.history.get_launch_count(&app.exec);
            app.last_launched = self.history.get_last_launched(&app.exec);
            app.is_favorite = self.history.is_favorite(&app.exec);
            // Keep built-in/platform tags unless the user has explicitly saved custom tags.
            if self.history.tags.contains_key(&app.exec) {
                app.tags = self.history.get_tags(&app.exec);
            }
        }
    }

    fn refresh_apps(&mut self) {
        self.apps = Self::discover_apps();
        self.sync_app_metadata();
        self.update_results();
        self.set_status(format!("Found {} apps", self.apps.len()));
    }

    fn update_results(&mut self) {
        let query = self.query.trim();
        let limit = self.settings.results_limit.clamp(1, 150);

        if query.is_empty() {
            let mut apps: Vec<AppEntry> = self
                .apps
                .iter()
                .filter(|app| !self.settings.hidden_apps.contains(&app.exec))
                .filter(|app| match self.settings.home_view {
                    HomeView::AllApps => {
                        !self.settings.hide_favorites_from_apps || !app.is_favorite
                    }
                    HomeView::History => is_history_entry_visible(
                        app.is_favorite,
                        app.launch_count,
                        self.settings.hide_favorites_from_history,
                    ),
                    HomeView::Favorites => app.is_favorite,
                })
                .cloned()
                .collect();

            apps.sort_by(|a, b| {
                let favorite_order = if self.settings.favorite_first {
                    b.is_favorite.cmp(&a.is_favorite)
                } else {
                    std::cmp::Ordering::Equal
                };
                let history_order = match self.settings.history_sort {
                    HistorySort::Recent => b.last_launched.cmp(&a.last_launched),
                    HistorySort::Frequent => b.launch_count.cmp(&a.launch_count),
                    HistorySort::Alphabetical => std::cmp::Ordering::Equal,
                };
                favorite_order
                    .then(history_order)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });

            if self.settings.home_view != HomeView::AllApps {
                apps.truncate(limit);
            }

            self.results = apps
                .into_iter()
                .map(|entry| SearchResult {
                    score: i64::from(entry.launch_count) + if entry.is_favorite { 1000 } else { 0 },
                    entry,
                    match_type: MatchType::Fuzzy,
                })
                .collect();
        } else if !self.settings.search_apps {
            self.results.clear();
        } else {
            let candidates: Vec<AppEntry> = self
                .apps
                .iter()
                .filter(|app| !self.settings.hidden_apps.contains(&app.exec))
                .filter(|app| !self.settings.hide_favorites_from_apps || !app.is_favorite)
                .cloned()
                .collect();
            self.results = self.search_engine.search_with_options(
                query,
                &candidates,
                limit,
                self.settings.search_descriptions,
                self.settings.search_tags,
                self.settings.search_categories,
            );
        }

        if self.results.is_empty() {
            self.selected_index = 0;
        } else {
            self.selected_index = self.selected_index.min(self.results.len() - 1);
        }
    }

    fn launch_app(&mut self, index: usize) {
        if let Some(entry) = self.results.get(index).map(|result| result.entry.clone()) {
            self.launch_entry(entry);
        }
    }

    fn launch_by_exec(&mut self, exec: &str) {
        if let Some(entry) = self.apps.iter().find(|app| app.exec == exec).cloned() {
            self.launch_entry(entry);
        }
    }

    fn search_web(&mut self, query: &str) {
        let url = web_search_url(self.settings.web_search_provider, query);
        #[cfg(target_os = "android")]
        let result = app_entry::android_jni::open_url_jni(&url);
        #[cfg(not(target_os = "android"))]
        let result = open::that(url.as_str()).map_err(|error| error.to_string());

        match result {
            Ok(()) => {
                self.search_surrender_pending = true;
                self.set_status(format!("Searching the web for {query}"));
            }
            Err(error) => self.set_status(format!("Could not open browser: {error}")),
        }
    }

    fn launch_entry(&mut self, entry: AppEntry) {
        let exec = entry.exec.clone();
        let name = entry.name.clone();
        match entry.launch() {
            Ok(()) => {
                if !self.settings.freeze_history {
                    self.history.record_launch(&exec);
                }
                self.sync_app_metadata();
                self.history.save();
                self.set_status(format!("Launched {name}"));
                if self.settings.clear_search_after_launch {
                    self.query.clear();
                    self.selected_index = 0;
                }
                if self.settings.hide_keyboard_after_launch {
                    self.search_surrender_pending = true;
                }
                self.update_results();
            }
            Err(error) => self.set_status(format!("Error: {error}")),
        }
    }

    fn toggle_favorite(&mut self, index: usize) {
        if let Some((exec, name)) = self
            .results
            .get(index)
            .map(|result| (result.entry.exec.clone(), result.entry.name.clone()))
        {
            self.toggle_favorite_for(&exec, &name);
        }
    }

    fn toggle_favorite_for(&mut self, exec: &str, name: &str) {
        let is_favorite = self.history.toggle_favorite(exec);
        for app in &mut self.apps {
            if app.exec == exec {
                app.is_favorite = is_favorite;
            }
        }
        self.history.save();
        self.update_results();
        self.set_status(if is_favorite {
            format!("Added {name} to favorites")
        } else {
            format!("Removed {name} from favorites")
        });
    }

    fn toggle_tag_editor(&mut self, exec: &str) {
        if self.editing_tags.as_deref() == Some(exec) {
            self.editing_tags = None;
            self.tag_focus_pending = false;
            return;
        }

        let tags = self
            .apps
            .iter()
            .find(|app| app.exec == exec)
            .map(|app| app.tags.clone())
            .unwrap_or_else(|| self.history.get_tags(exec));
        self.editing_tags = Some(exec.to_owned());
        self.tag_input = tags.join(", ");
        self.tag_focus_pending = true;
        self.search_surrender_pending = true;
    }

    fn save_tags(&mut self, exec: &str) {
        let mut tags: Vec<String> = self
            .tag_input
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        tags.sort_by_key(|tag| tag.to_lowercase());
        tags.dedup_by(|left, right| left.eq_ignore_ascii_case(right));

        self.history.set_tags(exec, tags.clone());
        for app in &mut self.apps {
            if app.exec == exec {
                app.tags = tags.clone();
            }
        }
        self.history.save();
        self.editing_tags = None;
        self.tag_focus_pending = false;
        self.search_focus_pending = self.settings.focus_search_on_start;
        self.update_results();
        self.set_status("Tags saved".to_owned());
    }

    fn hide_app(&mut self, exec: &str, name: &str) {
        if !self
            .settings
            .hidden_apps
            .iter()
            .any(|hidden| hidden == exec)
        {
            self.settings.hidden_apps.push(exec.to_owned());
        }
        self.settings.save();
        self.update_results();
        self.set_status(format!("{name} hidden — restore it in Settings"));
    }

    fn set_status(&mut self, message: String) {
        self.status_message = Some((message, Instant::now()));
    }

    fn request_search_focus(&mut self) {
        self.page = Page::Launcher;
        self.search_focus_pending = true;
        self.search_surrender_pending = false;
    }

    fn run_gesture(&mut self, action: GestureAction) {
        match action {
            GestureAction::None => {}
            GestureAction::FocusSearch => self.request_search_focus(),
            GestureAction::ClearSearch => {
                self.query.clear();
                self.selected_index = 0;
                self.update_results();
                self.request_search_focus();
            }
            GestureAction::ToggleSettings => {
                self.page = if self.page == Page::Settings {
                    Page::Launcher
                } else {
                    self.search_surrender_pending = true;
                    Page::Settings
                };
            }
            GestureAction::RefreshApps => self.refresh_apps(),
            GestureAction::CycleHomeView => {
                self.settings.home_view = match self.settings.home_view {
                    HomeView::AllApps => HomeView::History,
                    HomeView::History => HomeView::Favorites,
                    HomeView::Favorites => HomeView::AllApps,
                };
                self.settings.save();
                self.update_results();
                self.set_status(format!(
                    "Home view: {}",
                    home_view_name(self.settings.home_view)
                ));
            }
        }
    }

    fn perform_result_action(&mut self, exec: &str, name: &str, action: ResultAction) {
        match action {
            ResultAction::Launch => self.launch_by_exec(exec),
            ResultAction::ToggleFavorite => self.toggle_favorite_for(exec, name),
            ResultAction::EditTags => self.toggle_tag_editor(exec),
            ResultAction::HideApp => self.hide_app(exec, name),
        }
    }

    fn get_category_icon(category: &str) -> &str {
        match category.to_lowercase().as_str() {
            "game" | "games" => "🎮",
            "development" => "💻",
            "education" => "📚",
            "graphics" => "🎨",
            "audio" | "audiovideo" | "music" => "🎵",
            "video" => "🎬",
            "network" | "internet" => "🌐",
            "office" => "📄",
            "settings" | "system" => "⚙️",
            "utility" | "utilities" => "🔧",
            "accessories" => "📎",
            "science" => "🔬",
            "filemanager" => "📁",
            _ => "📦",
        }
    }

    fn get_app_icon(entry: &AppEntry) -> String {
        if let Some(category) = entry.categories.first() {
            Self::get_category_icon(category).to_owned()
        } else if entry.name.to_lowercase().contains("terminal") {
            "⌨️".to_owned()
        } else if entry.name.to_lowercase().contains("browser")
            || entry.name.to_lowercase().contains("firefox")
            || entry.name.to_lowercase().contains("chrome")
        {
            "🌐".to_owned()
        } else if entry.name.to_lowercase().contains("file") {
            "📁".to_owned()
        } else if entry.name.to_lowercase().contains("text")
            || entry.name.to_lowercase().contains("editor")
        {
            "📝".to_owned()
        } else if entry.name.to_lowercase().contains("calc") {
            "🧮".to_owned()
        } else if entry.name.to_lowercase().contains("mail")
            || entry.name.to_lowercase().contains("thunder")
        {
            "✉️".to_owned()
        } else if entry.name.to_lowercase().contains("music")
            || entry.name.to_lowercase().contains("spotify")
        {
            "🎵".to_owned()
        } else {
            entry
                .name
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string()
        }
    }
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    #[cfg(not(target_os = "android"))]
    {
        let font_paths = [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        ];

        for path in &font_paths {
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
    }

    #[cfg(target_os = "android")]
    {
        let android_font_paths = [
            "/system/fonts/Roboto-Regular.ttf",
            "/system/fonts/NotoSans-Regular.ttf",
            "/system/fonts/DroidSans.ttf",
        ];

        for path in &android_font_paths {
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
    }

    ctx.set_fonts(fonts);
}

impl eframe::App for RissApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let system_theme = ctx.input(|input| input.raw.system_theme);
        self.colors = Colors::from_settings(&self.settings, system_theme);

        let use_light_visuals = match self.settings.theme {
            ThemeMode::System => system_theme == Some(egui::Theme::Light),
            ThemeMode::Light => true,
            ThemeMode::Dark | ThemeMode::Amoled => false,
        };
        let mut visuals = if use_light_visuals {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        visuals.override_text_color = Some(self.colors.text);
        visuals.window_fill = self.colors.bg_light;
        visuals.panel_fill = self.colors.bg;
        visuals.widgets.noninteractive.bg_fill = self.colors.bg_light;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, self.colors.text);
        visuals.widgets.inactive.bg_fill = self.colors.search_bg;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, self.colors.text);
        visuals.widgets.hovered.bg_fill = self.colors.bg_hover;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, self.colors.text);
        visuals.widgets.active.bg_fill = self.colors.accent_dim;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, self.colors.text);
        visuals.selection.bg_fill = self.colors.accent_dim;
        ctx.set_visuals(visuals);

        self.handle_keyboard(ctx);

        if self
            .status_message
            .as_ref()
            .is_some_and(|(_, time)| time.elapsed() > Duration::from_secs(3))
        {
            self.status_message = None;
        }

        let colors = self.colors;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(colors.bg))
            .show(ctx, |ui| match self.page {
                Page::Launcher => self.show_launcher_page(ui, ctx),
                Page::Settings => self.show_settings_page(ui, ctx),
            });

        self.show_confirmation_dialog(ctx);
        self.show_import_dialog(ctx);

        if self.status_message.is_some() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }
}

impl RissApp {
    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        let escape_pressed =
            ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if escape_pressed {
            if self.import_dialog_open {
                self.import_dialog_open = false;
            } else if self.confirm_action.is_some() {
                self.confirm_action = None;
            } else if self.page == Page::Settings {
                self.page = Page::Launcher;
                if self.settings.focus_search_on_start {
                    self.search_focus_pending = true;
                }
            } else if self.editing_tags.is_some() {
                self.editing_tags = None;
                self.tag_focus_pending = false;
                if self.settings.focus_search_on_start {
                    self.search_focus_pending = true;
                }
            } else if !self.query.is_empty() {
                self.query.clear();
                self.selected_index = 0;
                self.update_results();
                self.search_focus_pending = true;
            } else {
                self.search_surrender_pending = true;
            }
            return;
        }

        if self.page == Page::Settings || self.import_dialog_open || self.confirm_action.is_some() {
            return;
        }
        if self.editing_tags.is_some() {
            return;
        }

        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
            if self.settings.always_default_web_search_on_enter
                && self.settings.web_search_enabled
                && !self.query.trim().is_empty()
            {
                let query = self.query.trim().to_owned();
                self.search_web(&query);
            } else if !self.results.is_empty() {
                self.launch_app(self.selected_index);
            }
        }

        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp))
            && self.selected_index > 0
        {
            self.selected_index -= 1;
        }

        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown))
            && !self.results.is_empty()
        {
            self.selected_index = (self.selected_index + 1).min(self.results.len() - 1);
        }

        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Tab))
            && !self.results.is_empty()
        {
            self.selected_index = (self.selected_index + 1) % self.results.len();
        }
    }

    fn show_launcher_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.show_home_header(ui, ctx);

        let favorites = self.favorite_apps();
        let favorite_bar_height = if self.settings.show_favorites_bar && !favorites.is_empty() {
            if self.settings.large_favorites_bar {
                78.0
            } else {
                62.0
            }
        } else {
            0.0
        };
        let search_height = (if self.settings.show_search_hints {
            90.0
        } else {
            64.0
        }) + if self.settings.large_search_bar {
            8.0
        } else {
            0.0
        };
        let status_height = if self.status_message.is_some() {
            16.0
        } else {
            0.0
        };
        let results_height =
            (ui.available_height() - favorite_bar_height - search_height - status_height - 4.0)
                .max(0.0);

        egui::ScrollArea::vertical()
            .id_salt("launcher-results")
            .max_height(results_height)
            .auto_shrink([false, false])
            .show(ui, |ui| self.show_results(ui, ctx));

        if favorite_bar_height > 0.0 {
            self.show_favorites_bar(ui, &favorites, favorite_bar_height);
        }

        ui.add_space(4.0);
        self.show_search_bar(ui, ctx);

        if let Some((message, _)) = &self.status_message {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(message)
                        .color(self.colors.text_dim)
                        .font(FontId::proportional(11.0)),
                );
            });
        }
    }

    fn show_home_header(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let width = ui.available_width();
        ui.allocate_ui(Vec2::new(width, 54.0), |ui| {
            ui.horizontal_centered(|ui| {
                let brand_width = (ui.available_width() - 120.0).max(72.0);
                let brand = ui
                    .add_sized(
                        Vec2::new(brand_width, 48.0),
                        egui::Button::new(
                            RichText::new("⚡ RISS  ·  Launcher")
                                .color(self.colors.accent)
                                .font(FontId::proportional(17.0))
                                .strong(),
                        )
                        .sense(egui::Sense::click_and_drag())
                        .fill(Color32::TRANSPARENT)
                        .corner_radius(CornerRadius::same(12)),
                    )
                    .on_hover_text("Tap to focus search · Swipe here for your gesture shortcuts");

                if brand.double_clicked() {
                    self.run_gesture(self.settings.double_tap_action);
                } else {
                    if brand.drag_started() {
                        self.gesture_delta = Vec2::ZERO;
                    }
                    if brand.dragged() || brand.drag_stopped() {
                        self.gesture_delta += ctx.input(|input| input.pointer.delta());
                    }
                    if brand.drag_stopped() {
                        let delta = std::mem::replace(&mut self.gesture_delta, Vec2::ZERO);
                        if delta.length() >= 32.0 {
                            let action = if delta.x.abs() > delta.y.abs() {
                                if delta.x > 0.0 {
                                    self.settings.swipe_right
                                } else {
                                    self.settings.swipe_left
                                }
                            } else if delta.y > 0.0 {
                                self.settings.swipe_down
                            } else {
                                self.settings.swipe_up
                            };
                            self.run_gesture(action);
                        }
                    } else if brand.clicked() {
                        self.request_search_focus();
                    }
                }

                if ui
                    .add_sized(
                        Vec2::splat(48.0),
                        egui::Button::new(
                            RichText::new("↻")
                                .color(self.colors.text_dim)
                                .font(FontId::proportional(24.0)),
                        )
                        .fill(Color32::TRANSPARENT)
                        .corner_radius(CornerRadius::same(12)),
                    )
                    .on_hover_text("Refresh app list")
                    .clicked()
                {
                    self.refresh_apps();
                }

                if ui
                    .add_sized(
                        Vec2::splat(48.0),
                        egui::Button::new(
                            RichText::new("⚙")
                                .color(self.colors.text_dim)
                                .font(FontId::proportional(22.0)),
                        )
                        .fill(Color32::TRANSPARENT)
                        .corner_radius(CornerRadius::same(12)),
                    )
                    .on_hover_text("Settings")
                    .clicked()
                {
                    self.search_surrender_pending = true;
                    self.page = Page::Settings;
                }
            });
        });
    }

    fn show_results(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let has_query = !self.query.trim().is_empty();
        if has_query && self.settings.calculator_enabled {
            if let Some(calc_result) = search::try_calculate(&self.query) {
                let value = calc_result.trim_start_matches("= ").to_owned();
                let response = ui.add_sized(
                    Vec2::new(ui.available_width(), 58.0),
                    egui::Button::new(
                        RichText::new(format!("🧮   {}   {calc_result}", self.query))
                            .color(self.colors.calc_text)
                            .font(FontId::proportional(16.0)),
                    )
                    .fill(self.colors.calc_bg)
                    .corner_radius(self.card_radius()),
                );
                if response.clicked() {
                    ctx.copy_text(value);
                    self.set_status("Calculation copied".to_owned());
                }
                response.on_hover_text("Tap to copy the result");
                ui.add_space(8.0);
            }
        }

        if self.results.is_empty() {
            if has_query && self.settings.web_search_enabled {
                self.show_web_search_result(ui);
            } else {
                ui.add_space(32.0);
                ui.vertical_centered(|ui| {
                    let message = if !has_query {
                        match self.settings.home_view {
                            HomeView::AllApps => {
                                "No visible apps yet. Search, refresh the app list, or restore hidden apps in Settings."
                            }
                            HomeView::History => {
                                "Recently used apps will appear here. Tap search to find an app or change the Home screen in Settings."
                            }
                            HomeView::Favorites => {
                                "No favorites yet. Search for an app and tap its star to pin it here."
                            }
                        }
                    } else {
                        "No matching apps. Try a shorter name or check your search settings."
                    };
                    ui.label(
                        RichText::new(message)
                            .color(self.colors.text_dim)
                            .font(FontId::proportional(14.0)),
                    );
                });
            }
            return;
        }

        let show_favorites_header = !has_query
            && self.settings.home_view == HomeView::AllApps
            && self.results.iter().any(|result| result.entry.is_favorite);
        let show_history_header = !has_query
            && self.settings.home_view != HomeView::Favorites
            && self
                .results
                .iter()
                .any(|result| result.entry.launch_count > 0 && !result.entry.is_favorite);
        let mut favorites_header_shown = false;
        let mut history_header_shown = false;
        // Interactions can update `self.results` while this frame is being painted. Render a
        // snapshot so a launch, favorite toggle, or hide action cannot invalidate the loop.
        let results = self.results.clone();

        for (index, result) in results.iter().enumerate() {
            let is_favorite = result.entry.is_favorite;
            let has_history = result.entry.launch_count > 0;
            if show_favorites_header && is_favorite && !favorites_header_shown {
                favorites_header_shown = true;
                self.show_section_header(ui, "★  FAVORITES");
            } else if show_history_header && !is_favorite && has_history && !history_header_shown {
                history_header_shown = true;
                self.show_section_header(
                    ui,
                    match self.settings.history_sort {
                        HistorySort::Recent => "◷  RECENTLY USED",
                        HistorySort::Frequent => "↗  FREQUENTLY USED",
                        HistorySort::Alphabetical => "◷  HISTORY",
                    },
                );
            }

            self.show_result_item(ui, index, result);
        }

        if has_query && self.settings.web_search_enabled {
            self.show_web_search_result(ui);
        }
    }

    fn show_web_search_result(&mut self, ui: &mut egui::Ui) {
        let query = self.query.trim().to_owned();
        let provider = web_search_provider_name(self.settings.web_search_provider);
        let label = format!("🌐  Search the web for ‘{}’", short_name(&query, 36));
        let response = ui.add_sized(
            Vec2::new(ui.available_width(), 54.0),
            egui::Button::new(
                RichText::new(label)
                    .color(self.colors.accent)
                    .font(FontId::proportional(14.0)),
            )
            .fill(self.colors.bg_light)
            .corner_radius(self.card_radius()),
        );
        if response.clicked() {
            self.search_web(&query);
        }
        response.on_hover_text(format!("Search with {provider}"));
    }

    fn show_section_header(&self, ui: &mut egui::Ui, title: &str) {
        ui.add_space(8.0);
        ui.label(
            RichText::new(title)
                .color(self.colors.text_dim)
                .font(FontId::proportional(11.0))
                .strong(),
        );
        ui.add_space(4.0);
    }

    fn show_result_item(&mut self, ui: &mut egui::Ui, index: usize, result: &SearchResult) {
        let density_height = match self.settings.result_density {
            ResultDensity::Compact => 60.0,
            ResultDensity::Comfortable => 76.0,
            ResultDensity::Large => 92.0,
        };
        let vertical_padding = if self.settings.large_result_margins {
            7.0
        } else {
            2.0
        };
        let row_height = density_height + vertical_padding;
        let row_width = ui.available_width();
        let (row_rect, _) =
            ui.allocate_exact_size(Vec2::new(row_width, row_height), egui::Sense::hover());
        let button_width = 48.0;
        let row_radius = self.card_radius();
        let main_rect = Rect::from_min_max(
            row_rect.min,
            Pos2::new(row_rect.max.x - button_width * 2.0, row_rect.max.y),
        );
        let response = ui.interact(
            main_rect,
            ui.id().with(("app-result", result.entry.exec.as_str())),
            egui::Sense::click(),
        );
        let selected = index == self.selected_index;
        let bg = if selected || response.hovered() {
            self.colors.bg_hover
        } else {
            self.colors.bg
        };
        ui.painter().rect_filled(row_rect, row_radius, bg);
        if selected {
            ui.painter().rect_stroke(
                row_rect,
                row_radius,
                Stroke::new(1.0_f32, self.colors.accent_dim),
                egui::StrokeKind::Inside,
            );
        } else if self.settings.show_separators {
            ui.painter().hline(
                row_rect.x_range(),
                row_rect.bottom(),
                Stroke::new(1.0_f32, self.colors.border),
            );
        }

        let content_padding = if self.settings.large_result_margins {
            18.0
        } else {
            12.0
        };
        let icon_width = if self.settings.show_app_icons {
            42.0
        } else {
            0.0
        };
        if self.settings.show_app_icons {
            let icon = Self::get_app_icon(&result.entry);
            ui.painter().text(
                Pos2::new(
                    row_rect.left() + content_padding + 14.0,
                    row_rect.center().y,
                ),
                Align2::CENTER_CENTER,
                icon,
                FontId::proportional(24.0),
                self.colors.accent,
            );
        }

        let text_left = row_rect.left() + content_padding + icon_width;
        let mut detail = if self.settings.show_app_descriptions {
            if !result.entry.comment.is_empty() {
                result.entry.comment.clone()
            } else {
                result.entry.categories.join(" · ")
            }
        } else {
            String::new()
        };
        if self.settings.show_launch_counts && result.entry.launch_count > 0 {
            if !detail.is_empty() {
                detail.push_str("  ·  ");
            }
            detail.push_str(&format!("×{}", result.entry.launch_count));
        }
        let tags = if self.settings.show_tags && !result.entry.tags.is_empty() {
            result
                .entry
                .tags
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join("  ·  ")
        } else {
            String::new()
        };
        let mut lines: Vec<(&str, Color32, f32)> = Vec::new();
        let name_color = if result.entry.is_favorite {
            self.colors.favorite
        } else if result.match_type == MatchType::Exact {
            self.colors.accent
        } else {
            self.colors.text
        };
        lines.push((result.entry.name.as_str(), name_color, 15.0));
        if !detail.is_empty() {
            lines.push((detail.as_str(), self.colors.text_dim, 11.5));
        }
        if !tags.is_empty() {
            lines.push((tags.as_str(), self.colors.accent_dim, 10.0));
        }

        let line_heights: Vec<f32> = lines.iter().map(|(_, _, size)| size + 2.0).collect();
        let total_text_height: f32 = line_heights.iter().sum();
        let mut text_y = row_rect.center().y - total_text_height / 2.0;
        let text_clip = Rect::from_min_max(
            Pos2::new(text_left, row_rect.top()),
            Pos2::new(main_rect.right() - 6.0, row_rect.bottom()),
        );
        let text_painter = ui.painter().with_clip_rect(text_clip);
        for ((text, color, size), line_height) in lines.iter().zip(line_heights) {
            text_painter.text(
                Pos2::new(text_left, text_y + line_height / 2.0),
                Align2::LEFT_CENTER,
                text,
                FontId::proportional(*size),
                *color,
            );
            text_y += line_height;
        }

        let favorite_rect = Rect::from_min_max(
            Pos2::new(row_rect.right() - button_width, row_rect.top()),
            row_rect.right_bottom(),
        );
        let tag_rect = Rect::from_min_max(
            Pos2::new(row_rect.right() - button_width * 2.0, row_rect.top()),
            Pos2::new(row_rect.right() - button_width, row_rect.bottom()),
        );
        let favorite_text = if result.entry.is_favorite {
            "★"
        } else {
            "☆"
        };
        let favorite_color = if result.entry.is_favorite {
            self.colors.favorite
        } else {
            self.colors.text_dim
        };
        let favorite_response = ui.put(
            favorite_rect,
            egui::Button::new(
                RichText::new(favorite_text)
                    .font(FontId::proportional(22.0))
                    .color(favorite_color),
            )
            .min_size(Vec2::splat(button_width))
            .fill(Color32::TRANSPARENT)
            .corner_radius(row_radius),
        );
        if favorite_response.clicked() {
            self.toggle_favorite(index);
        }
        favorite_response.on_hover_text(if result.entry.is_favorite {
            "Remove from favorites"
        } else {
            "Add to favorites"
        });

        let tag_response = ui.put(
            tag_rect,
            egui::Button::new(
                RichText::new("🏷")
                    .font(FontId::proportional(22.0))
                    .color(self.colors.text_dim),
            )
            .min_size(Vec2::splat(button_width))
            .fill(Color32::TRANSPARENT)
            .corner_radius(row_radius),
        );
        if tag_response.clicked() {
            self.toggle_tag_editor(&result.entry.exec);
        }
        tag_response.on_hover_text("Edit tags");

        let exec = result.entry.exec.clone();
        let name = result.entry.name.clone();
        let mut context_action = None;
        match self.settings.long_press_action {
            LongPressAction::ContextMenu => {
                response.context_menu(|menu| {
                    menu.set_min_width(190.0);
                    if menu.button("Open app").clicked() {
                        context_action = Some(ResultAction::Launch);
                        menu.close();
                    }
                    if menu
                        .button(if result.entry.is_favorite {
                            "Remove from favorites"
                        } else {
                            "Add to favorites"
                        })
                        .clicked()
                    {
                        context_action = Some(ResultAction::ToggleFavorite);
                        menu.close();
                    }
                    if menu.button("Edit tags").clicked() {
                        context_action = Some(ResultAction::EditTags);
                        menu.close();
                    }
                    if menu.button("Hide app").clicked() {
                        context_action = Some(ResultAction::HideApp);
                        menu.close();
                    }
                });
            }
            LongPressAction::ToggleFavorite if response.secondary_clicked() => {
                context_action = Some(ResultAction::ToggleFavorite);
            }
            LongPressAction::EditTags if response.secondary_clicked() => {
                context_action = Some(ResultAction::EditTags);
            }
            LongPressAction::LaunchApp if response.secondary_clicked() => {
                context_action = Some(ResultAction::Launch);
            }
            LongPressAction::None
            | LongPressAction::ToggleFavorite
            | LongPressAction::EditTags
            | LongPressAction::LaunchApp => {}
        }

        if response.clicked() {
            self.launch_app(index);
        }
        if response.hovered() {
            self.selected_index = index;
        }
        if let Some(action) = context_action {
            self.perform_result_action(&exec, &name, action);
        }

        if self.editing_tags.as_deref() == Some(result.entry.exec.as_str()) {
            self.show_tag_editor(ui, &result.entry.exec);
        }

        ui.add_space(vertical_padding);
    }

    fn show_tag_editor(&mut self, ui: &mut egui::Ui, exec: &str) {
        let frame = egui::Frame::NONE
            .fill(self.colors.bg_light)
            .corner_radius(CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(10, 8))
            .stroke(Stroke::new(1.0_f32, self.colors.border));
        frame.show(ui, |ui| {
            ui.label(
                RichText::new("Tags · separate with commas")
                    .color(self.colors.text_dim)
                    .font(FontId::proportional(12.0)),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let button_width = 64.0;
                let input_width =
                    (ui.available_width() - button_width * 2.0 - ui.spacing().item_spacing.x * 2.0)
                        .max(60.0);
                let response = ui.add_sized(
                    Vec2::new(input_width, 48.0),
                    egui::TextEdit::singleline(&mut self.tag_input)
                        .id_salt(("tag-editor", exec))
                        .desired_width(input_width)
                        .hint_text("work, web, games…")
                        .font(FontId::proportional(15.0)),
                );
                if self.tag_focus_pending {
                    response.request_focus();
                    self.tag_focus_pending = false;
                }
                if ui
                    .add_sized(
                        Vec2::new(button_width, 48.0),
                        egui::Button::new("Save")
                            .fill(self.colors.accent_dim)
                            .corner_radius(CornerRadius::same(8)),
                    )
                    .clicked()
                {
                    self.save_tags(exec);
                }
                if ui
                    .add_sized(
                        Vec2::new(button_width, 48.0),
                        egui::Button::new("Cancel")
                            .fill(self.colors.bg_hover)
                            .corner_radius(CornerRadius::same(8)),
                    )
                    .clicked()
                {
                    self.editing_tags = None;
                    self.tag_focus_pending = false;
                }
            });
        });
    }

    fn favorite_apps(&self) -> Vec<AppEntry> {
        let mut favorites: Vec<AppEntry> = self
            .apps
            .iter()
            .filter(|app| app.is_favorite && !self.settings.hidden_apps.contains(&app.exec))
            .cloned()
            .collect();
        favorites.sort_by_key(|app| app.name.to_lowercase());
        favorites
    }

    fn show_favorites_bar(&mut self, ui: &mut egui::Ui, favorites: &[AppEntry], height: f32) {
        let large_bar = self.settings.large_favorites_bar;
        let colors = self.colors;
        let radius = self.card_radius();
        let fill = if self.settings.transparent_favorites_bar {
            colors.bg
        } else {
            colors.bg_light
        };
        let frame = egui::Frame::NONE
            .fill(fill)
            .corner_radius(radius)
            .inner_margin(egui::Margin::symmetric(6, 4));
        let mut launch_exec = None;
        let mut remove_favorite = None;
        let mut edit_tags = None;

        frame.show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("favorites-bar")
                .max_height(height - 8.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for app in favorites {
                            let button_height = height - 12.0;
                            let (label, width) = if large_bar {
                                (
                                    format!(
                                        "{}\n{}",
                                        Self::get_app_icon(app),
                                        short_name(&app.name, 12)
                                    ),
                                    88.0,
                                )
                            } else {
                                (Self::get_app_icon(app), 52.0)
                            };
                            let response = ui
                                .add_sized(
                                    Vec2::new(width, button_height.max(48.0)),
                                    egui::Button::new(
                                        RichText::new(label)
                                            .font(FontId::proportional(if large_bar {
                                                13.0
                                            } else {
                                                22.0
                                            }))
                                            .color(colors.accent),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .corner_radius(radius),
                                )
                                .on_hover_text(app.name.clone());
                            if response.clicked() {
                                launch_exec = Some(app.exec.clone());
                            }
                            let exec = app.exec.clone();
                            let name = app.name.clone();
                            response.context_menu(|menu| {
                                if menu.button("Remove from favorites").clicked() {
                                    remove_favorite = Some((exec.clone(), name.clone()));
                                    menu.close();
                                }
                                if menu.button("Edit tags").clicked() {
                                    edit_tags = Some(exec.clone());
                                    menu.close();
                                }
                            });
                        }
                    });
                });
        });

        if let Some(exec) = launch_exec {
            self.launch_by_exec(&exec);
        }
        if let Some((exec, name)) = remove_favorite {
            self.toggle_favorite_for(&exec, &name);
        }
        if let Some(exec) = edit_tags {
            self.toggle_tag_editor(&exec);
        }
    }

    fn show_search_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let fill = if self.settings.transparent_search_bar {
            self.colors.bg
        } else {
            self.colors.search_bg
        };
        let frame = egui::Frame::NONE
            .fill(fill)
            .corner_radius(CornerRadius::same(14))
            .inner_margin(egui::Margin::symmetric(10, 8))
            .stroke(Stroke::new(1.0_f32, self.colors.border));

        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                let input_height = if self.settings.large_search_bar {
                    56.0
                } else {
                    48.0
                };
                ui.add_sized(
                    Vec2::new(30.0, input_height),
                    egui::Label::new(
                        RichText::new("⌕")
                            .font(FontId::proportional(25.0))
                            .color(self.colors.accent),
                    ),
                );
                let clear_width = if self.query.is_empty() { 0.0 } else { 48.0 };
                let input_width = (ui.available_width() - clear_width - 4.0).max(60.0);
                let hint = if self.settings.show_search_hints {
                    "Search apps, tags, or calculate…"
                } else {
                    ""
                };
                let response = ui.add_sized(
                    Vec2::new(input_width, input_height),
                    egui::TextEdit::singleline(&mut self.query)
                        .id_salt("main-search-field")
                        .desired_width(input_width)
                        .hint_text(hint)
                        .font(FontId::proportional(16.0))
                        .text_color(self.colors.text),
                );
                if response.changed() {
                    self.selected_index = 0;
                    self.update_results();
                }
                if self.search_focus_pending {
                    response.request_focus();
                    self.search_focus_pending = false;
                }
                if self.search_surrender_pending {
                    response.surrender_focus();
                    self.search_surrender_pending = false;
                }

                if !self.query.is_empty()
                    && ui
                        .add_sized(
                            Vec2::new(48.0, input_height),
                            egui::Button::new(
                                RichText::new("×")
                                    .font(FontId::proportional(22.0))
                                    .color(self.colors.text_dim),
                            )
                            .fill(Color32::TRANSPARENT)
                            .corner_radius(CornerRadius::same(12)),
                        )
                        .on_hover_text("Clear search")
                        .clicked()
                {
                    self.query.clear();
                    self.selected_index = 0;
                    self.update_results();
                    self.search_focus_pending = true;
                }
            });

            if self.settings.show_search_hints {
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("Tap to launch  ·  Hold for actions")
                            .color(self.colors.text_dim)
                            .font(FontId::proportional(10.0)),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("↑↓ navigate  ·  ↵ open  ·  Esc clear/back")
                            .color(self.colors.accent_dim)
                            .font(FontId::proportional(10.0)),
                    );
                });
            }
        });

        if self.status_message.is_some() {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
    }

    fn show_settings_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let width = ui.available_width();
        ui.allocate_ui(Vec2::new(width, 56.0), |ui| {
            ui.horizontal_centered(|ui| {
                if ui
                    .add_sized(
                        Vec2::splat(48.0),
                        egui::Button::new("‹")
                            .fill(Color32::TRANSPARENT)
                            .corner_radius(CornerRadius::same(12)),
                    )
                    .on_hover_text("Back to apps")
                    .clicked()
                {
                    self.page = Page::Launcher;
                    if self.settings.focus_search_on_start {
                        self.search_focus_pending = true;
                    }
                }
                ui.label(
                    RichText::new("Settings")
                        .font(FontId::proportional(20.0))
                        .strong()
                        .color(self.colors.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Changes save automatically")
                            .font(FontId::proportional(10.0))
                            .color(self.colors.text_dim),
                    );
                });
            });
        });

        ui.spacing_mut().interact_size.y = 48.0;
        ui.spacing_mut().button_padding = Vec2::new(12.0, 10.0);
        ui.add_space(4.0);
        egui::ScrollArea::horizontal()
            .id_salt("settings-categories")
            .max_height(52.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (category, title) in settings_categories() {
                        let selected = self.settings_category == category;
                        if ui
                            .add_sized(
                                Vec2::new(94.0, 46.0),
                                egui::Button::new(
                                    RichText::new(title).font(FontId::proportional(12.0)).color(
                                        if selected {
                                            Color32::WHITE
                                        } else {
                                            self.colors.text_dim
                                        },
                                    ),
                                )
                                .fill(if selected {
                                    self.colors.accent_dim
                                } else {
                                    self.colors.bg_light
                                })
                                .corner_radius(CornerRadius::same(12)),
                            )
                            .clicked()
                        {
                            self.settings_category = category;
                        }
                    }
                });
            });
        ui.add_space(4.0);
        ui.separator();

        let colors = self.colors;
        let mut changed = false;
        egui::ScrollArea::vertical()
            .id_salt("settings-content")
            .auto_shrink([false, false])
            .show(ui, |ui| match self.settings_category {
                SettingsCategory::Search => self.draw_search_settings(ui, colors, &mut changed),
                SettingsCategory::Appearance => {
                    self.draw_appearance_settings(ui, colors, &mut changed)
                }
                SettingsCategory::Touch => self.draw_touch_settings(ui, colors, &mut changed),
                SettingsCategory::History => self.draw_history_settings(ui, colors, &mut changed),
                SettingsCategory::HiddenApps => {
                    self.draw_hidden_app_settings(ui, colors, &mut changed)
                }
                SettingsCategory::Advanced => {
                    self.draw_advanced_settings(ui, colors, ctx, &mut changed)
                }
            });

        if changed {
            self.settings.save();
            self.update_results();
        }
    }

    fn draw_search_settings(&mut self, ui: &mut egui::Ui, colors: Colors, changed: &mut bool) {
        settings_heading(
            ui,
            colors,
            "Search & providers",
            "Choose what RISS searches and how many matches it shows.",
        );
        *changed |= settings_choice(
            ui,
            "Home screen",
            &mut self.settings.home_view,
            &[
                (HomeView::AllApps, "All apps"),
                (HomeView::History, "History"),
                (HomeView::Favorites, "Favorites only"),
            ],
            "home-view",
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!(
                "Maximum search results: {}",
                self.settings.results_limit
            ))
            .color(colors.text)
            .font(FontId::proportional(14.0)),
        );
        if ui
            .add_sized(
                Vec2::new(ui.available_width(), 44.0),
                egui::Slider::new(&mut self.settings.results_limit, 1..=150).show_value(false),
            )
            .changed()
        {
            *changed = true;
        }

        *changed |= settings_toggle(
            ui,
            colors,
            "Search installed apps",
            "Include app names and launchable programs in results.",
            &mut self.settings.search_apps,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Calculator",
            "Show a live result for arithmetic expressions typed into search.",
            &mut self.settings.calculator_enabled,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Web search",
            "Offer a one-tap browser search when you type a query.",
            &mut self.settings.web_search_enabled,
        );
        *changed |= settings_choice(
            ui,
            "Default web provider",
            &mut self.settings.web_search_provider,
            &[
                (WebSearchProvider::DuckDuckGo, "DuckDuckGo"),
                (WebSearchProvider::Google, "Google"),
                (WebSearchProvider::Brave, "Brave Search"),
            ],
            "web-search-provider",
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Always search the web on Enter",
            "When enabled, Enter opens a browser search instead of launching the selected app.",
            &mut self.settings.always_default_web_search_on_enter,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Search descriptions",
            "Match app descriptions as well as app names.",
            &mut self.settings.search_descriptions,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Search tags",
            "Match custom tags and built-in app keywords.",
            &mut self.settings.search_tags,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Search categories",
            "Match app categories such as Development or Games.",
            &mut self.settings.search_categories,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Show search and touch hints",
            "Show the search prompt, touch guidance, and keyboard shortcuts.",
            &mut self.settings.show_search_hints,
        );
    }

    fn draw_appearance_settings(&mut self, ui: &mut egui::Ui, colors: Colors, changed: &mut bool) {
        settings_heading(
            ui,
            colors,
            "Appearance",
            "Tune the theme, app list, favorites strip, and search bar.",
        );
        *changed |= settings_choice(
            ui,
            "Theme",
            &mut self.settings.theme,
            &[
                (ThemeMode::System, "Follow system"),
                (ThemeMode::Dark, "Dark"),
                (ThemeMode::Light, "Light"),
                (ThemeMode::Amoled, "AMOLED black"),
            ],
            "theme-mode",
        );
        *changed |= settings_choice(
            ui,
            "Accent color",
            &mut self.settings.accent,
            &[
                (AccentColor::Blue, "Blue"),
                (AccentColor::Purple, "Purple"),
                (AccentColor::Teal, "Teal"),
                (AccentColor::Green, "Green"),
                (AccentColor::Orange, "Orange"),
                (AccentColor::Rose, "Rose"),
            ],
            "accent-color",
        );
        *changed |= settings_choice(
            ui,
            "Result size",
            &mut self.settings.result_density,
            &[
                (ResultDensity::Compact, "Compact"),
                (ResultDensity::Comfortable, "Comfortable"),
                (ResultDensity::Large, "Large touch rows"),
            ],
            "result-density",
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Show app icons",
            "Show a category or app monogram beside each result.",
            &mut self.settings.show_app_icons,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Show descriptions",
            "Show the app description below its name.",
            &mut self.settings.show_app_descriptions,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Show launch counts",
            "Display usage counts in app results.",
            &mut self.settings.show_launch_counts,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Show tags on results",
            "Display custom tags on app cards.",
            &mut self.settings.show_tags,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Large search bar",
            "Make the search field taller for easier thumb access.",
            &mut self.settings.large_search_bar,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Separate result rows",
            "Draw subtle dividers between app results.",
            &mut self.settings.show_separators,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Rounded app cards",
            "Use rounded corners for results and touch buttons.",
            &mut self.settings.rounded_cards,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Larger list margins",
            "Add more space around results for easier one-handed tapping.",
            &mut self.settings.large_result_margins,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Transparent search bar",
            "Blend the search field into the launcher background.",
            &mut self.settings.transparent_search_bar,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Favorites bar",
            "Keep pinned apps in a quick-launch strip above search.",
            &mut self.settings.show_favorites_bar,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Large favorites bar",
            "Show app names as well as icons in the favorites strip.",
            &mut self.settings.large_favorites_bar,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Transparent favorites bar",
            "Let the favorites strip use the launcher background.",
            &mut self.settings.transparent_favorites_bar,
        );
    }

    fn draw_touch_settings(&mut self, ui: &mut egui::Ui, colors: Colors, changed: &mut bool) {
        settings_heading(
            ui,
            colors,
            "Touch & gestures",
            "App rows launch on tap. Press and hold a row for actions; buttons use large touch targets.",
        );
        *changed |= settings_choice(
            ui,
            "Press and hold an app",
            &mut self.settings.long_press_action,
            &[
                (LongPressAction::ContextMenu, "Show action menu"),
                (LongPressAction::ToggleFavorite, "Toggle favorite"),
                (LongPressAction::EditTags, "Edit tags"),
                (LongPressAction::LaunchApp, "Launch app"),
                (LongPressAction::None, "Do nothing"),
            ],
            "long-press-action",
        );
        let gesture_actions = [
            (GestureAction::None, "Do nothing"),
            (GestureAction::FocusSearch, "Focus search"),
            (GestureAction::ClearSearch, "Clear search"),
            (GestureAction::ToggleSettings, "Open Settings"),
            (GestureAction::RefreshApps, "Refresh apps"),
            (GestureAction::CycleHomeView, "Cycle home screen view"),
        ];
        *changed |= settings_choice(
            ui,
            "Swipe up on the RISS title",
            &mut self.settings.swipe_up,
            &gesture_actions,
            "swipe-up-action",
        );
        *changed |= settings_choice(
            ui,
            "Swipe down on the RISS title",
            &mut self.settings.swipe_down,
            &gesture_actions,
            "swipe-down-action",
        );
        *changed |= settings_choice(
            ui,
            "Swipe left on the RISS title",
            &mut self.settings.swipe_left,
            &gesture_actions,
            "swipe-left-action",
        );
        *changed |= settings_choice(
            ui,
            "Swipe right on the RISS title",
            &mut self.settings.swipe_right,
            &gesture_actions,
            "swipe-right-action",
        );
        *changed |= settings_choice(
            ui,
            "Double-tap the RISS title",
            &mut self.settings.double_tap_action,
            &gesture_actions,
            "double-tap-action",
        );
        ui.add_space(8.0);
        *changed |= settings_toggle(
            ui,
            colors,
            "Focus search on launch",
            "Put the cursor in search when RISS opens. Tapping another control will not steal focus back.",
            &mut self.settings.focus_search_on_start,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Hide keyboard after launching",
            "Release search focus after an app is opened so the on-screen keyboard can close.",
            &mut self.settings.hide_keyboard_after_launch,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Clear query after launching",
            "Return to the configured home list after opening an app.",
            &mut self.settings.clear_search_after_launch,
        );
    }

    fn draw_history_settings(&mut self, ui: &mut egui::Ui, colors: Colors, changed: &mut bool) {
        settings_heading(
            ui,
            colors,
            "History & favorites",
            "Control app ranking, pinned apps, and whether launches are remembered.",
        );
        *changed |= settings_choice(
            ui,
            "History order",
            &mut self.settings.history_sort,
            &[
                (HistorySort::Recent, "Most recent"),
                (HistorySort::Frequent, "Most used"),
                (HistorySort::Alphabetical, "Alphabetical"),
            ],
            "history-sort",
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Favorites first",
            "Place favorited apps above other matches when showing the full app list.",
            &mut self.settings.favorite_first,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Exclude favorites from app results",
            "Keep pinned apps in the favorites strip instead of regular app results.",
            &mut self.settings.hide_favorites_from_apps,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Exclude favorites from history",
            "Do not duplicate pinned apps in the recent or frequently used list.",
            &mut self.settings.hide_favorites_from_history,
        );
        *changed |= settings_toggle(
            ui,
            colors,
            "Pause launch history",
            "New launches will not change usage counts or recency until this is switched off.",
            &mut self.settings.freeze_history,
        );

        ui.add_space(8.0);
        if ui
            .add_sized(
                Vec2::new(ui.available_width(), 50.0),
                egui::Button::new("Clear launch history…")
                    .fill(colors.bg_light)
                    .corner_radius(CornerRadius::same(10)),
            )
            .clicked()
        {
            self.confirm_action = Some(ConfirmAction::ClearHistory);
        }
        if ui
            .add_sized(
                Vec2::new(ui.available_width(), 50.0),
                egui::Button::new("Clear favorites…")
                    .fill(colors.bg_light)
                    .corner_radius(CornerRadius::same(10)),
            )
            .clicked()
        {
            self.confirm_action = Some(ConfirmAction::ClearFavorites);
        }
    }

    fn draw_hidden_app_settings(&mut self, ui: &mut egui::Ui, colors: Colors, changed: &mut bool) {
        settings_heading(
            ui,
            colors,
            "Hidden apps",
            "With the long-press action set to Show action menu, hold an app and choose Hide app. Hidden apps can be restored here at any time.",
        );
        let hidden_apps = self.settings.hidden_apps.clone();
        if hidden_apps.is_empty() {
            ui.label(
                RichText::new("No apps are hidden.")
                    .color(colors.text_dim)
                    .font(FontId::proportional(14.0)),
            );
            return;
        }

        let mut restore = Vec::new();
        for exec in hidden_apps {
            let name = self
                .apps
                .iter()
                .find(|app| app.exec == exec)
                .map(|app| app.name.clone())
                .unwrap_or_else(|| exec.clone());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(name)
                        .color(colors.text)
                        .font(FontId::proportional(14.0)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized(
                            Vec2::new(92.0, 48.0),
                            egui::Button::new("Restore")
                                .fill(colors.accent_dim)
                                .corner_radius(CornerRadius::same(9)),
                        )
                        .clicked()
                    {
                        restore.push(exec.clone());
                    }
                });
            });
            ui.add_space(4.0);
        }

        if !restore.is_empty() {
            self.settings
                .hidden_apps
                .retain(|exec| !restore.iter().any(|restored| restored == exec));
            *changed = true;
        }
    }

    fn draw_advanced_settings(
        &mut self,
        ui: &mut egui::Ui,
        colors: Colors,
        ctx: &egui::Context,
        _changed: &mut bool,
    ) {
        settings_heading(
            ui,
            colors,
            "Data & advanced",
            "RISS saves preferences separately from launch history and favorites.",
        );
        #[cfg(target_os = "android")]
        if ui
            .add_sized(
                Vec2::new(ui.available_width(), 50.0),
                egui::Button::new("Choose RISS as the home app…")
                    .fill(colors.bg_light)
                    .corner_radius(CornerRadius::same(10)),
            )
            .clicked()
        {
            match app_entry::android_jni::open_default_apps_settings_jni() {
                Ok(()) => self.set_status("Choose RISS in Android's Home app setting".to_owned()),
                Err(error) => self.set_status(format!("Could not open Android settings: {error}")),
            }
        }
        #[cfg(not(target_os = "android"))]
        ui.label(
            RichText::new("Default launcher selection is managed by your desktop environment.")
                .color(colors.text_dim)
                .font(FontId::proportional(12.0)),
        );
        if ui
            .add_sized(
                Vec2::new(ui.available_width(), 50.0),
                egui::Button::new("Export settings to clipboard")
                    .fill(colors.bg_light)
                    .corner_radius(CornerRadius::same(10)),
            )
            .clicked()
        {
            match serde_json::to_string_pretty(&self.settings) {
                Ok(json) => {
                    ctx.copy_text(json);
                    self.set_status("Settings copied to clipboard".to_owned());
                }
                Err(error) => self.set_status(format!("Could not export settings: {error}")),
            }
        }
        if ui
            .add_sized(
                Vec2::new(ui.available_width(), 50.0),
                egui::Button::new("Import settings from JSON…")
                    .fill(colors.bg_light)
                    .corner_radius(CornerRadius::same(10)),
            )
            .clicked()
        {
            self.import_json.clear();
            self.import_error = None;
            self.import_dialog_open = true;
        }
        if ui
            .add_sized(
                Vec2::new(ui.available_width(), 50.0),
                egui::Button::new("Restore default settings…")
                    .fill(colors.bg_light)
                    .corner_radius(CornerRadius::same(10)),
            )
            .clicked()
        {
            self.confirm_action = Some(ConfirmAction::ResetSettings);
        }
        ui.add_space(12.0);
        ui.label(
            RichText::new("Settings file: $XDG_CONFIG_HOME/riss-launcher/settings.json (or ~/.config/riss-launcher/settings.json). Android uses app-private storage.\nApp history and favorites remain in history.json.")
                .color(colors.text_dim)
                .font(FontId::proportional(12.0)),
        );
        ui.add_space(12.0);
        ui.label(
            RichText::new("Launcher controls, appearance, history, favorites, result limits, app hiding, and gesture assignments are stored locally on this device.")
                .color(colors.text_dim)
                .font(FontId::proportional(12.0)),
        );
    }

    fn show_confirmation_dialog(&mut self, ctx: &egui::Context) {
        let Some(action) = self.confirm_action else {
            return;
        };
        let title = match action {
            ConfirmAction::ClearHistory => "Clear launch history?",
            ConfirmAction::ClearFavorites => "Clear all favorites?",
            ConfirmAction::ResetSettings => "Restore default settings?",
        };
        let message = match action {
            ConfirmAction::ClearHistory => {
                "This clears launch counts and last-used times. Favorites and tags are kept."
            }
            ConfirmAction::ClearFavorites => {
                "All pinned apps will be unpinned. This cannot be undone."
            }
            ConfirmAction::ResetSettings => {
                "Appearance, gestures, hidden apps, and search preferences will return to defaults."
            }
        };
        let mut confirmed = false;
        let mut cancelled = false;
        egui::Window::new(title)
            .id(egui::Id::new("riss-confirm-dialog"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_min_width(260.0);
                ui.label(message);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_sized(Vec2::new(104.0, 48.0), egui::Button::new("Cancel"))
                        .clicked()
                    {
                        cancelled = true;
                    }
                    if ui
                        .add_sized(
                            Vec2::new(124.0, 48.0),
                            egui::Button::new("Confirm")
                                .fill(self.colors.accent_dim)
                                .corner_radius(CornerRadius::same(8)),
                        )
                        .clicked()
                    {
                        confirmed = true;
                    }
                });
            });

        if confirmed {
            self.confirm_action = None;
            match action {
                ConfirmAction::ClearHistory => {
                    self.history.launch_counts.clear();
                    self.history.last_launched.clear();
                    self.history.save();
                    self.sync_app_metadata();
                    self.update_results();
                    self.set_status("Launch history cleared".to_owned());
                }
                ConfirmAction::ClearFavorites => {
                    self.history.favorites.clear();
                    self.history.save();
                    self.sync_app_metadata();
                    self.update_results();
                    self.set_status("Favorites cleared".to_owned());
                }
                ConfirmAction::ResetSettings => {
                    self.settings = LauncherSettings::default();
                    self.settings.save();
                    self.update_results();
                    self.set_status("Settings restored to defaults".to_owned());
                }
            }
        } else if cancelled {
            self.confirm_action = None;
        }
    }

    fn show_import_dialog(&mut self, ctx: &egui::Context) {
        if !self.import_dialog_open {
            return;
        }
        let mut open = self.import_dialog_open;
        let mut cancel = false;
        let mut imported = None;
        let mut error_message = None;
        egui::Window::new("Import settings")
            .id(egui::Id::new("riss-import-settings"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("Paste a settings JSON export below.");
                ui.add_sized(
                    Vec2::new(ui.available_width(), 180.0),
                    egui::TextEdit::multiline(&mut self.import_json)
                        .id_salt("settings-import-json")
                        .desired_rows(8)
                        .font(FontId::monospace(12.0)),
                );
                if let Some(error) = &self.import_error {
                    ui.label(RichText::new(error).color(Color32::LIGHT_RED));
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_sized(Vec2::new(100.0, 48.0), egui::Button::new("Cancel"))
                        .clicked()
                    {
                        cancel = true;
                    }
                    if ui
                        .add_sized(
                            Vec2::new(120.0, 48.0),
                            egui::Button::new("Import")
                                .fill(self.colors.accent_dim)
                                .corner_radius(CornerRadius::same(8)),
                        )
                        .clicked()
                    {
                        match serde_json::from_str::<LauncherSettings>(&self.import_json) {
                            Ok(settings) => imported = Some(settings),
                            Err(error) => {
                                error_message = Some(format!("Invalid settings JSON: {error}"))
                            }
                        }
                    }
                });
            });
        if cancel {
            open = false;
        }
        self.import_dialog_open = open;

        if let Some(settings) = imported {
            self.settings = settings;
            self.settings.save();
            self.update_results();
            self.import_dialog_open = false;
            self.import_error = None;
            self.set_status("Settings imported".to_owned());
        } else if let Some(error) = error_message {
            self.import_error = Some(error);
        }
    }

    fn card_radius(&self) -> CornerRadius {
        if self.settings.rounded_cards {
            CornerRadius::same(12)
        } else {
            CornerRadius::same(3)
        }
    }
}

fn settings_categories() -> [(SettingsCategory, &'static str); 6] {
    [
        (SettingsCategory::Search, "Search"),
        (SettingsCategory::Appearance, "Appearance"),
        (SettingsCategory::Touch, "Touch"),
        (SettingsCategory::History, "History"),
        (SettingsCategory::HiddenApps, "Hidden apps"),
        (SettingsCategory::Advanced, "Advanced"),
    ]
}

fn settings_heading(ui: &mut egui::Ui, colors: Colors, title: &str, subtitle: &str) {
    ui.add_space(12.0);
    ui.label(
        RichText::new(title)
            .color(colors.accent)
            .font(FontId::proportional(18.0))
            .strong(),
    );
    ui.label(
        RichText::new(subtitle)
            .color(colors.text_dim)
            .font(FontId::proportional(12.0)),
    );
    ui.add_space(10.0);
}

fn settings_toggle(
    ui: &mut egui::Ui,
    colors: Colors,
    title: &str,
    description: &str,
    value: &mut bool,
) -> bool {
    let width = ui.available_width();
    let description_font = FontId::proportional(10.5);
    let title_height = 18.0;
    let description_gap = 3.0;
    let description_galley = ui.painter().layout(
        description.to_owned(),
        description_font,
        colors.text_dim,
        (width - 78.0).max(60.0),
    );
    let content_height = title_height + description_gap + description_galley.size().y;
    let row_height = (content_height + 24.0).max(64.0);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, row_height), egui::Sense::click());
    let clicked = response.clicked();
    if clicked {
        *value = !*value;
    }
    let fill = if response.is_pointer_button_down_on() {
        colors.bg_hover
    } else if response.hovered() {
        colors.bg_light
    } else {
        colors.bg
    };
    ui.painter().rect_filled(rect, CornerRadius::same(10), fill);
    let switch_rect = Rect::from_center_size(
        Pos2::new(rect.right() - 32.0, rect.center().y),
        Vec2::new(42.0, 24.0),
    );
    let label_painter = ui.painter().with_clip_rect(Rect::from_min_max(
        Pos2::new(rect.left() + 12.0, rect.top()),
        Pos2::new(switch_rect.left() - 12.0, rect.bottom()),
    ));
    let text_left = rect.left() + 14.0;
    let text_top = rect.center().y - content_height / 2.0;
    label_painter.text(
        Pos2::new(text_left, text_top),
        Align2::LEFT_TOP,
        title,
        FontId::proportional(14.0),
        colors.text,
    );
    label_painter.galley(
        Pos2::new(text_left, text_top + title_height + description_gap),
        description_galley,
        colors.text_dim,
    );
    ui.painter().rect_filled(
        switch_rect,
        CornerRadius::same(12),
        if *value {
            colors.accent_dim
        } else {
            colors.border
        },
    );
    let thumb_x = if *value {
        switch_rect.right() - 12.0
    } else {
        switch_rect.left() + 12.0
    };
    ui.painter().circle_filled(
        Pos2::new(thumb_x, switch_rect.center().y),
        8.5,
        if *value {
            Color32::WHITE
        } else {
            colors.text_dim
        },
    );
    response.on_hover_text(description);
    ui.add_space(4.0);
    clicked
}

fn settings_choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    title: &str,
    value: &mut T,
    choices: &[(T, &str)],
    id: &'static str,
) -> bool {
    let selected_label = choices
        .iter()
        .find(|(candidate, _)| *candidate == *value)
        .map(|(_, label)| *label)
        .unwrap_or("Choose…");
    let mut changed = false;
    ui.vertical(|ui| {
        ui.label(
            RichText::new(title)
                .font(FontId::proportional(13.0))
                .color(Color32::from_gray(150)),
        );
        egui::ComboBox::from_id_salt(id)
            .selected_text(selected_label)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (candidate, label) in choices {
                    if ui.selectable_value(value, *candidate, *label).changed() {
                        changed = true;
                    }
                }
            });
    });
    ui.add_space(8.0);
    changed
}

fn is_history_entry_visible(is_favorite: bool, launch_count: u32, hide_favorites: bool) -> bool {
    (!is_favorite || !hide_favorites) && (launch_count > 0 || is_favorite)
}

fn home_view_name(view: HomeView) -> &'static str {
    match view {
        HomeView::AllApps => "All apps",
        HomeView::History => "History",
        HomeView::Favorites => "Favorites",
    }
}

fn web_search_provider_name(provider: WebSearchProvider) -> &'static str {
    match provider {
        WebSearchProvider::DuckDuckGo => "DuckDuckGo",
        WebSearchProvider::Google => "Google",
        WebSearchProvider::Brave => "Brave Search",
    }
}

fn web_search_url(provider: WebSearchProvider, query: &str) -> String {
    let mut encoded_query = String::new();
    for byte in query.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded_query.push(byte as char);
        } else {
            encoded_query.push_str(&format!("%{byte:02X}"));
        }
    }

    let base_url = match provider {
        WebSearchProvider::DuckDuckGo => "https://duckduckgo.com/?q=",
        WebSearchProvider::Google => "https://www.google.com/search?q=",
        WebSearchProvider::Brave => "https://search.brave.com/search?q=",
    };
    format!("{base_url}{encoded_query}")
}

fn short_name(name: &str, max_chars: usize) -> String {
    let mut chars = name.chars();
    let shortened: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{shortened}…")
    } else {
        shortened
    }
}

#[cfg(test)]
mod tests {
    use super::{is_history_entry_visible, web_search_url, WebSearchProvider};

    #[test]
    fn history_view_respects_favorite_exclusion() {
        assert!(is_history_entry_visible(false, 1, true));
        assert!(!is_history_entry_visible(false, 0, true));
        assert!(!is_history_entry_visible(true, 1, true));
        assert!(!is_history_entry_visible(true, 0, true));
        assert!(is_history_entry_visible(true, 1, false));
        assert!(is_history_entry_visible(true, 0, false));
    }

    #[test]
    fn web_search_url_encodes_query_as_utf8() {
        assert_eq!(
            web_search_url(WebSearchProvider::DuckDuckGo, "Rust & egui/猫"),
            "https://duckduckgo.com/?q=Rust%20%26%20egui%2F%E7%8C%AB"
        );
    }

    #[test]
    fn web_search_provider_changes_the_search_endpoint() {
        assert_eq!(
            web_search_url(WebSearchProvider::Google, "launcher"),
            "https://www.google.com/search?q=launcher"
        );
        assert_eq!(
            web_search_url(WebSearchProvider::Brave, "launcher"),
            "https://search.brave.com/search?q=launcher"
        );
    }
}
