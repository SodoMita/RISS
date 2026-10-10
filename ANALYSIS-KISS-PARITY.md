# RISS vs. the original KISS: where the Java launcher is right, and why it is faster

Branch `graft/lathe`, base `main@62e5f39`. Read side by side:

* RISS: `src/search.rs`, `src/ui/mod.rs`, `src/ui/results.rs`, `src/app_entry.rs`, `src/history.rs`
* KISS (Neamar/KISS, master): `dataprovider/AppProvider.java`, `searcher/Searcher.java`,
  `searcher/QuerySearcher.java`, `pojo/Pojo.java`, `normalizer/StringNormalizer.java`,
  `utils/fuzzy/*`, `IconsHandler.java`, `result/AppResult.java`, `MainActivity.java`,
  `adapter/RecordAdapter.java`

The short version: **KISS is not faster because Java is magically quick. It is faster because its
architecture refuses to redo work.** RISS had a systems language and still redid the work — once per
keystroke, on the render thread. Rust loses to Java every time you write that code.

---

## 1. Why the Java original is faster

### 1.1 The search never runs on the UI thread

```java
public static final ExecutorService SEARCH_THREAD = Executors.newSingleThreadExecutor();
public abstract class Searcher extends AsyncTask<Void, Result<?>, Void> { ... }
```

`MainActivity.search()` hands the query to `SEARCH_THREAD`. Providers push into a
`PriorityQueue<Pojo>` from that background thread (`Searcher.addResults`), and only
`onPostExecute` touches the adapter. `updateSearchRecords()` also calls `cancelSearch()` first, and
`addResults()` bails out on `isCancelled()`, so a stale query stops contributing the moment a newer
one arrives.

RISS: `update_results()` runs inside `eframe::update()`, i.e. on the render thread, *before* the
frame is painted. Every keystroke pays the whole search before a single pixel is drawn, and nothing
cancels the previous query.

### 1.2 Normalization is paid once per app, not once per keystroke

KISS stores the normalized name *on the pojo*:

```java
// Pojo.java
public StringNormalizer.Result normalizedName = null;
...
this.normalizedName = StringNormalizer.normalizeWithResult(this.name, false);
```

`AppProvider.requestResults()` therefore normalizes **only the query**, then loops over ready-made
code points:

```java
StringNormalizer.Result queryNormalized = StringNormalizer.normalizeWithResult(query, false);
FuzzyScore fuzzyScore = FuzzyFactory.createFuzzyScore(this, queryNormalized.codePoints);
for (AppPojo pojo : getPojos()) {
    MatchInfo matchInfo = fuzzyScore.match(pojo.normalizedName.codePoints);
    ...
}
```

`StringNormalizer` even has an ASCII fast path (`if (codepoint <= 'z')`) so Latin app names never
reach `java.text.Normalizer`. KISS's fuzzy scorer works on `int[]` code points and allocates nothing
per app.

RISS, per keystroke, for **every installed application**: `display_name(app)` (fresh `String`,
because an alias means `clone()`), `.to_lowercase()` (another `String`), `searchable_text(app)`
(`Vec` + `join` of name, comment, tags and categories), plus `to_lowercase()` per tag and per
category. ~300 apps → ~1000 allocations before the fuzzy matcher even starts.

### 1.3 The result set is bounded while it is collected

`Searcher` collects into a `PriorityQueue` ordered by `RelevanceComparator` (a plain `int` compare)
and only keeps `getMaxResultCount()` entries; `onPostExecute` polls the overflow away. KISS never
sorts a list it is going to throw away.

RISS collected **every** match into a `Vec<AppEntry>`-carrying struct (cloning each entry: two
`String`s, two `Vec<String>`s and a `PathBuf` each), sorted all of them, then truncated to
`number-of-display-elements`.

### 1.4 Providers reload only when packages change

`AppProvider` registers a package-listener and calls `reload()` on
`ACTION_PACKAGE_ADDED/REMOVED/CHANGED/...`. A favorite toggle, a rename or a tag edit does **not**
re-read the package manager — `DataHandler` keeps the pojos and only the records change.

RISS called `reload_apps()` → `app_entry::discover_apps()` (a full `.desktop` walk of every XDG data
directory) after **every launch, favorite toggle, favorite reorder, rename, tag edit, custom-icon
edit and several settings changes** (22 call sites). Toggling a favorite cost a whole app-list
rescan. That is the single clearest "why does RISS feel sluggish" answer in the tree.

### 1.5 Icons: memoized per result, cached on disk, bounded

* `AppResult.getDrawable()` double-checked-locks the `Drawable` onto the result object
  (`isDrawableCached()` / `setDrawableCache()`), so a recycled row never refetches.
* `IconsHandler` writes decoded icons to `cacheDir/icons/{iconPack}_{componentName.hashCode()}.png`
  and reads them back with `BitmapFactory` — the expensive part (resolving the activity icon, icon
  pack masking, adaptive shapes) happens once per app, ever.
