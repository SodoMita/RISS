//! Port of `fr.neamar.kiss.utils.fuzzy.FuzzyScoreV2` (+ `MatchInfo`).
//!
//! Upstream: Neamar/KISS @ `17fc7ebd473bbcbfc0e93592859278443cdb4ec0`,
//! `app/src/main/java/fr/neamar/kiss/utils/fuzzy/FuzzyScoreV2.java`.
//!
//! A Sublime-Text style matcher. The upstream class doc carries its own
//! acceptance vectors, which are reproduced as tests below:
//!
//! ```text
//! match("otw", "Power of the Wild") -> score 14
//! match("otw", "Druid of the Claw") -> score -3
//! match("otw", "Frostwolf Grunt")   -> score -13
//! ```
//!
//! `FuzzyFactory.createFuzzyScore` picks V2 unless the `use-fuzzy-score-v1`
//! preference is set (default `false`), so V2 is the production matcher. V1 is
//! not ported: it is not on the default path.
//!
//! ## Allocation
//!
//! Java allocates an `ArrayList` per recursion level per candidate. That is
//! the single hottest thing in the search, so the port hoists every list into
//! a [`FuzzyScratch`] owned by the search worker and reused across candidates
//! *and* across searches. `match_code_points` performs zero heap allocations
//! once the scratch has grown, which the test at the bottom of this file
//! asserts with the allocation counter rather than by inspection.

use super::normalizer::{java_is_whitespace, java_to_lowercase, java_to_uppercase};

/// `MatchInfo`, minus the optional `matchedIndices` list (RISS does not
/// highlight matched characters today; the indices are still produced in
/// [`FuzzyScratch::matched_indices`] for when it does).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MatchInfo {
    /// Higher is better. Only comparable between candidates scored with the
    /// same pattern.
    pub score: i32,
    pub matched: bool,
}

impl MatchInfo {
    pub const UNMATCHED: MatchInfo = MatchInfo {
        score: 0,
        matched: false,
    };

    const fn new(matched: bool, score: i32) -> Self {
        Self { score, matched }
    }
}

/// Upstream's recursion ceiling. The source says `int recursionLimit = 7; //
/// originally 10` -- commit eb0c3e59e, "improve fuzzy search", lowered it,
/// which changes scores for long candidates, so it is part of the behaviour.
const RECURSION_LIMIT: u32 = 7;

/// The `ArrayList`s upstream allocates per recursion level, hoisted out of the
/// candidate loop. Indexed by recursion depth, which is bounded by
/// [`RECURSION_LIMIT`].
pub struct FuzzyScratch {
    /// Top-level `matches` list from `match(int[])`.
    matches: Vec<u32>,
    /// Per-depth `bestRecursiveMatches`.
    best: Vec<Vec<u32>>,
    /// Per-depth `recursiveMatches`.
    recursive: Vec<Vec<u32>>,
    /// The winning match positions of the last `match_code_points` call.
    pub matched_indices: Vec<u32>,
}

impl Default for FuzzyScratch {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzyScratch {
    pub fn new() -> Self {
        let depth = RECURSION_LIMIT as usize + 1;
        Self {
            matches: Vec::new(),
            best: (0..depth).map(|_| Vec::new()).collect(),
            recursive: (0..depth).map(|_| Vec::new()).collect(),
            matched_indices: Vec::new(),
        }
    }

