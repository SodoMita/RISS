//! StringNormalizer - 1:1 port of KISS `fr.neamar.kiss.normalizer.StringNormalizer`
//!
//! Handles accented characters, diacritics stripping, case normalization,
//! and maps match positions back to original input strings.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizerResult {
    pub text: String,
    pub mapping: Vec<usize>,
}

impl NormalizerResult {
    pub fn empty() -> Self {
        Self {
            text: String::new(),
            mapping: Vec::new(),
        }
    }
}

pub struct StringNormalizer;

impl StringNormalizer {
    /// Normalizes a string according to KISS logic:
    /// 1. Basic latin ASCII fast path.
    /// 2. NFKD-like diacritics removal for unicode characters.
    /// 3. Drops hyphens/dashes for seamless matching (e.g. "wi-fi" -> "wifi").
    /// 4. Optionally lowercases.
    pub fn normalize(input: &str, lowercase: bool) -> NormalizerResult {
        if input.is_empty() {
            return NormalizerResult::empty();
        }

        let mut text = String::with_capacity(input.len());
        let mut mapping = Vec::with_capacity(input.len());

        for (byte_idx, ch) in input.char_indices() {
            // ASCII fast path
            if ch <= 'z' {
                if ch == '-' || ch == '–' || ch == '—' {
                    continue;
                }
                let mapped = if lowercase {
                    ch.to_ascii_lowercase()
                } else {
                    ch
                };
                text.push(mapped);
                mapping.push(byte_idx);
            } else {
                // Diacritics stripping: decomposition
                let ch_str = ch.to_string();
                for decomposed in ch_str.chars() {
                    // Check if combining mark (Mn, Mc, Me)
                    if is_combining_mark(decomposed)
                        || decomposed == '-'
                        || decomposed == '–'
                        || decomposed == '—'
                    {
                        continue;
                    }
                    let simplified = strip_accent(decomposed);
                    let final_ch = if lowercase {
                        simplified.to_lowercase().next().unwrap_or(simplified)
                    } else {
                        simplified
                    };
                    text.push(final_ch);
                    mapping.push(byte_idx);
                }
            }
        }

        NormalizerResult { text, mapping }
    }
}

fn is_combining_mark(c: char) -> bool {
    // Unicode combining character range
    ('\u{0300}'..='\u{036F}').contains(&c)
        || ('\u{1AB0}'..='\u{1AFF}').contains(&c)
        || ('\u{1DC0}'..='\u{1DFF}').contains(&c)
        || ('\u{20D0}'..='\u{20FF}').contains(&c)
        || ('\u{FE20}'..='\u{FE2F}').contains(&c)
}

fn strip_accent(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' | 'ą' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'ī' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ō' | 'ø' | 'ő' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => 'u',
        'ý' | 'ÿ' => 'y',
        'ç' | 'ć' | 'č' => 'c',
        'ñ' | 'ń' | 'ň' => 'n',
        'ś' | 'š' => 's',
        'ź' | 'ż' | 'ž' => 'z',
        'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' | 'Ā' | 'Ą' => 'A',
        'É' | 'È' | 'Ê' | 'Ë' | 'Ē' | 'Ę' | 'Ě' => 'E',
        'Í' | 'Ì' | 'Î' | 'Ï' | 'Ī' => 'I',
        'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' | 'Ō' | 'Ø' | 'Ő' => 'O',
        'Ú' | 'Ù' | 'Û' | 'Ü' | 'Ū' | 'Ů' | 'Ű' => 'U',
        'Ý' | 'Ÿ' => 'Y',
        'Ç' | 'Ć' | 'Č' => 'C',
        'Ñ' | 'Ń' | 'Ň' => 'N',
        'Ś' | 'Š' => 'S',
        'Ź' | 'Ż' | 'Ž' => 'Z',
        other => other,
    }
}
