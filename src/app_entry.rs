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
    /// Virtual entries and the built-in shortcuts have nothing to
    /// uninstall; for real apps the feasibility is decided when the
    /// responsible package manager is resolved.
    pub fn can_uninstall(&self) -> bool {
        !self.is_virtual() && !self.desktop_file.as_os_str().is_empty()
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

/// Icon file types RISS can draw, in the order the freedesktop icon theme
/// spec looks them up (png, then svg; xpm is not decoded), plus compressed
/// svgz and the jpeg RISS always accepted.
const ICON_EXTENSIONS: &[&str] = &["png", "svg", "svgz", "jpg", "jpeg"];

/// Edge length, in pixels, of the square texture an icon is decoded to.
const ICON_TEXTURE_PIXELS: u32 = 128;

/// Icon theme directories, best first. `scalable` (SVG) comes right after
/// the sizes that downscale to the 128px texture: an SVG renders crisply at
/// any size, while a 48px PNG has to be blown up and blurs.
const ICON_SIZES: &[&str] = &[
    "256x256", "128x128", "96x96", "64x64", "scalable", "48x48", "32x32", "24x24", "symbolic",
];

const ICON_THEMES: &[&str] = &["hicolor", "Adwaita", "gnome", "breeze", "Papirus", "Yaru"];

const ICON_CONTEXTS: &[&str] = &[
    "apps",
    "applications",
    "status",
    "places",
    "devices",
    "mimetypes",
];

fn extension_is(path: &Path, wanted: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| wanted.iter().any(|w| extension.eq_ignore_ascii_case(w)))
}

/// Load and decode a desktop entry's icon into RGBA pixels.
pub fn load_icon_rgba(entry: &AppEntry) -> Option<IconPixels> {
    decode_icon_file(&resolve_icon_file(&entry.icon)?)
}

/// Decode an icon file into straight-alpha RGBA, at most
/// [`ICON_TEXTURE_PIXELS`] on a side.
fn decode_icon_file(path: &Path) -> Option<IconPixels> {
    let bytes = fs::read(path).ok()?;
    if extension_is(path, &["svg", "svgz"]) {
        return render_svg(&bytes, ICON_TEXTURE_PIXELS);
    }
    let rgba = image::load_from_memory(&bytes)
        .ok()?
        .thumbnail(ICON_TEXTURE_PIXELS, ICON_TEXTURE_PIXELS)
        .to_rgba8();
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;
    Some((width, height, rgba.into_raw()))
}

/// Rasterise an SVG (or gzip-compressed SVGZ) icon into a `size` pixel
/// square, keeping its aspect ratio and centring it.
///
/// Most Linux app icons ship only as SVG (hicolor/scalable, Adwaita,
/// Papirus) and the `image` crate cannot decode SVG, so every such app used
/// to fall back to the letter badge (issue #34).
fn render_svg(bytes: &[u8], size: u32) -> Option<IconPixels> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default()).ok()?;
    let drawing = tree.size();
    let scale = size as f32 / drawing.width().max(drawing.height());
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let offset_x = (size as f32 - drawing.width() * scale) / 2.0;
    let offset_y = (size as f32 - drawing.height() * scale) / 2.0;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    let transform =
        tiny_skia::Transform::from_scale(scale, scale).post_translate(offset_x, offset_y);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    // tiny-skia keeps premultiplied alpha; textures are uploaded as straight
    // alpha (`ColorImage::from_rgba_unmultiplied`), exactly like PNGs.
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    Some((size as usize, size as usize, rgba))
}

fn resolve_icon_file(icon: &str) -> Option<PathBuf> {
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
    find_icon_file(icon, &data_dirs, &icon_roots)
}

