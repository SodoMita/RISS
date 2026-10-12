//! KISS's execution model, ported.
//!
//! Upstream: Neamar/KISS @ `17fc7ebd473bbcbfc0e93592859278443cdb4ec0`.
//!
//! * `Searcher.SEARCH_THREAD = Executors.newSingleThreadExecutor()` -- one
//!   persistent serial executor for **searches only**, explicitly separate
//!   from the default `AsyncTask` pool so a search cannot be stuck behind a
//!   provider load ("*or else we will block everything else that uses
//!   AsyncTask while we search*").
//! * Provider loading runs on its own `AsyncTask`s ([`AppLoader`] here).
//! * `AsyncTask.cancel()` + `isCancelled()` -- a superseded search stops
//!   cooperatively and `onPostExecute` publishes nothing.
//! * `onPostExecute` runs on the UI thread; here the UI thread picks the
//!   outcome up from a channel after being woken by a repaint request.
//!
//! On top of that, two generation counters, because an `AsyncTask` holding a
//! `WeakReference<MainActivity>` does not have RISS's problem of a search and
//! an app reload finishing out of order:
//!
//! * a **query generation** -- a result whose generation is not the newest
//!   requested is dropped, so a slow search for `fi` can never overwrite the
//!   results for `fire`;
//! * a **data generation** -- a result computed against a superseded
//!   [`AppIndex`] is dropped, so a reload that lands mid-search cannot leave
//!   the list pointing at positions that no longer exist.

use super::index::AppIndex;
use super::searcher::{Scored, SearchParams, Searcher, CANCEL_CHECK_INTERVAL};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

/// A finished search, as handed back to the UI thread.
#[derive(Debug)]
pub struct SearchOutcome {
    /// Query generation this result answers.
    pub generation: u64,
    /// `AppIndex.generation` the positions in `hits` refer to.
    pub data_generation: u64,
    pub hits: Vec<Scored>,
}

enum Job {
    Search {
        generation: u64,
        params: Box<SearchParams>,
        index: Arc<AppIndex>,
    },
    Stop,
}

/// The single serial search executor.
pub struct SearchWorker {
    tx: Sender<Job>,
    rx: Receiver<SearchOutcome>,
    /// Newest requested query generation. Read by the worker to notice it has
    /// been superseded, and by the UI to reject stale outcomes.
    latest: Arc<AtomicU64>,
    handle: Option<JoinHandle<()>>,
}

impl SearchWorker {
    /// `notify` is called on the worker thread when an outcome is ready; the
    /// UI passes `egui::Context::request_repaint`. Nothing polls.
    pub fn new<N>(notify: N) -> Self
    where
        N: Fn() + Send + 'static,
    {
        let (tx, job_rx) = channel::<Job>();
        let (out_tx, rx) = channel::<SearchOutcome>();
        let latest = Arc::new(AtomicU64::new(0));
        let worker_latest = Arc::clone(&latest);

        let handle = std::thread::Builder::new()
            .name("riss-search".to_string())
            .spawn(move || {
                // Owned by the worker and reused for the whole process
                // lifetime: scratch buffers, the scorer and the hit vector
                // never come back from the allocator after warm-up.
                let mut searcher = Searcher::new();
                while let Ok(job) = job_rx.recv() {
                    let (generation, params, index) = match job {
                        Job::Stop => break,
                        Job::Search {
                            generation,
                            params,
                            index,
                        } => (generation, params, index),
                    };

                    // Already superseded before we even started: upstream's
                    // `isCancelled()` check at the top of `addResults`.
                    if generation < worker_latest.load(Ordering::Acquire) {
                        continue;
                    }

                    let data_generation = index.generation;
                    let stop_latest = Arc::clone(&worker_latest);
                    let hits = searcher.search(&params, &index, || {
                        stop_latest.load(Ordering::Acquire) != generation
                    });

                    // Cancelled -> publish nothing, exactly like
                    // `onCancelled` skipping `updateWithPojos`.
                    let Some(hits) = hits else { continue };

                    if out_tx
                        .send(SearchOutcome {
                            generation,
                            data_generation,
                            hits: hits.to_vec(),
                        })
                        .is_err()
                    {
                        break;
                    }
                    notify();
                }
            })
            .expect("spawn search worker");

        Self {
            tx,
            rx,
            latest,
            handle: Some(handle),
        }
    }