    /// Give every buffer room for a pattern/candidate of this length so the
    /// candidate loop never grows one.
    pub fn reserve(&mut self, pattern_len: usize) {
        self.matches.reserve(pattern_len);
        self.matched_indices.reserve(pattern_len);
        for v in self.best.iter_mut().chain(self.recursive.iter_mut()) {
            v.reserve(pattern_len);
        }
    }
}

/// `FuzzyScoreV2`. Built once per search from the normalized query, then
/// matched against every candidate.
#[derive(Debug, Clone)]
pub struct FuzzyScoreV2 {
    pattern_length: usize,
    /// `patternLower`: the pattern lower-cased once at construction.
    pattern_lower: Vec<char>,
    adjacency_bonus: i32,
    separator_bonus: i32,
    camel_bonus: i32,
    first_letter_bonus: i32,
    leading_letter_penalty: i32,
    max_leading_letter_penalty: i32,
    unmatched_letter_penalty: i32,
    /// `int recursionLimit = 7; // originally 10`.
    ///
    /// A local in the Java method, lifted to a field here *only* so the test
    /// that establishes where the class doc-comment's fixtures came from can
    /// restore the original 10. Production never changes it.
    recursion_limit: u32,
}

impl FuzzyScoreV2 {
    /// `new FuzzyScoreV2(int[] pattern, boolean detailedMatchIndices)` with the
    /// upstream constants. Commit 91ec95a57, "improve ordering", is where
    /// `max_leading_letter_penalty` reached -45.
    pub fn new(pattern: &[char]) -> Self {
        Self {
            pattern_length: pattern.len(),
            pattern_lower: pattern.iter().copied().map(java_to_lowercase).collect(),
            adjacency_bonus: 15,
            separator_bonus: 30,
            camel_bonus: 30,
            first_letter_bonus: 30,
            leading_letter_penalty: -5,
            max_leading_letter_penalty: -45,
            unmatched_letter_penalty: -2,
            recursion_limit: RECURSION_LIMIT,
        }
    }

    /// Test-only: restore the pre-eb0c3e59e recursion limit.
    #[cfg(test)]
    fn set_recursion_limit(mut self, v: u32) -> Self {
        self.recursion_limit = v;
        self
    }

    /// Rebuild the pattern in place, so a new keystroke does not allocate a new
    /// matcher. Equivalent to constructing a fresh `FuzzyScoreV2`.
    pub fn set_pattern(&mut self, pattern: &[char]) {
        self.pattern_length = pattern.len();
        self.pattern_lower.clear();
        self.pattern_lower
            .extend(pattern.iter().copied().map(java_to_lowercase));
    }

    pub fn pattern_len(&self) -> usize {
        self.pattern_length
    }

    // The `setXxx` builders exist because upstream's own test overrides every
    // constant to a power of ten so a score can be read as a bonus histogram.
    // The ported test does the same, which is the only reason they are here.
    pub fn set_adjacency_bonus(mut self, v: i32) -> Self {
        self.adjacency_bonus = v;
        self
    }
    pub fn set_separator_bonus(mut self, v: i32) -> Self {
        self.separator_bonus = v;
        self
    }
    pub fn set_camel_bonus(mut self, v: i32) -> Self {
        self.camel_bonus = v;
        self
    }
    pub fn set_first_letter_bonus(mut self, v: i32) -> Self {
        self.first_letter_bonus = v;
        self
    }
    pub fn set_leading_letter_penalty(mut self, v: i32) -> Self {
        self.leading_letter_penalty = v;
        self
    }
    pub fn set_max_leading_letter_penalty(mut self, v: i32) -> Self {
        self.max_leading_letter_penalty = v;
        self
    }
    pub fn set_unmatched_letter_penalty(mut self, v: i32) -> Self {
        self.unmatched_letter_penalty = v;
        self
    }

    /// `MatchInfo match(int[] str)`.
    ///
    /// Upstream re-uses one `MatchInfo` instance per scorer and overwrites
    /// `score`/`match` on every call (see `testReusedMatchInfoScore`); the port
    /// returns the value instead, which has the same observable behaviour
    /// because `MatchInfo` is `Copy` here.
    pub fn match_code_points(&self, str: &[char], scratch: &mut FuzzyScratch) -> MatchInfo {
        let max_matches = self.pattern_length.min(str.len());
        let mut matches = std::mem::take(&mut scratch.matches);
        matches.clear();

        let info =
            self.match_recursive(str, 0, 0, None, &mut matches, max_matches, 0, 0, scratch, 0);

        scratch.matched_indices.clear();
        scratch.matched_indices.extend_from_slice(&matches);
        scratch.matches = matches;
        info
    }

