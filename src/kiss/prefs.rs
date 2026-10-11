//! The preference keys, defaults and parsing the live application-search path
//! depends on.
//!
//! Upstream: Neamar/KISS @ `17fc7ebd473bbcbfc0e93592859278443cdb4ec0`
//! (`searcher/QuerySearcher.java`, `searcher/Searcher.java`,
//! `db/HistoryMode.java`, `dataprovider/AppProvider.java`,
//! `res/xml/preferences.xml`).
//!
//! | key | XML default | used by |
//! |---|---|---|
//! | `number-of-display-elements` | `"20"` | `QuerySearcher.getMaxResultCount` |
//! | `history-mode` | `"recency"` | `QuerySearcher`, `DBHelper.getHistory` |
//! | `enable-excluded-apps` | `false` | `AppProvider.requestResults` |
//! | `use-fuzzy-score-v1` | `false` | `FuzzyFactory` (V2 is the default path) |
//!
//! No key, default or parsing rule here was invented: each one is read off the
//! pinned source.

/// `Searcher.DEFAULT_MAX_RESULTS`.
///
/// Note this is *not* the value a fresh install uses: it is the fallback
/// `getString` returns when the preference is absent. `preferences.xml`
/// declares `app:defaultValue="20"`, so a real device reads 20. Both numbers
/// are therefore part of the port.
pub const DEFAULT_MAX_RESULTS: i32 = 50;

/// `preferences.xml` `app:defaultValue` for `number-of-display-elements`.
pub const XML_DEFAULT_MAX_RESULTS: &str = "20";

/// `fr.neamar.kiss.db.HistoryMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HistoryMode {
    #[default]
    Recency,
    Frecency,
    Frequency,
    Adaptive,
    Time,
    Alphabetically,
}

impl HistoryMode {
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

    /// `HistoryMode.valueById(String)`: unknown ids fall back to `RECENCY`,
    /// they do not throw.
    pub fn value_by_id(id: &str) -> HistoryMode {
        const ALL: [HistoryMode; 6] = [
            HistoryMode::Recency,
            HistoryMode::Frecency,
            HistoryMode::Frequency,
            HistoryMode::Adaptive,
            HistoryMode::Time,
            HistoryMode::Alphabetically,
        ];
        ALL.into_iter()
            .find(|m| m.id() == id)
            .unwrap_or(HistoryMode::Recency)
    }
}

/// `java.lang.Double.valueOf(String)`, restricted to the spellings a
/// preference string can plausibly hold.
///
/// Deliberately not `str::parse::<f64>()`: Rust accepts `"inf"`, `"nan"` and
/// any ASCII-case spelling of `"infinity"`, none of which Java accepts, and
/// Rust rejects the trailing type suffix (`"20d"`) and leading/trailing
/// whitespace that Java accepts.
///
/// **Named deviation**: Java also accepts hexadecimal floating point
/// (`"0x1p4"` -> 16.0). This returns `None` for those, so they take the
/// `NumberFormatException` branch and yield `DEFAULT_MAX_RESULTS`. The case is
/// unreachable from KISS's own UI -- `RangeEditTextPreference` is a numeric
/// field clamped to 0..=150 -- and it is covered by a test so the deviation
/// cannot drift unnoticed.
fn java_double_value(raw: &str) -> Option<f64> {
    // `Double.valueOf` calls `String.trim()`, which strips every character
    // <= U+0020, not the Unicode whitespace set.
    let s = raw.trim_matches(|c: char| c <= ' ');
    if s.is_empty() {
        return None;
    }
    // One optional type suffix.
    let s = match s.strip_suffix(['f', 'F', 'd', 'D']) {
        Some(rest) if !rest.is_empty() => rest,
        Some(_) => return None,
        None => s,
    };
    let (sign, body) = match s.strip_prefix(['+', '-']) {
        Some(rest) => (if s.starts_with('-') { -1.0 } else { 1.0 }, rest),
        None => (1.0, s),
    };
    if body.is_empty() {
        return None;
    }
    if body == "Infinity" {
        return Some(sign * f64::INFINITY);
    }
    if body == "NaN" {
        return Some(f64::NAN);
    }
    // Decimal grammar: Digits . Digits? ExponentPart? | . Digits ExponentPart?
    // | Digits ExponentPart?
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(at) => (&body[..at], Some(&body[at + 1..])),
        None => (body, None),
    };
    let (int_part, frac_part) = match mantissa.find('.') {
        Some(at) => (&mantissa[..at], &mantissa[at + 1..]),
        None => (mantissa, ""),
    };
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    if !digits(int_part) || !digits(frac_part) {
        return None;
    }
    if let Some(exp) = exponent {
        let exp = exp.strip_prefix(['+', '-']).unwrap_or(exp);
        if exp.is_empty() || !digits(exp) {
            return None;
        }
    }
    // The grammar has been checked; the conversion itself is IEEE-754 and
    // identical in both languages.
    s.parse::<f64>().ok()
}

/// The JLS narrowing primitive conversion `double -> int`, which is what
/// `Double.intValue()` performs: NaN becomes 0, out-of-range saturates, and
/// anything else truncates toward zero.
fn java_double_to_int(v: f64) -> i32 {
    if v.is_nan() {
        0
    } else if v >= i32::MAX as f64 {
        i32::MAX
    } else if v <= i32::MIN as f64 {
        i32::MIN
    } else {
        v.trunc() as i32
    }
}

