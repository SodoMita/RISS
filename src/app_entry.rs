use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Special exec command: opens the RISS settings screen.
pub const EXEC_SETTINGS: &str = "riss:settings";

/// Represents a launchable application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub name: String,
    pub comment: String,
    pub exec: String,
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
    /// Build a synthetic entry for things that are not real applications
    /// (web searches, shell commands, settings rows, timers, …).
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

    /// Search text for fuzzy matching (name + comment + tags + categories)
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

    /// Is this one of the synthetic entries rather than a real application?
    pub fn is_virtual(&self) -> bool {
        self.desktop_file.as_os_str().is_empty()
    }

    /// Launch the application
    pub fn launch(&self) -> Result<(), String> {
        if self.exec.is_empty() {
            return Err("No exec command".to_string());
        }
        if self.exec == EXEC_SETTINGS {
            return Ok(());
        }

        let command = strip_field_codes(&self.exec);
        if command.is_empty() {
            return Err("Empty exec command".to_string());
        }

        // A shell is used so quoted arguments and `%` handling behave the same
        // way they do when the desktop environment launches the entry.
        std::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to launch {}: {}", self.name, e))
    }
}

/// Remove the desktop entry field codes (`%f`, `%U`, `%i`, …) from an `Exec`
/// line, leaving a plain command line.
pub fn strip_field_codes(exec: &str) -> String {
    let codes = [
        '%', 'f', 'F', 'u', 'U', 'i', 'c', 'k', 'd', 'D', 'n', 'N', 'v', 'm',
    ];
    let mut out = String::new();
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            // Skip the code character (`%%` is a literal percent sign).
            if chars.peek() == Some(&'%') {
                chars.next();
                out.push('%');
            } else if let Some(next) = chars.next() {
                if !codes.contains(&next) {
                    out.push('%');
                    out.push(next);
                }
            }
        } else {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse a .desktop file into an AppEntry
fn parse_desktop_file(path: &Path) -> Option<AppEntry> {
    let content = fs::read_to_string(path).ok()?;
    let mut name = String::new();
    let mut localized_name = String::new();
    let mut comment = String::new();
    let mut exec = String::new();
    let mut try_exec: Vec<String> = Vec::new();
    let mut icon = String::new();
    let mut categories = Vec::new();
    let mut keywords = Vec::new();
    let mut no_display = false;
    let mut hidden = false;
    let mut entry_type_ok = true;

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
        } else if line.starts_with("Name[") {
            if let Some((_, val)) = line.split_once('=') {
                if localized_name.is_empty() {
                    localized_name = val.to_string();
                }
            }
        } else if let Some(val) = line.strip_prefix("Comment=") {
            if comment.is_empty() {
                comment = val.to_string();
            }
        } else if let Some(val) = line.strip_prefix("Exec=") {
            if exec.is_empty() {
                exec = val.to_string();
            }
        } else if let Some(val) = line.strip_prefix("TryExec=") {
            try_exec.push(val.to_string());
        } else if let Some(val) = line.strip_prefix("Icon=") {
            if icon.is_empty() {
                icon = val.to_string();
            }
        } else if let Some(val) = line.strip_prefix("Categories=") {
            categories = val
                .split(';')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
        } else if let Some(val) = line.strip_prefix("Keywords=") {
            keywords = val
                .split(';')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
        } else if let Some(val) = line.strip_prefix("NoDisplay=") {
            no_display = val.eq_ignore_ascii_case("true");
        } else if let Some(val) = line.strip_prefix("Hidden=") {
            hidden = val.eq_ignore_ascii_case("true");
        } else if let Some(val) = line.strip_prefix("Type=") {
            entry_type_ok = val.eq_ignore_ascii_case("Application");
        }
    }

    if !entry_type_ok {
        return None;
    }
    if name.is_empty() {
        name = localized_name;
    }
    if name.is_empty() || no_display || hidden {
        return None;
    }

    // `TryExec` lets us hide entries whose binary is not installed.
    if !try_exec.is_empty() && !try_exec.iter().any(|bin| which(bin).is_some()) {
        return None;
    }

    Some(AppEntry {
        name,
        comment,
        exec,
        icon,
        categories,
        keywords,
        tags: Vec::new(),
        desktop_file: path.to_path_buf(),
        launch_count: 0,
        last_launched: 0,
        is_favorite: false,
    })
}

/// Minimal `which` lookup over `PATH`.
fn which(binary: &str) -> Option<PathBuf> {
    if binary.contains('/') {
        let path = PathBuf::from(binary);
        return if path.is_file() { Some(path) } else { None };
    }
    let paths = std::env::var("PATH").ok()?;
    for dir in paths.split(':') {
        if dir.is_empty() {
            continue;
        }
        let candidate = Path::new(dir).join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
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
    entries.sort_by_key(|a| a.name.to_lowercase());
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
        } else if path.extension().is_some_and(|ext| ext == "desktop") {
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
            name: "RISS Settings".to_string(),
            comment: "Tune the launcher".to_string(),
            exec: EXEC_SETTINGS.to_string(),
            icon: "preferences-system".to_string(),
            categories: vec!["Settings".to_string()],
            keywords: vec!["settings".to_string(), "preferences".to_string()],
            tags: vec!["settings".to_string(), "preferences".to_string()],
            desktop_file: PathBuf::new(),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
        },
        AppEntry {
            name: "Lock Screen".to_string(),
            comment: "Lock the screen".to_string(),
            exec: "xdg-screensaver lock".to_string(),
            icon: "system-lock-screen".to_string(),
            categories: vec!["System".to_string()],
            keywords: vec!["lock".to_string(), "screen".to_string()],
            tags: vec!["lock".to_string(), "screen".to_string()],
            desktop_file: PathBuf::new(),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
        },
        AppEntry {
            name: "System Settings".to_string(),
            comment: "System settings".to_string(),
            exec: "gnome-control-center".to_string(),
            icon: "preferences-system".to_string(),
            categories: vec!["Settings".to_string()],
            keywords: vec!["settings".to_string(), "preferences".to_string()],
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
            keywords: vec!["files".to_string(), "explorer".to_string()],
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
            keywords: vec!["terminal".to_string(), "console".to_string()],
            tags: vec![
                "terminal".to_string(),
                "console".to_string(),
                "shell".to_string(),
            ],
            desktop_file: PathBuf::new(),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_field_codes() {
        assert_eq!(strip_field_codes("firefox %u"), "firefox");
        assert_eq!(strip_field_codes("gimp-2.10 %U"), "gimp-2.10");
        assert_eq!(strip_field_codes("app --flag  42"), "app --flag 42");
    }

    #[test]
    fn virtual_entries_are_marked() {
        let entry = AppEntry::virtual_entry("Search", "web", "https://example.org");
        assert!(entry.is_virtual());
        assert_eq!(entry.name, "Search");
    }
}
