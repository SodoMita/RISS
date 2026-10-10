use crate::storage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Persistent launcher preferences. Keys intentionally mirror KISS so exported
/// configurations and the settings screen remain familiar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsData {
    #[serde(default = "default_bools")]
    pub bools: HashMap<String, bool>,
    #[serde(default = "default_values")]
    pub values: HashMap<String, String>,
}

impl Default for SettingsData {
    fn default() -> Self {
        Self {
            bools: default_bools(),
            values: default_values(),
        }
    }
}

impl SettingsData {
    /// Load settings, returning the data plus human-readable notes about any
    /// storage problems that were recovered from.
    pub fn load() -> (Self, Vec<String>) {
        let (mut loaded, notes) = storage::load::<Self>(storage::SETTINGS_FILE)
            .into_value(storage::SETTINGS_FILE, Self::default());
        merge_defaults(&mut loaded);
        (loaded, notes)
    }

    pub fn save(&self) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        storage::save(storage::SETTINGS_FILE, &json)
    }

    pub fn enabled(&self, key: &str) -> bool {
        self.bools.get(key).copied().unwrap_or(false)
    }
    pub fn value(&self, key: &str) -> &str {
        self.values.get(key).map(String::as_str).unwrap_or("")
    }
    pub fn number(&self, key: &str, fallback: usize) -> usize {
        self.value(key).parse().unwrap_or(fallback)
    }
    pub fn reset(&mut self) -> std::io::Result<()> {
        *self = Self::default();
        self.save()
    }
}

/// Fill in any keys a saved file does not mention. serde defaults only apply
/// when the whole map is absent, so newly-added keys must be merged in by hand.
pub fn merge_defaults(data: &mut SettingsData) {
    for (key, value) in default_bools() {
        data.bools.entry(key).or_insert(value);
    }
    for (key, value) in default_values() {
        data.values.entry(key).or_insert(value);
    }
}

