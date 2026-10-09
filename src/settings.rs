//! User settings for RISS Launcher.
//!
//! The settings model mirrors the option set of the original KISS launcher
//! (https://github.com/Neamar/KISS), translated to desktop concepts: gestures
//! become keyboard shortcuts, the favourites bar is a real row of buttons, the
//! "adaptive" layout options drive row/icon sizing and the grid layout.
//!
//! Everything is persisted as JSON in `~/.config/riss-launcher/settings.json`
//! (or next to `history.json`), unknown/removed keys fall back to their default
//! so old configuration files keep working.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Directory holding `history.json` and `settings.json`.
pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        PathBuf::from(".")
    }

    #[cfg(not(target_os = "android"))]
    {
        let mut path = if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
        } else {
            PathBuf::from(".")
        };
        path.push(".config");
        path.push("riss-launcher");
        fs::create_dir_all(&path).ok();
        path
    }
}

/// Where a setting lives in the settings screen (KISS' preference tree).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingsSection {
    Touch,
    Interface,
    Results,
    SearchBar,
    Favorites,
    History,
    Tags,
    Providers,
    Excluded,
    Backup,
    Advanced,
}

impl SettingsSection {
    pub const ALL: [SettingsSection; 11] = [
        SettingsSection::Touch,
        SettingsSection::Interface,
        SettingsSection::Results,
        SettingsSection::SearchBar,
        SettingsSection::Favorites,
        SettingsSection::History,
        SettingsSection::Tags,
        SettingsSection::Providers,
        SettingsSection::Excluded,
        SettingsSection::Backup,
        SettingsSection::Advanced,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            SettingsSection::Touch => "Touch & buttons",
            SettingsSection::Interface => "Interface",
            SettingsSection::Results => "Results",
            SettingsSection::SearchBar => "Search bar",
            SettingsSection::Favorites => "Favorites",
            SettingsSection::History => "History",
            SettingsSection::Tags => "Tags",
            SettingsSection::Providers => "Search providers",
            SettingsSection::Excluded => "Excluded apps",
            SettingsSection::Backup => "Import / export",
            SettingsSection::Advanced => "Advanced",
        }
    }

    pub fn summary(&self) -> &'static str {
        match self {
            SettingsSection::Touch => "Tap, long press and gestures",
            SettingsSection::Interface => "Theme, colors and window",
            SettingsSection::Results => "Sizing and content of the list",
            SettingsSection::SearchBar => "Where and how the query is edited",
            SettingsSection::Favorites => "Favourites bar and pinning",
            SettingsSection::History => "Ranking and usage data",
            SettingsSection::Tags => "Tag display and filtering",
            SettingsSection::Providers => "What can be typed in the search bar",
            SettingsSection::Excluded => "Apps hidden from the launcher",
            SettingsSection::Backup => "Save and restore your data",
            SettingsSection::Advanced => "Rarely needed knobs",
        }
    }
}

/// Colour scheme, mirroring KISS' themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeKind {
    Dark,
    Light,
    Solarized,
    System,
}

impl ThemeKind {
    pub const ALL: [ThemeKind; 4] = [
        ThemeKind::Dark,
        ThemeKind::Light,
        ThemeKind::Solarized,
        ThemeKind::System,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ThemeKind::Dark => "Dark",
            ThemeKind::Light => "Light",
            ThemeKind::Solarized => "Solarized",
            ThemeKind::System => "Follow system",
        }
    }
}

/// KISS' "night mode" switch: force light, force dark, or follow the OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NightMode {
    Auto,
    Day,
    Night,
}

impl NightMode {
    pub const ALL: [NightMode; 3] = [NightMode::Auto, NightMode::Day, NightMode::Night];

    pub fn label(&self) -> &'static str {
        match self {
            NightMode::Auto => "Automatic",
            NightMode::Day => "Always light",
            NightMode::Night => "Always dark",
        }
    }
}

/// Row height preset (`results-size` in KISS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResultSize {
    Small,
    Medium,
    Large,
    XLarge,
}

impl ResultSize {
    pub const ALL: [ResultSize; 4] = [
        ResultSize::Small,
        ResultSize::Medium,
        ResultSize::Large,
        ResultSize::XLarge,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ResultSize::Small => "Small",
            ResultSize::Medium => "Medium",
            ResultSize::Large => "Large",
            ResultSize::XLarge => "Extra large",
        }
    }

    /// Base row height in points, before adaptive tweaks.
    pub fn base_row_height(&self) -> f32 {
        match self {
            ResultSize::Small => 36.0,
            ResultSize::Medium => 46.0,
            ResultSize::Large => 58.0,
            ResultSize::XLarge => 72.0,
        }
    }

    /// Base icon size in points, before adaptive tweaks.
    pub fn base_icon_size(&self) -> f32 {
        match self {
            ResultSize::Small => 22.0,
            ResultSize::Medium => 28.0,
            ResultSize::Large => 36.0,
            ResultSize::XLarge => 44.0,
        }
    }
}

