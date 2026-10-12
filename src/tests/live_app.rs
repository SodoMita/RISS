//! End-to-end tests of the **real `RissApp`**.
//!
//! `src/tests/live_search.rs` drives the search worker directly. This file
//! goes one layer out and drives the actual application object the binary
//! runs: the same constructor, the same `update_results`, the same
//! `poll_workers` the `eframe::App::update` body calls on every frame, the
//! same generation guards, and the same `self.results` the result list
//! paints from.
//!
//! What is *not* covered here is egui's painting itself (that is
//! `scripts/gui-assertions.sh`, which needs a compositor) and the platform
//! app discovery (`discover_apps` reads the host's `.desktop` files, which a
//! test must not depend on). Everything between those two boundaries is the
//! production path, unmodified.

#[cfg(target_os = "android")]
use crate::android_app_entry::AppEntry;
#[cfg(not(target_os = "android"))]
use crate::app_entry::AppEntry;
use crate::settings::SettingsData;
use crate::ui::RissApp;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn app_entry(name: &str, exec: &str) -> AppEntry {
    AppEntry {
        name: name.to_string(),
        comment: "a comment that must never be searchable".to_string(),
        exec: exec.to_string(),
        icon: String::new(),
        categories: vec!["Utility".to_string()],
        tags: Vec::new(),
        desktop_file: PathBuf::from("/usr/share/applications/x.desktop"),
        launch_count: 0,
        last_launched: 0,
        is_favorite: false,
        is_system: false,
    }
}

/// Build the real app on a bare egui context and install a known app list
/// through the same entry point the loader thread's result takes.
fn launcher(entries: Vec<AppEntry>) -> RissApp {
    let ctx = egui::Context::default();
    let mut app = RissApp::with_context(&ctx, SettingsData::default(), Vec::new());
    app.install_apps_for_test(entries);
    app
}

