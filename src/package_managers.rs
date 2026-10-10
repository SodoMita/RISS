//! App-specific uninstall for Linux desktops.
//!
//! Deleting an app's `.desktop` file does not uninstall it, so the uninstall
//! action first works out which package manager provided the app and then
//! invokes that manager's own removal command:
//!
//! * Flatpak and Snap apps are recognized by their exported desktop files (or
//!   `flatpak run` / `/snap/bin` exec lines) and removed with
//!   `flatpak uninstall` / `snap remove`.
//! * AppImage binaries and hand-placed desktop files inside the user's own
//!   application directories are removed directly.
//! * Distro packages are traced from the desktop file back to their owning
//!   package with `dpkg`, `rpm`, `pacman`, `apk` or `xbps-query`, then
//!   removed with the matching tool (`apt-get`, `dnf`, `zypper`, …).
//!
//! Privileged removals escalate through `pkexec` when available and
//! fall back to `sudo` inside a terminal emulator.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::app_entry::AppEntry;
use crate::providers;

/// Where Snap exports its desktop files.
const SNAP_DESKTOP_DIR: &str = "/var/lib/snapd/desktop/applications";
/// Path fragment shared by every Flatpak export directory.
const FLATPAK_EXPORTS: &str = "flatpak/exports/share/applications";

/// Distro package managers RISS can ask to remove a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Apt,
    Dnf,
    Yum,
    Zypper,
    Pacman,
    Apk,
    Xbps,
}

impl Backend {
    /// Binary that performs the removal.
    pub fn tool(self) -> &'static str {
        match self {
            Backend::Apt => "apt-get",
            Backend::Dnf => "dnf",
            Backend::Yum => "yum",
            Backend::Zypper => "zypper",
            Backend::Pacman => "pacman",
            Backend::Apk => "apk",
            Backend::Xbps => "xbps-remove",
        }
    }

    /// Human readable name for status messages.
    pub fn label(self) -> &'static str {
        match self {
            Backend::Apt => "APT",
            Backend::Dnf => "DNF",
            Backend::Yum => "YUM",
            Backend::Zypper => "Zypper",
            Backend::Pacman => "Pacman",
            Backend::Apk => "apk",
            Backend::Xbps => "XBPS",
        }
    }

    /// Arguments (after the tool name) removing one package without
    /// interactive prompts; the polkit/`sudo` authentication stands in for
    /// the confirmation because the command runs without a terminal.
    pub fn remove_args(self, package: &str) -> Vec<String> {
        match self {
            Backend::Apt | Backend::Dnf | Backend::Yum | Backend::Zypper => vec![
                "remove".to_string(),
                "-y".to_string(),
                package.to_string(),
            ],
            Backend::Pacman => vec![
                "-R".to_string(),
                "--noconfirm".to_string(),
                package.to_string(),
            ],
            Backend::Apk => vec!["del".to_string(), package.to_string()],
            Backend::Xbps => vec![package.to_string()],
        }
    }
}

/// How an app ended up in the launcher's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Flatpak {
        app_id: String,
        user_install: bool,
    },
    Snap {
        snap: String,
    },
    AppImage {
        binary: PathBuf,
        desktop_file: Option<PathBuf>,
    },
    Distro {
        package: String,
        backend: Backend,
    },
    UserLocal {
        desktop_file: PathBuf,
    },
}

/// What the uninstall action ends up doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Remove these files in-process.
    DeleteFiles(Vec<PathBuf>),
    /// Spawn this command detached from the launcher.
    Spawn(Vec<String>),
    /// Run this command inside a terminal emulator.
    Terminal(Vec<String>),
}

/// A concrete uninstall operation for one app.
pub struct Plan {
    /// User-facing description of the operation, e.g. the command that runs.
    pub summary: String,
    pub action: Action,
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Directories holding desktop files the user owns (hand-installed apps).
fn user_applications_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(data_home).join("applications"));
    }
    if let Some(home) = home_dir() {
        dirs.push(home.join(".local/share/applications"));
    }
    dirs
}

/// Flatpak installation roots owned by the user.
fn flatpak_user_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
        roots.push(PathBuf::from(data_home).join("flatpak"));
    }
    if let Some(home) = home_dir() {
        roots.push(home.join(".local/share/flatpak"));
    }
    roots
}