/// Vertical placement of the info bar, favourites bar and search bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarPosition {
    Top,
    Bottom,
}

impl BarPosition {
    pub const ALL: [BarPosition; 2] = [BarPosition::Top, BarPosition::Bottom];

    pub fn label(&self) -> &'static str {
        match self {
            BarPosition::Top => "Top",
            BarPosition::Bottom => "Bottom",
        }
    }
}

/// Search bar placement (`Bottom`, `Middle`, `Hidden`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchBarPosition {
    Top,
    Bottom,
    Middle,
    Hidden,
}

impl SearchBarPosition {
    pub const ALL: [SearchBarPosition; 4] = [
        SearchBarPosition::Top,
        SearchBarPosition::Bottom,
        SearchBarPosition::Middle,
        SearchBarPosition::Hidden,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            SearchBarPosition::Top => "Top",
            SearchBarPosition::Bottom => "Bottom",
            SearchBarPosition::Middle => "Middle",
            SearchBarPosition::Hidden => "Hidden",
        }
    }
}

/// Horizontal alignment of the grid / row numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgePosition {
    Start,
    End,
}

impl EdgePosition {
    pub const ALL: [EdgePosition; 2] = [EdgePosition::Start, EdgePosition::End];

    pub fn label(&self) -> &'static str {
        match self {
            EdgePosition::Start => "Start",
            EdgePosition::End => "End",
        }
    }
}

/// How the default (empty query) list is ranked (`history-mode` in KISS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryMode {
    Recency,
    UsageCount,
    Frecent,
}

impl HistoryMode {
    pub const ALL: [HistoryMode; 3] = [
        HistoryMode::Recency,
        HistoryMode::UsageCount,
        HistoryMode::Frecent,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            HistoryMode::Recency => "Most recent first",
            HistoryMode::UsageCount => "Most used first",
            HistoryMode::Frecent => "Frequent and recent",
        }
    }
}

/// Sort order of results when a tag filter is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TagSort {
    Relevance,
    Alphabetical,
    Recency,
    UsageCount,
}

impl TagSort {
    pub const ALL: [TagSort; 4] = [
        TagSort::Relevance,
        TagSort::Alphabetical,
        TagSort::Recency,
        TagSort::UsageCount,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            TagSort::Relevance => "Relevance",
            TagSort::Alphabetical => "Alphabetical",
            TagSort::Recency => "Most recent",
            TagSort::UsageCount => "Most used",
        }
    }
}

/// KISS gestures, mapped onto keyboard shortcuts on the desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GestureAction {
    None,
    ShowHistory,
    ShowAllApps,
    ShowSettings,
    ShowExcluded,
    ShowTagsMenu,
    ShowSearchBar,
    HideWindow,
    LaunchSelected,
    ClearQuery,
    Relaunch,
}

impl GestureAction {
    pub const ALL: [GestureAction; 11] = [
        GestureAction::None,
        GestureAction::ShowHistory,
        GestureAction::ShowAllApps,
        GestureAction::ShowSettings,
        GestureAction::ShowExcluded,
        GestureAction::ShowTagsMenu,
        GestureAction::ShowSearchBar,
        GestureAction::HideWindow,
        GestureAction::LaunchSelected,
        GestureAction::ClearQuery,
        GestureAction::Relaunch,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            GestureAction::None => "Do nothing",
            GestureAction::ShowHistory => "Show history",
            GestureAction::ShowAllApps => "Show all apps",
            GestureAction::ShowSettings => "Show settings",
            GestureAction::ShowExcluded => "Show excluded apps",
            GestureAction::ShowTagsMenu => "Toggle tags menu",
            GestureAction::ShowSearchBar => "Toggle search bar",
            GestureAction::HideWindow => "Hide launcher",
            GestureAction::LaunchSelected => "Launch selected result",
            GestureAction::ClearQuery => "Clear query",
            GestureAction::Relaunch => "Relaunch apps list",
        }
    }

    pub fn hint(&self) -> &'static str {
        match self {
            GestureAction::None => "disabled",
            GestureAction::ShowHistory => "history view",
            GestureAction::ShowAllApps => "all apps view",
            GestureAction::ShowSettings => "settings",
            GestureAction::ShowExcluded => "excluded apps",
            GestureAction::ShowTagsMenu => "tag filters",
            GestureAction::ShowSearchBar => "query bar",
            GestureAction::HideWindow => "hide window",
            GestureAction::LaunchSelected => "launch selection",
            GestureAction::ClearQuery => "empty query",
            GestureAction::Relaunch => "rescan",
        }
    }
}

