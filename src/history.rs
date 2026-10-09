use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
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
    /// Custom tag overrides: exec -> tags. An empty vector explicitly clears platform tags.
    pub tags: HashMap<String, Vec<String>>,
}

impl HistoryData {
    fn data_path() -> PathBuf {
        #[cfg(target_os = "android")]
        {
            crate::android_app_entry::android_jni::internal_data_path()
                .map(|directory| directory.join("history.json"))
                .unwrap_or_else(|| PathBuf::from("history.json"))
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
            path.push("history.json");
            path
        }
    }

    pub fn load() -> Self {
        let path = Self::data_path();
        if let Ok(data) = fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        let path = Self::data_path();
        if let Ok(data) = serde_json::to_string_pretty(self) {
            fs::write(path, data).ok();
        }
    }

    pub fn record_launch(&mut self, exec: &str) {
        let count = self.launch_counts.entry(exec.to_string()).or_insert(0);
        *count += 1;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_launched.insert(exec.to_string(), now);
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
        self.tags.insert(exec.to_string(), tags);
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
    use super::HistoryData;

    #[test]
    fn older_history_files_without_tags_still_load() {
        let restored: HistoryData =
            serde_json::from_str(r#"{"favorites":["example.app"]}"#).unwrap();

        assert!(restored.is_favorite("example.app"));
        assert!(restored.tags.is_empty());
    }

    #[test]
    fn empty_tags_remain_an_explicit_override() {
        let mut history = HistoryData::default();
        history.set_tags("example.app", Vec::new());

        let encoded = serde_json::to_string(&history).unwrap();
        let restored: HistoryData = serde_json::from_str(&encoded).unwrap();
        assert!(restored.tags.contains_key("example.app"));
        assert!(restored.get_tags("example.app").is_empty());
    }
}
