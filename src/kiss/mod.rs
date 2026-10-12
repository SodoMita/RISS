//! The live application-search path, ported from Neamar/KISS.
//!
//! # Pinned upstream
//!
//! ```text
//! repository: https://github.com/Neamar/KISS
//! commit:     17fc7ebd473bbcbfc0e93592859278443cdb4ec0
//! date:       2026-10-06
//! ```
//!
//! Everything in this module was written with that tree open. Where a
//! behaviour is not obvious from the final source, the commit that introduced
//! it is cited at the point of use.
//!
//! # Java -> Rust map
//!
//! | upstream file | here |
//! |---|---|
//! | `normalizer/StringNormalizer.java` | [`normalizer`] |
//! | `normalizer/IntSequenceBuilder.java` | folded into [`normalizer::Normalized`] |
//! | `utils/fuzzy/FuzzyScoreV2.java`, `MatchInfo.java` | [`fuzzy`] |
//! | `pojo/NameComparator.java`, `ReversedNameComparator.java`, `RelevanceComparator.java` | [`comparator`] |
//! | `pojo/Pojo.java`, `AppPojo.java`, `PojoWithTags.java` (search-relevant state) | [`index`] |
//! | `dataprovider/AppProvider.requestResults`, `searcher/QuerySearcher.java`, `searcher/Searcher.onPostExecute` | [`searcher`] |
//! | `searcher/Searcher.SEARCH_THREAD`, `AsyncTask` cancellation | [`worker`] |
//! | `db/HistoryMode.java`, `QuerySearcher.getMaxResultCount`, `res/xml/preferences.xml` | [`prefs`] |
//!
//! # Not ported, and why
//!
//! * `FuzzyScoreV1` -- `FuzzyFactory` only selects it when the
//!   `use-fuzzy-score-v1` preference is on (default `false`). Not the
//!   production path.
//! * `ContactsProvider`, `ShortcutsProvider`, `SearchProvider`, ... -- this is
//!   the *application* search path. RISS's own provider rows are untouched.
//! * `MatchInfo.matchedIndices` is computed but unused: RISS does not
//!   highlight matched characters yet. The position map exists for when it
//!   does.

pub mod comparator;
pub mod fuzzy;
pub mod index;
pub mod normalizer;
pub mod prefs;
pub mod searcher;
pub mod worker;