    /// `matchRecursive(...)`, one argument at a time.
    ///
    /// `srcMatches` is only ever read (`matches.addAll(srcMatches)`), so it is
    /// a slice; `matches` is the caller-owned out-list. The two temporaries the
    /// Java body allocates (`bestRecursiveMatches`, `recursiveMatches`) come
    /// out of the depth-indexed scratch pool.
    #[allow(clippy::too_many_arguments)]
    fn match_recursive(
        &self,
        str: &[char],
        mut pattern_cur_index: usize,
        mut str_curr_index: usize,
        src_matches: Option<&[u32]>,
        matches: &mut Vec<u32>,
        max_matches: usize,
        mut next_match: usize,
        recursion_count: u32,
        scratch: &mut FuzzyScratch,
        depth: usize,
    ) -> MatchInfo {
        // "Return if recursion limit is reached." Java pre-increments the local
        // parameter, so the value passed down is the incremented one.
        let recursion_count = recursion_count + 1;
        if recursion_count >= self.recursion_limit {
            return MatchInfo::UNMATCHED;
        }

        // "Return if we reached ends of strings."
        if pattern_cur_index == self.pattern_length || str_curr_index == str.len() {
            return MatchInfo::UNMATCHED;
        }

        let mut recursive_match = false;
        let mut best_recursive_matches = std::mem::take(&mut scratch.best[depth]);
        best_recursive_matches.clear();
        let mut recursive_matches = std::mem::take(&mut scratch.recursive[depth]);
        let mut best_recursive_score = 0i32;

        let mut first_match = true;
        while pattern_cur_index < self.pattern_length && str_curr_index < str.len() {
            if self.pattern_lower[pattern_cur_index] == java_to_lowercase(str[str_curr_index]) {
                if next_match >= max_matches {
                    scratch.best[depth] = best_recursive_matches;
                    scratch.recursive[depth] = recursive_matches;
                    return MatchInfo::UNMATCHED;
                }

                if first_match {
                    if let Some(src) = src_matches {
                        matches.clear();
                        matches.extend_from_slice(src);
                    }
                    first_match = false;
                }

                recursive_matches.clear();
                let recursive_result = self.match_recursive(
                    str,
                    pattern_cur_index,
                    str_curr_index + 1,
                    Some(matches.as_slice()),
                    &mut recursive_matches,
                    max_matches,
                    next_match,
                    recursion_count,
                    scratch,
                    depth + 1,
                );

                if recursive_result.matched {
                    // "Pick best recursive score."
                    if !recursive_match || recursive_result.score > best_recursive_score {
                        best_recursive_matches.clear();
                        best_recursive_matches.extend_from_slice(&recursive_matches);
                        best_recursive_score = recursive_result.score;
                    }
                    recursive_match = true;
                }

                matches.push(str_curr_index as u32);
                next_match += 1;
                pattern_cur_index += 1;
            }
            str_curr_index += 1;
        }

        let mut out_score = 0i32;
        let matched = pattern_cur_index == self.pattern_length;

        if matched {
            out_score = 100;

            // Leading letter penalty, clamped.
            let penalty = self
                .max_leading_letter_penalty
                .max(self.leading_letter_penalty * matches[0] as i32);
            out_score += penalty;

            // Unmatched letter penalty.
            let unmatched = str.len() as i32 - next_match as i32;
            out_score += self.unmatched_letter_penalty * unmatched;

            // Ordering bonuses.
            for i in 0..matches.len() {
                let curr_idx = matches[i] as usize;

                if i > 0 {
                    let prev_idx = matches[i - 1] as usize;
                    if curr_idx == prev_idx + 1 {
                        out_score += self.adjacency_bonus;
                    }
                }

                if curr_idx > 0 {
                    // Camel case: previous is not upper, current is not lower.
                    // This is exactly why the normalizer must not pre-fold
                    // case: with a lower-cased corpus the test can never fire.
                    let neighbor = str[curr_idx - 1];
                    let curr = str[curr_idx];
                    if neighbor != java_to_uppercase(neighbor) && curr != java_to_lowercase(curr) {
                        out_score += self.camel_bonus;
                    }
                    if java_is_whitespace(neighbor) {
                        out_score += self.separator_bonus;
                    }
                } else {
                    out_score += self.first_letter_bonus;
                }
            }
        }

        let result = if recursive_match && (!matched || best_recursive_score > out_score) {
            // "Recursive score is better than this".
            matches.clear();
            matches.extend_from_slice(&best_recursive_matches);
            MatchInfo::new(true, best_recursive_score)
        } else if matched {
            MatchInfo::new(true, out_score)
        } else {
            MatchInfo::UNMATCHED
        };

        scratch.best[depth] = best_recursive_matches;
        scratch.recursive[depth] = recursive_matches;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kiss::normalizer::normalize_with_result;
    use crate::tests::alloc_count::measure;

    const FIRST_LETTER_BONUS: i32 = 1_000_000;
    const ADJACENCY_BONUS: i32 = 100_000;
    const SEPARATOR_BONUS: i32 = 10_000;
    const CAMEL_BONUS: i32 = 1_000;
    const LEADING_LETTER_PENALTY: i32 = -100;
    const MAX_LEADING_LETTER_PENALTY: i32 = -300;
    const UNMATCHED_LETTER_PENALTY: i32 = -1;

    /// Upstream's own scorer configuration: every constant is a distinct power
    /// of ten so the resulting score reads as a histogram of which bonuses
    /// fired. Copied from `FuzzyScoreV2Test.createFuzzyScore`.
    fn histogram_scorer(pattern: &[char]) -> FuzzyScoreV2 {
        FuzzyScoreV2::new(pattern)
            .set_adjacency_bonus(ADJACENCY_BONUS)
            .set_separator_bonus(SEPARATOR_BONUS)
            .set_camel_bonus(CAMEL_BONUS)
            .set_first_letter_bonus(FIRST_LETTER_BONUS)
            .set_leading_letter_penalty(LEADING_LETTER_PENALTY)
            .set_max_leading_letter_penalty(MAX_LEADING_LETTER_PENALTY)
            .set_unmatched_letter_penalty(UNMATCHED_LETTER_PENALTY)
    }

    fn do_fuzzy(query: &str, text: &str) -> MatchInfo {
        let q = normalize_with_result(query, false);
        let t = normalize_with_result(text, false);
        let scorer = histogram_scorer(&q.code_points);
        let mut scratch = FuzzyScratch::new();
        scorer.match_code_points(&t.code_points, &mut scratch)
    }

    /// Ported from upstream `FuzzyScoreV2Test.testOperations` /
    /// `testProvider()`, value for value.
    #[test]
    fn upstream_fuzzy_score_v2_test_vectors() {
        #[allow(clippy::type_complexity)]
        let cases: &[(&str, &str, i32, bool)] = &[
            ("no match", "some string", 0, false),
            (
                "yt",
                "YouTube",
                100 + CAMEL_BONUS + FIRST_LETTER_BONUS + 5 * UNMATCHED_LETTER_PENALTY,
                true,
            ),
            ("js", "js", 100 + ADJACENCY_BONUS + FIRST_LETTER_BONUS, true),
            (
                "js",
                "js end",
                100 + ADJACENCY_BONUS + FIRST_LETTER_BONUS + 4 * UNMATCHED_LETTER_PENALTY,
                true,
            ),
            (
                "js",
                "start js",
                100 + ADJACENCY_BONUS
                    + SEPARATOR_BONUS
                    + 6 * UNMATCHED_LETTER_PENALTY
                    + MAX_LEADING_LETTER_PENALTY,
                true,
            ),
            (
                "js",
                "John Smith",
                100 + SEPARATOR_BONUS + FIRST_LETTER_BONUS + 8 * UNMATCHED_LETTER_PENALTY,
                true,
            ),
            (
                "jsmith",
                "John Smith",
                100 + 4 * ADJACENCY_BONUS
                    + SEPARATOR_BONUS
                    + FIRST_LETTER_BONUS
                    + 4 * UNMATCHED_LETTER_PENALTY,
                true,
            ),
            (
                "second",
                "first second third word",
                100 + 5 * ADJACENCY_BONUS
                    + SEPARATOR_BONUS
                    + 17 * UNMATCHED_LETTER_PENALTY
                    + 3 * LEADING_LETTER_PENALTY,
                true,
            ),
            (
                "econd",
                "first second third word",
                100 + 4 * ADJACENCY_BONUS
                    + 18 * UNMATCHED_LETTER_PENALTY
                    + MAX_LEADING_LETTER_PENALTY,
                true,
            ),
            (
                "third",
                "first second third word",
                100 + 4 * ADJACENCY_BONUS
                    + SEPARATOR_BONUS
                    + 18 * UNMATCHED_LETTER_PENALTY
                    + MAX_LEADING_LETTER_PENALTY,
                true,
            ),
            (
                "word",
                "first second third word",
                100 + 3 * ADJACENCY_BONUS
                    + SEPARATOR_BONUS
                    + 19 * UNMATCHED_LETTER_PENALTY
                    + MAX_LEADING_LETTER_PENALTY,
                true,
            ),
            ("first second third word", "firss", 0, false),
        ];

        for &(query, text, score, matched) in cases {
            let info = do_fuzzy(query, text);
            assert_eq!(info.matched, matched, "match({query:?}, {text:?})");
            assert_eq!(info.score, score, "score({query:?}, {text:?})");
        }
    }

    /// Ported from upstream `FuzzyScoreV2Test.testReusedMatchInfoScore`: a
    /// scorer reused across candidates must not leak the previous candidate's
    /// score into a non-match.
    #[test]
    fn upstream_reused_match_info_score() {
        let q = normalize_with_result("Bob", false);
        let t1 = normalize_with_result("Bob", false);
        let t2 = normalize_with_result("Alice", false);
        let expected = 100 + 2 * ADJACENCY_BONUS + FIRST_LETTER_BONUS;

        let scorer = histogram_scorer(&q.code_points);
        let mut scratch = FuzzyScratch::new();

        let m = scorer.match_code_points(&t1.code_points, &mut scratch);
        assert!(m.matched);
        assert_eq!(m.score, expected);
        let m = scorer.match_code_points(&t2.code_points, &mut scratch);
        assert!(!m.matched);
        assert_eq!(m.score, 0);

        // ... and again through the same scorer, which is how the providers
        // use it.
        let m = scorer.match_code_points(&t1.code_points, &mut scratch);
        assert!(m.matched);
        assert_eq!(m.score, expected);
        let m = scorer.match_code_points(&t2.code_points, &mut scratch);
        assert!(!m.matched);
        assert_eq!(m.score, 0);
    }

    /// **The class doc-comment of `FuzzyScoreV2.java` is not reproducible
    /// from the pinned source, and this test is the receipt.**
    ///
    /// Upstream's header claims:
    ///
    /// ```text
    /// match("otw", "Power of the Wild") = 14
    /// match("otw", "Druid of the Claw") = -3
    /// match("otw", "Frostwolf Grunt")   = -13
    /// ```
    ///
    /// Those three numbers have been circulating in this project as an
    /// acceptance fixture. They are not one. Measured against the pinned
    /// tree:
    ///
    /// * With KISS's own constants the scores are 132 / 102 / 81.
    /// * With lib_fts' documented constants (adjacency 5, separator 10,
    ///   camel 10, first-letter 15, leading -3 / max -9, unmatched -1) and
    ///   the `outScore = 100` base subtracted, **two of the three reproduce
    ///   exactly**: -3 and -13.
    /// * The first one does not: it is 7, not 14, and restoring the
    ///   pre-eb0c3e59e `recursionLimit = 10` does not recover it either, so
    ///   the gap is not the recursion ceiling.
    ///
    /// Conclusion recorded rather than papered over: the comment is a
    /// partially stale import from lib_fts. Anything quoting "= 14" as a
    /// parity target is quoting a number this algorithm does not compute.
    /// What the doc genuinely asserts -- the ranking -- is pinned in the test
    /// below.
    #[test]
    fn the_doc_comment_fixtures_are_only_partly_reproducible() {
        fn libfts(query: &str, text: &str, limit: u32) -> i32 {
            let q = normalize_with_result(query, false);
            let t = normalize_with_result(text, false);
            let scorer = FuzzyScoreV2::new(&q.code_points)
                .set_adjacency_bonus(5)
                .set_separator_bonus(10)
                .set_camel_bonus(10)
                .set_first_letter_bonus(15)
                .set_leading_letter_penalty(-3)
                .set_max_leading_letter_penalty(-9)
                .set_unmatched_letter_penalty(-1)
                .set_recursion_limit(limit);
            let mut scratch = FuzzyScratch::new();
            scorer.match_code_points(&t.code_points, &mut scratch).score - 100
        }
        // Two of the three documented values, exactly.
        assert_eq!(libfts("otw", "Druid of the Claw", RECURSION_LIMIT), -3);
        assert_eq!(libfts("otw", "Frostwolf Grunt", RECURSION_LIMIT), -13);
        // The third does not reproduce, at either recursion limit.
        assert_eq!(libfts("otw", "Power of the Wild", RECURSION_LIMIT), 7);
        assert_eq!(
            libfts("otw", "Power of the Wild", 10),
            7,
            "eb0c3e59e's recursion-limit change is not the cause of the gap"
        );
    }

    /// The same three strings under the constants KISS actually ships.
    /// The absolute values differ from the doc comment; the *ranking* the
    /// doc is really asserting survives.
    #[test]
    fn production_constants_rank_the_doc_vectors_the_same_way() {
        fn score(query: &str, text: &str) -> MatchInfo {
            let q = normalize_with_result(query, false);
            let t = normalize_with_result(text, false);
            let scorer = FuzzyScoreV2::new(&q.code_points);
            let mut scratch = FuzzyScratch::new();
            scorer.match_code_points(&t.code_points, &mut scratch)
        }
        let wild = score("otw", "Power of the Wild");
        let claw = score("otw", "Druid of the Claw");
        let grunt = score("otw", "Frostwolf Grunt");
        assert_eq!(wild, MatchInfo::new(true, 132));
        assert_eq!(claw, MatchInfo::new(true, 102));
        assert_eq!(grunt, MatchInfo::new(true, 81));
        assert!(wild.score > claw.score && claw.score > grunt.score);
    }

    #[test]
    fn camel_case_bonus_needs_unfolded_case() {
        // With case preserved (production), "vb" scores the camel bonus on
        // VirtualBox. Pre-lowercasing the corpus silently deletes it, which is
        // the bug that makes a "cleaned up" normalizer diverge forever.
        let q = normalize_with_result("vb", false);
        let scorer = FuzzyScoreV2::new(&q.code_points);
        let mut scratch = FuzzyScratch::new();

        let kept = normalize_with_result("VirtualBox", false);
        let folded = normalize_with_result("VirtualBox", true);
        let a = scorer.match_code_points(&kept.code_points, &mut scratch);
        let b = scorer.match_code_points(&folded.code_points, &mut scratch);
        assert!(a.matched && b.matched);
        assert_eq!(a.score - b.score, 30, "the camel bonus is 30");
    }

    #[test]
    fn accents_and_dashes_match_through_normalization() {
        // "cafe" finds "Café" and "visualstudio" finds "Visual-Studio" only
        // because both sides go through the normalizer first.
        let q = normalize_with_result("cafe", false);
        let scorer = FuzzyScoreV2::new(&q.code_points);
        let mut scratch = FuzzyScratch::new();
        let t = normalize_with_result("Café", false);
        assert!(
            scorer
                .match_code_points(&t.code_points, &mut scratch)
                .matched
        );

        let q = normalize_with_result("visualstudio", false);
        let scorer = FuzzyScoreV2::new(&q.code_points);
        let t = normalize_with_result("Visual-Studio", false);
        assert!(
            scorer
                .match_code_points(&t.code_points, &mut scratch)
                .matched
        );
    }

    #[test]
    fn matches_code_points_beyond_the_bmp() {
        let q = normalize_with_result("ab", false);
        let scorer = FuzzyScoreV2::new(&q.code_points);
        let mut scratch = FuzzyScratch::new();
        // An astral code point between the two matched letters must count as
        // exactly one unmatched *code point*, not two UTF-16 units.
        let t = normalize_with_result("a\u{1F600}b", false);
        let m = scorer.match_code_points(&t.code_points, &mut scratch);
        assert!(m.matched);
        // 100 + first_letter(30) + unmatched: 3 code points - 2 matched = 1.
        assert_eq!(m.score, 100 + 30 + (-2));
    }

    /// The hot-path contract: after the scratch has grown once, scoring a
    /// candidate allocates nothing at all.
    #[test]
    fn scoring_a_candidate_allocates_nothing() {
        let q = normalize_with_result("jsm", false);
        let scorer = FuzzyScoreV2::new(&q.code_points);
        let mut scratch = FuzzyScratch::new();
        scratch.reserve(64);
        let texts: Vec<_> = [
            "John Smith",
            "Josh's Media",
            "Jellyfish Manager",
            "Nothing here",
            "jsmith",
        ]
        .iter()
        .map(|t| normalize_with_result(t, false))
        .collect();

        // Warm the per-depth buffers to their steady-state size.
        for _ in 0..4 {
            for t in &texts {
                scorer.match_code_points(&t.code_points, &mut scratch);
            }
        }

        let (_, allocs, _) = measure(|| {
            let mut sum = 0i64;
            for _ in 0..100 {
                for t in &texts {
                    sum += scorer.match_code_points(&t.code_points, &mut scratch).score as i64;
                }
            }
            sum
        });
        assert_eq!(allocs, 0, "500 candidate scorings allocated {allocs} times");
    }
}
