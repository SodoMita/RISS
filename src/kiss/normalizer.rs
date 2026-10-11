//! Faithful port of KISS `StringNormalizer`.
//!
//! Upstream: `app/src/main/java/fr/neamar/kiss/normalizer/StringNormalizer.java`
//! (Neamar/KISS). `normalizeWithResult` does, per input code point:
//!
//!   1. NFKD-decompose it (`java.text.Normalizer`, `Form.NFKD`),
//!   2. drop every decomposed code point whose `Character.getType` is
//!      `NON_SPACING_MARK`, `COMBINING_SPACING_MARK` or `DASH_PUNCTUATION`,
//!   3. lowercase the survivors (`Character.toLowerCase`).
//!
//! KISS applies this ONE function to both pojo names (at load, stored as
//! `Pojo.normalizedName`) and to the query, so dashes and diacritics fold on
//! both sides: "Café" is findable by typing "cafe", "Visual-Studio" by typing
//! "visualstudio".
//!
//! Why this matters here: `search.rs` folds with `to_lowercase()`, which does
//! none of the above. See `tests::documents_the_gap_vs_to_lowercase`.
//!
//! Rust mapping: `unicode-normalization` provides NFKD (the
//! `java.text.Normalizer` equivalent). Rust std has no `Character.getType`, so
//! `is_stripped` reproduces the three general categories KISS drops. KISS also
//! returns a position map (for highlighting matched spans); nothing in RISS
//! consumes it yet, so it is deliberately NOT ported rather than faked.

use unicode_normalization::UnicodeNormalization;

/// KISS `StringNormalizer.normalize(input)`.
///
/// Used for BOTH names (once, at load) and queries (once per keystroke),
/// exactly as KISS does.
pub fn normalize(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        // KISS decomposes each input code point independently.
        for decomposed in ch.to_string().nfkd() {
            if is_stripped(decomposed) {
                continue;
            }
            for lower in decomposed.to_lowercase() {
                out.push(lower);
            }
        }
    }
    out
}

/// Replicates the `Character.getType` classes KISS skips:
/// `NON_SPACING_MARK` (Mn), `COMBINING_SPACING_MARK` (Mc/Me),
/// `DASH_PUNCTUATION` (Pd).
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

    #[test]
    fn strips_diacritics_via_nfkd() {
        assert_eq!(normalize("Café"), "cafe");
        assert_eq!(normalize("ÜBER"), "uber");
        assert_eq!(normalize("Ångström"), "angstrom");
    }

    #[test]
    fn strips_dashes_on_both_sides() {
        assert_eq!(normalize("Visual-Studio"), "visualstudio");
        assert_eq!(normalize("e-mail"), "email");
        assert_eq!(normalize("a\u{2013}b"), "ab"); // en dash
        assert_eq!(normalize("a\u{2014}b"), "ab"); // em dash
    }

    #[test]
    fn decomposes_compatibility_characters() {
        // NFKD, not NFD: roman numerals and fullwidth forms decompose.
        assert_eq!(normalize("\u{2161}"), "ii"); // Roman numeral two
        assert_eq!(normalize("\u{FF21}"), "a"); // Fullwidth A
    }

    #[test]
    fn keeps_spaces_and_other_separators() {
        assert_eq!(normalize("Visual Studio Code"), "visual studio code");
        assert_eq!(normalize("a/b.c_d"), "a/b.c_d");
    }

    #[test]
    fn empty_input_is_empty() {
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn documents_the_gap_vs_to_lowercase() {
        // The parity gap this module closes: search.rs folds with
        // to_lowercase(), which keeps accents and dashes, so typing the plain
        // ASCII form finds nothing.
        for raw in ["Café", "Visual-Studio"] {
            assert_ne!(
                raw.to_lowercase(),
                normalize(raw),
                "to_lowercase must differ from KISS normalize for {raw:?}"
            );
        }
        assert_eq!(normalize("Café"), "cafe");
        assert_eq!("Café".to_lowercase(), "café"); // still accented
    }
}
