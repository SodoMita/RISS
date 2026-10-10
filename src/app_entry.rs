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
    /// Discovered apps carry a `desktop_file` (or, on Android, an icon name),
    /// while `virtual_entry` leaves both empty.
    pub fn is_virtual(&self) -> bool {
        self.desktop_file.as_os_str().is_empty() && self.icon.is_empty()
    }

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

    /// Whether the Uninstall action can do anything for this entry.
    ///
    /// Virtual entries have nothing to uninstall; for real apps the
    /// feasibility is decided when the responsible package manager is
    /// resolved.
    pub fn can_uninstall(&self) -> bool {
        !self.is_virtual()
    }

    /// Describe what Uninstall would do, without running anything.
    ///
    /// Resolves the package manager that installed the app so the
    /// confirmation prompt can show the exact removal command.
    pub fn uninstall_preview(&self) -> Result<String, String> {
        crate::package_managers::plan(self).map(|plan| plan.summary)
    }

    /// Uninstall the application through the package manager that installed
    /// it: Flatpak and Snap apps go through their own tools, AppImage
    /// binaries and hand-placed desktop files are deleted directly, and
    /// distro packages are removed with the native package manager,
    /// escalating through `pkexec` or `sudo` as needed.
    pub fn uninstall(&self) -> Result<(), String> {
        let plan = crate::package_managers::plan(self)?;
        crate::package_managers::execute(&plan)
    }

    /// Open app information
    pub fn open_app_info(&self) -> Result<(), String> {
        if self.desktop_file.is_file() {
            if let Some(parent) = self.desktop_file.parent() {
                open::that(parent).map_err(|e| format!("Failed to open app info: {e}"))
            } else {
                open::that(&self.desktop_file).map_err(|e| format!("Failed to open app info: {e}"))
            }
        } else {
            Err("No desktop file found for app".to_string())
        }
    }

    /// View application in store
    pub fn view_in_store(&self) -> Result<(), String> {
        let url = if self.exec.contains('.') && !self.exec.contains('/') {
            format!(
                "https://play.google.com/store/apps/details?id={}",
                self.exec
            )
        } else {
            format!(
                "https://flathub.org/apps/search?q={}",
                self.name.replace(' ', "+")
            )
        };
        open::that(&url).map_err(|e| format!("Failed to open store: {e}"))
    }
}

pub type IconPixels = (usize, usize, Vec<u8>);

/// Load and decode a desktop entry's icon into RGBA pixels.
pub fn load_icon_rgba(entry: &AppEntry) -> Option<IconPixels> {
    let path = resolve_icon_file(&entry.icon)?;
    let bytes = fs::read(path).ok()?;
    let rgba = image::load_from_memory(&bytes)
        .ok()?
        .thumbnail(128, 128)
        .to_rgba8();
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;
    Some((width, height, rgba.into_raw()))
}

fn resolve_icon_file(icon: &str) -> Option<PathBuf> {
    let icon = icon.trim();
    if icon.is_empty() {
        return None;
    }

    let supplied_path = Path::new(icon);
    let file_name = supplied_path.file_name()?.to_str()?;
    let stem = supplied_path.file_stem()?.to_str()?;
    let mut file_names = Vec::new();
    if matches!(
        supplied_path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("png" | "jpg" | "jpeg")
    ) {
        file_names.push(file_name.to_owned());
    }
    for extension in ["png", "jpg", "jpeg"] {
        let candidate = format!("{stem}.{extension}");
        if !file_names.contains(&candidate) {
            file_names.push(candidate);
        }
    }

    if supplied_path.is_file()
        && matches!(
            supplied_path
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("png" | "jpg" | "jpeg")
        )
    {
        return Some(supplied_path.to_path_buf());
    }

    let mut data_dirs = Vec::new();
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
        data_dirs.push(PathBuf::from(data_home));
    } else if let Some(home) = std::env::var_os("HOME") {
        data_dirs.push(PathBuf::from(home).join(".local/share"));
    }
    if let Some(data_dirs_env) = std::env::var_os("XDG_DATA_DIRS") {
        data_dirs.extend(std::env::split_paths(&data_dirs_env));
    } else {
        data_dirs.extend([
            PathBuf::from("/usr/local/share"),
            PathBuf::from("/usr/share"),
        ]);
    }

    let mut icon_roots: Vec<PathBuf> = data_dirs.iter().map(|dir| dir.join("icons")).collect();
    if let Some(home) = std::env::var_os("HOME") {
        icon_roots.push(PathBuf::from(home).join(".icons"));
    }

    for data_dir in &data_dirs {
        for name in &file_names {
            let candidate = data_dir.join("pixmaps").join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    const THEMES: &[&str] = &["hicolor", "Adwaita", "gnome", "breeze", "Papirus", "Yaru"];
    const SIZES: &[&str] = &[
        "256x256", "128x128", "96x96", "64x64", "48x48", "32x32", "24x24", "scalable", "symbolic",
    ];
    const CONTEXTS: &[&str] = &[
        "apps",
        "applications",
        "status",
        "places",
        "devices",
        "mimetypes",
    ];

    for root in icon_roots {
        if !root.is_dir() {
            continue;
        }
        for name in &file_names {
            let direct = root.join(name);
            if direct.is_file() {
                return Some(direct);
            }
        }
        for theme in THEMES {
            for size in SIZES {
                for context in CONTEXTS {
                    for name in &file_names {
                        let candidate = root.join(theme).join(size).join(context).join(name);
                        if candidate.is_file() {
                            return Some(candidate);
                        }
                    }
                }
            }
        }
    }

    None
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
            categories = val
                .split(';')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
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

    let is_system = path.starts_with("/usr") || path.starts_with("/var");

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
        is_system,
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
            is_system: true,
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
            is_system: true,
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
            is_system: true,
        },
        AppEntry {
            name: "Terminal".to_string(),
            comment: "Open a terminal".to_string(),
            exec: "x-terminal-emulator".to_string(),
            icon: "utilities-terminal".to_string(),
            categories: vec!["System".to_string()],
            tags: vec![
                "terminal".to_string(),
                "console".to_string(),
                "shell".to_string(),
            ],
            desktop_file: PathBuf::new(),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
            is_system: true,
        },
    ]
}
