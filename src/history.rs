use crate::storage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Persistent history and favorites data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HistoryData {
    /// Map from app exec command to launch count
    pub launch_counts: HashMap<String, u32>,
    /// Map from app exec command to last launch timestamp
    pub last_launched: HashMap<String, u64>,
    /// Set of favorite app exec commands
    pub favorites: Vec<String>,
    /// Custom tags: exec -> tags
    pub tags: HashMap<String, Vec<String>>,
    /// Custom display names (KISS' "rename app"): exec -> alias
    pub aliases: HashMap<String, String>,
    /// Custom icon name or path: exec -> custom icon
    pub custom_icons: HashMap<String, String>,
    /// Number shortcuts: "1".."9" -> exec
    pub shortcuts: HashMap<String, String>,
    /// Previously typed web searches
    pub search_history: Vec<String>,
    /// Previously executed commands
    pub exec_history: Vec<String>,
}

/// Maximum number of remembered searches / commands.
const MAX_HISTORY_ITEMS: usize = 50;

/// How the default (no-query) history list is ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryMode {
    /// Most recently launched first.
    Recency,
    /// Most launched first.
    Frequency,
    /// Launch count decayed exponentially by age, so often-used apps drift
    /// down when they stop being used (KISS-style adaptive ranking).
    Frecent,
    /// Name order. No score is involved; the caller sorts by name.
    Alphabetical,
}

impl HistoryMode {
    /// Parse the `history-mode` setting value; unknown values fall back to
    /// recency.
    pub fn from_key(key: &str) -> Self {
        match key {
            "frequency" => Self::Frequency,
            "frecent" => Self::Frecent,
            "alphabetical" => Self::Alphabetical,
            _ => Self::Recency,
        }
    }
}

/// Half-life used by [`HistoryMode::Frecent`], in seconds (two weeks).
const FRECENT_HALF_LIFE_SECS: f64 = 14.0 * 24.0 * 3600.0;

impl HistoryData {
    /// Load history, returning the data plus human-readable notes about any
    /// storage problems that were recovered from.
    pub fn load() -> (Self, Vec<String>) {
        storage::load::<Self>(storage::HISTORY_FILE)
            .into_value(storage::HISTORY_FILE, Self::default())
    }