/// A user defined web search provider (`url` may contain `{}` or `%s`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchProvider {
    pub name: String,
    pub url: String,
}

impl Default for SearchProvider {
    fn default() -> Self {
        Self {
            name: String::new(),
            url: String::new(),
        }
    }
}

/// All launcher settings. `#[serde(default)]` keeps old files loadable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // --- Touch, buttons and gestures -------------------------------------
    /// Long press (or right click) on a result opens the context menu.
    pub long_press_menu: bool,
    /// How long a press must be held to open the context menu.
    pub long_press_delay_ms: u32,
    /// Require a second tap on the same result before it is launched.
    pub prevent_fast_launch: bool,
    /// Window in which the "press again to launch" hint stays valid.
    pub tap_timeout_ms: u32,
    /// A single tap on a favourites bar item launches it (otherwise: select).
    pub single_tap_favorite_launches: bool,
    /// Launch the nth result by pressing its number (query must be empty).
    pub number_keys_to_launch: bool,
    /// Animate rows while they are pressed.
    pub press_feedback: bool,
    /// Double click launches, single click only selects.
    pub double_click_launches: bool,
    /// Hide the window after launching an app.
    pub hide_on_launch: bool,
    /// Minimum touch target height in points.
    pub min_touch_height: f32,
    pub gesture_up: GestureAction,
    pub gesture_down: GestureAction,
    pub gesture_left: GestureAction,
    pub gesture_right: GestureAction,
    pub gesture_long_press: GestureAction,

    // --- Interface --------------------------------------------------------
    pub theme: ThemeKind,
    pub night_mode: NightMode,
    pub font_scale: f32,
    pub widget_spacing: f32,
    pub rounded_list: bool,
    pub rounded_bars: bool,
    pub large_result_list_margins: bool,
    pub show_separators: bool,
    pub primary_color: [u8; 4],
    pub transparent_search_bar: bool,
    pub fullscreen: bool,
    pub window_width: f32,
    pub window_height: f32,
    pub force_portrait: bool,

    // --- Results ----------------------------------------------------------
    pub result_size: ResultSize,
    pub adaptive_results: bool,
    pub adaptive_icon_size: bool,
    pub adaptive_columns: bool,
    pub max_columns: usize,
    pub show_app_names: bool,
    pub show_subicons: bool,
    pub hide_main_icons: bool,
    pub show_tags: bool,
    pub show_launch_count: bool,
    pub show_row_actions: bool,
    pub select_last_result: bool,
    pub grid_position: EdgePosition,

    // --- Search bar -------------------------------------------------------
    pub search_bar_position: SearchBarPosition,
    pub hide_search_bar_on_start: bool,
    pub large_search_bar: bool,
    pub hide_search_bar_hint: bool,
    pub show_keyboard_hints: bool,
    pub info_bar_position: BarPosition,
    pub swap_kiss_button_with_menu: bool,

    // --- Favorites --------------------------------------------------------
    pub favorites_bar: bool,
    pub large_favorites_bar: bool,
    pub transparent_favorites_bar: bool,
    pub favorites_bar_position: BarPosition,
    pub favorites_limit: usize,
    pub exclude_favorites_from_apps: bool,
    pub exclude_favorites_from_history: bool,
    pub favorites_tags: Vec<String>,

    // --- History ----------------------------------------------------------
    pub history_mode: HistoryMode,
    pub history_length: usize,
    pub freeze_history: bool,
    pub enable_app_history: bool,
    pub search_through_history: bool,

    // --- Tags -------------------------------------------------------------
    pub tags_visible: bool,
    pub tags_menu: bool,
    pub show_untagged: bool,
    pub tagged_sort: TagSort,

    // --- Providers --------------------------------------------------------
    pub provider_settings: bool,
    pub provider_excluded: bool,
    pub provider_web: bool,
    pub provider_exec: bool,
    pub provider_timer: bool,
    pub provider_calc: bool,
    pub default_web_provider: String,
    pub custom_web_providers: Vec<SearchProvider>,
    pub always_web_search_on_enter: bool,
    pub min_match_precision: u8,
    pub legacy_fuzzy: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // Touch
            long_press_menu: true,
            long_press_delay_ms: 500,
            prevent_fast_launch: false,
            tap_timeout_ms: 1500,
            single_tap_favorite_launches: true,
            number_keys_to_launch: false,
            press_feedback: true,
            double_click_launches: false,
            hide_on_launch: false,
            min_touch_height: 44.0,
            gesture_up: GestureAction::ShowSearchBar,
            gesture_down: GestureAction::HideWindow,
            gesture_left: GestureAction::ShowHistory,
            gesture_right: GestureAction::ShowAllApps,
            gesture_long_press: GestureAction::ShowSettings,

            // Interface
            theme: ThemeKind::Dark,
            night_mode: NightMode::Auto,
            font_scale: 1.0,
            widget_spacing: 2.0,
            rounded_list: true,
            rounded_bars: true,
            large_result_list_margins: false,
            show_separators: false,
            primary_color: [137, 180, 250, 255],
            transparent_search_bar: false,
            fullscreen: false,
            window_width: 420.0,
            window_height: 640.0,
            force_portrait: false,

            // Results
            result_size: ResultSize::Medium,
            adaptive_results: true,
            adaptive_icon_size: true,
            adaptive_columns: true,
            max_columns: 5,
            show_app_names: true,
            show_subicons: true,
            hide_main_icons: false,
            show_tags: true,
            show_launch_count: true,
            show_row_actions: true,
            select_last_result: true,
            grid_position: EdgePosition::Start,

            // Search bar
            search_bar_position: SearchBarPosition::Bottom,
            hide_search_bar_on_start: false,
            large_search_bar: false,
            hide_search_bar_hint: false,
            show_keyboard_hints: true,
            info_bar_position: BarPosition::Top,
            swap_kiss_button_with_menu: false,

            // Favorites
            favorites_bar: true,
            large_favorites_bar: false,
            transparent_favorites_bar: false,
            favorites_bar_position: BarPosition::Bottom,
            favorites_limit: 12,
            exclude_favorites_from_apps: false,
            exclude_favorites_from_history: false,
            favorites_tags: vec!["favorite".to_string()],

            // History
            history_mode: HistoryMode::Frecent,
            history_length: 20,
            freeze_history: false,
            enable_app_history: true,
            search_through_history: false,

            // Tags
            tags_visible: true,
            tags_menu: false,
            show_untagged: false,
            tagged_sort: TagSort::Relevance,

            // Providers
            provider_settings: true,
            provider_excluded: false,
            provider_web: true,
            provider_exec: true,
            provider_timer: true,
            provider_calc: true,
            default_web_provider: "DuckDuckGo".to_string(),
            custom_web_providers: Vec::new(),
            always_web_search_on_enter: false,
            min_match_precision: 4,
            legacy_fuzzy: false,
        }
    }
}

