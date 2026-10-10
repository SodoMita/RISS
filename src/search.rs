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

/// One application's search corpus, precomputed once per app-list reload.
///
/// KISS keeps exactly this on the pojo itself: `Pojo.setName()` fills
/// `normalizedName` and `AppPojo` fills `normalizedTags` when tags change, so
/// `AppProvider.requestResults` only has to normalize the query and can then
/// match against ready-made code points. RISS used to lower-case the display
/// name, rebuild the joined "searchable text" and re-lower-case every tag of
/// every installed application on *every keystroke*.
#[derive(Debug, Clone, Default)]
pub struct IndexedApp {
    /// Position of this row inside [`SearchIndex`].
    pub position: u32,
    pub exec: String,
    /// Display name (alias honored) in lower case, for exact/prefix tiers.
    pub name_lower: String,
    /// `name_lower` split on whitespace, for the word-prefix tier.
    pub words_lower: Vec<String>,
    /// Tags in lower case, for the tag-prefix tier.
    pub tags_lower: Vec<String>,
    /// Categories in lower case, for the category-prefix tier.
    pub categories_lower: Vec<String>,
    /// Name + comment + tags + categories, original case, for fuzzy matching.
    /// The fuzzy matcher is smart-case, so the haystack keeps its case.
    pub corpus: String,
    pub launch_count: u32,
}

/// Precomputed search corpus for a list of applications.
#[derive(Debug, Clone, Default)]
pub struct SearchIndex {
    rows: Vec<IndexedApp>,
    entries: Vec<AppEntry>,
}

