#[cfg(target_os = "android")]
use crate::android_app_entry::AppEntry;
#[cfg(not(target_os = "android"))]
use crate::app_entry::AppEntry;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::cmp::Ordering;

/// Case-insensitive name order without allocating a lowered copy of either
/// name for every comparison. Equivalent to comparing `to_lowercase()` of both.
pub fn compare_names(a: &str, b: &str) -> Ordering {
    a.chars()
        .flat_map(|c| c.to_lowercase())
        .cmp(b.chars().flat_map(|c| c.to_lowercase()))
}

/// Result of a search with relevance score
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub entry: AppEntry,
    pub score: i64,
    pub match_type: MatchType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchType {
    Exact,
    Prefix,
    Fuzzy,
    Tag,
    Category,
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

    /// Search through apps with the given query.
    ///
    /// Candidates are ranked by index, so the entries that fall outside
    /// `max_results` are never cloned. Matching on the name, its prefix, the
    /// tags and the categories happens before the fuzzy matcher, because
    /// building the concatenated search text allocates once per app.
    pub fn search(&self, query: &str, apps: &[AppEntry], max_results: usize) -> Vec<SearchResult> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let mut hits: Vec<(i64, MatchType, usize)> = Vec::new();
        for (index, app) in apps.iter().enumerate() {
            if let Some((score, match_type)) = self.classify(app, query, &query_lower) {
                hits.push((score, match_type, index));
            }
        }

        hits.sort_by_key(|(score, _, _)| std::cmp::Reverse(*score));
        hits.truncate(max_results);
        hits.into_iter()
            .map(|(score, match_type, index)| SearchResult {
                entry: apps[index].clone(),
                score,
                match_type,
            })
            .collect()
    }

    /// Score one app against the query: `None` when it does not match.
    fn classify(&self, app: &AppEntry, query: &str, query_lower: &str) -> Option<(i64, MatchType)> {
        let name_lower = app.name.to_lowercase();

        // Exact match on name
        if name_lower == query_lower {
            return Some((10000, MatchType::Exact));
        }

        // Prefix match on name
        if name_lower.starts_with(query_lower) {
            let score = 5000 + (100 - app.name.len() as i64);
            return Some((score, MatchType::Prefix));
        }

        // Tag match
        for tag in &app.tags {
            if tag.to_lowercase().starts_with(query_lower) {
                return Some((3000, MatchType::Tag));
            }
        }

        // Category match
        for cat in &app.categories {
            if cat.to_lowercase().starts_with(query_lower) {
                return Some((2000, MatchType::Category));
            }
        }

        // Fuzzy match on searchable text, with a boost for how often the app
        // is used.
        self.matcher
            .fuzzy_match(&app.searchable_text(), query)
            .map(|score| (score + i64::from(app.launch_count) * 10, MatchType::Fuzzy))
    }

    /// Get default apps to show when no search query (favorites + most used).
    pub fn get_default_apps(&self, apps: &[AppEntry], max_results: usize) -> Vec<SearchResult> {
        let mut hits: Vec<(i64, usize)> = apps
            .iter()
            .enumerate()
            .filter(|(_, app)| app.is_favorite || app.launch_count > 0)
            .map(|(index, app)| {
                let score = if app.is_favorite {
                    10000 + i64::from(app.launch_count)
                } else {
                    i64::from(app.launch_count)
                };
                (score, index)
            })
            .collect();

        hits.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        hits.truncate(max_results);
        hits.into_iter()
            .map(|(score, index)| SearchResult {
                entry: apps[index].clone(),
                score,
                match_type: if apps[index].is_favorite {
                    MatchType::Exact
                } else {
                    MatchType::Fuzzy
                },
            })
            .collect()
    }
}

/// Try to evaluate a mathematical expression
pub fn try_calculate(input: &str) -> Option<String> {
    let input = input.trim();

    // Only try if it looks like a math expression
    if !input.chars().any(|c| "+-*/^".contains(c)) {
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
    fn compare_names_matches_lowercase_ordering() {
        assert_eq!(compare_names("Firefox", "firefox"), Ordering::Equal);
        assert_eq!(compare_names("Alpha", "beta"), Ordering::Less);
        // 'ä' sorts after 'z' once lowered, exactly like `to_lowercase()`.
        assert_eq!(compare_names("äpp", "Zeta"), Ordering::Greater);
    }

    fn app(name: &str, comment: &str, tags: &[&str]) -> AppEntry {
        AppEntry {
            name: name.to_owned(),
            comment: comment.to_owned(),
            exec: name.to_lowercase(),
            icon: String::new(),
            categories: Vec::new(),
            tags: tags.iter().map(|tag| tag.to_lowercase()).collect(),
            desktop_file: std::path::PathBuf::new(),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
        }
    }

    fn names(hits: &[SearchResult]) -> Vec<&str> {
        hits.iter().map(|hit| hit.entry.name.as_str()).collect()
    }

    #[test]
    fn search_ranks_name_matches_first_and_limits_results() {
        let engine = SearchEngine::new();
        let apps = vec![
            app("Firestarter", "Fire tools", &[]),
            app("firefox", "Browser", &["web"]),
            app("Terminal", "Shell", &["console"]),
        ];
        // Both "firefox" and "Firestarter" prefix-match; the shorter name wins,
        // and `max_results` must not leak the third hit.
        let hits = engine.search("fire", &apps, 1);
        assert_eq!(names(&hits), vec!["firefox"]);
        assert_eq!(hits[0].match_type, MatchType::Prefix);
    }

    #[test]
    fn search_matches_tags() {
        let engine = SearchEngine::new();
        let apps = vec![
            app("Firestarter", "Fire tools", &[]),
            app("firefox", "Browser", &["web"]),
        ];
        let hits = engine.search("web", &apps, 10);
        assert_eq!(names(&hits), vec!["firefox"]);
        assert_eq!(hits[0].match_type, MatchType::Tag);
    }

    #[test]
    fn default_list_orders_favorites_ahead_of_usage() {
        let engine = SearchEngine::new();
        let mut used = app("Vim", "Editor", &[]);
        used.launch_count = 5;
        let mut favorite = app("Gimp", "Images", &[]);
        favorite.is_favorite = true;
        let never_used = app("Files", "Manager", &[]);
        let hits = engine.get_default_apps(&[used, favorite, never_used], usize::MAX);
        assert_eq!(names(&hits), vec!["Gimp", "Vim"]);
    }
}
