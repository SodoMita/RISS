#[cfg(target_os = "android")]
use crate::android_app_entry::AppEntry;
#[cfg(not(target_os = "android"))]
use crate::app_entry::AppEntry;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

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

    /// Search through apps with the given query using all searchable fields.
    pub fn search(&self, query: &str, apps: &[AppEntry], max_results: usize) -> Vec<SearchResult> {
        self.search_with_options(query, apps, max_results, true, true, true)
    }

    /// Search through apps while allowing the user to enable/disable metadata providers.
    pub fn search_with_options(
        &self,
        query: &str,
        apps: &[AppEntry],
        max_results: usize,
        include_descriptions: bool,
        include_tags: bool,
        include_categories: bool,
    ) -> Vec<SearchResult> {
        let query = query.trim();

        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let mut results: Vec<SearchResult> = apps
            .iter()
            .filter_map(|app| {
                let name_lower = app.name.to_lowercase();
                let mut searchable_fields = vec![app.name.as_str()];
                if include_descriptions && !app.comment.is_empty() {
                    searchable_fields.push(app.comment.as_str());
                }
                if include_tags {
                    searchable_fields.extend(app.tags.iter().map(String::as_str));
                }
                if include_categories {
                    searchable_fields.extend(app.categories.iter().map(String::as_str));
                }
                let searchable = searchable_fields.join(" ");

                // Exact and prefix app-name matches are always enabled.
                if name_lower == query_lower {
                    return Some(SearchResult {
                        entry: app.clone(),
                        score: 10000,
                        match_type: MatchType::Exact,
                    });
                }

                if name_lower.starts_with(&query_lower) {
                    let score = 5000 + (100 - app.name.len() as i64);
                    return Some(SearchResult {
                        entry: app.clone(),
                        score,
                        match_type: MatchType::Prefix,
                    });
                }

                if include_tags {
                    for tag in &app.tags {
                        if tag.to_lowercase().starts_with(&query_lower) {
                            return Some(SearchResult {
                                entry: app.clone(),
                                score: 3000,
                                match_type: MatchType::Tag,
                            });
                        }
                    }
                }

                if include_categories {
                    for category in &app.categories {
                        if category.to_lowercase().starts_with(&query_lower) {
                            return Some(SearchResult {
                                entry: app.clone(),
                                score: 2000,
                                match_type: MatchType::Category,
                            });
                        }
                    }
                }

                if let Some(score) = self.matcher.fuzzy_match(&searchable, query) {
                    let boosted = score + (app.launch_count as i64 * 10);
                    return Some(SearchResult {
                        entry: app.clone(),
                        score: boosted,
                        match_type: MatchType::Fuzzy,
                    });
                }

                None
            })
            .collect();

        results.sort_by_key(|result| std::cmp::Reverse(result.score));
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
                }
            })
            .collect();

        results.sort_by_key(|a| std::cmp::Reverse(a.score));
        results.truncate(max_results);
        results
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
    use std::path::PathBuf;

    fn sample_app() -> AppEntry {
        AppEntry {
            name: "Editor".to_owned(),
            comment: "Practical text tool".to_owned(),
            exec: "editor".to_owned(),
            icon: String::new(),
            categories: vec!["Utility".to_owned()],
            tags: vec!["writing".to_owned()],
            desktop_file: PathBuf::new(),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
        }
    }

    #[test]
    fn metadata_search_respects_provider_settings() {
        let engine = SearchEngine::new();
        let apps = vec![sample_app()];

        assert_eq!(
            engine
                .search_with_options("practical", &apps, 10, true, true, true)
                .len(),
            1
        );
        assert!(engine
            .search_with_options("practical", &apps, 10, false, true, true)
            .is_empty());
        assert!(engine
            .search_with_options("writing", &apps, 10, true, false, true)
            .is_empty());
        assert!(engine
            .search_with_options("utility", &apps, 10, true, true, false)
            .is_empty());
    }

    #[test]
    fn test_calculate() {
        assert_eq!(try_calculate("2+2"), Some("= 4".to_string()));
        assert_eq!(try_calculate("10/3"), Some("= 3.333333".to_string()));
        assert_eq!(try_calculate("2^10"), Some("= 1024".to_string()));
        assert_eq!(try_calculate("(5+3)*2"), Some("= 16".to_string()));
        assert_eq!(try_calculate("hello"), None);
    }
}