* `IconsHandler.clearOldCache()` prunes the directory when the pack or the shape preference changes.

RISS: `icon_texture()` decoded the PNG/JPEG **on the render thread** the first time a row scrolled
into view (`ui.is_rect_visible` gates the decode, so one fling through the all-apps list decodes the
entire application set in a burst), and `icon_textures: HashMap<String, TextureHandle>` kept every
texture alive for the whole session.

### 1.6 Java's overhead is per-call, and KISS never multiplies it

At 300 apps × 5 keystrokes, JVM dispatch overhead is irrelevant; the *multiplier* is what matters.
KISS multiplies by `apps` once per query against a pre-normalized corpus. RISS multiplied by `apps`
the allocations, the clones, the joins, the icon decodes *and* a per-frame result clone. That is why
the Rust launcher lost to the Java one.

---

## 2. What I changed on this branch

| # | RISS before | KISS reference | Change |
|---|---|---|---|
| 1 | `search()` cloned an `AppEntry` for **every** app, then truncated | pojos are added to the queue by reference, never cloned | only matching rows become a `SearchResult`; `SearchIndex` owns the copies |
| 2 | per-keystroke `to_lowercase()` / `join()` / per-tag normalization for all apps | `normalizedName` + `normalizedTags` computed once at load | `search::SearchIndex::build()`, rebuilt only when the app list or the aliases change |
| 3 | sort comparator allocated two `String`s per comparison | `RelevanceComparator` compares `int`s | `sort_by_cached_key()` (one key per row) |
| 4 | history sort called `rank_score()` twice per comparison | comparator reads `pojo.relevance` | rank scores computed once per row, then two stable passes (score, then name) |
| 5 | `.desktop` rescan on 21 history/favorite/settings paths | provider reload on package broadcast only | `refresh_from_history()` (no I/O); `reload_apps()` is only the startup/package path |
| 6 | `icon_textures: HashMap` grew for the whole session | bounded + disk-backed + memoized per result | `ui/icon_cache.rs` LRU, `ICON_CACHE_LIMIT = 256`, with unit tests |
| 7 | icon cache key allocated per row per frame (`entry.icon.clone()`) | key already held on the pojo | borrowed `&str` key; lookups allocate nothing |
| 8 | `self.results.clone()` on **every frame** (deep clone of every visible entry) | adapter owns the list, `updateWithPojos` swaps it | `mem::take` + `results_generation` guard so a row handler that rebuilds results keeps its fresh rows |
| 9 | renames could shadow the original name in the search corpus | alias and original name are both searchable | the corpus keeps the original name with the alias in front (locked by `index_honors_renames_like_kiss_aliases`) |

New tests: `index_matches_the_plain_tiers`, `index_honors_renames_like_kiss_aliases`,
`tags_carry_a_match_without_being_a_prefix`, `history_boost_only_lands_on_fuzzy_matches`,
`candidates_limit_what_can_match`, `index_is_built_once_per_reload_not_per_keystroke`, plus the four
`icon_cache` tests. `SearchIndex` also absorbs the old "searchable pool" split: `included` /
`searchable` position lists replace the second full copy of the app list that `self.apps` used to be
for query purposes.

## 3. Known gaps left open on purpose

* **#21 — the launch-count boost only lands on fuzzy matches.** KISS adds history relevance to every
  pojo (`QuerySearcher.addResults`), RISS boosts only the fuzzy tier. That issue is already claimed
  in the orchard, so this branch only locks the current behaviour with a test instead of changing it.
* **No query-keyed history boost.** KISS reads `DBHelper.getHistory(max, mode, query)` and boosts
  `knownIds` by `max(0, 100 - position * 5)` — "you chose this app for this query before". RISS
  records launches without the query, so it cannot do this; it needs an optional `query` column on
  the history record.
* **No search cancellation.** Needs the worker thread (below); `results_generation` is the hook the
  generation counter would hang off.
* **SkimMatcherV2 allocates.** Porting KISS's `FuzzyScoreV2` (code points, no allocation, match-info
  reuse) is worthwhile only if a profile says the matcher is still hot.

## 4. How to close the remaining gap

1. **Move the search off the render thread** — the KISS architectural core:
   `Arc<SearchIndex>` + a `std::sync::mpsc` worker, a query generation counter for cancellation
   (`AsyncTask.cancel()` parity), `ctx.request_repaint()` on completion. The index is already
   immutable between reloads, which is exactly why it is a separate type now.
2. **Decode icons off-thread**, memoize the texture, keep the LRU as the eviction policy.
3. **Bound the collection**, not just the output: keep a `BinaryHeap` of the best
   `number-of-display-elements` rows while scanning, instead of sorting every match.
4. **Add the query column to history** and mirror `QuerySearcher`'s boost; this is the feature that
   makes KISS *feel* precognitive after a week of use.

## Receipts

`cargo check --all-targets` clean · `cargo test` **81 passed / 0 failed** · `cargo clippy
--all-targets` clean · `cargo fmt --all` clean.
