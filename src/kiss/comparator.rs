//! Faithful port of KISS's comparator chain.
//!
//! Upstream (Neamar/KISS, `app/src/main/java/fr/neamar/kiss/pojo/`):
//!
//! * `NameComparator.compare(lhs, rhs)`:
//!   1. `lhs.normalizedName.compareTo(rhs.normalizedName)` when both are
//!      non-null and the result is non-zero;
//!   2. else `lhs.getName().compareTo(rhs.getName())` when both non-null and
//!      non-zero;
//!   3. else `lhs.getUserHandle().compareTo(rhs.getUserHandle())`;
//!   4. else `0`.
//! * `ReversedNameComparator.compare(lhs, rhs)` = `NameComparator.compare(rhs, lhs)`.
//! * `RelevanceComparator.compare(lhs, rhs)` =
//!   `Integer.compare(lhs.relevance, rhs.relevance)`, falling back to the
//!   reversed name comparison on a tie.
//!
//! `searcher/Searcher` drains a **min-heap** `PriorityQueue` ordered by
//! `RelevanceComparator` (it evicts with `while (queue.size() > maxResults)
//! queue.poll()`), and `ApplicationsSearcher` documents why: "Sort from A to Z,
//! so reverse (last item needs to be A, listview starts at the bottom)".
//!
//! So the order the user SEES is the reverse of the drain order:
//! **relevance descending, ties by normalized name ascending.** That tie-break
//! is what makes a query like "fi" show `Files` before `Firefox`
//! deterministically instead of leaking the corpus order.
//!
//! GAP (named, not invented): `Pojo.userHandle` (work/private profiles) is not
//! modeled by RISS entries, so step 3 of `NameComparator` is a no-op here.

use std::cmp::Ordering;

/// What the comparators need from a ranked row.
pub trait Ranked {
    /// KISS `Pojo.relevance`.
    fn relevance(&self) -> i64;
    /// KISS `Pojo.normalizedName` (computed once at load, never per keystroke).
    fn normalized_name(&self) -> &str;
    /// KISS `Pojo.getName()`.
    fn display_name(&self) -> &str;
}

/// `NameComparator.compare(lhs, rhs)`, minus the unmodeled `userHandle` step.
pub fn name_compare(lhs: &dyn Ranked, rhs: &dyn Ranked) -> Ordering {
    let by_normalized = lhs.normalized_name().cmp(rhs.normalized_name());
    if by_normalized != Ordering::Equal {
        return by_normalized;
    }
    lhs.display_name().cmp(rhs.display_name())
}

/// `ReversedNameComparator.compare(lhs, rhs)`.
pub fn reversed_name_compare(lhs: &dyn Ranked, rhs: &dyn Ranked) -> Ordering {
    name_compare(rhs, lhs)
}

/// `RelevanceComparator.compare(lhs, rhs)` — the min-heap order KISS uses to
/// evict the lowest relevance and to drain the queue.
pub fn relevance_compare(lhs: &dyn Ranked, rhs: &dyn Ranked) -> Ordering {
    let by_relevance = lhs.relevance().cmp(&rhs.relevance());
    if by_relevance != Ordering::Equal {
        return by_relevance;
    }
    reversed_name_compare(lhs, rhs)
}

/// Display order: the reverse of the drain order, because KISS's ListView
/// starts at the bottom. Best first, deterministic ties.
pub fn display_compare(lhs: &dyn Ranked, rhs: &dyn Ranked) -> Ordering {
    relevance_compare(rhs, lhs)
}

/// Sort rows into KISS's visual order.
pub fn sort_for_display<T: Ranked>(rows: &mut [T]) {
    rows.sort_by(|a, b| display_compare(a, b));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kiss::normalizer::normalize;

    struct Row {
        name: String,
        key: String,
        score: i64,
    }

    impl Row {
        fn new(name: &str, score: i64) -> Self {
            Self {
                name: name.to_string(),
                key: normalize(name),
                score,
            }
        }
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

    #[test]
    fn higher_relevance_comes_first() {
        let mut v = vec![Row::new("Alpha", 10), Row::new("Beta", 90)];
        sort_for_display(&mut v);
        assert_eq!(v[0].name, "Beta");
    }

    #[test]
    fn ties_break_by_name_not_corpus_order() {
        // Corpus order puts Firefox first on purpose; the name tie-break must
        // still surface Files first.
        let mut v = vec![Row::new("Firefox", 400), Row::new("Files", 400)];
        sort_for_display(&mut v);
        let names: Vec<&str> = v.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["Files", "Firefox"]);
    }

    #[test]
    fn tie_break_uses_normalized_name() {
        // "Éditeur" normalizes to "editeur", so it sorts before "Zebra".
        let mut v = vec![Row::new("Zebra", 5), Row::new("Éditeur", 5)];
        sort_for_display(&mut v);
        assert_eq!(v[0].name, "Éditeur");
    }

    #[test]
    fn relevance_compare_is_min_heap_order() {
        let lo = Row::new("Alpha", 1);
        let hi = Row::new("Beta", 2);
        assert_eq!(relevance_compare(&lo, &hi), Ordering::Less);
        assert_eq!(display_compare(&lo, &hi), Ordering::Greater);
    }

    #[test]
    fn equal_rows_compare_equal() {
        let a = Row::new("Same", 7);
        let b = Row::new("Same", 7);
        assert_eq!(relevance_compare(&a, &b), Ordering::Equal);
    }
}
