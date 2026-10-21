//! Scoped tracing subscribers for tests.
//!
//! `tracing` caches callsite interest process-wide and rebuilds it when
//! dispatchers register or the interest cache is explicitly rebuilt. While only
//! one subscriber is registered, it asks only the registering thread's own
//! subscriber. A sibling test thread without a subscriber can therefore cache
//! disabled interest for a callsite and prevent scoped capture until a rebuild.
//! [`with_default`] keeps a second subscriber registered, so every live
//! subscriber is asked.

use std::sync::LazyLock;
use tracing::{Dispatch, subscriber::NoSubscriber};

/// Run `action` with `subscriber` as this thread's default subscriber.
///
/// `subscriber` receives this thread's events even from callsites another thread
/// reached first.
pub fn with_default<T>(
    subscriber: impl tracing::Subscriber + Send + Sync + 'static,
    action: impl FnOnce() -> T,
) -> T {
    static KEPT: LazyLock<Dispatch> = LazyLock::new(|| Dispatch::new(NoSubscriber::default()));
    LazyLock::force(&KEPT);
    tracing::subscriber::with_default(subscriber, action)
}