/// Type a query and pump frames until the worker's answer has been applied,
/// exactly as the UI does: `update_results` queues, `poll_workers` collects.
fn type_query(app: &mut RissApp, query: &str) -> Vec<String> {
    app.set_query_for_test(query);
    let deadline = Instant::now() + Duration::from_secs(10);
    let pending = app.search_pending_for_test();
    while Instant::now() < deadline {
        app.poll_workers_for_test();
        if !app.search_pending_for_test() || !pending {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    app.result_names_for_test()
}

fn sample() -> Vec<AppEntry> {
    vec![
        app_entry("Firefox", "firefox"),
        app_entry("Files", "files"),
        app_entry("Café Noir", "cafe"),
        app_entry("Visual-Studio", "vscode"),
        app_entry("VirtualBox", "vbox"),
        app_entry("Привет Мир", "privet"),
    ]
}

#[test]
fn the_launcher_shows_matches_for_a_typed_query() {
    let mut app = launcher(sample());
    let got = type_query(&mut app, "fire");
    assert!(
        got.iter().any(|n| n == "Firefox"),
        "the launcher painted {got:?}"
    );
}

#[test]
fn the_launcher_finds_an_accented_name_without_the_accent() {
    // The user-visible bug this port fixes: on `main`, `to_lowercase`
    // folding meant "cafe" could not find "Café".
    let mut app = launcher(sample());
    let got = type_query(&mut app, "cafe");
    assert!(
        got.iter().any(|n| n == "Café Noir"),
        "\"cafe\" must find \"Café Noir\"; painted {got:?}"
    );
}

#[test]
fn the_launcher_finds_a_hyphenated_name_without_the_hyphen() {
    let mut app = launcher(sample());
    let got = type_query(&mut app, "visualstudio");
    assert!(
        got.iter().any(|n| n == "Visual-Studio"),
        "\"visualstudio\" must find \"Visual-Studio\"; painted {got:?}"
    );
}

#[test]
fn the_launcher_matches_camel_case_initials() {
    let mut app = launcher(sample());
    let got = type_query(&mut app, "vb");
    assert_eq!(
        got.first().map(String::as_str),
        Some("VirtualBox"),
        "painted {got:?}"
    );
}

#[test]
fn the_launcher_matches_a_cyrillic_name() {
    let mut app = launcher(sample());
    let got = type_query(&mut app, "мир");
    assert!(got.iter().any(|n| n == "Привет Мир"), "painted {got:?}");
}

#[test]
fn the_launcher_does_not_match_the_comment_or_the_category() {
    // KISS's AppProvider matches the name and the tags. The old RISS path
    // folded the comment and the freedesktop categories into one haystack,
    // so "utility" matched the whole drawer.
    let mut app = launcher(sample());
    assert!(
        type_query(&mut app, "utility").is_empty(),
        "categories must not be searchable"
    );
    assert!(
        type_query(&mut app, "searchable").is_empty(),
        "comments must not be searchable"
    );
}

#[test]
fn clearing_the_query_returns_to_the_default_list() {
    let mut app = launcher(sample());
    assert!(!type_query(&mut app, "fire").is_empty());
    let empty = type_query(&mut app, "");
    // The no-query list only holds launched/favourite apps, and this fixture
    // has neither, so it is empty -- but crucially the search results are
    // gone rather than left stranded on screen.
    assert!(empty.is_empty(), "painted {empty:?}");
}

#[test]
fn rapid_typing_leaves_the_launcher_showing_the_last_query() {
    let mut entries = sample();
    for i in 0..3000 {
        entries.push(app_entry(&format!("Filler {i:04}"), &format!("f{i}")));
    }
    let mut app = launcher(entries);
    // Eight keystrokes with no pumping in between: the UI thread never waits
    // for the worker, so this is what a fast typist actually produces.
    for q in ["f", "fi", "fir", "fire", "firef", "firefo", "firefox"] {
        app.set_query_for_test(q);
    }
    let got = type_query(&mut app, "firefox");
    assert_eq!(
        got.first().map(String::as_str),
        Some("Firefox"),
        "the last query must win; painted {got:?}"
    );
}

#[test]
fn a_reload_while_typing_cannot_strand_the_list() {
    let mut app = launcher(sample());
    app.set_query_for_test("fi");
    // The package set changes mid-search, as a package manager would do.
    app.install_apps_for_test(vec![
        app_entry("Fish", "fish"),
        app_entry("Firefox", "firefox"),
    ]);
    let got = type_query(&mut app, "fi");
    assert!(!got.is_empty(), "the list must recover, painted {got:?}");
    for name in &got {
        assert!(
            name == "Fish" || name == "Firefox",
            "a row survived from the replaced app list: {name:?}"
        );
    }
}

#[test]
fn favouriting_an_app_is_visible_on_the_painted_row() {
    // Regression guard for a bug introduced by this very change: the painted
    // rows come from the search index, so a mutation that updated only
    // `all_apps`/`apps` left the row showing stale state.
    let mut app = launcher(sample());
    assert_eq!(type_query(&mut app, "fire").len(), 1);
    assert!(!app.result_is_favorite_for_test(0));

    app.toggle_favorite_for_test("firefox");
    let got = type_query(&mut app, "fire");
    assert_eq!(got.len(), 1);
    assert!(
        app.result_is_favorite_for_test(0),
        "the star must reach the painted row without a disk rescan"
    );
}

#[test]
fn launching_an_app_teaches_the_next_search_of_that_query() {
    let mut app = launcher(sample());
    // "fi" ties Files and Firefox, so the name tie-break puts Files first.
    assert_eq!(type_query(&mut app, "fi")[0], "Files");

    // The user picks Firefox for this query; KISS records the query
    // alongside the launch and QuerySearcher boosts it next time.
    app.record_query_launch_for_test("fi", "firefox");
    assert_eq!(
        type_query(&mut app, "fi")[0],
        "Firefox",
        "the remembered choice must come back first"
    );
    // ... and only for that query.
    assert_eq!(type_query(&mut app, "fil")[0], "Files");
}

#[test]
fn the_result_limit_setting_reaches_the_live_list() {
    let entries: Vec<AppEntry> = (0..40)
        .map(|i| app_entry(&format!("App {i:02}"), &format!("a{i}")))
        .collect();
    let mut app = launcher(entries);
    assert_eq!(type_query(&mut app, "app").len(), 20, "the XML default");

    app.set_setting_for_test("number-of-display-elements", "5");
    assert_eq!(type_query(&mut app, "app").len(), 5);

    // Upstream parses through Double and truncates.
    app.set_setting_for_test("number-of-display-elements", "7.9");
    assert_eq!(type_query(&mut app, "app").len(), 7);

    // A broken preference falls back to DEFAULT_MAX_RESULTS = 50.
    app.set_setting_for_test("number-of-display-elements", "nonsense");
    assert_eq!(type_query(&mut app, "app").len(), 40);
}
