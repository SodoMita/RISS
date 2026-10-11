#[cfg(target_os = "android")]
use crate::android_app_entry::AppEntry;
#[cfg(not(target_os = "android"))]
use crate::app_entry::AppEntry;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::collections::HashMap;

/// Result of a search with relevance score
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub entry: AppEntry,
    pub score: i64,
    pub match_type: MatchType,
    /// What happens when the result is activated.
    pub action: ResultAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchType {
    Exact,
    Prefix,
    Fuzzy,
    Tag,
    Category,
    /// A row contributed by a provider (web search, timer, settings…).
    Provider,
}

/// Special lists reachable from the search bar (KISS' info bar buttons).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultView {
    History,
    AllApps,
    Settings,
    /// Applications hidden from the results; activating one restores it.
    Excluded,
}

/// What happens when a result is activated.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultAction {
    /// Launch the application.
    Launch,
    /// Open a web search in the browser.
    WebSearch { provider: String, query: String },
    /// Run a shell command.
    Exec { command: String },
    /// Start a countdown; activating again cancels it.
    Timer { seconds: u64 },
    /// Copy the text to the clipboard.
    Copy { text: String },
    /// Jump to a row of the settings screen.
    Setting { id: String },
    /// Show one of the special lists.
    View(ResultView),
    /// An excluded (hidden) application; activating restores it.
    Excluded { exec: String },
}

/// Search engine for apps
pub struct SearchEngine {
    matcher: SkimMatcherV2,
}

/// Case-insensitive `starts_with` that allocates only when the text is not
/// ASCII. `str::to_lowercase` is not the same function as ASCII folding for
/// non-ASCII input ("\u{130}" lowercases to two chars), so the Unicode path is kept
/// for exactly the inputs where it matters and buffered for the ones where it
/// does not.
fn starts_with_ignore_case(hay: &str, needle: &str, buf: &mut String) -> bool {
    if hay.is_ascii() && needle.is_ascii() {
        hay.get(..needle.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(needle))
    } else {
        lower_into(hay, buf);
        buf.starts_with(needle)
    }
}

fn eq_ignore_case(hay: &str, needle: &str, buf: &mut String) -> bool {
    if hay.is_ascii() && needle.is_ascii() {
        hay.len() == needle.len() && hay.as_bytes().eq_ignore_ascii_case(needle.as_bytes())
    } else {
        lower_into(hay, buf);
        *buf == *needle
    }
}

/// `s.to_lowercase()` without the allocation, reusing `buf`'s capacity.
fn lower_into(s: &str, buf: &mut String) {
    buf.clear();
    buf.reserve(s.len());
    buf.extend(s.chars().flat_map(char::to_lowercase));
}

/// `display_name` without the `String` it builds: every candidate in a search
/// used to pay a clone of its name (or alias) per keystroke.
fn display_name_ref<'a>(app: &'a AppEntry, aliases: &'a HashMap<String, String>) -> &'a str {
    match aliases.get(&app.exec) {
        Some(alias) => alias.as_str(),
        None => app.name.as_str(),
    }
}

/// `searchable_text(app, aliases)` written into a caller-owned buffer.
fn write_searchable_text(app: &AppEntry, aliases: &HashMap<String, String>, out: &mut String) {
    // The buffer is reused across candidates, so it has to be emptied first:
    // appending instead would let candidate N+1 fuzzy-match against the text of
    // candidates 0..=N.
    out.clear();
    if let Some(alias) = aliases.get(&app.exec) {
        out.push_str(alias);
        out.push(' ');
    }
    out.push_str(&app.name);
    if !app.comment.is_empty() {
        out.push(' ');
        out.push_str(&app.comment);
    }
    for tag in &app.tags {
        out.push(' ');
        out.push_str(tag);
    }
    for cat in &app.categories {
        out.push(' ');
        out.push_str(cat);
    }
}

