//! KISS v2 Architecture - 1:1 Parity Module for KISS Launcher in Rust
//!
//! Recreating KISS components cleanly according to KISS Java architecture:
//! - Normalizer: NFKD normalization, phone normalization, diacritics removal
//! - DataHandler: registry of providers (Apps, Contacts, Shortcuts, Settings)
//! - SearchHandler: QuerySearcher, HistorySearcher, ApplicationSearcher
//! - Scorer: exact replica of KISS FuzzyScoreV2 algorithms
//! - Platform: clean abstraction trait separating Android/Linux host from launcher logic.

pub mod data_handler;
pub mod normalizer;
pub mod platform;
pub mod scorer;

#[cfg(test)]
mod tests;
