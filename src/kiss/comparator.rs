//! Port of `fr.neamar.kiss.pojo.NameComparator`,
//! `ReversedNameComparator` and `RelevanceComparator`.
//!
//! Upstream: Neamar/KISS @ `17fc7ebd473bbcbfc0e93592859278443cdb4ec0`.
//!
//! ## Why the display order is the *reverse* of the comparator
//!
//! `Searcher` keeps results in a `PriorityQueue<Pojo>` ordered by
//! `RelevanceComparator`, i.e. a **min-heap**: `poll()` returns the *least*
//! relevant pojo first. `onPostExecute` first polls away the overflow
//! (`while (queue.size() > maxResults) queue.poll();` -- dropping the worst),
//! then drains the rest into a `List` in ascending order and hands it to the
//! adapter. KISS's `ListView` is stacked from the bottom
//! (`ApplicationsSearcher`: *"Sort from A to Z, so reverse (last item needs to
//! be A, listview starts at the bottom)"*), so the item the user sees first is
//! the **last** one in that list.
//!
//! RISS paints its result list top-down, so the equivalent display order is
//! `RelevanceComparator` **descending**: relevance high to low, and for equal
//! relevance the reversed-name tie-break inverts too, leaving normalized name
//! *ascending*. That is why `Files` beats `Firefox` on the query `fi` when
//! both score the same.

use super::normalizer::Normalized;
use std::cmp::Ordering;

/// The three fields `NameComparator` looks at, in its order.
#[derive(Clone, Copy)]
pub struct NameKey<'a> {
    /// `Pojo.normalizedName`. `None` models Java's `null`.
    pub normalized_name: Option<&'a Normalized>,
    /// `Pojo.getName()`.
    pub name: Option<&'a str>,
    /// `Pojo.getUserHandle()`. RISS has no work profiles, so this is `None`
    /// for every application today; the step is ported so that adding them
    /// later cannot silently change the ordering.
    pub user_handle: Option<&'a str>,
}

/// `java.lang.String.compareTo`: lexicographic over **UTF-16 code units**.
///
/// Not the same as Rust's `str::cmp`, which is lexicographic over UTF-8 bytes
/// (equivalently, over code points). The two disagree whenever a supplementary
/// character is compared against a code point in `U+E000..=U+FFFF`, because
/// the surrogate pair `0xD800..` sorts *below* them in UTF-16 and *above* them
/// as code points.
pub fn java_string_compare(a: &str, b: &str) -> Ordering {
    let mut lhs = a.chars().flat_map(Utf16Units::new);
    let mut rhs = b.chars().flat_map(Utf16Units::new);
    loop {
        match (lhs.next(), rhs.next()) {
            (Some(x), Some(y)) if x == y => continue,
            (Some(x), Some(y)) => return x.cmp(&y),
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
        }
    }
}

/// Iterator over the one or two UTF-16 code units of a `char`.
struct Utf16Units {
    buf: [u16; 2],
    len: u8,
    pos: u8,
}

impl Utf16Units {
    fn new(c: char) -> Self {
        let mut buf = [0u16; 2];
        let len = c.encode_utf16(&mut buf).len() as u8;
        Self { buf, len, pos: 0 }
    }
}

impl Iterator for Utf16Units {
    type Item = u16;
    fn next(&mut self) -> Option<u16> {
        if self.pos < self.len {
            let v = self.buf[self.pos as usize];
            self.pos += 1;
            Some(v)
        } else {
            None
        }
    }
}

/// `NameComparator.compare(lhs, rhs)`.
pub fn name_compare(lhs: &NameKey, rhs: &NameKey) -> Ordering {
    if let (Some(a), Some(b)) = (lhs.normalized_name, rhs.normalized_name) {
        let result = a.compare_to(b);
        if result != Ordering::Equal {
            return result;
        }
    }
    if let (Some(a), Some(b)) = (lhs.name, rhs.name) {
        let result = java_string_compare(a, b);
        if result != Ordering::Equal {
            return result;
        }
    }
    if let (Some(a), Some(b)) = (lhs.user_handle, rhs.user_handle) {
        return java_string_compare(a, b);
    }
    Ordering::Equal
}

/// `ReversedNameComparator.compare(lhs, rhs)` = `super.compare(rhs, lhs)`.
pub fn reversed_name_compare(lhs: &NameKey, rhs: &NameKey) -> Ordering {
    name_compare(rhs, lhs)
}

/// `RelevanceComparator.compare(lhs, rhs)`: relevance ascending, then the
/// reversed name. This is the **min-heap** order, not the display order.
pub fn relevance_compare(
    lhs_relevance: i32,
    lhs: &NameKey,
    rhs_relevance: i32,
    rhs: &NameKey,
) -> Ordering {
    match lhs_relevance.cmp(&rhs_relevance) {
        Ordering::Equal => reversed_name_compare(lhs, rhs),
        other => other,
    }
}

