# Porting the live application-search path from Neamar/KISS

Branch `work/caliper-kiss-live-search`, based on `main@54f0449`.

This document is the receipt for the port. It is deliberately specific about
what was verified and what was not, because the one thing this repository has
paid for repeatedly is a claim that was never run.

---

## 1. Pinned upstream

```text
repository: https://github.com/Neamar/KISS
commit:     17fc7ebd473bbcbfc0e93592859278443cdb4ec0
date:       Tue Oct 6 15:53:36 2026 +0200
subject:    Translated using Weblate (Esperanto)
```

Resolved as a real object (`git cat-file -t` -> `commit`) before being quoted.
It was `master` at the time of the port.

## 2. KISS history consulted, and why each commit mattered

| commit | subject | why it is load-bearing |
|---|---|---|
| `91ec95a57` | improve ordering | Halved the per-query history boost: `200 - max(size-value,0)*10` became `100 - max(size-value,0)*5`, and the flat ALPHABETICALLY boost 200 -> 100. Same commit raised `max_leading_letter_penalty` to -45. A port from an older tree scores every remembered app twice as high as it should. |
| `eb0c3e59e` | improve fuzzy search | One line: the recursion limit. The source still reads `int recursionLimit = 7; // originally 10`. It changes scores for long candidates, so it is behaviour, not tuning noise. |
| `e1c3e0e0b` | add logic for disabled entries | Introduced `Pojo.isDisabled()` and the `relevance -= 200` branch. Crucially the branches are **exclusive** - a disabled entry gets the penalty and *no* history boost. |
| `e9a0c64c7` | Ensure empty number of results doesn't crash the app | Wrapped `Double.valueOf(...).intValue()` in `catch (NumberFormatException)`. This is why the preference is parsed as a `double` and then narrowed: an oversized value saturates rather than throwing. |
| `936197232` | Cache number of results to be displayed instead of parsing it all the time | The `MAX_RESULT_COUNT` static and `clearMaxResultCountCache()`. Ported as a cached `Option<usize>` invalidated on settings change. |
| `d71c69a34` | ASCII also includes z! | The normalizer's fast path is `codepoint <= 'z'`, not `< 'z'`. A one-character bug that silently routed `z` through NFKD for years. |
| `81e8c7b80` | Ensure Results are sorted alphabetically, and not by length (fix #812) | `Result.compareTo` compares code points first and falls back to length. Length-first is the bug it fixed. |
| `2caa1d27e` | strip dashes from normalized strings (fixes #261) | Why `Visual-Studio` is findable as `visualstudio`, on both sides of the comparison. |
| `c29a801d1` | predictable order of results | Moved the ordering into the `PriorityQueue` comparator and gave `ApplicationsSearcher` its own `ReversedNameComparator`, with the comment that explains the whole display-order inversion. |
| `41d7d234a` | reuse results | `onPostExecute` hands lightweight pojos to the adapter; nothing deep-clones before the trim. |
| `f6f807704`, `feeccfeac`, `4a67d71d4` | normalizer performance work | The ASCII short-circuit and the single-code-point `CharBuffer` exist because normalizing per keystroke was measurably too slow in Java too. |

## 3. RISS history consulted, and the constraints it revealed

| ref | what it revealed |
|---|---|
| `4a240f0` "Rank search candidates without cloning them" (issue #37) | The regression this port must not reintroduce: the old path deep-cloned every `AppEntry` before truncating and rebuilt two lowercased `String`s *per comparison*, measured at 89,112 allocations for one keystroke. The allocation-counting harness (`src/tests/alloc_count.rs`) survives from that work and is reused here. |
| `3174c06` "Virtualise the result list the way KISS ListView does" + `KISS.md` | Establishes that the result list is painted from a cached `Vec` and must not be rebuilt per frame. The new path keeps that contract: a frame never searches. |
| `c9dfcf9` IconCache with per-frame decode budget | Icon decoding is already off the hot path; the port must not drag it back in by cloning icons per candidate. |
| `graft/lathe` / PR #68 `SearchIndex` | Independently arrived at "normalize once at load, store it with the app". Confirms the shape; its corpus (name + comment + categories) is **not** KISS's, and that part is removed. |
| PR #79 / `graft/yardmaster-kiss-core` | Explicitly additive, "no existing call site rerouted". Its msg-69 retraction is the single most useful artefact in the project history: `normalizeWithResult(input, makeLowercase)` with production callers passing `false`, and `DEFAULT_MAX_RESULTS` being an initial capacity rather than a bound. Both verified independently here against the Java. No code taken. |
| issue #58 / PR #47 | Stale app list after an in-place refresh. The data-generation guard exists because of this class of bug. |
| issue #21 | The "ranking learns from ALL match types" behaviour. It is an RISS invention with no upstream counterpart and is **removed** by this change; see section 6. |

## 4. Java -> Rust map

| upstream | here |
|---|---|
| `normalizer/StringNormalizer.java` (`normalizeWithResult`, `Result`, `mapPosition`, `compareTo`) | `src/kiss/normalizer.rs` (`normalize_with_result`, `normalize_into`, `Normalized`, `map_position`, `compare_to`) |
| `normalizer/IntSequenceBuilder.java` | folded into `Normalized`'s two `Vec`s |
| `Character.toLowerCase/toUpperCase/isWhitespace/getType` | `java_to_lowercase`, `java_to_uppercase`, `java_is_whitespace`, `is_dropped_category` |
| `utils/fuzzy/FuzzyScoreV2.java` (`match`, `matchRecursive`) | `src/kiss/fuzzy.rs` (`FuzzyScoreV2::match_code_points`, `match_recursive`) |
| `utils/fuzzy/MatchInfo.java` | `fuzzy::MatchInfo` + `FuzzyScratch` |
| `pojo/NameComparator.java` / `ReversedNameComparator.java` / `RelevanceComparator.java` | `src/kiss/comparator.rs` (`name_compare`, `reversed_name_compare`, `relevance_compare`, `display_compare`) |
| `java.lang.String.compareTo` | `comparator::java_string_compare` (UTF-16 order) |
| `pojo/Pojo.java` + `AppPojo.java` + `PojoWithTags.java` search state | `src/kiss/index.rs` (`SearchItem`, `AppIndex::build`) |
| `dataprovider/AppProvider.requestResults` | `src/kiss/searcher.rs` `Searcher::search`, candidate loop |
| `searcher/QuerySearcher.addResults` | same function, boost/penalty block + `history_boost` |
| `searcher/Searcher.onPostExecute` (trim + drain) | same function, `sort_unstable_by(display_compare)` + `truncate` |
| `searcher/Searcher.SEARCH_THREAD`, `AsyncTask.cancel/isCancelled` | `src/kiss/worker.rs` `SearchWorker` |
| provider loading `AsyncTask`s | `src/kiss/worker.rs` `AppLoader` |
| `db/HistoryMode.java`, `QuerySearcher.getMaxResultCount`, `res/xml/preferences.xml` | `src/kiss/prefs.rs` |
| `db/DBHelper.getHistory(ctx, limit, mode, query)` + `readCursor` relevance | `HistoryData::known_ids_for_query` (`count - position`) |

## 5. The production call path, before and after

**Before**

```text
keystroke -> RissApp::update_results()            [UI thread, inside the frame]
             SearchEngine::search()
               to_lowercase() per candidate
               exact / prefix / word / tag / category ladder
               SkimMatcherV2::fuzzy_match over name+comment+tags+categories
               + usage_boost(launch_count * 10)
               sort by (score desc, lowercased display name)
               truncate, clone survivors
           -> self.results -> painted
```

**After**

```text
keystroke -> RissApp::update_results()            [UI thread, returns immediately]
             RissApp::search_params()             resolve prefs + knownIds once
             SearchWorker::submit()               -> query generation N

           [riss-search thread, serial, persistent]
             kiss::searcher::Searcher::search()
               normalize_into(query, false)       once
               FuzzyScoreV2::set_pattern()        once
               resolve knownIds / excluded favourites into position arrays
               for each app:  excluded? favourite-excluded?
                              match name, then tags, keep the larger
                              disabled -> -200  else  += history boost
               sort_unstable_by(display_compare); truncate(max_results)
             -> SearchOutcome{generation, data_generation, hits}
             -> ctx.request_repaint()

           [UI thread, next frame]
             RissApp::poll_workers()
               reject if generation != expected        (stale query)
               reject if data_generation != current    (stale app list)
               apply_search_outcome() -> self.results -> painted
```

Reloads follow the same shape: `reload_apps()` only calls `AppLoader::request()`;
`load_app_entries()` runs on the loader thread; `apply_loaded_apps()` ->
`rebuild_index()` bumps `data_generation` and re-submits the query.

## 6. Removed or disabled

* `SearchEngine::search` - **deleted**. The struct is now a zero-sized holder
  for the empty-query default list only.
* The `Exact` / `Prefix` / word-boundary / `Tag` / `Category` scoring ladder
  (10000 / 5000 / 4000 / 3000 / 2000 bands) - **deleted**. KISS has one score.
* `usage_boost(launch_count * 10)`, issue #21's "learn from all match types" -
  **deleted**. Upstream's only history influence is `QuerySearcher`'s
  per-query `knownIds` boost, which is now implemented.
* Comment and freedesktop-category matching - **deleted**. `AppProvider`
  matches the name and the tags, nothing else.
* `to_lowercase`-based folding (`starts_with_ignore_case`, `eq_ignore_case`,
  `lower_into`, `cmp_display_name`, `write_searchable_text`,
  `display_name_ref`, `searchable_text`) - **deleted**. Folding now happens
  inside `FuzzyScoreV2` on code points, which is why `cafe` finds `Café`.
* The `fuzzy-matcher` crate (`SkimMatcherV2`) - **removed from `Cargo.toml`**.
* `number-of-display-elements == 0` meaning "unlimited" - **deleted**. It was
  an invention; upstream trims to zero rows.
* The favourites picker was rerouted onto the same `kiss::searcher::Searcher`
  so there is exactly one scoring dialect in the process.

## 7. Tests

Ported from upstream:

* `StringNormalizerTest.testNormalization` - all 17 vectors.
* `FuzzyScoreV2Test.testOperations` - all 12 vectors, with upstream's
  power-of-ten constant overrides so a score reads as a bonus histogram.
* `FuzzyScoreV2Test.testReusedMatchInfoScore` - a reused scorer must not leak
  the previous candidate's score into a non-match.

Added at two levels.

`src/tests/live_app.rs` drives **the real `RissApp`** - the same constructor
the binary uses, the same `update_results`, the same `poll_workers` that
`eframe::App::update` calls first on every frame, the same generation guards,
and the same `self.results` the result list paints from. What it does not
cover is egui's painting (that is `scripts/gui-assertions.sh`, which needs a
compositor) and the platform app discovery (`discover_apps` reads the host's
`.desktop` files, which a test must not depend on). 12 tests: a typed query
paints matches, `cafe` finds `Café`, `visualstudio` finds `Visual-Studio`,
`vb` finds `VirtualBox`, a Cyrillic query matches, comments and categories are
*not* searchable, clearing the query drops the result rows, eight keystrokes
with no pumping leave the last query's answer on screen, a reload mid-typing
cannot strand a row from the replaced list, favouriting shows on the painted
row, launching teaches the next search of that query only, and the
`number-of-display-elements` preference reaches the live list through the
Java parse.

`src/tests/live_search.rs` drives the real `SearchWorker` and reads results
back through the same generation guards: accents/diacritics both directions, case insensitivity with the
lowercasing flag off, four non-Latin scripts, dashes, camel case, equal-score
ordering (both input orders), result limits including 0, limits derived from
the parsed preference, excluded apps with and without the preference, disabled
penalties, query-scoped history boost and its decay, `ALPHABETICALLY` flattening
the boost, excluded favourites, tag matching, stale-result rejection after
eight keystrokes with no waiting, cancellation of superseded searches,
concurrent reload during a search, and cache invalidation across app list /
history / settings / provider state.

Unit level: Java-vs-Rust case mapping (`U+0130`, `ß`), Java-vs-Rust whitespace
(`U+00A0`, `U+001C`), UTF-16 vs UTF-8 position mapping for an astral code
point, `String.compareTo` UTF-16 ordering vs Rust's code-point ordering,
`Double.valueOf(...).intValue()` including `Infinity` / `NaN` / saturation /
Rust-only spellings, and `HistoryMode.valueById`'s fallback.

`cargo test` **181 passed / 0 failed** (was 110 on `main`).

## 8. Measurements

Dataset: 1,200 synthetic apps across eight name shapes - Latin, Cyrillic,
Japanese, Greek, accented French with a dash, camel case, hyphenated ASCII and
a long multi-word name. Every third app carries tags, every fifth is excluded,
every seventh is disabled, and 30 apps carry history records, so the filter,
penalty, boost and tie-break branches all execute. Eight queries, ASCII and
non-ASCII, from one to five characters.

Hardware/OS: Intel Xeon @ 2.60 GHz, 4 cores, Linux 6.1.158 x86_64, container.

| mode | measurement |
|---|---|
| release (`cargo test --release`, `opt-level="s"`, thin LTO) | mean **305.9 µs/query**, worst 638.9 µs, 400 queries |
| release, re-provisioned container, same build | mean **482.9 µs/query**, worst 1.28 ms, 400 queries |
| release | **0 allocations, 0 bytes** for 8 queries x 1,200 candidates, both runs |
| debug | 0 allocations; latency not quoted (dev profile, meaningless) |

Both latency rows are reported rather than the prettier one: the same binary
measured 1.6x apart on two provisionings of the same container shape, which
is what a shared virtualised host does to wall-clock. The allocation count
was byte-identical across both runs, which is the point - it is the figure
that is actually a contract. Treat the latency as "well under a millisecond
per keystroke on a 1,200-app drawer", not as a benchmark score.

Allocation behaviour was verified with the existing counting allocator
(`src/tests/alloc_count.rs`), not by reading the code. Three real allocations
were found and removed this way, none of which were visible by inspection:

1. `sort_by` - Rust's *stable* sort allocates a merge buffer. Replaced with
   `sort_unstable_by`, which is also the faithful choice: `PriorityQueue.poll()`
   promises nothing about comparator-equal elements.
2. A `Vec<char>` for the NFKD expansion of a single code point, hit once per
   non-ASCII query. Replaced with a 32-element stack array (the longest
   compatibility decomposition in Unicode is 18).
3. The per-recursion-level `ArrayList`s of `matchRecursive`, hoisted into a
   depth-indexed `FuzzyScratch` owned by the worker.

Candidate-loop requirements verified: no `String`, no `Vec`, no
`to_lowercase`, no `chars().collect`, no `format!`, no regex, no I/O, no
metadata clone, no hash lookup, no `Rc<RefCell>`/lock. The two string-keyed
lookups upstream does per pojo (`knownIds.get`, `excludedFavoriteIds.contains`)
are resolved into position-indexed arrays once per search.

No before/after latency comparison is quoted: the old path and the new one do
not compute the same function, so a speed-up number would be meaningless. The
allocation count is the comparable figure, and issue #37's recorded baseline
for the pre-#37 code was 89,112 allocations per keystroke.

## 9. Commands run

```sh
cargo fmt                                              # clean
cargo clippy --all-targets -- -D warnings              # clean
cargo test                                             # 181 passed / 0 failed
cargo test --release --lib -- --nocapture tests::perf  # benchmark above
cargo check --target aarch64-linux-android --lib       # Android
```

## 10. Remaining gaps

Named, not papered over.

1. **`FuzzyScoreV2`'s own class doc-comment is not reproducible.** It claims
   `match("otw","Power of the Wild") = 14`, `-3`, `-13`. Measured against the
   pinned source: with lib_fts' documented constants and the `outScore = 100`
   base removed, `-3` and `-13` reproduce **exactly**; `14` does not - it is
   `7`, and restoring the pre-`eb0c3e59e` recursion limit of 10 does not
   recover it. With KISS's shipped constants the three are 132 / 102 / 81 and
   the documented *ranking* holds. These fixtures have been quoted in this
   project as an acceptance target; they are not one. Pinned by
   `the_doc_comment_fixtures_are_only_partly_reproducible`.
2. **`AppPojo.disabled` has no source on either RISS platform.** The -200
   penalty is ported and tested, but nothing sets the flag: Android's
   `LauncherApps.ApplicationInfo.enabled` is not read and Linux has no
   equivalent. `rebuild_index` passes a constant `false` and says so.
3. **`Pojo.getUserHandle()` (work profiles) is unmodelled**, so step 3 of
   `NameComparator` never fires. Ported and unit-tested so that adding work
   profiles cannot silently change the ordering.
4. **Provider rows are appended below the application matches** rather than
   being merged into one `PriorityQueue` as `DataHandler.requestResults` does.
   That layout predates this change and was left alone; merging it is a
   separate piece of work with its own per-provider relevance question.
5. **`FuzzyScoreV1` is not ported.** `FuzzyFactory` only selects it when
   `use-fuzzy-score-v1` is on (default `false`).
6. **Hexadecimal float preferences** (`0x1p4`) fall back to
   `DEFAULT_MAX_RESULTS` where Java would answer 16. Documented deviation,
   unreachable from the UI, pinned by a test.
7. **The empty-query lists** (`show_all_apps`, history ordering) are RISS's
   own and were not touched. Upstream's equivalents are `ApplicationsSearcher`
   and `HistorySearcher`; porting those is a separate scope.
8. `apply_loaded_apps` rebuilds the index on the UI thread (pure CPU over the
   loaded list, no I/O). The blocking discovery is on the loader thread. On a
   very large drawer this is a one-off cost per reload, not per keystroke.
9. No on-device Android run. `cargo check --target aarch64-linux-android`
   is a compile check, not a device test, and is reported as such.
10. **No compositor run of this branch.** `scripts/gui-assertions.sh` needs
    `sway`, `grim` and ImageMagick, none of which are installable in this
    sandbox. `src/tests/live_app.rs` covers everything up to egui's painting,
    so the gap is specifically "do these rows reach actual pixels" - the one
    thing the repo's GUI job exists for. Cherry fixed that job's apt names on
    `main@3afbe77`; this branch should ride it before anyone calls it done on
    a device.