/// Display-name tie-break for equal scores, allocation-free on ASCII.
fn cmp_display_name(
    a: &AppEntry,
    b: &AppEntry,
    aliases: &HashMap<String, String>,
    scratch: &mut String,
) -> std::cmp::Ordering {
    let x = display_name_ref(a, aliases);
    let y = display_name_ref(b, aliases);
    if x.is_ascii() && y.is_ascii() {
        x.bytes()
            .map(|c| c.to_ascii_lowercase())
            .cmp(y.bytes().map(|c| c.to_ascii_lowercase()))
    } else {
        let _ = scratch;
        x.to_lowercase().cmp(&y.to_lowercase())
    }
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            matcher: SkimMatcherV2::default().smart_case(),
        }
    }

    /// Search through apps with the given query
    pub fn search(
        &self,
        query: &str,
        apps: &[AppEntry],
        aliases: &HashMap<String, String>,
        max_results: usize,
    ) -> Vec<SearchResult> {
        let query = query.trim();

        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();

        // Candidates are ranked as (score, kind, index) over the borrowed pool
        // and cloned only after truncation: a two-character query matched most
        // of the app list, so the old code deep-cloned every AppEntry -- six
        // heap allocations each -- to throw all but `max_results` away, and the
        // comparator then built two more lowercased strings per comparison
        // (issue #37, "allocations in hot loops").
        let mut scored: Vec<(i64, MatchType, usize)> = Vec::new();
        let mut name_buf = String::new();
        let mut text_buf = String::new();

        for (idx, app) in apps.iter().enumerate() {
            let name = display_name_ref(app, aliases);
            let boost = usage_boost(app);
            let mut hit: Option<(i64, MatchType)> = None;

            // Exact match on name.
            if eq_ignore_case(name, &query_lower, &mut name_buf) {
                hit = Some((10000 + boost, MatchType::Exact));
            }
            // Prefix match on name.
            if hit.is_none() && starts_with_ignore_case(name, &query_lower, &mut name_buf) {
                let len = if name.is_ascii() {
                    name.len()
                } else {
                    lower_into(name, &mut name_buf);
                    name_buf.len()
                };
                hit = Some((5000 + (100 - len as i64) + boost, MatchType::Prefix));
            }
            // Word match inside the name ("fire" matching "Firefox Nightly").
            if hit.is_none() {
                let words = if name.is_ascii() {
                    name.split_whitespace()
                        .any(|w| starts_with_ignore_case(w, &query_lower, &mut name_buf))
                } else {
                    lower_into(name, &mut name_buf);
                    name_buf
                        .split_whitespace()
                        .any(|w| w.starts_with(&query_lower))
                };
                if words {
                    hit = Some((4000 + boost, MatchType::Prefix));
                }
            }
            // Tag match.
            if hit.is_none() {
                for tag in &app.tags {
                    if starts_with_ignore_case(tag, &query_lower, &mut name_buf) {
                        hit = Some((3000 + boost, MatchType::Tag));
                        break;
                    }
                }
            }
            // Category match.
            if hit.is_none() {
                for cat in &app.categories {
                    if starts_with_ignore_case(cat, &query_lower, &mut name_buf) {
                        hit = Some((2000 + boost, MatchType::Category));
                        break;
                    }
                }
            }
            // Fuzzy match on the searchable blob, built only for candidates
            // that nothing cheaper decided about.
            if hit.is_none() {
                write_searchable_text(app, aliases, &mut text_buf);
                if let Some(fuzzy) = self.matcher.fuzzy_match(&text_buf, query) {
                    hit = Some((fuzzy + boost, MatchType::Fuzzy));
                }
            }

            if let Some((score, match_type)) = hit {
                scored.push((score, match_type, idx));
            }
        }

        // Sort by score descending, then by name so the order is stable.
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| cmp_display_name(&apps[a.2], &apps[b.2], aliases, &mut name_buf))
        });
        scored.truncate(max_results);
        let results: Vec<SearchResult> = scored
            .into_iter()
            .map(|(score, match_type, idx)| SearchResult {
                entry: apps[idx].clone(),
                score,
                match_type,
                action: ResultAction::Launch,
            })
            .collect();
        results
    }

    /// Get default apps to show when no search query (favorites + recently used).
    pub fn get_default_apps(&self, apps: &[AppEntry], max_results: usize) -> Vec<SearchResult> {
        // Ranked by index, cloned after truncation, as in `search`.
        //
        // Favorites are pinned above every non-favorite. Among non-favorites the
        // order is recency (`last_launched`, most-recent first) rather than
        // lifetime launch count: ranking by count first and only *then*
        // truncating to `max_results` could drop a just-launched app while
        // keeping apps launched long ago (issue #22). An app counts as "used" as
        // soon as it has any of a favorite flag, a launch count, or a recorded
        // launch time.
        const FAVORITE_FLOOR: i64 = 1i64 << 62;
        let mut scored: Vec<(i64, MatchType, usize)> = Vec::new();
        for (idx, app) in apps.iter().enumerate() {
            if !(app.is_favorite || app.launch_count > 0 || app.last_launched > 0) {
                continue;
            }
            let score = if app.is_favorite {
                FAVORITE_FLOOR + app.launch_count as i64
            } else {
                app.last_launched as i64
            };
            let kind = if app.is_favorite {
                MatchType::Exact
            } else {
                MatchType::Fuzzy
            };
            scored.push((score, kind, idx));
        }
        scored.sort_by_key(|a| std::cmp::Reverse(a.0));
        scored.truncate(max_results);
        scored
            .into_iter()
            .map(|(score, match_type, idx)| SearchResult {
                entry: apps[idx].clone(),
                score,
                match_type,
                action: ResultAction::Launch,
            })
            .collect()
    }
}

