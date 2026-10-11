//! The live application search: a port of
//! `AppProvider.requestResults` + `QuerySearcher.addResults` +
//! `Searcher.onPostExecute`.
//!
//! Upstream: Neamar/KISS @ `17fc7ebd473bbcbfc0e93592859278443cdb4ec0`.
//!
//! ```text
//! QuerySearcher.doInBackground        -> knownIds = DBHelper.getHistory(.., maxResults, mode, query)
//!   DataHandler.requestResults        -> every provider
//!     AppProvider.requestResults      -> normalize query once, build FuzzyScore once,
//!                                        loop pojos, match name then tags
//!       QuerySearcher.addResults      -> disabled penalty / history boost
//!         Searcher.addResults         -> PriorityQueue<Pojo>(RelevanceComparator)
//! Searcher.onPostExecute              -> poll away the overflow, drain, hand to the adapter
//! ```
//!
//! ## What stays out of the candidate loop
//!
//! Upstream can afford a `HashMap<String, Integer>` lookup per pojo because
//! the JIT and the GC are doing other work anyway; RISS cannot, so the two
//! string-keyed lookups (`knownIds.get(pojo.id)` and
//! `excludedFavoriteIds.contains(pojo.getFavoriteId())`) are resolved into
//! position-indexed arrays **once per search**, before the loop. The result is
//! identical and the loop touches no hash map, allocates nothing, and creates
//! no `String`.

use super::comparator::{display_compare, NameKey};
use super::fuzzy::{FuzzyScoreV2, FuzzyScratch};
use super::index::AppIndex;
use super::normalizer::{normalize_into, Normalized};
use super::prefs::HistoryMode;

/// One surviving candidate: a stable position into [`AppIndex::items`] plus
/// the `Pojo.relevance` it ended up with. Nothing is cloned to build this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scored {
    pub index: u32,
    pub relevance: i32,
}

/// Everything a single search needs, resolved from settings and history on the
/// UI thread and then owned by the worker.
#[derive(Debug, Clone, Default)]
pub struct SearchParams {
    /// The raw query, already `trim`ed the way `Searcher`'s constructor trims.
    pub query: String,
    /// `QuerySearcher.getMaxResultCount()`.
    pub max_results: usize,
    /// `history-mode`.
    pub history_mode: HistoryMode,
    /// `enable-excluded-apps`.
    pub enable_excluded_apps: bool,
    /// `knownIds`: ids previously launched *for this query*, with
    /// `DBHelper.readCursor`'s relevance (`count - position`, so the best
    /// record carries `knownIds.size()`).
    pub known_ids: Vec<(String, i32)>,
    /// `DataHandler.getExcludedFavorites()`.
    pub excluded_favorite_ids: Vec<String>,
}

/// `QuerySearcher.addResults`' boost, split out so it can be tested on its own
/// and so the loop can read it from an array.
///
/// ```java
/// Integer value = knownIds.get(pojo.id);
/// if (value != null) {
///     if (historyMode != HistoryMode.ALPHABETICALLY)
///         pojo.relevance += Math.max(0, 100 - Math.max(knownIds.size() - value, 0) * 5);
///     else
///         pojo.relevance += 100;
/// }
/// ```
/// (upstream 91ec95a57, "improve ordering", halved this from 200/10 to 100/5.)
pub fn history_boost(known_ids_size: usize, value: i32, mode: HistoryMode) -> i32 {
    if mode == HistoryMode::Alphabetically {
        100
    } else {
        let behind = (known_ids_size as i32 - value).max(0);
        (100 - behind * 5).max(0)
    }
}

/// `Pojo.relevance -= 200` for a disabled entry.
pub const DISABLED_PENALTY: i32 = 200;

/// The worker-owned, reused state of one searcher. Allocations happen when a
/// buffer first grows and then never again.
pub struct Searcher {
    scorer: FuzzyScoreV2,
    scratch: FuzzyScratch,
    query_normalized: Normalized,
    /// Per-position history boost, rebuilt per search.
    boosts: Vec<i32>,
    /// Per-position "this favourite is excluded from the results", per search.
    favorite_excluded: Vec<bool>,
    /// The surviving candidates. Reused across searches.
    hits: Vec<Scored>,
}