/// The order rows are painted in: `relevance_compare` reversed.
///
/// Equivalent to "relevance descending, ties by normalized name ascending",
/// and to reading KISS's drained `List<Pojo>` back to front.
pub fn display_compare(
    lhs_relevance: i32,
    lhs: &NameKey,
    rhs_relevance: i32,
    rhs: &NameKey,
) -> Ordering {
    relevance_compare(rhs_relevance, rhs, lhs_relevance, lhs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kiss::normalizer::normalize_with_result;

    fn key<'a>(normalized: &'a Normalized, name: &'a str) -> NameKey<'a> {
        NameKey {
            normalized_name: Some(normalized),
            name: Some(name),
            user_handle: None,
        }
    }

    #[test]
    fn java_string_compare_uses_utf16_order() {
        // U+FF21 FULLWIDTH LATIN CAPITAL A is a single UTF-16 unit 0xFF21.
        // U+1F600 is the surrogate pair 0xD83D 0xDE00, so UTF-16 puts it
        // *below* U+FF21 while code-point order puts it above.
        let a = "\u{1F600}";
        let b = "\u{FF21}";
        assert_eq!(java_string_compare(a, b), Ordering::Less);
        assert_eq!(a.cmp(b), Ordering::Greater, "Rust's own order disagrees");

        assert_eq!(java_string_compare("abc", "abd"), Ordering::Less);
        assert_eq!(java_string_compare("abc", "ab"), Ordering::Greater);
        assert_eq!(java_string_compare("abc", "abc"), Ordering::Equal);
        // Upper-case sorts before lower-case, as in Java.
        assert_eq!(java_string_compare("Z", "a"), Ordering::Less);
    }

    #[test]
    fn name_comparator_folds_case_through_the_normalized_name() {
        let f = normalize_with_result("Files", false);
        let x = normalize_with_result("Firefox", false);
        assert_eq!(
            name_compare(&key(&f, "Files"), &key(&x, "Firefox")),
            Ordering::Less
        );
        assert_eq!(
            reversed_name_compare(&key(&f, "Files"), &key(&x, "Firefox")),
            Ordering::Greater
        );
    }

    #[test]
    fn name_comparator_falls_through_to_the_raw_name() {
        // Same normalized form ("Cafe"), different raw names: step 2 decides.
        let a = normalize_with_result("Café", false);
        let b = normalize_with_result("Cafe", false);
        assert_eq!(a.compare_to(&b), Ordering::Equal);
        assert_eq!(
            name_compare(&key(&a, "Café"), &key(&b, "Cafe")),
            Ordering::Greater
        );
    }

    #[test]
    fn name_comparator_falls_through_to_the_user_handle() {
        let n = normalize_with_result("Clock", false);
        let owner = NameKey {
            normalized_name: Some(&n),
            name: Some("Clock"),
            user_handle: Some(""),
        };
        let work = NameKey {
            normalized_name: Some(&n),
            name: Some("Clock"),
            user_handle: Some("#10"),
        };
        assert_eq!(name_compare(&owner, &work), Ordering::Less);
        assert_eq!(name_compare(&owner, &owner), Ordering::Equal);
    }

    #[test]
    fn relevance_wins_over_the_name() {
        let f = normalize_with_result("Files", false);
        let x = normalize_with_result("Firefox", false);
        // Min-heap order: the lower relevance is "less", i.e. polled first.
        assert_eq!(
            relevance_compare(10, &key(&f, "Files"), 20, &key(&x, "Firefox")),
            Ordering::Less
        );
        // Display order: higher relevance first.
        assert_eq!(
            display_compare(10, &key(&f, "Files"), 20, &key(&x, "Firefox")),
            Ordering::Greater
        );
    }

    /// The tie-break the channel got wrong twice: equal relevance must order
    /// by normalized name *ascending* on screen.
    #[test]
    fn equal_relevance_orders_by_name_ascending_on_screen() {
        let f = normalize_with_result("Files", false);
        let x = normalize_with_result("Firefox", false);
        assert_eq!(
            display_compare(100, &key(&f, "Files"), 100, &key(&x, "Firefox")),
            Ordering::Less,
            "Files must be painted above Firefox at equal relevance"
        );
        assert_eq!(
            display_compare(100, &key(&x, "Firefox"), 100, &key(&f, "Files")),
            Ordering::Greater
        );
    }

    #[test]
    fn null_normalized_names_fall_through_like_java() {
        let n = normalize_with_result("Clock", false);
        let null_side = NameKey {
            normalized_name: None,
            name: Some("Alarm"),
            user_handle: None,
        };
        let full = key(&n, "Clock");
        // Java skips the normalized step when either side is null and
        // compares the raw names instead.
        assert_eq!(name_compare(&null_side, &full), Ordering::Less);
    }
}