/// Usage boost shared by every match type (issue #21): frequently launched
/// apps rank higher regardless of how they matched, like KISS.
fn usage_boost(app: &AppEntry) -> i64 {
    app.launch_count as i64 * 10
}

/// The name shown for an app, honoring the user's rename (KISS alias).
pub fn display_name(app: &AppEntry, aliases: &HashMap<String, String>) -> String {
    match aliases.get(&app.exec) {
        Some(alias) => alias.clone(),
        None => app.name.clone(),
    }
}

/// Searchable text, including any rename the user gave the app.
fn searchable_text(app: &AppEntry, aliases: &HashMap<String, String>) -> String {
    let mut text = app.searchable_text();
    if let Some(alias) = aliases.get(&app.exec) {
        text.insert(0, ' ');
        text.insert_str(0, alias);
    }
    text
}

// --- Synthetic results used by the optional providers ---------------------

/// A web search result for `query`, using `template` as the URL of the
/// provider (`{}` or `%s` is replaced by the encoded query).
pub fn web_search_result(provider: &str, template: &str, query: &str) -> SearchResult {
    let url = crate::providers::provider_url(template, query);
    SearchResult {
        entry: AppEntry::virtual_entry(&format!("“{}”", query), provider, &url),
        score: 400,
        match_type: MatchType::Provider,
        action: ResultAction::WebSearch {
            provider: provider.to_string(),
            query: query.to_string(),
        },
    }
}

pub fn exec_result(command: &str) -> SearchResult {
    SearchResult {
        entry: AppEntry::virtual_entry(&format!("Run “{}”", command), "Shell command", command),
        score: 300,
        match_type: MatchType::Provider,
        action: ResultAction::Exec {
            command: command.to_string(),
        },
    }
}

pub fn timer_result(seconds: u64) -> SearchResult {
    SearchResult {
        entry: AppEntry::virtual_entry(
            &format!("Wait {}", crate::providers::format_duration(seconds)),
            "Timer — activate to start",
            "",
        ),
        score: 500,
        match_type: MatchType::Provider,
        action: ResultAction::Timer { seconds },
    }
}

pub fn copy_result(text: &str, label: &str) -> SearchResult {
    SearchResult {
        entry: AppEntry::virtual_entry(text, label, ""),
        score: 800,
        match_type: MatchType::Provider,
        action: ResultAction::Copy {
            text: text.to_string(),
        },
    }
}

pub fn setting_result(title: &str, section: &str, id: &str, value: &str) -> SearchResult {
    SearchResult {
        entry: AppEntry::virtual_entry(
            &format!("{} · {}", section, title),
            if value.is_empty() { "Setting" } else { value },
            id,
        ),
        score: 900,
        match_type: MatchType::Provider,
        action: ResultAction::Setting { id: id.to_string() },
    }
}

pub fn view_result(view: ResultView, title: &str, comment: &str) -> SearchResult {
    SearchResult {
        entry: AppEntry::virtual_entry(title, comment, ""),
        score: 900,
        match_type: MatchType::Provider,
        action: ResultAction::View(view),
    }
}

/// Try to evaluate a mathematical expression
pub fn try_calculate(input: &str) -> Option<String> {
    let input = input.trim();

    // Only try if it looks like a math expression
    if !crate::providers::looks_like_math(input) {
        return None;
    }

    // Simple expression evaluator
    // Supports: +, -, *, /, ^, parentheses
    let result = evaluate_expression(input)?;

    // Format nicely
    if result.fract() == 0.0 && result.abs() < 1e15 {
        Some(format!("= {}", result as i64))
    } else {
        Some(
            format!("= {:.6}", result)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string(),
        )
    }
}

