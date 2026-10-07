//! Shared cooperative invocation cancellation.

use std::future::Future;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::EventStream;

/// Cooperative signal independent of reader and result-observer lifetimes.
#[derive(Clone)]
pub struct Cancellation {
    aborted: Arc<AtomicBool>,
    notification: EventStream<(), ()>,
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
            notification: EventStream::new(|()| false, |()| ()),
        }
    }

    /// Abort this signal and notify all observers; repeated calls do nothing.
    pub fn abort(&self) {
        if !self.aborted.swap(true, Ordering::AcqRel) {
            self.notification.end(Some(()));
        }
    }

    /// Return whether this signal or any clone has been aborted.
    #[must_use]
    pub fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::Acquire)
    }

    /// Observe cancellation without taking ownership of request work.
    pub fn cancelled(&self) -> impl Future<Output = ()> + Send + 'static {
        self.notification.result()
    }
}
