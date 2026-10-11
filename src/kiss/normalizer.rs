//! Port of `fr.neamar.kiss.normalizer.StringNormalizer`.
//!
//! Upstream: Neamar/KISS @ `17fc7ebd473bbcbfc0e93592859278443cdb4ec0`,
//! `app/src/main/java/fr/neamar/kiss/normalizer/StringNormalizer.java`.
//!
//! The Java function is `normalizeWithResult(CharSequence input, boolean
//! makeLowercase)`. Both of its production callers pass `false`:
//!
//! * `Pojo.setName`      -> `normalizeWithResult(this.name, false)`
//! * `AppProvider.requestResults` -> `normalizeWithResult(query, false)`
//!
//! Case is therefore *preserved* in the normalized form and folded later, in
//! `FuzzyScoreV2` (which lower-cases the pattern once and each candidate code
//! point as it compares) and in `Pojo.normalizedName.compareTo`. Lower-casing
//! here would make `FuzzyScoreV2`'s camel-case bonus permanently unreachable,
//! because that bonus tests `curr != toLowerCase(curr)`.
//!
//! ## Java -> Rust position mapping
//!
//! Java records `resultMap.add(i)` where `i` is the **UTF-16 code-unit index**
//! of the source character, and `originalInputLastCharPosition` is
//! `input.length()`, also in UTF-16 code units. Rust strings are UTF-8, so the
//! port stores **UTF-8 byte offsets** -- the thing a Rust caller can actually
//! slice with -- and keeps the UTF-16 length alongside so the Java numbers
//! remain derivable. [`utf16_index`] converts one to the other and is used by
//! the tests that pin the correspondence for astral code points, where the two
//! indexings disagree (one UTF-16 surrogate pair = 4 UTF-8 bytes = 1 char).

use unicode_general_category::{get_general_category, GeneralCategory};
use unicode_normalization::char::decompose_compatible;

/// `Character.toLowerCase(int)`: the *simple* (single code point) lowercase
/// mapping from `UnicodeData.txt`, not Rust's full `SpecialCasing` mapping.
///
/// The only unconditional full lowercase mapping that expands is
/// `U+0130 LATIN CAPITAL LETTER I WITH DOT ABOVE` -> `U+0069 U+0307`, and its
/// simple mapping is `U+0069`, i.e. the first code point of the expansion. So
/// "take the first" reproduces Java for every input.
#[inline]
pub fn java_to_lowercase(c: char) -> char {
    let mut it = c.to_lowercase();
    // `to_lowercase` always yields at least one char.
    it.next().unwrap_or(c)
}

/// `Character.toUpperCase(int)`: again the *simple* mapping.
///
/// Here "take the first" would be wrong. Rust's full uppercase mapping expands
/// `ß` to `SS`, but `ß` has **no** simple uppercase mapping, so Java returns
/// `ß` unchanged. Every character whose full uppercase expands has an empty
/// simple mapping, so an expansion means "identity".
#[inline]
pub fn java_to_uppercase(c: char) -> char {
    let mut it = c.to_uppercase();
    let first = it.next().unwrap_or(c);
    if it.next().is_some() {
        c
    } else {
        first
    }
}

/// `Character.isWhitespace(int)`.
///
/// Deliberately *not* `char::is_whitespace`: Rust follows the Unicode
/// `White_Space` property, which includes the non-breaking spaces
/// (`U+00A0`, `U+2007`, `U+202F`) that Java explicitly excludes, and omits the
/// `U+001C..U+001F` file/group/record/unit separators that Java includes.
/// `FuzzyScoreV2`'s separator bonus is driven by this predicate, so the
/// difference is score-visible.
#[inline]
pub fn java_is_whitespace(c: char) -> bool {
    match c {
        '\u{0009}'..='\u{000D}' | '\u{001C}'..='\u{001F}' => true,
        '\u{00A0}' | '\u{2007}' | '\u{202F}' => false,
        _ => matches!(
            get_general_category(c),
            GeneralCategory::SpaceSeparator
                | GeneralCategory::LineSeparator
                | GeneralCategory::ParagraphSeparator
        ),
    }
}

/// True for the three Unicode categories `StringNormalizer` drops:
/// `NON_SPACING_MARK` (Mn), `COMBINING_SPACING_MARK` (Mc) and
/// `DASH_PUNCTUATION` (Pd).
#[inline]
fn is_dropped_category(c: char) -> bool {
    matches!(
        get_general_category(c),
        GeneralCategory::NonspacingMark
            | GeneralCategory::SpacingMark
            | GeneralCategory::DashPunctuation
    )
}