impl Settings {
    /// Path of `settings.json`.
    pub fn path() -> PathBuf {
        config_dir().join("settings.json")
    }

    pub fn load() -> Self {
        match fs::read_to_string(Self::path()) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        if let Ok(data) = serde_json::to_string_pretty(self) {
            fs::write(Self::path(), data).ok();
        }
    }

    /// Row height for the current result count (KISS' "adaptive" sizing).
    pub fn row_height(&self, count: usize) -> f32 {
        let mut height = self.result_size.base_row_height();
        if self.adaptive_results && count > 6 {
            // Tighten up as the list grows so more items stay on screen.
            let shrink = match count {
                0..=6 => 0.0,
                7..=10 => 0.88,
                11..=15 => 0.8,
                16..=25 => 0.72,
                _ => 0.66,
            };
            height *= shrink;
        }
        (height + self.widget_spacing).max(self.min_touch_height)
    }

    /// Icon size for the current result count.
    pub fn icon_size(&self, count: usize) -> f32 {
        let mut size = self.result_size.base_icon_size();
        if self.adaptive_icon_size {
            match count {
                0 => size *= 1.35,
                1 => size *= 1.7,
                2 => size *= 1.45,
                3 => size *= 1.25,
                4..=5 => size *= 1.1,
                6..=9 => size *= 1.0,
                10..=15 => size *= 0.9,
                _ => size *= 0.82,
            }
        }
        (size * self.font_scale).min(self.result_size.base_row_height())
    }

    /// Number of columns of the grid layout.
    pub fn columns(&self, available_width: f32, count: usize, icon_size: f32) -> usize {
        if !self.adaptive_columns || icon_size <= 0.0 {
            return 1;
        }
        let cell = icon_size * 3.0;
        let columns = (available_width / cell).floor() as usize;
        let columns = if columns == 0 { 1 } else { columns };
        // Never show a grid with a lonely last line when there are few items.
        let max_columns = self.max_columns.clamp(1, 8);
        let columns = columns.min(max_columns);
        if count == 0 || columns <= 1 {
            return columns;
        }
        // A single lonely row in a wide grid looks broken: shrink to the items.
        let rows = (count + columns - 1) / columns;
        if rows <= 1 {
            columns.min(count)
        } else {
            columns
        }
    }

