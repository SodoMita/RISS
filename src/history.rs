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
}

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
    /// favorites and tags intact.
    pub fn reset_rank(&mut self, exec: &str) {
        self.launch_counts.remove(exec);
        self.last_launched.remove(exec);
    }

    /// Clear usage history while preserving favorites and tags.
    pub fn clear_history(&mut self) {
        self.launch_counts.clear();
        self.last_launched.clear();
    }

    /// Add or remove `exec` from the favorites bar. Returns the new state:
    /// `true` when it ended up favorited.
    pub fn toggle_favorite(&mut self, exec: &str) -> bool {
        if self.remove_favorite(exec) {
            false
        } else {
            self.add_favorite(exec);
            true
        }
    }

    /// Append `exec` to the favorites bar unless it is already there.
    /// Returns `true` when the bar changed. Mirrors KISS'
    /// `DataHandler.addToFavorites`, which also ignores ids already present.
    pub fn add_favorite(&mut self, exec: &str) -> bool {
        if self.is_favorite(exec) {
            return false;
        }
        self.favorites.push(exec.to_string());
        true
    }

    /// Take `exec` off the favorites bar. Returns `true` when it was there.
    /// Mirrors KISS' `DataHandler.removeFromFavorites`.
    pub fn remove_favorite(&mut self, exec: &str) -> bool {
        match self.favorites.iter().position(|e| e == exec) {
            Some(pos) => {
                self.favorites.remove(pos);
                true
            }
            None => false,
        }
    }

    /// Position of `exec` in the favorites bar, or `None` when it is not a
    /// favorite. The position is the order the bar is drawn in.
    pub fn favorite_index(&self, exec: &str) -> Option<usize> {
        self.favorites.iter().position(|e| e == exec)
    }

    /// Move a favorite to the slot `to`, clamped to the ends of the bar.
    /// Returns `true` when the order changed.
    ///
    /// Same remove-then-insert rule KISS uses when a favorite is dragged
    /// (`FavoriteAdapter.moveItem`, persisted through
    /// `DataHandler.setFavoritePositions`, which clamps the index too).
    pub fn move_favorite_to(&mut self, exec: &str, to: usize) -> bool {
        let Some(from) = self.favorite_index(exec) else {
            return false;
        };
        let to = to.min(self.favorites.len().saturating_sub(1));
        if from == to {
            return false;
        }
        let entry = self.favorites.remove(from);
        self.favorites.insert(to, entry);
        true
    }

    /// Shift a favorite by `delta` slots; negative moves it towards the start
    /// of the bar. Clamped at both ends, returns `true` when it moved.
    pub fn move_favorite_by(&mut self, exec: &str, delta: isize) -> bool {
        let Some(from) = self.favorite_index(exec) else {
            return false;
        };
        let target = from as isize + delta;
        if target < 0 {
            return false;
        }
        self.move_favorite_to(exec, target as usize)
    }

    pub fn is_favorite(&self, exec: &str) -> bool {
        self.favorites.iter().any(|e| e == exec)
    }

    /// Empty the favorites bar, returning how many favorites were dropped
    /// (KISS' `DataHandler.resetFavorites`).
    pub fn clear_favorites(&mut self) -> usize {
        let count = self.favorites.len();
        self.favorites.clear();
        count
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

    /// Three favorites in bar order, as `add_favorite` would have stored them.
    fn sample_bar() -> HistoryData {
        let mut history = HistoryData::default();
        for exec in ["firefox", "vim", "thunderbird"] {
            history.add_favorite(exec);
        }
        history
    }

    #[test]
    fn add_favorite_is_idempotent_and_keeps_order() {
        let mut history = sample_bar();
        assert!(!history.add_favorite("vim"), "re-adding should be a no-op");
        assert_eq!(
            history.favorites,
            vec![
                "firefox".to_string(),
                "vim".to_string(),
                "thunderbird".to_string()
            ]
        );
    }

    #[test]
    fn toggle_favorite_adds_then_removes() {
        let mut history = HistoryData::default();
        assert!(history.toggle_favorite("firefox"));
        assert_eq!(history.favorite_index("firefox"), Some(0));
        assert!(!history.toggle_favorite("firefox"));
        assert!(history.favorites.is_empty());
    }

    #[test]
    fn remove_favorite_reports_whether_it_was_there() {
        let mut history = sample_bar();
        assert!(history.remove_favorite("vim"));
        assert!(!history.remove_favorite("vim"));
        assert_eq!(
            history.favorites,
            vec!["firefox".to_string(), "thunderbird".to_string()]
        );
    }

    #[test]
    fn favorite_index_is_none_for_unknown_apps() {
        assert_eq!(sample_bar().favorite_index("ghost"), None);
    }

    #[test]
    fn move_favorite_by_shifts_within_the_bar() {
        let mut history = sample_bar();
        assert!(history.move_favorite_by("thunderbird", -2));
        assert_eq!(
            history.favorites.first().map(String::as_str),
            Some("thunderbird")
        );
        assert!(history.move_favorite_by("thunderbird", 1));
        assert_eq!(history.favorite_index("thunderbird"), Some(1));
    }

    #[test]
    fn move_favorite_by_clamps_at_both_ends() {
        let mut history = sample_bar();
        assert!(
            !history.move_favorite_by("firefox", -1),
            "already first, nothing to move"
        );
        assert_eq!(
            history.favorites.first().map(String::as_str),
            Some("firefox")
        );
        assert!(
            history.move_favorite_by("firefox", 99),
            "large deltas clamp to the last slot"
        );
        assert_eq!(history.favorite_index("firefox"), Some(2));
    }

    #[test]
    fn move_favorite_to_reorders_and_clamps() {
        let mut history = sample_bar();
        assert!(history.move_favorite_to("firefox", 2));
        assert_eq!(
            history.favorites,
            vec![
                "vim".to_string(),
                "thunderbird".to_string(),
                "firefox".to_string()
            ]
        );
        assert!(!history.move_favorite_to("firefox", 2), "already there");
        assert!(
            !history.move_favorite_to("firefox", 500),
            "clamped, no move"
        );
        assert!(!history.move_favorite_to("ghost", 0), "not a favorite");
    }

    #[test]
    fn moving_unknown_apps_leaves_the_bar_alone() {
        let mut history = sample_bar();
        assert!(!history.move_favorite_by("ghost", 1));
        assert!(!history.move_favorite_to("ghost", 0));
        assert_eq!(history.favorites.len(), 3);
    }

    #[test]
    fn clear_favorites_reports_the_count_and_keeps_usage() {
        let mut history = sample_bar();
        history.record_launch_at("vim", NOW);
        assert_eq!(history.clear_favorites(), 3);
        assert!(history.favorites.is_empty());
        assert_eq!(history.get_launch_count("vim"), 1, "usage data survives");
        assert_eq!(history.clear_favorites(), 0);
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
        let json = serde_json::to_string(&history).unwrap();
        let parsed: HistoryData = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.get_launch_count("app"), 1);
        assert!(parsed.is_favorite("app"));
        assert_eq!(parsed.get_tags("app"), vec!["a".to_string()]);
    }

    #[test]
    fn files_from_older_versions_still_load() {
        let old = r#"{"launch_counts":{"app":3},"last_launched":{"app":123}}"#;
        let parsed: HistoryData = serde_json::from_str(old).unwrap();
        assert_eq!(parsed.get_launch_count("app"), 3);
        assert_eq!(parsed.get_last_launched("app"), 123);
        assert!(parsed.favorites.is_empty());
        assert!(parsed.tags.is_empty());
    }
}