/// `StringNormalizer.Result`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Normalized {
    /// `Result.codePoints`. One `char` per Java `int` code point.
    pub code_points: Vec<char>,
    /// `Result.mapPositions`, expressed as **UTF-8 byte offsets** into the
    /// original string (Java stores UTF-16 code-unit offsets).
    map_positions: Vec<u32>,
    /// `Result.originalInputLastCharPosition`, in UTF-8 bytes.
    original_len_utf8: u32,
    /// The same length in UTF-16 code units, which is the number Java stores.
    original_len_utf16: u32,
}

impl Normalized {
    pub const EMPTY: Normalized = Normalized {
        code_points: Vec::new(),
        map_positions: Vec::new(),
        original_len_utf8: 0,
        original_len_utf16: 0,
    };

    /// `Result.length()`.
    #[inline]
    pub fn len(&self) -> usize {
        self.code_points.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.code_points.is_empty()
    }

    /// `Result.mapPosition(int)`, in UTF-8 byte offsets.
    pub fn map_position(&self, position: usize) -> usize {
        match self.map_positions.get(position) {
            Some(p) => *p as usize,
            // "We are behind the last character, return the position of the
            // end of the original input".
            None => self.original_len_utf8 as usize,
        }
    }

    /// The same mapping expressed the way Java expresses it, for tests and for
    /// anyone comparing against upstream numbers.
    pub fn map_position_utf16(&self, position: usize, original: &str) -> usize {
        match self.map_positions.get(position) {
            Some(p) => utf16_index(original, *p as usize),
            None => self.original_len_utf16 as usize,
        }
    }

    pub fn original_len_utf16(&self) -> usize {
        self.original_len_utf16 as usize
    }

    /// `Result.toString()`.
    pub fn to_display_string(&self) -> String {
        self.code_points.iter().collect()
    }

    /// `Result.compareTo(Result)`: compare lower-cased code point by lower-cased
    /// code point, then by length. Java subtracts the two `int`s; the sign is
    /// all that is observable through a `Comparator`, so an `Ordering` is the
    /// faithful shape.
    pub fn compare_to(&self, other: &Normalized) -> std::cmp::Ordering {
        let min = self.code_points.len().min(other.code_points.len());
        for i in 0..min {
            let a = java_to_lowercase(self.code_points[i]) as u32;
            let b = java_to_lowercase(other.code_points[i]) as u32;
            if a != b {
                return a.cmp(&b);
            }
        }
        self.code_points.len().cmp(&other.code_points.len())
    }
}

/// Number of UTF-16 code units in `s[..byte_idx]`.
///
/// This is the bridge between Java's indexing and Rust's: every code point
/// below `U+10000` is one UTF-16 unit, everything above is a surrogate pair.
pub fn utf16_index(s: &str, byte_idx: usize) -> usize {
    s[..byte_idx].chars().map(char::len_utf16).sum()
}

/// `StringNormalizer.normalizeWithResult(input, makeLowercase)`.
pub fn normalize_with_result(input: &str, make_lowercase: bool) -> Normalized {
    let mut out = Normalized::EMPTY;
    normalize_into(input, make_lowercase, &mut out);
    out
}