    /// Use the grid (icon) layout instead of the list layout.
    pub fn use_grid(&self, count: usize, available_width: f32) -> bool {
        if !self.adaptive_columns || count < 2 {
            return false;
        }
        let icon = self.icon_size(count);
        let columns = self.columns(available_width, count, icon);
        columns > 1 && !self.large_result_list_margins
    }

    pub fn all_providers(&self) -> Vec<SearchProvider> {
        let mut providers = crate::providers::builtin_web_providers()
            .into_iter()
            .map(|(name, url)| SearchProvider {
                name: name.to_string(),
                url: url.to_string(),
            })
            .collect::<Vec<_>>();
        providers.extend(self.custom_web_providers.iter().cloned());
        providers
    }

    pub fn provider_by_name(&self, name: &str) -> Option<SearchProvider> {
        self.all_providers()
            .into_iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Does `value` contain `query` (case insensitive)? Used by the settings
    /// search box and by the `Settings …` search provider.
    pub fn matches_filter(haystack: &str, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        haystack.to_lowercase().contains(&needle.to_lowercase())
    }

    /// True when the effective palette should be dark.
    pub fn prefers_dark(&self, system_dark: bool) -> bool {
        match self.night_mode {
            NightMode::Auto => match self.theme {
                ThemeKind::Dark | ThemeKind::Solarized => true,
                ThemeKind::Light => false,
                ThemeKind::System => system_dark,
            },
            NightMode::Day => false,
            NightMode::Night => true,
        }
    }
}

/// One row of the settings catalog. Used by the `Settings …` search provider
/// and by the filter box of the settings screen.
pub struct SettingEntry {
    pub id: &'static str,
    pub section: SettingsSection,
    pub title: &'static str,
    pub keywords: &'static str,
    pub value: String,
}

fn entries() -> Vec<(&'static str, SettingsSection, &'static str, &'static str)> {
    vec![
        (
            "long_press_menu",
            SettingsSection::Touch,
            "Long press opens the menu",
            "context menu tap hold right click",
        ),
        (
            "long_press_delay_ms",
            SettingsSection::Touch,
            "Long press duration",
            "delay milliseconds hold",
        ),
        (
            "prevent_fast_launch",
            SettingsSection::Touch,
            "Prevent fast launch",
            "accidental double tap confirm",
        ),
        (
            "tap_timeout_ms",
            SettingsSection::Touch,
            "Tap timeout",
            "milliseconds press again",
        ),
        (
            "single_tap_favorite_launches",
            SettingsSection::Touch,
            "Single tap launches favorites",
            "favourites bar tap",
        ),
        (
            "number_keys_to_launch",
            SettingsSection::Touch,
            "Number keys launch results",
            "shortcut digit",
        ),
        (
            "press_feedback",
            SettingsSection::Touch,
            "Press feedback",
            "animation ripple highlight",
        ),
        (
            "double_click_launches",
            SettingsSection::Touch,
            "Double click launches",
            "double tap select",
        ),
        (
            "hide_on_launch",
            SettingsSection::Touch,
            "Hide after launching",
            "minimize close window",
        ),
        (
            "min_touch_height",
            SettingsSection::Touch,
            "Minimum touch height",
            "row size accessibility",
        ),
        (
            "gesture_up",
            SettingsSection::Touch,
            "Gesture: up",
            "keyboard shortcut ctrl alt",
        ),
        (
            "gesture_down",
            SettingsSection::Touch,
            "Gesture: down",
            "keyboard shortcut ctrl alt",
        ),
        (
            "gesture_left",
            SettingsSection::Touch,
            "Gesture: left",
            "keyboard shortcut ctrl alt",
        ),
        (
            "gesture_right",
            SettingsSection::Touch,
            "Gesture: right",
            "keyboard shortcut ctrl alt",
        ),
        (
            "gesture_long_press",
            SettingsSection::Touch,
            "Gesture: long press",
            "keyboard shortcut ctrl alt",
        ),
        (
            "theme",
            SettingsSection::Interface,
            "Theme",
            "dark light solarized colors",
        ),
        (
            "night_mode",
            SettingsSection::Interface,
            "Night mode",
            "automatic day night system",
        ),
        (
            "font_scale",
            SettingsSection::Interface,
            "Font scale",
            "text size zoom",
        ),
        (
            "widget_spacing",
            SettingsSection::Interface,
            "Widget spacing",
            "margins padding",
        ),
        (
            "rounded_list",
            SettingsSection::Interface,
            "Rounded results",
            "corners bars",
        ),
        (
            "rounded_bars",
            SettingsSection::Interface,
            "Rounded bars",
            "corners",
        ),
        (
            "large_result_list_margins",
            SettingsSection::Interface,
            "Large result margins",
            "spacing",
        ),
        (
            "show_separators",
            SettingsSection::Interface,
            "Show separators",
            "divider lines",
        ),
        (
            "primary_color",
            SettingsSection::Interface,
            "Main color",
            "accent highlight",
        ),
        (
            "transparent_search_bar",
            SettingsSection::Interface,
            "Transparent search bar",
            "background",
        ),
        (
            "fullscreen",
            SettingsSection::Interface,
            "Fullscreen",
            "maximize kiosk",
        ),
        (
            "window_width",
            SettingsSection::Interface,
            "Window width",
            "size resize",
        ),
        (
            "window_height",
            SettingsSection::Interface,
            "Window height",
            "size resize",
        ),
        (
            "force_portrait",
            SettingsSection::Interface,
            "Force portrait",
            "tall aspect ratio",
        ),
        (
            "result_size",
            SettingsSection::Results,
            "Results size",
            "small medium large rows",
        ),
        (
            "adaptive_results",
            SettingsSection::Results,
            "Adaptive results",
            "shrink rows count",
        ),
        (
            "adaptive_icon_size",
            SettingsSection::Results,
            "Adaptive icon size",
            "scale icons",
        ),
        (
            "adaptive_columns",
            SettingsSection::Results,
            "Adaptive columns",
            "grid layout",
        ),
        (
            "max_columns",
            SettingsSection::Results,
            "Maximum columns",
            "grid landscape",
        ),
        (
            "show_app_names",
            SettingsSection::Results,
            "Display app names",
            "titles text",
        ),
        (
            "show_subicons",
            SettingsSection::Results,
            "Display sub icons",
            "comments categories",
        ),
        (
            "hide_main_icons",
            SettingsSection::Results,
            "Hide main icons",
            "text only",
        ),
        (
            "show_tags",
            SettingsSection::Results,
            "Display tags",
            "hashtags",
        ),
        (
            "show_launch_count",
            SettingsSection::Results,
            "Display launch counters",
            "usage",
        ),
        (
            "show_row_actions",
            SettingsSection::Results,
            "Row action buttons",
            "star tag buttons",
        ),
        (
            "select_last_result",
            SettingsSection::Results,
            "Select the last result",
            "enter nearest",
        ),
        (
            "grid_position",
            SettingsSection::Results,
            "Numbers on the",
            "start end left right",
        ),
        (
            "search_bar_position",
            SettingsSection::SearchBar,
            "Search bar position",
            "top bottom middle hidden",
        ),
        (
            "hide_search_bar_on_start",
            SettingsSection::SearchBar,
            "Hide search bar on start",
            "minimalistic",
        ),
        (
            "large_search_bar",
            SettingsSection::SearchBar,
            "Large search bar",
            "bigger input",
        ),
        (
            "hide_search_bar_hint",
            SettingsSection::SearchBar,
            "Hide search bar hint",
            "placeholder",
        ),
        (
            "show_keyboard_hints",
            SettingsSection::SearchBar,
            "Show keyboard hints",
            "shortcut legend",
        ),
        (
            "info_bar_position",
            SettingsSection::SearchBar,
            "Info bar position",
            "top bottom",
        ),
        (
            "swap_kiss_button_with_menu",
            SettingsSection::SearchBar,
            "Swap launcher and menu buttons",
            "kiss button",
        ),
        (
            "favorites_bar",
            SettingsSection::Favorites,
            "Favorites bar",
            "quick access",
        ),
        (
            "large_favorites_bar",
            SettingsSection::Favorites,
            "Large favorites bar",
            "bigger",
        ),
        (
            "transparent_favorites_bar",
            SettingsSection::Favorites,
            "Transparent favorites bar",
            "background",
        ),
        (
            "favorites_bar_position",
            SettingsSection::Favorites,
            "Favorites bar position",
            "top bottom",
        ),
        (
            "favorites_limit",
            SettingsSection::Favorites,
            "Favorites bar capacity",
            "limit",
        ),
        (
            "exclude_favorites_from_apps",
            SettingsSection::Favorites,
            "Exclude favorites from apps",
            "filter",
        ),
        (
            "exclude_favorites_from_history",
            SettingsSection::Favorites,
            "Exclude favorites from history",
            "filter",
        ),
        (
            "favorites_tags",
            SettingsSection::Favorites,
            "Favorites tags",
            "default tags list",
        ),
        (
            "history_mode",
            SettingsSection::History,
            "History mode",
            "recency usage frecent",
        ),
        (
            "history_length",
            SettingsSection::History,
            "Number of displayed items",
            "limit results",
        ),
        (
            "freeze_history",
            SettingsSection::History,
            "Freeze history",
            "do not record launches",
        ),
        (
            "enable_app_history",
            SettingsSection::History,
            "Track app history",
            "usage statistics",
        ),
        (
            "search_through_history",
            SettingsSection::History,
            "Search through history",
            "previous searches",
        ),
        (
            "tags_visible",
            SettingsSection::Tags,
            "Tags are visible",
            "hashtags",
        ),
        (
            "tags_menu",
            SettingsSection::Tags,
            "Tags menu",
            "filter chips",
        ),
        (
            "show_untagged",
            SettingsSection::Tags,
            "Show untagged",
            "untagged results",
        ),
        (
            "tagged_sort",
            SettingsSection::Tags,
            "Tagged results sort mode",
            "relevance alphabetical",
        ),
        (
            "provider_settings",
            SettingsSection::Providers,
            "Enable settings search",
            "search settings",
        ),
        (
            "provider_excluded",
            SettingsSection::Providers,
            "Enable excluded apps search",
            "hidden apps",
        ),
        (
            "provider_web",
            SettingsSection::Providers,
            "Enable web search",
            "google duckduckgo",
        ),
        (
            "provider_exec",
            SettingsSection::Providers,
            "Enable command execution",
            "shell exec",
        ),
        (
            "provider_timer",
            SettingsSection::Providers,
            "Enable timer",
            "sleep alarm countdown",
        ),
        (
            "provider_calc",
            SettingsSection::Providers,
            "Enable calculator",
            "math",
        ),
        (
            "default_web_provider",
            SettingsSection::Providers,
            "Default search provider",
            "google duckduckgo",
        ),
        (
            "custom_web_providers",
            SettingsSection::Providers,
            "Custom search providers",
            "add delete url",
        ),
        (
            "always_web_search_on_enter",
            SettingsSection::Providers,
            "Always default web search on enter",
            "search",
        ),
        (
            "min_match_precision",
            SettingsSection::Providers,
            "Min match precision",
            "fuzzy relevance",
        ),
        (
            "legacy_fuzzy",
            SettingsSection::Providers,
            "Use legacy fuzzy search",
            "algorithm",
        ),
    ]
}

/// The settings catalog, with the current value of every entry.
pub fn catalog(settings: &Settings) -> Vec<SettingEntry> {
    let yes_no = |v: bool| if v { "on" } else { "off" };
    entries()
        .into_iter()
        .map(|(id, section, title, keywords)| {
            let value: &'static str = match id {
                "long_press_menu" => yes_no(settings.long_press_menu),
                "long_press_delay_ms" => leak_u32(settings.long_press_delay_ms),
                "prevent_fast_launch" => yes_no(settings.prevent_fast_launch),
                "tap_timeout_ms" => leak_u32(settings.tap_timeout_ms),
                "single_tap_favorite_launches" => yes_no(settings.single_tap_favorite_launches),
                "number_keys_to_launch" => yes_no(settings.number_keys_to_launch),
                "press_feedback" => yes_no(settings.press_feedback),
                "double_click_launches" => yes_no(settings.double_click_launches),
                "hide_on_launch" => yes_no(settings.hide_on_launch),
                "min_touch_height" => leak_u32(settings.min_touch_height as u32),
                "gesture_up" => settings.gesture_up.hint(),
                "gesture_down" => settings.gesture_down.hint(),
                "gesture_left" => settings.gesture_left.hint(),
                "gesture_right" => settings.gesture_right.hint(),
                "gesture_long_press" => settings.gesture_long_press.hint(),
                "theme" => settings.theme.label(),
                "night_mode" => settings.night_mode.label(),
                "font_scale" => leak_f32(settings.font_scale),
                "widget_spacing" => leak_f32(settings.widget_spacing),
                "rounded_list" => yes_no(settings.rounded_list),
                "rounded_bars" => yes_no(settings.rounded_bars),
                "large_result_list_margins" => yes_no(settings.large_result_list_margins),
                "show_separators" => yes_no(settings.show_separators),
                "primary_color" => "color",
                "transparent_search_bar" => yes_no(settings.transparent_search_bar),
                "fullscreen" => yes_no(settings.fullscreen),
                "window_width" => leak_f32(settings.window_width),
                "window_height" => leak_f32(settings.window_height),
                "force_portrait" => yes_no(settings.force_portrait),
                "result_size" => settings.result_size.label(),
                "adaptive_results" => yes_no(settings.adaptive_results),
                "adaptive_icon_size" => yes_no(settings.adaptive_icon_size),
                "adaptive_columns" => yes_no(settings.adaptive_columns),
                "max_columns" => leak_u32(settings.max_columns as u32),
                "show_app_names" => yes_no(settings.show_app_names),
                "show_subicons" => yes_no(settings.show_subicons),
                "hide_main_icons" => yes_no(settings.hide_main_icons),
                "show_tags" => yes_no(settings.show_tags),
                "show_launch_count" => yes_no(settings.show_launch_count),
                "show_row_actions" => yes_no(settings.show_row_actions),
                "select_last_result" => yes_no(settings.select_last_result),
                "grid_position" => settings.grid_position.label(),
                "search_bar_position" => settings.search_bar_position.label(),
                "hide_search_bar_on_start" => yes_no(settings.hide_search_bar_on_start),
                "large_search_bar" => yes_no(settings.large_search_bar),
                "hide_search_bar_hint" => yes_no(settings.hide_search_bar_hint),
                "show_keyboard_hints" => yes_no(settings.show_keyboard_hints),
                "info_bar_position" => settings.info_bar_position.label(),
                "swap_kiss_button_with_menu" => yes_no(settings.swap_kiss_button_with_menu),
                "favorites_bar" => yes_no(settings.favorites_bar),
                "large_favorites_bar" => yes_no(settings.large_favorites_bar),
                "transparent_favorites_bar" => yes_no(settings.transparent_favorites_bar),
                "favorites_bar_position" => settings.favorites_bar_position.label(),
                "favorites_limit" => leak_u32(settings.favorites_limit as u32),
                "exclude_favorites_from_apps" => yes_no(settings.exclude_favorites_from_apps),
                "exclude_favorites_from_history" => yes_no(settings.exclude_favorites_from_history),
                "favorites_tags" => "list",
                "history_mode" => settings.history_mode.label(),
                "history_length" => leak_u32(settings.history_length as u32),
                "freeze_history" => yes_no(settings.freeze_history),
                "enable_app_history" => yes_no(settings.enable_app_history),
                "search_through_history" => yes_no(settings.search_through_history),
                "tags_visible" => yes_no(settings.tags_visible),
                "tags_menu" => yes_no(settings.tags_menu),
                "show_untagged" => yes_no(settings.show_untagged),
                "tagged_sort" => settings.tagged_sort.label(),
                "provider_settings" => yes_no(settings.provider_settings),
                "provider_excluded" => yes_no(settings.provider_excluded),
                "provider_web" => yes_no(settings.provider_web),
                "provider_exec" => yes_no(settings.provider_exec),
                "provider_timer" => yes_no(settings.provider_timer),
                "provider_calc" => yes_no(settings.provider_calc),
                "default_web_provider" => "provider",
                "custom_web_providers" => "list",
                "always_web_search_on_enter" => yes_no(settings.always_web_search_on_enter),
                "min_match_precision" => leak_u32(settings.min_match_precision as u32),
                "legacy_fuzzy" => yes_no(settings.legacy_fuzzy),
                _ => "",
            };
            SettingEntry {
                id,
                section,
                title,
                keywords,
                value: value.to_string(),
            }
        })
        .collect()
}