impl Default for Searcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Searcher {
    pub fn new() -> Self {
        Self {
            scorer: FuzzyScoreV2::new(&[]),
            scratch: FuzzyScratch::new(),
            query_normalized: Normalized::EMPTY,
            boosts: Vec::new(),
            favorite_excluded: Vec::new(),
            hits: Vec::new(),
        }
    }

    /// Run one search.
    ///
    /// `should_stop` is polled every [`CANCEL_CHECK_INTERVAL`] candidates and
    /// is the cooperative half of cancellation: a superseded search abandons
    /// its work instead of finishing it and being thrown away.
    ///
    /// Returns `None` when the search was cancelled, so the caller can tell
    /// "no results" from "never finished" and publish nothing in the second
    /// case.
    pub fn search<F: FnMut() -> bool>(
        &mut self,
        params: &SearchParams,
        index: &AppIndex,
        mut should_stop: F,
    ) -> Option<&[Scored]> {
        self.hits.clear();

        // --- once per search -------------------------------------------------
        // `StringNormalizer.normalizeWithResult(query, false)`, then
        // "if (queryNormalized.codePoints.length == 0) return;".
        normalize_into(params.query.trim(), false, &mut self.query_normalized);
        if self.query_normalized.is_empty() {
            return Some(&self.hits);
        }
        self.scorer.set_pattern(&self.query_normalized.code_points);
        self.scratch.reserve(self.query_normalized.len().max(32));

        let n = index.len();
        self.boosts.clear();
        self.boosts.resize(n, 0);
        self.favorite_excluded.clear();
        self.favorite_excluded.resize(n, false);

        let known_ids_size = params.known_ids.len();
        for (id, value) in &params.known_ids {
            if let Some(pos) = index.position_of(id) {
                self.boosts[pos as usize] =
                    history_boost(known_ids_size, *value, params.history_mode);
            }
        }
        for id in &params.excluded_favorite_ids {
            if let Some(pos) = index.position_of(id) {
                self.favorite_excluded[pos as usize] = true;
            }
        }

        // --- the candidate loop ---------------------------------------------
        // Allocation-free from here to the sort: no String, no Vec, no
        // HashMap, no to_lowercase, no clone.
        let enable_excluded = params.enable_excluded_apps;
        for (i, item) in index.items.iter().enumerate() {
            if i % CANCEL_CHECK_INTERVAL == 0 && should_stop() {
                return None;
            }

            // "exclude apps from results"
            if item.excluded && !enable_excluded {
                continue;
            }
            // "exclude favorites from results"
            if self.favorite_excluded[i] {
                continue;
            }

            // Pojo.updateMatchingRelevance(matchInfo, false) on the name...
            let info = self
                .scorer
                .match_code_points(&item.normalized_name.code_points, &mut self.scratch);
            let mut matched = info.matched;
            let mut relevance = if info.matched { info.score } else { 0 };

            // ... then updateMatchingRelevance(matchInfo, match) on the tags,
            // which only raises the relevance if the tag match scores higher.
            if let Some(tags) = &item.normalized_tags {
                let info = self
                    .scorer
                    .match_code_points(&tags.code_points, &mut self.scratch);
                if info.matched && (!matched || info.score > relevance) {
                    relevance = info.score;
                    matched = true;
                }
            }

            if !matched {
                continue;
            }

            // QuerySearcher.addResults: the two branches are exclusive, a
            // disabled entry never also collects the history boost.
            if item.disabled {
                relevance -= DISABLED_PENALTY;
            } else {
                relevance += self.boosts[i];
            }

            self.hits.push(Scored {
                index: i as u32,
                relevance,
            });
        }

        if should_stop() {
            return None;
        }

        // --- Searcher.onPostExecute -----------------------------------------
        // Upstream collects everything and *then* trims: DEFAULT_MAX_RESULTS
        // is the PriorityQueue's initial capacity, not a bound. Draining a
        // min-heap and reading the result list from the bottom of a ListView
        // is the same as sorting by the comparator descending.
        // `sort_unstable_by`, not `sort_by`: Rust's stable sort allocates a
        // scratch buffer, and upstream has no stability to preserve anyway --
        // `PriorityQueue.poll()` makes no promise about the order of
        // comparator-equal elements. Unstable is both the faithful choice and
        // the allocation-free one.
        let items = &index.items;
        self.hits.sort_unstable_by(|a, b| {
            let ia = &items[a.index as usize];
            let ib = &items[b.index as usize];
            display_compare(
                a.relevance,
                &NameKey {
                    normalized_name: Some(&ia.normalized_name),
                    name: Some(&ia.entry.name),
                    user_handle: None,
                },
                b.relevance,
                &NameKey {
                    normalized_name: Some(&ib.normalized_name),
                    name: Some(&ib.entry.name),
                    user_handle: None,
                },
            )
        });
        self.hits.truncate(params.max_results);
        Some(&self.hits)
    }
}