/// Flatpak app ids are dot-separated segments starting with a letter, with
/// at least three segments (e.g. `org.vlc.VLC`).
fn looks_like_flatpak_id(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    parts.len() >= 3
        && parts.iter().all(|part| {
            let mut chars = part.chars();
            matches!(chars.next(), Some('a'..='z') | Some('A'..='Z') | Some('_'))
                && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        })
}

/// Extract the app id from a `flatpak run …` exec line.
pub fn flatpak_app_id_from_exec(exec: &str) -> Option<String> {
    let mut tokens = exec.split_whitespace();
    let first = tokens.next()?;
    let program = first.rsplit('/').next().unwrap_or(first);
    if program != "flatpak" || tokens.next()? != "run" {
        return None;
    }
    for token in tokens {
        if token.starts_with('-') {
            continue;
        }
        return looks_like_flatpak_id(token).then(|| token.to_string());
    }
    None
}

/// Snap name from one of snapd's exported desktop files, which are named
/// `<snap>_<app>.desktop` under `/var/lib/snapd/desktop/applications`.
pub fn snap_from_desktop_file(path: &Path) -> Option<String> {
    if !path.starts_with(SNAP_DESKTOP_DIR) {
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    Some(stem.split('_').next()?.to_string())
}

/// Snap name from a `/snap/bin/…` exec line (`/snap/bin/<snap>[.<app>]`).
pub fn snap_from_exec(exec: &str) -> Option<String> {
    let first = exec.split_whitespace().next()?;
    let rest = first.strip_prefix("/snap/bin/")?;
    Some(rest.split('.').next()?.to_string())
}

/// Parse `dpkg -S` output into the owning package name, tolerating the
/// multiarch (`pkg:amd64:`) and divergence (`pkg1, pkg2:`) formats.
pub fn parse_dpkg_owner(output: &str, queried_path: &str) -> Option<String> {
    for line in output.lines() {
        let Some((package, path)) = line.split_once(": ") else {
            continue;
        };
        if path.trim() != queried_path {
            continue;
        }
        let package = package.split(',').next()?.trim();
        let package = package.split(':').next()?.trim();
        if package.is_empty() {
            return None;
        }
        return Some(package.to_string());
    }
    None
}

/// Parse `apk info --who-owns` output ("<file> is owned by <pkg>") into the
/// owning package name.
pub fn parse_apk_who_owns(output: &str) -> Option<String> {
    let line = output.lines().find(|line| line.contains(" is owned by "))?;
    line.rsplit(" is owned by ")
        .next()
        .map(str::trim)
        .filter(|package| !package.is_empty())
        .map(str::to_string)
}

fn first_line(output: &str) -> Option<String> {
    output.lines().map(str::trim).find(|line| !line.is_empty()).map(str::to_string)
}

fn run_and_parse(
    tool: &str,
    args: &[&str],
    parse: impl FnOnce(&str) -> Option<String>,
) -> Option<String> {
    let output = Command::new(tool).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse(std::str::from_utf8(&output.stdout).ok()?)
}

/// Which removal frontend matches the installed rpm tooling.
fn rpm_backend() -> Backend {
    if providers::command_exists("dnf") {
        Backend::Dnf
    } else if providers::command_exists("zypper") {
        Backend::Zypper
    } else {
        Backend::Yum
    }
}

/// Ask the local package databases which package owns `path`.
pub fn query_owner(path: &Path) -> Option<(Backend, String)> {
    let path_str = path.to_str()?;
    if providers::command_exists("dpkg") {
        if let Some(package) =
            run_and_parse("dpkg", &["-S", path_str], |out| parse_dpkg_owner(out, path_str))
        {
            return Some((Backend::Apt, package));
        }
    }
    if providers::command_exists("rpm") {
        if let Some(package) = run_and_parse("rpm", &["-qf", path_str], first_line) {
            return Some((rpm_backend(), package));
        }
    }
    if providers::command_exists("pacman") {
        if let Some(package) = run_and_parse("pacman", &["-Qq", "--", path_str], first_line) {
            return Some((Backend::Pacman, package));
        }
    }
    if providers::command_exists("apk") {
        if let Some(package) =
            run_and_parse("apk", &["info", "--who-owns", path_str], parse_apk_who_owns)
        {
            return Some((Backend::Apk, package));
        }
    }
    if providers::command_exists("xbps-query") {
        if let Some(package) = run_and_parse("xbps-query", &["-o", path_str], first_line) {
            return Some((Backend::Xbps, package));
        }
    }
    None
}

/// Walk upward until an existing path is found and report whether it is
/// writable; missing metadata counts as read-only.
fn is_user_writable(path: &Path) -> bool {
    let mut current = path;
    loop {
        if let Ok(meta) = std::fs::metadata(current) {
            return !meta.permissions().readonly();
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return false,
        }
    }
}

/// Work out how the app was installed, if possible.
pub fn resolve_source(entry: &AppEntry) -> Option<Source> {
    resolve_source_with(entry, query_owner)
}

/// Same as [`resolve_source`] with the package-owner lookup injectable for
/// tests.
fn resolve_source_with(
    entry: &AppEntry,
    owner_lookup: impl FnOnce(&Path) -> Option<(Backend, String)>,
) -> Option<Source> {
    if entry.desktop_file.as_os_str().is_empty() {
        return None;
    }
    let canonical = std::fs::canonicalize(&entry.desktop_file)
        .unwrap_or_else(|_| entry.desktop_file.clone());
    let canonical_str = canonical.to_str().unwrap_or_default();

    // Flatpak exports, or an exec line going through `flatpak run`.
    let flatpak_id = flatpak_app_id_from_exec(&entry.exec).or_else(|| {
        if canonical_str.contains(FLATPAK_EXPORTS) {
            canonical.file_stem().and_then(|stem| stem.to_str()).map(str::to_string)
        } else {
            None
        }
    });
    if let Some(app_id) = flatpak_id {
        let user_install = flatpak_user_roots().iter().any(|root| canonical.starts_with(root));
        return Some(Source::Flatpak {
            app_id,
            user_install,
        });
    }

    // Snap exports.
    if let Some(snap) = snap_from_desktop_file(&canonical).or_else(|| snap_from_exec(&entry.exec)) {
        return Some(Source::Snap { snap });
    }

    // AppImage binaries launched directly from disk.
    let exec_binary = entry.exec.split_whitespace().next().unwrap_or_default();
    if exec_binary.ends_with(".AppImage") && Path::new(exec_binary).is_file() {
        let desktop_file = if is_user_writable(&canonical) {
            Some(canonical.clone())
        } else {
            None
        };
        return Some(Source::AppImage {
            binary: PathBuf::from(exec_binary),
            desktop_file,
        });
    }

    // Hand-placed desktop files the user owns.
    if user_applications_dirs().iter().any(|dir| canonical.starts_with(dir))
        && is_user_writable(&canonical)
    {
        return Some(Source::UserLocal {
            desktop_file: canonical,
        });
    }

    // Anything else: ask the distro package database who owns the file.
    let (backend, package) = owner_lookup(&canonical)?;
    Some(Source::Distro { package, backend })
}

fn require_tool(tool: &str) -> Result<(), String> {
    if providers::command_exists(tool) {
        Ok(())
    } else {
        Err(format!("{tool} is not installed"))
    }
}

/// Build the uninstall operation for an app, failing when RISS cannot tell
/// how the app was installed or the required tooling is missing.
pub fn plan(entry: &AppEntry) -> Result<Plan, String> {
    let source = resolve_source(entry).ok_or_else(|| {
        format!(
            "Cannot tell how {} was installed; use your package manager or software center",
            entry.name
        )
    })?;
    match source {
        Source::Flatpak { app_id, user_install } => {
            require_tool("flatpak")?;
            let scope = if user_install { "--user" } else { "--system" };
            let argv = vec![
                "flatpak".to_string(),
                "uninstall".to_string(),
                "-y".to_string(),
                scope.to_string(),
                app_id,
            ];
            Ok(Plan {
                summary: format!("Flatpak: {}", argv.join(" ")),
                action: Action::Spawn(argv),
            })
        }
        Source::Snap { snap } => {
            require_tool("snap")?;
            let mut remove = vec!["snap".to_string(), "remove".to_string(), snap];
            let argv = if providers::command_exists("pkexec") {
                let mut privileged = vec!["pkexec".to_string()];
                privileged.append(&mut remove);
                privileged
            } else {
                remove
            };
            Ok(Plan {
                summary: format!("Snap: {}", argv.join(" ")),
                action: Action::Spawn(argv),
            })
        }
        Source::AppImage { binary, desktop_file } => {
            let mut files = vec![binary.clone()];
            if let Some(desktop_file) = &desktop_file {
                files.push(desktop_file.clone());
            }
            Ok(Plan {
                summary: format!("Delete {}", binary.display()),
                action: Action::DeleteFiles(files),
            })
        }
        Source::UserLocal { desktop_file } => Ok(Plan {
            summary: format!("Remove {}", desktop_file.display()),
            action: Action::DeleteFiles(vec![desktop_file]),
        }),
        Source::Distro { package, backend } => {
            let tool = backend.tool();
            if !providers::command_exists(tool) {
                return Err(format!(
                    "{} is owned by {} but {} is not available",
                    entry.name,
                    backend.label(),
                    tool
                ));
            }
            let mut remove = vec![tool.to_string()];
            remove.extend(backend.remove_args(&package));
            if providers::command_exists("pkexec") {
                let mut argv = vec!["pkexec".to_string()];
                argv.extend(remove);
                return Ok(Plan {
                    summary: format!("{}: {}", backend.label(), argv.join(" ")),
                    action: Action::Spawn(argv),
                });
            }
            if providers::command_exists("sudo") {
                let mut argv = vec!["sudo".to_string()];
                argv.extend(remove);
                return Ok(Plan {
                    summary: format!(
                        "{}: {} (asks for your password in a terminal)",
                        backend.label(),
                        argv.join(" ")
                    ),
                    action: Action::Terminal(argv),
                });
            }
            Err(format!(
                "Removing {} needs administrator rights but neither pkexec nor sudo was found",
                package
            ))
        }
    }
}

/// Carry out a plan. File deletions happen synchronously; package manager
/// commands are spawned detached so the launcher does not block on them.
pub fn execute(plan: &Plan) -> Result<(), String> {
    match &plan.action {
        Action::DeleteFiles(files) => {
            for file in files {
                std::fs::remove_file(file)
                    .map_err(|error| format!("Could not remove {}: {error}", file.display()))?;
            }
            Ok(())
        }
        Action::Spawn(argv) => spawn_detached(argv),
        Action::Terminal(argv) => providers::run_in_terminal(argv),
    }
}

fn spawn_detached(argv: &[String]) -> Result<(), String> {
    let (program, args) = argv.split_first().ok_or_else(|| "Empty command".to_string())?;
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Failed to run {}: {error}", argv.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(desktop_file: &str, exec: &str) -> AppEntry {
        AppEntry {
            name: "Test".to_string(),
            comment: String::new(),
            exec: exec.to_string(),
            icon: String::new(),
            categories: Vec::new(),
            tags: Vec::new(),
            desktop_file: PathBuf::from(desktop_file),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
            is_system: false,
        }
    }

    fn no_owner(_: &Path) -> Option<(Backend, String)> {
        None
    }

    fn home() -> PathBuf {
        PathBuf::from(std::env::var_os("HOME").expect("HOME to be set"))
    }

    #[test]
    fn parses_flatpak_ids_from_exec_lines() {
        assert_eq!(
            flatpak_app_id_from_exec("flatpak run org.vlc.VLC"),
            Some("org.vlc.VLC".to_string())
        );
        assert_eq!(
            flatpak_app_id_from_exec("/usr/bin/flatpak run --branch=stable org.vlc.VLC %U"),
            Some("org.vlc.VLC".to_string())
        );
        assert_eq!(flatpak_app_id_from_exec("firefox %u"), None);
        assert_eq!(flatpak_app_id_from_exec("flatpak list"), None);
        assert_eq!(flatpak_app_id_from_exec("flatpak run not-an-id"), None);
    }

    #[test]
    fn parses_snap_names() {
        assert_eq!(
            snap_from_desktop_file(Path::new(
                "/var/lib/snapd/desktop/applications/firefox_firefox.desktop"
            )),
            Some("firefox".to_string())
        );
        assert_eq!(
            snap_from_desktop_file(Path::new("/usr/share/applications/firefox.desktop")),
            None
        );
        assert_eq!(snap_from_exec("/snap/bin/firefox"), Some("firefox".to_string()));
        assert_eq!(
            snap_from_exec("/snap/bin/firefox.geckodriver"),
            Some("firefox".to_string())
        );
        assert_eq!(snap_from_exec("firefox %u"), None);
    }

    #[test]
    fn parses_dpkg_owner_output() {
        let path = "/usr/share/applications/firefox.desktop";
        assert_eq!(
            parse_dpkg_owner(&format!("firefox: {path}\n"), path),
            Some("firefox".to_string())
        );
        assert_eq!(
            parse_dpkg_owner(&format!("libc6:amd64: {path}\n"), path),
            Some("libc6".to_string())
        );
        assert_eq!(
            parse_dpkg_owner(&format!("foo, bar: {path}\n"), path),
            Some("foo".to_string())
        );
        assert_eq!(
            parse_dpkg_owner("dpkg-query: no path found matching pattern /x\n", path),
            None
        );
    }

    #[test]
    fn parses_apk_who_owns_output() {
        assert_eq!(
            parse_apk_who_owns("usr/share/applications/foo.desktop is owned by foo-1.0-r0\n"),
            Some("foo-1.0-r0".to_string())
        );
        assert_eq!(parse_apk_who_owns("WARNING: opening store\n"), None);
    }

    #[test]
    fn builds_remove_arguments_per_backend() {
        assert_eq!(
            Backend::Apt.remove_args("firefox"),
            vec!["remove".to_string(), "-y".to_string(), "firefox".to_string()]
        );
        assert_eq!(
            Backend::Pacman.remove_args("firefox"),
            vec!["-R".to_string(), "--noconfirm".to_string(), "firefox".to_string()]
        );
        assert_eq!(
            Backend::Apk.remove_args("firefox"),
            vec!["del".to_string(), "firefox".to_string()]
        );
        assert_eq!(Backend::Xbps.remove_args("firefox"), vec!["firefox".to_string()]);
    }

    #[test]
    fn resolves_flatpak_sources() {
        let system = entry(
            "/var/lib/flatpak/exports/share/applications/org.vlc.VLC.desktop",
            "flatpak run org.vlc.VLC",
        );
        assert_eq!(
            resolve_source_with(&system, no_owner),
            Some(Source::Flatpak {
                app_id: "org.vlc.VLC".to_string(),
                user_install: false,
            })
        );

        let user_path = home()
            .join(".local/share/flatpak/exports/share/applications/org.vlc.VLC.desktop");
        let user = entry(user_path.to_str().unwrap(), "flatpak run org.vlc.VLC");
        assert_eq!(
            resolve_source_with(&user, no_owner),
            Some(Source::Flatpak {
                app_id: "org.vlc.VLC".to_string(),
                user_install: true,
            })
        );
    }

    #[test]
    fn resolves_snap_source() {
        let app = entry(
            "/var/lib/snapd/desktop/applications/firefox_firefox.desktop",
            "/snap/bin/firefox",
        );
        assert_eq!(
            resolve_source_with(&app, no_owner),
            Some(Source::Snap {
                snap: "firefox".to_string(),
            })
        );
    }

    #[test]
    fn resolves_appimage_source() {
        let binary = std::env::temp_dir().join("riss-test-app.AppImage");
        std::fs::write(&binary, b"#!/bin/sh\n").expect("temp file write");
        let desktop = home().join(".local/share/applications/riss-test-app.desktop");
        let app = entry(desktop.to_str().unwrap(), binary.to_str().unwrap());
        let source = resolve_source_with(&app, no_owner);
        let _ = std::fs::remove_file(&binary);
        assert_eq!(
            source,
            Some(Source::AppImage {
                binary,
                desktop_file: Some(desktop),
            })
        );
    }

    #[test]
    fn resolves_user_local_source() {
        let desktop = home().join(".local/share/applications/riss-test-local.desktop");
        let app = entry(desktop.to_str().unwrap(), "/opt/some/app");
        assert_eq!(
            resolve_source_with(&app, no_owner),
            Some(Source::UserLocal {
                desktop_file: desktop,
            })
        );
    }

    #[test]
    fn resolves_distro_source_via_owner_lookup() {
        let app = entry("/usr/share/applications/gimp.desktop", "gimp %U");
        let lookup = |_path: &Path| Some((Backend::Apt, "gimp".to_string()));
        assert_eq!(
            resolve_source_with(&app, lookup),
            Some(Source::Distro {
                package: "gimp".to_string(),
                backend: Backend::Apt,
            })
        );
    }

    #[test]
    fn virtual_entries_have_no_source() {
        let app = entry("", "xdg-screensaver lock");
        assert_eq!(resolve_source_with(&app, no_owner), None);
    }
}
