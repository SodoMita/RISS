//! Tests that go through the **live** search path -- the one the UI paints --
//! rather than through private helpers.
//!
//! "Live" here means: build an [`AppIndex`] the way a reload builds it, hand a
//! [`SearchParams`] to the real [`SearchWorker`] (KISS's `SEARCH_THREAD`), and
//! read the published outcome back the way `poll_workers` reads it, including
//! the generation guards. Nothing below reaches into a scoring helper.

#[cfg(target_os = "android")]
use crate::android_app_entry::AppEntry;
#[cfg(not(target_os = "android"))]
use crate::app_entry::AppEntry;
use crate::history::HistoryData;
use crate::kiss::index::AppIndex;
use crate::kiss::prefs::{self, HistoryMode};
use crate::kiss::searcher::SearchParams;
use crate::kiss::worker::SearchWorker;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub fn app(name: &str, exec: &str) -> AppEntry {
    AppEntry {
        name: name.to_string(),
        comment: String::new(),
        exec: exec.to_string(),
        icon: String::new(),
        categories: Vec::new(),
        tags: Vec::new(),
        desktop_file: PathBuf::from("/usr/share/applications/x.desktop"),
        launch_count: 0,
        last_launched: 0,
        is_favorite: false,
        is_system: false,
    }
}

fn index(generation: u64, entries: Vec<AppEntry>) -> Arc<AppIndex> {
    Arc::new(AppIndex::build(generation, entries, |_| false, |_| false))
}

fn params(query: &str, max_results: usize) -> SearchParams {
    SearchParams {
        query: query.to_string(),
        max_results,
        ..Default::default()
    }
}

/// The UI side of `poll_workers`: wait for the generation we are expecting and
/// refuse everything else, then resolve the positions into names.
fn live_results(
    worker: &SearchWorker,
    index: &Arc<AppIndex>,
    expected: u64,
) -> Option<Vec<String>> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Some(outcome) = worker.poll(index.generation) {
            if outcome.generation != expected {
                continue;
            }
            return Some(
                outcome
                    .hits
                    .iter()
                    .filter_map(|h| index.get(h.index))
                    .map(|item| item.entry.name.clone())
                    .collect(),
            );
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    None
}

fn search_live(worker: &SearchWorker, index: &Arc<AppIndex>, p: SearchParams) -> Vec<String> {
    let g = worker.submit(p, Arc::clone(index));
    live_results(worker, index, g).expect("the live path produced no outcome")
}

#[test]
fn live_path_finds_accented_names_without_accents() {
    let worker = SearchWorker::new(|| {});
    let idx = index(
        1,
        vec![
            app("Café Noir", "cafe"),
            app("Nautilus", "nautilus"),
            app("Événement", "evt"),
        ],
    );
    assert_eq!(
        search_live(&worker, &idx, params("cafe", 20))[0],
        "Café Noir"
    );
    assert_eq!(
        search_live(&worker, &idx, params("evenement", 20))[0],
        "Événement"
    );
    // ... and the accented query finds it too.
    assert_eq!(
        search_live(&worker, &idx, params("café", 20))[0],
        "Café Noir"
    );
}

#[test]
fn live_path_is_case_insensitive_but_keeps_case_for_the_camel_bonus() {
    let worker = SearchWorker::new(|| {});
    let idx = index(
        1,
        vec![app("VirtualBox", "vbox"), app("Vorbis Browser", "vorbis")],
    );
    assert_eq!(
        search_live(&worker, &idx, params("VB", 20))[0],
        "VirtualBox"
    );
    assert_eq!(
        search_live(&worker, &idx, params("vb", 20))[0],
        "VirtualBox"
    );
}

#[test]
fn live_path_ignores_dashes_on_both_sides() {
    let worker = SearchWorker::new(|| {});
    let idx = index(
        1,
        vec![app("Visual-Studio", "vs"), app("Visualizer", "viz")],
    );
    assert_eq!(
        search_live(&worker, &idx, params("visualstudio", 20))[0],
        "Visual-Studio"
    );
    assert_eq!(
        search_live(&worker, &idx, params("visual-studio", 20))[0],
        "Visual-Studio"
    );
}

