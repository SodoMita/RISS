use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Persistent history, favorites and user customizations.
///
/// This mirrors the `KissSettings` database of the original launcher: launch
/// counts, last used timestamps, favorites, tags, the exclusion lists, renamed
/// apps and the numbered shortcuts, plus the small amount of history needed by
/// the search and command providers.
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
    /// Apps hidden from the launcher entirely
    pub excluded: Vec<String>,
    /// Apps whose usage is not recorded
    pub excluded_from_history: Vec<String>,
    /// Custom display names: exec -> alias
    pub aliases: HashMap<String, String>,
    /// Number shortcuts: "1".."9" -> exec
    pub shortcuts: HashMap<String, String>,
    /// Previously typed web searches
    pub search_history: Vec<String>,
    /// Previously executed commands
    pub exec_history: Vec<String>,
}

/// Maximum number of tracked apps before the oldest ones are dropped.
const MAX_TRACKED_APPS: usize = 512;
/// Maximum number of remembered searches / commands.
const MAX_HISTORY_ITEMS: usize = 50;

impl HistoryData {
    fn data_path() -> PathBuf {
        crate::settings::config_dir().join("history.json")
    }

    pub fn load() -> Self {
        match fs::read_to_string(Self::data_path()) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        if let Ok(data) = serde_json::to_string_pretty(self) {
            fs::write(Self::data_path(), data).ok();
        }
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    // --- launching --------------------------------------------------------

    pub fn record_launch(&mut self, exec: &str) {
        if !exec.is_empty() {
            let count = self.launch_counts.entry(exec.to_string()).or_insert(0);
            *count += 1;
            self.last_launched.insert(exec.to_string(), Self::now());
        }
    }

    /// Forget everything we know about an app (KISS' "reset rank").
    pub fn reset_rank(&mut self, exec: &str) {
        self.launch_counts.remove(exec);
        self.last_launched.remove(exec);
        self.shortcuts.retain(|_, value| value != exec);
    }

    /// Drop the least recently used entries so the database stays small.
    pub fn prune(&mut self) {
        if self.launch_counts.len() <= MAX_TRACKED_APPS {
            return;
        }
        let mut entries: Vec<(String, u64)> = self
            .last_launched
            .iter()
            .map(|(exec, ts)| (exec.clone(), *ts))
            .collect();
        entries.sort_by_key(|(_, ts)| std::cmp::Reverse(*ts));
        entries.truncate(MAX_TRACKED_APPS);
        let keep: Vec<String> = entries.into_iter().map(|(exec, _)| exec).collect();
        self.launch_counts.retain(|exec, _| keep.contains(exec));
        self.last_launched.retain(|exec, _| keep.contains(exec));
    }

    pub fn get_launch_count(&self, exec: &str) -> u32 {
        self.launch_counts.get(exec).copied().unwrap_or(0)
    }

    pub fn get_last_launched(&self, exec: &str) -> u64 {
        self.last_launched.get(exec).copied().unwrap_or(0)
    }

    // --- favorites --------------------------------------------------------

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

    // --- tags -------------------------------------------------------------

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

    // --- exclusion --------------------------------------------------------

    pub fn is_excluded(&self, exec: &str) -> bool {
        self.excluded.iter().any(|e| e == exec)
    }

    pub fn add_excluded(&mut self, exec: &str) {
        if !self.is_excluded(exec) {
            self.excluded.push(exec.to_string());
        }
    }

    pub fn remove_excluded(&mut self, exec: &str) {
        self.excluded.retain(|e| e != exec);
    }

    pub fn is_excluded_from_history(&self, exec: &str) -> bool {
        self.excluded_from_history.iter().any(|e| e == exec)
    }

    pub fn add_excluded_from_history(&mut self, exec: &str) {
        if !self.is_excluded_from_history(exec) {
            self.excluded_from_history.push(exec.to_string());
        }
    }

    pub fn remove_excluded_from_history(&mut self, exec: &str) {
        self.excluded_from_history.retain(|e| e != exec);
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

    // --- resets (KISS' "reset …" buttons) --------------------------------

    pub fn reset_history(&mut self) {
        self.launch_counts.clear();
        self.last_launched.clear();
        self.search_history.clear();
        self.exec_history.clear();
    }

    pub fn reset_favorites(&mut self) {
        self.favorites.clear();
        self.shortcuts.clear();
    }

    pub fn reset_tags(&mut self) {
        self.tags.clear();
    }

    pub fn reset_excluded(&mut self) {
        self.excluded.clear();
        self.excluded_from_history.clear();
    }

    pub fn reset_all(&mut self) {
        *self = HistoryData::default();
    }

    /// Get top N most frequently used apps
    pub fn top_apps(&self, n: usize) -> Vec<(String, u32)> {
        let mut counts: Vec<(String, u32)> = self
            .launch_counts
            .iter()
            .filter(|(_, &c)| c > 0)
            .map(|(exec, &count)| (exec.clone(), count))
            .collect();
        counts.sort_by_key(|a| std::cmp::Reverse(a.1));
        counts.truncate(n);
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favorites_toggle() {
        let mut history = HistoryData::default();
        assert!(history.toggle_favorite("firefox"));
        assert!(history.is_favorite("firefox"));
        assert!(!history.toggle_favorite("firefox"));
        assert!(!history.is_favorite("firefox"));
    }

    #[test]
    fn shortcuts_are_unique() {
        let mut history = HistoryData::default();
        history.set_shortcut(3, "firefox");
        assert_eq!(history.shortcut_for(3), Some(&"firefox".to_string()));
        history.set_shortcut(7, "firefox");
        assert!(history.shortcut_for(3).is_none());
        assert_eq!(history.shortcut_for(7), Some(&"firefox".to_string()));
        assert_eq!(history.next_free_shortcut(), Some(1));
    }

    #[test]
    fn reset_rank_forgets_usage_only() {
        let mut history = HistoryData::default();
        history.record_launch("firefox");
        history.set_tags("firefox", vec!["web".to_string()]);
        history.reset_rank("firefox");
        assert_eq!(history.get_launch_count("firefox"), 0);
        assert_eq!(history.get_tags("firefox"), vec!["web".to_string()]);
    }

    #[test]
    fn history_serializes_with_defaults() {
        let mut history = HistoryData::default();
        history.record_launch("xdg-open ~");
        let json = serde_json::to_string(&history).unwrap();
        let parsed: HistoryData = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.get_launch_count("xdg-open ~"), 1);
    }
}
