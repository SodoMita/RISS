//! Faithful port of KISS `QuerySearcher`'s relevance adjustments, the
//! `HistoryMode` enum, and `Searcher`'s bounded queue.
//!
//! Upstream (Neamar/KISS):
//! * `searcher/Searcher.java` — `DEFAULT_MAX_RESULTS = 50`; results collect
//!   into a `PriorityQueue(DEFAULT_MAX_RESULTS, new RelevanceComparator())`
//!   and `onPostExecute` evicts with
//!   `while (queue.size() > maxResults) queue.poll();`. It is a min-heap, so
//!   the LOWEST relevance is dropped and the highest survive. Search runs on a
//!   dedicated `SEARCH_THREAD` and is cancellable (`isCancelled()`).
//! * `searcher/QuerySearcher.java` — `addResults` adjusts each pojo before it
//!   enters the queue:
//!   ```java
//!   if (pojo.isDisabled()) { pojo.relevance -= 200; }
//!   else { Integer value = knownIds.get(pojo.id);
//!          if (value != null) {
//!            if (historyMode != HistoryMode.ALPHABETICALLY)
//!              pojo.relevance += Math.max(0, 100 - Math.max(knownIds.size() - value, 0) * 5);
//!            else pojo.relevance += 100; } }
//!   ```
//!   and `getMaxResultCount` reads the `number-of-display-elements` preference
//!   through `Double` first so oversized values cannot throw
//!   `NumberFormatException`, falling back to `DEFAULT_MAX_RESULTS`.
//! * `db/HistoryMode.java` — `valueById` falls back to `RECENCY`.
//!
//! GAP (named, not invented): `knownIds` comes from
//! `DBHelper.getHistory(context, max, historyMode, query)` mapped
//! `record -> relevance`. How `DBHelper` computes those relevance values is NOT
//! ported here; `adjusted_relevance` takes them as given, exactly as
//! `addResults` does. Wiring a real history store into this is the follow-up
//! that makes the query-keyed boost live.

/// KISS `db/HistoryMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryMode {
    Recency,
    Frecency,
    Frequency,
    Adaptive,
    Time,
    Alphabetically,
}

impl HistoryMode {
    /// KISS `HistoryMode.valueById`: an unknown id falls back to `RECENCY`.
    pub fn value_by_id(id: &str) -> Self {
        match id {
            "frecency" => HistoryMode::Frecency,
            "frequency" => HistoryMode::Frequency,
            "adaptive" => HistoryMode::Adaptive,
            "time" => HistoryMode::Time,
            "alphabetically" => HistoryMode::Alphabetically,
            // "recency" and anything unrecognised
            _ => HistoryMode::Recency,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            HistoryMode::Recency => "recency",
            HistoryMode::Frecency => "frecency",
            HistoryMode::Frequency => "frequency",
            HistoryMode::Adaptive => "adaptive",
            HistoryMode::Time => "time",
            HistoryMode::Alphabetically => "alphabetically",
        }
    }
}

/// KISS `Searcher.DEFAULT_MAX_RESULTS`.
pub const DEFAULT_MAX_RESULTS: usize = 50;

/// KISS `QuerySearcher.getMaxResultCount`, minus the static cache.
///
/// Parses through `f64` (as KISS parses through `Double`) then truncates toward
/// zero, so an oversized preference saturates instead of throwing. Anything
/// unparseable falls back to [`DEFAULT_MAX_RESULTS`].
pub fn max_result_count(pref: &str) -> usize {
    let trimmed = pref.trim();
    if trimmed.is_empty() {
        return DEFAULT_MAX_RESULTS;
    }
    match trimmed.parse::<f64>() {
        // Java's Double.intValue() truncates toward zero and saturates at the
        // int bounds; Rust's `as i32` saturates identically.
        Ok(v) if v.is_finite() => (v as i32).max(0) as usize,
        _ => DEFAULT_MAX_RESULTS,
    }
}

/// KISS `QuerySearcher.addResults`, for a single pojo.
///
/// `known_ids` mirrors the Java `Map<String,Integer>` built from history for
/// this exact query. Returns the adjusted relevance.
///
/// Faithful detail: a disabled pojo takes the penalty and gets NO boost — the
/// Java branches are exclusive (`if (disabled) ... else ...`).
pub fn adjusted_relevance(
    relevance: i64,
    disabled: bool,
    id: &str,
    known_ids: &[(String, i64)],
    mode: HistoryMode,
) -> i64 {
    if disabled {
        // "Give penalty for disabled items, these should not be preferred"
        return relevance - 200;
    }
    let Some((_, value)) = known_ids.iter().find(|(k, _)| k == id) else {
        return relevance;
    };
    let boost = if mode != HistoryMode::Alphabetically {
        // max(0, 100 - max(knownIds.size() - value, 0) * 5)
        let size = known_ids.len() as i64;
        let behind = (size - value).max(0);
        (100 - behind * 5).max(0)
    } else {
        100
    };
    relevance + boost
}