#[test]
fn live_path_matches_beyond_ascii() {
    let worker = SearchWorker::new(|| {});
    let idx = index(
        1,
        vec![
            app("Привет Мир", "ru"),
            app("日本語入力", "ja"),
            app("Ελληνικά", "el"),
            app("한국어", "ko"),
        ],
    );
    assert_eq!(
        search_live(&worker, &idx, params("мир", 20))[0],
        "Привет Мир"
    );
    assert_eq!(
        search_live(&worker, &idx, params("日本", 20))[0],
        "日本語入力"
    );
    assert_eq!(search_live(&worker, &idx, params("ελλ", 20))[0], "Ελληνικά");
    assert_eq!(search_live(&worker, &idx, params("한국", 20))[0], "한국어");
}

#[test]
fn live_path_breaks_equal_scores_by_name() {
    let worker = SearchWorker::new(|| {});
    // Insertion order must not leak into the answer.
    let a = index(1, vec![app("Ab Zulu", "z"), app("Ab Alfa", "a")]);
    let b = index(2, vec![app("Ab Alfa", "a"), app("Ab Zulu", "z")]);
    assert_eq!(
        search_live(&worker, &a, params("ab", 20)),
        ["Ab Alfa", "Ab Zulu"]
    );
    assert_eq!(
        search_live(&worker, &b, params("ab", 20)),
        ["Ab Alfa", "Ab Zulu"]
    );
}

#[test]
fn live_path_honours_the_result_limit() {
    let worker = SearchWorker::new(|| {});
    let entries: Vec<AppEntry> = (0..60)
        .map(|i| app(&format!("App {i:02}"), &format!("a{i}")))
        .collect();
    let idx = index(1, entries);
    assert_eq!(search_live(&worker, &idx, params("app", 20)).len(), 20);
    assert_eq!(search_live(&worker, &idx, params("app", 5)).len(), 5);
    // Upstream trims the queue to the configured count; zero means zero.
    assert_eq!(search_live(&worker, &idx, params("app", 0)).len(), 0);
}

#[test]
fn live_path_result_limit_comes_from_the_parsed_preference() {
    let worker = SearchWorker::new(|| {});
    let entries: Vec<AppEntry> = (0..60)
        .map(|i| app(&format!("App {i:02}"), &format!("a{i}")))
        .collect();
    let idx = index(1, entries);
    // The preference string travels through `Double.valueOf(..).intValue()`.
    for (raw, expect) in [("20", 20usize), ("7.9", 7), ("2e1", 20), ("0", 0)] {
        let max = prefs::max_result_count_usize(Some(raw));
        assert_eq!(
            search_live(&worker, &idx, params("app", max)).len(),
            expect,
            "preference {raw:?}"
        );
    }
    // A broken preference falls back to DEFAULT_MAX_RESULTS = 50.
    let max = prefs::max_result_count_usize(Some("not a number"));
    assert_eq!(search_live(&worker, &idx, params("app", max)).len(), 50);
}

#[test]
fn live_path_hides_excluded_apps_unless_the_preference_is_on() {
    let worker = SearchWorker::new(|| {});
    let idx = Arc::new(AppIndex::build(
        1,
        vec![app("Firefox", "ff"), app("Fossil", "fossil")],
        |e| e.exec == "fossil",
        |_| false,
    ));
    assert_eq!(search_live(&worker, &idx, params("f", 20)), ["Firefox"]);
    let mut p = params("f", 20);
    p.enable_excluded_apps = true;
    assert_eq!(search_live(&worker, &idx, p).len(), 2);
}

