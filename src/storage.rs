//! Shared persistence for `settings.json` and `history.json`.
//!
//! On desktop the files live in `~/.config/riss-launcher/`, matching earlier
//! versions. On Android the process working directory is not usable, so the
//! app-private directory reported by the native activity (the equivalent of
//! `Context.getFilesDir()`) is used instead.
//!
//! Writes are atomic (temp file + rename). Read and write errors are reported
//! to the caller instead of being silently discarded, and files that fail to
//! parse are quarantined next to the original so user data is never lost.

use serde::de::DeserializeOwned;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SETTINGS_FILE: &str = "settings.json";
pub const HISTORY_FILE: &str = "history.json";

/// Environment variable that overrides the data directory. Primarily meant
/// for tests and portable setups.
pub const DATA_DIR_ENV: &str = "RISS_DATA_DIR";

/// Directory that holds `settings.json` and `history.json`.
pub fn data_dir() -> PathBuf {
    match std::env::var(DATA_DIR_ENV) {
        Ok(dir) if !dir.trim().is_empty() => PathBuf::from(dir),
        _ => platform_data_dir(),
    }
}

#[cfg(target_os = "android")]
fn platform_data_dir() -> PathBuf {
    match crate::android_app_entry::android_jni::internal_data_path() {
        Some(dir) => dir,
        None => {
            log::error!(
                "Android internal data path unavailable; \
                 falling back to the working directory"
            );
            PathBuf::from(".")
        }
    }
}

#[cfg(not(target_os = "android"))]
fn platform_data_dir() -> PathBuf {
    let mut dir = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    dir.push(".config");
    dir.push("riss-launcher");
    dir
}

/// Canonical path for a persisted file.
pub fn file_path(name: &str) -> PathBuf {
    data_dir().join(name)
}

/// Directories where earlier versions stored files, used for one-time
/// migration into [`data_dir`].
fn legacy_dirs() -> Vec<PathBuf> {
    #[cfg(target_os = "android")]
    {
        // Before storage was fixed, files were written relative to the
        // process working directory (on the rare devices where that
        // happened to be writable).
        vec![PathBuf::from(".")]
    }
    #[cfg(not(target_os = "android"))]
    {
        Vec::new()
    }
}

/// Move a legacy copy of `name` into the canonical data directory if the
/// canonical file does not exist yet. Returns the legacy path it moved.
pub fn migrate(name: &str) -> Option<PathBuf> {
    migrate_between(&legacy_dirs(), &data_dir(), name)
}

fn migrate_between(legacy_dirs: &[PathBuf], dir: &Path, name: &str) -> Option<PathBuf> {
    let target = dir.join(name);
    if target.exists() {
        return None;
    }
    for legacy in legacy_dirs.iter().map(|legacy_dir| legacy_dir.join(name)) {
        if !legacy.is_file() || legacy == target {
            continue;
        }
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::rename(&legacy, &target).is_ok() {
            log::info!("Migrated {} to {}", legacy.display(), target.display());
            return Some(legacy);
        }
        // Renaming fails across file systems; copy instead and leave the
        // original in place.
        if let Ok(contents) = fs::read(&legacy) {
            if fs::write(&target, &contents).is_ok() {
                log::info!("Copied {} to {}", legacy.display(), target.display());
                return Some(legacy);
            }
        }
    }
    None
}

/// What happened while loading a persisted file.
#[derive(Debug)]
pub enum LoadOutcome<T> {
    /// The file existed and parsed successfully.
    Loaded(T),
    /// No file exists yet (first launch).
    Missing,
    /// The file existed but was not valid JSON. The original content is kept
    /// in a quarantine copy when `backup` is `Some`.
    Corrupt { backup: Option<PathBuf> },
    /// The file exists but could not be read.
    Unreadable(io::Error),
}

impl<T> LoadOutcome<T> {
    /// Collapse the outcome into a value, collecting human-readable notes
    /// for everything that did not load cleanly.
    pub fn into_value(self, name: &str, default: T) -> (T, Vec<String>) {
        match self {
            LoadOutcome::Loaded(value) => (value, Vec::new()),
            LoadOutcome::Missing => (default, Vec::new()),
            LoadOutcome::Corrupt { backup } => {
                let note = match backup {
                    Some(path) => format!(
                        "{name} was unreadable and has been reset; \
                         the original was kept at {}",
                        path.display()
                    ),
                    None => format!("{name} was unreadable and has been reset"),
                };
                log::error!("{note}");
                (default, vec![note])
            }
            LoadOutcome::Unreadable(err) => {
                let note = format!("Could not read {name}: {err}");
                log::error!("{note}");
                (default, vec![note])
            }
        }
    }
}

/// Load and parse `name` from the canonical data directory, migrating any
/// legacy copy first.
pub fn load<T: DeserializeOwned>(name: &str) -> LoadOutcome<T> {
    migrate(name);
    load_from(&data_dir(), name)
}

fn load_from<T: DeserializeOwned>(dir: &Path, name: &str) -> LoadOutcome<T> {
    let path = dir.join(name);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return LoadOutcome::Missing,
        Err(err) => return LoadOutcome::Unreadable(err),
    };
    match serde_json::from_str::<T>(&text) {
        Ok(value) => LoadOutcome::Loaded(value),
        Err(_) => {
            let backup = quarantine(&path);
            LoadOutcome::Corrupt { backup }
        }
    }
}