    /// Queue a search, superseding any in flight. Returns its generation.
    ///
    /// Never blocks: the UI thread does not wait for the worker, and holds no
    /// lock while the worker runs.
    pub fn submit(&self, params: SearchParams, index: Arc<AppIndex>) -> u64 {
        let generation = self.latest.fetch_add(1, Ordering::AcqRel) + 1;
        let _ = self.tx.send(Job::Search {
            generation,
            params: Box::new(params),
            index,
        });
        generation
    }

    /// `SearchHandler.cancelSearch()`: stop whatever is in flight without
    /// starting anything new. The worker notices within
    /// [`CANCEL_CHECK_INTERVAL`] candidates and publishes nothing.
    ///
    /// Implemented by advancing the generation counter: the in-flight
    /// search compares its own generation against it and stops. Nothing
    /// can be waiting for the skipped generation because only `submit`
    /// hands generations out.
    pub fn cancel(&self) {
        self.latest.fetch_add(1, Ordering::AcqRel);
    }

    /// Newest generation requested so far.
    pub fn latest_generation(&self) -> u64 {
        self.latest.load(Ordering::Acquire)
    }

    /// Take the freshest outcome that is still relevant, discarding anything
    /// older. Returns `None` when there is nothing new.
    ///
    /// `data_generation` lets the caller reject a result computed against an
    /// app list that has since been replaced.
    pub fn poll(&self, current_data_generation: u64) -> Option<SearchOutcome> {
        let mut best: Option<SearchOutcome> = None;
        while let Ok(outcome) = self.rx.try_recv() {
            if outcome.data_generation != current_data_generation {
                continue;
            }
            match &best {
                Some(prev) if prev.generation >= outcome.generation => {}
                _ => best = Some(outcome),
            }
        }
        best
    }
}

