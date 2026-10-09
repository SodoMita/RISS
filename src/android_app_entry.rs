// Android-specific app entry - uses JNI to read installed apps
#[cfg(target_os = "android")]
pub mod android_jni;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Special exec command: opens the RISS settings screen.
pub const EXEC_SETTINGS: &str = "riss:settings";

/// Represents a launchable application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub name: String,
    pub comment: String,
    pub exec: String, // On Android: package name
    pub icon: String,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub tags: Vec<String>,
    pub desktop_file: PathBuf,
    pub launch_count: u32,
    pub last_launched: u64,
    pub is_favorite: bool,
}

impl AppEntry {
    /// Build a synthetic entry for providers (web search, commands, …).
    pub fn virtual_entry(name: &str, comment: &str, exec: &str) -> Self {
        AppEntry {
            name: name.to_string(),
            comment: comment.to_string(),
            exec: exec.to_string(),
            icon: String::new(),
            categories: Vec::new(),
            keywords: Vec::new(),
            tags: Vec::new(),
            desktop_file: PathBuf::new(),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
        }
    }

    /// Is this one of the synthetic entries rather than a real application?
    pub fn is_virtual(&self) -> bool {
        self.desktop_file.as_os_str().is_empty()
    }

    pub fn searchable_text(&self) -> String {
        let mut parts = vec![self.name.clone()];
        if !self.comment.is_empty() {
            parts.push(self.comment.clone());
        }
        parts.extend(self.tags.iter().cloned());
        parts.extend(self.categories.iter().cloned());
        parts.extend(self.keywords.iter().cloned());
        parts.join(" ")
    }

    pub fn launch(&self) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            android_jni::launch_app_jni(&self.exec)
        }

        #[cfg(not(target_os = "android"))]
        {
            Err("Not on Android".to_string())
        }
    }
}

/// Discover all installed applications via JNI
pub fn discover_apps() -> Vec<AppEntry> {
    #[cfg(target_os = "android")]
    {
        android_jni::discover_apps_jni()
    }

    #[cfg(not(target_os = "android"))]
    {
        Vec::new()
    }
}

pub fn builtin_entries() -> Vec<AppEntry> {
    Vec::new()
}
