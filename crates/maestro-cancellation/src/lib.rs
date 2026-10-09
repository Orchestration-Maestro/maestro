//! Shared cooperative invocation cancellation.

use std::future::Future;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::sync::Notify;

/// Cooperative signal independent of reader and result-observer lifetimes.
#[derive(Clone)]
pub struct Cancellation {
    /// Shared flag marking cancellation across cloned handles.
    aborted: Arc<AtomicBool>,
    /// Notification shared by observers of cancellation.
    notification: Arc<Notify>,
}

impl Default for Cancellation {
    fn default() -> Self {
        Self::new()
    }
}

impl Cancellation {
    /// Construct a signal that has not been aborted.
    #[must_use]
    pub fn new() -> Self {
        Self {
            aborted: Arc::new(AtomicBool::new(false)),
            notification: Arc::new(Notify::new()),
        }
    }

    /// Abort this signal and notify all observers; repeated calls do nothing.
    pub fn abort(&self) {
        if !self.aborted.swap(true, Ordering::AcqRel) {
            self.notification.notify_waiters();
        }
    }

    /// Return whether this signal or any clone has been aborted.
    #[must_use]
    pub fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::Acquire)
    }

    /// Observe cancellation without taking ownership of request work.
    pub fn cancelled(&self) -> impl Future<Output = ()> + Send + 'static {
        let notification = Arc::clone(&self.notification).notified_owned();
        let aborted = self.is_aborted();
        async move {
            if !aborted {
                notification.await;
            }
        }
    }
}