/// How often the candidate loop asks whether it has been superseded. Small
/// enough that a cancelled search dies within microseconds, large enough that
/// the atomic load is not part of the per-candidate cost.
pub const CANCEL_CHECK_INTERVAL: usize = 64;

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "android")]
    use crate::android_app_entry::AppEntry;
    #[cfg(not(target_os = "android"))]
    use crate::app_entry::AppEntry;
    use crate::kiss::index::AppIndex;
    use std::path::PathBuf;

    fn app(name: &str) -> AppEntry {
        AppEntry {
            name: name.to_string(),
            comment: String::new(),
            exec: name.to_lowercase(),
            icon: String::new(),
            categories: Vec::new(),
            tags: Vec::new(),
            desktop_file: PathBuf::from("/x.desktop"),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
            is_system: false,
        }
    }

    fn index_of(names: &[&str]) -> AppIndex {
        AppIndex::build(
            1,
            names.iter().map(|n| app(n)).collect(),
            |_| false,
            |_| false,
        )
    }

    fn run(s: &mut Searcher, params: &SearchParams, index: &AppIndex) -> Vec<(String, i32)> {
        s.search(params, index, || false)
            .unwrap()
            .iter()
            .map(|h| {
                (
                    index.items[h.index as usize].entry.name.clone(),
                    h.relevance,
                )
            })
            .collect()
    }

    fn params(query: &str, max: usize) -> SearchParams {
        SearchParams {
            query: query.to_string(),
            max_results: max,
            ..Default::default()
        }
    }

    #[test]
    fn empty_query_yields_nothing() {
        let index = index_of(&["Firefox"]);
        let mut s = Searcher::new();
        assert!(run(&mut s, &params("", 20), &index).is_empty());
        assert!(run(&mut s, &params("   ", 20), &index).is_empty());
        // A query that normalizes away (only dashes) is also empty: upstream
        // returns before building the scorer.
        assert!(run(&mut s, &params("---", 20), &index).is_empty());
    }

    #[test]
    fn accented_names_are_found_by_unaccented_queries() {
        let index = index_of(&["Café", "Nautilus"]);
        let mut s = Searcher::new();
        let hits = run(&mut s, &params("cafe", 20), &index);
        assert_eq!(hits.first().map(|h| h.0.as_str()), Some("Café"));
    }

    #[test]
    fn dashes_are_ignored_on_both_sides() {
        let index = index_of(&["Visual-Studio", "Nautilus"]);
        let mut s = Searcher::new();
        assert_eq!(
            run(&mut s, &params("visualstudio", 20), &index)
                .first()
                .map(|h| h.0.clone()),
            Some("Visual-Studio".to_string())
        );
        assert_eq!(
            run(&mut s, &params("visual-studio", 20), &index)
                .first()
                .map(|h| h.0.clone()),
            Some("Visual-Studio".to_string())
        );
    }

    #[test]
    fn equal_scores_order_by_name_ascending() {
        // Same length, same match positions, same neighbours: a real tie,
        // not an assumed one. The comparator is then the only thing left.
        let index = index_of(&["Ab Zulu", "Ab Alfa"]);
        let mut s = Searcher::new();
        let hits = run(&mut s, &params("ab", 20), &index);
        assert_eq!(hits[0].1, hits[1].1, "the premise is an exact tie");
        assert_eq!(hits[0].0, "Ab Alfa");
        assert_eq!(hits[1].0, "Ab Zulu");

        // ... and the input order must not change the answer.
        let index = index_of(&["Ab Alfa", "Ab Zulu"]);
        let hits = run(&mut s, &params("ab", 20), &index);
        assert_eq!(hits[0].0, "Ab Alfa");
        assert_eq!(hits[1].0, "Ab Zulu");
    }

    #[test]
    fn results_are_truncated_at_the_max_result_count() {
        let names: Vec<String> = (0..40).map(|i| format!("App {i:02}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let index = index_of(&refs);
        let mut s = Searcher::new();
        assert_eq!(run(&mut s, &params("app", 7), &index).len(), 7);
        assert_eq!(run(&mut s, &params("app", 40), &index).len(), 40);
        // A stored 0 really means zero rows, as upstream's trim loop does.
        assert_eq!(run(&mut s, &params("app", 0), &index).len(), 0);
    }

    #[test]
    fn truncation_keeps_the_best_not_the_first() {
        let index = index_of(&["Zebra Fish", "Fish"]);
        let mut s = Searcher::new();
        let hits = run(&mut s, &params("fish", 1), &index);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "Fish", "the higher relevance must survive");
    }

    #[test]
    fn excluded_apps_are_hidden_unless_the_preference_is_on() {
        let index = AppIndex::build(
            1,
            vec![app("Firefox"), app("Fossil")],
            |e| e.name == "Fossil",
            |_| false,
        );
        let mut s = Searcher::new();
        let hits = run(&mut s, &params("f", 20), &index);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "Firefox");

        let mut p = params("f", 20);
        p.enable_excluded_apps = true;
        assert_eq!(run(&mut s, &p, &index).len(), 2);
    }

    #[test]
    fn disabled_apps_are_penalised_not_hidden() {
        let index = AppIndex::build(
            1,
            vec![app("Clock"), app("Clockwork")],
            |_| false,
            |e| e.name == "Clock",
        );
        let mut s = Searcher::new();
        let hits = run(&mut s, &params("clock", 20), &index);
        assert_eq!(hits.len(), 2, "disabled entries still appear");
        assert_eq!(hits[0].0, "Clockwork", "the penalised one sinks");
        // Exactly -200 relative to the same match without the flag.
        let enabled = AppIndex::build(1, vec![app("Clock")], |_| false, |_| false);
        let base = run(&mut s, &params("clock", 20), &enabled)[0].1;
        let penalised = hits.iter().find(|h| h.0 == "Clock").unwrap().1;
        assert_eq!(base - penalised, DISABLED_PENALTY);
    }

    #[test]
    fn history_boost_matches_the_java_arithmetic() {
        // The best record carries relevance == knownIds.size() -> +100.
        assert_eq!(history_boost(3, 3, HistoryMode::Recency), 100);
        assert_eq!(history_boost(3, 2, HistoryMode::Recency), 95);
        assert_eq!(history_boost(3, 1, HistoryMode::Recency), 90);
        // Never negative, and never above 100.
        assert_eq!(history_boost(50, 1, HistoryMode::Recency), 0);
        assert_eq!(history_boost(3, 99, HistoryMode::Recency), 100);
        // ALPHABETICALLY is a flat +100.
        assert_eq!(history_boost(50, 1, HistoryMode::Alphabetically), 100);
    }

    #[test]
    fn history_lifts_a_previously_chosen_app_for_this_query() {
        let index = index_of(&["Firefox", "Files"]);
        let mut s = Searcher::new();
        let before = run(&mut s, &params("fi", 20), &index);
        assert_eq!(before[0].0, "Files");

        let mut p = params("fi", 20);
        p.known_ids = vec![("firefox".to_string(), 1)];
        let after = run(&mut s, &p, &index);
        assert_eq!(after[0].0, "Firefox", "the remembered choice wins");
        assert_eq!(after[0].1 - before[1].1, 100);
    }

    #[test]
    fn a_disabled_app_never_also_collects_the_history_boost() {
        // The Java branches are exclusive; a port with two `if`s would give
        // this app -100 instead of -200.
        let index = AppIndex::build(1, vec![app("Clock")], |_| false, |_| true);
        let plain = AppIndex::build(1, vec![app("Clock")], |_| false, |_| false);
        let mut s = Searcher::new();
        let mut p = params("clock", 20);
        p.known_ids = vec![("clock".to_string(), 1)];
        let base = run(&mut s, &params("clock", 20), &plain)[0].1;
        let got = run(&mut s, &p, &index)[0].1;
        assert_eq!(base - got, DISABLED_PENALTY);
    }

    #[test]
    fn excluded_favorites_are_dropped() {
        let index = index_of(&["Firefox", "Files"]);
        let mut s = Searcher::new();
        let mut p = params("fi", 20);
        p.excluded_favorite_ids = vec!["files".to_string()];
        let hits = run(&mut s, &p, &index);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "Firefox");
    }

    #[test]
    fn tags_are_matched_and_only_raise_the_relevance() {
        let mut tagged = app("Nautilus");
        tagged.tags = vec!["files".to_string()];
        let index = AppIndex::build(1, vec![tagged, app("Firefox")], |_| false, |_| false);
        let mut s = Searcher::new();
        let hits = run(&mut s, &params("files", 20), &index);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "Nautilus", "matched through its tag");

        // A tag that matches worse than the name must not lower the score.
        let mut both = app("Files");
        both.tags = vec!["zzz files zzz".to_string()];
        let index = AppIndex::build(1, vec![both], |_| false, |_| false);
        let name_only = index_of(&["Files"]);
        assert_eq!(
            run(&mut s, &params("files", 20), &index)[0].1,
            run(&mut s, &params("files", 20), &name_only)[0].1
        );
    }

    #[test]
    fn comments_and_categories_are_not_searchable() {
        // KISS's AppProvider matches the name and the tags, nothing else.
        // RISS used to fold the comment and the freedesktop categories into
        // one haystack, which made "utility" match half the drawer.
        let mut e = app("Nautilus");
        e.comment = "Browse the filesystem".to_string();
        e.categories = vec!["Utility".to_string()];
        let index = AppIndex::build(1, vec![e], |_| false, |_| false);
        let mut s = Searcher::new();
        assert!(run(&mut s, &params("filesystem", 20), &index).is_empty());
        assert!(run(&mut s, &params("utility", 20), &index).is_empty());
    }

    #[test]
    fn cancellation_abandons_the_search() {
        let names: Vec<String> = (0..1000).map(|i| format!("App {i}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let index = index_of(&refs);
        let mut s = Searcher::new();
        let p = params("app", 20);
        assert!(s.search(&p, &index, || true).is_none());
        assert!(s.search(&p, &index, || false).is_some());
    }

    #[test]
    fn case_is_preserved_so_camel_case_still_scores() {
        let index = index_of(&["VirtualBox", "Vorbis Tools"]);
        let mut s = Searcher::new();
        let hits = run(&mut s, &params("vb", 20), &index);
        assert_eq!(hits[0].0, "VirtualBox");
    }

    #[test]
    fn multilingual_names_are_searchable() {
        let index = index_of(&["Привет Мир", "日本語入力", "Ελληνικά"]);
        let mut s = Searcher::new();
        assert_eq!(run(&mut s, &params("мир", 20), &index)[0].0, "Привет Мир");
        assert_eq!(run(&mut s, &params("日本", 20), &index)[0].0, "日本語入力");
        assert_eq!(run(&mut s, &params("ελλ", 20), &index)[0].0, "Ελληνικά");
    }
}
