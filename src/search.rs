#[cfg(target_os = "android")]
use crate::android_app_entry::AppEntry;
#[cfg(not(target_os = "android"))]
use crate::app_entry::AppEntry;
use crate::settings::HistoryMode;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::collections::HashMap;

/// Result of a search with relevance score
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub entry: AppEntry,
    pub score: i64,
    pub match_type: MatchType,
    pub action: ResultAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchType {
    Exact,
    Prefix,
    Fuzzy,
    Tag,
    Category,
    Untagged,
    Provider,
    History,
}

/// Special lists reachable from the search bar (KISS' info bar buttons).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultView {
    History,
    AllApps,
    Settings,
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
    /// An excluded app, activating it restores it.
    Excluded { exec: String },
}

/// Everything the search engine needs to know about the user preferences.
#[derive(Debug, Clone, Default)]
pub struct SearchConfig {
    pub max_results: usize,
    /// 1 (permissive) … 6 (very strict), KISS' "min match precision".
    pub precision: u8,
    pub legacy_fuzzy: bool,
    pub exclude_favorites: bool,
    pub active_tags: Vec<String>,
    pub show_untagged: bool,
    pub track_history: bool,
}

/// Search engine for apps
pub struct SearchEngine {
    matcher: SkimMatcherV2,
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            matcher: SkimMatcherV2::default().smart_case(),
        }
    }

    /// Search through apps with the given query.
    pub fn search(
        &self,
        query: &str,
        apps: &[AppEntry],
        aliases: &HashMap<String, String>,
        config: &SearchConfig,
    ) -> Vec<SearchResult> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let mut results: Vec<SearchResult> = apps
            .iter()
            .filter(|app| !config.exclude_favorites || !app.is_favorite)
            .filter(|app| matches_tags(app, config))
            .filter_map(|app| {
                let name = display_name(app, aliases);
                let name_lower = name.to_lowercase();
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
                    let score = 5000 + (100 - name.len() as i64);
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
                    let tag_lower = tag.to_lowercase();
                    if tag_lower == query_lower || tag_lower.starts_with(&query_lower) {
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

                if config.show_untagged && app.tags.is_empty() {
                    return Some(SearchResult {
                        entry: app.clone(),
                        score: 1500,
                        match_type: MatchType::Untagged,
                        action: ResultAction::Launch,
                    });
                }

                // Fuzzy match on searchable text
                let fuzzy = if config.legacy_fuzzy {
                    legacy_fuzzy_score(&searchable_lower(&searchable), &query_lower)
                } else {
                    self.matcher.fuzzy_match(&searchable, query)
                };
                if let Some(score) = fuzzy {
                    let boost = if config.track_history {
                        app.launch_count as i64 * 10
                    } else {
                        0
                    };
                    let favorite_boost = if app.is_favorite { 500 } else { 0 };
                    return Some(SearchResult {
                        entry: app.clone(),
                        score: score + boost + favorite_boost,
                        match_type: MatchType::Fuzzy,
                        action: ResultAction::Launch,
                    });
                }

                None
            })
            .collect();

        // Sort by score descending, then alphabetically for a stable order.
        results.sort_by(|a, b| {
            b.score.cmp(&a.score).then_with(|| {
                a.entry
                    .name
                    .to_lowercase()
                    .cmp(&b.entry.name.to_lowercase())
            })
        });

        // Precision filter: only applied to fuzzy matches, a prefix or tag hit
        // is precise by construction.
        if let Some(best) = results.first().map(|r| r.score) {
            let precision = config.precision.clamp(1, 6) as f64;
            let threshold = best as f64 * (precision / 6.0);
            results.retain(|r| r.match_type != MatchType::Fuzzy || r.score as f64 >= threshold);
        }

        results.truncate(config.max_results.max(1));
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
                        MatchType::History
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

/// Default list: favorites first, then history ranked with the chosen mode.
pub fn default_results(
    apps: &[AppEntry],
    aliases: &HashMap<String, String>,
    mode: HistoryMode,
    max_results: usize,
) -> Vec<SearchResult> {
    let mut results: Vec<SearchResult> = Vec::with_capacity(apps.len());

    for app in apps {
        let score = history_score(app, mode);
        let match_type = if app.is_favorite {
            MatchType::Exact
        } else if app.launch_count > 0 {
            MatchType::History
        } else {
            MatchType::Provider
        };
        results.push(SearchResult {
            entry: app.clone(),
            score,
            match_type,
            action: ResultAction::Launch,
        });
    }

    results.sort_by(|a, b| {
        let fav_a = a.entry.is_favorite;
        let fav_b = b.entry.is_favorite;
        fav_b
            .cmp(&fav_a)
            .then_with(|| b.score.cmp(&a.score))
            .then_with(|| {
                display_name(&a.entry, aliases)
                    .to_lowercase()
                    .cmp(&display_name(&b.entry, aliases).to_lowercase())
            })
    });

    results.truncate(max_results.max(1));
    results
}

fn history_score(app: &AppEntry, mode: HistoryMode) -> i64 {
    let count = app.launch_count as i64;
    let recency = app.last_launched as i64;
    match mode {
        HistoryMode::Recency => recency,
        HistoryMode::UsageCount => count * 1000,
        HistoryMode::Frecent => count * 1000 + recency % 100_000,
    }
}

fn matches_tags(app: &AppEntry, config: &SearchConfig) -> bool {
    if config.active_tags.is_empty() {
        return true;
    }
    config
        .active_tags
        .iter()
        .all(|wanted| app.tags.iter().any(|tag| tag == wanted))
}

pub fn display_name(app: &AppEntry, aliases: &HashMap<String, String>) -> String {
    match aliases.get(&app.exec) {
        Some(alias) => alias.clone(),
        None => app.name.clone(),
    }
}

fn searchable_text(app: &AppEntry, aliases: &HashMap<String, String>) -> String {
    let mut text = app.searchable_text();
    if let Some(alias) = aliases.get(&app.exec) {
        text.insert_str(0, alias);
        text.insert(0, ' ');
    }
    text
}

fn searchable_lower(text: &str) -> String {
    text.to_lowercase()
}

/// A deliberately simple matcher used by the "legacy fuzzy search" option:
/// every query character must appear in order, contiguity is rewarded.
pub fn legacy_fuzzy_score(candidate: &str, query: &str) -> Option<i64> {
    let haystack: Vec<char> = candidate.chars().collect();
    let needle: Vec<char> = query.chars().collect();
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    let mut position = 0usize;
    let mut score = 0i64;
    let mut previous = None;
    for c in needle {
        let mut found = None;
        for (index, h) in haystack.iter().enumerate().skip(position) {
            if *h == c {
                found = Some(index);
                break;
            }
        }
        let index = found?;
        score += 100;
        if let Some(prev) = previous {
            if index == prev + 1 {
                score += 50;
            }
        }
        previous = Some(index);
        position = index + 1;
    }
    // Prefer matches that start early in the candidate.
    Some(score - haystack.len() as i64 / 4)
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

pub fn excluded_result(app: &AppEntry) -> SearchResult {
    SearchResult {
        entry: app.clone(),
        score: 950,
        match_type: MatchType::Provider,
        action: ResultAction::Excluded {
            exec: app.exec.clone(),
        },
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

    fn app(name: &str, tags: &[&str]) -> AppEntry {
        let mut entry = AppEntry::virtual_entry(name, "", name);
        entry.tags = tags.iter().map(|t| t.to_string()).collect();
        entry
    }

    #[test]
    fn test_calculate() {
        assert_eq!(try_calculate("2+2"), Some("= 4".to_string()));
        assert_eq!(try_calculate("10/3"), Some("= 3.333333".to_string()));
        assert_eq!(try_calculate("2^10"), Some("= 1024".to_string()));
        assert_eq!(try_calculate("(5+3)*2"), Some("= 16".to_string()));
        assert_eq!(try_calculate("hello"), None);
    }

    #[test]
    fn exact_match_wins() {
        let apps = vec![
            app("Files", &[]),
            app("Filesystem", &[]),
            app("Terminal", &[]),
        ];
        let engine = SearchEngine::new();
        let config = SearchConfig {
            max_results: 10,
            ..Default::default()
        };
        let results = engine.search("files", &apps, &HashMap::new(), &config);
        assert_eq!(results[0].entry.name, "Files");
        assert_eq!(results[0].match_type, MatchType::Exact);

        // A longer query that only covers the start of the name is a prefix hit.
        let partial = engine.search("fil", &apps, &HashMap::new(), &config);
        assert_eq!(partial[0].entry.name, "Files");
        assert_eq!(partial[0].match_type, MatchType::Prefix);
    }

    #[test]
    fn tags_filter_results() {
        let apps = vec![app("Firefox", &["web"]), app("Gimp", &["image"])];
        let engine = SearchEngine::new();
        let config = SearchConfig {
            max_results: 10,
            active_tags: vec!["web".to_string()],
            ..Default::default()
        };
        let results = engine.search("i", &apps, &HashMap::new(), &config);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.name, "Firefox");
    }

    #[test]
    fn default_list_puts_favorites_first() {
        let mut apps = vec![app("Alpha", &[]), app("Beta", &[])];
        apps[1].is_favorite = true;
        apps[0].launch_count = 7;
        let results = default_results(&apps, &HashMap::new(), HistoryMode::UsageCount, 10);
        assert_eq!(results[0].entry.name, "Beta");
        assert_eq!(results[1].entry.name, "Alpha");
    }

    #[test]
    fn legacy_matcher_requires_all_characters() {
        assert!(legacy_fuzzy_score("firefox", "frx").is_some());
        assert!(legacy_fuzzy_score("firefox", "fxr").is_none());
    }
}
