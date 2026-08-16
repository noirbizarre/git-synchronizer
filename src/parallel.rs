//! A minimal, dependency-free parallel map over a work queue.
//!
//! Analysis spends nearly all its time waiting on `git` subprocesses that are
//! independent of one another, so overlapping them is close to free. What is
//! *not* negotiable is determinism: `git sync` must produce byte-identical
//! output whatever the job count.
//!
//! Determinism is guaranteed by construction rather than by discipline: workers
//! pull indices off a shared cursor, and every result is filed back under the
//! index it came from. The caller therefore always reduces results in input
//! order, and nothing a worker does may depend on the order in which items are
//! picked up.
//!
//! Only read-only work belongs here. Fetch, pull, branch deletion and worktree
//! removal stay strictly serial: concurrent writers to the same repository
//! corrupt its state.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

/// Apply `f` to every item of `items`, returning the results in input order.
///
/// `jobs` caps the number of worker threads. `jobs <= 1` (and any input short
/// enough not to be worth a thread) runs entirely on the calling thread, which
/// is both faster and exactly what `--jobs 1` promises.
///
/// `f` receives the index alongside the item so workers can key their own
/// bookkeeping without relying on execution order.
pub fn map<T, R, F>(items: &[T], jobs: usize, f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(usize, &T) -> R + Sync,
{
    if jobs <= 1 || items.len() <= 1 {
        return items.iter().enumerate().map(|(i, it)| f(i, it)).collect();
    }

    let workers = jobs.min(items.len());
    let cursor = AtomicUsize::new(0);
    let f = &f;
    let cursor = &cursor;

    // Each worker accumulates `(index, result)` locally, so the only shared
    // mutable state is the cursor. Results are merged once every thread has
    // joined, then placed back in input order.
    let collected: Vec<Vec<(usize, R)>> = thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(move || {
                    let mut local = Vec::new();
                    loop {
                        let index = cursor.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(index) else {
                            break;
                        };
                        local.push((index, f(index, item)));
                    }
                    local
                })
            })
            .collect();

        handles
            .into_iter()
            // A panicking worker is a bug, not a git failure: every closure
            // used here returns its errors as values. Propagate it.
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|e| std::panic::resume_unwind(e))
            })
            .collect()
    });

    let mut slots: Vec<Option<R>> = (0..items.len()).map(|_| None).collect();
    for (index, result) in collected.into_iter().flatten() {
        slots[index] = Some(result);
    }
    slots
        .into_iter()
        .map(|slot| slot.expect("every index is handed to exactly one worker"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The contract that matters: parallelism never reorders results.
    #[test]
    fn results_are_in_input_order_whatever_the_job_count() {
        let items: Vec<usize> = (0..64).collect();
        let expected: Vec<usize> = items.iter().map(|n| n * 2).collect();

        for jobs in [1, 2, 3, 8, 64, 128] {
            let got = map(&items, jobs, |_, n| n * 2);
            assert_eq!(got, expected, "job count {jobs} changed the result order");
        }
    }

    #[test]
    fn index_matches_position() {
        let items: Vec<&str> = vec!["a", "b", "c", "d", "e"];
        let got = map(&items, 4, |i, s| format!("{i}{s}"));
        assert_eq!(got, vec!["0a", "1b", "2c", "3d", "4e"]);
    }

    #[test]
    fn empty_input_spawns_nothing() {
        let items: Vec<usize> = Vec::new();
        assert!(map(&items, 8, |_, n| *n).is_empty());
    }

    #[test]
    fn every_item_is_visited_exactly_once() {
        use std::sync::Mutex;

        let items: Vec<usize> = (0..500).collect();
        let visits = Mutex::new(Vec::new());
        map(&items, 8, |_, n| visits.lock().unwrap().push(*n));

        let mut visited = visits.into_inner().unwrap();
        visited.sort_unstable();
        assert_eq!(visited, items);
    }
}