impl SearchIndex {
    /// Build the index for `apps`, honoring the user's renames.
    pub fn build(apps: &[AppEntry], aliases: &HashMap<String, String>) -> SearchIndex {
        let mut index = SearchIndex {
            rows: Vec::with_capacity(apps.len()),
            entries: Vec::with_capacity(apps.len()),
        };
        for (position, app) in apps.iter().enumerate() {
            let name = display_name(app, aliases);
            let name_lower = name.to_lowercase();
            let words_lower = name_lower
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let tags_lower = app
                .tags
                .iter()
                .map(|tag| tag.to_lowercase())
                .collect::<Vec<_>>();
            let categories_lower = app
                .categories
                .iter()
                .map(|category| category.to_lowercase())
                .collect::<Vec<_>>();

            let mut corpus = String::with_capacity(name.len() + app.comment.len() + 16);
            // The alias goes in front of the original name, exactly like the
            // old `searchable_text()`: a rename must not make the real name
            // unfindable.
            if aliases.get(&app.exec).is_some() {
                corpus.push_str(&name);
                corpus.push(' ');
            }
            corpus.push_str(&app.name);
            if !app.comment.is_empty() {
                corpus.push(' ');
                corpus.push_str(&app.comment);
            }
            for tag in &app.tags {
                corpus.push(' ');
                corpus.push_str(tag);
            }
            for category in &app.categories {
                corpus.push(' ');
                corpus.push_str(category);
            }

            index.rows.push(IndexedApp {
                position: position as u32,
                exec: app.exec.clone(),
                name_lower,
                words_lower,
                tags_lower,
                categories_lower,
                corpus,
                launch_count: app.launch_count,
            });
            index.entries.push(app.clone());
        }
        index
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The indexed row at `position`, if it exists.
    pub fn get(&self, position: u32) -> Option<&IndexedApp> {
        self.rows.get(position as usize)
    }

    /// The application at `position`.
    pub fn entry(&self, position: u32) -> &AppEntry {
        &self.entries[position as usize]
    }
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

    /// Search the prebuilt [`SearchIndex`] for `query`.
    ///
    /// `candidates` lists the index positions allowed to match, which is how
    /// the caller keeps excluded applications out of typed queries without
    /// keeping a second copy of the whole app list.
    ///
    /// Only the applications that actually match are cloned into a
    /// [`SearchResult`]; RISS used to clone all of them before truncating to
    /// the configured number of display elements.
    pub fn search(
        &self,
        query: &str,
        index: &SearchIndex,
        candidates: &[u32],
        max_results: usize,
    ) -> Vec<SearchResult> {
        let query = query.trim();

        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let mut results: Vec<SearchResult> = Vec::new();

        for &position in candidates {
            let Some(row) = index.get(position) else {
                continue;
            };

            // Exact match on name
            if row.name_lower == query_lower {
                results.push(self.result(index, position, 10000, MatchType::Exact));
                continue;
            }

            // Prefix match on name
            if row.name_lower.starts_with(&query_lower) {
                let score = 5000 + (100 - row.name_lower.len() as i64);
                results.push(self.result(index, position, score, MatchType::Prefix));
                continue;
            }

            // Word match inside the name ("fire" matching "Firefox Nightly")
            if row
                .words_lower
                .iter()
                .any(|word| word.starts_with(&query_lower))
            {
                results.push(self.result(index, position, 4000, MatchType::Prefix));
                continue;
            }

            // Tag match
            if row
                .tags_lower
                .iter()
                .any(|tag| tag.starts_with(&query_lower))
            {
                results.push(self.result(index, position, 3000, MatchType::Tag));
                continue;
            }

            // Category match
            if row
                .categories_lower
                .iter()
                .any(|category| category.starts_with(&query_lower))
            {
                results.push(self.result(index, position, 2000, MatchType::Category));
                continue;
            }

            // Fuzzy match over the whole corpus (name, rename, comment, tags
            // and categories). KISS matches the name and the tags of a pojo
            // separately and keeps the better relevance
            // (`Pojo.updateMatchingRelevance`); RISS folds both into one
            // haystack, so a tag carries a result even when the query only hits
            // the middle of it — the match is then reported as `Fuzzy` rather
            // than `Tag`, because the corpus does not say which part matched.
            let score = self
                .matcher
                .fuzzy_match(&row.corpus, query)
                .map(|score| score + row.launch_count as i64 * 10);
            if let Some(score) = score {
                results.push(self.result(index, position, score, MatchType::Fuzzy));
            }
        }

        // Sort by score descending, then by name so the order is stable.
        // `sort_by_cached_key` builds each key once, instead of allocating two
        // fresh `String`s per comparison like `sort_by` with `to_lowercase`
        // inside the comparator did.
        results.sort_by_cached_key(|result| {
            (
                std::cmp::Reverse(result.score),
                result.entry.name.to_lowercase(),
            )
        });
        results.truncate(max_results);
        results
    }

    fn result(
        &self,
        index: &SearchIndex,
        position: u32,
        score: i64,
        match_type: MatchType,
    ) -> SearchResult {
        SearchResult {
            entry: index.entry(position).clone(),
            score,
            match_type,
            action: ResultAction::Launch,
        }
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

    fn entry(name: &str, comment: &str, exec: &str) -> AppEntry {
        let mut app = AppEntry::virtual_entry(name, comment, exec);
        app.tags = Vec::new();
        app
    }

    fn corpus() -> Vec<AppEntry> {
        let mut browser = entry("Firefox", "Web browser", "firefox");
        browser.tags = vec!["web browser".to_string(), "internet".to_string()];
        browser.categories = vec!["Network".to_string()];
        let mut files = entry("Files", "File manager", "nautilus");
        files.categories = vec!["Utility".to_string()];
        let mut terminal = entry("Terminal", "Command line", "kgx");
        terminal.launch_count = 4;
        let editor = entry(
            "GNOME Text Editor",
            "Write things down",
            "gnome-text-editor",
        );
        vec![browser, files, terminal, editor]
    }

    fn positions(apps: &[AppEntry]) -> Vec<u32> {
        (0..apps.len() as u32).collect()
    }

    fn names(results: &[SearchResult]) -> Vec<String> {
        results
            .iter()
            .map(|result| result.entry.name.clone())
            .collect()
    }

    #[test]
    fn index_matches_the_plain_tiers() {
        let apps = corpus();
        let index = SearchIndex::build(&apps, &HashMap::new());
        let engine = SearchEngine::new();
        let candidates = positions(&apps);

        assert_eq!(
            names(&engine.search("firefox", &index, &candidates, 10)),
            ["Firefox"]
        );
        assert_eq!(
            engine.search("firefox", &index, &candidates, 10)[0].match_type,
            MatchType::Exact
        );
        // Prefix on the name outranks the fuzzy matches the corpus adds.
        assert_eq!(
            engine.search("fir", &index, &candidates, 10)[0].entry.name,
            "Firefox"
        );
        // Word prefix inside the name, not at the start of it: "text" is the
        // second word of "GNOME Text Editor".
        let by_word = engine.search("text", &index, &candidates, 10);
        assert_eq!(by_word[0].entry.name, "GNOME Text Editor");
        assert_eq!(by_word[0].match_type, MatchType::Prefix);
        // Case insensitive, as before.
        assert_eq!(
            engine.search("TERM", &index, &candidates, 10)[0].entry.name,
            "Terminal"
        );
        // Categories keep their prefix tier, ahead of anything the fuzzy
        // corpus happens to pick up ("net" is also inside "manager").
        let by_category = engine.search("net", &index, &candidates, 10);
        assert_eq!(by_category[0].entry.name, "Firefox");
        assert_eq!(by_category[0].match_type, MatchType::Category);
    }

    #[test]
    fn index_honors_renames_like_kiss_aliases() {
        let apps = corpus();
        let mut aliases = HashMap::new();
        aliases.insert("firefox".to_string(), "Netscape".to_string());
        let index = SearchIndex::build(&apps, &aliases);
        let engine = SearchEngine::new();
        let candidates = positions(&apps);

        let results = engine.search("netsc", &index, &candidates, 10);
        assert_eq!(names(&results), ["Firefox"]);
        assert_eq!(results[0].match_type, MatchType::Prefix);
        // The original name still matches too: it stays in the corpus, so a
        // rename never makes an application unfindable.
        let original = engine.search("firef", &index, &candidates, 10);
        assert_eq!(original[0].entry.name, "Firefox");
        assert_eq!(original[0].match_type, MatchType::Fuzzy);
    }

    #[test]
    fn tags_carry_a_match_without_being_a_prefix() {
        // KISS fuzzy-matches the tags of a pojo (`Pojo.updateMatchingRelevance`)
        // rather than demanding a prefix. RISS folds the tags into its corpus,
        // which reaches the same answer: "rowser" finds the "web browser" tag,
        // reported as a fuzzy match.
        let apps = corpus();
        let index = SearchIndex::build(&apps, &HashMap::new());
        let engine = SearchEngine::new();
        let candidates = positions(&apps);

        assert_eq!(
            engine.search("rowser", &index, &candidates, 10)[0]
                .entry
                .name,
            "Firefox"
        );
        // A tag prefix is still its own, higher tier.
        let tag_prefix = engine.search("inter", &index, &candidates, 10);
        assert_eq!(tag_prefix[0].entry.name, "Firefox");
        assert_eq!(tag_prefix[0].match_type, MatchType::Tag);
    }

    #[test]
    fn history_boost_only_lands_on_fuzzy_matches() {
        // Deliberate: the launch-count boost keeps its current scope. Teaching
        // every match type about history is issue #21, owned elsewhere.
        let apps = corpus();
        let index = SearchIndex::build(&apps, &HashMap::new());
        let engine = SearchEngine::new();
        let candidates = positions(&apps);

        // A query that only fuzzy-matches, so the prefix tiers cannot
        // short-circuit and hide the boost.
        let plain = engine.search("trmnl", &index, &candidates, 10);
        let mut heavier = corpus();
        heavier[2].launch_count = 40;
        let boosted_index = SearchIndex::build(&heavier, &HashMap::new());
        let boosted = engine.search("trmnl", &boosted_index, &candidates, 10);
        let plain_score = plain
            .iter()
            .find(|r| r.entry.name == "Terminal")
            .expect("Terminal should match")
            .score;
        let boosted_score = boosted
            .iter()
            .find(|r| r.entry.name == "Terminal")
            .expect("Terminal should match")
            .score;
        assert!(boosted_score > plain_score);
        assert_eq!(boosted[0].match_type, MatchType::Fuzzy);
    }

    #[test]
    fn candidates_limit_what_can_match() {
        let apps = corpus();
        let index = SearchIndex::build(&apps, &HashMap::new());
        let engine = SearchEngine::new();

        // Only the first two applications are allowed to answer.
        assert!(engine.search("terminal", &index, &[0, 1], 10).is_empty());
        assert_eq!(
            names(&engine.search("terminal", &index, &[2], 10)),
            ["Terminal"]
        );
        assert!(engine.search("", &index, &[0, 1, 2], 10).is_empty());
        // Out-of-range positions are ignored rather than panicking.
        assert!(engine.search("nope", &index, &[9], 10).is_empty());
    }

    #[test]
    fn index_is_built_once_per_reload_not_per_keystroke() {
        // The index only depends on the app list and the aliases, so two
        // queries against the same index must return the same rows.
        let apps = corpus();
        let index = SearchIndex::build(&apps, &HashMap::new());
        let engine = SearchEngine::new();
        let candidates = positions(&apps);

        let first = engine.search("fir", &index, &candidates, 10);
        let second = engine.search("fir", &index, &candidates, 10);
        assert_eq!(first.len(), second.len());
        assert_eq!(first[0].score, second[0].score);
        assert_eq!(index.len(), apps.len());
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
