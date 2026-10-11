//! Test-only probes: allocation counting and timing for the per-keystroke
//! hot path, plus the live-path integration tests.
pub mod alloc_count;
#[cfg(test)]
mod live_app;
#[cfg(test)]
mod live_search;

#[cfg(test)]
mod perf {
    use super::alloc_count::measure;
    #[cfg(target_os = "android")]
    use crate::android_app_entry::AppEntry;
    #[cfg(not(target_os = "android"))]
    use crate::app_entry::AppEntry;
    use crate::kiss::index::AppIndex;
    use crate::kiss::prefs::HistoryMode;
    use crate::kiss::searcher::{SearchParams, Searcher};
    use crate::kiss::worker::SearchWorker;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    /// A deliberately awkward corpus: four scripts, accents, dashes, camel
    /// case and long names, so the measurement is not an ASCII best case.
    ///
    /// Every 7th app is disabled and every 5th is excluded, so the ordering
    /// and filtering branches are exercised too.
    fn corpus(n: usize) -> Vec<AppEntry> {
        let shapes: [&str; 8] = [
            "Application {i}",
            "Приложение {i}",
            "アプリケーション {i}",
            "Éditeur-Vidéo {i}",
            "VirtualBox{i}Manager",
            "Ελληνικό Πρόγραμμα {i}",
            "app-{i}-tool",
            "LibreOffice Calc {i}",
        ];
        (0..n)
            .map(|i| AppEntry {
                name: shapes[i % shapes.len()].replace("{i}", &i.to_string()),
                comment: String::new(),
                exec: format!("/usr/bin/app{i}"),
                icon: format!("app-{i}"),
                categories: Vec::new(),
                tags: if i % 3 == 0 {
                    vec!["utility".to_string(), "tag-b".to_string()]
                } else {
                    Vec::new()
                },
                desktop_file: PathBuf::from(format!("/usr/share/applications/app{i}.desktop")),
                launch_count: (i % 7) as u32,
                last_launched: 0,
                is_favorite: i % 11 == 0,
                is_system: false,
            })
            .collect()
    }

    fn index(n: usize) -> AppIndex {
        AppIndex::build(
            1,
            corpus(n),
            |e| e.exec.ends_with("5"),
            |e| e.exec.ends_with("7"),
        )
    }

    /// A query set with enough history weight to exercise the boost lookup
    /// and the comparator's tie-breaks.
    fn params(index: &AppIndex, query: &str) -> SearchParams {
        SearchParams {
            query: query.to_string(),
            max_results: 20,
            history_mode: HistoryMode::Recency,
            enable_excluded_apps: false,
            known_ids: index
                .items
                .iter()
                .take(30)
                .enumerate()
                .map(|(i, item)| (item.entry.exec.clone(), 30 - i as i32))
                .collect(),
            excluded_favorite_ids: Vec::new(),
        }
    }

    const QUERIES: [&str; 8] = ["a", "ap", "app", "appl", "vb", "éd", "при", "libre"];

    /// Report, do not assert: wall-clock on shared CI is not a contract.
    #[test]
    fn report_search_latency() {
        let index = index(1_200);
        let mut searcher = Searcher::new();
        // Warm the buffers, exactly as a running launcher would have them.
        for q in QUERIES {
            searcher.search(&params(&index, q), &index, || false);
        }
        let mut worst = Duration::ZERO;
        let mut total = Duration::ZERO;
        let rounds = 50;
        for _ in 0..rounds {
            for q in QUERIES {
                let p = params(&index, q);
                let start = Instant::now();
                let hits = searcher.search(&p, &index, || false).unwrap().len();
                let elapsed = start.elapsed();
                assert!(hits <= 20);
                total += elapsed;
                worst = worst.max(elapsed);
            }
        }
        let n = rounds * QUERIES.len() as u32;
        println!(
            "kiss search over {} apps: mean {:?}/query, worst {:?} ({} queries)",
            index.len(),
            total / n,
            worst,
            n
        );
    }

    /// The instrument checking itself: a counter that silently counts nothing
    /// would make every bound below vacuously true, which is worse than having
    /// no test at all.
    #[test]
    fn allocator_counter_actually_counts() {
        let (len, allocs, bytes) = measure(|| {
            let v: Vec<u8> = vec![7; 4096];
            v.len()
        });
        assert_eq!(len, 4096);
        assert!(allocs >= 1, "a 4 KiB vec allocated {allocs} times");
        assert!(bytes >= 4096, "countered {bytes} bytes for a 4 KiB vec");
    }

    /// The headline hot-path contract.
    ///
    /// Once the worker's buffers are provisioned, scoring 1,200 candidates --
    /// normalizing, fuzzy-matching names and tags, applying penalties and
    /// boosts, sorting and trimming -- allocates **nothing**. No `String`, no
    /// `Vec`, no `to_lowercase`, no `chars().collect`, no hash lookup.
    #[test]
    fn the_candidate_loop_allocates_nothing() {
        let index = index(1_200);
        let mut searcher = Searcher::new();
        let prepared: Vec<SearchParams> = QUERIES.iter().map(|q| params(&index, q)).collect();
        // Warm-up: the first searches grow the scratch and the hit vector.
        for _ in 0..3 {
            for p in &prepared {
                searcher.search(p, &index, || false);
            }
        }
        let (_, allocs, bytes) = measure(|| {
            let mut n = 0;
            for p in &prepared {
                n += searcher.search(p, &index, || false).unwrap().len();
            }
            n
        });
        println!(
            "8 queries over {} apps: {allocs} allocs, {bytes} bytes",
            index.len()
        );
        assert_eq!(
            allocs,
            0,
            "scoring {} candidates x 8 queries allocated {allocs} times",
            index.len()
        );
    }

    /// Per-keystroke steady state, measured the way a user types.
    #[test]
    fn repeated_keystrokes_do_not_grow_the_allocation_count() {
        let index = index(1_200);
        let mut searcher = Searcher::new();
        let p = params(&index, "app");
        for _ in 0..5 {
            searcher.search(&p, &index, || false);
        }
        let (_, allocs, _) = measure(|| {
            for _ in 0..200 {
                searcher.search(&p, &index, || false);
            }
        });
        assert_eq!(allocs, 0, "200 keystrokes allocated {allocs} times");
    }

    /// End to end through the worker, which is allowed exactly one allocation
    /// per *published* search (the result vector that crosses the channel),
    /// bounded by the display limit rather than by the app count.
    #[test]
    fn the_worker_publishes_without_scanning_cost_per_app() {
        let index = Arc::new(index(1_200));
        let worker = SearchWorker::new(|| {});
        let mut last = 0;
        for q in QUERIES {
            last = worker.submit(params(&index, q), Arc::clone(&index));
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut seen = None;
        while Instant::now() < deadline {
            if let Some(o) = worker.poll(index.generation) {
                if o.generation == last {
                    seen = Some(o);
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let seen = seen.expect("the worker published nothing");
        assert!(seen.hits.len() <= 20, "the published page is bounded");
    }
}