/// The same function writing into a caller-owned [`Normalized`], so a reload
/// can reuse the buffers instead of allocating two `Vec`s per application.
pub fn normalize_into(input: &str, make_lowercase: bool, out: &mut Normalized) {
    out.code_points.clear();
    out.map_positions.clear();
    out.original_len_utf8 = input.len() as u32;
    out.original_len_utf16 = input.chars().map(char::len_utf16).sum::<usize>() as u32;

    if input.is_empty() {
        return;
    }

    out.code_points.reserve(input.len());
    out.map_positions.reserve(input.len());

    // The NFKD expansion of a single code point, on the stack. The longest
    // compatibility decomposition in Unicode is 18 code points (the Arabic
    // ligatures), so this can never spill -- and a `Vec` here would be a heap
    // allocation on the one path a non-ASCII query takes every keystroke.
    let mut decomposed = ['\0'; 32];
    let mut decomposed_len;
    for (byte_idx, codepoint) in input.char_indices() {
        let i = byte_idx as u32;
        if codepoint <= 'z' {
            // "Ascii range, no need to normalize!" -- HYPHEN-MINUS is the only
            // character at or below 'z' in Mn/Mc/Pd, so upstream skips
            // `Character.getType` and tests for it directly. (Upstream commit
            // d71c69a34, "ASCII also includes z!", widened this from `< 'z'`.)
            if codepoint != '-' {
                out.code_points.push(if make_lowercase {
                    java_to_lowercase(codepoint)
                } else {
                    codepoint
                });
                out.map_positions.push(i);
            }
        } else {
            // `Normalizer.normalize(buffer, Normalizer.Form.NFKD)` on a buffer
            // holding exactly this one code point.
            decomposed_len = 0;
            decompose_compatible(codepoint, |c| {
                if decomposed_len < decomposed.len() {
                    decomposed[decomposed_len] = c;
                    decomposed_len += 1;
                }
            });
            for &result_char in &decomposed[..decomposed_len] {
                if is_dropped_category(result_char) {
                    continue;
                }
                out.code_points.push(if make_lowercase {
                    java_to_lowercase(result_char)
                } else {
                    result_char
                });
                out.map_positions.push(i);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ported verbatim from upstream `StringNormalizerTest.testNormalization`
    /// (`app/src/test/java/fr/neamar/kiss/normalizer/StringNormalizerTest.java`).
    #[test]
    fn upstream_string_normalizer_test_vectors() {
        let cases = [
            ("è", "e"),
            ("é", "e"),
            ("ê", "e"),
            ("ë", "e"),
            ("û", "u"),
            ("ù", "u"),
            ("ï", "i"),
            ("î", "i"),
            ("à", "a"),
            ("â", "a"),
            ("ô", "o"),
            ("ä", "a"),
            ("ö", "o"),
            ("ü", "u"),
            ("Å", "A"),
            ("Æ", "Æ"),
            ("ﬁ", "fi"),
        ];
        for (input, expected) in cases {
            let r = normalize_with_result(input, false);
            assert_eq!(
                r.to_display_string(),
                expected,
                "normalize({input:?}, makeLowercase=false)"
            );
        }
    }

    #[test]
    fn lowercasing_is_a_flag_and_production_passes_false() {
        // `Å` stays upper-case with the flag off (the upstream vector above)
        // and folds with it on. Both callers in KISS pass false.
        assert_eq!(normalize_with_result("Å", false).to_display_string(), "A");
        assert_eq!(normalize_with_result("Å", true).to_display_string(), "a");
        assert_eq!(
            normalize_with_result("VirtualBox", false).to_display_string(),
            "VirtualBox"
        );
        assert_eq!(
            normalize_with_result("VirtualBox", true).to_display_string(),
            "virtualbox"
        );
    }

    #[test]
    fn dashes_are_stripped_on_both_sides() {
        // Upstream 2caa1d27e, "Update StringNormalizer to strip dashes from
        // normalized strings (fixes #261)". HYPHEN-MINUS takes the ASCII
        // branch; EN DASH / EM DASH / MINUS SIGN take the category branch.
        assert_eq!(
            normalize_with_result("Visual-Studio", false).to_display_string(),
            "VisualStudio"
        );
        assert_eq!(
            normalize_with_result("a\u{2013}b\u{2014}c", false).to_display_string(),
            "abc"
        );
        // ... but U+2212 MINUS SIGN is Sm (math symbol), not Pd, so Java
        // keeps it. Looking like a dash is not being one.
        assert_eq!(
            normalize_with_result("a\u{2212}b", false).to_display_string(),
            "a\u{2212}b"
        );
    }

    #[test]
    fn empty_input_is_the_empty_result() {
        let r = normalize_with_result("", false);
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert_eq!(r.map_position(0), 0);
    }

    #[test]
    fn position_map_points_back_at_the_source_character() {
        // "Café Noir": the accented e is one source char that survives as one
        // code point, and every later position must still point at its own
        // source byte.
        let src = "Café Noir";
        let r = normalize_with_result(src, false);
        assert_eq!(r.to_display_string(), "Cafe Noir");
        for (i, c) in r.code_points.iter().enumerate() {
            let at = r.map_position(i);
            assert_eq!(
                src[at..].chars().next().map(|s| s == *c || *c == 'e'),
                Some(true),
                "code point {i} ({c:?}) maps to byte {at}"
            );
        }
        // Behind the last character -> end of the original input.
        assert_eq!(r.map_position(r.len()), src.len());
        assert_eq!(r.map_position(999), src.len());
    }

    #[test]
    fn one_source_char_can_produce_several_code_points() {
        // `ﬁ` (U+FB00 ligature) decomposes to two code points that both map
        // back to the single source character.
        let src = "aﬁb";
        let r = normalize_with_result(src, false);
        assert_eq!(r.to_display_string(), "afib");
        assert_eq!(r.map_position(0), 0); // 'a'
        assert_eq!(r.map_position(1), 1); // 'f' <- U+FB00 at byte 1
        assert_eq!(r.map_position(2), 1); // 'i' <- same source char
        assert_eq!(r.map_position(3), 4); // 'b' after a 3-byte char
    }

    /// The trap called out in the brief: Java indexes UTF-16 code units, Rust
    /// indexes UTF-8 bytes, and an astral code point is 1 char / 2 UTF-16 units
    /// / 4 UTF-8 bytes. A port that stored Java's numbers unchanged would slice
    /// a Rust string at the wrong place.
    #[test]
    fn java_utf16_positions_versus_rust_utf8_positions() {
        let src = "a\u{1F600}b"; // 'a', GRINNING FACE, 'b'
        assert_eq!(src.len(), 6); // UTF-8 bytes
        assert_eq!(src.chars().map(char::len_utf16).sum::<usize>(), 4); // Java length()

        let r = normalize_with_result(src, false);
        assert_eq!(r.code_points, vec!['a', '\u{1F600}', 'b']);

        // Rust-native view: byte offsets we can slice with.
        assert_eq!(r.map_position(0), 0);
        assert_eq!(r.map_position(1), 1);
        assert_eq!(r.map_position(2), 5);
        assert_eq!(r.map_position(3), 6);
        assert_eq!(&src[r.map_position(2)..], "b");

        // Java's view, reconstructed: upstream would have stored 0, 1, 3 and
        // an end position of 4.
        assert_eq!(r.map_position_utf16(0, src), 0);
        assert_eq!(r.map_position_utf16(1, src), 1);
        assert_eq!(r.map_position_utf16(2, src), 3);
        assert_eq!(r.map_position_utf16(3, src), 4);
        assert_eq!(r.original_len_utf16(), 4);
        assert_eq!(utf16_index(src, 5), 3);
    }

    #[test]
    fn non_ascii_beyond_latin_survives() {
        // Greek, Cyrillic and CJK have no compatibility decomposition that
        // changes them, so they pass through as themselves.
        assert_eq!(
            normalize_with_result("Ελληνικά", false).to_display_string(),
            "Ελληνικα"
        );
        assert_eq!(
            normalize_with_result("Привет", false).to_display_string(),
            "Привет"
        );
        assert_eq!(
            normalize_with_result("日本語", false).to_display_string(),
            "日本語"
        );
        // Half-width katakana is a compatibility decomposition (the class
        // doc-comment's own example).
        assert_eq!(normalize_with_result("ﾐ", false).to_display_string(), "ミ");
        // Roman numeral, the other documented example.
        assert_eq!(normalize_with_result("Ⅱ", false).to_display_string(), "II");
    }

    #[test]
    fn java_case_helpers_differ_from_rust_full_case_mappings() {
        // Rust's full lowercase of U+0130 is two chars; Java's simple mapping
        // is 'i', the first of them.
        assert_eq!('\u{0130}'.to_lowercase().count(), 2);
        assert_eq!(java_to_lowercase('\u{0130}'), 'i');
        // Rust's full uppercase of 'ß' is "SS"; Java leaves it alone.
        assert_eq!('ß'.to_uppercase().collect::<String>(), "SS");
        assert_eq!(java_to_uppercase('ß'), 'ß');
        assert_eq!(java_to_uppercase('a'), 'A');
        assert_eq!(java_to_lowercase('A'), 'a');
    }

    #[test]
    fn java_whitespace_is_not_rust_whitespace() {
        // Score-visible: these decide FuzzyScoreV2's separator bonus.
        assert!('\u{00A0}'.is_whitespace());
        assert!(!java_is_whitespace('\u{00A0}'));
        assert!(!'\u{001C}'.is_whitespace());
        assert!(java_is_whitespace('\u{001C}'));
        assert!(java_is_whitespace(' '));
        assert!(java_is_whitespace('\t'));
        assert!(java_is_whitespace('\n'));
        assert!(!java_is_whitespace('a'));
    }

    #[test]
    fn compare_to_folds_case_then_falls_back_to_length() {
        // Upstream 81e8c7b80, "Ensure Results are sorted alphabetically, and
        // not by length (fix #812)": the length comparison is the tie-break,
        // not the first test.
        let a = normalize_with_result("apple", false);
        let b = normalize_with_result("Apple", false);
        let c = normalize_with_result("Apple Pie", false);
        let d = normalize_with_result("banana", false);
        assert_eq!(a.compare_to(&b), std::cmp::Ordering::Equal);
        assert_eq!(a.compare_to(&c), std::cmp::Ordering::Less);
        assert_eq!(d.compare_to(&c), std::cmp::Ordering::Greater);
    }

    #[test]
    fn normalize_into_reuses_its_buffers() {
        let mut buf = Normalized::EMPTY;
        normalize_into("first long name", false, &mut buf);
        let cap = buf.code_points.capacity();
        normalize_into("second", false, &mut buf);
        assert_eq!(buf.to_display_string(), "second");
        assert_eq!(buf.code_points.capacity(), cap, "buffer was reallocated");
    }
}
