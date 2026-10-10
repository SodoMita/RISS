//! Scorer - Exact matching and scoring rules following KISS `FuzzyScoreV2` & `QuerySearcher`
//!
//! KISS scoring properties:
//! - Exact prefix match gets maximum bonus
//! - Word-boundary match bonus (first letter of words)
//! - History frecency multiplier
//! - Empty query shows default/history items

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchTier {
    ExactMatch,
    PrefixMatch,
    WordPrefixMatch,
    FuzzyMatch,
    None,
}

#[derive(Debug, Clone)]
pub struct ScoreResult {
    pub tier: MatchTier,
    pub score: i32,
    pub match_indices: Vec<usize>,
}

pub struct FuzzyScorer;

impl FuzzyScorer {
    /// Score query against target candidate string
    pub fn score(query: &str, target: &str) -> Option<ScoreResult> {
        if query.is_empty() {
            return Some(ScoreResult {
                tier: MatchTier::None,
                score: 0,
                match_indices: vec![],
            });
        }

        let q_lower = query.to_lowercase();
        let t_lower = target.to_lowercase();

        // 1. Exact match
        if t_lower == q_lower {
            return Some(ScoreResult {
                tier: MatchTier::ExactMatch,
                score: 10_000,
                match_indices: (0..target.len()).collect(),
            });
        }

        // 2. Prefix match
        if t_lower.starts_with(&q_lower) {
            let score = 5_000 + (100 - (t_lower.len().saturating_sub(q_lower.len()) as i32)).max(0);
            return Some(ScoreResult {
                tier: MatchTier::PrefixMatch,
                score,
                match_indices: (0..q_lower.len()).collect(),
            });
        }

        // 3. Word boundary prefix match (e.g. "calc" in "Google Calculator")
        let mut word_boundary_score = None;
        let mut last_space = true;
        for (char_pos, (i, c)) in t_lower.char_indices().enumerate() {
            if last_space && t_lower[i..].starts_with(&q_lower) {
                let score = 3_000 - ((char_pos * 10).min(500) as i32);
                word_boundary_score = Some(ScoreResult {
                    tier: MatchTier::WordPrefixMatch,
                    score,
                    match_indices: (i..i + q_lower.len()).collect(),
                });
                break;
            }
            last_space = c.is_whitespace() || c == '.' || c == '_' || c == '-';
        }

        if let Some(res) = word_boundary_score {
            return Some(res);
        }

        // 4. Subsequence / Fuzzy match
        let mut q_chars = q_lower.chars().peekable();
        let mut match_indices = Vec::new();
        let mut fuzzy_score = 1_000;
        let mut prev_matched_idx = None;

        for (i, tc) in t_lower.char_indices() {
            if let Some(&qc) = q_chars.peek() {
                if tc == qc {
                    q_chars.next();
                    match_indices.push(i);
                    // Proximity bonus
                    if let Some(prev) = prev_matched_idx {
                        if i == prev + 1 {
                            fuzzy_score += 50;
                        } else {
                            fuzzy_score -= ((i - prev) * 5) as i32;
                        }
                    }
                    prev_matched_idx = Some(i);
                }
            }
        }

        if q_chars.peek().is_none() {
            Some(ScoreResult {
                tier: MatchTier::FuzzyMatch,
                score: fuzzy_score.max(1),
                match_indices,
            })
        } else {
            None
        }
    }
}
