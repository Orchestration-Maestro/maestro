//! Input admission without running channel wakers under a guard.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::Notify;

/// High bit closes admission; remaining bits count admitted submissions.
const CLOSED: usize = 1 << (usize::BITS - 1);

/// Atomically admits a submission before its channel can call a waker.
pub(super) struct Admission {
    /// Closed bit and admitted submission count.
    state: AtomicUsize,
    /// Wakes the sole worker when the last admitted submission settles.
    drained: Notify,
}

impl Admission {
    /// Creates an initially open or closed admission owner.
    pub(super) fn new(open: bool) -> Self {
        Self {
            state: AtomicUsize::new(if open { 0 } else { CLOSED }),
            drained: Notify::new(),
        }
    }

    /// Observes the closed bit independently of outstanding submissions.
    pub(super) fn closed(&self) -> bool {
        self.state.load(Ordering::Acquire) & CLOSED != 0
    }

    /// Admits one call only while the closed bit is clear.
    pub(super) fn enter(self: &Arc<Self>) -> Option<Admitted<'_>> {
        self.state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| {
                (state & CLOSED == 0).then_some(state + 1)
            })
            .ok()?;
        Some(Admitted(self))
    }

    /// Prevents future admission without interrupting admitted channel sends.
    pub(super) fn close(&self) {
        self.state.fetch_or(CLOSED, Ordering::AcqRel);
    }
}

/// One in-flight submission; release never holds a mutex while waking.
pub(super) struct Admitted<'a>(&'a Admission);
impl Drop for Admitted<'_> {
    fn drop(&mut self) {
        if self.0.state.fetch_sub(1, Ordering::AcqRel) == CLOSED + 1 {
            self.0.drained.notify_one();
        }
    }
}

/// Independent terminal controls.
pub(super) enum Control {
    /// Drain accepted input and pause terminal reads.
    Begin(Reply<()>),
    /// Duplicate a quiesced master with owned close-on-exec lifetime.
    Duplicate(Reply<std::os::fd::OwnedFd>),
    /// Read the master foreground process group.
    Foreground(tokio::sync::oneshot::Sender<Option<u32>>),
    /// Resume the worker after a failed handoff.
    Rollback(Reply<()>),
    /// Stop I/O before acknowledging committed release.
    Release(Reply<()>),
    /// Stop all worker I/O.
    Shutdown,
}

/// Coalesced geometry controls and ordered terminal responses.
#[derive(Default)]
pub(super) struct Pending {
    /// Latest resize with replies emitted only after its ioctl.
    pub(super) resize: Option<(crate::terminal::PtySize, Vec<Vec<u8>>)>,
    /// Latest redraw nudge.
    pub(super) nudge: Option<crate::terminal::PtySize>,
    /// Ordinary replies in publication order.
    pub(super) responses: Vec<Vec<u8>>,
}

/// Shared controls and synchronous callback ordering.
#[derive(Default)]
pub(super) struct Controls {
    /// Native-value controls, never supplied callback objects.
    pub(super) pending: std::sync::Mutex<Pending>,
    /// Wakes the worker after a control update.
    pub(super) changed: Notify,
    /// Serializes callback effects without retaining a mutex guard.
    pub(super) order: ResponseOrder,
}

/// A callback lease, with the mutex used only to transfer ownership.
#[derive(Default)]
pub(super) struct ResponseOrder {
    /// Whether a callback currently owns the ordered response lane.
    busy: std::sync::Mutex<bool>,
    /// Notifies synchronous contenders after the lease ends.
    available: std::sync::Condvar,
}

impl ResponseOrder {
    /// Takes the lane and releases the mutex before returning to caller code.
    pub(super) fn enter(&self) -> ResponseLease<'_> {
        let mut busy = self
            .busy
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while *busy {
            busy = self
                .available
                .wait(busy)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *busy = true;
        ResponseLease(self)
    }
}

/// Releases the lane even when a synchronous callback unwinds.
pub(super) struct ResponseLease<'a>(&'a ResponseOrder);
impl Drop for ResponseLease<'_> {
    fn drop(&mut self) {
        *self
            .0
            .busy
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = false;
        self.0.available.notify_one();
    }
}

/// One result reply owned by an independent control command.
pub(super) type Reply<T> = tokio::sync::oneshot::Sender<std::io::Result<T>>;

impl Admission {
    /// Reopens admission only after rollback has been acknowledged.
    pub(super) fn reopen(&self) {
        self.state.fetch_and(!CLOSED, Ordering::AcqRel);
    }

    /// Waits for admitted channel sends to settle before draining accepted bytes.
    pub(super) async fn wait_drained(&self) {
        loop {
            let notified = self.drained.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.state.load(Ordering::Acquire) & !CLOSED == 0 {
                return;
            }
            notified.await;
        }
    }
}
