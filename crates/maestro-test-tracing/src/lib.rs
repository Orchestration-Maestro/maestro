//! Scoped tracing subscribers for tests.
//!
//! `tracing` decides once, for the whole process, whether a callsite is enabled,
//! when a thread first reaches it. While only one subscriber is registered, it
//! asks only the reaching thread's own subscriber. So when a test captures events
//! through a scoped subscriber and a sibling test thread without one reaches the
//! same callsite first, the callsite stays off and the capture misses its events.
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