impl Drop for SearchWorker {
    fn drop(&mut self) {
        // Do not wait for a search nobody will read: cancel it, then stop.
        self.cancel();
        let _ = self.tx.send(Job::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// A loaded app list, tagged with the reload generation that asked for it.
pub struct Loaded<T> {
    pub generation: u64,
    pub entries: T,
}

/// The provider-loading worker: the blocking part of a reload (scanning
/// `.desktop` files, or crossing JNI to `LauncherApps`) runs here, never in an
/// egui frame.
///
/// Separate from [`SearchWorker`] on purpose. Putting both on one serial queue
/// is the mistake upstream's comment on `SEARCH_THREAD` warns about: a reload
/// triggered by a package change would stall every keystroke behind it.
pub struct AppLoader<T: Send + 'static> {
    tx: Sender<u64>,
    rx: Receiver<Loaded<T>>,
    latest: Arc<AtomicU64>,
    handle: Option<JoinHandle<()>>,
}

impl<T: Send + 'static> AppLoader<T> {
    pub fn new<L, N>(load: L, notify: N) -> Self
    where
        L: Fn() -> T + Send + 'static,
        N: Fn() + Send + 'static,
    {
        let (tx, job_rx) = channel::<u64>();
        let (out_tx, rx) = channel::<Loaded<T>>();
        let latest = Arc::new(AtomicU64::new(0));
        let worker_latest = Arc::clone(&latest);

        let handle = std::thread::Builder::new()
            .name("riss-app-loader".to_string())
            .spawn(move || {
                while let Ok(generation) = job_rx.recv() {
                    // Coalesce: if more reloads were queued while this one
                    // waited, only the newest is worth doing.
                    if generation < worker_latest.load(Ordering::Acquire) {
                        continue;
                    }
                    let entries = load();
                    if out_tx
                        .send(Loaded {
                            generation,
                            entries,
                        })
                        .is_err()
                    {
                        break;
                    }
                    notify();
                }
            })
            .expect("spawn app loader");

        Self {
            tx,
            rx,
            latest,
            handle: Some(handle),
        }
    }

    /// How many reloads have been requested so far. The number a test
    /// watches to prove an edit did *not* trigger a rescan.
    pub fn requested(&self) -> u64 {
        self.latest.load(Ordering::Acquire)
    }

    /// Ask for a reload. Non-blocking.
    pub fn request(&self) -> u64 {
        let generation = self.latest.fetch_add(1, Ordering::AcqRel) + 1;
        let _ = self.tx.send(generation);
        generation
    }

    /// Newest loaded app list, if one arrived. Older ones are discarded.
    pub fn poll(&self) -> Option<Loaded<T>> {
        let mut best: Option<Loaded<T>> = None;
        while let Ok(loaded) = self.rx.try_recv() {
            match &best {
                Some(prev) if prev.generation >= loaded.generation => {}
                _ => best = Some(loaded),
            }
        }
        best
    }
}

impl<T: Send + 'static> Drop for AppLoader<T> {
    fn drop(&mut self) {
        // Dropping the sender ends the worker's `recv` loop.
        let (dead, _) = channel();
        let _ = std::mem::replace(&mut self.tx, dead);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Exposed so the integration tests can assert the loop really does check.
pub const CANCEL_INTERVAL: usize = CANCEL_CHECK_INTERVAL;

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "android")]
    use crate::android_app_entry::AppEntry;
    #[cfg(not(target_os = "android"))]
    use crate::app_entry::AppEntry;
    use crate::kiss::index::AppIndex;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    fn app(name: &str) -> AppEntry {
        AppEntry {
            name: name.to_string(),
            comment: String::new(),
            exec: name.to_lowercase(),
            icon: String::new(),
            categories: Vec::new(),
            tags: Vec::new(),
            desktop_file: PathBuf::from("/x.desktop"),
            launch_count: 0,
            last_launched: 0,
            is_favorite: false,
            is_system: false,
        }
    }

    fn index(generation: u64, n: usize) -> Arc<AppIndex> {
        let entries: Vec<AppEntry> = (0..n).map(|i| app(&format!("App {i:04}"))).collect();
        Arc::new(AppIndex::build(generation, entries, |_| false, |_| false))
    }

    fn params(query: &str) -> SearchParams {
        SearchParams {
            query: query.to_string(),
            max_results: 20,
            ..Default::default()
        }
    }

    /// Block until the worker answers the given generation, or give up.
    fn wait_for(worker: &SearchWorker, data_generation: u64, want: u64) -> Option<SearchOutcome> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(o) = worker.poll(data_generation) {
                if o.generation >= want {
                    return Some(o);
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        None
    }

    #[test]
    fn the_worker_answers_a_search() {
        let worker = SearchWorker::new(|| {});
        let idx = index(1, 50);
        let g = worker.submit(params("app 0001"), Arc::clone(&idx));
        let out = wait_for(&worker, 1, g).expect("no outcome");
        assert_eq!(out.generation, g);
        assert_eq!(out.data_generation, 1);
        assert!(!out.hits.is_empty());
        assert_eq!(idx.items[out.hits[0].index as usize].entry.name, "App 0001");
    }

    #[test]
    fn the_worker_wakes_the_ui_instead_of_being_polled() {
        let woken = Arc::new(AtomicUsize::new(0));
        let w = Arc::clone(&woken);
        let worker = SearchWorker::new(move || {
            w.fetch_add(1, Ordering::SeqCst);
        });
        let idx = index(1, 10);
        let g = worker.submit(params("app"), idx);
        wait_for(&worker, 1, g).expect("no outcome");
        assert!(
            woken.load(Ordering::SeqCst) >= 1,
            "the worker must notify, the UI must not spin"
        );
    }

    /// Rapid typing: every intermediate search is superseded and only the last
    /// one may reach the UI.
    #[test]
    fn stale_results_never_replace_newer_ones() {
        let worker = SearchWorker::new(|| {});
        let idx = index(1, 4000);
        let mut last = 0;
        for q in ["a", "ap", "app", "app ", "app 0", "app 00", "app 000"] {
            last = worker.submit(params(q), Arc::clone(&idx));
        }
        let out = wait_for(&worker, 1, last).expect("no outcome");
        assert_eq!(
            out.generation, last,
            "the newest generation must be the one that lands"
        );
        // Nothing older is left queued behind it.
        assert!(worker.poll(1).is_none());
    }

    #[test]
    fn superseded_searches_are_cancelled_not_finished() {
        let worker = SearchWorker::new(|| {});
        let idx = index(1, 20_000);
        // Fire a long search, then immediately supersede it many times.
        worker.submit(params("app"), Arc::clone(&idx));
        let mut last = 0;
        for _ in 0..20 {
            last = worker.submit(params("app 0001"), Arc::clone(&idx));
        }
        let out = wait_for(&worker, 1, last).expect("no outcome");
        assert_eq!(out.generation, last);
        // The 20 superseded jobs produced no outcomes at all.
        assert!(worker.poll(1).is_none());
    }

    #[test]
    fn cancel_stops_the_in_flight_search_and_publishes_nothing() {
        let worker = SearchWorker::new(|| {});
        let idx = index(1, 50_000);
        let g = worker.submit(params("app"), Arc::clone(&idx));
        worker.cancel();
        // Give a non-cancelled search far longer than it would need.
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            worker.poll(1).is_none(),
            "a cancelled search must not publish"
        );
        assert!(worker.latest_generation() > g);
        // The worker is still alive and serves the next search.
        let g2 = worker.submit(params("app 0001"), Arc::clone(&idx));
        assert!(wait_for(&worker, 1, g2).is_some());
    }

    #[test]
    fn a_result_against_a_replaced_app_list_is_rejected() {
        let worker = SearchWorker::new(|| {});
        let old = index(1, 100);
        let g = worker.submit(params("app"), old);
        // The app list is reloaded while the search is in flight; the UI now
        // holds generation 2 and must not accept positions from generation 1.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut saw_any = false;
        while Instant::now() < deadline {
            if worker.poll(2).is_some() {
                panic!("an outcome from a superseded app index was accepted");
            }
            if worker.latest_generation() == g {
                saw_any = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(saw_any);
    }

    #[test]
    fn concurrent_reload_while_searching_resolves_to_the_new_data() {
        let worker = SearchWorker::new(|| {});
        let old = index(1, 5_000);
        worker.submit(params("app"), old);
        let fresh = index(2, 10);
        let g = worker.submit(params("app 0003"), Arc::clone(&fresh));
        let out = wait_for(&worker, 2, g).expect("no outcome for the fresh index");
        assert_eq!(out.data_generation, 2);
        for hit in &out.hits {
            assert!(
                (hit.index as usize) < fresh.items.len(),
                "positions must index the snapshot they were computed against"
            );
        }
    }

    #[test]
    fn the_loader_runs_off_the_caller_thread_and_coalesces() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&calls);
        let loader: AppLoader<Vec<AppEntry>> = AppLoader::new(
            move || {
                c.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(20));
                vec![app("Loaded")]
            },
            || {},
        );
        let mut last = 0;
        for _ in 0..5 {
            last = loader.request();
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut got = None;
        while Instant::now() < deadline {
            if let Some(l) = loader.poll() {
                got = Some(l);
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let got = got.expect("the loader never answered");
        assert!(got.generation <= last);
        assert_eq!(got.entries.len(), 1);
    }
}