fn default_bools() -> HashMap<String, bool> {
    [
        ("freeze-history", false),
        ("enable-phone-history", false),
        ("enable-app-history", true),
        ("enable-notification-history", false),
        ("exclude-favorites-apps", false),
        ("exclude-favorites-history", true),
        ("enable-favorites-bar", true),
        ("large-favorites-bar", false),
        ("transparent-favorites", true),
        ("transparent-search", false),
        ("black-notification-icons", false),
        ("force-adaptive", true),
        ("force-shape", false),
        ("contact-pack-mask", true),
        ("themed-icons", false),
        ("subicon-visible", true),
        ("icons-hide", false),
        ("pref-rounded-list", false),
        ("pref-rounded-bars", true),
        ("pref-swap-kiss-button-with-menu", false),
        ("large-result-list-margins", false),
        ("display-keyboard", false),
        ("enable-suggestions-keyboard", false),
        ("hide-keyboard", false),
        ("history-hide", false),
        ("history-onclick", false),
        ("history-onkeyboard", false),
        ("favorites-hide", false),
        ("pref-hide-navbar", false),
        ("pref-hide-statusbar", false),
        ("pref-hide-circle", false),
        ("double-tap", false),
        ("tags-visible", true),
        ("pref-fav-tags-drawable", false),
        ("pref-tags-menu", false),
        ("pref-show-untagged", false),
        ("pref-tags-menu-dismiss", false),
        ("force-portrait", false),
        ("call-contact-on-click", false),
        ("lwp-touch", true),
        ("lwp-drag", false),
        ("wp-drag-animate", false),
        ("wp-animate-center", true),
        ("wp-animate-sides", false),
        ("enable-contacts", true),
        ("enable-settings", true),
        ("enable-timer", true),
        ("enable-shortcuts", true),
        ("enable-search", true),
        ("enable-excluded-apps", false),
        ("always-default-web-search-on-enter", false),
        ("large-search-bar", false),
        ("pref-hide-search-bar-hint", false),
        ("enable-notifications", true),
        ("use-fuzzy-score-v1", false),
        ("root-mode", false),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
}

fn default_values() -> HashMap<String, String> {
    [
        ("number-of-display-elements", "20"),
        ("history-mode", "recency"),
        ("pref-fav-tags-list", "favorites"),
        ("theme", "transparent"),
        ("night-mode", "system"),
        ("theme-shadow", "default"),
        ("theme-separator", "default"),
        ("theme-result-color", "default"),
        ("theme-wallpaper", "default"),
        ("theme-bar-color", "default"),
        ("results-size", "default"),
        ("pref-result-highlighting", "name,alias"),
        ("primary-color", "#89B4FA"),
        ("notification-bar-color", "#1E1E2E"),
        ("icons-pack", "system"),
        ("adaptive-shape", "0"),
        ("contacts-shape", "0"),
        ("gesture-up", "display-keyboard"),
        ("gesture-down", "display-notifications"),
        ("gesture-left", "display-apps"),
        ("gesture-right", "display-apps"),
        ("gesture-long-press", "display-apps"),
        ("gesture-up-launch-id", ""),
        ("gesture-down-launch-id", ""),
        ("gesture-left-launch-id", ""),
        ("gesture-right-launch-id", ""),
        ("gesture-long-press-launch-id", ""),
        ("pref-toggle-tags-list", "favorites"),
        ("tagged-result-sort-mode", "relevance"),
        ("default-search-provider", "duckduckgo"),
        (
            "selected-search-provider-names",
            "duckduckgo,google,wikipedia",
        ),
        ("deleting-search-providers-names", ""),
        ("custom-search-provider-add", ""),
        ("edit-excluded-apps", ""),
        ("edit-excluded-from-history-apps", ""),
        ("edit-excluded-app-shortcuts", ""),
        ("selected-contact-mime-types", "phone,email"),
        ("widget-spacing", "0"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect()
}

#[derive(Clone, Copy)]
pub enum SettingKind {
    Toggle,
    Number { min: usize, max: usize },
    Choice(&'static [&'static str]),
    Text,
    Action,
}

#[derive(Clone, Copy)]
pub struct SettingSpec {
    pub section: &'static str,
    pub key: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub kind: SettingKind,
}

const THEMES: &[&str] = &["transparent", "light", "dark"];
const NIGHT: &[&str] = &["system", "light", "dark"];
const DEFAULT: &[&str] = &["default", "show", "hide"];
const SIZE: &[&str] = &["small", "default", "large"];
const SHAPES: &[&str] = &["system", "circle", "square", "rounded", "squircle"];
const HISTORY: &[&str] = &["recency", "frequency", "frecent", "alphabetical"];
const SORT: &[&str] = &["relevance", "alphabetical", "recency", "frequency"];
const GESTURES: &[&str] = &[
    "do-nothing",
    "display-keyboard",
    "hide-keyboard",
    "display-apps",
    "display-history",
    "display-notifications",
    "display-quicksettings",
    "display-menu",
    "go-to-homescreen",
    "launch-pojo",
];
const SEARCH: &[&str] = &["duckduckgo", "google", "wikipedia", "brave", "bing"];

macro_rules! s {
    ($section:expr,$key:expr,$title:expr,$kind:expr) => {
        SettingSpec {
            section: $section,
            key: $key,
            title: $title,
            summary: "",
            kind: $kind,
        }
    };
}

/// Every preference exposed by KISS is represented here, plus RISS-specific
/// reset/refresh actions. Platform-only options remain configurable and are
/// applied whenever the platform supports them.
pub fn specs() -> Vec<SettingSpec> {
    use SettingKind::*;
    vec![
        s!("History", "reset-history", "Clear history", Action),
        s!(
            "History",
            "number-of-display-elements",
            "Number of displayed items",
            Number { min: 0, max: 150 }
        ),
        s!(
            "History",
            "history-mode",
            "History sorting",
            Choice(HISTORY)
        ),
        s!("History", "freeze-history", "Freeze history", Toggle),
        s!(
            "History",
            "enable-phone-history",
            "Include phone calls",
            Toggle
        ),
        s!(
            "History",
            "enable-app-history",
            "Include applications",
            Toggle
        ),
        s!(
            "History",
            "enable-notification-history",
            "Include notifications",
            Toggle
        ),
        s!("Favorites", "reset-favorites", "Clear favorites", Action),
        s!(
            "Favorites",
            "exclude-favorites-apps",
            "Hide favorites from app results",
            Toggle
        ),
        s!(
            "Favorites",
            "exclude-favorites-history",
            "Hide favorites from history",
            Toggle
        ),
        s!("Favorites", "pref-fav-tags-list", "Favorite tags", Text),
        s!(
            "Favorites",
            "enable-favorites-bar",
            "Show favorites bar",
            Toggle
        ),
        s!(
            "Favorites",
            "large-favorites-bar",
            "Large favorites bar",
            Toggle
        ),
        s!(
            "Favorites",
            "transparent-favorites",
            "Transparent favorites bar",
            Toggle
        ),
        s!("Appearance", "theme", "Theme", Choice(THEMES)),
        s!("Appearance", "night-mode", "Night mode", Choice(NIGHT)),
        s!(
            "Appearance",
            "theme-shadow",
            "Result shadows",
            Choice(DEFAULT)
        ),
        s!(
            "Appearance",
            "theme-separator",
            "Result separators",
            Choice(DEFAULT)
        ),
        s!(
            "Appearance",
            "theme-result-color",
            "Result color",
            Choice(DEFAULT)
        ),
        s!(
            "Appearance",
            "theme-wallpaper",
            "Wallpaper visibility",
            Choice(DEFAULT)
        ),
        s!(
            "Appearance",
            "theme-bar-color",
            "Search bar color",
            Choice(DEFAULT)
        ),
        s!(
            "Appearance",
            "transparent-search",
            "Transparent search bar",
            Toggle
        ),
        s!("Appearance", "results-size", "Result size", Choice(SIZE)),
        s!(
            "Appearance",
            "pref-result-highlighting",
            "Result highlighting",
            Text
        ),
        s!("Appearance", "primary-color", "Primary color (hex)", Text),
        s!(
            "Appearance",
            "notification-bar-color",
            "Status bar color (hex)",
            Text
        ),
        s!(
            "Appearance",
            "black-notification-icons",
            "Dark status bar icons",
            Toggle
        ),
        s!("Icons", "icons-pack", "Icon pack", Text),
        s!(
            "Icons",
            "adaptive-shape",
            "Adaptive icon shape",
            Choice(SHAPES)
        ),
        s!("Icons", "force-adaptive", "Use legacy icons", Toggle),
        s!("Icons", "force-shape", "Force icon shape", Toggle),
        s!(
            "Icons",
            "contact-pack-mask",
            "Use icon mask for contacts",
            Toggle
        ),
        s!(
            "Icons",
            "contacts-shape",
            "Contact icon shape",
            Choice(SHAPES)
        ),
        s!("Icons", "themed-icons", "Themed icons", Toggle),
        s!("Icons", "subicon-visible", "Show result sub-icons", Toggle),
        s!("Icons", "icons-hide", "Hide main icons", Toggle),
        s!("Layout", "pref-rounded-list", "Rounded results", Toggle),
        s!("Layout", "pref-rounded-bars", "Rounded bars", Toggle),
        s!(
            "Layout",
            "pref-swap-kiss-button-with-menu",
            "Swap apps and menu buttons",
            Toggle
        ),
        s!(
            "Layout",
            "large-result-list-margins",
            "Large result margins",
            Toggle
        ),
        s!(
            "Keyboard",
            "display-keyboard",
            "Show keyboard on start",
            Toggle
        ),
        s!(
            "Keyboard",
            "enable-suggestions-keyboard",
            "Allow keyboard suggestions",
            Toggle
        ),
        s!(
            "Keyboard",
            "hide-keyboard",
            "Hide keyboard after launch",
            Toggle
        ),
        s!("Minimal mode", "history-hide", "Hide history", Toggle),
        s!(
            "Minimal mode",
            "history-onclick",
            "Show history on tap",
            Toggle
        ),
        s!(
            "Minimal mode",
            "history-onkeyboard",
            "Show history with keyboard",
            Toggle
        ),
        s!("Minimal mode", "favorites-hide", "Hide favorites", Toggle),
        s!(
            "Minimal mode",
            "pref-hide-navbar",
            "Hide navigation bar",
            Toggle
        ),
        s!(
            "Minimal mode",
            "pref-hide-statusbar",
            "Hide status bar",
            Toggle
        ),
        s!(
            "Minimal mode",
            "pref-hide-circle",
            "Hide launcher circle",
            Toggle
        ),
        s!("Gestures", "gesture-up", "Swipe up", Choice(GESTURES)),
        s!(
            "Gestures",
            "gesture-up-launch-id",
            "Swipe up launch target",
            Text
        ),
        s!("Gestures", "gesture-down", "Swipe down", Choice(GESTURES)),
        s!(
            "Gestures",
            "gesture-down-launch-id",
            "Swipe down launch target",
            Text
        ),
        s!("Gestures", "gesture-left", "Swipe left", Choice(GESTURES)),
        s!(
            "Gestures",
            "gesture-left-launch-id",
            "Swipe left launch target",
            Text
        ),
        s!("Gestures", "gesture-right", "Swipe right", Choice(GESTURES)),
        s!(
            "Gestures",
            "gesture-right-launch-id",
            "Swipe right launch target",
            Text
        ),
        s!(
            "Gestures",
            "gesture-long-press",
            "Long press empty space",
            Choice(GESTURES)
        ),
        s!(
            "Gestures",
            "gesture-long-press-launch-id",
            "Long press launch target",
            Text
        ),
        s!("Gestures", "double-tap", "Double tap to lock", Toggle),
        s!("Tags", "tags-visible", "Show tags", Toggle),
        s!(
            "Tags",
            "pref-fav-tags-drawable",
            "Use tag icons in favorites",
            Toggle
        ),
        s!("Tags", "pref-tags-menu", "Enable tag menu", Toggle),
        s!("Tags", "pref-show-untagged", "Show untagged filter", Toggle),
        s!(
            "Tags",
            "pref-tags-menu-dismiss",
            "Close tag menu after selection",
            Toggle
        ),
        s!("Tags", "pref-toggle-tags-list", "Tags in menu", Text),
        s!(
            "Tags",
            "tagged-result-sort-mode",
            "Tagged result sorting",
            Choice(SORT)
        ),
        s!("Behavior", "force-portrait", "Force portrait", Toggle),
        s!(
            "Behavior",
            "call-contact-on-click",
            "Call contact on tap",
            Toggle
        ),
        s!(
            "Wallpaper",
            "lwp-touch",
            "Send touches to live wallpaper",
            Toggle
        ),
        s!(
            "Wallpaper",
            "lwp-drag",
            "Send drags to live wallpaper",
            Toggle
        ),
        s!(
            "Wallpaper",
            "wp-drag-animate",
            "Animate wallpaper while dragging",
            Toggle
        ),
        s!(
            "Wallpaper",
            "wp-animate-center",
            "Animate wallpaper to center",
            Toggle
        ),
        s!(
            "Wallpaper",
            "wp-animate-sides",
            "Animate wallpaper to sides",
            Toggle
        ),
        s!("Search providers", "enable-contacts", "Contacts", Toggle),
        s!(
            "Search providers",
            "enable-settings",
            "Device settings",
            Toggle
        ),
        s!("Search providers", "enable-timer", "Timers", Toggle),
        s!(
            "Search providers",
            "enable-shortcuts",
            "App shortcuts",
            Toggle
        ),
        s!(
            "Search providers",
            "reset-shortcuts",
            "Regenerate shortcuts",
            Action
        ),
        s!("Search providers", "enable-search", "Web search", Toggle),
        s!(
            "Search providers",
            "enable-excluded-apps",
            "Search excluded apps",
            Toggle
        ),
        s!(
            "Web search",
            "default-search-provider",
            "Default provider",
            Choice(SEARCH)
        ),
        s!(
            "Web search",
            "selected-search-provider-names",
            "Enabled providers",
            Text
        ),
        s!(
            "Web search",
            "deleting-search-providers-names",
            "Providers to delete",
            Text
        ),
        s!(
            "Web search",
            "custom-search-provider-add",
            "Add custom provider",
            Text
        ),
        s!(
            "Web search",
            "reset-search-providers",
            "Reset search providers",
            Action
        ),
        s!(
            "Web search",
            "always-default-web-search-on-enter",
            "Always web search on Enter",
            Toggle
        ),
        s!("Web search", "large-search-bar", "Large search bar", Toggle),
        s!(
            "Web search",
            "pref-hide-search-bar-hint",
            "Hide search hint",
            Toggle
        ),
        s!(
            "Excluded apps",
            "edit-excluded-apps",
            "Apps excluded from search",
            Text
        ),
        s!(
            "Excluded apps",
            "edit-excluded-from-history-apps",
            "Apps excluded from history",
            Text
        ),
        s!(
            "Excluded apps",
            "edit-excluded-app-shortcuts",
            "Apps excluded from shortcuts",
            Text
        ),
        s!(
            "Excluded apps",
            "reset-excluded-apps",
            "Reset search exclusions",
            Action
        ),
        s!(
            "Excluded apps",
            "reset-excluded-from-history-apps",
            "Reset history exclusions",
            Action
        ),
        s!(
            "Excluded apps",
            "reset-excluded-app-shortcuts",
            "Reset shortcut exclusions",
            Action
        ),
        s!(
            "Import & export",
            "export-settings",
            "Export settings",
            Action
        ),
        s!(
            "Import & export",
            "import-settings",
            "Import settings",
            Action
        ),
        s!(
            "Advanced",
            "enable-notifications",
            "Notification support",
            Toggle
        ),
        s!(
            "Advanced",
            "default-launcher",
            "Choose default launcher",
            Action
        ),
        s!(
            "Advanced",
            "selected-contact-mime-types",
            "Contact data types",
            Text
        ),
        s!(
            "Advanced",
            "use-fuzzy-score-v1",
            "Legacy fuzzy search",
            Toggle
        ),
        s!(
            "Advanced",
            "widget-spacing",
            "Widget spacing",
            Number { min: 0, max: 300 }
        ),
        s!("Advanced", "root-mode", "Root mode", Toggle),
        s!("Advanced", "restart", "Restart launcher", Action),
        s!("Advanced", "rate-app", "Rate RISS", Action),
        s!("Advanced", "reset-all", "Reset every setting", Action),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_non_action_spec_has_a_default() {
        let defaults = SettingsData::default();
        for spec in specs() {
            match spec.kind {
                SettingKind::Toggle => {
                    assert!(
                        defaults.bools.contains_key(spec.key),
                        "toggle spec '{}' has no default bool",
                        spec.key
                    );
                }
                SettingKind::Number { .. } | SettingKind::Choice(_) | SettingKind::Text => {
                    assert!(
                        defaults.values.contains_key(spec.key),
                        "value spec '{}' has no default value",
                        spec.key
                    );
                }
                SettingKind::Action => {}
            }
        }
    }

    #[test]
    fn merge_defaults_fills_missing_keys_but_keeps_overrides() {
        let mut data = SettingsData::default();
        // Simulate an old file that is missing newer keys and overrides one.
        data.bools.remove("freeze-history");
        data.values.insert("theme".to_owned(), "light".to_owned());
        data.values.remove("history-mode");
        merge_defaults(&mut data);
        assert_eq!(data.bools.get("freeze-history"), Some(&false));
        assert_eq!(data.values.get("theme"), Some(&"light".to_owned()));
        assert_eq!(data.values.get("history-mode"), Some(&"recency".to_owned()));
    }

    #[test]
    fn history_mode_choices_are_parseable() {
        use crate::history::HistoryMode;
        for option in HISTORY {
            // from_key never panics; recency is the documented fallback.
            let _ = HistoryMode::from_key(option);
        }
        assert_eq!(HistoryMode::from_key("frecent"), HistoryMode::Frecent);
    }

    #[test]
    fn serde_roundtrip_preserves_values() {
        let mut data = SettingsData::default();
        data.bools.insert("freeze-history".to_owned(), true);
        data.values.insert("theme".to_owned(), "dark".to_owned());
        let json = serde_json::to_string(&data).unwrap();
        let parsed: SettingsData = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.bools.get("freeze-history"), Some(&true));
        assert_eq!(parsed.values.get("theme"), Some(&"dark".to_owned()));
    }
}
