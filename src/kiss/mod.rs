//! `kiss` — algorithms ported from the original KISS launcher, kept together
//! so a reviewer can diff them against the Java in one place.
//!
//! Upstream: <https://github.com/Neamar/KISS> (GPL-3.0). RISS is a port, so
//! these modules translate KISS behavior rather than reinventing it.
//!
//! Rules for this directory:
//! * every module names the exact upstream file it came from;
//! * anything not line-verified against that file says so in its own docs;
//! * gaps are written down (see `comparator`'s `userHandle` note and
//!   `query_boost`'s `DBHelper` note) instead of being guessed at.
//!
//! Nothing here touches egui, JNI, or the filesystem, so it is all testable
//! with a plain `cargo test`.

pub mod comparator;
pub mod normalizer;
pub mod query_boost;
