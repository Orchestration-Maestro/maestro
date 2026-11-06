//! Scoped worker threads that apply one operation to every item of a slice.

use std::num::NonZero;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Apply `work` to every item on one scoped thread per available core, in item order.
///
/// Each thread takes the next unclaimed item, so one slow item delays only its own thread.
///
/// # Errors
///
/// Returns a message when a worker thread panicked.
pub(crate) fn map<T, R>(items: &[T], work: impl Fn(&T) -> R + Sync) -> Result<Vec<R>, String>
where
    T: Sync,
    R: Send,
{
    let next = AtomicUsize::new(0);
    let drain = || {
        let mut done = Vec::new();
        loop {
            let index = next.fetch_add(1, Ordering::Relaxed);
            let Some(item) = items.get(index) else {
                return done;
            };
            done.push((index, work(item)));
        }
    };
    let threads = std::thread::available_parallelism()
        .map_or(1, NonZero::get)
        .min(items.len());
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads).map(|_| scope.spawn(drain)).collect();
        let mut results = Vec::with_capacity(items.len());
        for worker in workers {
            results.extend(
                worker
                    .join()
                    .map_err(|_| "a worker thread panicked".to_owned())?,
            );
        }
        results.sort_unstable_by_key(|(index, _)| *index);
        Ok(results.into_iter().map(|(_, result)| result).collect())
    })
}

#[cfg(test)]
mod tests;