#[test]
fn live_path_penalises_disabled_apps_without_hiding_them() {
    let worker = SearchWorker::new(|| {});
    let idx = Arc::new(AppIndex::build(
        1,
        vec![app("Clock", "clock"), app("Clockwork", "cw")],
        |_| false,
        |e| e.exec == "clock",
    ));
    let got = search_live(&worker, &idx, params("clock", 20));
    assert_eq!(got.len(), 2);
    assert_eq!(
        got[0], "Clockwork",
        "the -200 penalty sinks the disabled one"
    );
}

#[test]
fn live_path_applies_the_query_specific_history_boost() {
    let worker = SearchWorker::new(|| {});
    let idx = index(1, vec![app("Firefox", "ff"), app("Files", "fl")]);
    assert_eq!(search_live(&worker, &idx, params("fi", 20))[0], "Files");

    // The user previously picked Firefox *for this query*.
    let mut history = HistoryData::default();
    history.record_launch_for_query("fi", "ff");
    let mut p = params("fi", 20);
    p.known_ids = history.known_ids_for_query("fi", 20);
    assert_eq!(p.known_ids, vec![("ff".to_string(), 1)]);
    assert_eq!(search_live(&worker, &idx, p)[0], "Firefox");

    // ... and the boost is query-scoped: a different query is unaffected.
    let mut p = params("fi", 20);
    p.known_ids = history.known_ids_for_query("fire", 20);
    assert!(p.known_ids.is_empty());
    assert_eq!(search_live(&worker, &idx, p)[0], "Files");
}

#[test]
fn live_path_history_boost_decays_down_the_record_list() {
    let worker = SearchWorker::new(|| {});
    let idx = index(
        1,
        vec![app("Alpha", "a"), app("Alphabet", "b"), app("Alphorn", "c")],
    );
    let mut history = HistoryData::default();
    // Oldest first, so "c" ends up the most recent record.
    history.record_launch_for_query("alph", "a");
    history.record_launch_for_query("alph", "b");
    history.record_launch_for_query("alph", "c");
    let known = history.known_ids_for_query("alph", 20);
    assert_eq!(
        known,
        vec![
            ("c".to_string(), 3),
            ("b".to_string(), 2),
            ("a".to_string(), 1)
        ]
    );
    let mut p = params("alph", 20);
    p.known_ids = known;
    assert_eq!(search_live(&worker, &idx, p)[0], "Alphorn");
}

#[test]
fn live_path_history_mode_alphabetically_flattens_the_boost() {
    let worker = SearchWorker::new(|| {});
    let idx = index(1, vec![app("Alpha", "a"), app("Alphabet", "b")]);
    let mut p = params("alph", 20);
    // A record far down the list: a decaying boost would be 0 here.
    p.known_ids = vec![("b".to_string(), 1)];
    p.history_mode = HistoryMode::Recency;
    let decayed = search_live(&worker, &idx, p.clone());
    p.history_mode = HistoryMode::Alphabetically;
    let flat = search_live(&worker, &idx, p);
    assert_eq!(decayed[0], "Alphabet");
    assert_eq!(flat[0], "Alphabet");
    // The modes differ in magnitude, which shows up with 21+ records.
    let mut p = params("alph", 20);
    p.known_ids = (0..21)
        .map(|i| (if i == 20 { "b" } else { "zz" }.to_string(), 1))
        .collect();
    p.history_mode = HistoryMode::Recency;
    assert_eq!(search_live(&worker, &idx, p.clone())[0], "Alpha");
    p.history_mode = HistoryMode::Alphabetically;
    assert_eq!(search_live(&worker, &idx, p)[0], "Alphabet");
}

#[test]
fn live_path_drops_excluded_favorites() {
    let worker = SearchWorker::new(|| {});
    let idx = index(1, vec![app("Firefox", "ff"), app("Files", "fl")]);
    let mut p = params("fi", 20);
    p.excluded_favorite_ids = vec!["fl".to_string()];
    assert_eq!(search_live(&worker, &idx, p), ["Firefox"]);
}

