//! Scoped tracing subscribers for tests.
//!
//! `tracing` caches callsite interest process-wide and rebuilds it when
//! dispatchers register or the interest cache is explicitly rebuilt. While only
//! one subscriber is registered, it asks only the registering thread's own
//! subscriber. A sibling test thread without a subscriber can therefore cache
//! disabled interest for a callsite and prevent scoped capture until a rebuild.
//! [`with_default`] and [`with_subscriber`] keep a second subscriber
//! registered, so every live subscriber is asked. A formatting subscriber
//! writes into an [`Output`].

use std::{
    io::{self, Write},
    sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError},
};
use tracing::{
    Dispatch,
    instrument::{WithDispatch, WithSubscriber},
    subscriber::NoSubscriber,
};

/// The second subscriber, registered for the life of the process; it records nothing.
static KEPT: LazyLock<Dispatch> = LazyLock::new(|| Dispatch::new(NoSubscriber::default()));

/// Bytes a formatting subscriber writes, shared by every clone.
///
/// Give the subscriber's writer a clone and read the capture with [`Output::text`].
#[derive(Clone, Default)]
pub struct Output(Arc<Mutex<Vec<u8>>>);

impl Output {
    /// Text written so far.
    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes()).into_owned()
    }

    /// Forget everything written so far.
    pub fn clear(&self) {
        self.bytes().clear();
    }

    /// Captured bytes, still readable after a writer panicked.
    fn bytes(&self) -> MutexGuard<'_, Vec<u8>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Run `action` with `subscriber` as this thread's default subscriber.
///
/// `subscriber` receives this thread's events even from callsites another thread
/// reached first.
pub fn with_default<T>(
    subscriber: impl tracing::Subscriber + Send + Sync + 'static,
    action: impl FnOnce() -> T,
) -> T {
    LazyLock::force(&KEPT);
    tracing::subscriber::with_default(subscriber, action)
}

/// Make `subscriber` the default subscriber whenever `future` is polled.
///
/// As with [`with_default`], `subscriber` receives the future's events even from
/// callsites another thread reached first. It is dropped with the returned future.
pub fn with_subscriber<F>(
    future: F,
    subscriber: impl tracing::Subscriber + Send + Sync + 'static,
) -> WithDispatch<F> {
    LazyLock::force(&KEPT);
    future.with_subscriber(subscriber)
}
