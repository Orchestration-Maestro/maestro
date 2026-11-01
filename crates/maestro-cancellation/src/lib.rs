//! Shared cooperative invocation cancellation.

mod observers;

use std::future::Future;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::sync::Notify;

/// Cooperative signal independent of reader and result-observer lifetimes.
#[derive(Clone)]
pub struct Cancellation {
    /// State shared by every cloned handle.
    shared: Arc<Shared>,
}

/// The flag and notification shared by the clones of one signal.
struct Shared {
    /// Marks cancellation across cloned handles.
    aborted: AtomicBool,
    /// Notification shared by observers of cancellation.
    notification: Arc<Notify>,
}

impl Shared {
    /// Whether the signal has been aborted.
    fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::Acquire)
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        observers::release(self);
    }
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
            shared: Arc::new(Shared {
                aborted: AtomicBool::new(false),
                notification: Arc::new(Notify::new()),
            }),
        }
    }

    /// Abort this signal, notify all waiters and run this thread's observers.
    ///
    /// The first call marks the signal and wakes its waiters. Every call then runs,
    /// before returning, the observers registered on the calling thread that have not
    /// run yet, so a repeated call on the same thread does nothing.
    pub fn abort(&self) {
        if !self.shared.aborted.swap(true, Ordering::AcqRel) {
            self.shared.notification.notify_waiters();
        }
        observers::deliver(&self.shared);
    }

    /// Return whether this signal or any clone has been aborted.
    #[must_use]
    pub fn is_aborted(&self) -> bool {
        self.shared.is_aborted()
    }

    /// Observe cancellation without taking ownership of request work.
    pub fn cancelled(&self) -> impl Future<Output = ()> + Send + 'static {
        let notification = Arc::clone(&self.shared.notification).notified_owned();
        let aborted = self.is_aborted();
        async move {
            if !aborted {
                notification.await;
            }
        }
    }

    /// Runs `observer` once when this signal is aborted, and returns its removal.
    ///
    /// Observers belong to the thread that registers them and run in registration
    /// order, each callback identity once, inside the [`Cancellation::abort`] call
    /// that reaches them on that thread. An observer may abort the signal again,
    /// which does nothing, and may register or remove observers; the running walk
    /// sees those changes. A registration after the signal was aborted, outside
    /// such a walk, never runs. Dropping the returned removal keeps the observer;
    /// calling it removes the observer, and later calls do nothing.
    ///
    /// The signal crosses threads while its observers cannot, so an abort on
    /// another thread only wakes the waiters of [`Cancellation::cancelled`]; the
    /// owning thread runs its observers when it calls [`Cancellation::abort`]
    /// itself, typically from such a waiter.
    #[must_use]
    pub fn subscribe(&self, observer: Rc<dyn Fn()>) -> Box<dyn Fn()> {
        observers::subscribe(&self.shared, observer)
    }
}
