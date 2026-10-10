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
        let mut results: Vec<SearchResult> = apps
            .iter()
            .filter_map(|app| {
                let name_lower = display_name(app, aliases).to_lowercase();
                let searchable = searchable_text(app, aliases);

                // Exact match on name
                if name_lower == query_lower {
                    return Some(SearchResult {
                        entry: app.clone(),
                        score: 10000,
                        match_type: MatchType::Exact,
                        action: ResultAction::Launch,
                    });
                }

                // Prefix match on name
                if name_lower.starts_with(&query_lower) {
                    let score = 5000 + (100 - name_lower.len() as i64);
                    return Some(SearchResult {
                        entry: app.clone(),
                        score,
                        match_type: MatchType::Prefix,
                        action: ResultAction::Launch,
                    });
                }

                // Word match inside the name ("fire" matching "Firefox Nightly")
                if name_lower
                    .split_whitespace()
                    .any(|w| w.starts_with(&query_lower))
                {
                    return Some(SearchResult {
                        entry: app.clone(),
                        score: 4000,
                        match_type: MatchType::Prefix,
                        action: ResultAction::Launch,
                    });
                }

                // Tag match
                for tag in &app.tags {
                    if tag.to_lowercase().starts_with(&query_lower) {
                        return Some(SearchResult {
                            entry: app.clone(),
                            score: 3000,
                            match_type: MatchType::Tag,
                            action: ResultAction::Launch,
                        });
                    }
                }

                // Category match
                for cat in &app.categories {
                    if cat.to_lowercase().starts_with(&query_lower) {
                        return Some(SearchResult {
                            entry: app.clone(),
                            score: 2000,
                            match_type: MatchType::Category,
                            action: ResultAction::Launch,
                        });
                    }
                }

                // Fuzzy match on searchable text
                if let Some(score) = self.matcher.fuzzy_match(&searchable, query) {
                    // Boost by launch count
                    let boosted = score + (app.launch_count as i64 * 10);
                    return Some(SearchResult {
                        entry: app.clone(),
                        score: boosted,
                        match_type: MatchType::Fuzzy,
                        action: ResultAction::Launch,
                    });
                }

                None
            })
            .collect();

        // Sort by score descending, then by name so the order is stable.
        results.sort_by(|a, b| {
            b.score.cmp(&a.score).then_with(|| {
                display_name(&a.entry, aliases)
                    .to_lowercase()
                    .cmp(&display_name(&b.entry, aliases).to_lowercase())
            })
        });
        results.truncate(max_results);
        results
    }

    /// Get default apps to show when no search query (favorites + most used)
    pub fn get_default_apps(&self, apps: &[AppEntry], max_results: usize) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = apps
            .iter()
            .filter(|app| app.is_favorite || app.launch_count > 0)
            .map(|app| {
                let score = if app.is_favorite {
                    10000 + app.launch_count as i64
                } else {
                    app.launch_count as i64
                };
                SearchResult {
                    entry: app.clone(),
                    score,
                    match_type: if app.is_favorite {
                        MatchType::Exact
                    } else {
                        MatchType::Fuzzy
                    },
                    action: ResultAction::Launch,
                }
            })
            .collect();

        results.sort_by_key(|a| std::cmp::Reverse(a.score));
        results.truncate(max_results);
        results
    }
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

/// Simple recursive descent parser for math expressions
fn evaluate_expression(input: &str) -> Option<f64> {
    let input = input.trim();
    let mut chars = input.chars().peekable();

    fn parse_expr(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<f64> {
        let mut left = parse_term(chars)?;

        loop {
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

        if chars.peek() == Some(&'^') {
            chars.next();
            let exp = parse_power(chars)?; // Right associative
            Some(base.powf(exp))
        } else {
            Some(base)
        }
    }

    fn parse_factor(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<f64> {
        // Skip whitespace
        while chars.peek() == Some(&' ') {
            chars.next();
        }

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

        num_str.parse().ok()
    }

    parse_expr(&mut chars)
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
}