    pub fn save(&self) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        storage::save(storage::HISTORY_FILE, &json)
    }

    /// Current unix timestamp in seconds.
    pub fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0)
    }

    pub fn record_launch(&mut self, exec: &str) {
        self.record_launch_at(exec, Self::now_secs());
    }

    /// Deterministic variant of [`Self::record_launch`] for tests.
    pub fn record_launch_at(&mut self, exec: &str, now: u64) {
        let count = self.launch_counts.entry(exec.to_string()).or_insert(0);
        *count += 1;
        self.last_launched.insert(exec.to_string(), now);
    }

    /// Forget usage statistics for one app (KISS' "reset rank"), keeping
    /// favorites, tags and renames intact.
    pub fn reset_rank(&mut self, exec: &str) {
        self.launch_counts.remove(exec);
        self.last_launched.remove(exec);
        self.shortcuts.retain(|_, value| value != exec);
    }

    /// Clear usage history while preserving favorites and tags.
    pub fn clear_history(&mut self) {
        self.launch_counts.clear();
        self.last_launched.clear();
    }

    pub fn toggle_favorite(&mut self, exec: &str) -> bool {
        if let Some(pos) = self.favorites.iter().position(|e| e == exec) {
            self.favorites.remove(pos);
            false
        } else {
            self.favorites.push(exec.to_string());
            true
        }
    }

    pub fn is_favorite(&self, exec: &str) -> bool {
        self.favorites.iter().any(|e| e == exec)
    }

    pub fn get_launch_count(&self, exec: &str) -> u32 {
        self.launch_counts.get(exec).copied().unwrap_or(0)
    }

    pub fn get_last_launched(&self, exec: &str) -> u64 {
        self.last_launched.get(exec).copied().unwrap_or(0)
    }

    pub fn get_tags(&self, exec: &str) -> Vec<String> {
        self.tags.get(exec).cloned().unwrap_or_default()
    }

    pub fn set_tags(&mut self, exec: &str, tags: Vec<String>) {
        if tags.is_empty() {
            self.tags.remove(exec);
        } else {
            self.tags.insert(exec.to_string(), tags);
        }
    }

    // --- aliases (KISS' "rename app") ------------------------------------

    pub fn alias(&self, exec: &str) -> Option<&String> {
        self.aliases.get(exec)
    }

    pub fn set_alias(&mut self, exec: &str, alias: String) {
        if alias.trim().is_empty() {
            self.aliases.remove(exec);
        } else {
            self.aliases
                .insert(exec.to_string(), alias.trim().to_string());
        }
    }

    // --- custom icons ----------------------------------------------------

    pub fn custom_icon(&self, exec: &str) -> Option<&String> {
        self.custom_icons.get(exec)
    }

    pub fn set_custom_icon(&mut self, exec: &str, icon: String) {
        if icon.trim().is_empty() {
            self.custom_icons.remove(exec);
        } else {
            self.custom_icons
                .insert(exec.to_string(), icon.trim().to_string());
        }
    }

    // --- numbered shortcuts ----------------------------------------------

    pub fn shortcut_for(&self, key: u8) -> Option<&String> {
        self.shortcuts.get(&key.to_string())
    }

    pub fn set_shortcut(&mut self, key: u8, exec: &str) {
        // A shortcut is unique: steal it from whoever had it.
        self.shortcuts.retain(|_, value| value != exec);
        self.shortcuts.insert(key.to_string(), exec.to_string());
    }

    pub fn clear_shortcut(&mut self, key: u8) {
        self.shortcuts.remove(&key.to_string());
    }

    pub fn clear_all_shortcuts(&mut self) {
        self.shortcuts.clear();
    }

    /// Lowest free number between 1 and 9.
    pub fn next_free_shortcut(&self) -> Option<u8> {
        (1..=9).find(|key| self.shortcut_for(*key).is_none())
    }

    // --- provider history -------------------------------------------------

    pub fn record_search(&mut self, query: &str) {
        let query = query.trim().to_string();
        if query.is_empty() {
            return;
        }
        self.search_history.retain(|item| item != &query);
        self.search_history.insert(0, query);
        self.search_history.truncate(MAX_HISTORY_ITEMS);
    }

    pub fn recent_searches(&self, max: usize) -> Vec<String> {
        self.search_history.iter().take(max).cloned().collect()
    }

    pub fn record_exec(&mut self, command: &str) {
        let command = command.trim().to_string();
        if command.is_empty() {
            return;
        }
        self.exec_history.retain(|item| item != &command);
        self.exec_history.insert(0, command);
        self.exec_history.truncate(MAX_HISTORY_ITEMS);
    }

    pub fn recent_execs(&self, max: usize) -> Vec<String> {
        self.exec_history.iter().take(max).cloned().collect()
    }

    /// Ordering score for one app under `mode` at time `now`. Higher sorts
    /// first; ties are expected to be broken by name in the caller.
    pub fn rank_score(&self, mode: HistoryMode, exec: &str, now: u64) -> f64 {
        let count = self.get_launch_count(exec);
        let last = self.get_last_launched(exec);
        match mode {
            HistoryMode::Recency => {
                if count == 0 {
                    0.0
                } else {
                    last as f64
                }
            }
            HistoryMode::Frequency => f64::from(count),
            HistoryMode::Frecent => {
                if count == 0 {
                    0.0
                } else {
                    let age = (now.saturating_sub(last)) as f64;
                    f64::from(count) * 2.0_f64.powf(-age / FRECENT_HALF_LIFE_SECS)
                }
            }
            HistoryMode::Alphabetical => 0.0,
        }
    }

    /// Get top N most frequently used apps
    pub fn top_apps(&self, n: usize) -> Vec<(String, u32)> {
        let mut counts: Vec<(String, u32)> = self
            .launch_counts
            .iter()
            .filter(|(_, &c)| c > 0)
            .map(|(k, &v)| (k.clone(), v))
            .collect();
        counts.sort_by_key(|a| std::cmp::Reverse(a.1));
        counts.truncate(n);
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 4_000_000;

    fn sample_history() -> HistoryData {
        let mut history = HistoryData::default();
        // Used heavily, but more than a month ago.
        history.launch_counts.insert("heavy".to_string(), 10);
        history
            .last_launched
            .insert("heavy".to_string(), NOW - 40 * 24 * 3600);
        // Used twice, an hour ago.
        history.launch_counts.insert("recent".to_string(), 2);
        history
            .last_launched
            .insert("recent".to_string(), NOW - 3600);
        history
    }

    #[test]
    fn recency_orders_by_last_launch() {
        let history = sample_history();
        let recent = history.rank_score(HistoryMode::Recency, "recent", NOW);
        let heavy = history.rank_score(HistoryMode::Recency, "heavy", NOW);
        assert!(recent > heavy, "recent {recent} should beat heavy {heavy}");
    }

    #[test]
    fn frequency_orders_by_launch_count() {
        let history = sample_history();
        let recent = history.rank_score(HistoryMode::Frequency, "recent", NOW);
        let heavy = history.rank_score(HistoryMode::Frequency, "heavy", NOW);
        assert!(heavy > recent, "heavy {heavy} should beat recent {recent}");
    }

    #[test]
    fn frecent_prefers_recent_usage_over_old_volume() {
        let history = sample_history();
        let recent = history.rank_score(HistoryMode::Frecent, "recent", NOW);
        let heavy = history.rank_score(HistoryMode::Frecent, "heavy", NOW);
        assert!(
            recent > heavy,
            "fresh usage {recent} should beat stale volume {heavy}"
        );
    }

    #[test]
    fn frecent_decay_is_monotonic_in_age() {
        let mut history = HistoryData::default();
        history.launch_counts.insert("app".to_string(), 5);
        history.last_launched.insert("app".to_string(), NOW - 60);
        let fresh = history.rank_score(HistoryMode::Frecent, "app", NOW);
        history
            .last_launched
            .insert("app".to_string(), NOW - 30 * 24 * 3600);
        let stale = history.rank_score(HistoryMode::Frecent, "app", NOW);
        assert!(fresh > stale, "fresh {fresh} should beat stale {stale}");
    }

    #[test]
    fn never_launched_apps_score_zero() {
        let history = HistoryData::default();
        for mode in [
            HistoryMode::Recency,
            HistoryMode::Frequency,
            HistoryMode::Frecent,
        ] {
            assert_eq!(history.rank_score(mode, "ghost", NOW), 0.0);
        }
    }

    #[test]
    fn history_mode_from_key() {
        assert_eq!(HistoryMode::from_key("frequency"), HistoryMode::Frequency);
        assert_eq!(HistoryMode::from_key("frecent"), HistoryMode::Frecent);
        assert_eq!(
            HistoryMode::from_key("alphabetical"),
            HistoryMode::Alphabetical
        );
        assert_eq!(HistoryMode::from_key("recency"), HistoryMode::Recency);
        assert_eq!(HistoryMode::from_key("bogus"), HistoryMode::Recency);
    }

    #[test]
    fn reset_rank_keeps_favorites_and_tags() {
        let mut history = HistoryData::default();
        history.record_launch_at("firefox", NOW);
        history.toggle_favorite("firefox");
        history.set_tags("firefox", vec!["web".to_string()]);
        history.reset_rank("firefox");
        assert_eq!(history.get_launch_count("firefox"), 0);
        assert_eq!(history.get_last_launched("firefox"), 0);
        assert!(history.is_favorite("firefox"));
        assert_eq!(history.get_tags("firefox"), vec!["web".to_string()]);
    }

    #[test]
    fn clear_history_keeps_favorites_and_tags() {
        let mut history = HistoryData::default();
        history.record_launch_at("firefox", NOW);
        history.record_launch_at("vim", NOW);
        history.toggle_favorite("firefox");
        history.set_tags("vim", vec!["editor".to_string()]);
        history.clear_history();
        assert!(history.launch_counts.is_empty());
        assert!(history.last_launched.is_empty());
        assert_eq!(history.favorites, vec!["firefox".to_string()]);
        assert_eq!(history.get_tags("vim"), vec!["editor".to_string()]);
    }

    #[test]
    fn record_launch_updates_count_and_timestamp() {
        let mut history = HistoryData::default();
        history.record_launch_at("app", NOW);
        history.record_launch_at("app", NOW + 10);
        assert_eq!(history.get_launch_count("app"), 2);
        assert_eq!(history.get_last_launched("app"), NOW + 10);
    }

    #[test]
    fn serde_roundtrip_preserves_everything() {
        let mut history = HistoryData::default();
        history.record_launch_at("app", NOW);
        history.toggle_favorite("app");
        history.set_tags("app", vec!["a".to_string()]);
        history.set_alias("app", "My App".to_string());
        history.set_custom_icon("app", "app-icon".to_string());
        let json = serde_json::to_string(&history).unwrap();
        let parsed: HistoryData = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.get_launch_count("app"), 1);
        assert!(parsed.is_favorite("app"));
        assert_eq!(parsed.get_tags("app"), vec!["a".to_string()]);
        assert_eq!(parsed.alias("app"), Some(&"My App".to_string()));
        assert_eq!(parsed.custom_icon("app"), Some(&"app-icon".to_string()));
    }

    #[test]
    fn aliases_and_custom_icons_operations() {
        let mut history = HistoryData::default();
        history.set_alias("firefox", "Browser".to_string());
        history.set_custom_icon("firefox", "web-browser".to_string());
        assert_eq!(history.alias("firefox"), Some(&"Browser".to_string()));
        assert_eq!(
            history.custom_icon("firefox"),
            Some(&"web-browser".to_string())
        );

        // Setting empty string clears alias and custom icon
        history.set_alias("firefox", "".to_string());
        history.set_custom_icon("firefox", "   ".to_string());
        assert_eq!(history.alias("firefox"), None);
        assert_eq!(history.custom_icon("firefox"), None);
    }

    #[test]
    fn files_from_older_versions_still_load() {
        let old = r#"{"launch_counts":{"app":3},"last_launched":{"app":123}}"#;
        let parsed: HistoryData = serde_json::from_str(old).unwrap();
        assert_eq!(parsed.get_launch_count("app"), 3);
        assert_eq!(parsed.get_last_launched("app"), 123);
        assert!(parsed.favorites.is_empty());
        assert!(parsed.tags.is_empty());
        assert!(parsed.aliases.is_empty());
        assert!(parsed.custom_icons.is_empty());
        assert!(parsed.shortcuts.is_empty());
        assert!(parsed.search_history.is_empty());
        assert!(parsed.exec_history.is_empty());
    }

    #[test]
    fn alias_can_be_set_and_cleared() {
        let mut history = HistoryData::default();
        history.set_alias("firefox", "  Web  ".to_string());
        assert_eq!(history.alias("firefox"), Some(&"Web".to_string()));
        // An empty alias restores the original name.
        history.set_alias("firefox", String::new());
        assert!(history.alias("firefox").is_none());
    }

    #[test]
    fn custom_icon_can_be_set_and_cleared() {
        let mut history = HistoryData::default();
        history.set_custom_icon("firefox", "  browser-icon  ".to_string());
        assert_eq!(
            history.custom_icon("firefox"),
            Some(&"browser-icon".to_string())
        );
        history.set_custom_icon("firefox", String::new());
        assert!(history.custom_icon("firefox").is_none());
    }

    #[test]
    fn shortcuts_are_unique_per_app() {
        let mut history = HistoryData::default();
        history.set_shortcut(1, "firefox");
        history.set_shortcut(2, "firefox");
        // The app moved: the old number is released.
        assert!(history.shortcut_for(1).is_none());
        assert_eq!(history.shortcut_for(2), Some(&"firefox".to_string()));
        assert_eq!(history.next_free_shortcut(), Some(1));
        history.clear_shortcut(2);
        assert!(history.shortcut_for(2).is_none());
    }

    #[test]
    fn search_and_exec_history_dedup_and_cap() {
        let mut history = HistoryData::default();
        history.record_search("rust");
        history.record_search("egui");
        history.record_search("rust");
        assert_eq!(history.recent_searches(10), vec!["rust", "egui"]);
        history.record_exec("ls -la");
        assert_eq!(history.recent_execs(10), vec!["ls -la"]);
        for i in 0..60 {
            history.record_exec(&format!("cmd {i}"));
        }
        assert_eq!(history.recent_execs(usize::MAX).len(), MAX_HISTORY_ITEMS);
    }

    #[test]
    fn new_fields_survive_serde_roundtrip() {
        let mut history = HistoryData::default();
        history.set_alias("app", "Alias".to_string());
        history.set_custom_icon("app", "custom-icon".to_string());
        history.set_shortcut(3, "app");
        history.record_search("query");
        history.record_exec("do-thing");
        let json = serde_json::to_string(&history).unwrap();
        let parsed: HistoryData = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.alias("app"), Some(&"Alias".to_string()));
        assert_eq!(parsed.custom_icon("app"), Some(&"custom-icon".to_string()));
        assert_eq!(parsed.shortcut_for(3), Some(&"app".to_string()));
        assert_eq!(parsed.recent_searches(5), vec!["query"]);
        assert_eq!(parsed.recent_execs(5), vec!["do-thing"]);
    }
}
