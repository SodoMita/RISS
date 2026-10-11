//! Test-only allocator that counts allocations, so "the hot loops stopped
//! allocating" is a number CI can fail on rather than a claim in a PR.
//!
//! Compiled only into the test target: a `#[global_allocator]` is a
//! process-wide switch, and nothing here is reachable from a release build.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    /// Counting is per-thread and opt-in, and both halves matter.
    ///
    /// The first version gated only on "is this thread measuring", but kept a
    /// single process-wide counter. That was wrong in a way worth writing
    /// down: as soon as two measured tests ran concurrently -- which is the
    /// default under `cargo test` -- each charged its allocations to the shared
    /// total and each `measure` reset it for the other, so an unrelated test
    /// could fail a bound by existing. A guard that fails because a neighbour
    /// ran is a guard people learn to ignore, so the counts live here instead.
    static COUNTING: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
    static MEASURING: Cell<bool> = const { Cell::new(false) };
}

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if MEASURING.with(|flag| flag.get()) {
            COUNTING.with(|c| {
                let (n, b) = c.get();
                c.set((n + 1, b + layout.size()));
            });
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if MEASURING.with(|flag| flag.get()) {
            COUNTING.with(|c| {
                let (n, b) = c.get();
                c.set((n + 1, b + new_size));
            });
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static COUNTING_ALLOCATOR: Counting = Counting;

/// Run `f` and return how many allocations it performed and how many bytes it
/// asked for, counting only this thread and only inside `measure`.
pub fn measure<T>(f: impl FnOnce() -> T) -> (T, usize, usize) {
    COUNTING.with(|c| c.set((0, 0)));
    let out = MEASURING.with(|flag| {
        flag.set(true);
        let out = f();
        flag.set(false);
        out
    });
    let (allocs, bytes) = COUNTING.with(|c| c.get());
    (out, allocs, bytes)
}