/// Simple recursive descent parser for math expressions.
///
/// The whole input must be one valid expression (issue #23): trailing
/// garbage such as the `abc` in `2+2abc` rejects the input, so it is treated
/// as a search query instead of a partial calculation.
fn evaluate_expression(input: &str) -> Option<f64> {
    let input = input.trim();
    let mut chars = input.chars().peekable();

    fn skip_ws(chars: &mut std::iter::Peekable<std::str::Chars>) {
        while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
            chars.next();
        }
    }

    fn parse_expr(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<f64> {
        let mut left = parse_term(chars)?;

        loop {
            skip_ws(chars);
            match chars.peek() {
                Some('+') => {
                    chars.next();
                    let right = parse_term(chars)?;
                    left += right;
                }
                Some('-') => {
                    chars.next();
                    let right = parse_term(chars)?;
                    left -= right;
                }
                _ => break,
            }
        }

        Some(left)
    }

    fn parse_term(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<f64> {
        let mut left = parse_power(chars)?;

        loop {
            skip_ws(chars);
            match chars.peek() {
                Some('*') => {
                    chars.next();
                    let right = parse_power(chars)?;
                    left *= right;
                }
                Some('/') => {
                    chars.next();
                    let right = parse_power(chars)?;
                    if right == 0.0 {
                        return None;
                    }
                    left /= right;
                }
                _ => break,
            }
        }

        Some(left)
    }

    fn parse_power(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<f64> {
        let base = parse_factor(chars)?;

        skip_ws(chars);
        if chars.peek() == Some(&'^') {
            chars.next();
            let exp = parse_power(chars)?; // Right associative
            Some(base.powf(exp))
        } else {
            Some(base)
        }
    }

    fn parse_factor(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<f64> {
        skip_ws(chars);

        // Handle unary minus
        if chars.peek() == Some(&'-') {
            chars.next();
            let val = parse_factor(chars)?;
            return Some(-val);
        }

        // Handle parentheses
        if chars.peek() == Some(&'(') {
            chars.next();
            let val = parse_expr(chars)?;
            skip_ws(chars);
            if chars.peek() == Some(&')') {
                chars.next();
                return Some(val);
            }
            return None;
        }

        // Parse number
        let mut num_str = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() || c == '.' {
                num_str.push(c);
                chars.next();
            } else {
                break;
            }
        }

        if num_str.is_empty() {
            return None;
        }
        num_str.parse().ok()
    }

    let value = parse_expr(&mut chars)?;
    skip_ws(&mut chars);
    // Anything left over means the input was not just math.
    if chars.peek().is_some() {
        return None;
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate() {
        assert_eq!(try_calculate("2+2"), Some("= 4".to_string()));
        assert_eq!(try_calculate("10/3"), Some("= 3.333333".to_string()));
        assert_eq!(try_calculate("2^10"), Some("= 1024".to_string()));
        assert_eq!(try_calculate("(5+3)*2"), Some("= 16".to_string()));
        assert_eq!(try_calculate("hello"), None);
    }

    #[test]
    fn aliases_rename_results() {
        let entry = AppEntry::virtual_entry("Old Name", "comment", "exec-key");
        let mut aliases = HashMap::new();
        aliases.insert("exec-key".to_string(), "New Name".to_string());
        assert_eq!(display_name(&entry, &aliases), "New Name");
        assert_eq!(display_name(&entry, &HashMap::new()), "Old Name");
    }

    #[test]
    fn virtual_entries_carry_actions() {
        let web = web_search_result("DuckDuckGo", "https://duckduckgo.com/?q={}", "hi");
        assert!(web.entry.is_virtual());
        assert_eq!(
            web.action,
            ResultAction::WebSearch {
                provider: "DuckDuckGo".to_string(),
                query: "hi".to_string(),
            }
        );
        assert_eq!(timer_result(95).entry.name, "Wait 1m 35s");
    }

    #[test]
    fn ranking_learns_from_usage_across_match_types() {
        let engine = SearchEngine::new();
        let aliases = HashMap::new();

        // Prefix matches: the used app wins (issue #21).
        let mut used = AppEntry::virtual_entry("Firefox", "browser", "firefox");
        used.launch_count = 50;
        let fresh = AppEntry::virtual_entry("Fire Tools", "utils", "fire-tools");
        let results = engine.search("fire", &[fresh, used], &aliases, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].entry.exec, "firefox");
        assert_eq!(results[0].match_type, MatchType::Prefix);

        // Exact matches learn too.
        let mut used_exact = AppEntry::virtual_entry("Terminal", "shell", "term-used");
        used_exact.launch_count = 30;
        let fresh_exact = AppEntry::virtual_entry("terminal", "shell", "term-fresh");
        let results = engine.search("terminal", &[fresh_exact, used_exact], &aliases, 10);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.match_type == MatchType::Exact));
        assert_eq!(results[0].entry.exec, "term-used");

        // Tag matches learn too.
        let mut tagged_used = AppEntry::virtual_entry("Alpha", "a", "alpha");
        tagged_used.tags = vec!["music".to_string()];
        tagged_used.launch_count = 20;
        let mut tagged_fresh = AppEntry::virtual_entry("Beta", "b", "beta");
        tagged_fresh.tags = vec!["music".to_string()];
        let results = engine.search("mus", &[tagged_fresh, tagged_used], &aliases, 10);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.match_type == MatchType::Tag));
        assert_eq!(results[0].entry.exec, "alpha");

        // Category matches learn too.
        let mut cat_used = AppEntry::virtual_entry("Gamma", "g", "gamma");
        cat_used.categories = vec!["Office".to_string()];
        cat_used.launch_count = 20;
        let mut cat_fresh = AppEntry::virtual_entry("Delta", "d", "delta");
        cat_fresh.categories = vec!["Office".to_string()];
        let results = engine.search("off", &[cat_fresh, cat_used], &aliases, 10);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.match_type == MatchType::Category));
        assert_eq!(results[0].entry.exec, "gamma");
    }

    #[test]
    fn default_apps_never_drop_a_just_launched_app() {
        // Issue #22: the default list must not omit an app merely because it
        // has a low lifetime launch count. A freshly launched app should appear
        // ahead of older, more-frequently-used apps once the list is capped.
        let engine = SearchEngine::new();

        // 20 long-time apps, each launched once a long time ago.
        let mut apps = Vec::new();
        for i in 0..20 {
            let mut app = AppEntry::virtual_entry(
                &format!("Old {i}"),
                "util",
                &format!("old-{i}"),
            );
            app.launch_count = 1;
            app.last_launched = 1_000_000 + i as u64;
            apps.push(app);
        }
        // One app launched just now but only once.
        let mut fresh = AppEntry::virtual_entry("Just Now", "util", "just-now");
        fresh.launch_count = 1;
        fresh.last_launched = 9_999_999;
        apps.push(fresh);

        let results = engine.get_default_apps(&apps, 20);
        assert!(
            results.iter().any(|r| r.entry.exec == "just-now"),
            "the just-launched app must survive the 20-item cap"
        );
        assert_eq!(results[0].entry.exec, "just-now");
    }

    #[test]
    fn default_apps_pin_favorites_above_recency() {
        let engine = SearchEngine::new();
        let mut old_fav = AppEntry::virtual_entry("Pinned", "util", "pinned");
        old_fav.is_favorite = true;
        old_fav.last_launched = 1; // ancient
        let mut fresh = AppEntry::virtual_entry("Fresh", "util", "fresh");
        fresh.last_launched = 9_999_999; // just launched
        let apps = vec![old_fav, fresh];
        let results = engine.get_default_apps(&apps, 20);
        assert_eq!(results[0].entry.exec, "pinned");
    }

    #[test]
    fn calculator_consumes_the_whole_input() {
        // Whitespace between tokens is fine (issue #23).
        assert_eq!(try_calculate("2 + 2"), Some("= 4".to_string()));
        assert_eq!(try_calculate("(5 + 3) * 2"), Some("= 16".to_string()));
        assert_eq!(try_calculate("3 * (2 + 1)"), Some("= 9".to_string()));
        assert_eq!(try_calculate("  10 / 4  "), Some("= 2.5".to_string()));
        // Trailing garbage or incomplete input is a query, not math.
        assert_eq!(try_calculate("2+2abc"), None);
        assert_eq!(try_calculate("2 + 2 x"), None);
        assert_eq!(try_calculate("2 +"), None);
        assert_eq!(try_calculate("(5+3"), None);
    }
}
