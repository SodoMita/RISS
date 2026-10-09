use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Where the launcher gets its empty-query app list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HomeView {
    AllApps,
    #[default]
    History,
    Favorites,
}

/// Sort order for apps shown before a query is entered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistorySort {
    Recent,
    #[default]
    Frequent,
    Alphabetical,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    System,
    Light,
    #[default]
    Dark,
    Amoled,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccentColor {
    #[default]
    Blue,
    Purple,
    Teal,
    Green,
    Orange,
    Rose,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebSearchProvider {
    #[default]
    DuckDuckGo,
    Google,
    Brave,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultDensity {
    Compact,
    #[default]
    Comfortable,
    Large,
}

/// Actions that can be assigned to the launcher gesture surface in the toolbar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GestureAction {
    #[default]
    None,
    FocusSearch,
    ClearSearch,
    ToggleSettings,
    RefreshApps,
    CycleHomeView,
}

/// What a press-and-hold on an app result should do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LongPressAction {
    #[default]
    ContextMenu,
    ToggleFavorite,
    EditTags,
    LaunchApp,
    None,
}

/// User-configurable launcher behavior and appearance.
///
/// `serde(default)` keeps settings from older versions loadable as new options are added.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherSettings {
    pub home_view: HomeView,
    pub history_sort: HistorySort,
    pub results_limit: usize,
    pub favorite_first: bool,
    pub hide_favorites_from_apps: bool,
    pub hide_favorites_from_history: bool,
    pub freeze_history: bool,

    pub search_apps: bool,
    pub search_descriptions: bool,
    pub search_tags: bool,
    pub search_categories: bool,
    pub calculator_enabled: bool,
    pub web_search_enabled: bool,
    pub web_search_provider: WebSearchProvider,
    pub always_default_web_search_on_enter: bool,

    pub show_app_icons: bool,
    pub show_app_descriptions: bool,
    pub show_launch_counts: bool,
    pub show_tags: bool,
    pub show_search_hints: bool,
    pub large_search_bar: bool,
    pub show_separators: bool,
    pub focus_search_on_start: bool,
    pub hide_keyboard_after_launch: bool,
    pub clear_search_after_launch: bool,

    pub theme: ThemeMode,
    pub accent: AccentColor,
    pub result_density: ResultDensity,
    pub rounded_cards: bool,
    pub large_result_margins: bool,
    pub transparent_search_bar: bool,
    pub show_favorites_bar: bool,
    pub large_favorites_bar: bool,
    pub transparent_favorites_bar: bool,

    pub long_press_action: LongPressAction,
    pub swipe_up: GestureAction,
    pub swipe_down: GestureAction,
    pub swipe_left: GestureAction,
    pub swipe_right: GestureAction,
    pub double_tap_action: GestureAction,

    /// App IDs excluded from results. The entries themselves remain available here to restore.
    pub hidden_apps: Vec<String>,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            home_view: HomeView::History,
            history_sort: HistorySort::Frequent,
            results_limit: 20,
            favorite_first: true,
            hide_favorites_from_apps: false,
            hide_favorites_from_history: true,
            freeze_history: false,

            search_apps: true,
            search_descriptions: true,
            search_tags: true,
            search_categories: true,
            calculator_enabled: true,
            web_search_enabled: true,
            web_search_provider: WebSearchProvider::DuckDuckGo,
            always_default_web_search_on_enter: false,

            show_app_icons: true,
            show_app_descriptions: true,
            show_launch_counts: true,
            show_tags: true,
            show_search_hints: true,
            large_search_bar: false,
            show_separators: false,
            focus_search_on_start: true,
            hide_keyboard_after_launch: true,
            clear_search_after_launch: true,

            theme: ThemeMode::Dark,
            accent: AccentColor::Blue,
            result_density: ResultDensity::Comfortable,
            rounded_cards: true,
            large_result_margins: false,
            transparent_search_bar: false,
            show_favorites_bar: true,
            large_favorites_bar: false,
            transparent_favorites_bar: true,

            long_press_action: LongPressAction::ContextMenu,
            swipe_up: GestureAction::None,
            swipe_down: GestureAction::None,
            swipe_left: GestureAction::None,
            swipe_right: GestureAction::None,
            double_tap_action: GestureAction::None,

            hidden_apps: Vec::new(),
        }
    }
}

impl LauncherSettings {
    fn data_path() -> PathBuf {
        #[cfg(target_os = "android")]
        {
            crate::android_app_entry::android_jni::internal_data_path()
                .map(|directory| directory.join("settings.json"))
                .unwrap_or_else(|| PathBuf::from("settings.json"))
        }

        #[cfg(not(target_os = "android"))]
        {
            let config_home = std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(PathBuf::from)
                        .map(|home| home.join(".config"))
                })
                .unwrap_or_else(|| PathBuf::from("."));
            config_home.join("riss-launcher").join("settings.json")
        }
    }

    pub fn load() -> Self {
        fs::read_to_string(Self::data_path())
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::data_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_use_defaults() {
        let settings: LauncherSettings = serde_json::from_str("{}").unwrap();
        assert!(settings.search_apps);
        assert!(settings.calculator_enabled);
        assert!(settings.web_search_enabled);
        assert_eq!(settings.results_limit, 20);
        assert_eq!(settings.home_view, HomeView::History);
        assert_eq!(settings.long_press_action, LongPressAction::ContextMenu);
    }

    #[test]
    fn settings_round_trip() {
        let mut settings = LauncherSettings::default();
        settings.theme = ThemeMode::Amoled;
        settings.hidden_apps.push("example.app".to_owned());

        let encoded = serde_json::to_string(&settings).unwrap();
        let decoded: LauncherSettings = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.theme, ThemeMode::Amoled);
        assert_eq!(decoded.hidden_apps, vec!["example.app".to_owned()]);
    }
}
