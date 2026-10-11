//! The loaded application data the search runs over.
//!
//! This is RISS's stand-in for the `List<AppPojo>` that
//! `DataHandler.getApplicationsWithoutExcluded()` hands to `AppProvider`.
//!
//! Upstream normalizes a pojo's name **once**, in `Pojo.setName`, when the
//! provider loads it -- never per keystroke. The same rule applies here: every
//! [`SearchItem`] carries its own `StringNormalizer.Result`, stored next to the
//! app, not in a side map keyed by the display string. A `HashMap<String, _>`
//! lookup per candidate per keystroke is precisely the cost this layout exists
//! to remove.

use super::normalizer::{normalize_into, Normalized};
#[cfg(target_os = "android")]
use crate::android_app_entry::AppEntry;
#[cfg(not(target_os = "android"))]
use crate::app_entry::AppEntry;
use std::collections::HashMap;

/// One searchable application: `AppPojo` plus its pre-normalized text.
#[derive(Debug, Clone)]
pub struct SearchItem {
    /// The entry as the UI knows it. `name` is already the display name
    /// (RISS substitutes a user alias at load time, like `Pojo.setName`).
    pub entry: AppEntry,
    /// `Pojo.normalizedName`, computed once at load.
    pub normalized_name: Normalized,
    /// `PojoWithTags.normalizedTags`. `None` when the app has no tags, which
    /// is how upstream signals "skip the tag match".
    pub normalized_tags: Option<Normalized>,
    /// `AppPojo.isExcluded()`.
    pub excluded: bool,
    /// `AppPojo.isDisabled()` -- an app that is installed but not currently
    /// launchable. Upstream penalises these by 200 instead of hiding them.
    pub disabled: bool,
}

impl SearchItem {
    /// `Pojo.id` / `Pojo.getFavoriteId()`. RISS keys everything by `exec`
    /// (the package name on Android, the desktop `Exec` line on Linux).
    #[inline]
    pub fn id(&self) -> &str {
        &self.entry.exec
    }
}

/// An immutable snapshot of the loaded applications, shared with the search
/// worker behind an `Arc`.
///
/// `generation` is bumped on every rebuild so a result computed against an old
/// snapshot can be recognised and dropped instead of being painted.
#[derive(Debug, Default)]
pub struct AppIndex {
    pub items: Vec<SearchItem>,
    pub generation: u64,
    /// `exec` -> position in `items`. Used **once per search** to resolve the
    /// history boost and the excluded-favourite set into position-indexed
    /// arrays; never consulted inside the candidate loop.
    by_id: HashMap<String, u32>,
}

impl AppIndex {
    pub fn new(generation: u64) -> Self {
        Self {
            items: Vec::new(),
            generation,
            by_id: HashMap::new(),
        }
    }

    /// Build a snapshot from loaded entries.
    ///
    /// `is_excluded` and `is_disabled` are the platform's view of the two
    /// `AppPojo` flags the search path reads.
    pub fn build<F, G>(
        generation: u64,
        entries: Vec<AppEntry>,
        mut is_excluded: F,
        mut is_disabled: G,
    ) -> Self
    where
        F: FnMut(&AppEntry) -> bool,
        G: FnMut(&AppEntry) -> bool,
    {
        let mut items = Vec::with_capacity(entries.len());
        let mut by_id = HashMap::with_capacity(entries.len());
        let mut name_buf = Normalized::EMPTY;
        let mut tag_buf = String::new();

        for entry in entries {
            // `Pojo.setName(name)` -> normalizeWithResult(name, false).
            normalize_into(&entry.name, false, &mut name_buf);
            let normalized_name = name_buf.clone();

            // `PojoWithTags.setTags(tags)`: lower-cased with the default
            // locale first, then normalized with makeLowercase=false.
            let normalized_tags = if entry.tags.is_empty() {
                None
            } else {
                tag_buf.clear();
                for (i, tag) in entry.tags.iter().enumerate() {
                    if i > 0 {
                        tag_buf.push(' ');
                    }
                    tag_buf.push_str(tag);
                }
                let lowered = tag_buf.trim().to_lowercase();
                if lowered.is_empty() {
                    None
                } else {
                    Some(super::normalizer::normalize_with_result(&lowered, false))
                }
            };

            let excluded = is_excluded(&entry);
            let disabled = is_disabled(&entry);
            by_id.insert(entry.exec.clone(), items.len() as u32);
            items.push(SearchItem {
                entry,
                normalized_name,
                normalized_tags,
                excluded,
                disabled,
            });
        }

        Self {
            items,
            generation,
            by_id,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Resolve an id to its position. Called once per history record per
    /// search, never per candidate.
    #[inline]
    pub fn position_of(&self, id: &str) -> Option<u32> {
        self.by_id.get(id).copied()
    }

    pub fn get(&self, index: u32) -> Option<&SearchItem> {
        self.items.get(index as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    pub fn entry(name: &str, exec: &str, tags: &[&str]) -> AppEntry {
        AppEntry {
            name: name.to_string(),
            comment: String::new(),
            exec: exec.to_string(),
            icon: String::new(),
            categories: Vec::new(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            desktop_file: PathBuf::from("/x.desktop"),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
            is_system: false,
        }
    }

    #[test]
    fn names_are_normalized_once_at_load() {
        let index = AppIndex::build(
            1,
            vec![
                entry("Café", "cafe", &[]),
                entry("Visual-Studio", "vs", &[]),
            ],
            |_| false,
            |_| false,
        );
        assert_eq!(index.items[0].normalized_name.to_display_string(), "Cafe");
        assert_eq!(
            index.items[1].normalized_name.to_display_string(),
            "VisualStudio"
        );
        // Case is preserved: the camel bonus depends on it.
        assert_eq!(index.items[1].entry.name, "Visual-Studio");
    }

    #[test]
    fn tags_are_lowercased_then_normalized() {
        let index = AppIndex::build(
            1,
            vec![entry("X", "x", &["Édito", "WEB"])],
            |_| false,
            |_| false,
        );
        let tags = index.items[0].normalized_tags.as_ref().unwrap();
        assert_eq!(tags.to_display_string(), "edito web");
    }

    #[test]
    fn apps_without_tags_have_no_tag_result() {
        let index = AppIndex::build(1, vec![entry("X", "x", &[])], |_| false, |_| false);
        assert!(index.items[0].normalized_tags.is_none());
    }

    #[test]
    fn positions_resolve_by_id() {
        let index = AppIndex::build(
            7,
            vec![entry("A", "a", &[]), entry("B", "b", &[])],
            |_| false,
            |_| false,
        );
        assert_eq!(index.generation, 7);
        assert_eq!(index.position_of("b"), Some(1));
        assert_eq!(index.position_of("missing"), None);
    }
}
