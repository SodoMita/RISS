//! Test-only allocator that counts allocations, so "the hot loops stopped
//! allocating" is a number a CI job can fail on rather than a claim in a PR.
//!
//! Compiled only into the test target: the `#[global_allocator]` is a
//! process-wide switch, and this module is never reached by a release build.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

thread_local! {
    /// Only the thread inside `measure` counts. A process-wide counter would
    /// also charge the other cargo-test threads for their own allocations, which
    /// is exactly the noise that made an early version of this guard report
    /// 15_218 allocations for a call that performs 145.
    static MEASURING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if MEASURING.with(|flag| flag.get()) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if MEASURING.with(|flag| flag.get()) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(new_size, Ordering::Relaxed);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// Run `f` and return how many allocations it performed and how many bytes it
/// asked for. Reset immediately before, so only `f` is measured.
pub fn measure<T>(f: impl FnOnce() -> T) -> (T, usize, usize) {
    ALLOCS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    let out = MEASURING.with(|flag| {
        flag.set(true);
        let out = f();
        flag.set(false);
        out
    });
    (
        out,
        ALLOCS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    )
}
