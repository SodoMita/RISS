// Android-specific app entry - uses JNI to read installed apps
#[cfg(target_os = "android")]
pub mod android_jni;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a launchable application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub name: String,
    pub comment: String,
    pub exec: String, // On Android: package name
    pub icon: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub desktop_file: PathBuf,
    pub launch_count: u32,
    pub last_launched: u64,
    pub is_favorite: bool,
    #[serde(default)]
    pub is_system: bool,
}

impl AppEntry {
    /// A synthetic entry for provider results (web search, timers, settings…)
    /// that does not correspond to an installed application.
    pub fn virtual_entry(name: &str, comment: &str, exec: &str) -> Self {
        AppEntry {
            name: name.to_string(),
            comment: comment.to_string(),
            exec: exec.to_string(),
            icon: String::new(),
            categories: Vec::new(),
            tags: Vec::new(),
            desktop_file: PathBuf::new(),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
            is_system: false,
        }
    }

    /// Is this one of the synthetic entries rather than a real application?
    ///
    /// Discovered apps carry an icon name (their package), while
    /// `virtual_entry` leaves both `desktop_file` and `icon` empty.
    pub fn is_virtual(&self) -> bool {
        self.desktop_file.as_os_str().is_empty() && self.icon.is_empty()
    }

    pub fn searchable_text(&self) -> String {
        let mut parts = vec![self.name.clone()];
        if !self.comment.is_empty() {
            parts.push(self.comment.clone());
        }
        parts.extend(self.tags.iter().cloned());
        parts.extend(self.categories.iter().cloned());
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

    pub fn uninstall(&self) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            android_jni::uninstall_app_jni(&self.exec)
        }

        #[cfg(not(target_os = "android"))]
        {
            Err("Not on Android".to_string())
        }
    }

    pub fn open_app_info(&self) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            android_jni::open_app_info_jni(&self.exec)
        }

        #[cfg(not(target_os = "android"))]
        {
            Err("Not on Android".to_string())
        }
    }

    pub fn view_in_store(&self) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            android_jni::view_in_store_jni(&self.exec)
        }

        #[cfg(not(target_os = "android"))]
        {
            Err("Not on Android".to_string())
        }
    }
}

pub type IconPixels = (usize, usize, Vec<u8>);

/// Load an Android application's icon using its package name or custom icon.
pub fn load_icon_rgba(entry: &AppEntry) -> Option<IconPixels> {
    #[cfg(target_os = "android")]
    {
        let icon_pkg = if entry.icon.trim().is_empty() {
            &entry.exec
        } else {
            &entry.icon
        };
        android_jni::load_app_icon_jni(icon_pkg)
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = entry;
        None
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