/// Rename a corrupt file out of the way so it is preserved but no longer
/// blocks a fresh start.
fn quarantine(path: &Path) -> Option<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let file_name = path.file_name()?.to_str()?;
    let backup = path.with_file_name(format!("{file_name}.corrupt-{stamp}"));
    match fs::rename(path, &backup) {
        Ok(()) => Some(backup),
        Err(_) => None,
    }
}

/// Write `json` into `name` inside the canonical data directory, atomically.
pub fn save(name: &str, json: &str) -> io::Result<()> {
    save_to(&data_dir(), name, json)
}

fn save_to(dir: &Path, name: &str, json: &str) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(name);
    let tmp = dir.join(format!("{name}.tmp"));
    fs::write(&tmp, json)?;
    if let Err(rename_err) = fs::rename(&tmp, &path) {
        // Renaming can fail on exotic file systems; fall back to writing in
        // place, then clean up the temp file either way.
        let fallback = fs::write(&path, json);
        let _ = fs::remove_file(&tmp);
        fallback.map_err(|_| rename_err)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static UNIQUE: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir(tag: &str) -> PathBuf {
        let count = UNIQUE.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "riss-storage-test-{tag}-{}-{count}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[derive(Debug, Deserialize)]
    struct Doc {
        value: u32,
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = temp_dir("roundtrip");
        save_to(&dir, "doc.json", "{\"value\": 7}").unwrap();
        match load_from::<Doc>(&dir, "doc.json") {
            LoadOutcome::Loaded(doc) => assert_eq!(doc.value, 7),
            other => panic!("expected Loaded, got {other:?}"),
        }
        cleanup(&dir);
    }

    #[test]
    fn missing_file_reports_missing() {
        let dir = temp_dir("missing");
        let outcome = load_from::<Doc>(&dir, "doc.json");
        assert!(matches!(outcome, LoadOutcome::Missing));
        cleanup(&dir);
    }

    #[test]
    fn corrupt_file_is_quarantined() {
        let dir = temp_dir("corrupt");
        fs::write(dir.join("doc.json"), "{ not json").unwrap();
        let backup = match load_from::<Doc>(&dir, "doc.json") {
            LoadOutcome::Corrupt { backup } => backup.expect("quarantine copy"),
            other => panic!("expected Corrupt, got {other:?}"),
        };
        assert!(backup.exists());
        assert!(fs::read_to_string(&backup).unwrap().contains("not json"));
        assert!(!dir.join("doc.json").exists());
        cleanup(&dir);
    }

    #[test]
    fn save_creates_missing_directories() {
        let base = temp_dir("nested");
        let dir = base.join("deeper");
        save_to(&dir, "doc.json", "{\"value\": 1}").unwrap();
        assert!(dir.join("doc.json").exists());
        cleanup(&base);
    }

    #[test]
    fn overwrite_replaces_previous_content() {
        let dir = temp_dir("overwrite");
        save_to(&dir, "doc.json", "{\"value\": 1}").unwrap();
        save_to(&dir, "doc.json", "{\"value\": 2}").unwrap();
        let text = fs::read_to_string(dir.join("doc.json")).unwrap();
        assert_eq!(text, "{\"value\": 2}");
        assert!(!dir.join("doc.json.tmp").exists());
        cleanup(&dir);
    }

    #[test]
    fn legacy_file_is_migrated() {
        let legacy_dir = temp_dir("legacy");
        let dir = temp_dir("canonical");
        fs::write(legacy_dir.join("doc.json"), "{\"value\": 3}").unwrap();
        let migrated = migrate_between(std::slice::from_ref(&legacy_dir), &dir, "doc.json");
        assert_eq!(migrated, Some(legacy_dir.join("doc.json")));
        assert!(dir.join("doc.json").exists());
        assert!(!legacy_dir.join("doc.json").exists());
        cleanup(&legacy_dir);
        cleanup(&dir);
    }

    #[test]
    fn canonical_file_wins_over_legacy() {
        let legacy_dir = temp_dir("legacy-wins");
        let dir = temp_dir("canonical-wins");
        fs::write(legacy_dir.join("doc.json"), "{\"value\": 3}").unwrap();
        fs::write(dir.join("doc.json"), "{\"value\": 9}").unwrap();
        assert_eq!(
            migrate_between(std::slice::from_ref(&legacy_dir), &dir, "doc.json"),
            None
        );
        let text = fs::read_to_string(dir.join("doc.json")).unwrap();
        assert!(text.contains('9'));
        cleanup(&legacy_dir);
        cleanup(&dir);
    }

    #[test]
    fn corrupt_history_falls_back_to_default() {
        let dir = temp_dir("history-corrupt");
        fs::write(dir.join(HISTORY_FILE), "not json at all").unwrap();
        let outcome = load_from::<crate::history::HistoryData>(&dir, HISTORY_FILE);
        let (history, notes) = outcome.into_value(HISTORY_FILE, Default::default());
        assert!(history.launch_counts.is_empty());
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("unreadable"));
        // The corrupt original is preserved next to the fresh file.
        let backups: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains("corrupt"))
            .collect();
        assert_eq!(backups.len(), 1);
        cleanup(&dir);
    }
}
