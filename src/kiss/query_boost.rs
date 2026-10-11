//! Faithful port of KISS `QuerySearcher`'s relevance adjustments, the
//! `HistoryMode` enum, and `Searcher`'s bounded queue.
//!
//! Upstream: Neamar/KISS pinned at `17fc7ebd473bbcbfc0e93592859278443cdb4ec0`.
//!
//! * `searcher/Searcher.java` — `DEFAULT_MAX_RESULTS = 50`. Results collect
//!   into `new PriorityQueue<>(DEFAULT_MAX_RESULTS, new RelevanceComparator())`.
//!
//!   That constructor argument is the queue's *initial capacity*, NOT a cap:
//!   `java.util.PriorityQueue` grows without bound, so every result added
//!   during the search is retained. The trim happens afterwards, in
//!   `onPostExecute`:
//!   `while (queue.size() > maxResults) queue.poll();`
//!   It is a min-heap ordered by `RelevanceComparator`, so `poll()` removes the
//!   *least* relevant and the highest-relevance rows survive.
//!
//!   (An earlier revision of this file and of PORTING.md claimed KISS "bounds
//!   while collecting". It does not. Corrected here so the next reader does not
//!   inherit the error.)
//!
//!   Search runs on a dedicated `SEARCH_THREAD` and is cancellable
//!   (`isCancelled()` is checked in `addResults`).
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

use crate::kiss::comparator::{relevance_compare, Ranked};

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
/// KISS does, verbatim:
/// ```java
/// MAX_RESULT_COUNT = Double.valueOf(prefs.getString("number-of-display-elements",
///         String.valueOf(DEFAULT_MAX_RESULTS))).intValue();
/// // catch (NumberFormatException e) -> DEFAULT_MAX_RESULTS
/// ```
///
/// DELIBERATE DEVIATIONS (not upstream behaviour — flagged rather than hidden):
/// * KISS has no finiteness check. `Double.valueOf("NaN")` and `"Infinity"`
///   parse, and `intValue()` yields `0` and `Integer.MAX_VALUE` respectively.
///   Here non-finite input falls back to [`DEFAULT_MAX_RESULTS`] instead,
///   because a `0` result limit renders an empty list with no way back.
/// * KISS can produce a negative count (`"-5".intValue() == -5`), which its
///   `while (queue.size() > maxResults)` loop then treats as "drain
///   everything". Here it is clamped to `0` because the return type is
///   `usize`; the observable effect of a negative preference is the same
///   (no results), but the value is not identical.
///
/// Faithful parts: parsing through `f64` (as Java parses through `Double`) so
/// an oversized preference saturates instead of throwing, truncation toward
/// zero, and the [`DEFAULT_MAX_RESULTS`] fallback for unparseable input.
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

/// KISS `Searcher.onPostExecute` eviction:
/// `while (queue.size() > maxResults) queue.poll();`
///
/// Ordering is `RelevanceComparator` in full — relevance ascending, ties broken
/// by `ReversedNameComparator` — not relevance alone. The tie-break decides
/// which row is dropped when several sit on the capacity boundary with equal
/// relevance, so using a bare relevance sort here would silently diverge from
/// upstream exactly at the cut.
///
/// Survivors are returned in the order KISS drains its queue (ascending
/// relevance, because its ListView starts at the bottom). Callers that display
/// best-first should use [`crate::kiss::comparator::sort_for_display`].
pub fn bounded_top_k<T: Ranked>(mut items: Vec<T>, max_results: usize) -> Vec<T> {
    items.sort_by(|a, b| relevance_compare(a, b));
    if items.len() > max_results {
        let drop = items.len() - max_results;
        items.drain(0..drop);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kiss::normalizer::normalize_with_result;

    struct Row {
        name: String,
        key: String,
        score: i64,
    }

    impl Ranked for Row {
        fn relevance(&self) -> i64 {
            self.score
        }
        fn normalized_name(&self) -> &str {
            &self.key
        }
        fn display_name(&self) -> &str {
            &self.name
        }
    }

    fn row(name: &str, score: i64) -> Row {
        Row {
            name: name.to_string(),
            key: normalize_with_result(name, true).to_string(),
            score,
        }
    }

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
        let items = vec![row("low", 1), row("high", 100), row("mid", 50)];
        let kept = bounded_top_k(items, 2);
        let names: Vec<&str> = kept.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["mid", "high"]); // ascending drain order
    }

    #[test]
    fn bounded_top_k_keeps_all_under_capacity() {
        let items = vec![row("a", 5), row("b", 9)];
        let kept = bounded_top_k(items, DEFAULT_MAX_RESULTS);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].name, "a");
    }

    #[test]
    fn eviction_tie_break_matches_relevance_comparator() {
        // Three rows, two tied on relevance, capacity 2. Which tied row is
        // dropped is decided by RelevanceComparator's reversed-name fallback,
        // not by insertion order -- this is the case a bare relevance sort
        // would get wrong.
        let items = vec![row("Apple", 10), row("Banana", 10), row("Top", 99)];
        let kept = bounded_top_k(items, 2);
        let names: Vec<&str> = kept.iter().map(|r| r.name.as_str()).collect();
        // RelevanceComparator ties fall through to ReversedNameComparator,
        // i.e. NameComparator(rhs, lhs). So compare(Apple, Banana) is Greater
        // and the ascending drain order is [Banana, Apple, Top]; poll() takes
        // from the front, dropping "Banana".
        //
        // Read the other way round: display order is the reverse of the drain,
        // so at equal relevance the alphabetically FIRST name is the one shown
        // highest -- and therefore the alphabetically LAST is the one evicted
        // at the capacity boundary. That is the same rule that puts "Files"
        // above "Firefox" in comparator::tests.
        assert_eq!(names, vec!["Apple", "Top"]);

        // And the survivors really are in KISS's drain order, not display
        // order: ascending relevance.
        assert!(kept[0].relevance() <= kept[1].relevance());
    }
}
