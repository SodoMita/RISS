//! Test-only probes: allocation counting for the per-keystroke hot paths.
pub mod alloc_count;

#[cfg(test)]
mod perf {
    use super::alloc_count::measure;
    use crate::app_entry::AppEntry;
    use crate::search::SearchEngine;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn corpus(n: usize) -> Vec<AppEntry> {
        (0..n)
            .map(|i| AppEntry {
                name: format!("App {i}"),
                comment: format!("A comment for application number {i}"),
                exec: format!("/usr/bin/app{i}"),
                icon: format!("app-{i}"),
                categories: vec!["Utility".to_string(), "Development".to_string()],
                tags: vec!["tag-a".to_string(), "tag-b".to_string()],
                desktop_file: PathBuf::from(format!("/usr/share/applications/app{i}.desktop")),
                launch_count: (i % 7) as u32,
                last_launched: 0,
                is_favorite: i % 11 == 0,
                is_system: false,
            })
            .collect()
    }

    #[test]
    fn report_allocations_per_query() {
        let apps = corpus(2000);
        let aliases: HashMap<String, String> = HashMap::new();
        let engine = SearchEngine::new();
        for query in ["a", "app 1", "firefox"] {
            let (_, allocs, bytes) = measure(|| engine.search(query, &apps, &aliases, 12));
            println!("search({query:?}, 2000 apps, max 12): {allocs} allocs, {bytes} bytes");
        }
        let (_, allocs, bytes) = measure(|| engine.get_default_apps(&apps, usize::MAX));
        println!("get_default_apps(2000, MAX): {allocs} allocs, {bytes} bytes");
        let (_, allocs2, bytes2) = measure(|| engine.get_default_apps(&apps, 12));
        println!("get_default_apps(2000, 12): {allocs2} allocs, {bytes2} bytes");
        let (_, allocs3, bytes3) = measure(|| {
            let mut out = Vec::new();
            for _ in 0..50 {
                out.push(engine.search("app 5", &apps, &aliases, 12));
            }
            out
        });
        println!(
            "50 keystrokes: {allocs3} allocs, {bytes3} bytes ({} per keystroke)",
            allocs3 / 50
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

    /// A regression guard with no timing in it: allocation counts are
    /// deterministic, wall-clock on shared CI is not.
    #[test]
    fn search_clones_only_the_survivors() {
        let apps = corpus(2000);
        let aliases: HashMap<String, String> = HashMap::new();
        let engine = SearchEngine::new();
        let (_, allocs, _) = measure(|| engine.search("a", &apps, &aliases, 12));
        assert!(
            allocs < 2_000,
            "a 12-row page allocated {allocs} times; ranking must stay over \
             borrowed candidates and clone only what is shown (main@pre-fix: 89_112)"
        );
    }

    #[test]
    fn default_view_clones_only_the_survivors() {
        let apps = corpus(2000);
        let engine = SearchEngine::new();
        let (_, allocs, _) = measure(|| engine.get_default_apps(&apps, 12));
        assert!(
            allocs < 2_000,
            "the default list allocated {allocs} times for 12 rows \
             (main@pre-fix: 19_151)"
        );
    }
}