/// Find the file for an `Icon=` value: an absolute path, or an icon name
/// looked up in `pixmaps` and the icon theme directories.
fn find_icon_file(icon: &str, data_dirs: &[PathBuf], icon_roots: &[PathBuf]) -> Option<PathBuf> {
    let icon = icon.trim();
    if icon.is_empty() {
        return None;
    }
    let supplied = Path::new(icon);
    if supplied.is_file() && extension_is(supplied, ICON_EXTENSIONS) {
        return Some(supplied.to_path_buf());
    }
    // `Icon=` holds an icon *name*, not a file name: the dots in reverse-DNS
    // names such as org.gnome.Nautilus or org.mozilla.firefox are part of the
    // name. `file_stem` used to cut them to "org.gnome", so no such icon was
    // ever found. Only a real image extension is stripped (some entries write
    // Icon=foo.png anyway).
    let base = supplied.file_name()?.to_str()?;
    let name = match base.rsplit_once('.') {
        Some((stem, extension))
            if !stem.is_empty()
                && (ICON_EXTENSIONS
                    .iter()
                    .any(|e| extension.eq_ignore_ascii_case(e))
                    || extension.eq_ignore_ascii_case("xpm")) =>
        {
            stem
        }
        _ => base,
    };
    let file_names: Vec<String> = ICON_EXTENSIONS
        .iter()
        .map(|extension| format!("{name}.{extension}"))
        .collect();
    let first_in = |dir: &Path| {
        file_names
            .iter()
            .map(|file_name| dir.join(file_name))
            .find(|candidate| candidate.is_file())
    };

    for data_dir in data_dirs {
        if let Some(found) = first_in(&data_dir.join("pixmaps")) {
            return Some(found);
        }
    }
    // Directories are checked before their contents, so an icon that is not
    // installed costs a few dozen stat calls instead of thousands.
    for root in icon_roots {
        if !root.is_dir() {
            continue;
        }
        if let Some(found) = first_in(root) {
            return Some(found);
        }
        for theme in ICON_THEMES {
            let theme_dir = root.join(theme);
            if !theme_dir.is_dir() {
                continue;
            }
            for size in ICON_SIZES {
                let size_dir = theme_dir.join(size);
                if !size_dir.is_dir() {
                    continue;
                }
                for context in ICON_CONTEXTS {
                    if let Some(found) = first_in(&size_dir.join(context)) {
                        return Some(found);
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

#[cfg(test)]
mod icon_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static UNIQUE: AtomicUsize = AtomicUsize::new(0);

    /// A fresh `share` directory standing in for an XDG data dir.
    fn share_dir(tag: &str) -> PathBuf {
        let n = UNIQUE.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "riss-icon-test-{tag}-{}-{n}/share",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp share dir");
        dir
    }

    fn put(path: &Path, contents: &[u8]) -> PathBuf {
        fs::create_dir_all(path.parent().expect("parent")).expect("create icon dir");
        fs::write(path, contents).expect("write icon");
        path.to_path_buf()
    }

    fn find(icon: &str, share: &Path) -> Option<PathBuf> {
        find_icon_file(icon, &[share.to_path_buf()], &[share.join("icons")])
    }

    fn pixel(icon: &IconPixels, x: usize, y: usize) -> [u8; 4] {
        let (width, _, rgba) = icon;
        let at = (y * width + x) * 4;
        [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
    }

    /// 16x16 drawing, a half-transparent red square in the middle.
    const HALF_RED: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect x="4" y="4" width="8" height="8" fill="#ff0000" fill-opacity="0.5"/></svg>"##;
    /// 2:1 drawing, filled edge to edge with opaque blue.
    const WIDE_BLUE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect width="200" height="100" fill="#0000ff"/></svg>"##;

    #[test]
    fn reverse_dns_icon_names_keep_their_dots() {
        let share = share_dir("rdns");
        let png = put(
            &share.join("icons/hicolor/48x48/apps/org.gnome.Nautilus.png"),
            b"png",
        );
        assert_eq!(find("org.gnome.Nautilus", &share), Some(png));
    }

    #[test]
    fn svg_only_icons_are_found() {
        let share = share_dir("svg-only");
        let svg = put(
            &share.join("icons/hicolor/scalable/apps/org.gnome.Settings.svg"),
            HALF_RED,
        );
        assert_eq!(find("org.gnome.Settings", &share), Some(svg));
        let pixmap = put(&share.join("pixmaps/legacy-app.svg"), HALF_RED);
        assert_eq!(find("legacy-app", &share), Some(pixmap));
    }

    #[test]
    fn scalable_beats_small_pngs_but_not_big_ones() {
        let share = share_dir("sizes");
        let apps = share.join("icons/hicolor");
        put(&apps.join("48x48/apps/editor.png"), b"png");
        let svg = put(&apps.join("scalable/apps/editor.svg"), HALF_RED);
        assert_eq!(find("editor", &share), Some(svg), "48px PNG would blur");
        let big = put(&apps.join("128x128/apps/editor.png"), b"png");
        assert_eq!(find("editor", &share), Some(big), "128px PNG is enough");
    }

    #[test]
    fn only_real_image_extensions_are_stripped() {
        let share = share_dir("ext");
        let png = put(&share.join("icons/hicolor/48x48/apps/tool.png"), b"png");
        assert_eq!(find("tool.png", &share), Some(png.clone()));
        assert_eq!(find("tool.xpm", &share), Some(png), "xpm is not drawable");
        assert_eq!(find("tool.unknown", &share), None);
    }

    #[test]
    fn svg_renders_with_straight_alpha() {
        let icon = render_svg(HALF_RED, ICON_TEXTURE_PIXELS).expect("renders");
        assert_eq!((icon.0, icon.1), (128, 128));
        let [r, g, b, a] = pixel(&icon, 64, 64);
        // Premultiplied output would read about (128, 0, 0, 128): a dark
        // fringe on every antialiased edge once egui multiplies again.
        assert_eq!((r, g, b), (255, 0, 0));
        assert!((126..=130).contains(&a), "alpha {a}");
        assert_eq!(pixel(&icon, 2, 2)[3], 0, "outside the drawing is clear");
    }

    #[test]
    fn non_square_svg_is_centred() {
        let icon = render_svg(WIDE_BLUE, ICON_TEXTURE_PIXELS).expect("renders");
        assert_eq!(pixel(&icon, 64, 64), [0, 0, 255, 255]);
        assert_eq!(pixel(&icon, 1, 64), [0, 0, 255, 255], "full width");
        assert_eq!(pixel(&icon, 64, 10)[3], 0, "letterboxed above");
        assert_eq!(pixel(&icon, 64, 117)[3], 0, "letterboxed below");
    }

    #[test]
    fn an_absolute_svg_icon_path_is_drawn() {
        let share = share_dir("abs");
        let path = put(&share.join("opt/app/icon.svg"), HALF_RED);
        let mut entry = AppEntry::virtual_entry("App", "", "app");
        entry.icon = path.to_string_lossy().into_owned();
        let icon = load_icon_rgba(&entry).expect("absolute .svg icon loads");
        assert_eq!(pixel(&icon, 64, 64)[0], 255);
    }

    #[test]
    fn broken_svg_falls_back_instead_of_panicking() {
        assert!(render_svg(b"<svg", ICON_TEXTURE_PIXELS).is_none());
        assert!(render_svg(b"not xml at all", ICON_TEXTURE_PIXELS).is_none());
    }
}
