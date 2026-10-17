//! The process-wide lock that keeps owner launches from overlapping a live pipe.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Held by every launch through the owner, and by a live child until it is waited for.
static SPAWN: Mutex<()> = Mutex::new(());

/// Take the lock so no sibling test forks while this launch's pipes exist.
///
/// A forked child inherits every open pipe end until it execs, so a sibling fork overlapping
/// a live transaction would keep a pipe the parent dropped open. Launches built outside the
/// owner are not covered. A test must not start another owner launch on the same thread
/// while it holds a live child.
pub(crate) fn acquire() -> MutexGuard<'static, ()> {
    SPAWN.lock().unwrap_or_else(PoisonError::into_inner)
}
