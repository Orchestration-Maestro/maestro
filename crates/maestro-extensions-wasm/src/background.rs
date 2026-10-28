//! Futures the host runs on its event-tail executor, each until it ends or its task drops.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use std::cell::RefCell;
use std::future::poll_fn;
use std::rc::Rc;
use std::task::{Poll, Waker};

use crate::{EventBus, EventListener, ExtensionFuture, ExtensionResult};

/// The channel each start announces its work on.
const CHANNEL: &str = "maestro.background";

/// One piece of started work and the host tail that polls it.
struct Job {
    /// The work, until it ends or its task stops it.
    running: Option<ExtensionFuture<'static, ()>>,
    /// Wakes the host tail once the task stops the work.
    waker: Option<Waker>,
}

/// Work shared by its task and its host tail.
type SharedJob = Rc<RefCell<Job>>;

/// Starts background work for one extension.
///
/// A component has no task of its own that outlives a callback. Each start announces its
/// work on the event bus, and the listener this value registered returns the work as that
/// emission's tail, which the host polls independently. Clones share the listener.
#[derive(Clone)]
pub struct Background {
    /// The bus the work is announced on.
    events: EventBus,
    /// Work announced and not yet claimed by the listener.
    pending: Rc<RefCell<Option<SharedJob>>>,
}

impl Background {
    /// Registers the listener that hands started work to the host.
    ///
    /// # Errors
    /// Returns the host's registration rejection.
    pub fn new(events: EventBus) -> ExtensionResult<Self> {
        let pending = Rc::default();
        events.on(CHANNEL, listener(Rc::clone(&pending)))?;
        Ok(Self { events, pending })
    }

    /// Hands `work` to the host; it runs until it ends or the returned task drops.
    ///
    /// A failure of the work goes to the host's event-bus reporter.
    ///
    /// # Errors
    /// Returns the host's refusal of the announcement; the work then never runs.
    #[must_use]
    pub fn spawn(&self, work: ExtensionFuture<'static, ()>) -> ExtensionFuture<'_, BackgroundTask> {
        Box::pin(async move {
            let job = Rc::new(RefCell::new(Job {
                running: Some(work),
                waker: None,
            }));
            self.pending.replace(Some(Rc::clone(&job)));
            if let Err(error) = self.events.emit(CHANNEL, serde_json::Value::Null).await {
                self.pending.take();
                return Err(error);
            }
            Ok(BackgroundTask(job))
        })
    }
}

/// Returns each announced job as its emission's tail; other emissions on the channel find
/// nothing to claim.
fn listener(pending: Rc<RefCell<Option<SharedJob>>>) -> EventListener {
    Rc::new(move |_, _| {
        let Some(job) = pending.borrow_mut().take() else {
            return Ok(None);
        };
        Ok(Some(Box::pin(poll_fn(move |cx| {
            let running = {
                let mut job = job.borrow_mut();
                job.waker = Some(cx.waker().clone());
                job.running.take()
            };
            let Some(mut running) = running else {
                job.borrow_mut().waker = None;
                return Poll::Ready(Ok(()));
            };
            // The work is out of its cell while it runs, so it may stop its own task.
            let result = running.as_mut().poll(cx);
            let mut job = job.borrow_mut();
            if result.is_pending() && job.waker.is_some() {
                job.running = Some(running);
                return Poll::Pending;
            }
            job.waker = None;
            match result {
                Poll::Ready(result) => Poll::Ready(result),
                Poll::Pending => Poll::Ready(Ok(())),
            }
        }))))
    })
}

/// Started background work; dropping it stops the work and settles its host tail.
pub struct BackgroundTask(SharedJob);

impl Drop for BackgroundTask {
    fn drop(&mut self) {
        let mut job = self.0.borrow_mut();
        job.running = None;
        if let Some(waker) = job.waker.take() {
            waker.wake();
        }
    }
}