/// KISS `Searcher.onPostExecute` eviction: keep the `max_results` highest
/// relevance rows, dropping the lowest as a min-heap would.
///
/// Survivors are returned in the order KISS drains its queue — ascending
/// relevance, because its ListView starts at the bottom. Callers that display
/// best-first should use `comparator::sort_for_display` instead.
pub fn bounded_top_k<T, F>(mut items: Vec<T>, max_results: usize, relevance_of: F) -> Vec<T>
where
    F: Fn(&T) -> i64,
{
    items.sort_by_key(|a| relevance_of(a));
    if items.len() > max_results {
        let drop = items.len() - max_results;
        items.drain(0..drop);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_mode_falls_back_to_recency() {
        assert_eq!(HistoryMode::value_by_id("frecency"), HistoryMode::Frecency);
        assert_eq!(
            HistoryMode::value_by_id("alphabetically"),
            HistoryMode::Alphabetically
        );
        assert_eq!(HistoryMode::value_by_id("recency"), HistoryMode::Recency);
        assert_eq!(HistoryMode::value_by_id("nonsense"), HistoryMode::Recency);
        assert_eq!(HistoryMode::value_by_id(""), HistoryMode::Recency);
        assert_eq!(HistoryMode::Adaptive.id(), "adaptive");
    }

    #[test]
    fn max_result_count_matches_kiss_fallbacks() {
        assert_eq!(max_result_count(""), DEFAULT_MAX_RESULTS);
        assert_eq!(max_result_count("12"), 12);
        assert_eq!(max_result_count("abc"), DEFAULT_MAX_RESULTS);
        // The crash KISS guards against: a value larger than Integer.MAX_VALUE.
        assert_eq!(max_result_count("99999999999999999999"), i32::MAX as usize);
    }

    #[test]
    fn disabled_takes_penalty_and_no_boost() {
        let known = vec![("firefox".to_string(), 3i64)];
        let boosted = adjusted_relevance(10, false, "firefox", &known, HistoryMode::Recency);
        let disabled = adjusted_relevance(10, true, "firefox", &known, HistoryMode::Recency);
        assert_eq!(disabled, 10 - 200); // exactly -200, boost not applied
        assert!(boosted > disabled);
    }

    #[test]
    fn boost_decreases_by_five_per_step_behind() {
        // knownIds.size() == 3: value 3 -> 100, value 2 -> 95, value 1 -> 90
        let known = vec![
            ("a".to_string(), 3i64),
            ("b".to_string(), 2),
            ("c".to_string(), 1),
        ];
        assert_eq!(
            adjusted_relevance(0, false, "a", &known, HistoryMode::Recency),
            100
        );
        assert_eq!(
            adjusted_relevance(0, false, "b", &known, HistoryMode::Recency),
            95
        );
        assert_eq!(
            adjusted_relevance(0, false, "c", &known, HistoryMode::Recency),
            90
        );
    }

    #[test]
    fn boost_is_clamped_at_zero() {
        let known = vec![("a".to_string(), 1i64), ("b".to_string(), -100)];
        assert_eq!(
            adjusted_relevance(50, false, "b", &known, HistoryMode::Recency),
            50
        );
    }

    #[test]
    fn alphabetically_mode_gives_flat_hundred() {
        let known = vec![
            ("a".to_string(), 0i64),
            ("b".to_string(), 0),
            ("c".to_string(), 0),
        ];
        // recency: 100 - (3-0)*5 = 85; alphabetically: flat 100
        assert_eq!(
            adjusted_relevance(0, false, "a", &known, HistoryMode::Recency),
            85
        );
        assert_eq!(
            adjusted_relevance(0, false, "a", &known, HistoryMode::Alphabetically),
            100
        );
    }

    #[test]
    fn unknown_id_gets_no_boost() {
        let known = vec![("a".to_string(), 3i64)];
        assert_eq!(
            adjusted_relevance(7, false, "zzz", &known, HistoryMode::Recency),
            7
        );
    }

    #[test]
    fn bounded_top_k_evicts_lowest() {
        let items = vec![("low", 1i64), ("high", 100), ("mid", 50)];
        let kept = bounded_top_k(items, 2, |(_, r)| *r);
        let names: Vec<&str> = kept.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["mid", "high"]); // ascending drain order
    }

    #[test]
    fn bounded_top_k_keeps_all_under_capacity() {
        let items = vec![("a", 5i64), ("b", 9)];
        let kept = bounded_top_k(items, DEFAULT_MAX_RESULTS, |(_, r)| *r);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].0, "a");
    }
}
