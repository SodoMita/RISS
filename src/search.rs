#[cfg(target_os = "android")]
use crate::android_app_entry::AppEntry;
#[cfg(not(target_os = "android"))]
use crate::app_entry::AppEntry;
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

/// Ranking for the no-query default list.
///
/// The *query* search path does not live here any more: it is
/// [`crate::kiss::searcher`], a port of KISS's `AppProvider.requestResults` +
/// `QuerySearcher` + `Searcher.onPostExecute`, and it runs on the search
/// worker thread. What is left in this file is the synthetic provider rows
/// and the default (empty-query) list, neither of which is part of the
/// application-search path.
pub struct SearchEngine;

impl SearchEngine {
    pub fn new() -> Self {
        Self
    }

    /// Get default apps to show when no search query (favorites + most used)
    pub fn get_default_apps(&self, apps: &[AppEntry], max_results: usize) -> Vec<SearchResult> {
        // Ranked by index, cloned after truncation.
        let mut scored: Vec<(i64, MatchType, usize)> = Vec::new();
        for (idx, app) in apps.iter().enumerate() {
            if !(app.is_favorite || app.launch_count > 0) {
                continue;
            }
            let score = if app.is_favorite {
                10000 + app.launch_count as i64
            } else {
                app.launch_count as i64
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

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// The name shown for an app, honoring the user's rename (KISS alias).
pub fn display_name(app: &AppEntry, aliases: &HashMap<String, String>) -> String {
    match aliases.get(&app.exec) {
        Some(alias) => alias.clone(),
        None => app.name.clone(),
    }
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

    /// Issue #21's "learn from every match type" boost is **gone**, along
    /// with the `Exact`/`Prefix`/`Tag`/`Category` ladder it rode on.
    ///
    /// KISS has no such ladder: `AppProvider.requestResults` produces one
    /// `FuzzyScoreV2` score for the name, one for the tags, keeps the larger,
    /// and the only history influence is `QuerySearcher`'s per-query
    /// `knownIds` boost. A lifetime launch counter that lifts an app for
    /// *every* query is an RISS invention, and inventions are what this port
    /// removes. The behaviour now lives in `crate::kiss::searcher` and is
    /// tested there and through the live path in `crate::tests::live_search`.
    #[test]
    fn the_old_match_type_ladder_is_no_longer_a_search_path() {
        // The type still exists because provider rows are tagged with it,
        // but nothing scores apps with it any more: `SearchEngine` has no
        // `search` method at all.
        let engine = SearchEngine::new();
        let fresh = AppEntry::virtual_entry("Fire Tools", "utils", "fire-tools");
        let mut used = AppEntry::virtual_entry("Firefox", "browser", "firefox");
        used.launch_count = 50;
        // The only list `SearchEngine` still builds is the empty-query one,
        // which is not the application-search path.
        let default = engine.get_default_apps(&[fresh, used], 10);
        assert_eq!(default.len(), 1, "only launched/favourite apps are listed");
        assert_eq!(default[0].entry.exec, "firefox");
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