/// Small helper so `catalog()` can hand out a formatted value.
fn leak_u32(value: u32) -> &'static str {
    leak_string(&value.to_string())
}

fn leak_f32(value: f32) -> &'static str {
    leak_string(&format!("{:.2}", value))
}

fn leak_string(value: &str) -> &'static str {
    Box::leak(value.to_string().into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip() {
        let settings = Settings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(settings, parsed);
    }

    #[test]
    fn partial_file_uses_defaults() {
        let parsed: Settings = serde_json::from_str("{\"long_press_menu\": false}").unwrap();
        assert!(!parsed.long_press_menu);
        assert_eq!(
            parsed.long_press_delay_ms,
            Settings::default().long_press_delay_ms
        );
    }

    #[test]
    fn row_height_never_below_touch_target() {
        let mut settings = Settings::default();
        settings.min_touch_height = 44.0;
        assert!(settings.row_height(50) >= 44.0);
    }

    #[test]
    fn catalog_matches_filter_case_insensitive() {
        let settings = Settings::default();
        let catalog = catalog(&settings);
        assert!(catalog.iter().any(|e| e.id == "theme"));
        assert!(Settings::matches_filter("Display app names", "APP N"));
        assert!(!Settings::matches_filter("Display app names", "zzz"));
    }
}
