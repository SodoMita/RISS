use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Represents a launchable application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub name: String,
    pub comment: String,
    pub exec: String,
    pub icon: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub desktop_file: PathBuf,
    pub launch_count: u32,
    pub last_launched: u64,
    pub is_favorite: bool,
}

impl AppEntry {
    /// Search text for fuzzy matching (name + comment + tags + categories)
    pub fn searchable_text(&self) -> String {
        let mut parts = vec![self.name.clone()];
        if !self.comment.is_empty() {
            parts.push(self.comment.clone());
        }
        parts.extend(self.tags.iter().cloned());
        parts.extend(self.categories.iter().cloned());
        parts.join(" ")
    }

    /// Launch the application
    pub fn launch(&self) -> Result<(), String> {
        if self.exec.is_empty() {
            return Err("No exec command".to_string());
        }

        // Parse the exec string - remove field codes like %f, %F, %u, %U, etc.
        let exec_parts: Vec<&str> = self.exec.split_whitespace().collect();
        if exec_parts.is_empty() {
            return Err("Empty exec command".to_string());
        }

        let cmd = exec_parts[0];
        let args: Vec<&str> = exec_parts[1..]
            .iter()
            .filter(|a| !a.starts_with('%'))
            .copied()
            .collect();

        std::process::Command::new(cmd)
            .args(&args)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to launch {}: {}", self.name, e))
    }
}

/// Parse a .desktop file into an AppEntry
fn parse_desktop_file(path: &Path) -> Option<AppEntry> {
    let content = fs::read_to_string(path).ok()?;
    let mut name = String::new();
    let mut comment = String::new();
    let mut exec = String::new();
    let mut icon = String::new();
    let mut categories = Vec::new();
    let mut no_display = false;
    let mut hidden = false;

    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();

        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }

        if line.starts_with('[') && line != "[Desktop Entry]" {
            if in_desktop_entry {
                break;
            }
            continue;
        }

        if !in_desktop_entry {
            continue;
        }

        if let Some(val) = line.strip_prefix("Name=") {
            if name.is_empty() {
                name = val.to_string();
            }
        } else if let Some(val) = line.strip_prefix("Comment=") {
            if comment.is_empty() {
                comment = val.to_string();
            }
        } else if let Some(val) = line.strip_prefix("Exec=") {
            exec = val.to_string();
        } else if let Some(val) = line.strip_prefix("Icon=") {
            icon = val.to_string();
        } else if let Some(val) = line.strip_prefix("Categories=") {
            categories = val.split(';').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
        } else if let Some(val) = line.strip_prefix("NoDisplay=") {
            no_display = val.eq_ignore_ascii_case("true");
        } else if let Some(val) = line.strip_prefix("Hidden=") {
            hidden = val.eq_ignore_ascii_case("true");
        } else if let Some(val) = line.strip_prefix("Type=") {
            if !val.eq_ignore_ascii_case("Application") {
                return None;
            }
        }
    }

    if name.is_empty() || no_display || hidden {
        return None;
    }

    Some(AppEntry {
        name,
        comment,
        exec,
        icon,
        categories,
        tags: Vec::new(),
        desktop_file: path.to_path_buf(),
        launch_count: 0,
        last_launched: 0,
        is_favorite: false,
    })
}

/// Scan standard directories for .desktop files
pub fn discover_apps() -> Vec<AppEntry> {
    let mut entries = Vec::new();
    let mut seen = HashMap::new();

    // Standard XDG application directories
    let mut search_dirs: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];

    // Add user directory
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
        search_dirs.push(PathBuf::from(data_home).join("applications"));
    } else if let Ok(home) = std::env::var("HOME") {
        search_dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }

    // Add XDG_DATA_DIRS
    if let Ok(data_dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in data_dirs.split(':') {
            search_dirs.push(PathBuf::from(dir).join("applications"));
        }
    }

    for dir in &search_dirs {
        if !dir.exists() {
            continue;
        }
        scan_directory(dir, &mut entries, &mut seen);
    }

    // Sort by name
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries
}

fn scan_directory(dir: &Path, entries: &mut Vec<AppEntry>, seen: &mut HashMap<String, usize>) {
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_directory(&path, entries, seen);
        } else if path.extension().map_or(false, |ext| ext == "desktop") {
            if let Some(app) = parse_desktop_file(&path) {
                let key = app.exec.clone();
                if let Some(&idx) = seen.get(&key) {
                    // Keep the entry with more info (longer name/comment)
                    if app.name.len() > entries[idx].name.len() {
                        entries[idx] = app;
                    }
                } else {
                    seen.insert(key, entries.len());
                    entries.push(app);
                }
            }
        }
    }
}

/// Add some built-in "virtual" entries for common actions
pub fn builtin_entries() -> Vec<AppEntry> {
    vec![
        AppEntry {
            name: "Lock Screen".to_string(),
            comment: "Lock the screen".to_string(),
            exec: "xdg-screensaver lock".to_string(),
            icon: "system-lock-screen".to_string(),
            categories: vec!["System".to_string()],
            tags: vec!["lock".to_string(), "screen".to_string()],
            desktop_file: PathBuf::new(),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
        },
        AppEntry {
            name: "Settings".to_string(),
            comment: "System settings".to_string(),
            exec: "gnome-control-center".to_string(),
            icon: "preferences-system".to_string(),
            categories: vec!["Settings".to_string()],
            tags: vec!["settings".to_string(), "preferences".to_string()],
            desktop_file: PathBuf::new(),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
        },
        AppEntry {
            name: "File Manager".to_string(),
            comment: "Browse files".to_string(),
            exec: "xdg-open ~".to_string(),
            icon: "system-file-manager".to_string(),
            categories: vec!["System".to_string()],
            tags: vec!["files".to_string(), "explorer".to_string()],
            desktop_file: PathBuf::new(),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
        },
        AppEntry {
            name: "Terminal".to_string(),
            comment: "Open a terminal".to_string(),
            exec: "x-terminal-emulator".to_string(),
            icon: "utilities-terminal".to_string(),
            categories: vec!["System".to_string()],
            tags: vec!["terminal".to_string(), "console".to_string(), "shell".to_string()],
            desktop_file: PathBuf::new(),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
        },
    ]
}
