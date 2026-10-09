use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Persistent history and favorites data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

impl HistoryData {
    fn data_path() -> PathBuf {
        #[cfg(target_os = "android")]
        {
            // On Android, use the app's internal storage directory
            // This would typically be obtained from the Android context
            // For now, use a relative path that should work
            PathBuf::from("history.json")
        }

        #[cfg(not(target_os = "android"))]
        {
            let mut path = if let Ok(home) = std::env::var("HOME") {
                PathBuf::from(home)
            } else {
                PathBuf::from(".")
            };
            path.push(".config");
            path.push("kiss-launcher");
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
        if tags.is_empty() {
            self.tags.remove(exec);
        } else {
            self.tags.insert(exec.to_string(), tags);
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
        counts.sort_by(|a, b| b.1.cmp(&a.1));
        counts.truncate(n);
        counts
    }
}