/// `QuerySearcher.getMaxResultCount()`.
///
/// ```text
/// MAX_RESULT_COUNT = Double.valueOf(prefs.getString(
///     "number-of-display-elements", String.valueOf(DEFAULT_MAX_RESULTS))).intValue();
/// ```
/// wrapped in `catch (NumberFormatException e) { MAX_RESULT_COUNT = DEFAULT_MAX_RESULTS; }`
/// (upstream e9a0c64c7, "Ensure empty number of results doesn't crash the app").
///
/// `raw = None` models the preference being absent, in which case `getString`
/// hands back `String.valueOf(DEFAULT_MAX_RESULTS)`.
///
/// The result is **not** clamped to a sane range: a stored `0` really does
/// mean "show nothing", and upstream's `onPostExecute` trims the queue to
/// empty in that case. Replacing that with "0 means unlimited" would be an
/// invention, and the RISS code this replaces made exactly that invention.
pub fn max_result_count(raw: Option<&str>) -> i32 {
    let raw = raw.unwrap_or(DEFAULT_MAX_RESULTS_STR);
    match java_double_value(raw) {
        Some(v) => java_double_to_int(v),
        None => DEFAULT_MAX_RESULTS,
    }
}

const DEFAULT_MAX_RESULTS_STR: &str = "50";

/// `getMaxResultCount` as a count usable by `Vec::truncate`. Negative
/// preferences cannot ask for fewer than zero rows.
pub fn max_result_count_usize(raw: Option<&str>) -> usize {
    max_result_count(raw).max(0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_mode_ids_and_fallback() {
        assert_eq!(HistoryMode::value_by_id("recency"), HistoryMode::Recency);
        assert_eq!(HistoryMode::value_by_id("frecency"), HistoryMode::Frecency);
        assert_eq!(
            HistoryMode::value_by_id("frequency"),
            HistoryMode::Frequency
        );
        assert_eq!(HistoryMode::value_by_id("adaptive"), HistoryMode::Adaptive);
        assert_eq!(HistoryMode::value_by_id("time"), HistoryMode::Time);
        assert_eq!(
            HistoryMode::value_by_id("alphabetically"),
            HistoryMode::Alphabetically
        );
        // Unknown and empty fall back, they do not panic.
        assert_eq!(HistoryMode::value_by_id("nonsense"), HistoryMode::Recency);
        assert_eq!(HistoryMode::value_by_id(""), HistoryMode::Recency);
        assert_eq!(HistoryMode::default(), HistoryMode::Recency);
    }

    #[test]
    fn max_result_count_parses_like_java() {
        assert_eq!(max_result_count(Some("20")), 20);
        assert_eq!(max_result_count(Some("0")), 0);
        assert_eq!(max_result_count(Some("150")), 150);
        // Double-then-truncate: a fractional preference truncates toward zero.
        assert_eq!(max_result_count(Some("20.9")), 20);
        assert_eq!(max_result_count(Some("-20.9")), -20);
        assert_eq!(max_result_count(Some("2e2")), 200);
        assert_eq!(max_result_count(Some("20d")), 20);
        assert_eq!(max_result_count(Some("  20  ")), 20);
        // The whole reason for the Double detour: a value beyond int range
        // saturates instead of throwing.
        assert_eq!(max_result_count(Some("99999999999999")), i32::MAX);
        assert_eq!(max_result_count(Some("-99999999999999")), i32::MIN);
        assert_eq!(max_result_count(Some("Infinity")), i32::MAX);
        assert_eq!(max_result_count(Some("NaN")), 0);
        // NumberFormatException -> DEFAULT_MAX_RESULTS.
        assert_eq!(max_result_count(Some("")), DEFAULT_MAX_RESULTS);
        assert_eq!(max_result_count(Some("   ")), DEFAULT_MAX_RESULTS);
        assert_eq!(max_result_count(Some("abc")), DEFAULT_MAX_RESULTS);
        assert_eq!(max_result_count(Some("1,5")), DEFAULT_MAX_RESULTS);
        // Absent preference -> getString's default.
        assert_eq!(max_result_count(None), DEFAULT_MAX_RESULTS);
    }

    #[test]
    fn rust_only_float_spellings_are_rejected_like_java() {
        for raw in ["inf", "infinity", "INFINITY", "nan", "NAN", "+inf"] {
            assert_eq!(
                max_result_count(Some(raw)),
                DEFAULT_MAX_RESULTS,
                "{raw:?} must not parse: Java's Double.valueOf rejects it"
            );
        }
    }

    /// The one documented deviation, pinned so it cannot change silently.
    #[test]
    fn hexadecimal_float_literals_are_a_named_deviation() {
        assert_eq!(
            max_result_count(Some("0x1p4")),
            DEFAULT_MAX_RESULTS,
            "Java would answer 16 here; RISS falls back. See the doc comment."
        );
    }

    #[test]
    fn usize_view_floors_at_zero() {
        assert_eq!(max_result_count_usize(Some("-5")), 0);
        assert_eq!(max_result_count_usize(Some("7")), 7);
        assert_eq!(max_result_count_usize(Some("0")), 0);
    }
}
