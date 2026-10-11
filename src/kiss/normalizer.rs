//! Partial port of KISS `StringNormalizer`.
//!
//! Upstream: `app/src/main/java/fr/neamar/kiss/normalizer/StringNormalizer.java`
//! pinned at `17fc7ebd473bbcbfc0e93592859278443cdb4ec0`.
//!
//! `normalizeWithResult(CharSequence input, boolean makeLowercase)` walks the
//! input one code point at a time and, for each:
//!   1. NFKD-decomposes it (`java.text.Normalizer`, `Form.NFKD`),
//!   2. drops every decomposed code point whose `Character.getType` is
//!      `NON_SPACING_MARK`, `COMBINING_SPACING_MARK` or `DASH_PUNCTUATION`,
//!   3. lowercases the survivors **only when `makeLowercase` is true**,
//!   4. records, for each surviving code point, the offset in the *original*
//!      input it came from.
//!
//! ## `makeLowercase` is false on the paths that matter
//!
//! Verified at the pinned ref:
//! * `pojo/Pojo.java`             -> `normalizeWithResult(this.name, false)`
//! * `dataprovider/AppProvider.java` -> `normalizeWithResult(query, false)`
//!
//! So KISS keeps the original case in `Pojo.normalizedName` and folds case
//! *later*, at comparison time (`Result.compareTo` lowercases both sides).
//!
//! This is not a detail. `utils/fuzzy/FuzzyScoreV2` awards `camel_bonus` when
//! `neighbor != Character.toUpperCase(neighbor) && curr != Character.toLowerCase(curr)`
//! — a lower-then-upper transition. If normalization had already lowercased the
//! name, that bonus could never fire and "VLC" or "GIMP" would rank differently
//! from upstream. An always-lowercasing normalizer is therefore wrong, not
//! merely stricter.
//!
//! ## What is ported, and what is not
//!
//! Ported: the decompose/strip/optional-lowercase pipeline, the code-point
//! output, the source-position map, `mapPosition`, and `compareTo`'s
//! case-insensitive ordering.
//!
//! NOT ported (hence "partial"):
//! * `Character.getType` is a full Unicode category table. [`is_stripped`]
//!   approximates the three categories KISS drops (Mn, Mc/Me, Pd) with explicit
//!   ranges. Characters outside those ranges are kept where Java would drop
//!   them. Replacing this with a real Unicode category lookup is a follow-up.
//! * `IntSequenceBuilder`'s growth strategy is irrelevant here; `Vec` is used.

use unicode_normalization::UnicodeNormalization;

/// Port of KISS `StringNormalizer.Result`.
///
/// Holds the normalized code points plus, for each one, the byte offset in the
/// original input that produced it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Normalized {
    /// Normalized code points, in order. KISS: `Result.codePoints`.
    pub code_points: Vec<char>,
    /// `map_positions[i]` is the offset in the original input that produced
    /// `code_points[i]`. KISS: `Result.mapPositions`.
    pub map_positions: Vec<usize>,
    /// Offset just past the last character of the original input. KISS:
    /// `Result.originalInputLastCharPosition`.
    pub original_input_last_char_position: usize,
}

impl Normalized {
    /// KISS `Result.length()`.
    pub fn length(&self) -> usize {
        self.code_points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.code_points.is_empty()
    }

    /// KISS `Result.mapPosition(position)`: map a position in the normalized
    /// string back to a position in the original input. Positions at or past
    /// the end return the end of the original input.
    pub fn map_position(&self, position: usize) -> usize {
        self.map_positions
            .get(position)
            .copied()
            .unwrap_or(self.original_input_last_char_position)
    }

    /// KISS `Result.compareTo`: compares code points **case-insensitively**,
    /// then by length. This is where KISS folds case, having deliberately
    /// preserved it during normalization.
    pub fn compare_to(&self, other: &Normalized) -> std::cmp::Ordering {
        let min_len = self.code_points.len().min(other.code_points.len());
        for i in 0..min_len {
            let a = lower_first(self.code_points[i]);
            let b = lower_first(other.code_points[i]);
            if a != b {
                return a.cmp(&b);
            }
        }
        self.code_points.len().cmp(&other.code_points.len())
    }
}

