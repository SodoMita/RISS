#[cfg(not(target_os = "android"))]
use crate::app_entry::AppEntry;
#[cfg(target_os = "android")]
use crate::android_app_entry::AppEntry;
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

    /// Search through apps with the given query
    pub fn search(&self, query: &str, apps: &[AppEntry], max_results: usize) -> Vec<SearchResult> {
        let query = query.trim();
        
        if query.is_empty() {
            return Vec::new();
        }

        // Check if it's a calculation
        if let Some(_result) = try_calculate(query) {
            // We'll handle this specially in the UI
            // For now, return empty and handle in UI
        }

        let query_lower = query.to_lowercase();
        let mut results: Vec<SearchResult> = apps
            .iter()
            .filter_map(|app| {
                let name_lower = app.name.to_lowercase();
                let searchable = app.searchable_text();

                // Exact match on name
                if name_lower == query_lower {
                    return Some(SearchResult {
                        entry: app.clone(),
                        score: 10000,
                        match_type: MatchType::Exact,
                    });
                }

                // Prefix match on name
                if name_lower.starts_with(&query_lower) {
                    let score = 5000 + (100 - app.name.len() as i64);
                    return Some(SearchResult {
                        entry: app.clone(),
                        score,
                        match_type: MatchType::Prefix,
                    });
                }

                // Tag match
                for tag in &app.tags {
                    if tag.to_lowercase().starts_with(&query_lower) {
                        return Some(SearchResult {
                            entry: app.clone(),
                            score: 3000,
                            match_type: MatchType::Tag,
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
                    });
                }

                None
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.score.cmp(&a.score));
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

        results.sort_by(|a, b| b.score.cmp(&a.score));
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
        Some(format!("= {:.6}", result).trim_end_matches('0').trim_end_matches('.').to_string())
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
            if c.is_digit(10) || c == '.' {
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
}