#[test]
fn rapid_typing_never_paints_a_stale_answer() {
    let worker = SearchWorker::new(|| {});
    let entries: Vec<AppEntry> = (0..6000)
        .map(|i| app(&format!("App {i:05}"), &format!("a{i}")))
        .collect();
    let idx = index(1, entries);

    // Eight keystrokes, no waiting in between -- the UI never blocks.
    let queries = [
        "a", "ap", "app", "app ", "app 0", "app 00", "app 000", "app 0001",
    ];
    let mut expected = 0;
    for q in queries {
        expected = worker.submit(params(q, 20), Arc::clone(&idx));
    }
    let got = live_results(&worker, &idx, expected).expect("no outcome");
    assert!(
        got[0].starts_with("App 0001"),
        "the newest query's answer is what landed, got {:?}",
        got[0]
    );
    // Every superseded search was abandoned: nothing else is waiting.
    std::thread::sleep(Duration::from_millis(50));
    assert!(worker.poll(idx.generation).is_none());
}

#[test]
fn a_reload_landing_mid_search_invalidates_the_answer() {
    let worker = SearchWorker::new(|| {});
    let old: Vec<AppEntry> = (0..4000)
        .map(|i| app(&format!("Old {i:04}"), &format!("o{i}")))
        .collect();
    let old_idx = index(1, old);
    worker.submit(params("old", 20), Arc::clone(&old_idx));

    // The package list changed; the UI now holds generation 2.
    let new_idx = index(2, vec![app("New App", "n1")]);
    let g = worker.submit(params("new", 20), Arc::clone(&new_idx));

    let got = live_results(&worker, &new_idx, g).expect("no outcome");
    assert_eq!(got, ["New App"]);
    // The in-flight generation-1 answer is refused by the data-generation
    // guard, so it can never index into the new snapshot.
    assert!(worker.poll(new_idx.generation).is_none());
}

#[test]
fn the_ui_is_notified_instead_of_polling_every_frame() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let w = Arc::clone(&wakes);
    let worker = SearchWorker::new(move || {
        w.fetch_add(1, Ordering::SeqCst);
    });
    let idx = index(1, vec![app("Firefox", "ff")]);
    let g = worker.submit(params("fi", 20), Arc::clone(&idx));
    live_results(&worker, &idx, g).expect("no outcome");
    assert_eq!(
        wakes.load(Ordering::SeqCst),
        1,
        "exactly one repaint request per published search"
    );
}

#[test]
fn cache_invalidation_when_the_inputs_change() {
    let worker = SearchWorker::new(|| {});
    let idx = index(1, vec![app("Firefox", "ff"), app("Files", "fl")]);

    // Baseline.
    assert_eq!(search_live(&worker, &idx, params("fi", 20))[0], "Files");

    // 1. History changed.
    let mut p = params("fi", 20);
    p.known_ids = vec![("ff".to_string(), 1)];
    assert_eq!(search_live(&worker, &idx, p)[0], "Firefox");

    // 2. A relevant setting changed (the limit).
    assert_eq!(search_live(&worker, &idx, params("fi", 1)).len(), 1);

    // 3. The app list changed.
    let idx2 = index(2, vec![app("Fish", "fish")]);
    assert_eq!(search_live(&worker, &idx2, params("fi", 20)), ["Fish"]);

    // 4. Provider state changed (an app became excluded).
    let idx3 = Arc::new(AppIndex::build(
        3,
        vec![app("Firefox", "ff"), app("Files", "fl")],
        |e| e.exec == "fl",
        |_| false,
    ));
    assert_eq!(search_live(&worker, &idx3, params("fi", 20)), ["Firefox"]);
}

#[test]
fn tags_are_part_of_the_live_match() {
    let worker = SearchWorker::new(|| {});
    let mut tagged = app("Nautilus", "nautilus");
    tagged.tags = vec!["Files".to_string(), "browser".to_string()];
    let idx = index(1, vec![tagged, app("Gedit", "gedit")]);
    assert_eq!(
        search_live(&worker, &idx, params("files", 20)),
        ["Nautilus"]
    );
}