impl std::fmt::Display for Normalized {
    /// KISS `Result.toString()`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for c in &self.code_points {
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

/// `Character.toLowerCase(int)` maps one code point to one code point. Rust's
/// `char::to_lowercase` can yield several (e.g. `İ`); KISS's int-based compare
/// cannot, so the first is used to stay faithful to its ordering.
fn lower_first(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Port of KISS `StringNormalizer.normalizeWithResult(input, makeLowercase)`.
pub fn normalize_with_result(input: &str, make_lowercase: bool) -> Normalized {
    let mut code_points = Vec::new();
    let mut map_positions = Vec::new();

    for (offset, ch) in input.char_indices() {
        // KISS decomposes each input code point independently.
        for decomposed in ch.to_string().nfkd() {
            if is_stripped(decomposed) {
                continue;
            }
            if make_lowercase {
                for lower in decomposed.to_lowercase() {
                    code_points.push(lower);
                    map_positions.push(offset);
                }
            } else {
                code_points.push(decomposed);
                map_positions.push(offset);
            }
        }
    }

    Normalized {
        code_points,
        map_positions,
        original_input_last_char_position: input.len(),
    }
}

/// Convenience wrapper: normalize and lowercase, returning a `String`.
///
/// NOTE: this is **not** what KISS uses for app names or queries — both pass
/// `makeLowercase = false` and fold case at comparison time. Use
/// [`normalize_with_result`] for anything that feeds ranking; this helper is
/// for call sites that genuinely want a case-folded key.
pub fn normalize_lowercase(input: &str) -> String {
    normalize_with_result(input, true).to_string()
}

/// Approximates the `Character.getType` classes KISS skips:
/// `NON_SPACING_MARK` (Mn), `COMBINING_SPACING_MARK` (Mc/Me) and
/// `DASH_PUNCTUATION` (Pd). See the module docs: this is the main reason this
/// port is "partial".
fn is_stripped(c: char) -> bool {
    is_combining_mark(c) || is_dash_punctuation(c)
}

/// Combining marks emitted by NFKD decomposition.
fn is_combining_mark(c: char) -> bool {
    matches!(c,
        '\u{0300}'..='\u{036F}' // Combining Diacritical Marks
        | '\u{0483}'..='\u{0489}' // Cyrillic
        | '\u{0591}'..='\u{05BD}' // Hebrew
        | '\u{0610}'..='\u{061A}' // Arabic
        | '\u{064B}'..='\u{065F}' // Arabic
        | '\u{06D6}'..='\u{06DC}' // Arabic
        | '\u{0900}'..='\u{0903}' // Devanagari
        | '\u{093A}'..='\u{094F}' // Devanagari
        | '\u{0951}'..='\u{0957}' // Devanagari
        | '\u{0E31}'..='\u{0E3A}' // Thai
        | '\u{1AB0}'..='\u{1AFF}' // Combining Diacritical Marks Extended
        | '\u{1DC0}'..='\u{1DFF}' // Combining Diacritical Marks Supplement
        | '\u{20D0}'..='\u{20FF}' // Combining Marks for Symbols
        | '\u{FE20}'..='\u{FE2F}' // Combining Half Marks
    )
}

/// The Unicode `Dash_Punctuation` (Pd) category.
fn is_dash_punctuation(c: char) -> bool {
    matches!(
        c,
        '\u{002D}' // HYPHEN-MINUS
        | '\u{058A}' | '\u{05BE}' | '\u{1400}' | '\u{1806}'
        // hyphen, non-breaking hyphen, figure/en/em dash, horizontal bar
        | '\u{2010}'
            ..='\u{2015}'
                | '\u{2E17}'
                | '\u{2E1A}'
                | '\u{2E3A}'
                | '\u{2E3B}'
                | '\u{2E40}'
                | '\u{301C}'
                | '\u{3030}'
                | '\u{30A0}'
                | '\u{FE31}'
                | '\u{FE32}'
                | '\u{FE58}'
                | '\u{FE63}'
                | '\u{FF0D}' // FULLWIDTH HYPHEN-MINUS
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(s: &str, lower: bool) -> String {
        normalize_with_result(s, lower).to_string()
    }

    #[test]
    fn case_is_preserved_when_make_lowercase_is_false() {
        // This is the behaviour KISS actually uses for app names and queries.
        // Upstream's own test expects `Å` to normalize to `A`, not `a`.
        assert_eq!(norm("Ångström", false), "Angstrom");
        assert_eq!(norm("Café", false), "Cafe");
        assert_eq!(norm("ÜBER", false), "UBER");
    }

    #[test]
    fn case_is_folded_only_when_asked() {
        assert_eq!(norm("Ångström", true), "angstrom");
        assert_eq!(norm("Café", true), "cafe");
        assert_eq!(normalize_lowercase("ÜBER"), "uber");
    }

    #[test]
    fn camel_case_survives_normalization() {
        // FuzzyScoreV2's camel_bonus needs the lower->upper transition to still
        // be visible. Pre-lowercasing would make that bonus unreachable.
        let n = normalize_with_result("VirtualBox", false);
        assert!(n.code_points.contains(&'B'));
        assert_eq!(n.to_string(), "VirtualBox");
    }

    #[test]
    fn strips_diacritics_via_nfkd() {
        assert_eq!(norm("Café", true), "cafe");
        assert_eq!(norm("naïve", true), "naive");
    }

    #[test]
    fn strips_dashes() {
        assert_eq!(norm("Visual-Studio", true), "visualstudio");
        assert_eq!(norm("e-mail", true), "email");
        assert_eq!(norm("a\u{2013}b", true), "ab"); // en dash
        assert_eq!(norm("a\u{2014}b", true), "ab"); // em dash
    }

    #[test]
    fn decomposes_compatibility_characters() {
        // NFKD, not NFD: roman numerals and fullwidth forms decompose.
        assert_eq!(norm("\u{2161}", false), "II"); // Roman numeral two
        assert_eq!(norm("\u{2161}", true), "ii");
        assert_eq!(norm("\u{FF21}", false), "A"); // Fullwidth A
    }

    #[test]
    fn keeps_spaces_and_other_separators() {
        assert_eq!(norm("Visual Studio Code", false), "Visual Studio Code");
        assert_eq!(norm("a/b.c_d", false), "a/b.c_d");
    }

    #[test]
    fn empty_input_yields_empty_result() {
        let n = normalize_with_result("", false);
        assert!(n.is_empty());
        assert_eq!(n.length(), 0);
        assert_eq!(n.to_string(), "");
    }

    #[test]
    fn position_map_points_back_at_the_original() {
        // "Café" -> "Cafe": every normalized char maps to its source offset,
        // and the stripped combining mark does not shift later positions.
        let n = normalize_with_result("Café", false);
        assert_eq!(n.to_string(), "Cafe");
        assert_eq!(n.length(), 4);
        for i in 0..n.length() {
            assert!(n.map_position(i) < "Café".len());
        }
        // Positions past the end clamp to the end of the original input.
        assert_eq!(n.map_position(99), "Café".len());
    }

    #[test]
    fn position_map_survives_a_dropped_dash() {
        let input = "e-mail";
        let n = normalize_with_result(input, false);
        assert_eq!(n.to_string(), "email");
        // The 'm' in the output came from index 2 of the input, not index 1.
        assert_eq!(n.map_position(1), 2);
    }

    #[test]
    fn compare_to_is_case_insensitive() {
        use std::cmp::Ordering;
        let a = normalize_with_result("Firefox", false);
        let b = normalize_with_result("firefox", false);
        assert_eq!(a.compare_to(&b), Ordering::Equal);

        let files = normalize_with_result("Files", false);
        assert_eq!(files.compare_to(&a), Ordering::Less);
        // Shorter sorts first when it is a prefix of the longer.
        let fi = normalize_with_result("Fi", false);
        assert_eq!(fi.compare_to(&files), Ordering::Less);
    }

    #[test]
    fn documents_the_gap_vs_to_lowercase() {
        // The parity gap this module closes: search.rs folds with
        // to_lowercase(), which keeps accents and dashes, so typing the plain
        // ASCII form finds nothing.
        for raw in ["Café", "Visual-Studio"] {
            assert_ne!(
                raw.to_lowercase(),
                normalize_lowercase(raw),
                "to_lowercase must differ from KISS normalization for {raw:?}"
            );
        }
        assert_eq!(normalize_lowercase("Café"), "cafe");
        assert_eq!("Café".to_lowercase(), "café"); // still accented
    }
}
