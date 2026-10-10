//! DataHandler - Corresponds to KISS `fr.neamar.kiss.DataHandler`
//!
//! Orchestrates providers, caches items, dispatches queries, and handles history frecency.

use super::normalizer::StringNormalizer;
use super::platform::{InstalledApp, Platform};
use super::scorer::{FuzzyScorer, MatchTier, ScoreResult};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SearchItem {
    pub app: InstalledApp,
    pub normalized_name: String,
    pub usage_count: u32,
    pub last_used_timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub item: SearchItem,
    pub score: ScoreResult,
}

pub struct DataHandler {
    platform: Arc<dyn Platform>,
    items: Vec<SearchItem>,
}

impl DataHandler {
    pub fn new(platform: Arc<dyn Platform>) -> Self {
        let mut handler = Self {
            platform,
            items: Vec::new(),
        };
        handler.reload();
        handler
    }

    pub fn reload(&mut self) {
        let apps = self.platform.list_applications();
        self.items = apps
            .into_iter()
            .map(|app| {
                let normalized_name = StringNormalizer::normalize(&app.name, true).text;
                SearchItem {
                    app,
                    normalized_name,
                    usage_count: 0,
                    last_used_timestamp: 0,
                }
            })
            .collect();
    }

    pub fn record_launch(&mut self, app_id: &str, timestamp: u64) {
        if let Some(item) = self.items.iter_mut().find(|i| i.app.id == app_id) {
            item.usage_count += 1;
            item.last_used_timestamp = timestamp;
        }
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let query_trimmed = query.trim();
        let query_normalized = StringNormalizer::normalize(query_trimmed, true).text;

        let mut hits: Vec<SearchHit> = self
            .items
            .iter()
            .filter_map(|item| {
                if query_normalized.is_empty() {
                    // Empty query: order by usage / frecency
                    let score = ScoreResult {
                        tier: MatchTier::None,
                        score: item.usage_count as i32 * 100,
                        match_indices: vec![],
                    };
                    Some(SearchHit {
                        item: item.clone(),
                        score,
                    })
                } else if let Some(mut score) =
                    FuzzyScorer::score(&query_normalized, &item.normalized_name)
                {
                    // Apply usage boost
                    score.score += (item.usage_count as i32 * 25).min(250);
                    Some(SearchHit {
                        item: item.clone(),
                        score,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Sort descending by score
        hits.sort_by_key(|a| std::cmp::Reverse(a.score.score));
        hits.truncate(limit);
        hits
    }
}
